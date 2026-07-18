//! Explosion effects: flying debris, dust, rising smoke, a muzzle/impact flash,
//! and an ember that smolders and dims out over a few seconds.
//!
//! There is no particle system in Bevy, so each puff/chunk is a small entity
//! with its own timed component; simple update systems move, scale, fade, and
//! despawn them.

use crate::terrain::Terrain;
use bevy::prelude::*;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_effect_assets).add_systems(
            Update,
            (update_debris, update_particles, update_smolder, update_lifetimes),
        );
    }
}

/// Shared meshes/materials reused by every explosion.
#[derive(Resource)]
pub struct EffectAssets {
    debris_mesh: Handle<Mesh>,
    puff_mesh: Handle<Mesh>,
    ember_mesh: Handle<Mesh>,
    debris_mat: Handle<StandardMaterial>,
}

fn setup_effect_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let debris_mesh = meshes.add(Cuboid::new(0.35, 0.35, 0.35));
    let puff_mesh = meshes.add(Sphere::new(1.0));
    let ember_mesh = meshes.add(Sphere::new(0.7));
    let debris_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.09, 0.08),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.insert_resource(EffectAssets {
        debris_mesh,
        puff_mesh,
        ember_mesh,
        debris_mat,
    });
}

/// A chunk of debris thrown by an explosion; falls under gravity and settles.
#[derive(Component)]
struct Debris {
    vel: Vec3,
    spin: Vec3,
    settled: bool,
}

/// A dust or smoke puff that drifts, expands, and fades.
#[derive(Component)]
struct Particle {
    vel: Vec3,
    age: f32,
    ttl: f32,
    expand: f32,
    start_scale: f32,
    start_alpha: f32,
    mat: Handle<StandardMaterial>,
}

/// A glowing light (flash or ember) that dims over its lifetime.
#[derive(Component)]
struct Smolder {
    age: f32,
    ttl: f32,
    base_intensity: f32,
    base_emissive: LinearRgba,
    mat: Option<Handle<StandardMaterial>>,
    flicker: bool,
}

/// Despawn after a fixed time.
#[derive(Component)]
struct Lifetime {
    timer: Timer,
}

const GRAVITY: f32 = 22.0;

/// Spawn a full main-gun impact explosion at `pos` (on the ground).
pub fn spawn_explosion(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    seed: u32,
) {
    let mut rng = Rng::new(seed ^ 0x9E37_79B9);

    // Bright initial flash.
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.72, 0.35),
            intensity: 5_000_000.0,
            range: 45.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(pos + Vec3::Y * 1.6),
        Smolder {
            age: 0.0,
            ttl: 0.22,
            base_intensity: 5_000_000.0,
            base_emissive: LinearRgba::BLACK,
            mat: None,
            flicker: false,
        },
    ));

    // Smoldering ember: a dim, flickering glow that lingers.
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

    // Debris chunks.
    for _ in 0..11 {
        let dir = Vec3::new(rng.range(-1.0, 1.0), rng.range(0.4, 1.5), rng.range(-1.0, 1.0))
            .normalize_or_zero();
        let speed = rng.range(7.0, 17.0);
        let scale = rng.range(0.5, 1.4);
        commands.spawn((
            Mesh3d(fx.debris_mesh.clone()),
            MeshMaterial3d(fx.debris_mat.clone()),
            Transform::from_translation(pos + Vec3::Y * 0.4).with_scale(Vec3::splat(scale)),
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

    // Dust: low, tan, spreading outward.
    for _ in 0..7 {
        let mat = materials.add(puff_material(Color::srgba(0.72, 0.66, 0.5, 0.55)));
        let out = Vec3::new(rng.range(-1.0, 1.0), rng.range(0.1, 0.4), rng.range(-1.0, 1.0))
            .normalize_or_zero()
            * rng.range(3.0, 7.0);
        commands.spawn((
            Mesh3d(fx.puff_mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos + Vec3::Y * rng.range(0.2, 0.8))
                .with_scale(Vec3::splat(0.8)),
            Particle {
                vel: out,
                age: 0.0,
                ttl: rng.range(1.4, 2.0),
                expand: 2.6,
                start_scale: rng.range(0.7, 1.1),
                start_alpha: 0.55,
                mat,
            },
        ));
    }

    // Smoke: dark, rising.
    for _ in 0..8 {
        let mat = materials.add(puff_material(Color::srgba(0.14, 0.14, 0.15, 0.6)));
        let drift = Vec3::new(rng.range(-1.2, 1.2), rng.range(2.5, 4.5), rng.range(-1.2, 1.2));
        commands.spawn((
            Mesh3d(fx.puff_mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos + Vec3::Y * rng.range(0.5, 1.5))
                .with_scale(Vec3::splat(1.0)),
            Particle {
                vel: drift,
                age: 0.0,
                ttl: rng.range(2.4, 3.4),
                expand: 3.0,
                start_scale: rng.range(0.9, 1.4),
                start_alpha: 0.6,
                mat,
            },
        ));
    }
}

/// A small dust puff for a machine-gun round hitting the ground.
pub fn spawn_impact_puff(
    commands: &mut Commands,
    fx: &EffectAssets,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
) {
    let mat = materials.add(puff_material(Color::srgba(0.72, 0.68, 0.55, 0.5)));
    commands.spawn((
        Mesh3d(fx.puff_mesh.clone()),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(pos + Vec3::Y * 0.2).with_scale(Vec3::splat(0.25)),
        Particle {
            vel: Vec3::Y * 1.2,
            age: 0.0,
            ttl: 0.5,
            expand: 2.0,
            start_scale: 0.3,
            start_alpha: 0.5,
            mat,
        },
    ));
}

fn puff_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
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
        let ground = terrain.height_at(tf.translation.x, tf.translation.z) + 0.15;
        if tf.translation.y <= ground {
            tf.translation.y = ground;
            d.vel = Vec3::ZERO;
            d.settled = true;
        }
    }
}

fn update_particles(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut p, mut tf) in &mut particles {
        p.age += dt;
        let t = (p.age / p.ttl).clamp(0.0, 1.0);
        if p.age >= p.ttl {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        // Drift and slow down; smoke keeps rising.
        tf.translation += p.vel * dt;
        p.vel *= 1.0 - (1.5 * dt).min(0.9);
        let scale = p.start_scale * (1.0 + p.expand * t);
        tf.scale = Vec3::splat(scale);
        if let Some(mat) = materials.get_mut(&p.mat) {
            let alpha = p.start_alpha * (1.0 - t);
            mat.base_color = mat.base_color.with_alpha(alpha);
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

/// Tiny deterministic PRNG (xorshift) so effects vary without an external crate.
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
