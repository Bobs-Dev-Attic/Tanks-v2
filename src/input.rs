//! Unified pointer input for desktop (mouse + keyboard) and mobile (touch).
//!
//! Everything downstream reads the [`GameInput`] resource, so the camera and
//! command systems don't care whether an orbit came from a middle-mouse drag,
//! the `Q`/`E` keys, or a one-finger swipe.
//!
//! Control scheme
//! - Desktop: middle-drag or `Q`/`E` orbit, `R`/`F` pitch, wheel or `Z`/`X`
//!   zoom, `WASD`/arrows pan, left-click select (drag = box select), right-click
//!   issue move order.
//! - Mobile: one-finger drag orbits, two-finger pinch zooms and drags pans, a
//!   quick tap selects a tank or (with a selection) orders it to the ground.

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Movement (in logical px) beyond which a press is treated as a drag, not a tap.
const DRAG_THRESHOLD: f32 = 8.0;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameInput>()
            .init_resource::<PointerState>()
            .add_systems(PreUpdate, gather_input);
    }
}

/// High-level, device-agnostic input for one frame.
#[derive(Resource, Default)]
pub struct GameInput {
    /// Primary pointer position in logical window pixels, if any.
    pub pointer: Option<Vec2>,
    /// Yaw (x) / pitch (y) orbit request this frame.
    pub orbit: Vec2,
    /// Screen-space pan request this frame.
    pub pan: Vec2,
    /// Zoom request this frame; positive means zoom in.
    pub zoom: f32,
    /// A select action (left click / tap) at this screen position.
    pub primary_action: Option<Vec2>,
    /// An explicit move order (right click) at this screen position.
    pub secondary_action: Option<Vec2>,
    /// Active box-select rectangle (min, max) while dragging.
    pub box_drag: Option<(Vec2, Vec2)>,
    /// A box-select rectangle that was just completed this frame.
    pub box_finished: Option<(Vec2, Vec2)>,
}

#[derive(Resource, Default)]
struct PointerState {
    left_down_pos: Option<Vec2>,
    left_dragging: bool,
    touch_multi: bool,
    last_pinch: Option<f32>,
}

fn gather_input(
    mut game: ResMut<GameInput>,
    mut state: ResMut<PointerState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    touches: Res<Touches>,
    time: Res<Time>,
) {
    // Reset the per-frame view of input.
    *game = GameInput::default();

    let Ok(window) = windows.get_single() else {
        return;
    };
    let cursor = window.cursor_position();
    game.pointer = cursor;

    let dt = time.delta_secs().max(1.0 / 240.0);

    // --- Mouse orbit (middle button drag) ---
    let mut mouse_delta = Vec2::ZERO;
    for ev in motion.read() {
        mouse_delta += ev.delta;
    }
    if mouse_buttons.pressed(MouseButton::Middle) {
        game.orbit += Vec2::new(mouse_delta.x, mouse_delta.y) * 0.35;
    }

    // --- Keyboard orbit / pitch / zoom / pan ---
    let key_axis = |neg: KeyCode, pos: KeyCode| -> f32 {
        (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32
    };
    game.orbit.y += key_axis(KeyCode::KeyF, KeyCode::KeyR) * 60.0 * dt;
    game.zoom += key_axis(KeyCode::KeyX, KeyCode::KeyZ) * 30.0 * dt;
    // Note: WASD / arrows drive the tank; Q / E fire weapons (see `control` and
    // `weapons`). Camera orbit is middle-drag or one-finger touch.

    // --- Mouse wheel zoom ---
    for ev in wheel.read() {
        game.zoom += ev.y * 2.2;
    }

    // --- Mouse select / box-select / order ---
    if mouse_buttons.just_pressed(MouseButton::Left) {
        state.left_down_pos = cursor;
        state.left_dragging = false;
    }
    if mouse_buttons.pressed(MouseButton::Left) {
        if let (Some(start), Some(now)) = (state.left_down_pos, cursor) {
            if start.distance(now) > DRAG_THRESHOLD {
                state.left_dragging = true;
            }
            if state.left_dragging {
                let min = start.min(now);
                let max = start.max(now);
                game.box_drag = Some((min, max));
            }
        }
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some(start) = state.left_down_pos {
            let end = cursor.unwrap_or(start);
            if state.left_dragging {
                game.box_finished = Some((start.min(end), start.max(end)));
            } else {
                game.primary_action = Some(end);
            }
        }
        state.left_down_pos = None;
        state.left_dragging = false;
    }
    if mouse_buttons.just_released(MouseButton::Right) {
        if let Some(pos) = cursor {
            game.secondary_action = Some(pos);
        }
    }

    // --- Touch gestures ---
    let active: Vec<_> = touches.iter().collect();
    match active.len() {
        0 => {
            state.touch_multi = false;
            state.last_pinch = None;
        }
        1 => {
            let t = active[0];
            // One-finger drag orbits (unless part of a lifted multi-touch).
            game.orbit += t.delta() * 0.4;
            game.pointer = Some(t.position());
        }
        _ => {
            state.touch_multi = true;
            let a = active[0];
            let b = active[1];
            // Pinch to zoom.
            let dist = a.position().distance(b.position());
            if let Some(prev) = state.last_pinch {
                game.zoom += (dist - prev) * 0.05;
            }
            state.last_pinch = Some(dist);
            // Two-finger drag to pan.
            game.pan += -(a.delta() + b.delta()) * 0.5;
        }
    }

    // A quick single-finger tap becomes a primary (select/order) action.
    for t in touches.iter_just_released() {
        if !state.touch_multi && t.position().distance(t.start_position()) <= DRAG_THRESHOLD {
            game.primary_action = Some(t.position());
        }
    }
}
