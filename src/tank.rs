//! Procedural low-poly WWII tanks with a detailed hull, gradient-shaded armor,
//! and running gear that mimics real tracks: drive sprocket, idler, road wheels,
//! and return rollers all spin at speeds set by their radius, while cleated
//! track links march around the bottom run and the track band scrolls.

use crate::combat::{Armor, TankRig};
use crate::control::PlayerControlled;
use crate::effects::{spawn_dust, spawn_track_mark, EffectAssets, Wreckage};
use crate::physics::Vehicle;
use crate::terrain::Terrain;
use crate::weapons::{GunMount, HullMg, Muzzle, Shake, Turret, Weapons};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::f32::consts::{FRAC_PI_2, PI};

/// Length of the visible bottom track run (where links march and wrap).
const TRACK_RUN: f32 = 4.8;
/// Track centre line offset from the hull centre.
const TRACK_X: f32 = 1.45;

pub struct TankPlugin;

impl Plugin for TankPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_training_mission)
            .add_systems(Update, (animate_tracks, emit_track_effects));
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
    /// Absolute ground travelled, for spacing dust and track marks.
    traveled: f32,
    last_dust: f32,
    last_mark: f32,
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

    // A few dark-gray enemy panzers downrange to shoot at. The tank's front is
    // -Z, so these sit ahead of the player; yaw PI turns them to face back.
    for (dx, dz) in [(-20.0, -34.0), (6.0, -44.0), (30.0, -28.0)] {
        let px = center.x + dx;
        let pz = center.y + dz;
        let g = terrain
            .as_ref()
            .map(|t| t.height_at(px, pz))
            .unwrap_or(0.0);
        spawn_tank(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut images,
            Vec2::new(px, pz),
            g + 0.6,
            PI,
            Team::Enemy,
        );
    }
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
    // German WWII tanks (Tiger/Panzer look) get a boxier, bigger turret, a long
    // overhanging gun, a commander's cupola, and side skirts (Schürzen).
    let german = team == Team::Enemy;

    // --- Meshes ---
    let hull_mesh = meshes.add(gradient_box(2.8, 1.0, 4.8, 1.15, 0.6));
    let glacis_mesh = meshes.add(gradient_box(2.7, 0.95, 0.16, 1.2, 0.7));
    let deck_mesh = meshes.add(gradient_box(2.5, 0.18, 2.0, 1.05, 0.75));
    let fender_mesh = meshes.add(Cuboid::new(0.95, 0.1, 5.2));
    // Big slab turret for the German tanks, rounder for the player.
    let turret_mesh = if german {
        meshes.add(gradient_box(2.5, 0.95, 2.7, 1.15, 0.6))
    } else {
        meshes.add(gradient_box(2.0, 0.8, 2.2, 1.15, 0.65))
    };
    let bustle_mesh = meshes.add(gradient_box(1.5, 0.55, 0.7, 1.05, 0.75));
    let mantlet_mesh = meshes.add(gradient_box(1.2, 0.75, 0.5, 1.1, 0.7));
    let barrel_len = if german { 3.8 } else { 2.8 };
    let barrel_mesh = meshes.add(Cylinder::new(if german { 0.13 } else { 0.12 }, barrel_len));
    let brake_mesh = meshes.add(Cuboid::new(0.34, 0.34, 0.5));
    let hatch_mesh = meshes.add(Cylinder::new(0.3, 0.14));
    let cupola_mesh = meshes.add(Cylinder::new(0.42, 0.36));
    let skirt_mesh = meshes.add(Cuboid::new(0.08, 0.5, 4.6));
    let bowplate_mesh = meshes.add(gradient_box(2.7, 1.0, 0.16, 1.2, 0.7));
    let antenna_mesh = meshes.add(Cylinder::new(0.03, 2.2));
    let exhaust_mesh = meshes.add(Cylinder::new(0.11, 0.9));
    let headlight_mesh = meshes.add(Cuboid::new(0.22, 0.22, 0.12));
    let mg_barrel_mesh = meshes.add(Cylinder::new(0.05, 0.7));
    let band_mesh = meshes.add(Cuboid::new(0.78, 0.55, 5.0));
    let sprocket_mesh = meshes.add(Cylinder::new(0.55, 0.36));
    let roadwheel_mesh = meshes.add(Cylinder::new(0.42, 0.32));
    let roller_mesh = meshes.add(Cylinder::new(0.18, 0.28));
    let link_mesh = meshes.add(Cuboid::new(0.82, 0.14, 0.34));

    // --- Materials ---
    let body_color = match team {
        Team::Player => Color::srgb(0.32, 0.37, 0.2),
        // Panzergrau — a dark battleship gray.
        Team::Enemy => Color::srgb(0.15, 0.16, 0.18),
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

    // Only the player's tank gets working weapon components (Turret / GunMount /
    // Muzzle / HullMg). Enemy panzers are static targets, so their turret and gun
    // are purely visual — this also keeps the player's parts unique singletons
    // for the weapon systems.
    let is_player = team == Team::Player;

    let start = Vec3::new(ground_xz.x, start_y, ground_xz.y);
    let mut vehicle = Vehicle::default();
    vehicle.yaw = yaw;
    // Enemies keep a normal max_speed (a zero would divide-by-zero in the
    // physics' speed factor and NaN out their position). Nothing writes their
    // throttle, so they stay parked — and they're immovable, so driving into one
    // stops the player rather than shoving it aside.
    if german {
        vehicle.radius = 3.0;
        vehicle.movable = false;
    }

    let mut wheels: Vec<(Entity, f32)> = Vec::new();
    let mut links: Vec<(Entity, f32)> = Vec::new();
    let mut turret_ent: Option<Entity> = None;

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
        // The tank's front is -Z (the way it drives and the gun points at rest).
        // Sloped front glacis.
        p.spawn((
            Mesh3d(glacis_mesh.clone()),
            MeshMaterial3d(hull_mat.clone()),
            Transform::from_xyz(0.0, 0.75, -2.45).with_rotation(Quat::from_rotation_x(0.6)),
        ));
        // Engine deck (rear, +Z).
        p.spawn((
            Mesh3d(deck_mesh.clone()),
            MeshMaterial3d(accent_mat.clone()),
            Transform::from_xyz(0.0, 1.28, 1.3),
        ));
        // Fenders over each track.
        for s in [-1.0f32, 1.0] {
            p.spawn((
                Mesh3d(fender_mesh.clone()),
                MeshMaterial3d(accent_mat.clone()),
                Transform::from_xyz(s * TRACK_X, 1.05, 0.0),
            ));
        }
        // Exhausts (rear) on both; headlights only on the player (German tanks
        // instead get a flat vertical bow plate and side skirts below).
        for s in [-1.0f32, 1.0] {
            if !german {
                p.spawn((
                    Mesh3d(headlight_mesh.clone()),
                    MeshMaterial3d(light_mat.clone()),
                    Transform::from_xyz(s * 0.95, 1.02, -2.45),
                ));
            }
            p.spawn((
                Mesh3d(exhaust_mesh.clone()),
                MeshMaterial3d(metal_mat.clone()),
                Transform::from_xyz(s * 1.0, 1.1, 2.5)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ));
        }
        // German cues: a near-vertical bow plate and Schürzen side skirts.
        if german {
            p.spawn((
                Mesh3d(bowplate_mesh.clone()),
                MeshMaterial3d(hull_mat.clone()),
                Transform::from_xyz(0.0, 1.0, -2.45).with_rotation(Quat::from_rotation_x(0.18)),
            ));
            for s in [-1.0f32, 1.0] {
                p.spawn((
                    Mesh3d(skirt_mesh.clone()),
                    MeshMaterial3d(accent_mat.clone()),
                    Transform::from_xyz(s * (TRACK_X + 0.18), 0.95, 0.0),
                ));
            }
        }
        // Hull machine gun at the front (co-driver's position). The marker is
        // an unrotated point so its -Z is hull-forward; a short barrel is visual.
        // Only the player fires, so only the player gets the HullMg marker.
        if is_player {
            p.spawn((
                Transform::from_xyz(0.55, 0.95, -2.4),
                Visibility::default(),
                HullMg,
            ));
        }
        p.spawn((
            Mesh3d(mg_barrel_mesh.clone()),
            MeshMaterial3d(metal_mat.clone()),
            Transform::from_xyz(0.55, 0.95, -2.7).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        ));

        // --- Turret assembly (yaw) with nested gun mount (pitch) ---
        // German turret sits a touch forward; the gun overhangs the bow.
        let turret_z = if german { 0.0 } else { 0.2 };
        let barrel_z = -(barrel_len / 2.0 + 0.1);
        let brake_z = barrel_z - barrel_len / 2.0 + 0.05;
        let mut turret_ec = p.spawn((Transform::default(), Visibility::default()));
        if is_player {
            turret_ec.insert(Turret::new(0.9));
        }
        turret_ent = Some(turret_ec.id());
        turret_ec.with_children(|t| {
                t.spawn((
                    Mesh3d(turret_mesh.clone()),
                    MeshMaterial3d(hull_mat.clone()),
                    Transform::from_xyz(0.0, 1.55, turret_z),
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
                // German commander's cupola: a drum on the turret roof rear.
                if german {
                    t.spawn((
                        Mesh3d(cupola_mesh.clone()),
                        MeshMaterial3d(hull_mat.clone()),
                        Transform::from_xyz(-0.55, 2.05, 0.7),
                    ));
                }
                // Gun mount at the trunnion (functional only for the player).
                let mut gun_ec = t.spawn((Transform::from_xyz(0.0, 1.5, -0.9), Visibility::default()));
                if is_player {
                    gun_ec.insert(GunMount::new(0.5));
                }
                gun_ec.with_children(|g| {
                    g.spawn((
                        Mesh3d(mantlet_mesh.clone()),
                        MeshMaterial3d(hull_mat.clone()),
                        Transform::default(),
                    ));
                    g.spawn((
                        Mesh3d(barrel_mesh.clone()),
                        MeshMaterial3d(metal_mat.clone()),
                        Transform::from_xyz(0.0, 0.0, barrel_z)
                            .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
                    ));
                    // Muzzle brake at the tip.
                    g.spawn((
                        Mesh3d(brake_mesh.clone()),
                        MeshMaterial3d(metal_mat.clone()),
                        Transform::from_xyz(0.0, 0.0, brake_z),
                    ));
                    if is_player {
                        g.spawn((Transform::from_xyz(0.0, 0.0, brake_z - 0.25), Muzzle));
                    }
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
        traveled: 0.0,
        last_dust: 0.0,
        last_mark: 0.0,
    });

    // Health and damageable-visual rig for all tanks. German tanks are tougher
    // (thick armor) but they're the ones taking fire.
    commands.entity(root).insert((
        Armor::new(if german { 150.0 } else { 200.0 }),
        TankRig {
            hull_mat: hull_mat.clone(),
            base_color: body_color,
            turret: turret_ent,
            hull_top: 1.7,
        },
    ));

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

/// Kick up ground-tinted dust and leave track marks as the tank drives.
#[allow(clippy::too_many_arguments)]
fn emit_track_effects(
    time: Res<Time>,
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    fx: Option<Res<EffectAssets>>,
    mut wreckage: ResMut<Wreckage>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tanks: Query<(&Vehicle, &Transform, &mut TankVisual)>,
) {
    let (Some(terrain), Some(fx)) = (terrain, fx) else {
        return;
    };
    let dt = time.delta_secs();
    for (vehicle, tf, mut visual) in &mut tanks {
        let speed = vehicle.forward_speed.abs();
        if speed < 0.6 {
            continue;
        }
        visual.traveled += speed * dt;
        let do_dust = visual.traveled - visual.last_dust > 0.5;
        let do_mark = visual.traveled - visual.last_mark > 0.7;
        if !do_dust && !do_mark {
            continue;
        }
        let seed = (visual.traveled * 350.0) as u32 | 1;
        for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            // Behind the tank (its front is -Z, so the rear is +Z).
            let rear = tf.translation + tf.rotation * Vec3::new(side * TRACK_X, 0.0, 2.3);
            let g = terrain.height_at(rear.x, rear.z);
            let ground = terrain.ground_color(rear.x, rear.z);
            if do_dust {
                let tint = Color::srgba(
                    (ground.x * 1.25 + 0.12).min(1.0),
                    (ground.y * 1.25 + 0.12).min(1.0),
                    (ground.z * 1.25 + 0.12).min(1.0),
                    1.0,
                );
                let scale = 0.45 + speed * 0.03;
                spawn_dust(
                    &mut commands,
                    &fx,
                    &mut materials,
                    Vec3::new(rear.x, g + 0.1, rear.z),
                    tint,
                    scale,
                    seed.wrapping_add(i as u32 * 7919),
                );
            }
            if do_mark {
                let dark = Color::srgba(ground.x * 0.42, ground.y * 0.42, ground.z * 0.42, 0.55);
                spawn_track_mark(
                    &mut commands,
                    &fx,
                    &mut materials,
                    &mut wreckage,
                    Vec3::new(rear.x, g, rear.z),
                    dark,
                );
            }
        }
        if do_dust {
            visual.last_dust = visual.traveled;
        }
        if do_mark {
            visual.last_mark = visual.traveled;
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
