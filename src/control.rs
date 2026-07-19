//! Direct driving for the training tank.
//!
//! Drive intent comes pre-mixed from [`crate::input::GameInput`] (keyboard or
//! on-screen stick) and is written straight onto the tank's [`Vehicle`]. This is
//! separate from the RTS-style squad orders in [`crate::squad`]; a
//! [`PlayerControlled`] tank has no `Commandable`, so the order system leaves it
//! alone.

use crate::input::GameInput;
use crate::physics::Vehicle;
use bevy::prelude::*;

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive_tank);
    }
}

/// The tank the player drives directly.
#[derive(Component)]
pub struct PlayerControlled;

fn drive_tank(input: Res<GameInput>, mut tanks: Query<&mut Vehicle, With<PlayerControlled>>) {
    for mut vehicle in &mut tanks {
        vehicle.throttle = input.drive.y.clamp(-1.0, 1.0);
        // Negated so pushing right steers to the player's right.
        vehicle.steer = -input.drive.x.clamp(-1.0, 1.0);
    }
}
