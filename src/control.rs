//! Direct driving for the training tank.
//!
//! Drive intent comes pre-mixed from [`crate::input::GameInput`] (keyboard or
//! on-screen stick) and is written straight onto the tank's [`Vehicle`]. This is
//! separate from the RTS-style squad orders in [`crate::squad`]; a
//! [`PlayerControlled`] tank has no `Commandable`, so the order system leaves it
//! alone.

use crate::combat::Armor;
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

fn drive_tank(
    input: Res<GameInput>,
    mut tanks: Query<(&mut Vehicle, Option<&Armor>), With<PlayerControlled>>,
) {
    for (mut vehicle, armor) in &mut tanks {
        // Damage saps mobility: a battered tank crawls (and a wreck can't move).
        let cond = armor.map(|a| a.condition()).unwrap_or(1.0);
        let mobility = 0.2 + 0.8 * cond;
        vehicle.throttle = input.drive.y.clamp(-1.0, 1.0) * mobility;
        // Negated so pushing right steers to the player's right.
        vehicle.steer = -input.drive.x.clamp(-1.0, 1.0) * (0.5 + 0.5 * cond);
    }
}
