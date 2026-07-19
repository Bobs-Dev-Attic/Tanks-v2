//! Procedural low-poly WWII tanks and their tread animation.
//!
//! Each tank is assembled from primitive meshes (no external models) so the
//! whole game ships as a single self-contained wasm binary. Treads use a
//! repeating stripe texture that scrolls with speed, and the road wheels spin,
//! selling the tracked-vehicle motion.

use crate::control::PlayerControlled;
use crate::physics::Vehicle;
use crate::terrain::Terrain;
use crate::weapons::{GunMount, Muzzle, Turret, Weapons};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::f32::consts::FRAC_PI_2;

const WHEEL_RADIUS: f32 = 0.45;

pub struct TankPlugin;

impl Plugin for TankPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_training_mission)
            .add_systems(Update, animate_treads);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Player,
    Enemy,
}

/// Root marker for a tank.
#[derive(Component)]
pub struct Tank {
    pub team: Team,
}

/// Handles/entities needed to animate a specific tank instance.
#[derive(Component)]
pub struct TankVisual {
    tread_material: Handle<StandardMaterial>,
    wheels: Vec<Entity>,
    spin: f32,
}

/// A spinning road wheel.
#[derive(Component)]
pub struct RoadWheel;

/// Training mission: a single tank the player drives with the keyboard.
fn spawn_training_mission(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    terrain: Option<Res<Terrain>>,
) {
    // Shared meshes reused by every tank.
    let hull_mesh = meshes.add(Cuboid::new(2.8, 1.0, 4.8));
    let turret_mesh = meshes.add(Cuboid::new(2.0, 0.8, 2.2));
    let mantlet_mesh = meshes.add(Cuboid::new(1.2, 0.7, 0.5));
    let barrel_mesh = meshes.add(Cylinder::new(0.12, 2.8));
    let tread_mesh = meshes.add(Cuboid::new(0.75, 0.75, 5.2));
    let wheel_mesh = meshes.add(Cylinder::new(WHEEL_RADIUS, 0.32));
    let cupola_mesh = meshes.add(Cuboid::new(0.7, 0.4, 0.7));

    let tread_tex = tread_texture(&mut images);
    let detail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.13),
        perceptual_roughness: 0.9,
        ..default()
    });

    let center = terrain
        .as_ref()
        .map(|t| t.spawn_area_center())
        .unwrap_or(Vec2::ZERO);
    // Start resting on the ground (or just above; physics snaps it down).
    let ground = terrain
        .as_ref()
        .map(|t| t.height_at(center.x, center.y))
        .unwrap_or(0.0);

    spawn_tank(
        &mut commands,
        SpawnAssets {
            hull_mesh: &hull_mesh,
            turret_mesh: &turret_mesh,
            mantlet_mesh: &mantlet_mesh,
            barrel_mesh: &barrel_mesh,
            tread_mesh: &tread_mesh,
            wheel_mesh: &wheel_mesh,
            cupola_mesh: &cupola_mesh,
            tread_tex: &tread_tex,
            detail_mat: &detail_mat,
        },
        &mut materials,
        center,
        ground + 0.6,
        0.0,
        Team::Player,
    );
}

/// Bundle of shared asset handles passed to [`spawn_tank`].
struct SpawnAssets<'a> {
    hull_mesh: &'a Handle<Mesh>,
    turret_mesh: &'a Handle<Mesh>,
    mantlet_mesh: &'a Handle<Mesh>,
    barrel_mesh: &'a Handle<Mesh>,
    tread_mesh: &'a Handle<Mesh>,
    wheel_mesh: &'a Handle<Mesh>,
    cupola_mesh: &'a Handle<Mesh>,
    tread_tex: &'a Handle<Image>,
    detail_mat: &'a Handle<StandardMaterial>,
}

fn spawn_tank(
    commands: &mut Commands,
    assets: SpawnAssets,
    materials: &mut Assets<StandardMaterial>,
    ground_xz: Vec2,
    start_y: f32,
    yaw: f32,
    team: Team,
) {
    let body_color = match team {
        Team::Player => Color::srgb(0.30, 0.35, 0.18),
        Team::Enemy => Color::srgb(0.30, 0.31, 0.34),
    };
    let hull_mat = materials.add(StandardMaterial {
        base_color: body_color,
        perceptual_roughness: 0.85,
        metallic: 0.05,
        ..default()
    });
    // Per-instance tread material so each tank scrolls at its own speed.
    let tread_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.52),
        base_color_texture: Some(assets.tread_tex.clone()),
        perceptual_roughness: 1.0,
        ..default()
    });

    let start = Vec3::new(ground_xz.x, start_y, ground_xz.y);

    let mut vehicle = Vehicle::default();
    vehicle.yaw = yaw;
    if team == Team::Enemy {
        vehicle.max_speed = 0.0; // enemies hold position for this slice
    }

    let mut wheels = Vec::new();
    let root = commands
        .spawn((
            Tank { team },
            vehicle,
            Transform::from_translation(start).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            Name::new("Tank"),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        // Hull.
        p.spawn((
            Mesh3d(assets.hull_mesh.clone()),
            MeshMaterial3d(hull_mat.clone()),
            Transform::from_xyz(0.0, 0.75, 0.0),
        ));
        // Treads (left / right).
        for side in [-1.0f32, 1.0] {
            p.spawn((
                Mesh3d(assets.tread_mesh.clone()),
                MeshMaterial3d(tread_material.clone()),
                Transform::from_xyz(side * 1.45, 0.4, 0.0),
            ));
        }
        // Rotating turret assembly. The turret pivot yaws (traverse); a nested
        // gun-mount pivot at the trunnion pitches (elevation). Turret box and
        // cupola yaw only; the mantlet, barrel, and muzzle elevate with the gun.
        p.spawn((Transform::default(), Visibility::default(), Turret::new(0.9)))
            .with_children(|t| {
                t.spawn((
                    Mesh3d(assets.turret_mesh.clone()),
                    MeshMaterial3d(hull_mat.clone()),
                    Transform::from_xyz(0.0, 1.55, 0.2),
                ));
                t.spawn((
                    Mesh3d(assets.cupola_mesh.clone()),
                    MeshMaterial3d(hull_mat.clone()),
                    Transform::from_xyz(0.45, 2.05, 0.7),
                ));
                // Gun mount pivots at the trunnion (0, 1.5, -0.9); its children
                // are positioned relative to that point.
                t.spawn((
                    Transform::from_xyz(0.0, 1.5, -0.9),
                    Visibility::default(),
                    GunMount::new(0.5),
                ))
                .with_children(|g| {
                    g.spawn((
                        Mesh3d(assets.mantlet_mesh.clone()),
                        MeshMaterial3d(hull_mat.clone()),
                        Transform::default(),
                    ));
                    g.spawn((
                        Mesh3d(assets.barrel_mesh.clone()),
                        MeshMaterial3d(assets.detail_mat.clone()),
                        Transform::from_xyz(0.0, 0.0, -1.5)
                            .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
                    ));
                    g.spawn((Transform::from_xyz(0.0, 0.0, -3.0), Muzzle));
                });
            });
        // Road wheels along each track.
        for side in [-1.0f32, 1.0] {
            for k in 0..5 {
                let z = -2.0 + k as f32 * 1.0;
                let e = p
                    .spawn((
                        Mesh3d(assets.wheel_mesh.clone()),
                        MeshMaterial3d(assets.detail_mat.clone()),
                        Transform::from_xyz(side * 1.45, 0.4, z)
                            .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                        RoadWheel,
                    ))
                    .id();
                wheels.push(e);
            }
        }
    });

    commands.entity(root).insert(TankVisual {
        tread_material,
        wheels,
        spin: 0.0,
    });

    if team == Team::Player {
        commands
            .entity(root)
            .insert((PlayerControlled, Weapons::default()));
    }
}

/// Scroll tread textures and spin road wheels in proportion to ground speed.
fn animate_treads(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tanks: Query<(&Vehicle, &mut TankVisual)>,
    mut wheels: Query<&mut Transform, With<RoadWheel>>,
) {
    let dt = time.delta_secs();
    for (vehicle, mut visual) in &mut tanks {
        visual.spin += vehicle.forward_speed * dt / WHEEL_RADIUS;
        if let Some(mat) = materials.get_mut(&visual.tread_material) {
            mat.uv_transform.translation.y -= vehicle.forward_speed * dt * 0.18;
        }
        // Roll about the wheel's own axle (the cylinder's central axis, local
        // Y) — then lay it on its side so the axle points across the hull.
        let rot = Quat::from_rotation_z(FRAC_PI_2) * Quat::from_rotation_y(visual.spin);
        for &wheel in &visual.wheels {
            if let Ok(mut transform) = wheels.get_mut(wheel) {
                transform.rotation = rot;
            }
        }
    }
}

/// Build a small repeating stripe texture that reads as tank-track cleats.
fn tread_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let w = 8u32;
    let h = 8u32;
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for _x in 0..w {
            let cleat = (y / 2) % 2 == 0;
            let c: [u8; 4] = if cleat {
                [38, 38, 42, 255]
            } else {
                [66, 66, 70, 255]
            };
            data.extend_from_slice(&c);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });
    images.add(image)
}
