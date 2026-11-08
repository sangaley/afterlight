use bevy::input::gamepad::{GamepadConnection, GamepadConnectionEvent};
use bevy::input::InputSystems;
use bevy::prelude::*;
use std::collections::HashSet;

use crate::events::{NotificationType, ShowNotification};
use crate::resources::InputState;
use crate::states::ShipSet;

// ============================================================================
// CONTROLLER SUPPORT
//
// Two layers:
//
// 1. A button→key bridge: every digital pad button is mapped to the KeyCode
//    that already drives the feature, and the bridge presses/releases those
//    keys on the shared ButtonInput<KeyCode> resource. Existing systems
//    (menus, docking, build mode, radar...) pick the input up without
//    knowing a controller exists.
//
// 2. Analog flight: the left stick writes throttle/strafe into InputState
//    directly (real analog, not WASD emulation), and the right stick sets
//    InputState.gamepad_aim, which ship facing and dumb-fire weapon aim
//    consume instead of the mouse cursor. Moving the mouse hands aim back
//    to the cursor.
//
// Build mode stays mouse-driven for now — the ghost cursor and grid picking
// all follow the OS cursor. Next step for full couch play would be a
// stick-driven virtual cursor.
// ============================================================================

/// Radial deadzone for the movement stick. Inside it the keyboard keeps
/// full control; outside, input rescales so the usable range starts at 0.
const MOVE_DEADZONE: f32 = 0.15;
// Aim-stick deadzone is player-tunable (Settings → Controls, default 35%),
// deliberately larger than movement so a slightly worn stick doesn't wrestle
// the nose away from the mouse. See `GameSettings::aim_deadzone`.
/// Cursor travel (logical px) that hands aim control back to the mouse.
const MOUSE_RECLAIM_PX: f32 = 4.0;

/// The controller layout: which pad button presses which key binding.
///
/// Left stick = throttle + strafe, right stick = aim. Everything digital
/// goes through this table, so retuning the layout is editing data, not
/// systems.
#[derive(Resource)]
pub struct ControllerLayout {
    pub buttons: Vec<(GamepadButton, KeyCode)>,
}

impl Default for ControllerLayout {
    fn default() -> Self {
        Self {
            buttons: vec![
                // Face buttons
                (GamepadButton::South, KeyCode::Enter),      // confirm / launch
                (GamepadButton::East, KeyCode::Escape),      // cancel / pause
                (GamepadButton::West, KeyCode::KeyF),        // interact / dock / salvage
                (GamepadButton::North, KeyCode::KeyZ),       // radar ping
                // Shoulders + triggers
                (GamepadButton::LeftTrigger, KeyCode::Tab),  // cycle target (and build category)
                (GamepadButton::RightTrigger, KeyCode::KeyB), // build mode while docked
                (GamepadButton::LeftTrigger2, KeyCode::ShiftLeft), // brake
                (GamepadButton::RightTrigger2, KeyCode::Space),    // fire
                // DPad doubles as menu navigation and digital flight
                (GamepadButton::DPadUp, KeyCode::ArrowUp),
                (GamepadButton::DPadDown, KeyCode::ArrowDown),
                (GamepadButton::DPadLeft, KeyCode::ArrowLeft),
                (GamepadButton::DPadRight, KeyCode::ArrowRight),
                // Center cluster
                (GamepadButton::Select, KeyCode::KeyM),      // map / inventory
                (GamepadButton::Start, KeyCode::Escape),     // pause
            ],
        }
    }
}

/// Presses/releases mapped KeyCodes to mirror pad button state.
///
/// Runs in PreUpdate after Bevy's input systems: the keyboard system has
/// already cleared last frame's just_pressed/just_released and applied real
/// key events, so emulated presses get correct just_pressed semantics.
/// `ButtonInput::press` is a no-op on an already-held key, so holding the
/// physical key and the pad button together doesn't double-fire; the one
/// rough edge is that releasing the pad button releases the key even if
/// the physical key is still down.
pub(crate) fn bridge_gamepad_buttons(
    gamepads: Query<&Gamepad>,
    layout: Res<ControllerLayout>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut emulated: Local<HashSet<KeyCode>>,
) {
    let mut desired: HashSet<KeyCode> = HashSet::new();
    for gamepad in gamepads.iter() {
        for &(button, key) in &layout.buttons {
            if gamepad.pressed(button) {
                desired.insert(key);
            }
        }
    }

    for &key in emulated.iter() {
        if !desired.contains(&key) {
            keyboard.release(key);
        }
    }
    // Press every desired key each frame (idempotent while held) rather
    // than only on transitions — if the window loses focus Bevy force-
    // releases all keys, and a transition-only bridge would leave a held
    // pad button stuck released afterwards.
    for &key in desired.iter() {
        keyboard.press(key);
    }

    *emulated = desired;
}

/// Left stick → analog throttle/strafe. Right stick → aim direction.
///
/// Runs after ship_input (which rebuilds InputState.movement from the
/// keyboard every frame) and only overrides it while the stick is actually
/// deflected, so keyboard and pad coexist. gamepad_aim persists after the
/// stick is released — the nose holds its heading instead of snapping back
/// to wherever the mouse cursor was parked — until the mouse moves again
/// and reclaims aim.
fn gamepad_flight(
    gamepads: Query<&Gamepad>,
    settings: Res<crate::ui::menu_buttons::GameSettings>,
    mut input_state: ResMut<InputState>,
    windows: Query<&Window>,
    mut last_cursor: Local<Option<Vec2>>,
) {
    // Player-tuned aim deadzone (Settings → Controls, default 35%).
    let aim_deadzone = settings.aim_deadzone.clamp(0.05, 0.9);
    for gamepad in gamepads.iter() {
        let stick = gamepad.left_stick();
        let len = stick.length();
        if len > MOVE_DEADZONE {
            let scaled = ((len - MOVE_DEADZONE) / (1.0 - MOVE_DEADZONE)).min(1.0);
            input_state.movement = stick / len * scaled;
        }

        let mut aim = gamepad.right_stick();
        if settings.invert_aim_y {
            aim.y = -aim.y;
        }
        if aim.length() > aim_deadzone {
            input_state.gamepad_aim = Some(aim.normalize());
        }
    }

    if let Ok(window) = windows.single() {
        if let Some(cursor) = window.cursor_position() {
            if let Some(prev) = *last_cursor {
                if (cursor - prev).length() > MOUSE_RECLAIM_PX {
                    input_state.gamepad_aim = None;
                }
            }
            *last_cursor = Some(cursor);
        }
    }
}

/// Announce controllers coming and going.
fn gamepad_connection_notifications(
    mut events: MessageReader<GamepadConnectionEvent>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for event in events.read() {
        match &event.connection {
            GamepadConnection::Connected { name, .. } => {
                notifications.write(ShowNotification {
                    message: format!("Controller connected: {}", name),
                    notification_type: NotificationType::Success,
                    duration: 3.0,
                });
            }
            GamepadConnection::Disconnected => {
                notifications.write(ShowNotification {
                    message: "Controller disconnected".into(),
                    notification_type: NotificationType::Warning,
                    duration: 3.0,
                });
            }
        }
    }
}

pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ControllerLayout>()
            .add_systems(PreUpdate, bridge_gamepad_buttons.after(InputSystems))
            .add_systems(
                Update,
                gamepad_flight
                    .in_set(ShipSet::Input)
                    .after(crate::ship::ship_input),
            )
            .add_systems(Update, gamepad_connection_notifications);
    }
}
