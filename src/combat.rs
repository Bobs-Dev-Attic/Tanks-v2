//! Tank health, battle damage, and destruction.
//!
//! Every tank carries [`Armor`]. Shell hits (see `weapons::update_projectiles`)
//! subtract HP and flag a hit. This module turns that HP into consequences:
//! the hull visibly chars and glows when struck, a tank's *abilities* fade with
//! its condition (a damaged crew traverses, reloads, and drives slower), and a
//! killed tank is knocked out — its turret slews askew and it burns, smoking and
//! smoldering as persistent wreckage.

use crate::effects::{spawn_explosion, spawn_smoke_column, spawn_wreck_fire, EffectAssets, Wreckage};
use crate::physics::Vehicle;
use crate::weapons::Weapons;
use bevy::prelude::*;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (apply_damage, burn_wrecks).chain());
    }
}

/// Health and damage state for a tank.
#[derive(Component)]
pub struct Armor {
    pub hp: f32,
    pub max: f32,
    pub destroyed: bool,
    /// Brief glow (0..1) right after a hit; decays each frame.
    pub hit_glow: f32,
    /// Set true the frame HP crosses zero, so destruction runs exactly once.
    just_killed: bool,
    /// Paces the ongoing smoke/fire while burning.
    smoke: Timer,
}

impl Armor {
    pub fn new(max: f32) -> Self {
        Self {
            hp: max,
            max,
            destroyed: false,
            hit_glow: 0.0,
            just_killed: false,
            smoke: Timer::from_seconds(0.32, TimerMode::Repeating),
        }
    }

    /// Condition in 0..1 — how intact the tank is.
    pub fn condition(&self) -> f32 {
        (self.hp / self.max).clamp(0.0, 1.0)
    }

    /// Apply damage; returns true if this blow destroyed the tank.
    pub fn damage(&mut self, amount: f32) -> bool {
        if self.destroyed {
            return false;
        }
        self.hp -= amount;
        self.hit_glow = 1.0;
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.destroyed = true;
            self.just_killed = true;
            return true;
        }
        false
    }
}

/// References the tank's damageable visuals so the damage system can char the
/// hull and knock the turret off when the tank brews up.
#[derive(Component)]
pub struct TankRig {
    /// The shared hull/turret armor material (charred as damage mounts).
    pub hull_mat: Handle<StandardMaterial>,
    /// Undamaged armor colour, to interpolate away from.
    pub base_color: Color,
    /// The turret pivot entity, knocked askew on destruction.
    pub turret: Option<Entity>,
    /// Approximate top of the hull (local Y), where smoke and fire vent.
    pub hull_top: f32,
}

/// Update armor visuals, scale abilities by condition, and handle destruction.
#[allow(clippy::too_many_arguments)]
fn apply_damage(
    time: Res<Time>,
    mut commands: Commands,
    fx: Option<Res<EffectAssets>>,
    mut wreckage: ResMut<Wreckage>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tanks: Query<(
        &GlobalTransform,
        &mut Armor,
        &TankRig,
        Option<&mut Weapons>,
        Option<&mut Vehicle>,
    )>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    for (gt, mut armor, rig, weapons, vehicle) in &mut tanks {
        armor.hit_glow = (armor.hit_glow - dt * 2.2).max(0.0);
        let cond = armor.condition();

        // Abilities fade with condition: a battered crew lays and reloads slower.
        if let Some(mut w) = weapons {
            w.condition = if armor.destroyed { 0.0 } else { 0.25 + 0.75 * cond };
        }
        // A wrecked tank is dead in the water.
        if armor.destroyed {
            if let Some(mut v) = vehicle {
                v.throttle = 0.0;
                v.forward_speed *= 1.0 - (dt * 3.0).min(1.0);
            }
        }

        // Char the armor as HP drops; flash bright when freshly hit; smolder red
        // once destroyed.
        if let Some(mat) = materials.get_mut(&rig.hull_mat) {
            let base = rig.base_color.to_srgba();
            let char = 0.35 + 0.65 * cond; // darker as it burns
            mat.base_color = Color::srgb(base.red * char, base.green * char, base.blue * char);
            let glow = armor.hit_glow;
            let ember = if armor.destroyed {
                0.5 + 0.4 * (time.elapsed_secs() * 9.0).sin().abs()
            } else {
                0.0
            };
            mat.emissive = LinearRgba::rgb(
                glow * 2.5 + ember * 1.6,
                glow * 0.9 + ember * 0.4,
                glow * 0.2 + ember * 0.08,
            );
        }

        // Destruction, once: knock the turret askew, blow up, and start burning.
        if armor.just_killed {
            armor.just_killed = false;
            let pos = gt.translation();
            if let (Some(fx), Some(turret)) = (fx.as_ref(), rig.turret) {
                if let Ok(mut tf) = transforms.get_mut(turret) {
                    // Slew and lift the turret off its ring.
                    tf.rotation = Quat::from_rotation_y(0.6) * Quat::from_rotation_z(0.22);
                    tf.translation += Vec3::new(0.3, 0.25, -0.2);
                }
                let top = pos + Vec3::Y * rig.hull_top;
                spawn_explosion(&mut commands, fx, &mut materials, &mut wreckage, top, pos.x as u32 ^ 0x51F);
                // A long-lived smoke column and a burning ember light.
                spawn_smoke_column(&mut commands, top + Vec3::Y * 0.4, 45.0, 1.6);
            }
        }
    }
}

/// Damaged and destroyed tanks keep smoking and flickering with fire.
#[allow(clippy::too_many_arguments)]
fn burn_wrecks(
    time: Res<Time>,
    mut commands: Commands,
    fx: Option<Res<EffectAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tanks: Query<(&GlobalTransform, &mut Armor, &TankRig)>,
) {
    let Some(fx) = fx else {
        return;
    };
    for (gt, mut armor, rig) in &mut tanks {
        // Smoke once heavily damaged; a destroyed tank also spits fire.
        let hurt = armor.destroyed || armor.condition() < 0.5;
        if !hurt {
            continue;
        }
        if !armor.smoke.tick(time.delta()).just_finished() {
            continue;
        }
        let top = gt.translation() + Vec3::Y * rig.hull_top;
        let seed = (time.elapsed_secs() * 331.0) as u32 ^ gt.translation().x.to_bits();
        spawn_wreck_fire(
            &mut commands,
            &fx,
            &mut materials,
            top,
            armor.destroyed,
            seed,
        );
    }
}
