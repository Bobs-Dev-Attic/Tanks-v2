//! Isometric orbit camera.
//!
//! An orthographic projection tilted to a classic isometric angle. The player
//! can orbit (yaw + pitch), zoom (orthographic scale), and pan the focus across
//! the battlefield. All input arrives pre-digested in [`crate::input::GameInput`],
//! so this module only turns those requests into camera motion.

use crate::control::PlayerControlled;
use crate::input::GameInput;
use crate::terrain::{Terrain, MAP_SIZE};
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, drive_camera);
    }
}

/// State for the orbiting isometric camera.
#[derive(Component)]
pub struct IsoCamera {
    /// Point the camera looks at and orbits around.
    focus: Vec3,
    /// Rotation around the world Y axis, radians.
    yaw: f32,
    /// Elevation angle above the ground, radians.
    pitch: f32,
    /// Orthographic vertical extent in world units — smaller is more zoomed in.
    scale: f32,
    /// Discrete zoom level, 0 (closest) .. ZOOM_LEVELS-1 (farthest).
    zoom_level: usize,
    /// Accumulates zoom input until it crosses a whole level step.
    zoom_accum: f32,
}

impl Default for IsoCamera {
    fn default() -> Self {
        let zoom_level = 3;
        Self {
            focus: Vec3::ZERO,
            yaw: std::f32::consts::FRAC_PI_4, // 45°
            pitch: 35.264_f32.to_radians(),   // true isometric
            scale: scale_for_level(zoom_level),
            zoom_level,
            zoom_accum: 0.0,
        }
    }
}

const MIN_SCALE: f32 = 9.0;
const MAX_SCALE: f32 = 360.0;
/// Number of discrete zoom steps.
const ZOOM_LEVELS: usize = 10;

/// Orthographic scale for a discrete zoom level (geometric spacing).
fn scale_for_level(level: usize) -> f32 {
    let t = level as f32 / (ZOOM_LEVELS - 1) as f32;
    MIN_SCALE * (MAX_SCALE / MIN_SCALE).powf(t)
}
const MIN_PITCH: f32 = 12.0_f32 * std::f32::consts::PI / 180.0;
const MAX_PITCH: f32 = 82.0_f32 * std::f32::consts::PI / 180.0;

fn spawn_camera(mut commands: Commands) {
    let iso = IsoCamera::default();
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 1.0,
            },
            scale: iso.scale,
            near: -1000.0,
            far: 3000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::default(),
        iso,
        Name::new("IsoCamera"),
    ));
}

fn drive_camera(
    time: Res<Time>,
    input: Res<GameInput>,
    terrain: Option<Res<Terrain>>,
    mut cameras: Query<(&mut IsoCamera, &mut Transform, &mut Projection)>,
    players: Query<&Transform, (With<PlayerControlled>, Without<IsoCamera>)>,
) {
    let dt = time.delta_secs();
    let Ok((mut iso, mut transform, mut projection)) = cameras.get_single_mut() else {
        return;
    };

    // Orbit.
    iso.yaw -= input.orbit.x.to_radians();
    iso.pitch = (iso.pitch - input.orbit.y.to_radians()).clamp(MIN_PITCH, MAX_PITCH);

    // Discrete zoom: each notch steps one of ZOOM_LEVELS levels; the scale then
    // eases toward the level's target for a smooth transition.
    iso.zoom_accum += input.zoom;
    while iso.zoom_accum >= 1.0 {
        iso.zoom_level = iso.zoom_level.saturating_sub(1);
        iso.zoom_accum -= 1.0;
    }
    while iso.zoom_accum <= -1.0 {
        iso.zoom_level = (iso.zoom_level + 1).min(ZOOM_LEVELS - 1);
        iso.zoom_accum += 1.0;
    }
    let target_scale = scale_for_level(iso.zoom_level);
    iso.scale += (target_scale - iso.scale) * (dt * 12.0).min(1.0);

    // Follow the player tank: ease the focus toward it, plus any manual pan
    // (e.g. two-finger drag on mobile) as an offset.
    let mut target = iso.focus;
    if let Ok(player) = players.get_single() {
        target.x = player.translation.x;
        target.z = player.translation.z;
    }
    if input.pan != Vec2::ZERO {
        let right = Vec3::new(iso.yaw.cos(), 0.0, -iso.yaw.sin());
        let forward = Vec3::new(-iso.yaw.sin(), 0.0, -iso.yaw.cos());
        let world_per_px = iso.scale * 0.0016;
        target += (right * input.pan.x + forward * -input.pan.y) * world_per_px;
    }
    let limit = MAP_SIZE * 0.5 - 8.0;
    iso.focus.x += (target.x - iso.focus.x) * 0.12;
    iso.focus.z += (target.z - iso.focus.z) * 0.12;
    iso.focus.x = iso.focus.x.clamp(-limit, limit);
    iso.focus.z = iso.focus.z.clamp(-limit, limit);

    // Keep the focus point resting on the terrain for nice framing.
    if let Some(terrain) = terrain.as_ref() {
        let ground = terrain.height_at(iso.focus.x, iso.focus.z);
        iso.focus.y = iso.focus.y + (ground - iso.focus.y) * 0.15;
    }

    // Recompute the camera transform from the orbit parameters.
    let rot = Quat::from_rotation_y(iso.yaw) * Quat::from_rotation_x(-iso.pitch);
    let offset = rot * Vec3::new(0.0, 0.0, 600.0);
    transform.translation = iso.focus + offset;
    transform.look_at(iso.focus, Vec3::Y);

    if let Projection::Orthographic(ortho) = projection.as_mut() {
        ortho.scale = iso.scale;
    }
}
