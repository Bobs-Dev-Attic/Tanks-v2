//! Polygon-based effects: muzzle flashes, explosions, flying debris shards,
//! fire, and rotating smoke — all built from flat polygon meshes with vertex
//! gradients that fade over their lifetime. No sprites, no round billboards:
//! every puff is an n-gon or star that faces the camera, spins, and dims.

use crate::camera::IsoCamera;
use crate::terrain::Terrain;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_effect_assets).add_systems(
            Update,
            (update_debris, update_particles, update_smolder, update_lifetimes),
        );
    }
}

/// Shared meshes/materials for effects.
#[derive(Resource)]
pub struct EffectAssets {
    /// Soft-edged polygon (center opaque, rim transparent) for smoke/dust/fire.
    puff_mesh: Handle<Mesh>,
    /// Spiky star for muzzle flashes.
    flash_mesh: Handle<Mesh>,
    /// Angular shard for debris.
    debris_mesh: Handle<Mesh>,
    /// Faceted ember.
    ember_mesh: Handle<Mesh>,
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
    let ember_mesh = meshes.add(Sphere::new(0.6));
    let debris_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.09, 0.08),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.insert_resource(EffectAssets {
        puff_mesh,
        flash_mesh,
        debris_mesh,
        ember_mesh,
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

#[derive(Component)]
struct Smolder {
    age: f32,
    ttl: f32,
    base_intensity: f32,
    base_emissive: LinearRgba,
    mat: Option<Handle<StandardMaterial>>,
    flicker: bool,
}

#[derive(Component)]
struct Lifetime {
    timer: Timer,
}

const GRAVITY: f32 = 22.0;

/// A full main-gun impact explosion at `pos`.
pub fn spawn_explosion(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    seed: u32,
) {
    let mut rng = Rng::new(seed ^ 0x9E37_79B9);

    // Flash.
    spawn_flash(commands, fx, materials, pos + Vec3::Y * 1.0, 5.0, &mut rng);

    // Fireballs.
    for _ in 0..6 {
        let tint = Color::srgba(1.0, rng.range(0.5, 0.8), 0.15, 0.9);
        spawn_particle(
            commands,
            fx.puff_mesh.clone(),
            materials,
            pos + Vec3::Y * rng.range(0.3, 1.2),
            ParticleSpec {
                tint,
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

    // Smoke (dark, rising, rotating).
    for _ in 0..9 {
        let g = rng.range(0.10, 0.18);
        let tint = Color::srgba(g, g, g + 0.02, 0.62);
        spawn_particle(
            commands,
            fx.puff_mesh.clone(),
            materials,
            pos + Vec3::Y * rng.range(0.6, 1.6),
            ParticleSpec {
                tint,
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

    // Dust (tan, low, spreading).
    for _ in 0..7 {
        let dir = Vec3::new(rng.range(-1.0, 1.0), rng.range(0.1, 0.4), rng.range(-1.0, 1.0))
            .normalize_or_zero();
        let tint = Color::srgba(0.72, 0.66, 0.5, 0.5);
        spawn_particle(
            commands,
            fx.puff_mesh.clone(),
            materials,
            pos + Vec3::Y * rng.range(0.2, 0.7),
            ParticleSpec {
                tint,
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

    // Debris shards.
    for _ in 0..11 {
        let dir = Vec3::new(rng.range(-1.0, 1.0), rng.range(0.4, 1.5), rng.range(-1.0, 1.0))
            .normalize_or_zero();
        let speed = rng.range(7.0, 17.0);
        commands.spawn((
            Mesh3d(fx.debris_mesh.clone()),
            MeshMaterial3d(fx.debris_mat.clone()),
            Transform::from_translation(pos + Vec3::Y * 0.4)
                .with_scale(Vec3::splat(rng.range(0.35, 0.9))),
            Debris {
                vel: dir * speed,
                spin: Vec3::new(rng.range(-8.0, 8.0), rng.range(-8.0, 8.0), rng.range(-8.0, 8.0)),
                settled: false,
            },
            Lifetime {
                timer: Timer::from_seconds(4.5, TimerMode::Once),
            },
        ));
    }

    // Smoldering ember (glow + light).
    let ember_emissive = LinearRgba::rgb(3.2, 0.8, 0.15);
    let ember_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.06, 0.02),
        emissive: ember_emissive,
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(fx.ember_mesh.clone()),
        MeshMaterial3d(ember_mat.clone()),
        Transform::from_translation(pos + Vec3::Y * 0.35),
        PointLight {
            color: Color::srgb(1.0, 0.45, 0.15),
            intensity: 280_000.0,
            range: 24.0,
            shadows_enabled: false,
            ..default()
        },
        Smolder {
            age: 0.0,
            ttl: 5.0,
            base_intensity: 280_000.0,
            base_emissive: ember_emissive,
            mat: Some(ember_mat),
            flicker: true,
        },
    ));
}

/// A small dust puff where a machine-gun round hits the ground.
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

/// A muzzle flash: a big star plus a couple of fire wisps. `scale` sizes it (the
/// main gun uses a much larger value than the machine gun).
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
        // Extra fire and a smoke wisp for the main gun.
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

/// Parameters for a fading polygon particle.
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
        // Horizontal drift decays; vertical rise stays.
        tf.translation += p.vel * dt;
        tf.translation.y += p.rise * dt;
        p.vel.x *= 1.0 - (1.4 * dt).min(0.9);
        p.vel.z *= 1.0 - (1.4 * dt).min(0.9);

        let scale = p.start_scale * (1.0 + p.expand * t);
        tf.scale = Vec3::splat(scale);
        // Face the camera and spin about the view axis.
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut smolders: Query<(Entity, &mut Smolder, &mut PointLight)>,
) {
    let dt = time.delta_secs();
    for (entity, mut s, mut light) in &mut smolders {
        s.age += dt;
        if s.age >= s.ttl {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        let t = s.age / s.ttl;
        let mut dim = 1.0 - t;
        if s.flicker {
            dim *= 0.72 + 0.28 * (s.age * 26.0).sin().abs();
        }
        light.intensity = s.base_intensity * dim;
        if let Some(handle) = &s.mat {
            if let Some(mat) = materials.get_mut(handle) {
                mat.emissive = LinearRgba::rgb(
                    s.base_emissive.red * dim,
                    s.base_emissive.green * dim,
                    s.base_emissive.blue * dim,
                );
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

/// A flat n-gon in the XY plane: opaque center vertex fading to a transparent
/// rim (a radial gradient baked into vertex colors).
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
        let alpha = if i % 2 == 0 { 0.15 } else { 0.75 };
        colors.push([1.0, 1.0, 1.0, alpha]);
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
