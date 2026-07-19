//! Polygon-based effects: muzzle flashes, explosions, flying debris shards,
//! fire, rotating smoke, and smoldering craters. Every puff is an n-gon or star
//! that faces the camera, spins, and fades — no round sprites.
//!
//! Debris and craters persist as scenery (they are not despawned on a timer);
//! instead a fixed-size pool recycles the oldest ones only when the budget is
//! exceeded, so wreckage lingers until the engine needs the memory back.

use crate::camera::IsoCamera;
use crate::terrain::Terrain;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::VecDeque;
use std::f32::consts::FRAC_PI_2;

/// How many pieces of persistent wreckage (debris + craters) to keep before the
/// oldest are recycled.
const WRECKAGE_BUDGET: usize = 600;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wreckage>().add_systems(Startup, setup_effect_assets).add_systems(
            Update,
            (
                update_debris,
                update_particles,
                update_smolder,
                update_craters,
                update_smokers,
                update_lifetimes,
            ),
        );
    }
}

/// Ring buffer of persistent wreckage entities.
#[derive(Resource, Default)]
pub struct Wreckage {
    items: VecDeque<Entity>,
}

impl Wreckage {
    fn add(&mut self, commands: &mut Commands, entity: Entity) {
        self.items.push_back(entity);
        while self.items.len() > WRECKAGE_BUDGET {
            if let Some(old) = self.items.pop_front() {
                commands.entity(old).despawn_recursive();
            }
        }
    }
}

/// Shared meshes/materials for effects.
#[derive(Resource)]
pub struct EffectAssets {
    puff_mesh: Handle<Mesh>,
    flash_mesh: Handle<Mesh>,
    debris_mesh: Handle<Mesh>,
    debris_mat: Handle<StandardMaterial>,
}

fn setup_effect_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let puff_mesh = meshes.add(radial_ngon(7));
    let flash_mesh = meshes.add(star_mesh(7, 0.42));
    let debris_mesh = meshes.add(Tetrahedron::default());
    let debris_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.09, 0.08),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.insert_resource(EffectAssets {
        puff_mesh,
        flash_mesh,
        debris_mesh,
        debris_mat,
    });
}

#[derive(Component)]
struct Debris {
    vel: Vec3,
    spin: Vec3,
    settled: bool,
}

/// A camera-facing polygon that drifts, expands, spins, and fades.
#[derive(Component)]
struct Particle {
    vel: Vec3,
    rise: f32,
    age: f32,
    ttl: f32,
    expand: f32,
    start_scale: f32,
    spin_rate: f32,
    angle0: f32,
    start_alpha: f32,
    mat: Handle<StandardMaterial>,
}

/// A glowing light (flash) that dims over its lifetime.
#[derive(Component)]
struct Smolder {
    age: f32,
    ttl: f32,
    base_intensity: f32,
}

/// A scorched crater that smolders (glow + light dimming) then stays as a dark
/// scar. Not despawned by the smolder — it persists as wreckage.
#[derive(Component)]
struct Crater {
    age: f32,
    ttl: f32,
    base_emissive: LinearRgba,
    base_intensity: f32,
    mat: Handle<StandardMaterial>,
}

/// Emits smoke puffs periodically for a while (craters, smoking debris).
#[derive(Component)]
struct Smoker {
    timer: Timer,
    remaining: f32,
    scale: f32,
    despawn_when_done: bool,
}

#[derive(Component)]
struct Lifetime {
    timer: Timer,
}

const GRAVITY: f32 = 22.0;

/// A full main-gun impact: flash, fire, smoke, dust, debris, and a crater.
#[allow(clippy::too_many_arguments)]
pub fn spawn_explosion(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    wreckage: &mut Wreckage,
    pos: Vec3,
    seed: u32,
) {
    let mut rng = Rng::new(seed ^ 0x9E37_79B9);

    spawn_flash(commands, fx, materials, pos + Vec3::Y * 1.0, 5.0, &mut rng);
    spawn_crater(commands, fx, materials, wreckage, pos, &mut rng);

    // Fireballs.
    for _ in 0..6 {
        spawn_particle(
            commands,
            fx.puff_mesh.clone(),
            materials,
            pos + Vec3::Y * rng.range(0.3, 1.2),
            ParticleSpec {
                tint: Color::srgba(1.0, rng.range(0.5, 0.8), 0.15, 0.9),
                emissive: LinearRgba::rgb(3.0, 1.0, 0.2),
                vel: Vec3::new(rng.range(-3.0, 3.0), rng.range(2.0, 5.0), rng.range(-3.0, 3.0)),
                rise: 1.0,
                ttl: rng.range(0.35, 0.6),
                expand: 1.4,
                start_scale: rng.range(1.2, 2.2),
                spin_rate: rng.range(-3.0, 3.0),
                start_alpha: 0.95,
            },
            &mut rng,
        );
    }
    // Smoke.
    for _ in 0..9 {
        let g = rng.range(0.10, 0.18);
        spawn_particle(
            commands,
            fx.puff_mesh.clone(),
            materials,
            pos + Vec3::Y * rng.range(0.6, 1.6),
            ParticleSpec {
                tint: Color::srgba(g, g, g + 0.02, 0.62),
                emissive: LinearRgba::BLACK,
                vel: Vec3::new(rng.range(-1.2, 1.2), 0.0, rng.range(-1.2, 1.2)),
                rise: rng.range(2.5, 4.0),
                ttl: rng.range(2.4, 3.6),
                expand: 3.0,
                start_scale: rng.range(1.1, 1.8),
                spin_rate: rng.range(-1.4, 1.4),
                start_alpha: 0.6,
            },
            &mut rng,
        );
    }
    // Dust.
    for _ in 0..7 {
        let dir = Vec3::new(rng.range(-1.0, 1.0), rng.range(0.1, 0.4), rng.range(-1.0, 1.0))
            .normalize_or_zero();
        spawn_particle(
            commands,
            fx.puff_mesh.clone(),
            materials,
            pos + Vec3::Y * rng.range(0.2, 0.7),
            ParticleSpec {
                tint: Color::srgba(0.72, 0.66, 0.5, 0.5),
                emissive: LinearRgba::BLACK,
                vel: dir * rng.range(3.0, 7.0),
                rise: 0.3,
                ttl: rng.range(1.3, 1.9),
                expand: 2.6,
                start_scale: rng.range(0.8, 1.3),
                spin_rate: rng.range(-2.0, 2.0),
                start_alpha: 0.55,
            },
            &mut rng,
        );
    }
    // Persistent debris shards; roughly half of them smoke for a while.
    for i in 0..11 {
        let dir = Vec3::new(rng.range(-1.0, 1.0), rng.range(0.4, 1.5), rng.range(-1.0, 1.0))
            .normalize_or_zero();
        let speed = rng.range(7.0, 17.0);
        let smokes = i % 2 == 0;
        let smoke_life = rng.range(2.5, 4.0);
        let id = {
            let mut e = commands.spawn((
                Mesh3d(fx.debris_mesh.clone()),
                MeshMaterial3d(fx.debris_mat.clone()),
                Transform::from_translation(pos + Vec3::Y * 0.4)
                    .with_scale(Vec3::splat(rng.range(0.35, 0.9))),
                Debris {
                    vel: dir * speed,
                    spin: Vec3::new(
                        rng.range(-8.0, 8.0),
                        rng.range(-8.0, 8.0),
                        rng.range(-8.0, 8.0),
                    ),
                    settled: false,
                },
            ));
            if smokes {
                e.insert(Smoker {
                    timer: Timer::from_seconds(0.3, TimerMode::Repeating),
                    remaining: smoke_life,
                    scale: 0.5,
                    despawn_when_done: false,
                });
            }
            e.id()
        };
        wreckage.add(commands, id);
    }
}

/// A machine-gun ground-hit dust puff.
pub fn spawn_impact_puff(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
) {
    let mut rng = Rng::new((pos.x * 131.0 + pos.z * 197.0) as i32 as u32 | 1);
    spawn_particle(
        commands,
        fx.puff_mesh.clone(),
        materials,
        pos + Vec3::Y * 0.2,
        ParticleSpec {
            tint: Color::srgba(0.72, 0.68, 0.55, 0.5),
            emissive: LinearRgba::BLACK,
            vel: Vec3::ZERO,
            rise: 1.0,
            ttl: 0.5,
            expand: 2.5,
            start_scale: 0.3,
            spin_rate: rng.range(-3.0, 3.0),
            start_alpha: 0.5,
        },
        &mut rng,
    );
}

/// A low dust puff kicked up by the tracks, tinted by the ground material.
pub fn spawn_dust(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    tint: Color,
    scale: f32,
    seed: u32,
) {
    let mut rng = Rng::new(seed | 1);
    spawn_particle(
        commands,
        fx.puff_mesh.clone(),
        materials,
        pos + Vec3::Y * 0.15,
        ParticleSpec {
            tint,
            emissive: LinearRgba::BLACK,
            vel: Vec3::new(rng.range(-0.4, 0.4), rng.range(0.3, 0.9), rng.range(-0.4, 0.4)),
            rise: 0.5,
            ttl: rng.range(0.6, 1.1),
            expand: 2.0,
            start_scale: scale * rng.range(0.7, 1.1),
            spin_rate: rng.range(-2.0, 2.0),
            start_alpha: 0.4,
        },
        &mut rng,
    );
}

/// A persistent flat track mark on the ground.
pub fn spawn_track_mark(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    wreckage: &mut Wreckage,
    pos: Vec3,
    tint: Color,
) {
    let mat = materials.add(StandardMaterial {
        base_color: tint,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let mark = commands
        .spawn((
            Mesh3d(fx.puff_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(pos + Vec3::Y * 0.04)
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
                .with_scale(Vec3::splat(0.5)),
        ))
        .id();
    wreckage.add(commands, mark);
}

/// Lingering gun smoke drifting from the muzzle after the main gun fires.
pub fn spawn_gun_smoke(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    dir: Vec3,
    seed: u32,
) {
    let mut rng = Rng::new(seed | 1);
    for _ in 0..6 {
        let g = rng.range(0.18, 0.3);
        spawn_particle(
            commands,
            fx.puff_mesh.clone(),
            materials,
            pos + dir * rng.range(-0.2, 1.2),
            ParticleSpec {
                tint: Color::srgba(g, g, g + 0.02, 0.55),
                emissive: LinearRgba::BLACK,
                vel: dir * rng.range(1.5, 4.5)
                    + Vec3::new(rng.range(-0.6, 0.6), rng.range(0.2, 0.9), rng.range(-0.6, 0.6)),
                rise: rng.range(0.8, 1.6),
                ttl: rng.range(1.2, 2.2),
                expand: 2.6,
                start_scale: rng.range(0.5, 0.95),
                spin_rate: rng.range(-1.6, 1.6),
                start_alpha: 0.5,
            },
            &mut rng,
        );
    }
}

/// A long-lived rising smoke column pinned to a point (destroyed tanks).
pub fn spawn_smoke_column(commands: &mut Commands, at: Vec3, duration: f32, scale: f32) {
    commands.spawn((
        Transform::from_translation(at),
        Smoker {
            timer: Timer::from_seconds(0.3, TimerMode::Repeating),
            remaining: duration,
            scale,
            despawn_when_done: true,
        },
    ));
}

/// One tick of a burning/smoldering wreck: a dark smoke puff, plus a flame lick
/// if it is still actively on fire.
pub fn spawn_wreck_fire(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    burning: bool,
    seed: u32,
) {
    let mut rng = Rng::new(seed | 1);
    let g = rng.range(0.08, 0.16);
    spawn_particle(
        commands,
        fx.puff_mesh.clone(),
        materials,
        pos + Vec3::new(rng.range(-0.5, 0.5), rng.range(0.2, 0.8), rng.range(-0.5, 0.5)),
        ParticleSpec {
            tint: Color::srgba(g, g, g + 0.02, 0.55),
            emissive: LinearRgba::BLACK,
            vel: Vec3::new(rng.range(-0.5, 0.5), 0.0, rng.range(-0.5, 0.5)),
            rise: rng.range(2.0, 3.4),
            ttl: rng.range(1.8, 3.0),
            expand: 2.6,
            start_scale: rng.range(0.7, 1.2),
            spin_rate: rng.range(-1.2, 1.2),
            start_alpha: 0.5,
        },
        &mut rng,
    );
    if burning {
        spawn_particle(
            commands,
            fx.puff_mesh.clone(),
            materials,
            pos + Vec3::new(rng.range(-0.4, 0.4), rng.range(0.0, 0.4), rng.range(-0.4, 0.4)),
            ParticleSpec {
                tint: Color::srgba(1.0, rng.range(0.45, 0.7), 0.12, 0.9),
                emissive: LinearRgba::rgb(2.6, 0.9, 0.2),
                vel: Vec3::new(rng.range(-0.4, 0.4), rng.range(1.0, 2.2), rng.range(-0.4, 0.4)),
                rise: 1.2,
                ttl: rng.range(0.35, 0.6),
                expand: 1.2,
                start_scale: rng.range(0.5, 0.9),
                spin_rate: rng.range(-3.0, 3.0),
                start_alpha: 0.95,
            },
            &mut rng,
        );
    }
}

/// A muzzle flash: a big star plus (for the main gun) fire wisps.
pub fn spawn_muzzle_flash(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    scale: f32,
    seed: u32,
) {
    let mut rng = Rng::new(seed | 1);
    spawn_flash(commands, fx, materials, pos, scale, &mut rng);
    if scale > 1.5 {
        for _ in 0..3 {
            spawn_particle(
                commands,
                fx.puff_mesh.clone(),
                materials,
                pos,
                ParticleSpec {
                    tint: Color::srgba(1.0, 0.7, 0.2, 0.9),
                    emissive: LinearRgba::rgb(3.0, 1.2, 0.2),
                    vel: Vec3::new(rng.range(-2.0, 2.0), rng.range(0.0, 2.0), rng.range(-2.0, 2.0)),
                    rise: 0.5,
                    ttl: rng.range(0.18, 0.3),
                    expand: 1.6,
                    start_scale: rng.range(0.6, 1.1),
                    spin_rate: rng.range(-4.0, 4.0),
                    start_alpha: 0.9,
                },
                &mut rng,
            );
        }
    }
}

fn spawn_crater(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    wreckage: &mut Wreckage,
    pos: Vec3,
    rng: &mut Rng,
) {
    let base_emissive = LinearRgba::rgb(2.6, 0.7, 0.12);
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.06, 0.05, 0.04, 0.9),
        emissive: base_emissive,
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let radius = rng.range(1.8, 2.6);
    let crater = commands
        .spawn((
            Mesh3d(fx.puff_mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos + Vec3::Y * 0.06)
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
                .with_scale(Vec3::splat(radius)),
            PointLight {
                color: Color::srgb(1.0, 0.45, 0.15),
                intensity: 260_000.0,
                range: 22.0,
                shadows_enabled: false,
                ..default()
            },
            Crater {
                age: 0.0,
                ttl: 7.0,
                base_emissive,
                base_intensity: 260_000.0,
                mat,
            },
        ))
        .id();
    wreckage.add(commands, crater);

    // A smoke column rising from the crater for a while.
    commands.spawn((
        Transform::from_translation(pos + Vec3::Y * 0.2),
        Smoker {
            timer: Timer::from_seconds(0.28, TimerMode::Repeating),
            remaining: 7.0,
            scale: 1.3,
            despawn_when_done: true,
        },
    ));
}

fn spawn_flash(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    scale: f32,
    rng: &mut Rng,
) {
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.85, 0.4, 1.0),
        emissive: LinearRgba::rgb(6.0, 4.0, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(fx.flash_mesh.clone()),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(pos).with_scale(Vec3::splat(scale)),
        Particle {
            vel: Vec3::ZERO,
            rise: 0.0,
            age: 0.0,
            ttl: 0.09 + scale * 0.02,
            expand: 0.4,
            start_scale: scale,
            spin_rate: rng.range(-1.0, 1.0),
            angle0: rng.range(0.0, 6.28),
            start_alpha: 1.0,
            mat,
        },
    ));
}

struct ParticleSpec {
    tint: Color,
    emissive: LinearRgba,
    vel: Vec3,
    rise: f32,
    ttl: f32,
    expand: f32,
    start_scale: f32,
    spin_rate: f32,
    start_alpha: f32,
}

fn spawn_particle(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    spec: ParticleSpec,
    rng: &mut Rng,
) {
    let mat = materials.add(StandardMaterial {
        base_color: spec.tint,
        emissive: spec.emissive,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(pos).with_scale(Vec3::splat(spec.start_scale)),
        Particle {
            vel: spec.vel,
            rise: spec.rise,
            age: 0.0,
            ttl: spec.ttl,
            expand: spec.expand,
            start_scale: spec.start_scale,
            spin_rate: spec.spin_rate,
            angle0: rng.range(0.0, 6.28),
            start_alpha: spec.start_alpha,
            mat,
        },
    ));
}

fn update_particles(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<&GlobalTransform, With<IsoCamera>>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let cam_rot = cameras
        .get_single()
        .map(|c| c.to_scale_rotation_translation().1)
        .unwrap_or(Quat::IDENTITY);

    for (entity, mut p, mut tf) in &mut particles {
        p.age += dt;
        if p.age >= p.ttl {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        let t = p.age / p.ttl;
        tf.translation += p.vel * dt;
        tf.translation.y += p.rise * dt;
        p.vel.x *= 1.0 - (1.4 * dt).min(0.9);
        p.vel.z *= 1.0 - (1.4 * dt).min(0.9);

        tf.scale = Vec3::splat(p.start_scale * (1.0 + p.expand * t));
        tf.rotation = cam_rot * Quat::from_rotation_z(p.angle0 + p.spin_rate * p.age);

        if let Some(mat) = materials.get_mut(&p.mat) {
            mat.base_color = mat.base_color.with_alpha(p.start_alpha * (1.0 - t));
        }
    }
}

fn update_debris(
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut debris: Query<(&mut Debris, &mut Transform)>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs().min(1.0 / 20.0);
    for (mut d, mut tf) in &mut debris {
        if d.settled {
            continue;
        }
        d.vel.y -= GRAVITY * dt;
        tf.translation += d.vel * dt;
        tf.rotate_local_x(d.spin.x * dt);
        tf.rotate_local_y(d.spin.y * dt);
        tf.rotate_local_z(d.spin.z * dt);
        let ground = terrain.height_at(tf.translation.x, tf.translation.z) + 0.12;
        if tf.translation.y <= ground {
            tf.translation.y = ground;
            d.vel = Vec3::ZERO;
            d.settled = true;
        }
    }
}

fn update_smolder(
    time: Res<Time>,
    mut commands: Commands,
    mut smolders: Query<(Entity, &mut Smolder, &mut PointLight)>,
) {
    let dt = time.delta_secs();
    for (entity, mut s, mut light) in &mut smolders {
        s.age += dt;
        if s.age >= s.ttl {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        light.intensity = s.base_intensity * (1.0 - s.age / s.ttl);
    }
}

fn update_craters(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut craters: Query<(Entity, &mut Crater, &mut PointLight)>,
) {
    let dt = time.delta_secs();
    for (entity, mut c, mut light) in &mut craters {
        c.age += dt;
        let t = (c.age / c.ttl).clamp(0.0, 1.0);
        let flicker = 0.7 + 0.3 * (c.age * 24.0).sin().abs();
        let dim = (1.0 - t) * flicker;
        light.intensity = c.base_intensity * dim;
        if let Some(mat) = materials.get_mut(&c.mat) {
            mat.emissive = LinearRgba::rgb(
                c.base_emissive.red * dim,
                c.base_emissive.green * dim,
                c.base_emissive.blue * dim,
            );
        }
        if c.age >= c.ttl {
            // Cooled: leave a dark scar, drop the light, stop updating.
            if let Some(mat) = materials.get_mut(&c.mat) {
                mat.emissive = LinearRgba::BLACK;
            }
            light.intensity = 0.0;
            commands.entity(entity).remove::<Crater>();
        }
    }
}

fn update_smokers(
    time: Res<Time>,
    mut commands: Commands,
    fx: Option<Res<EffectAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut smokers: Query<(Entity, &mut Smoker, &GlobalTransform)>,
) {
    let Some(fx) = fx else {
        return;
    };
    let dt = time.delta_secs();
    for (entity, mut smoker, gt) in &mut smokers {
        smoker.remaining -= dt;
        let scale = smoker.scale;
        if smoker.timer.tick(time.delta()).just_finished() {
            let pos = gt.translation();
            let mut rng = Rng::new(entity.index() ^ (time.elapsed_secs() * 120.0) as u32 | 1);
            let g = rng.range(0.12, 0.2);
            spawn_particle(
                &mut commands,
                fx.puff_mesh.clone(),
                &mut materials,
                pos + Vec3::Y * 0.3,
                ParticleSpec {
                    tint: Color::srgba(g, g, g + 0.02, 0.5),
                    emissive: LinearRgba::BLACK,
                    vel: Vec3::new(rng.range(-0.5, 0.5), 0.0, rng.range(-0.5, 0.5)),
                    rise: rng.range(1.5, 2.6),
                    ttl: rng.range(1.6, 2.6),
                    expand: 2.4,
                    start_scale: scale * rng.range(0.7, 1.1),
                    spin_rate: rng.range(-1.2, 1.2),
                    start_alpha: 0.45,
                },
                &mut rng,
            );
        }
        if smoker.remaining <= 0.0 {
            if smoker.despawn_when_done {
                commands.entity(entity).despawn_recursive();
            } else {
                commands.entity(entity).remove::<Smoker>();
            }
        }
    }
}

fn update_lifetimes(
    time: Res<Time>,
    mut commands: Commands,
    mut items: Query<(Entity, &mut Lifetime)>,
) {
    for (entity, mut life) in &mut items {
        if life.timer.tick(time.delta()).finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// A flat n-gon: opaque center fading to a transparent rim (radial gradient in
/// vertex colors).
fn radial_ngon(sides: usize) -> Mesh {
    let mut positions = vec![[0.0f32, 0.0, 0.0]];
    let mut colors = vec![[1.0f32, 1.0, 1.0, 1.0]];
    let mut normals = vec![[0.0f32, 0.0, 1.0]];
    let mut uvs = vec![[0.5f32, 0.5]];
    for i in 0..sides {
        let a = i as f32 / sides as f32 * std::f32::consts::TAU;
        positions.push([a.cos(), a.sin(), 0.0]);
        colors.push([1.0, 1.0, 1.0, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([a.cos() * 0.5 + 0.5, a.sin() * 0.5 + 0.5]);
    }
    let mut indices = Vec::new();
    for i in 0..sides {
        let next = (i + 1) % sides;
        indices.extend_from_slice(&[0u32, (i + 1) as u32, (next + 1) as u32]);
    }
    build_mesh(positions, normals, uvs, colors, indices)
}

/// A flat star: bright center, faded tips, brighter valleys.
fn star_mesh(points: usize, inner: f32) -> Mesh {
    let mut positions = vec![[0.0f32, 0.0, 0.0]];
    let mut colors = vec![[1.0f32, 1.0, 1.0, 1.0]];
    let mut normals = vec![[0.0f32, 0.0, 1.0]];
    let mut uvs = vec![[0.5f32, 0.5]];
    let ring = points * 2;
    for i in 0..ring {
        let a = i as f32 / ring as f32 * std::f32::consts::TAU;
        let r = if i % 2 == 0 { 1.0 } else { inner };
        positions.push([a.cos() * r, a.sin() * r, 0.0]);
        colors.push([1.0, 1.0, 1.0, if i % 2 == 0 { 0.15 } else { 0.75 }]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([0.5, 0.5]);
    }
    let mut indices = Vec::new();
    for i in 0..ring {
        let next = (i + 1) % ring;
        indices.extend_from_slice(&[0u32, (i + 1) as u32, (next + 1) as u32]);
    }
    build_mesh(positions, normals, uvs, colors, indices)
}

fn build_mesh(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Tiny deterministic PRNG (xorshift).
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Rng(seed | 1)
    }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
}
