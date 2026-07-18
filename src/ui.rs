//! Loading screen (shows the build version) and the in-game HUD.

use crate::version::VERSION;
use crate::GameState;
use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LoadingTimer(Timer::from_seconds(1.4, TimerMode::Once)))
            .add_systems(OnEnter(GameState::Loading), spawn_loading)
            .add_systems(OnExit(GameState::Loading), despawn::<LoadingRoot>)
            .add_systems(OnEnter(GameState::Playing), (spawn_hud, dismiss_html_overlay))
            .add_systems(
                Update,
                tick_loading.run_if(in_state(GameState::Loading)),
            );
    }
}

#[derive(Resource)]
struct LoadingTimer(Timer);

#[derive(Component)]
struct LoadingRoot;

#[derive(Component)]
struct HudRoot;

fn spawn_loading(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.07, 0.09)),
            LoadingRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("TANKS-V2"),
                TextFont {
                    font_size: 66.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.88, 0.72)),
            ));
            p.spawn((
                Text::new("WWII Isometric Squad Commander"),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.66, 0.7)),
            ));
            p.spawn((
                Text::new(format!("v{VERSION}")),
                TextFont {
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.85, 0.55)),
            ));
            p.spawn((
                Text::new("Deploying armored column…"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.52, 0.56)),
            ));
        });
}

fn tick_loading(
    time: Res<Time>,
    mut timer: ResMut<LoadingTimer>,
    mut next: ResMut<NextState<GameState>>,
) {
    if timer.0.tick(time.delta()).finished() {
        next.set(GameState::Playing);
    }
}

fn spawn_hud(mut commands: Commands) {
    // Controls help, top-left.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            HudRoot,
        ))
        .with_children(|p| {
            for line in [
                "TRAINING — DRIVE THE TANK",
                "Accelerate / reverse: W S  or  Up / Down",
                "Steer: A D  or  Left / Right",
                "Orbit camera: middle-drag / Q E / one-finger",
                "Pitch: R F   Zoom: wheel / Z X / pinch",
                "Camera follows your tank.",
            ] {
                p.spawn((
                    Text::new(line),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.88, 0.8)),
                ));
            }
        });

    // Version, bottom-right.
    commands.spawn((
        Text::new(format!("Tanks-v2 v{VERSION}")),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgba(0.8, 0.85, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            right: Val::Px(10.0),
            ..default()
        },
        HudRoot,
    ));
}

/// Hide the HTML loading overlay in `index.html` now that the engine is running.
/// On the web, winit uses an exception for control flow, so the JS `init()`
/// promise never resolves — the engine has to dismiss the overlay itself.
fn dismiss_html_overlay() {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(element) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("boot"))
        {
            // The `.hidden` class fades it out and disables pointer events.
            element.set_class_name("hidden");
        }
    }
}

fn despawn<C: Component>(mut commands: Commands, query: Query<Entity, With<C>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}
