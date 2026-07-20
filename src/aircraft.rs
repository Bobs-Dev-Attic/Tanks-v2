//! WWII aircraft that make aerial attacks on the player.
//!
//! Every so often an enemy dive-bomber (a low-poly Stuka-like plane, spinning
//! prop and fixed spatted gear) sweeps in from a map edge, lines up on the
//! player, releases a bomb with a ballistic lead so it lands on target, and
//! roars off the far side. Bombs fall under gravity and detonate with a big HE
//! blast that splash-damages the player through the shared damage system.

use crate::combat::Armor;
use crate::control::PlayerControlled;
use crate::effects::{spawn_explosion, EffectAssets, Wreckage};
use crate::soldiers::Soldier;
use crate::terrain::{Terrain, MAP_SIZE};
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// Cruising speed of an attacking plane (world units / second).
const AIR_SPEED: f32 = 58.0;
/// Altitude the planes fly at — well above the tallest terrain.
const AIR_ALT: f32 = 46.0;
/// Overall scale of a plane. The base model is roughly tank-sized; scaling it up
/// makes the aircraft read as clearly larger than the tanks below (a real
/// dive-bomber dwarfs a tank), and keeps them legible from the high iso camera.
const PLANE_SCALE: f32 = 2.2;
/// Seconds between attack waves.
const WAVE_INTERVAL: f32 = 15.0;
/// Bomb ballistics and effect.
const BOMB_GRAVITY: f32 = 32.0;
const BOMB_DAMAGE: f32 = 60.0;
const BOMB_SPLASH: f32 = 8.0;
/// A bomb's HE blast fells any infantry within this (larger) radius.
const BOMB_INFANTRY_KILL: f32 = 12.0;

pub struct AircraftPlugin;

impl Plugin for AircraftPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_aircraft_assets)
            .insert_resource(WaveTimer(Timer::from_seconds(8.0, TimerMode::Repeating)))
            .add_systems(Update, (spawn_waves, fly_aircraft, spin_props, update_bombs));
    }
}

#[derive(Resource)]
struct WaveTimer(Timer);

/// Shared meshes/materials for the planes and their bombs.
#[derive(Resource)]
struct AircraftAssets {
    fuselage: Handle<Mesh>,
    nose: Handle<Mesh>,
    wing: Handle<Mesh>,
    tail_h: Handle<Mesh>,
    tail_v: Handle<Mesh>,
    canopy: Handle<Mesh>,
    strut: Handle<Mesh>,
    wheel: Handle<Mesh>,
    prop: Handle<Mesh>,
    hub: Handle<Mesh>,
    bomb: Handle<Mesh>,
    body_mat: Handle<StandardMaterial>,
    dark_mat: Handle<StandardMaterial>,
    nose_mat: Handle<StandardMaterial>,
    glass_mat: Handle<StandardMaterial>,
    bomb_mat: Handle<StandardMaterial>,
}

/// An attacking plane on a bombing run.
#[derive(Component)]
struct Aircraft {
    vel: Vec3,
    /// The ground point it is trying to bomb (the player's position at spawn).
    target: Vec3,
    dropped: bool,
}

/// A spinning propeller (child of an aircraft).
#[derive(Component)]
struct Prop;

/// A falling bomb.
#[derive(Component)]
struct Bomb {
    vel: Vec3,
}

fn setup_aircraft_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = AircraftAssets {
        fuselage: meshes.add(Cuboid::new(0.85, 0.95, 4.8)),
        nose: meshes.add(Cuboid::new(0.7, 0.7, 0.7)),
        wing: meshes.add(Cuboid::new(7.4, 0.16, 1.6)),
        tail_h: meshes.add(Cuboid::new(2.8, 0.14, 0.95)),
        tail_v: meshes.add(Cuboid::new(0.14, 1.15, 0.95)),
        canopy: meshes.add(Cuboid::new(0.6, 0.42, 1.4)),
        strut: meshes.add(Cuboid::new(0.16, 0.7, 0.5)),
        wheel: meshes.add(Cylinder::new(0.28, 0.16)),
        prop: meshes.add(Cuboid::new(0.12, 2.6, 0.06)),
        hub: meshes.add(Cylinder::new(0.16, 0.3)),
        bomb: meshes.add(Cuboid::new(0.3, 0.3, 0.95)),
        body_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.28, 0.31, 0.24),
            perceptual_roughness: 0.85,
            ..default()
        }),
        dark_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.1, 0.11),
            perceptual_roughness: 0.9,
            ..default()
        }),
        nose_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.75, 0.65, 0.12),
            perceptual_roughness: 0.7,
            ..default()
        }),
        glass_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.4, 0.55, 0.6, 0.6),
            perceptual_roughness: 0.2,
            metallic: 0.3,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        bomb_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.17, 0.15),
            perceptual_roughness: 0.8,
            ..default()
        }),
    };
    commands.insert_resource(assets);
}

/// Periodically send in a plane aimed at the player.
fn spawn_waves(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<WaveTimer>,
    assets: Option<Res<AircraftAssets>>,
    players: Query<&GlobalTransform, With<PlayerControlled>>,
) {
    let (Some(assets), Ok(player)) = (assets, players.get_single()) else {
        return;
    };
    if !timer.0.tick(time.delta()).finished() {
        return;
    }
    timer.0.set_duration(std::time::Duration::from_secs_f32(WAVE_INTERVAL));

    let target = player.translation();
    // Approach from a direction that varies each wave (deterministic, no rng).
    let a = time.elapsed_secs() * 1.7;
    let dir = Vec3::new(a.cos(), 0.0, a.sin()).normalize_or_zero();
    let start = Vec3::new(
        target.x - dir.x * MAP_SIZE * 0.55,
        AIR_ALT,
        target.z - dir.z * MAP_SIZE * 0.55,
    );
    spawn_aircraft(&mut commands, &assets, start, dir * AIR_SPEED, target);
}

fn spawn_aircraft(
    commands: &mut Commands,
    a: &AircraftAssets,
    pos: Vec3,
    vel: Vec3,
    target: Vec3,
) {
    let facing = Transform::from_translation(pos).looking_to(vel.normalize_or_zero(), Vec3::Y);
    let root = commands
        .spawn((
            Aircraft {
                vel,
                target,
                dropped: false,
            },
            Transform {
                translation: pos,
                rotation: facing.rotation,
                scale: Vec3::splat(PLANE_SCALE),
            },
            Visibility::default(),
            Name::new("Aircraft"),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        // Fuselage (nose at -Z) and a bright spinner nose.
        p.spawn((
            Mesh3d(a.fuselage.clone()),
            MeshMaterial3d(a.body_mat.clone()),
            Transform::default(),
        ));
        p.spawn((
            Mesh3d(a.nose.clone()),
            MeshMaterial3d(a.nose_mat.clone()),
            Transform::from_xyz(0.0, 0.0, -2.5),
        ));
        // Wings, tailplane, and fin.
        p.spawn((
            Mesh3d(a.wing.clone()),
            MeshMaterial3d(a.body_mat.clone()),
            Transform::from_xyz(0.0, -0.15, -0.2),
        ));
        p.spawn((
            Mesh3d(a.tail_h.clone()),
            MeshMaterial3d(a.body_mat.clone()),
            Transform::from_xyz(0.0, 0.05, 2.2),
        ));
        p.spawn((
            Mesh3d(a.tail_v.clone()),
            MeshMaterial3d(a.body_mat.clone()),
            Transform::from_xyz(0.0, 0.6, 2.2),
        ));
        // Canopy.
        p.spawn((
            Mesh3d(a.canopy.clone()),
            MeshMaterial3d(a.glass_mat.clone()),
            Transform::from_xyz(0.0, 0.55, 0.1),
        ));
        // Fixed spatted landing gear (Stuka signature) under each wing.
        for s in [-1.0f32, 1.0] {
            p.spawn((
                Mesh3d(a.strut.clone()),
                MeshMaterial3d(a.dark_mat.clone()),
                Transform::from_xyz(s * 1.7, -0.6, -0.2),
            ));
            p.spawn((
                Mesh3d(a.wheel.clone()),
                MeshMaterial3d(a.dark_mat.clone()),
                Transform::from_xyz(s * 1.7, -0.95, -0.2)
                    .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
            ));
        }
        // Prop hub and spinning blades at the nose.
        p.spawn((
            Mesh3d(a.hub.clone()),
            MeshMaterial3d(a.dark_mat.clone()),
            Transform::from_xyz(0.0, 0.0, -2.85).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        ));
        p.spawn((
            Mesh3d(a.prop.clone()),
            MeshMaterial3d(a.dark_mat.clone()),
            Transform::from_xyz(0.0, 0.0, -2.9),
            Prop,
        ));
    });
}

/// Fly the planes, release bombs with a ballistic lead, and retire them off-map.
#[allow(clippy::too_many_arguments)]
fn fly_aircraft(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    assets: Option<Res<AircraftAssets>>,
    mut planes: Query<(Entity, &mut Transform, &mut Aircraft)>,
) {
    let (Some(terrain), Some(assets)) = (terrain, assets) else {
        return;
    };
    let dt = time.delta_secs().min(1.0 / 20.0);
    let limit = MAP_SIZE * 0.62;

    for (entity, mut tf, mut plane) in &mut planes {
        tf.translation += plane.vel * dt;
        // Face the direction of travel, with a slight bank into the run.
        let dir = plane.vel.normalize_or_zero();
        if dir != Vec3::ZERO {
            tf.rotation = Transform::from_translation(tf.translation)
                .looking_to(dir, Vec3::Y)
                .rotation
                * Quat::from_rotation_z(0.12);
        }

        // Release the bomb once the target is within the ballistic lead distance.
        if !plane.dropped {
            let ground = terrain.height_at(tf.translation.x, tf.translation.z);
            let fall = (2.0 * (tf.translation.y - ground).max(1.0) / BOMB_GRAVITY).sqrt();
            let lead = Vec2::new(plane.vel.x, plane.vel.z).length() * fall;
            let flat = Vec2::new(
                plane.target.x - tf.translation.x,
                plane.target.z - tf.translation.z,
            )
            .length();
            // Only drop while still approaching (heading roughly at the target).
            let approaching = Vec2::new(plane.vel.x, plane.vel.z)
                .normalize_or_zero()
                .dot(
                    Vec2::new(
                        plane.target.x - tf.translation.x,
                        plane.target.z - tf.translation.z,
                    )
                    .normalize_or_zero(),
                )
                > 0.3;
            if approaching && flat <= lead {
                commands.spawn((
                    Mesh3d(assets.bomb.clone()),
                    MeshMaterial3d(assets.bomb_mat.clone()),
                    Transform::from_translation(tf.translation - Vec3::Y * 0.7),
                    Bomb {
                        vel: plane.vel * 0.85,
                    },
                ));
                plane.dropped = true;
            }
        }

        // Retire the plane once it has left the field.
        if tf.translation.x.abs() > limit || tf.translation.z.abs() > limit {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn spin_props(time: Res<Time>, mut props: Query<&mut Transform, With<Prop>>) {
    let d = time.delta_secs() * 40.0;
    for mut tf in &mut props {
        tf.rotate_local_z(d);
    }
}

/// Bombs fall under gravity and detonate on the ground (or the player), splash-
/// damaging the player through the shared damage system.
#[allow(clippy::too_many_arguments)]
fn update_bombs(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    effects: Option<Res<EffectAssets>>,
    mut wreckage: ResMut<Wreckage>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bombs: Query<(Entity, &mut Bomb, &mut Transform)>,
    mut players: Query<(&GlobalTransform, &mut Armor), With<PlayerControlled>>,
    mut soldiers: Query<(&GlobalTransform, &mut Soldier)>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs().min(1.0 / 20.0);
    let limit = MAP_SIZE * 0.5;
    let mut seed = (time.elapsed_secs() * 613.0) as u32 | 1;

    for (entity, mut bomb, mut tf) in &mut bombs {
        bomb.vel.y -= BOMB_GRAVITY * dt;
        tf.translation += bomb.vel * dt;
        // Point the bomb along its fall.
        if let Some(dir) = bomb.vel.try_normalize() {
            tf.rotation = Transform::IDENTITY.looking_to(dir, Vec3::Y).rotation;
        }

        let ground = terrain.height_at(tf.translation.x, tf.translation.z);
        let out = tf.translation.x.abs() > limit || tf.translation.z.abs() > limit;
        if tf.translation.y <= ground + 0.2 || out {
            let at = Vec3::new(tf.translation.x, ground, tf.translation.z);
            if let Some(fx) = effects.as_ref() {
                spawn_explosion(&mut commands, fx, &mut materials, &mut wreckage, at, seed);
            }
            // Splash the player.
            if let Ok((pgt, mut armor)) = players.get_single_mut() {
                if !armor.destroyed {
                    let d = pgt.translation().distance(at);
                    if d < BOMB_SPLASH {
                        armor.damage(BOMB_DAMAGE * (1.0 - d / BOMB_SPLASH));
                    }
                }
            }
            // Infantry near the bomb are killed.
            for (stf, mut soldier) in soldiers.iter_mut() {
                if !soldier.dead && stf.translation().distance(at) < BOMB_INFANTRY_KILL {
                    soldier.dead = true;
                }
            }
            commands.entity(entity).despawn_recursive();
        }
        seed = seed.wrapping_add(2_654_435_761);
    }
}
