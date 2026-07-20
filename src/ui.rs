//! Loading screen (shows the build version) and the in-game HUD.

use crate::version::VERSION;
use crate::GameState;
use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LoadingTimer(Timer::from_seconds(1.4, TimerMode::Once)))
            .init_resource::<MobileControlsShown>()
            .add_systems(OnEnter(GameState::Loading), spawn_loading)
            .add_systems(OnExit(GameState::Loading), despawn::<LoadingRoot>)
            .add_systems(OnEnter(GameState::Playing), (spawn_hud, dismiss_html_overlay))
            .add_systems(Update, tick_loading.run_if(in_state(GameState::Loading)))
            .add_systems(
                Update,
                reveal_mobile_controls.run_if(in_state(GameState::Playing)),
            );
    }
}

#[derive(Resource)]
struct LoadingTimer(Timer);

/// Whether the on-screen touch controls have been shown yet (first touch).
#[derive(Resource, Default)]
struct MobileControlsShown(bool);

#[derive(Component)]
struct MobileControls;

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
                "BATTLE — panzers manoeuvre & flank • infantry advance • aircraft bomb",
                "Drive: W A S D / arrows • Aim: mouse (turret traverses to it)",
                "Main gun: click / E to designate a target — the turret then",
                "   traverses, lays on it, and fires when ready",
                "Machine gun (hull, forward, short range): Q / right-click",
                "Touch: stick drives • double-tap = target • pinch = zoom • MG btn",
                "Camera: middle-drag orbit • R F pitch • wheel zoom",
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

/// Show the on-screen touch controls the first time the player touches the
/// screen (so they never clutter a desktop session).
fn reveal_mobile_controls(
    mut commands: Commands,
    mut shown: ResMut<MobileControlsShown>,
    touches: Res<Touches>,
) {
    if shown.0 || touches.iter().next().is_none() {
        return;
    }
    shown.0 = true;
    spawn_mobile_controls(&mut commands);
}

fn spawn_mobile_controls(commands: &mut Commands) {
    use crate::input::{BTN_MARGIN, BTN_R};
    let diameter = BTN_R * 2.0;
    let edge = BTN_MARGIN - BTN_R;

    // The MG button. (The main gun is fired by double-tapping the battlefield,
    // so there is no separate FIRE button.) Scoped so its mutable borrow of
    // `commands` is released before the stick is spawned below.
    {
        let mut button = |right: f32, bottom: f32, color: Color, label: &str| {
            commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(right),
                    bottom: Val::Px(bottom),
                    width: Val::Px(diameter),
                    height: Val::Px(diameter),
                    border: UiRect::all(Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(color),
                BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                BorderRadius::all(Val::Percent(50.0)),
                MobileControls,
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(label),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.95, 0.9)),
                ));
            });
    };

        button(edge, edge, Color::srgba(0.75, 0.7, 0.25, 0.30), "MG");
    }

    // Left thumb-stick base (the stick itself is dynamic; this is a guide).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                bottom: Val::Px(20.0),
                width: Val::Px(150.0),
                height: Val::Px(150.0),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
            BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.35)),
            BorderRadius::all(Val::Percent(50.0)),
            MobileControls,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("MOVE"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgba(0.9, 0.9, 0.85, 0.7)),
            ));
        });
}

fn despawn<C: Component>(mut commands: Commands, query: Query<Entity, With<C>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}
