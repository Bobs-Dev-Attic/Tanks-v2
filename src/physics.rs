//! Lightweight but plausible vehicle physics.
//!
//! Each tank is a [`Vehicle`]: it has gravity, momentum, and traction, drives on
//! a heading that steering rotates, and hugs the ground via a four-point
//! suspension sample so the hull pitches and rolls to match the terrain. This is
//! hand-written (no physics engine) which keeps the wasm build small and the
//! behaviour deterministic.

use crate::terrain::Terrain;
use bevy::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, integrate_vehicles);
    }
}

/// Drivable ground vehicle. Command systems write [`throttle`](Self::throttle)
/// and [`steer`](Self::steer); this module integrates the rest.
#[derive(Component)]
pub struct Vehicle {
    /// Heading in radians. Movement is along `(sin, cos)` of this angle.
    pub yaw: f32,
    /// Desired drive, -1 (reverse) .. 1 (full forward).
    pub throttle: f32,
    /// Desired turn, -1 (left) .. 1 (right).
    pub steer: f32,
    /// Current signed speed along the heading (m/s). Used by tread animation.
    pub forward_speed: f32,
    /// Vertical velocity, for the gravity/suspension spring.
    pub vertical_velocity: f32,
    /// Whether the tracks are on the ground this frame.
    pub grounded: bool,
    /// Top speed in world units per second.
    pub max_speed: f32,
    /// Turn rate in radians per second at speed.
    pub turn_rate: f32,
    /// Half the track length / width used for the suspension footprint.
    pub half_length: f32,
    pub half_width: f32,
}

impl Default for Vehicle {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            throttle: 0.0,
            steer: 0.0,
            forward_speed: 0.0,
            vertical_velocity: 0.0,
            grounded: false,
            max_speed: 16.0,
            turn_rate: 1.5,
            half_length: 2.6,
            half_width: 1.6,
        }
    }
}

const GRAVITY: f32 = 24.0;
const RIDE_HEIGHT: f32 = 0.55;

fn integrate_vehicles(
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut vehicles: Query<(&mut Vehicle, &mut Transform)>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs().min(1.0 / 20.0);
    if dt <= 0.0 {
        return;
    }

    for (mut v, mut transform) in &mut vehicles {
        let pos = transform.translation;

        // --- Steering & drive (traction only while grounded) ---
        let traction = if v.grounded { 1.0 } else { 0.15 };
        let target_speed = v.throttle * v.max_speed;
        // Accelerate toward target speed; heavier easing when braking.
        let accel = if target_speed.abs() > v.forward_speed.abs() {
            5.0
        } else {
            8.0
        };
        v.forward_speed += (target_speed - v.forward_speed) * (accel * dt * traction).min(1.0);

        // Turn rate falls off when nearly stationary, like a real tracked
        // vehicle needing some track motion to pivot.
        let speed_factor = (v.forward_speed.abs() / v.max_speed).clamp(0.25, 1.0);
        v.yaw += v.steer * v.turn_rate * speed_factor * dt * traction;

        let heading = Vec3::new(v.yaw.sin(), 0.0, v.yaw.cos());
        let mut new_pos = pos + heading * v.forward_speed * dt;

        // --- Gravity + ground spring ---
        let ground = terrain.height_at(new_pos.x, new_pos.z);
        let desired_y = ground + RIDE_HEIGHT;
        v.vertical_velocity -= GRAVITY * dt;
        new_pos.y += v.vertical_velocity * dt;
        if new_pos.y <= desired_y {
            new_pos.y = desired_y;
            v.vertical_velocity = 0.0;
            v.grounded = true;
        } else {
            v.grounded = false;
        }
        transform.translation = new_pos;

        // --- Four-point suspension: derive hull orientation from the terrain
        //     under each track corner, then face the heading. ---
        let up = suspension_normal(&terrain, new_pos, v.yaw, v.half_length, v.half_width);
        let target_rot = Transform::from_translation(new_pos)
            .looking_to(heading, up)
            .rotation;
        let blend = (dt * 8.0).min(1.0);
        transform.rotation = transform.rotation.slerp(target_rot, blend);
    }
}

/// Average the terrain heights under the four track corners into an up-vector,
/// so the hull sits flush with slopes and bumps.
fn suspension_normal(
    terrain: &Terrain,
    pos: Vec3,
    yaw: f32,
    half_len: f32,
    half_wid: f32,
) -> Vec3 {
    let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());

    let sample = |o: Vec3| -> Vec3 {
        let p = pos + o;
        Vec3::new(p.x, terrain.height_at(p.x, p.z), p.z)
    };
    let fl = sample(forward * half_len - right * half_wid);
    let fr = sample(forward * half_len + right * half_wid);
    let bl = sample(-forward * half_len - right * half_wid);
    let br = sample(-forward * half_len + right * half_wid);

    // Two diagonals give a robust averaged normal.
    let n1 = (fr - bl).cross(fl - br);
    let mut n = n1.normalize_or_zero();
    if n.y < 0.0 {
        n = -n;
    }
    if n == Vec3::ZERO {
        Vec3::Y
    } else {
        // Bias toward straight up a little so tanks don't lie totally flat on
        // steep faces — reads better and keeps them drivable.
        (n + Vec3::Y * 0.6).normalize()
    }
}
