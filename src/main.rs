//! Tanks-v2 — a mobile & web friendly 3D isometric WWII tank-squad commander,
//! built on the Bevy engine and deployable to Vercel as static WebAssembly.
//!
//! The crate is split into focused plugins:
//! - [`camera`]   isometric orbit / zoom / pan camera (mouse + touch)
//! - [`terrain`]  procedural low-poly heightmap battlefield
//! - [`tank`]     procedural low-poly tanks with animated treads
//! - [`physics`]  custom suspension + terrain following
//! - [`squad`]    selection and move-order command & control
//! - [`ui`]       loading screen (with version) and in-game HUD

mod camera;
mod combat;
mod control;
mod effects;
mod input;
mod physics;
// Retained for the full squad mode; the current training mission uses direct
// keyboard control instead (see `control`).
#[allow(dead_code)]
mod squad;
mod tank;
mod terrain;
mod ui;
mod version;
mod weapons;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

/// Top-level game flow. We open on a branded loading screen (which shows the
/// version) and then drop into gameplay.
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    Playing,
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: format!("Tanks-v2 {}", version::build_label()),
                        // Bind to the canvas in index.html and let it track the
                        // element size so the game is responsive on mobile.
                        canvas: Some("#game-canvas".into()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        present_mode: PresentMode::AutoVsync,
                        resolution: WindowResolution::new(1280.0, 720.0),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(ClearColor(Color::srgb(0.52, 0.64, 0.78)))
        // Lower, cooler ambient (was a very bright 380 that washed everything
        // flat) so the sun below actually shapes the geometry.
        .insert_resource(AmbientLight {
            color: Color::srgb(0.55, 0.62, 0.80),
            brightness: 150.0,
        })
        .init_state::<GameState>()
        .add_plugins((
            input::InputPlugin,
            camera::CameraPlugin,
            terrain::TerrainPlugin,
            tank::TankPlugin,
            physics::PhysicsPlugin,
            control::ControlPlugin,
            weapons::WeaponsPlugin,
            combat::CombatPlugin,
            effects::EffectsPlugin,
            ui::UiPlugin,
        ))
        .add_systems(Startup, setup_lighting)
        .run();
}

/// A strong, warm low-angle sun over cool ambient fill — the contrast between
/// them shades the flat-shaded low-poly geometry so it reads with depth (a
/// golden-hour look) instead of the old washed-out flatness.
fn setup_lighting(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.84),
            illuminance: 16_000.0,
            shadows_enabled: true,
            ..default()
        },
        // Lower on the horizon than before, for longer, more legible shadows.
        Transform::from_xyz(95.0, 80.0, 55.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
