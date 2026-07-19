//! Unified input for desktop (mouse + keyboard) and mobile (touch).
//!
//! Everything downstream reads the [`GameInput`] resource, so `control` and
//! `weapons` don't care whether "drive forward" came from the `W` key or the
//! on-screen left stick, or whether "fire" came from a mouse click or a touch
//! button.
//!
//! Control scheme
//! - Desktop: WASD / arrows drive, mouse aims the turret, `E`/left-click fire the
//!   main gun, `Q`/right-click fire the MG. Camera: middle-drag orbit, `R`/`F`
//!   pitch, wheel or `Z`/`X` zoom.
//! - Mobile: a left thumb-stick drives, touching the right side aims the turret,
//!   and the on-screen FIRE / MG buttons fire.

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Movement (in logical px) beyond which a mouse press is a drag, not a click.
const DRAG_THRESHOLD: f32 = 8.0;

/// On-screen button radius (logical px).
pub const BTN_R: f32 = 60.0;
/// Distance of a button's centre from the bottom-right corner.
pub const BTN_MARGIN: f32 = 80.0;
/// Extent (logical px) of the drive-stick's active zone in the bottom-left
/// corner — sized to the on-screen stick base (150px at a 20px margin) plus a
/// little slack, so touches anywhere else are free for targeting.
pub const STICK_ZONE: f32 = 200.0;

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
    /// Camera orbit request this frame (yaw x, pitch y).
    pub orbit: Vec2,
    /// Camera pan request this frame.
    pub pan: Vec2,
    /// Camera zoom request this frame; positive means zoom in.
    pub zoom: f32,
    /// Drive intent: x = steer (+ right), y = throttle (+ forward).
    pub drive: Vec2,
    /// Screen position the turret/gun should aim at, if any.
    pub aim: Option<Vec2>,
    /// Main gun fired this frame (edge).
    pub fire_main: bool,
    /// Machine gun firing this frame (held).
    pub fire_mg: bool,
    // --- retained for the (currently unwired) squad mode ---
    pub primary_action: Option<Vec2>,
    pub secondary_action: Option<Vec2>,
    pub box_drag: Option<(Vec2, Vec2)>,
    pub box_finished: Option<(Vec2, Vec2)>,
}

#[derive(Resource, Default)]
struct PointerState {
    left_down_pos: Option<Vec2>,
    left_dragging: bool,
    /// Last two-finger distance, for pinch-to-zoom.
    last_pinch: Option<f32>,
    pinch_accum: f32,
    /// Last single tap (position and time), for double-tap detection.
    last_tap_pos: Option<Vec2>,
    last_tap_time: f32,
}

/// Centre of the MG button for the given window size.
pub fn mg_button_center(w: f32, h: f32) -> Vec2 {
    Vec2::new(w - BTN_MARGIN, h - BTN_MARGIN)
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
    *game = GameInput::default();

    let Ok(window) = windows.get_single() else {
        return;
    };
    let cursor = window.cursor_position();
    game.pointer = cursor;
    let (w, h) = (window.width(), window.height());
    let dt = time.delta_secs().max(1.0 / 240.0);

    // --- Camera: mouse orbit (middle drag), keyboard pitch, wheel/keys zoom ---
    let mut mouse_delta = Vec2::ZERO;
    for ev in motion.read() {
        mouse_delta += ev.delta;
    }
    if mouse_buttons.pressed(MouseButton::Middle) {
        game.orbit += mouse_delta * 0.35;
    }
    let key_axis = |neg: KeyCode, pos: KeyCode| -> f32 {
        (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32
    };
    game.orbit.y += key_axis(KeyCode::KeyF, KeyCode::KeyR) * 60.0 * dt;
    // Zoom is discrete: one notch (or Z/X press) is one level step.
    game.zoom +=
        (keys.just_pressed(KeyCode::KeyZ) as i32 - keys.just_pressed(KeyCode::KeyX) as i32) as f32;
    for ev in wheel.read() {
        game.zoom += ev.y.signum();
    }

    // --- Drive: keyboard + on-screen left stick ---
    let mut drive = Vec2::new(
        key_axis(KeyCode::KeyA, KeyCode::KeyD) + key_axis(KeyCode::ArrowLeft, KeyCode::ArrowRight),
        key_axis(KeyCode::KeyS, KeyCode::KeyW) + key_axis(KeyCode::ArrowDown, KeyCode::ArrowUp),
    );
    let mg_center = mg_button_center(w, h);
    let on_button = |p: Vec2| p.distance(mg_center) < BTN_R;
    // The drive stick only claims touches that begin on its on-screen base in the
    // bottom-left corner (see `ui::spawn_mobile_controls`). Keeping this zone
    // tight leaves the rest of the screen — including most of the lower-left —
    // free for designating targets.
    let in_stick = |p: Vec2| p.x < STICK_ZONE && p.y > h - STICK_ZONE;

    for t in touches.iter() {
        let start = t.start_position();
        // Left thumb-stick: a touch that began on the stick base.
        if in_stick(start) {
            let d = t.position() - start;
            let radius = 70.0;
            drive.x += (d.x / radius).clamp(-1.0, 1.0);
            drive.y += (-d.y / radius).clamp(-1.0, 1.0);
            break;
        }
    }
    game.drive = drive.clamp(Vec2::splat(-1.0), Vec2::splat(1.0));

    // "Free" touches are those not on the left stick and not on a button.
    let is_free = |start: Vec2| !in_stick(start) && !on_button(start);
    let free: Vec<_> = touches.iter().filter(|t| is_free(t.start_position())).collect();

    // --- Pinch to zoom (two free fingers) or single-finger aim ---
    let mut aim_touch = None;
    if free.len() >= 2 {
        let dist = free[0].position().distance(free[1].position());
        if let Some(prev) = state.last_pinch {
            state.pinch_accum += dist - prev;
        }
        state.last_pinch = Some(dist);
        // Each ~45px of spread is one zoom level (spread = zoom in).
        while state.pinch_accum > 45.0 {
            game.zoom += 1.0;
            state.pinch_accum -= 45.0;
        }
        while state.pinch_accum < -45.0 {
            game.zoom -= 1.0;
            state.pinch_accum += 45.0;
        }
    } else {
        state.last_pinch = None;
        state.pinch_accum = 0.0;
        if let Some(t) = free.first() {
            aim_touch = Some(t.position());
        }
    }
    game.aim = aim_touch.or(cursor);

    // --- Double-tap on the battlefield designates the main-gun target ---
    let mut touch_designate = None;
    for t in touches.iter_just_released() {
        let start = t.start_position();
        if !is_free(start) || t.position().distance(start) > DRAG_THRESHOLD {
            continue;
        }
        let now = time.elapsed_secs();
        if let Some(prev) = state.last_tap_pos {
            if now - state.last_tap_time < 0.4 && t.position().distance(prev) < 60.0 {
                touch_designate = Some(t.position());
                state.last_tap_pos = None;
                continue;
            }
        }
        state.last_tap_pos = Some(t.position());
        state.last_tap_time = now;
    }

    // --- Fire ---
    // There is no on-screen FIRE button: the main gun is designated (and thus
    // fired) by a double-tap on the battlefield, handled above.
    let mut touch_fire_main = false;
    let mut touch_fire_mg = false;
    for t in touches.iter() {
        if t.start_position().distance(mg_center) < BTN_R {
            touch_fire_mg = true;
        }
    }
    if let Some(pos) = touch_designate {
        game.aim = Some(pos);
        touch_fire_main = true;
    }
    game.fire_main = keys.just_pressed(KeyCode::KeyE)
        || mouse_buttons.just_pressed(MouseButton::Left)
        || touch_fire_main;
    game.fire_mg = keys.pressed(KeyCode::KeyQ)
        || mouse_buttons.pressed(MouseButton::Right)
        || touch_fire_mg;

    // --- Mouse box-select / order (retained for squad mode) ---
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
                game.box_drag = Some((start.min(now), start.max(now)));
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
}
