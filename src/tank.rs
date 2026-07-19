//! Procedural low-poly WWII tanks with a detailed hull, gradient-shaded armor,
//! and running gear that mimics real tracks: drive sprocket, idler, road wheels,
//! and return rollers all spin at speeds set by their radius, while cleated
//! track links march around the bottom run and the track band scrolls.

use crate::control::PlayerControlled;
use crate::physics::Vehicle;
use crate::terrain::Terrain;
use crate::weapons::{GunMount, Muzzle, Shake, Turret, Weapons};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::f32::consts::FRAC_PI_2;

/// Length of the visible bottom track run (where links march and wrap).
const TRACK_RUN: f32 = 4.8;
/// Track centre line offset from the hull centre.
const TRACK_X: f32 = 1.45;

pub struct TankPlugin;

impl Plugin for TankPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_training_mission)
            .add_systems(Update, animate_tracks);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Player,
    Enemy,
}

#[derive(Component)]
pub struct Tank {
    pub team: Team,
}

/// Everything needed to animate one tank's running gear.
#[derive(Component)]
pub struct TankVisual {
    tread_material: Handle<StandardMaterial>,
    /// Rolling wheels as (entity, radius); angular speed = distance / radius.
    wheels: Vec<(Entity, f32)>,
    /// Track-link cleats as (entity, base position along the run).
    links: Vec<(Entity, f32)>,
    /// Distance the tracks have travelled, for wheel spin and link marching.
    distance: f32,
}

fn spawn_training_mission(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    terrain: Option<Res<Terrain>>,
) {
    let center = terrain
        .as_ref()
        .map(|t| t.spawn_area_center())
        .unwrap_or(Vec2::ZERO);
    let ground = terrain
        .as_ref()
        .map(|t| t.height_at(center.x, center.y))
        .unwrap_or(0.0);

    spawn_tank(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
        center,
        ground + 0.6,
        0.0,
        Team::Player,
    );
}

fn spawn_tank(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    ground_xz: Vec2,
    start_y: f32,
    yaw: f32,
    team: Team,
) {
    // --- Meshes ---
    let hull_mesh = meshes.add(gradient_box(2.8, 1.0, 4.8, 1.15, 0.6));
    let glacis_mesh = meshes.add(gradient_box(2.7, 0.95, 0.16, 1.2, 0.7));
    let deck_mesh = meshes.add(gradient_box(2.5, 0.18, 2.0, 1.05, 0.75));
    let fender_mesh = meshes.add(Cuboid::new(0.95, 0.1, 5.2));
    let turret_mesh = meshes.add(gradient_box(2.0, 0.8, 2.2, 1.15, 0.65));
    let bustle_mesh = meshes.add(gradient_box(1.5, 0.55, 0.7, 1.05, 0.75));
    let mantlet_mesh = meshes.add(gradient_box(1.2, 0.75, 0.5, 1.1, 0.7));
    let barrel_mesh = meshes.add(Cylinder::new(0.12, 2.8));
    let brake_mesh = meshes.add(Cuboid::new(0.34, 0.34, 0.5));
    let hatch_mesh = meshes.add(Cylinder::new(0.3, 0.14));
    let antenna_mesh = meshes.add(Cylinder::new(0.03, 2.2));
    let exhaust_mesh = meshes.add(Cylinder::new(0.11, 0.9));
    let headlight_mesh = meshes.add(Cuboid::new(0.22, 0.22, 0.12));
    let band_mesh = meshes.add(Cuboid::new(0.78, 0.55, 5.0));
    let sprocket_mesh = meshes.add(Cylinder::new(0.55, 0.36));
    let roadwheel_mesh = meshes.add(Cylinder::new(0.42, 0.32));
    let roller_mesh = meshes.add(Cylinder::new(0.18, 0.28));
    let link_mesh = meshes.add(Cuboid::new(0.82, 0.14, 0.34));

    // --- Materials ---
    let body_color = match team {
        Team::Player => Color::srgb(0.32, 0.37, 0.2),
        Team::Enemy => Color::srgb(0.31, 0.32, 0.35),
    };
    let hull_mat = materials.add(StandardMaterial {
        base_color: body_color,
        perceptual_roughness: 0.85,
        metallic: 0.05,
        ..default()
    });
    let accent_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.25, 0.14),
        perceptual_roughness: 0.9,
        ..default()
    });
    let metal_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.14, 0.15),
        perceptual_roughness: 0.7,
        metallic: 0.4,
        ..default()
    });
    let dark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.09),
        perceptual_roughness: 1.0,
        ..default()
    });
    let light_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.85, 0.7),
        emissive: LinearRgba::rgb(0.9, 0.85, 0.5),
        ..default()
    });
    let tread_tex = tread_texture(images);
    let tread_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.4, 0.42),
        base_color_texture: Some(tread_tex),
        perceptual_roughness: 1.0,
        ..default()
    });

    let start = Vec3::new(ground_xz.x, start_y, ground_xz.y);
    let mut vehicle = Vehicle::default();
    vehicle.yaw = yaw;
    if team == Team::Enemy {
        vehicle.max_speed = 0.0;
    }

    let mut wheels: Vec<(Entity, f32)> = Vec::new();
    let mut links: Vec<(Entity, f32)> = Vec::new();

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
        // --- Hull & armor ---
        p.spawn((
            Mesh3d(hull_mesh.clone()),
            MeshMaterial3d(hull_mat.clone()),
            Transform::from_xyz(0.0, 0.75, 0.0),
        ));
        // Sloped front glacis.
        p.spawn((
            Mesh3d(glacis_mesh.clone()),
            MeshMaterial3d(hull_mat.clone()),
            Transform::from_xyz(0.0, 0.75, 2.45).with_rotation(Quat::from_rotation_x(-0.6)),
        ));
        // Engine deck.
        p.spawn((
            Mesh3d(deck_mesh.clone()),
            MeshMaterial3d(accent_mat.clone()),
            Transform::from_xyz(0.0, 1.28, -1.3),
        ));
        // Fenders over each track.
        for s in [-1.0f32, 1.0] {
            p.spawn((
                Mesh3d(fender_mesh.clone()),
                MeshMaterial3d(accent_mat.clone()),
                Transform::from_xyz(s * TRACK_X, 1.05, 0.0),
            ));
        }
        // Headlights and exhausts.
        for s in [-1.0f32, 1.0] {
            p.spawn((
                Mesh3d(headlight_mesh.clone()),
                MeshMaterial3d(light_mat.clone()),
                Transform::from_xyz(s * 0.95, 1.02, 2.45),
            ));
            p.spawn((
                Mesh3d(exhaust_mesh.clone()),
                MeshMaterial3d(metal_mat.clone()),
                Transform::from_xyz(s * 1.0, 1.1, -2.5)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ));
        }

        // --- Turret assembly (yaw) with nested gun mount (pitch) ---
        p.spawn((Transform::default(), Visibility::default(), Turret::new(0.9)))
            .with_children(|t| {
                t.spawn((
                    Mesh3d(turret_mesh.clone()),
                    MeshMaterial3d(hull_mat.clone()),
                    Transform::from_xyz(0.0, 1.55, 0.2),
                ));
                // Rear stowage bustle.
                t.spawn((
                    Mesh3d(bustle_mesh.clone()),
                    MeshMaterial3d(accent_mat.clone()),
                    Transform::from_xyz(0.0, 1.5, 1.45),
                ));
                // Commander's hatch and antenna.
                t.spawn((
                    Mesh3d(hatch_mesh.clone()),
                    MeshMaterial3d(accent_mat.clone()),
                    Transform::from_xyz(0.45, 2.0, 0.5),
                ));
                t.spawn((
                    Mesh3d(antenna_mesh.clone()),
                    MeshMaterial3d(dark_mat.clone()),
                    Transform::from_xyz(0.8, 3.0, 0.6),
                ));
                // Gun mount at the trunnion.
                t.spawn((
                    Transform::from_xyz(0.0, 1.5, -0.9),
                    Visibility::default(),
                    GunMount::new(0.5),
                ))
                .with_children(|g| {
                    g.spawn((
                        Mesh3d(mantlet_mesh.clone()),
                        MeshMaterial3d(hull_mat.clone()),
                        Transform::default(),
                    ));
                    g.spawn((
                        Mesh3d(barrel_mesh.clone()),
                        MeshMaterial3d(metal_mat.clone()),
                        Transform::from_xyz(0.0, 0.0, -1.5)
                            .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
                    ));
                    // Muzzle brake at the tip.
                    g.spawn((
                        Mesh3d(brake_mesh.clone()),
                        MeshMaterial3d(metal_mat.clone()),
                        Transform::from_xyz(0.0, 0.0, -2.85),
                    ));
                    g.spawn((Transform::from_xyz(0.0, 0.0, -3.1), Muzzle));
                });
            });

        // --- Running gear per side ---
        for s in [-1.0f32, 1.0] {
            let x = s * TRACK_X;
            // Track band (textured, scrolls).
            p.spawn((
                Mesh3d(band_mesh.clone()),
                MeshMaterial3d(tread_material.clone()),
                Transform::from_xyz(x, 0.42, 0.0),
            ));
            // Drive sprocket (front) and idler (rear) — larger.
            for (z, _name) in [(2.4, "sprocket"), (-2.4, "idler")] {
                let e = wheel_entity(p, &sprocket_mesh, &dark_mat, x, 0.55, z);
                wheels.push((e, 0.55));
            }
            // Road wheels.
            for k in 0..5 {
                let z = -1.6 + k as f32 * 0.8;
                let e = wheel_entity(p, &roadwheel_mesh, &metal_mat, x, 0.45, z);
                wheels.push((e, 0.42));
            }
            // Return rollers on the top run.
            for z in [-0.9f32, 0.9] {
                let e = wheel_entity(p, &roller_mesh, &metal_mat, x, 1.05, z);
                wheels.push((e, 0.18));
            }
            // Marching cleats on the bottom run.
            let count = 10;
            for i in 0..count {
                let base = i as f32 / count as f32 * TRACK_RUN;
                let z = base - TRACK_RUN * 0.5;
                let e = p
                    .spawn((
                        Mesh3d(link_mesh.clone()),
                        MeshMaterial3d(dark_mat.clone()),
                        Transform::from_xyz(x, 0.12, z),
                    ))
                    .id();
                links.push((e, base));
            }
        }
    });

    commands.entity(root).insert(TankVisual {
        tread_material,
        wheels,
        links,
        distance: 0.0,
    });

    if team == Team::Player {
        commands
            .entity(root)
            .insert((PlayerControlled, Weapons::default(), Shake::default()));
    }
}

/// Spawn one rolling wheel laid on its side (axle across the hull) and return it.
fn wheel_entity(
    p: &mut ChildBuilder,
    mesh: &Handle<Mesh>,
    mat: &Handle<StandardMaterial>,
    x: f32,
    y: f32,
    z: f32,
) -> Entity {
    p.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(mat.clone()),
        Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
    ))
    .id()
}

/// Spin the running gear and march the track links in step with ground speed.
fn animate_tracks(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tanks: Query<(&Vehicle, &mut TankVisual)>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    for (vehicle, mut visual) in &mut tanks {
        visual.distance += vehicle.forward_speed * dt;
        let distance = visual.distance;

        if let Some(mat) = materials.get_mut(&visual.tread_material) {
            mat.uv_transform.translation.y -= vehicle.forward_speed * dt * 0.18;
        }

        // Wheels roll: angular speed scales inversely with radius.
        for &(wheel, radius) in &visual.wheels {
            if let Ok(mut tf) = transforms.get_mut(wheel) {
                let angle = distance / radius;
                tf.rotation = Quat::from_rotation_z(FRAC_PI_2) * Quat::from_rotation_y(angle);
            }
        }
        // Links march along the bottom run and wrap continuously.
        for &(link, base) in &visual.links {
            if let Ok(mut tf) = transforms.get_mut(link) {
                tf.translation.z = (base - distance).rem_euclid(TRACK_RUN) - TRACK_RUN * 0.5;
            }
        }
    }
}

/// A cuboid with a top-to-bottom grayscale gradient baked into vertex colors,
/// so flat armor plates read with shaded depth (multiplied by the material).
fn gradient_box(x: f32, y: f32, z: f32, top: f32, bottom: f32) -> Mesh {
    let mut mesh: Mesh = Cuboid::new(x, y, z).into();
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        let half = y * 0.5;
        let colors: Vec<[f32; 4]> = positions
            .iter()
            .map(|pos| {
                let t = ((pos[1] + half) / y).clamp(0.0, 1.0);
                let g = bottom + (top - bottom) * t;
                [g, g, g, 1.0]
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
    mesh
}

/// A small repeating stripe texture that reads as track cleats on the band.
fn tread_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let (w, h) = (8u32, 8u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for _x in 0..w {
            let cleat = (y / 2) % 2 == 0;
            let c: [u8; 4] = if cleat {
                [34, 34, 38, 255]
            } else {
                [70, 70, 74, 255]
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
