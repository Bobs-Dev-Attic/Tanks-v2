//! Turret traverse, gun laying, and the two weapons.
//!
//! The turret yaws toward the aim point at a fixed traverse rate, and the gun
//! auto-elevates to the ballistic angle that best hits the target. Pressing
//! FIRE only *requests* a shot — the gun actually fires once it is loaded and
//! laid on target (turret aligned and gun at the right elevation). Rates scale
//! with the crew/vehicle `condition`, so a damaged or heavier tank lays and
//! reloads more slowly.

use crate::camera::IsoCamera;
use crate::control::PlayerControlled;
use crate::effects::{spawn_explosion, spawn_impact_puff, spawn_muzzle_flash, EffectAssets, Wreckage};
use crate::input::GameInput;
use crate::terrain::{Terrain, MAP_SIZE};
use bevy::prelude::*;
use bevy::transform::TransformSystem;
use std::f32::consts::{FRAC_PI_4, PI, TAU};
use std::time::Duration;

/// Muzzle velocity of a main-gun shell (world units / second).
const SHELL_SPEED: f32 = 90.0;
/// Gravity applied to shells (must match the ballistic solver).
const SHELL_GRAVITY: f32 = 30.0;
/// Turret is "on target" within this yaw error (radians).
const YAW_TOL: f32 = 0.03;
/// Gun is "laid" within this elevation error (radians).
const ELEV_TOL: f32 = 0.02;

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_weapon_assets)
            .add_systems(
                Update,
                (operate_main_gun, fire_machine_gun, update_projectiles),
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

/// Empty marker at the gun muzzle; its `GlobalTransform` is the shot origin.
#[derive(Component)]
pub struct Muzzle;

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
    /// The on-ground marker entity shown while a shot is pending.
    marker: Option<Entity>,
    /// Crew/vehicle condition in 0..1; scales traverse, elevation, and reload.
    condition: f32,
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
            marker: None,
            condition: 1.0,
        }
    }
}

#[derive(Component)]
struct Shell {
    vel: Vec3,
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
    marker_mesh: Handle<Mesh>,
    marker_mat: Handle<StandardMaterial>,
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
    // Target marker: a flat ring on the ground.
    let marker_mesh = meshes.add(Torus::new(0.72, 0.95));
    let marker_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.25, 0.2),
        emissive: LinearRgba::rgb(3.5, 0.4, 0.3),
        unlit: true,
        ..default()
    });
    commands.insert_resource(WeaponAssets {
        shell_mesh,
        shell_mat,
        tracer_mesh,
        tracer_mat,
        marker_mesh,
        marker_mat,
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

    let live_target = input
        .aim
        .zip(cameras.get_single().ok())
        .zip(terrain.as_ref())
        .and_then(|((screen, (camera, cam_tf)), t)| cursor_ground(screen, camera, cam_tf, t));

    // A fresh fire request commits the current aim point and drops a marker.
    if input.fire_main && !weapon.fire_requested {
        if let Some(tp) = live_target {
            weapon.fire_requested = true;
            weapon.fire_target = Some(tp);
            let marker = commands
                .spawn((
                    Mesh3d(assets.marker_mesh.clone()),
                    MeshMaterial3d(assets.marker_mat.clone()),
                    Transform::from_translation(tp + Vec3::Y * 0.1),
                ))
                .id();
            weapon.marker = Some(marker);
        }
    }

    // While a shot is pending, lay the gun on the committed point; otherwise
    // keep following the live aim.
    let target = if weapon.fire_requested {
        weapon.fire_target
    } else {
        live_target
    };

    let (_, root_rot, root_pos) = root_gt.to_scale_rotation_translation();

    if let Some(tp) = target {
        // --- Turret traverse (yaw in the hull's local frame) ---
        let local = root_rot.inverse() * (tp - root_pos);
        let desired_yaw = (-local.x).atan2(-local.z);
        let yaw_step = turret.rate * cond * dt;
        turret.yaw = step_angle(turret.yaw, desired_yaw, yaw_step);
        turret_tf.rotation = Quat::from_rotation_y(turret.yaw);
        turret.aligned = wrap_pi(desired_yaw - turret.yaw).abs() < YAW_TOL;

        // --- Gun laying (ballistic elevation) ---
        let muzzle_pos = muzzles
            .get_single()
            .map(|m| m.translation())
            .unwrap_or(root_pos + Vec3::Y * 1.6);
        let flat = Vec2::new(tp.x - muzzle_pos.x, tp.z - muzzle_pos.z)
            .length()
            .max(1.0);
        let rise = tp.y - muzzle_pos.y;
        let desired_elev =
            ballistic_angle(flat, rise, SHELL_SPEED, SHELL_GRAVITY).clamp(gun.min, gun.max);
        let elev_step = gun.rate * cond * dt;
        gun.elev = step_toward(gun.elev, desired_elev, elev_step);
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
                },
            ));
            // Big muzzle flash, gun recoil, and a hull shake.
            if let Some(fx) = effects.as_ref() {
                let seed = (time.elapsed_secs() * 733.0) as u32 | 1;
                spawn_muzzle_flash(&mut commands, fx, &mut materials, muzzle_pos, 2.4, seed);
            }
            gun.recoil = 0.55;
            shake.time = 0.35;
            shake.duration = 0.35;
            shake.magnitude = 0.2;
            weapon.main.reset();
            weapon.fire_requested = false;
            weapon.fire_target = None;
            if let Some(marker) = weapon.marker.take() {
                commands.entity(marker).despawn_recursive();
            }
        }
    }

    // Recoil slides the gun back, then eases home.
    gun.recoil = (gun.recoil - gun.recoil * 9.0 * dt).max(0.0);
    gun_tf.translation = GUN_PIVOT + Vec3::Z * gun.recoil;
}

/// The machine gun sprays tracers toward the aim point; direct fire, no waiting.
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
    muzzles: Query<&GlobalTransform, With<Muzzle>>,
    mut weapons: Query<&mut Weapons, With<PlayerControlled>>,
) {
    let (Ok(mut weapon), Ok(muzzle)) = (weapons.get_single_mut(), muzzles.get_single()) else {
        return;
    };
    weapon.mg.tick(time.delta());
    if !(input.fire_mg && weapon.mg.finished()) {
        return;
    }
    weapon.mg.reset();

    let (_, muzzle_rot, muzzle_pos) = muzzle.to_scale_rotation_translation();

    // Small muzzle flash for each round.
    if let Some(fx) = effects.as_ref() {
        let seed = (time.elapsed_secs() * 971.0) as u32 | 1;
        spawn_muzzle_flash(&mut commands, fx, &mut materials, muzzle_pos, 0.55, seed);
    }
    let forward = muzzle_rot * Vec3::NEG_Z;
    let aim_dir = input
        .aim
        .zip(cameras.get_single().ok())
        .zip(terrain.as_ref())
        .and_then(|((screen, (camera, cam_tf)), t)| {
            let target = cursor_ground(screen, camera, cam_tf, t)?;
            (target - muzzle_pos).try_normalize()
        })
        .unwrap_or(forward);

    let jitter = Vec3::new(
        (time.elapsed_secs() * 91.0).sin() * 0.03,
        0.0,
        (time.elapsed_secs() * 57.0).cos() * 0.03,
    );
    commands.spawn((
        Mesh3d(assets.tracer_mesh.clone()),
        MeshMaterial3d(assets.tracer_mat.clone()),
        Transform::from_translation(muzzle_pos),
        Tracer {
            vel: (aim_dir + jitter).normalize_or_zero() * 150.0,
            life: 1.2,
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
        let ground = terrain.height_at(tf.translation.x, tf.translation.z);
        let out = tf.translation.x.abs() > limit || tf.translation.z.abs() > limit;
        if tf.translation.y <= ground + 0.1 || out {
            if let Some(fx) = effects.as_ref() {
                let at = Vec3::new(tf.translation.x, ground, tf.translation.z);
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

/// Best-effort launch elevation to hit a target `x` ahead and `y` above the
/// muzzle, given muzzle speed `v` and gravity `g`. Returns the low (direct-fire)
/// solution, or 45° when the target is out of range.
fn ballistic_angle(x: f32, y: f32, v: f32, g: f32) -> f32 {
    let v2 = v * v;
    let disc = v2 * v2 - g * (g * x * x + 2.0 * y * v2);
    if disc < 0.0 {
        return FRAC_PI_4;
    }
    ((v2 - disc.sqrt()) / (g * x)).atan()
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
