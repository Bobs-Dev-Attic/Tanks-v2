//! WWII infantry squads.
//!
//! Low-poly foot soldiers (feldgrau uniform, Stahlhelm, slung rifle) advance on
//! the player in loose squads, legs and arms swinging in a marching gait. They
//! hold at a standoff and pop the odd rifle shot for atmosphere. They are soft
//! targets: a nearby shell or bomb blast, or a machine-gun burst, cuts them
//! down — they topple and lie where they fell before fading away.

use crate::control::PlayerControlled;
use crate::effects::{spawn_impact_puff, spawn_muzzle_flash, EffectAssets};
use crate::terrain::{Terrain, MAP_SIZE};
use bevy::prelude::*;
use std::f32::consts::PI;

/// Marching speed of a soldier (world units / second) — slower than the tanks.
const MARCH_SPEED: f32 = 6.5;
/// How close the infantry press before holding.
const INFANTRY_STANDOFF: f32 = 34.0;
/// Seconds (roughly) between a settled soldier's rifle shots.
const FIRE_INTERVAL: f32 = 2.2;
/// How long a body lies on the field before it fades out.
const CORPSE_TTL: f32 = 6.0;

pub struct SoldiersPlugin;

impl Plugin for SoldiersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_soldiers)
            .add_systems(Update, march_soldiers);
    }
}

/// Shared meshes/materials for the infantry.
#[derive(Resource)]
struct SoldierAssets {
    torso: Handle<Mesh>,
    head: Handle<Mesh>,
    helmet: Handle<Mesh>,
    pack: Handle<Mesh>,
    limb: Handle<Mesh>,
    rifle: Handle<Mesh>,
    uniform_mat: Handle<StandardMaterial>,
    helmet_mat: Handle<StandardMaterial>,
    skin_mat: Handle<StandardMaterial>,
    gear_mat: Handle<StandardMaterial>,
}

/// One foot soldier.
#[derive(Component)]
pub struct Soldier {
    /// Set true when killed; the body then topples and fades.
    pub dead: bool,
    dead_time: f32,
    /// Lateral / depth slot within the squad (world units).
    slot: Vec2,
    /// Distance this soldier holds from the player.
    standoff: f32,
    /// Accumulated gait phase, advanced by movement.
    phase: f32,
    /// Countdown to the next rifle shot.
    fire_timer: f32,
    /// Hip-pivot leg entities (left, right).
    legs: [Entity; 2],
    /// Shoulder-pivot arm entities (left, right).
    arms: [Entity; 2],
}

fn setup_soldiers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    terrain: Option<Res<Terrain>>,
) {
    let assets = SoldierAssets {
        torso: meshes.add(Cuboid::new(0.42, 0.62, 0.26)),
        head: meshes.add(Cuboid::new(0.24, 0.24, 0.24)),
        helmet: meshes.add(Cuboid::new(0.34, 0.14, 0.36)),
        pack: meshes.add(Cuboid::new(0.34, 0.4, 0.18)),
        limb: meshes.add(Cuboid::new(0.15, 0.72, 0.17)),
        rifle: meshes.add(Cuboid::new(0.05, 0.05, 1.0)),
        uniform_mat: materials.add(StandardMaterial {
            // Feldgrau — a muted gray-green.
            base_color: Color::srgb(0.31, 0.35, 0.28),
            perceptual_roughness: 0.95,
            ..default()
        }),
        helmet_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.17, 0.19, 0.16),
            perceptual_roughness: 0.8,
            metallic: 0.2,
            ..default()
        }),
        skin_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.56, 0.44),
            perceptual_roughness: 0.9,
            ..default()
        }),
        gear_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.11, 0.10),
            perceptual_roughness: 1.0,
            ..default()
        }),
    };

    // Three squads advancing from downrange (-Z), each a loose two-rank wedge,
    // spread across the wider field and offset in depth so they read as distinct
    // squads rather than one crowd.
    let squads: [Vec2; 3] = [
        Vec2::new(-120.0, -150.0),
        Vec2::new(0.0, -175.0),
        Vec2::new(120.0, -150.0),
    ];
    for (si, base) in squads.into_iter().enumerate() {
        for i in 0..6usize {
            // Two ranks of three, staggered.
            let col = (i % 3) as f32 - 1.0;
            let rank = (i / 3) as f32;
            let slot = Vec2::new(col * 5.0 + rank * 1.5, rank * 6.0);
            let px = base.x + col * 5.0;
            let pz = base.y - rank * 6.0;
            let g = terrain.as_ref().map(|t| t.height_at(px, pz)).unwrap_or(0.0);
            // Stagger the standoff a touch per squad so the lines don't overlap.
            let standoff = INFANTRY_STANDOFF + si as f32 * 9.0;
            let phase = (si * 6 + i) as f32 * 1.7;
            spawn_soldier(
                &mut commands,
                &assets,
                Vec3::new(px, g, pz),
                slot,
                standoff,
                phase,
            );
        }
    }

    commands.insert_resource(assets);
}

fn spawn_soldier(
    commands: &mut Commands,
    a: &SoldierAssets,
    pos: Vec3,
    slot: Vec2,
    standoff: f32,
    phase: f32,
) {
    let mut legs = [Entity::PLACEHOLDER; 2];
    let mut arms = [Entity::PLACEHOLDER; 2];

    let root = commands
        .spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(0.0)),
            Visibility::default(),
            Name::new("Soldier"),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        // Torso, head, helmet, backpack. The soldier faces -Z (forward).
        p.spawn((
            Mesh3d(a.torso.clone()),
            MeshMaterial3d(a.uniform_mat.clone()),
            Transform::from_xyz(0.0, 1.06, 0.0),
        ));
        p.spawn((
            Mesh3d(a.head.clone()),
            MeshMaterial3d(a.skin_mat.clone()),
            Transform::from_xyz(0.0, 1.5, 0.0),
        ));
        p.spawn((
            Mesh3d(a.helmet.clone()),
            MeshMaterial3d(a.helmet_mat.clone()),
            Transform::from_xyz(0.0, 1.63, 0.02),
        ));
        p.spawn((
            Mesh3d(a.pack.clone()),
            MeshMaterial3d(a.gear_mat.clone()),
            Transform::from_xyz(0.0, 1.08, 0.2),
        ));

        // Legs pivot at the hips; each leg mesh hangs below its pivot so a
        // rotation about X swings the whole leg.
        for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
            let leg = p
                .spawn((Transform::from_xyz(s * 0.11, 0.74, 0.0), Visibility::default()))
                .with_children(|l| {
                    l.spawn((
                        Mesh3d(a.limb.clone()),
                        MeshMaterial3d(a.uniform_mat.clone()),
                        Transform::from_xyz(0.0, -0.36, 0.0),
                    ));
                })
                .id();
            legs[i] = leg;
        }

        // Arms pivot at the shoulders. The right arm carries the rifle.
        for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
            let arm = p
                .spawn((Transform::from_xyz(s * 0.3, 1.32, 0.0), Visibility::default()))
                .with_children(|arm| {
                    arm.spawn((
                        Mesh3d(a.limb.clone()),
                        MeshMaterial3d(a.uniform_mat.clone()),
                        Transform::from_xyz(0.0, -0.3, 0.0).with_scale(Vec3::new(0.85, 0.8, 0.85)),
                    ));
                    // Rifle held across the right arm, pointing forward (-Z).
                    if s > 0.0 {
                        arm.spawn((
                            Mesh3d(a.rifle.clone()),
                            MeshMaterial3d(a.gear_mat.clone()),
                            Transform::from_xyz(-0.16, -0.32, -0.35),
                        ));
                    }
                })
                .id();
            arms[i] = arm;
        }
    });

    commands.entity(root).insert(Soldier {
        dead: false,
        dead_time: 0.0,
        slot,
        standoff,
        phase,
        fire_timer: FIRE_INTERVAL + (phase % FIRE_INTERVAL),
        legs,
        arms,
    });
}

/// Advance the living soldiers on the player, animate their gait, pop the odd
/// rifle shot, and topple/fade the dead.
#[allow(clippy::too_many_arguments)]
fn march_soldiers(
    time: Res<Time>,
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    fx: Option<Res<EffectAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<&Transform, (With<PlayerControlled>, Without<Soldier>)>,
    mut soldiers: Query<(Entity, &mut Transform, &mut Soldier), Without<PlayerControlled>>,
    mut limbs: Query<&mut Transform, (Without<Soldier>, Without<PlayerControlled>)>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs().min(1.0 / 20.0);
    let player = players.get_single().ok();
    let bound = MAP_SIZE * 0.47;

    for (entity, mut tf, mut soldier) in &mut soldiers {
        // --- Dead: topple forward, sink, and fade out, then despawn. ---
        if soldier.dead {
            // On the first dead frame, kick up a puff of dust and dirt.
            if soldier.dead_time == 0.0 {
                if let Some(fx) = fx.as_ref() {
                    spawn_impact_puff(&mut commands, fx, &mut materials, tf.translation);
                }
                // Freeze the limbs mid-stride.
                for l in soldier.legs.iter().chain(soldier.arms.iter()) {
                    if let Ok(mut lt) = limbs.get_mut(*l) {
                        lt.rotation = Quat::from_rotation_x(0.9);
                    }
                }
            }
            soldier.dead_time += dt;
            let k = (soldier.dead_time / 0.5).clamp(0.0, 1.0);
            // Fall onto the back (pitch about local X) as it drops.
            let ground = terrain.height_at(tf.translation.x, tf.translation.z);
            let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
            tf.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-PI * 0.5 * k);
            tf.translation.y = ground + 0.1 * (1.0 - k);
            if soldier.dead_time >= CORPSE_TTL {
                commands.entity(entity).despawn_recursive();
            } else if soldier.dead_time > CORPSE_TTL - 1.0 {
                // Sink into the ground over the final second so it disappears.
                let s = (CORPSE_TTL - soldier.dead_time).clamp(0.0, 1.0);
                tf.scale = Vec3::splat(s.max(0.02));
            }
            continue;
        }

        let pos = tf.translation;
        // --- Work out where this soldier wants to be. ---
        let (goal, aim_dir) = if let Some(player_tf) = player {
            let ppos = player_tf.translation;
            let to_player = Vec2::new(ppos.x - pos.x, ppos.z - pos.z);
            let range = to_player.length().max(0.001);
            let dirp = to_player / range;
            let perp = Vec2::new(dirp.y, -dirp.x);
            let g = Vec2::new(ppos.x, ppos.z) - dirp * soldier.standoff
                + perp * soldier.slot.x
                + dirp * soldier.slot.y;
            (
                Vec2::new(g.x.clamp(-bound, bound), g.y.clamp(-bound, bound)),
                Vec3::new(dirp.x, 0.0, dirp.y),
            )
        } else {
            (Vec2::new(pos.x, pos.z), Vec3::NEG_Z)
        };

        let to_goal = goal - Vec2::new(pos.x, pos.z);
        let dist = to_goal.length();
        let moving = dist > 1.2;

        // Move toward the goal; face travel while moving, else face the player.
        let (face_dir, step) = if moving {
            let dir = to_goal / dist;
            let step = MARCH_SPEED * dt;
            tf.translation.x += dir.x * step;
            tf.translation.z += dir.y * step;
            (Vec3::new(dir.x, 0.0, dir.y), step)
        } else {
            (aim_dir, 0.0)
        };
        if face_dir.length_squared() > 1e-5 {
            let yaw = face_dir.x.atan2(face_dir.z);
            let target = Quat::from_rotation_y(yaw);
            tf.rotation = tf.rotation.slerp(target, (dt * 8.0).min(1.0));
        }

        // Sit on the ground.
        tf.translation.y = terrain.height_at(tf.translation.x, tf.translation.z);

        // --- Gait: swing limbs by the marching phase; add a small body bob. ---
        soldier.phase += step * 3.2 + dt * if moving { 0.0 } else { 0.6 };
        let swing = if moving { 0.7 } else { 0.06 };
        let p = soldier.phase;
        set_limb(&mut limbs, soldier.legs[0], swing * p.sin());
        set_limb(&mut limbs, soldier.legs[1], swing * (p + PI).sin());
        // Arms counter-swing (the rifle arm more stiffly).
        set_limb(&mut limbs, soldier.arms[0], swing * 0.8 * (p + PI).sin());
        set_limb(&mut limbs, soldier.arms[1], swing * 0.4 * p.sin());
        if moving {
            tf.translation.y += 0.05 * (p * 2.0).sin().abs();
        }

        // --- Occasional rifle crack toward the player once settled (visual). ---
        soldier.fire_timer -= dt;
        if soldier.fire_timer <= 0.0 {
            soldier.fire_timer = FIRE_INTERVAL + (soldier.phase % 1.3);
            if !moving {
                if let Some(fx) = fx.as_ref() {
                    let muzzle = tf.translation
                        + tf.rotation * Vec3::new(0.12, 1.3, -0.85);
                    let seed = (time.elapsed_secs() * 300.0) as u32 ^ entity.index();
                    spawn_muzzle_flash(&mut commands, fx, &mut materials, muzzle, 0.32, seed);
                }
            }
        }
    }
}

/// Rotate a limb pivot about its local X axis (keeps its translation).
fn set_limb(
    limbs: &mut Query<&mut Transform, (Without<Soldier>, Without<PlayerControlled>)>,
    limb: Entity,
    angle: f32,
) {
    if let Ok(mut tf) = limbs.get_mut(limb) {
        tf.rotation = Quat::from_rotation_x(angle);
    }
}
