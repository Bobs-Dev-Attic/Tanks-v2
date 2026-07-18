//! Squad command & control.
//!
//! The player selects tanks (click / tap / drag-box) and issues move orders
//! (right-click, or tap the ground with a live selection). Selected tanks are
//! ringed with a gizmo, the order destination gets a marker, and a steering
//! controller drives each ordered tank toward its formation slot.

use crate::camera::IsoCamera;
use crate::input::GameInput;
use crate::physics::Vehicle;
use crate::tank::{Tank, Team};
use crate::terrain::Terrain;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

/// Distance at which a tank considers its move order complete.
const ARRIVE_RADIUS: f32 = 2.6;
/// Spacing between formation slots when ordering multiple tanks.
const FORMATION_SPACING: f32 = 6.5;

pub struct SquadPlugin;

impl Plugin for SquadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (handle_commands, drive_to_order, draw_markers).chain(),
        );
    }
}

/// Marks a player-controllable tank and carries its current move order.
#[derive(Component, Default)]
pub struct Commandable {
    /// Ground destination (x, z), if the tank has somewhere to be.
    pub order: Option<Vec2>,
    /// Whether this tank is currently selected.
    pub selected: bool,
}

fn handle_commands(
    input: Res<GameInput>,
    cameras: Query<(&Camera, &GlobalTransform), With<IsoCamera>>,
    terrain: Option<Res<Terrain>>,
    mut tanks: Query<(Entity, &Transform, &Tank, &mut Commandable)>,
) {
    let (Some(terrain), Ok((camera, cam_tf))) = (terrain, cameras.get_single()) else {
        return;
    };

    // Box selection completed this frame.
    if let Some((min, max)) = input.box_finished {
        for (_, transform, tank, mut cmd) in &mut tanks {
            if tank.team != Team::Player {
                continue;
            }
            let on_screen = camera
                .world_to_viewport(cam_tf, transform.translation)
                .ok();
            cmd.selected = matches!(on_screen, Some(p)
                if p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y);
        }
        return;
    }

    // Primary action: select a tank under the pointer, else order the selection.
    if let Some(screen) = input.primary_action {
        if let Some(hit) = pick_tank(screen, camera, cam_tf, &tanks) {
            for (entity, _, _, mut cmd) in &mut tanks {
                cmd.selected = entity == hit;
            }
        } else if let Some(ground) = ground_point(screen, camera, cam_tf, &terrain) {
            issue_orders(Vec2::new(ground.x, ground.z), &mut tanks);
        }
        return;
    }

    // Secondary action always issues a move order to the current selection.
    if let Some(screen) = input.secondary_action {
        if let Some(ground) = ground_point(screen, camera, cam_tf, &terrain) {
            issue_orders(Vec2::new(ground.x, ground.z), &mut tanks);
        }
    }
}

/// Distribute the selected tanks into a compact grid formation around `target`.
fn issue_orders(
    target: Vec2,
    tanks: &mut Query<(Entity, &Transform, &Tank, &mut Commandable)>,
) {
    let selected: Vec<Entity> = tanks
        .iter()
        .filter(|(_, _, _, c)| c.selected)
        .map(|(e, _, _, _)| e)
        .collect();
    if selected.is_empty() {
        return;
    }

    let count = selected.len();
    let cols = (count as f32).sqrt().ceil() as usize;
    for (i, entity) in selected.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let offset = Vec2::new(
            (col as f32 - (cols as f32 - 1.0) * 0.5) * FORMATION_SPACING,
            (row as f32) * FORMATION_SPACING,
        );
        if let Ok((_, _, _, mut cmd)) = tanks.get_mut(*entity) {
            cmd.order = Some(target + offset);
        }
    }
}

/// Pick the closest player tank whose screen position is near `screen`.
fn pick_tank(
    screen: Vec2,
    camera: &Camera,
    cam_tf: &GlobalTransform,
    tanks: &Query<(Entity, &Transform, &Tank, &mut Commandable)>,
) -> Option<Entity> {
    const PICK_RADIUS: f32 = 46.0;
    let mut best: Option<(Entity, f32)> = None;
    for (entity, transform, tank, _) in tanks.iter() {
        if tank.team != Team::Player {
            continue;
        }
        let aim = transform.translation + Vec3::Y * 1.4;
        if let Ok(p) = camera.world_to_viewport(cam_tf, aim) {
            let d = p.distance(screen);
            if d < PICK_RADIUS && best.map_or(true, |(_, bd)| d < bd) {
                best = Some((entity, d));
            }
        }
    }
    best.map(|(e, _)| e)
}

/// Cast a ray from the pointer into the world and find where it meets terrain.
fn ground_point(
    screen: Vec2,
    camera: &Camera,
    cam_tf: &GlobalTransform,
    terrain: &Terrain,
) -> Option<Vec3> {
    let ray = camera.viewport_to_world(cam_tf, screen).ok()?;
    let origin = ray.origin;
    let dir = ray.direction.as_vec3();

    // March until the ray dips below the heightfield, then bisect to refine.
    let mut t = 0.0f32;
    let mut prev_above = origin.y - terrain.height_at(origin.x, origin.z) > 0.0;
    let step = 1.5;
    while t < 2000.0 {
        t += step;
        let p = origin + dir * t;
        let above = p.y - terrain.height_at(p.x, p.z) > 0.0;
        if above != prev_above {
            // Bisect between t-step and t.
            let (mut lo, mut hi) = (t - step, t);
            for _ in 0..12 {
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

/// Steer each ordered tank toward its destination; clear the order on arrival.
fn drive_to_order(mut tanks: Query<(&Transform, &mut Vehicle, &mut Commandable)>) {
    for (transform, mut vehicle, mut cmd) in &mut tanks {
        let Some(target) = cmd.order else {
            // No order: coast to a stop.
            vehicle.throttle = 0.0;
            vehicle.steer = 0.0;
            continue;
        };
        let pos = transform.translation;
        let to = target - Vec2::new(pos.x, pos.z);
        let dist = to.length();
        if dist < ARRIVE_RADIUS {
            vehicle.throttle = 0.0;
            vehicle.steer = 0.0;
            cmd.order = None;
            continue;
        }

        // Heading is (sin yaw, cos yaw), so the desired yaw is atan2(x, z).
        let desired_yaw = to.x.atan2(to.y);
        let mut err = desired_yaw - vehicle.yaw;
        err = (err + PI).rem_euclid(TAU) - PI;

        vehicle.steer = (err / FRAC_PI_4).clamp(-1.0, 1.0);
        // Ease off the throttle while turning hard or arriving.
        let turn_penalty = if err.abs() > FRAC_PI_2 { 0.25 } else { 1.0 };
        let arrive = (dist / 6.0).clamp(0.35, 1.0);
        vehicle.throttle = turn_penalty * arrive;
    }
}

/// Draw selection rings and order markers with immediate-mode gizmos.
fn draw_markers(
    mut gizmos: Gizmos,
    terrain: Option<Res<Terrain>>,
    tanks: Query<(&Transform, &Commandable)>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    for (transform, cmd) in &tanks {
        if cmd.selected {
            ring(&mut gizmos, transform.translation, 3.4, Color::srgb(0.2, 1.0, 0.4));
        }
        if let Some(target) = cmd.order {
            let y = terrain.height_at(target.x, target.y) + 0.2;
            let center = Vec3::new(target.x, y, target.y);
            ring(&mut gizmos, center, 1.2, Color::srgb(1.0, 0.85, 0.2));
        }
    }
}

/// A ground-hugging circle drawn from line segments (stable across gizmo API).
fn ring(gizmos: &mut Gizmos, center: Vec3, radius: f32, color: Color) {
    const SEGMENTS: usize = 24;
    let mut prev = center + Vec3::new(radius, 0.0, 0.0);
    for i in 1..=SEGMENTS {
        let a = i as f32 / SEGMENTS as f32 * TAU;
        let next = center + Vec3::new(radius * a.cos(), 0.0, radius * a.sin());
        gizmos.line(prev, next, color);
        prev = next;
    }
}
