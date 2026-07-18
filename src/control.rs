//! Direct keyboard driving for the training mission.
//!
//! The training tank is driven arcade-style: throttle forward/back and steer
//! left/right, written straight onto its [`Vehicle`]. This is separate from the
//! RTS-style squad orders in [`crate::squad`]; a [`PlayerControlled`] tank has no
//! `Commandable`, so the order system leaves it alone.

use crate::physics::Vehicle;
use bevy::prelude::*;

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive_with_keyboard);
    }
}

/// The tank the player drives directly with the keyboard.
#[derive(Component)]
pub struct PlayerControlled;

fn drive_with_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut tanks: Query<&mut Vehicle, With<PlayerControlled>>,
) {
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let forward = held(KeyCode::KeyW, KeyCode::ArrowUp) as i32
        - held(KeyCode::KeyS, KeyCode::ArrowDown) as i32;
    let turn = held(KeyCode::KeyD, KeyCode::ArrowRight) as i32
        - held(KeyCode::KeyA, KeyCode::ArrowLeft) as i32;

    for mut vehicle in &mut tanks {
        vehicle.throttle = forward as f32;
        vehicle.steer = turn as f32;
    }
}
