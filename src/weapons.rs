//! Turret aiming and the two weapons.
//!
//! The turret yaws to follow the mouse cursor's point on the ground. The main
//! gun (E / left mouse) lobs a shell that explodes on impact; the machine gun
//! (Q / right mouse) sprays fast tracers that kick up dust.

use crate::camera::IsoCamera;
use crate::control::PlayerControlled;
use crate::effects::{spawn_explosion, spawn_impact_puff, EffectAssets};
use crate::terrain::{Terrain, MAP_SIZE};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::time::Duration;

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_weapon_assets).add_systems(
            Update,
            (aim_turret, fire_weapons, update_projectiles),
        );
    }
}

/// Rotating turret pivot (parent of turret box, gun, and muzzle).
#[derive(Component)]
pub struct Turret;

/// Empty marker at the gun's muzzle; its `GlobalTransform` is the spawn point.
#[derive(Component)]
pub struct Muzzle;

/// Per-tank weapon cooldowns.
#[derive(Component)]
pub struct Weapons {
    main: Timer,
    mg: Timer,
}

impl Default for Weapons {
    fn default() -> Self {
        // Start finished so the first shot is immediate.
        let mut main = Timer::from_seconds(1.1, TimerMode::Once);
        main.tick(Duration::from_secs(5));
        let mut mg = Timer::from_seconds(0.09, TimerMode::Once);
        mg.tick(Duration::from_secs(5));
        Self { main, mg }
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
}

fn setup_weapon_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let shell_mesh = meshes.add(Sphere::new(0.18));
    let shell_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.18, 0.15),
        emissive: LinearRgba::rgb(1.2, 0.5, 0.15),
        ..default()
    });
    let tracer_mesh = meshes.add(Sphere::new(0.1));
    let tracer_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.4),
        emissive: LinearRgba::rgb(4.0, 3.0, 0.6),
        unlit: true,
        ..default()
    });
    commands.insert_resource(WeaponAssets {
        shell_mesh,
        shell_mat,
        tracer_mesh,
        tracer_mat,
    });
}

/// Yaw each turret to face the ground point under the mouse cursor.
fn aim_turret(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<IsoCamera>>,
    terrain: Option<Res<Terrain>>,
    parents: Query<&GlobalTransform>,
    mut turrets: Query<(&Parent, &mut Transform), With<Turret>>,
) {
    let (Some(terrain), Ok(window), Ok((camera, cam_tf))) =
        (terrain, windows.get_single(), cameras.get_single())
    else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Some(target) = cursor_ground(cursor, camera, cam_tf, &terrain) else {
        return;
    };

    let blend = (time.delta_secs() * 10.0).min(1.0);
    for (parent, mut tf) in &mut turrets {
        let Ok(parent_gt) = parents.get(parent.get()) else {
            continue;
        };
        let (_, parent_rot, parent_pos) = parent_gt.to_scale_rotation_translation();
        let mut dir = target - parent_pos;
        dir.y = 0.0;
        if dir.length_squared() < 0.05 {
            continue;
        }
        // Desired world rotation, then express it in the hull's local frame.
        let desired = Transform::IDENTITY.looking_to(dir.normalize(), Vec3::Y).rotation;
        let local = parent_rot.inverse() * desired;
        tf.rotation = tf.rotation.slerp(local, blend);
    }
}

#[allow(clippy::too_many_arguments)]
fn fire_weapons(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<IsoCamera>>,
    terrain: Option<Res<Terrain>>,
    assets: Res<WeaponAssets>,
    muzzles: Query<&GlobalTransform, With<Muzzle>>,
    mut weapons: Query<&mut Weapons, With<PlayerControlled>>,
) {
    let (Ok(muzzle), Ok(mut weapon)) = (muzzles.get_single(), weapons.get_single_mut()) else {
        return;
    };
    weapon.main.tick(time.delta());
    weapon.mg.tick(time.delta());

    let (_, muzzle_rot, muzzle_pos) = muzzle.to_scale_rotation_translation();
    let forward = muzzle_rot * Vec3::NEG_Z;

    // Aim toward the cursor's ground point if we have one, else straight ahead.
    let aim_dir = terrain
        .as_ref()
        .zip(windows.get_single().ok())
        .zip(cameras.get_single().ok())
        .and_then(|((t, window), (camera, cam_tf))| {
            let cursor = window.cursor_position()?;
            let target = cursor_ground(cursor, camera, cam_tf, t)?;
            (target - muzzle_pos).try_normalize()
        })
        .unwrap_or(forward);

    let fire_main = keys.just_pressed(KeyCode::KeyE) || mouse.just_pressed(MouseButton::Left);
    if fire_main && weapon.main.finished() {
        weapon.main.reset();
        commands.spawn((
            Mesh3d(assets.shell_mesh.clone()),
            MeshMaterial3d(assets.shell_mat.clone()),
            Transform::from_translation(muzzle_pos),
            Shell {
                vel: aim_dir * 95.0,
            },
        ));
    }

    let fire_mg = keys.pressed(KeyCode::KeyQ) || mouse.pressed(MouseButton::Right);
    if fire_mg && weapon.mg.finished() {
        weapon.mg.reset();
        // A little spread so it reads as a spraying MG.
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
}

fn update_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    effects: Option<Res<EffectAssets>>,
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
        shell.vel.y -= 12.0 * dt; // light drop for a slight arc
        tf.translation += shell.vel * dt;
        let ground = terrain.height_at(tf.translation.x, tf.translation.z);
        let out = tf.translation.x.abs() > limit || tf.translation.z.abs() > limit;
        if tf.translation.y <= ground + 0.1 || out {
            if let Some(fx) = effects.as_ref() {
                let at = Vec3::new(tf.translation.x, ground, tf.translation.z);
                spawn_explosion(&mut commands, fx, &mut materials, at, seed ^ entity.index());
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
