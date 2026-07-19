//! Turret traverse, gun laying, and the two weapons.
//!
//! The turret yaws toward the aim point at a fixed traverse rate, and the gun
//! auto-elevates to the ballistic angle that best hits the target. Pressing
//! FIRE only *requests* a shot — the gun actually fires once it is loaded and
//! laid on target (turret aligned and gun at the right elevation). Rates scale
//! with the crew/vehicle `condition`, so a damaged or heavier tank lays and
//! reloads more slowly.

use crate::camera::IsoCamera;
use crate::combat::Armor;
use crate::control::PlayerControlled;
use crate::effects::{
    spawn_explosion, spawn_gun_smoke, spawn_impact_puff, spawn_muzzle_flash, EffectAssets, Wreckage,
};
use crate::input::GameInput;
use crate::tank::Tank;
use crate::terrain::{Terrain, MAP_SIZE};
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use bevy::transform::TransformSystem;
use std::f32::consts::{FRAC_PI_4, PI, TAU};
use std::time::Duration;

/// Muzzle velocity of a main-gun shell (world units / second). Ballistic range
/// scales with the square of this, so raising it by √3 (≈90→156) roughly triples
/// the gun's reach (max flat-ground range ≈ SPEED²/GRAVITY, ~270 → ~810 units).
const SHELL_SPEED: f32 = 156.0;
/// Gravity applied to shells (must match the ballistic solver).
const SHELL_GRAVITY: f32 = 30.0;
/// Main-gun HE damage at the point of impact, and the blast radius over which
/// that damage tapers to zero.
const SHELL_DAMAGE: f32 = 70.0;
const SHELL_SPLASH: f32 = 5.0;
/// Machine-gun tracer speed and its limited effective range (world units).
const MG_SPEED: f32 = 130.0;
const MG_RANGE: f32 = 55.0;
/// The hull MG only engages targets within this half-arc of straight ahead.
const MG_ARC_COS: f32 = 0.5; // 60°
/// Turret is "on target" within this yaw error (radians).
const YAW_TOL: f32 = 0.03;
/// Gun is "laid" within this elevation error (radians).
const ELEV_TOL: f32 = 0.02;
/// Enemy gunnery: engagement range, reload time, traverse/elevation rates, and
/// the aim spread (radians) that makes their fire imperfect (more misses at
/// longer range, so the player isn't instantly overwhelmed).
const ENEMY_RANGE: f32 = 230.0;
const ENEMY_RELOAD: f32 = 5.0;
const ENEMY_TRAVERSE: f32 = 0.5;
const ENEMY_GUN_RATE: f32 = 0.4;
const ENEMY_SPREAD: f32 = 0.06;
/// Enemy shells hit softer than the player's, so several tanks firing back stay
/// survivable in the training mission.
const ENEMY_DAMAGE: f32 = 26.0;

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_weapon_assets)
            .add_systems(
                Update,
                (
                    operate_main_gun,
                    enemy_ai,
                    fire_machine_gun,
                    update_projectiles,
                    pulse_marker,
                ),
            )
            // Shake is applied after physics has placed the hull, just before
            // transforms propagate, so the jitter is purely visual.
            .add_systems(
                PostUpdate,
                apply_shake.before(TransformSystem::TransformPropagate),
            );
    }
}

/// A short recoil shake applied to a tank's hull after firing.
#[derive(Component, Default)]
pub struct Shake {
    time: f32,
    duration: f32,
    magnitude: f32,
}

fn apply_shake(time: Res<Time>, mut shakers: Query<(&mut Transform, &mut Shake)>) {
    let dt = time.delta_secs();
    for (mut tf, mut shake) in &mut shakers {
        if shake.time <= 0.0 {
            continue;
        }
        shake.time -= dt;
        let k = (shake.time / shake.duration).clamp(0.0, 1.0);
        let amp = shake.magnitude * k;
        let f = shake.time * 62.0;
        tf.translation.x += amp * (f * 1.1).sin();
        tf.translation.y += amp * 0.6 * (f * 1.7).sin();
        tf.translation.z += amp * (f * 0.9).cos();
        tf.rotate_local_z(amp * 0.06 * f.sin());
    }
}

/// Rotating turret pivot. Traverses toward the aim at `rate` rad/s.
#[derive(Component)]
pub struct Turret {
    /// Traverse rate in radians per second.
    rate: f32,
    /// Current yaw in the hull's local frame.
    yaw: f32,
    /// Whether the turret is currently on target.
    aligned: bool,
}

impl Turret {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            yaw: 0.0,
            aligned: false,
        }
    }
}

/// Gun-elevation pivot. Pitches toward the ballistic angle at `rate` rad/s.
#[derive(Component)]
pub struct GunMount {
    rate: f32,
    min: f32,
    max: f32,
    elev: f32,
    aligned: bool,
    /// Current recoil slide (world units the gun is pushed back).
    recoil: f32,
}

/// Local position of the gun-mount pivot (the trunnion) on the turret.
const GUN_PIVOT: Vec3 = Vec3::new(0.0, 1.5, -0.9);

impl GunMount {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            min: -0.09,      // ~ -5° depression
            max: FRAC_PI_4,  // 45° for maximum range
            elev: 0.0,
            aligned: false,
            recoil: 0.0,
        }
    }
}

/// Empty marker at the main-gun muzzle; its `GlobalTransform` is the shot origin.
#[derive(Component)]
pub struct Muzzle;

/// Hull-mounted machine gun at the front of the tank; fixed to the hull, so it
/// only fires forward. Its `GlobalTransform` is the MG shot origin.
#[derive(Component)]
pub struct HullMg;

/// Per-tank weapon state.
#[derive(Component)]
pub struct Weapons {
    /// Main-gun reload timer (loaded when finished).
    main: Timer,
    /// Machine-gun cadence timer.
    mg: Timer,
    /// A pending "fire the main gun" request, honored when laid and loaded.
    fire_requested: bool,
    /// The committed impact point for the pending shot.
    fire_target: Option<Vec3>,
    /// The solved gun elevation for the committed shot.
    fire_elev: f32,
    /// The on-ground marker entity shown while a shot is pending.
    marker: Option<Entity>,
    /// Crew/vehicle condition in 0..1; scales traverse, elevation, and reload.
    /// Written by the combat system as the tank takes damage.
    pub condition: f32,
}

impl Default for Weapons {
    fn default() -> Self {
        let mut main = Timer::from_seconds(2.5, TimerMode::Once);
        main.tick(Duration::from_secs(5)); // start loaded
        let mut mg = Timer::from_seconds(0.09, TimerMode::Once);
        mg.tick(Duration::from_secs(5));
        Self {
            main,
            mg,
            fire_requested: false,
            fire_target: None,
            fire_elev: 0.0,
            marker: None,
            condition: 1.0,
        }
    }
}

/// The reverse-shockwave marker on the target: a ground ring plus a low dome.
/// It owns its own materials so it can pulse and fade independently of any
/// later marker.
#[derive(Component)]
pub struct TargetMarker {
    age: f32,
    ring_mat: Handle<StandardMaterial>,
    dome_mat: Handle<StandardMaterial>,
    base_y: f32,
}

/// Added to a marker once the shot is away: it expands, rises, and fades out.
#[derive(Component)]
pub struct MarkerFading {
    t: f32,
}

#[derive(Component)]
struct Shell {
    vel: Vec3,
    /// True for enemy shells (they damage the player); false for the player's
    /// shells (they damage enemy tanks).
    hits_player: bool,
    /// Peak HE damage at the point of impact (enemy shells hit softer).
    damage: f32,
}

/// Fire-control for an enemy tank: the AI traverses `turret`, elevates `gun`,
/// and fires shells from `muzzle` at the player. (Enemies deliberately do NOT
/// carry the player's `Turret`/`GunMount`/`Muzzle` components, so the player's
/// singleton weapon queries stay unique — this drives them by entity instead.)
#[derive(Component)]
pub struct EnemyGunner {
    turret: Entity,
    gun: Entity,
    muzzle: Entity,
    yaw: f32,
    elev: f32,
    reload: Timer,
}

impl EnemyGunner {
    pub fn new(turret: Entity, gun: Entity, muzzle: Entity) -> Self {
        Self {
            turret,
            gun,
            muzzle,
            yaw: 0.0,
            elev: 0.0,
            reload: Timer::from_seconds(ENEMY_RELOAD, TimerMode::Once),
        }
    }
}

#[derive(Component)]
struct Tracer {
    vel: Vec3,
    life: f32,
}

#[derive(Resource)]
struct WeaponAssets {
    shell_mesh: Handle<Mesh>,
    shell_mat: Handle<StandardMaterial>,
    tracer_mesh: Handle<Mesh>,
    tracer_mat: Handle<StandardMaterial>,
    /// Flat ring that lies on the ground under the aim point.
    marker_ring_mesh: Handle<Mesh>,
    /// Low translucent dome above the ring.
    marker_dome_mesh: Handle<Mesh>,
}

fn setup_weapon_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Projectiles are little polygon bolts, not spheres.
    let shell_mesh = meshes.add(Cuboid::new(0.22, 0.22, 0.5));
    let shell_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.18, 0.15),
        emissive: LinearRgba::rgb(1.2, 0.5, 0.15),
        ..default()
    });
    let tracer_mesh = meshes.add(Cuboid::new(0.1, 0.1, 0.6));
    let tracer_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.4),
        emissive: LinearRgba::rgb(4.0, 3.0, 0.6),
        unlit: true,
        ..default()
    });
    // Target marker: a flat ring lying on the ground with a low translucent dome
    // over it — a shockwave run in reverse, collapsing inward toward the aim
    // point on the surface. Each marker gets its own materials (see
    // operate_main_gun) so it can fade independently. A Bevy `Torus` already lies
    // flat in the XZ plane, so it reads as a ring painted on the ground.
    let marker_ring_mesh = meshes.add(Torus::new(0.74, 1.0));
    let marker_dome_mesh = meshes.add(Sphere::new(1.0));
    commands.insert_resource(WeaponAssets {
        shell_mesh,
        shell_mat,
        tracer_mesh,
        tracer_mat,
        marker_ring_mesh,
        marker_dome_mesh,
    });
}

/// Traverse the turret, lay the gun, and fire the main gun when ready.
#[allow(clippy::too_many_arguments)]
fn operate_main_gun(
    mut commands: Commands,
    time: Res<Time>,
    input: Res<GameInput>,
    cameras: Query<(&Camera, &GlobalTransform), With<IsoCamera>>,
    terrain: Option<Res<Terrain>>,
    assets: Res<WeaponAssets>,
    effects: Option<Res<EffectAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    muzzles: Query<&GlobalTransform, With<Muzzle>>,
    mut roots: Query<(&GlobalTransform, &mut Weapons, &mut Shake), With<PlayerControlled>>,
    mut turrets: Query<(&mut Transform, &mut Turret), Without<GunMount>>,
    mut guns: Query<(&mut Transform, &mut GunMount), Without<Turret>>,
) {
    let (
        Ok((root_gt, mut weapon, mut shake)),
        Ok((mut turret_tf, mut turret)),
        Ok((mut gun_tf, mut gun)),
    ) = (
        roots.get_single_mut(),
        turrets.get_single_mut(),
        guns.get_single_mut(),
    ) else {
        return;
    };

    let dt = time.delta_secs().min(1.0 / 20.0);
    let cond = weapon.condition.clamp(0.05, 1.0);
    // Reload progresses slower when the tank is in poor condition.
    weapon.main.tick(Duration::from_secs_f32(dt * cond));

    // The target is exactly the ground point under the cursor/tap.
    let live_target = input
        .aim
        .zip(cameras.get_single().ok())
        .zip(terrain.as_ref())
        .and_then(|((screen, (camera, cam_tf)), t)| cursor_ground(screen, camera, cam_tf, t));

    let (_, root_rot, root_pos) = root_gt.to_scale_rotation_translation();

    // Selecting a target (a fresh fire request) commits the aim point, solves a
    // firing elevation that accounts for the terrain along the trajectory, and
    // drops a pulsing marker. The turret does NOT follow the cursor otherwise.
    if input.fire_main && !weapon.fire_requested {
        if let Some(tp) = live_target {
            let launch = muzzles
                .get_single()
                .map(|m| m.translation())
                .unwrap_or(root_pos + Vec3::Y * 1.6);
            weapon.fire_requested = true;
            weapon.fire_target = Some(tp);
            weapon.fire_elev = terrain
                .as_ref()
                .map(|t| solve_elevation(t, launch, tp, SHELL_SPEED, SHELL_GRAVITY, gun.min, gun.max))
                .unwrap_or(0.0);
            // Anchor the marker to the ground surface under the target point.
            let ground_y = terrain
                .as_ref()
                .map(|t| t.height_at(tp.x, tp.z))
                .unwrap_or(tp.y);
            let base_y = ground_y + 0.04;
            // Bright ring painted flat on the ground.
            let ring_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.65, 0.95, 1.0, 0.95),
                emissive: LinearRgba::rgb(1.2, 4.0, 5.6),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                double_sided: true,
                ..default()
            });
            // Low translucent dome over it — the shockwave bubble, sitting on the
            // surface (its lower half is hidden by the terrain).
            let dome_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.55, 0.9, 1.0, 0.45),
                emissive: LinearRgba::rgb(0.8, 2.4, 3.4),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: Some(Face::Front),
                double_sided: true,
                ..default()
            });
            let ring_mesh = assets.marker_ring_mesh.clone();
            let dome_mesh = assets.marker_dome_mesh.clone();
            let (ring_mat_c, dome_mat_c) = (ring_mat.clone(), dome_mat.clone());
            let marker = commands
                .spawn((
                    Transform::from_translation(Vec3::new(tp.x, base_y, tp.z)),
                    Visibility::default(),
                    TargetMarker {
                        age: 0.0,
                        ring_mat: ring_mat_c,
                        dome_mat: dome_mat_c,
                        base_y,
                    },
                ))
                .with_children(|m| {
                    m.spawn((
                        Mesh3d(ring_mesh),
                        MeshMaterial3d(ring_mat),
                        Transform::from_xyz(0.0, 0.02, 0.0),
                    ));
                    // Flatten the dome so it hugs the ground.
                    m.spawn((
                        Mesh3d(dome_mesh),
                        MeshMaterial3d(dome_mat),
                        Transform::from_scale(Vec3::new(1.0, 0.5, 1.0)),
                    ));
                })
                .id();
            weapon.marker = Some(marker);
        }
    }

    // Lay the turret and gun only while a shot is pending; otherwise hold.
    if let (true, Some(tp)) = (weapon.fire_requested, weapon.fire_target) {
        let local = root_rot.inverse() * (tp - root_pos);
        let desired_yaw = (-local.x).atan2(-local.z);
        turret.yaw = step_angle(turret.yaw, desired_yaw, turret.rate * cond * dt);
        turret_tf.rotation = Quat::from_rotation_y(turret.yaw);
        turret.aligned = wrap_pi(desired_yaw - turret.yaw).abs() < YAW_TOL;

        // Once the turret is on target, re-solve the firing elevation from the
        // CURRENT muzzle position. The turret has been traversing since the shot
        // was requested, so the launch point (and its height over the terrain) has
        // moved; solving from the live, aligned muzzle means the gun lays on a
        // solution that matches the geometry at the moment it actually fires — the
        // key to hitting targets across uneven ground. (Gating on `aligned` keeps
        // the per-frame solver cost off the whole traverse.)
        if turret.aligned {
            if let (Ok(muzzle), Some(t)) = (muzzles.get_single(), terrain.as_ref()) {
                weapon.fire_elev = solve_elevation(
                    t,
                    muzzle.translation(),
                    tp,
                    SHELL_SPEED,
                    SHELL_GRAVITY,
                    gun.min,
                    gun.max,
                );
            }
        }

        let desired_elev = weapon.fire_elev;
        gun.elev = step_toward(gun.elev, desired_elev, gun.rate * cond * dt);
        gun_tf.rotation = Quat::from_rotation_x(gun.elev);
        gun.aligned = (gun.elev - desired_elev).abs() < ELEV_TOL;
    } else {
        turret.aligned = false;
        gun.aligned = false;
    }

    // --- Fire when laid and loaded; the marker disappears as the shell leaves ---
    if weapon.fire_requested && weapon.main.finished() && turret.aligned && gun.aligned {
        if let Ok(muzzle) = muzzles.get_single() {
            let (_, muzzle_rot, muzzle_pos) = muzzle.to_scale_rotation_translation();
            let forward = muzzle_rot * Vec3::NEG_Z;
            commands.spawn((
                Mesh3d(assets.shell_mesh.clone()),
                MeshMaterial3d(assets.shell_mat.clone()),
                Transform::from_translation(muzzle_pos),
                Shell {
                    vel: forward * SHELL_SPEED,
                    hits_player: false,
                    damage: SHELL_DAMAGE,
                },
            ));
            // Big muzzle flash, drifting gun smoke, recoil, and a hull shake.
            if let Some(fx) = effects.as_ref() {
                let seed = (time.elapsed_secs() * 733.0) as u32 | 1;
                spawn_muzzle_flash(&mut commands, fx, &mut materials, muzzle_pos, 2.4, seed);
                spawn_gun_smoke(&mut commands, fx, &mut materials, muzzle_pos, forward, seed ^ 0x51);
            }
            gun.recoil = 0.55;
            shake.time = 0.35;
            shake.duration = 0.35;
            shake.magnitude = 0.2;
            weapon.main.reset();
            weapon.fire_requested = false;
            weapon.fire_target = None;
            // The marker fades out after the shot instead of vanishing instantly.
            if let Some(marker) = weapon.marker.take() {
                commands.entity(marker).insert(MarkerFading { t: 0.0 });
            }
        }
    }

    // Recoil slides the gun back, then eases home.
    gun.recoil = (gun.recoil - gun.recoil * 9.0 * dt).max(0.0);
    gun_tf.translation = GUN_PIVOT + Vec3::Z * gun.recoil;
}

/// Enemy tanks acquire the player, traverse their turret and lay their gun on
/// him (accounting for terrain via the same ballistic solver), and fire back on
/// a reload cadence with a little aim spread so they aren't pin-point accurate.
#[allow(clippy::too_many_arguments)]
fn enemy_ai(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    assets: Res<WeaponAssets>,
    effects: Option<Res<EffectAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<(&GlobalTransform, &Armor), With<PlayerControlled>>,
    globals: Query<&GlobalTransform>,
    mut gunners: Query<(&GlobalTransform, &mut EnemyGunner, &Armor), Without<PlayerControlled>>,
    mut transforms: Query<&mut Transform>,
) {
    let Ok((player_gt, player_armor)) = player.get_single() else {
        return;
    };
    if player_armor.destroyed {
        return;
    }
    let player_pos = player_gt.translation();
    let dt = time.delta_secs().min(1.0 / 20.0);

    for (enemy_gt, mut gunner, armor) in &mut gunners {
        gunner.reload.tick(time.delta());
        if armor.destroyed {
            continue;
        }
        let (_, enemy_rot, enemy_pos) = enemy_gt.to_scale_rotation_translation();
        let to_player = player_pos - enemy_pos;
        if to_player.length() > ENEMY_RANGE {
            continue;
        }

        // Traverse the turret toward the player in the hull's local frame.
        let local = enemy_rot.inverse() * to_player;
        let desired_yaw = (-local.x).atan2(-local.z);
        gunner.yaw = step_angle(gunner.yaw, desired_yaw, ENEMY_TRAVERSE * dt);
        if let Ok(mut t) = transforms.get_mut(gunner.turret) {
            t.rotation = Quat::from_rotation_y(gunner.yaw);
        }
        let turret_aligned = wrap_pi(desired_yaw - gunner.yaw).abs() < 0.05;

        // Lay the gun using the terrain-aware ballistic solver, from the muzzle.
        let muzzle_pos = globals
            .get(gunner.muzzle)
            .map(|g| g.translation())
            .unwrap_or(enemy_pos + Vec3::Y * 2.0);
        let desired_elev = terrain
            .as_ref()
            .map(|t| {
                solve_elevation(t, muzzle_pos, player_pos, SHELL_SPEED, SHELL_GRAVITY, -0.09, FRAC_PI_4)
            })
            .unwrap_or(0.0);
        gunner.elev = step_toward(gunner.elev, desired_elev, ENEMY_GUN_RATE * dt);
        if let Ok(mut g) = transforms.get_mut(gunner.gun) {
            g.rotation = Quat::from_rotation_x(gunner.elev);
        }
        let gun_aligned = (gunner.elev - desired_elev).abs() < 0.03;

        // Fire when loaded and laid on target.
        if gunner.reload.finished() && turret_aligned && gun_aligned {
            if let Ok(muzzle_gt) = globals.get(gunner.muzzle) {
                let (_, mrot, mpos) = muzzle_gt.to_scale_rotation_translation();
                // Aim spread: a small deterministic jitter that varies per shot.
                let s = time.elapsed_secs() * 13.0 + mpos.x * 0.7 + mpos.z * 1.3;
                let jitter = Vec3::new(s.sin(), (s * 0.5).sin() * 0.4, (s * 1.7).cos()) * ENEMY_SPREAD;
                let dir = (mrot * Vec3::NEG_Z + jitter).normalize_or_zero();
                commands.spawn((
                    Mesh3d(assets.shell_mesh.clone()),
                    MeshMaterial3d(assets.shell_mat.clone()),
                    Transform::from_translation(mpos),
                    Shell {
                        vel: dir * SHELL_SPEED,
                        hits_player: true,
                        damage: ENEMY_DAMAGE,
                    },
                ));
                if let Some(fx) = effects.as_ref() {
                    let seed = (s * 131.0) as u32 | 1;
                    spawn_muzzle_flash(&mut commands, fx, &mut materials, mpos, 2.0, seed);
                    spawn_gun_smoke(&mut commands, fx, &mut materials, mpos, dir, seed ^ 0x7A);
                }
                gunner.reload.reset();
            }
        }
    }
}

/// The hull machine gun fires short-range tracers forward from the front of the
/// tank; it only engages targets within a forward arc.
#[allow(clippy::too_many_arguments)]
fn fire_machine_gun(
    mut commands: Commands,
    time: Res<Time>,
    input: Res<GameInput>,
    cameras: Query<(&Camera, &GlobalTransform), With<IsoCamera>>,
    terrain: Option<Res<Terrain>>,
    assets: Res<WeaponAssets>,
    effects: Option<Res<EffectAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    hull_mgs: Query<&GlobalTransform, With<HullMg>>,
    mut weapons: Query<&mut Weapons, With<PlayerControlled>>,
) {
    let (Ok(mut weapon), Ok(mg)) = (weapons.get_single_mut(), hull_mgs.get_single()) else {
        return;
    };
    weapon.mg.tick(time.delta());
    if !(input.fire_mg && weapon.mg.finished()) {
        return;
    }
    weapon.mg.reset();

    let (_, mg_rot, mg_pos) = mg.to_scale_rotation_translation();
    // The hull MG is fixed forward (hull local -Z).
    let forward = mg_rot * Vec3::NEG_Z;

    // Aim toward the cursor only if it is within the forward arc; otherwise the
    // co-driver can't bring the gun to bear, so it fires straight ahead.
    let aim_dir = input
        .aim
        .zip(cameras.get_single().ok())
        .zip(terrain.as_ref())
        .and_then(|((screen, (camera, cam_tf)), t)| {
            let target = cursor_ground(screen, camera, cam_tf, t)?;
            (target - mg_pos).try_normalize()
        })
        .filter(|dir| dir.dot(forward) >= MG_ARC_COS)
        .unwrap_or(forward);

    let seed = (time.elapsed_secs() * 971.0) as u32 | 1;
    let flash_pos = mg_pos + forward * 0.4;
    if let Some(fx) = effects.as_ref() {
        spawn_muzzle_flash(&mut commands, fx, &mut materials, flash_pos, 0.4, seed);
    }

    let jitter = Vec3::new(
        (time.elapsed_secs() * 91.0).sin() * 0.03,
        0.0,
        (time.elapsed_secs() * 57.0).cos() * 0.03,
    );
    commands.spawn((
        Mesh3d(assets.tracer_mesh.clone()),
        MeshMaterial3d(assets.tracer_mat.clone()),
        Transform::from_translation(mg_pos + forward * 0.5),
        Tracer {
            vel: (aim_dir + jitter).normalize_or_zero() * MG_SPEED,
            // Limited range: despawns after travelling MG_RANGE.
            life: MG_RANGE / MG_SPEED,
        },
    ));
}

#[allow(clippy::too_many_arguments)]
fn update_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    effects: Option<Res<EffectAssets>>,
    mut wreckage: ResMut<Wreckage>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shells: Query<(Entity, &mut Shell, &mut Transform), Without<Tracer>>,
    mut tracers: Query<(Entity, &mut Tracer, &mut Transform), Without<Shell>>,
    mut targets: Query<(&GlobalTransform, &mut Armor, Has<PlayerControlled>), With<Tank>>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs().min(1.0 / 20.0);
    let limit = MAP_SIZE * 0.5;
    let mut seed = (time.elapsed_secs() * 977.0) as u32;

    for (entity, mut shell, mut tf) in &mut shells {
        shell.vel.y -= SHELL_GRAVITY * dt;
        tf.translation += shell.vel * dt;
        // This shell only affects its target team: enemy shells hit the player,
        // the player's shells hit enemies.
        let hits_player = shell.hits_player;
        let shell_damage = shell.damage;

        // A direct hit on a target tank's hull detonates the shell on contact.
        let mut tank_hit = None;
        for (etf, _, is_player) in targets.iter() {
            if is_player != hits_player {
                continue;
            }
            let c = etf.translation();
            let flat = (tf.translation.x - c.x).hypot(tf.translation.z - c.z);
            let dy = tf.translation.y - c.y;
            if flat < 3.0 && dy > -0.8 && dy < 3.6 {
                tank_hit = Some(tf.translation);
                break;
            }
        }

        let ground = terrain.height_at(tf.translation.x, tf.translation.z);
        let out = tf.translation.x.abs() > limit || tf.translation.z.abs() > limit;
        let hit_ground = tf.translation.y <= ground + 0.1;
        if tank_hit.is_some() || hit_ground || out {
            let at = tank_hit.unwrap_or(Vec3::new(tf.translation.x, ground, tf.translation.z));
            // High-explosive splash: full damage at the impact, tapering to zero
            // at the blast radius, so near-misses still hurt nearby tanks — but
            // only tanks on this shell's target team.
            for (etf, mut armor, is_player) in targets.iter_mut() {
                if is_player != hits_player || armor.destroyed {
                    continue;
                }
                let d = etf.translation().distance(at);
                if d < SHELL_SPLASH {
                    armor.damage(shell_damage * (1.0 - d / SHELL_SPLASH));
                }
            }
            if let Some(fx) = effects.as_ref() {
                spawn_explosion(
                    &mut commands,
                    fx,
                    &mut materials,
                    &mut wreckage,
                    at,
                    seed ^ entity.index(),
                );
            }
            commands.entity(entity).despawn_recursive();
        }
        seed = seed.wrapping_add(2_654_435_761);
    }

    for (entity, mut tracer, mut tf) in &mut tracers {
        tracer.vel.y -= 6.0 * dt;
        tf.translation += tracer.vel * dt;
        tracer.life -= dt;
        let ground = terrain.height_at(tf.translation.x, tf.translation.z);
        let hit_ground = tf.translation.y <= ground + 0.05;
        let out = tf.translation.x.abs() > limit || tf.translation.z.abs() > limit;
        if hit_ground || tracer.life <= 0.0 || out {
            if hit_ground {
                if let Some(fx) = effects.as_ref() {
                    let at = Vec3::new(tf.translation.x, ground, tf.translation.z);
                    spawn_impact_puff(&mut commands, fx, &mut materials, at);
                }
            }
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// Find the gun elevation whose simulated shell lands closest to `target`,
/// sampling the trajectory against the actual terrain so shots clear (or clip)
/// hills correctly. A coarse sweep finds the best candidate angle, then a couple
/// of refinement passes narrow in around it for a precise firing solution.
fn solve_elevation(
    terrain: &Terrain,
    launch: Vec3,
    target: Vec3,
    speed: f32,
    gravity: f32,
    min_elev: f32,
    max_elev: f32,
) -> f32 {
    let flat = Vec2::new(target.x - launch.x, target.z - launch.z);
    let azimuth = Vec3::new(flat.x, 0.0, flat.y).normalize_or_zero();
    if azimuth == Vec3::ZERO {
        return 0.0;
    }

    // Error of a candidate elevation: how far its simulated impact lands from the
    // target (measured in the ground plane, so a shot that lands at the right
    // spot but on a slope still scores as a hit).
    let err_at = |theta: f32| {
        let impact = simulate_impact(terrain, launch, azimuth, speed, gravity, theta);
        Vec2::new(impact.x - target.x, impact.z - target.z).length()
    };

    // Coarse sweep across the whole elevation band.
    let steps = 48;
    let mut best = min_elev;
    let mut best_err = f32::MAX;
    for i in 0..=steps {
        let theta = min_elev + (max_elev - min_elev) * (i as f32 / steps as f32);
        let err = err_at(theta);
        if err < best_err {
            best_err = err;
            best = theta;
        }
    }

    // Refine: repeatedly shrink a window around the current best and re-sample.
    let mut half = (max_elev - min_elev) / steps as f32;
    for _ in 0..4 {
        let lo = (best - half).max(min_elev);
        let hi = (best + half).min(max_elev);
        let sub = 8;
        for i in 0..=sub {
            let theta = lo + (hi - lo) * (i as f32 / sub as f32);
            let err = err_at(theta);
            if err < best_err {
                best_err = err;
                best = theta;
            }
        }
        half *= 0.35;
    }
    best
}

/// Integrate a shell from `launch` at elevation `theta` (along `azimuth`) until
/// it meets the terrain, returning the impact point. Uses a small step and
/// interpolates the exact crossing so the impact is precise across slopes.
fn simulate_impact(
    terrain: &Terrain,
    launch: Vec3,
    azimuth: Vec3,
    speed: f32,
    gravity: f32,
    theta: f32,
) -> Vec3 {
    let horizontal = azimuth * (speed * theta.cos());
    let mut vel = Vec3::new(horizontal.x, speed * theta.sin(), horizontal.z);
    let mut pos = launch;
    let dt = 0.015;
    for _ in 0..1400 {
        let prev = pos;
        vel.y -= gravity * dt;
        pos += vel * dt;
        let prev_gap = prev.y - terrain.height_at(prev.x, prev.z);
        let gap = pos.y - terrain.height_at(pos.x, pos.z);
        // Crossed from above the terrain to at/below it — interpolate the hit.
        if gap <= 0.0 && prev_gap > 0.0 {
            let s = prev_gap / (prev_gap - gap);
            let hit = prev.lerp(pos, s.clamp(0.0, 1.0));
            let g = terrain.height_at(hit.x, hit.z);
            return Vec3::new(hit.x, g, hit.z);
        }
        if pos.distance(launch) > MAP_SIZE {
            break;
        }
    }
    pos
}

/// Animate the target marker on the ground: a ring-and-dome shockwave that
/// collapses inward toward the target point over and over while the gun lays;
/// once the shot is away it releases outward and fades before despawning. The
/// marker stays pinned to the ground surface (`base_y`) throughout.
fn pulse_marker(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<&Projection, With<IsoCamera>>,
    mut markers: Query<(Entity, &mut Transform, &mut TargetMarker, Option<&mut MarkerFading>)>,
) {
    let dt = time.delta_secs();
    let period = 0.7;

    // The orthographic scale is how many world units fill the view vertically, so
    // it grows as the camera zooms out. Size the marker's *starting* width as a
    // fraction of it (with a floor), so it stays visible at every zoom level
    // without being oversized.
    let cam_scale = cameras
        .get_single()
        .ok()
        .and_then(|p| match p {
            Projection::Orthographic(o) => Some(o.scale),
            _ => None,
        })
        .unwrap_or(30.0);
    let span = (cam_scale * 0.06).max(2.2);

    for (entity, mut tf, mut marker, fading) in &mut markers {
        marker.age += dt;
        tf.translation.y = marker.base_y;
        let (ring_a, dome_a);
        if let Some(mut fade) = fading {
            // After firing: a small release outward, then fade to nothing.
            fade.t += dt;
            let f = (fade.t / 0.5).clamp(0.0, 1.0);
            tf.scale = Vec3::splat(span * (0.15 + 0.7 * f));
            ring_a = 0.95 * (1.0 - f);
            dome_a = 0.45 * (1.0 - f);
            if fade.t >= 0.5 {
                commands.entity(entity).despawn_recursive();
                continue;
            }
        } else {
            // A shockwave in reverse: a bubble that starts wide and collapses all
            // the way down to the exact clicked point, over and over, brightening
            // as it converges.
            let t = (marker.age % period) / period;
            tf.scale = Vec3::splat(span * (0.02 + 0.98 * (1.0 - t)));
            ring_a = 0.55 + 0.45 * (1.0 - t);
            dome_a = 0.25 + 0.35 * (1.0 - t);
        }
        if let Some(mat) = materials.get_mut(&marker.ring_mat) {
            mat.base_color = mat.base_color.with_alpha(ring_a);
        }
        if let Some(mat) = materials.get_mut(&marker.dome_mat) {
            mat.base_color = mat.base_color.with_alpha(dome_a);
        }
    }
}

/// Step `current` toward `target` (both radians) by at most `max_step`, taking
/// the shortest way around the circle.
fn step_angle(current: f32, target: f32, max_step: f32) -> f32 {
    let diff = wrap_pi(target - current);
    if diff.abs() <= max_step {
        wrap_pi(target)
    } else {
        wrap_pi(current + max_step * diff.signum())
    }
}

/// Step a non-wrapping scalar toward a target by at most `max_step`.
fn step_toward(current: f32, target: f32, max_step: f32) -> f32 {
    let diff = target - current;
    if diff.abs() <= max_step {
        target
    } else {
        current + max_step * diff.signum()
    }
}

fn wrap_pi(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// March a camera ray until it dips below the heightfield; bisect to refine.
fn cursor_ground(
    screen: Vec2,
    camera: &Camera,
    cam_tf: &GlobalTransform,
    terrain: &Terrain,
) -> Option<Vec3> {
    let ray = camera.viewport_to_world(cam_tf, screen).ok()?;
    let origin = ray.origin;
    let dir = ray.direction.as_vec3();
    let mut t = 0.0f32;
    let step = 2.0;
    let mut prev_above = origin.y - terrain.height_at(origin.x, origin.z) > 0.0;
    while t < 3000.0 {
        t += step;
        let p = origin + dir * t;
        let above = p.y - terrain.height_at(p.x, p.z) > 0.0;
        if above != prev_above {
            let (mut lo, mut hi) = (t - step, t);
            for _ in 0..14 {
                let mid = (lo + hi) * 0.5;
                let pm = origin + dir * mid;
                if (pm.y - terrain.height_at(pm.x, pm.z) > 0.0) == prev_above {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some(origin + dir * hi);
        }
        prev_above = above;
    }
    None
}
