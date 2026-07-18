//! Isometric orbit camera.
//!
//! An orthographic projection tilted to a classic isometric angle. The player
//! can orbit (yaw + pitch), zoom (orthographic scale), and pan the focus across
//! the battlefield. All input arrives pre-digested in [`crate::input::GameInput`],
//! so this module only turns those requests into camera motion.

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
}

impl Default for IsoCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            yaw: std::f32::consts::FRAC_PI_4, // 45°
            pitch: 35.264_f32.to_radians(),   // true isometric
            scale: 70.0,
        }
    }
}

const MIN_SCALE: f32 = 14.0;
const MAX_SCALE: f32 = 200.0;
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
    input: Res<GameInput>,
    terrain: Option<Res<Terrain>>,
    mut cameras: Query<(&mut IsoCamera, &mut Transform, &mut Projection)>,
) {
    let Ok((mut iso, mut transform, mut projection)) = cameras.get_single_mut() else {
        return;
    };

    // Orbit.
    iso.yaw -= input.orbit.x.to_radians();
    iso.pitch = (iso.pitch - input.orbit.y.to_radians()).clamp(MIN_PITCH, MAX_PITCH);

    // Zoom (scale shrinks as we zoom in).
    iso.scale = (iso.scale - input.zoom).clamp(MIN_SCALE, MAX_SCALE);

    // Pan across the ground plane, relative to current facing. Speed scales with
    // zoom so panning feels consistent at any zoom level.
    if input.pan != Vec2::ZERO {
        let forward = Vec3::new(-iso.yaw.sin(), 0.0, -iso.yaw.cos());
        let right = Vec3::new(iso.yaw.cos(), 0.0, -iso.yaw.sin());
        let world_per_px = iso.scale * 0.0016;
        let mut focus = iso.focus + (right * input.pan.x + forward * -input.pan.y) * world_per_px;
        let limit = MAP_SIZE * 0.5 - 8.0;
        focus.x = focus.x.clamp(-limit, limit);
        focus.z = focus.z.clamp(-limit, limit);
        iso.focus = focus;
    }

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
