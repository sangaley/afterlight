//! Clickable menu buttons + full tabbed Settings screen.
//!
//! The main/pause/game-over menus were originally keyboard-only ("press ENTER").
//! This module adds a reusable *clickable* button (mouse hover/press feedback +
//! action dispatch) plus a full **Settings** overlay with three tabs — SOUND,
//! GRAPHICS, and CONTROLS — without removing any of the existing keyboard
//! shortcuts (they still work exactly as before).
//!
//! A button carries a `MenuAction`; one dispatch system reads button presses
//! and performs the action (change state, save/load, open settings, quit, tweak
//! audio buses, toggle display options, tune the gamepad). Settings live in the
//! persistent `GameSettings` resource, which is loaded from `meta/settings.json`
//! at startup and re-saved whenever it changes.
//!
//! Application of settings is split by domain so each stays cheap and only fires
//! on change: `apply_audio_settings` mirrors master/mute onto Bevy's
//! `GlobalVolume`, `apply_display_settings` pushes fullscreen/vsync/resolution/
//! ui-scale onto the window, and the per-bus volume scalars (sfx/music/ui) are
//! read directly by `audio.rs`. Gamepad tuning (invert-aim-Y, aim deadzone) is
//! read by `gamepad.rs`.

use bevy::prelude::*;
use bevy::app::AppExit;
use bevy::audio::{GlobalVolume, Volume};
use bevy::ui::UiScale;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};
use serde::{Deserialize, Serialize};

use crate::events::{LoadGameRequest, SaveGameRequest};
use crate::resources::PrePauseState;
use crate::states::GameState;
use super::theme::{self, ThemeColors, ThemeFonts, ThemeSpacing};

/// Where settings persist. Mirrors the `meta/unlocks.json` convention.
const SETTINGS_PATH: &str = "meta/settings.json";

/// Windowed resolution presets the RESOLUTION stepper cycles through.
pub const RESOLUTIONS: [(u32, u32); 5] = [
    (1280, 720),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
];

// ============================================================================
// SETTINGS DATA
// ============================================================================

/// The four mixer buses. Master scales everything (via `GlobalVolume`); the
/// other three are relative scalars read by `audio.rs` per sound.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AudioBus {
    Master,
    Sfx,
    Music,
    Ui,
}

/// Which settings tab is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsTab {
    #[default]
    Sound,
    Graphics,
    Controls,
}

/// Persistent player-facing settings. Source of truth for audio/display/control
/// options. Loaded from disk at startup, saved on every change.
#[derive(Resource, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    // --- Sound (all 0.0..=1.0) ---
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub ui_volume: f32,
    pub muted: bool,

    // --- Graphics ---
    pub fullscreen: bool,
    pub vsync: bool,
    /// Index into `RESOLUTIONS` (only applied while windowed).
    pub resolution_index: usize,
    /// Global UI zoom, 0.5..=1.5.
    pub ui_scale: f32,

    // --- Controls (gamepad) ---
    pub invert_aim_y: bool,
    /// Right-stick deadzone, 0.05..=0.60.
    pub aim_deadzone: f32,

    // --- Callout lines (ui::callouts): the notes pinned to the ship ---
    /// 0 Full (line and words), 1 Small (a stub and a number; the words go
    /// in the title block), 2 Off.
    pub callout_style: u8,
    /// Words on the lines (Full only).
    pub callout_words: bool,
    /// The heading arrow while thrusting.
    pub callout_heading: bool,
    /// Most notes on screen at once, 1..=6.
    pub callout_max: u8,
}

impl Default for GameSettings {
    fn default() -> Self {
        // 1.0 everywhere == Bevy defaults, so out of the box nothing is quieter
        // or scaled differently than before — the controls only let players
        // turn things down / adjust from a neutral baseline.
        Self {
            master_volume: 1.0,
            sfx_volume: 1.0,
            music_volume: 1.0,
            ui_volume: 1.0,
            muted: false,
            fullscreen: false,
            vsync: true,
            resolution_index: 0,
            ui_scale: 1.0,
            invert_aim_y: false,
            aim_deadzone: 0.35,
            callout_style: 0,
            callout_words: true,
            callout_heading: true,
            callout_max: 4,
        }
    }
}

impl GameSettings {
    fn bus_mut(&mut self, bus: AudioBus) -> &mut f32 {
        match bus {
            AudioBus::Master => &mut self.master_volume,
            AudioBus::Sfx => &mut self.sfx_volume,
            AudioBus::Music => &mut self.music_volume,
            AudioBus::Ui => &mut self.ui_volume,
        }
    }

    pub fn bus(&self, bus: AudioBus) -> f32 {
        match bus {
            AudioBus::Master => self.master_volume,
            AudioBus::Sfx => self.sfx_volume,
            AudioBus::Music => self.music_volume,
            AudioBus::Ui => self.ui_volume,
        }
    }

    fn resolution(&self) -> (u32, u32) {
        RESOLUTIONS
            .get(self.resolution_index)
            .copied()
            .unwrap_or(RESOLUTIONS[0])
    }
}

/// Whether the Settings overlay is showing, and which tab. Toggled by buttons /
/// Escape; `manage_settings_overlay` spawns or despawns the overlay to match.
#[derive(Resource, Default)]
pub struct SettingsMenu {
    pub open: bool,
    pub tab: SettingsTab,
}

/// Root marker for the Settings overlay (for despawn).
#[derive(Component)]
pub struct SettingsOverlay;

/// Marks the currently-active tab button so `menu_button_visuals` leaves its
/// highlight alone (it keeps the styling baked at build time).
#[derive(Component)]
pub struct ActiveTabButton;

/// A dynamic value label inside the Settings overlay, refreshed each frame.
#[derive(Component, Clone, Copy)]
pub enum SettingsValue {
    Volume(AudioBus),
    Mute,
    Fullscreen,
    Vsync,
    Resolution,
    UiScale,
    InvertAimY,
    AimDeadzone,
    CalloutStyle,
    CalloutWords,
    CalloutHeading,
    CalloutMax,
}

// ============================================================================
// ACTIONS
// ============================================================================

/// What a clickable menu button does when pressed.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub enum MenuAction {
    /// Main menu → start a fresh expedition (StationDocked).
    NewGame,
    /// Load a specific save slot (99 = auto-save).
    LoadSlot(u32),
    /// Save the current game to a slot.
    SaveSlot(u32),
    /// Open the Settings overlay.
    OpenSettings,
    /// Close the Settings overlay.
    CloseSettings,
    /// Switch the visible settings tab.
    SelectSettingsTab(SettingsTab),
    /// Pause menu → resume play (returns to the pre-pause state).
    Resume,
    /// Pause menu → abandon the run and return to the main menu.
    QuitToMainMenu,
    /// Main menu → exit the application.
    QuitToDesktop,
    /// Game-over screen → back to the main menu.
    ReturnToMainMenu,

    // --- Sound ---
    /// Nudge a mixer bus ±10%.
    AdjustVolume { bus: AudioBus, up: bool },
    ToggleMute,

    // --- Graphics ---
    ToggleFullscreen,
    ToggleVsync,
    CycleResolution { up: bool },
    AdjustUiScale { up: bool },

    // --- Controls ---
    ToggleInvertAimY,
    AdjustAimDeadzone { up: bool },

    // --- Callout lines ---
    CycleCalloutStyle { up: bool },
    ToggleCalloutWords,
    ToggleCalloutHeading,
    AdjustCalloutMax { up: bool },

    /// Restore every setting to its default.
    ResetSettings,
}

/// Marker + payload on every clickable menu button.
#[derive(Component, Clone, Copy)]
pub struct MenuButton {
    pub action: MenuAction,
}

// ============================================================================
// BUTTON CONSTRUCTION HELPERS
// ============================================================================

/// Spawn a full-width menu button (label on the left, optional key hint on the
/// right). Styled from the theme so it matches the rest of the UI.
pub fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    hint: Option<&str>,
    accent: Color,
    action: MenuAction,
) {
    parent
        .spawn((
            Node {
                width: Val::Px(320.0),
                padding: UiRect::new(
                    Val::Px(ThemeSpacing::XL),
                    Val::Px(ThemeSpacing::XL),
                    Val::Px(ThemeSpacing::MD),
                    Val::Px(ThemeSpacing::MD),
                ),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(ThemeSpacing::MD),
                ..default()
            },
            BackgroundColor(ThemeColors::BG_ELEVATED),
            BorderColor::all(ThemeColors::BORDER_DEFAULT),
            Button,
            Interaction::default(),
            MenuButton { action },
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() },
                TextColor(accent),
            ));
            if let Some(hint) = hint {
                b.spawn((
                    Text::new(hint),
                    TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                    TextColor(ThemeColors::TEXT_MUTED),
                ));
            }
        });
}

/// Spawn a compact square "chip" button (used for save/load slot rows and the
/// volume −/+ steppers).
pub fn spawn_chip_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    accent: Color,
    action: MenuAction,
) {
    parent
        .spawn((
            Node {
                min_width: Val::Px(40.0),
                padding: UiRect::new(
                    Val::Px(ThemeSpacing::MD),
                    Val::Px(ThemeSpacing::MD),
                    Val::Px(ThemeSpacing::SM),
                    Val::Px(ThemeSpacing::SM),
                ),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ThemeColors::BG_ELEVATED),
            BorderColor::all(ThemeColors::BORDER_DEFAULT),
            Button,
            Interaction::default(),
            MenuButton { action },
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() },
                TextColor(accent),
            ));
        });
}

/// A toggle pill (OFF/ON) carrying a `SettingsValue` so its label recolors live.
fn spawn_toggle_button(
    parent: &mut ChildSpawnerCommands,
    value: SettingsValue,
    action: MenuAction,
) {
    parent
        .spawn((
            Node {
                min_width: Val::Px(72.0),
                padding: UiRect::new(
                    Val::Px(ThemeSpacing::LG),
                    Val::Px(ThemeSpacing::LG),
                    Val::Px(ThemeSpacing::SM),
                    Val::Px(ThemeSpacing::SM),
                ),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ThemeColors::BG_ELEVATED),
            BorderColor::all(ThemeColors::BORDER_DEFAULT),
            Button,
            Interaction::default(),
            MenuButton { action },
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("--"),
                TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() },
                TextColor(ThemeColors::TEXT_SECONDARY),
                value,
            ));
        });
}

// ============================================================================
// SYSTEMS — interaction
// ============================================================================

/// Hover/press color feedback for every menu button (except the active tab,
/// which keeps its baked-in highlight).
pub fn menu_button_visuals(
    mut query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<MenuButton>, Without<ActiveTabButton>, Changed<Interaction>),
    >,
) {
    for (interaction, mut bg, mut border) in query.iter_mut() {
        *bg = theme::button_color_for_interaction(interaction).into();
        *border = match interaction {
            Interaction::Hovered | Interaction::Pressed => {
                BorderColor::all(ThemeColors::BORDER_ACTIVE)
            }
            Interaction::None => BorderColor::all(ThemeColors::BORDER_DEFAULT),
        };
    }
}

/// Fire a button's action once, on the frame it becomes pressed.
pub fn menu_button_dispatch(
    interactions: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    current_state: Res<State<GameState>>,
    pre_pause: Res<PrePauseState>,
    mut next_state: ResMut<NextState<GameState>>,
    mut settings_menu: ResMut<SettingsMenu>,
    mut game_settings: ResMut<GameSettings>,
    mut save_ev: MessageWriter<SaveGameRequest>,
    mut load_ev: MessageWriter<LoadGameRequest>,
    mut exit_ev: MessageWriter<AppExit>,
    mut new_expedition: MessageWriter<crate::events::NewExpeditionRequest>,
    mut tutorial: ResMut<crate::tutorial::Tutorial>,
) {
    for (interaction, btn) in interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn.action {
            MenuAction::NewGame => {
                // This used to be the whole arm. Clicking the button started a
                // game with no tutorial and no reset, while pressing Enter did
                // both — the same menu item behaving differently depending on
                // how it was activated.
                tutorial.begin();
                new_expedition.write(crate::events::NewExpeditionRequest);
                next_state.set(GameState::StationDocked);
            }
            MenuAction::LoadSlot(slot) => {
                settings_menu.open = false;
                load_ev.write(LoadGameRequest { slot });
            }
            MenuAction::SaveSlot(slot) => {
                save_ev.write(SaveGameRequest { slot });
            }
            MenuAction::OpenSettings => settings_menu.open = true,
            MenuAction::CloseSettings => settings_menu.open = false,
            MenuAction::SelectSettingsTab(tab) => settings_menu.tab = tab,
            MenuAction::Resume => {
                let target = pre_pause.0.unwrap_or(GameState::Exploring);
                next_state.set(target);
            }
            MenuAction::QuitToMainMenu => {
                settings_menu.open = false;
                next_state.set(GameState::MainMenu);
            }
            MenuAction::QuitToDesktop => {
                exit_ev.write(AppExit::Success);
            }
            MenuAction::ReturnToMainMenu => {
                if *current_state.get() == GameState::GameOver {
                    next_state.set(GameState::MainMenu);
                }
            }

            // ---- Sound ----
            MenuAction::AdjustVolume { bus, up } => {
                if bus == AudioBus::Master {
                    game_settings.muted = false;
                }
                let v = game_settings.bus_mut(bus);
                *v = round1((*v + if up { 0.1 } else { -0.1 }).clamp(0.0, 1.0));
            }
            MenuAction::ToggleMute => {
                game_settings.muted = !game_settings.muted;
            }

            // ---- Graphics ----
            MenuAction::ToggleFullscreen => {
                game_settings.fullscreen = !game_settings.fullscreen;
            }
            MenuAction::ToggleVsync => {
                game_settings.vsync = !game_settings.vsync;
            }
            MenuAction::CycleResolution { up } => {
                let last = RESOLUTIONS.len() - 1;
                game_settings.resolution_index = if up {
                    (game_settings.resolution_index + 1).min(last)
                } else {
                    game_settings.resolution_index.saturating_sub(1)
                };
            }
            MenuAction::AdjustUiScale { up } => {
                game_settings.ui_scale =
                    round1((game_settings.ui_scale + if up { 0.1 } else { -0.1 }).clamp(0.5, 1.5));
            }

            // ---- Controls ----
            MenuAction::ToggleInvertAimY => {
                game_settings.invert_aim_y = !game_settings.invert_aim_y;
            }
            MenuAction::AdjustAimDeadzone { up } => {
                let d = (game_settings.aim_deadzone + if up { 0.05 } else { -0.05 }).clamp(0.05, 0.6);
                game_settings.aim_deadzone = round2(d);
            }

            MenuAction::CycleCalloutStyle { up } => {
                game_settings.callout_style = (game_settings.callout_style + if up { 1 } else { 2 }) % 3;
            }
            MenuAction::ToggleCalloutWords => {
                game_settings.callout_words = !game_settings.callout_words;
            }
            MenuAction::ToggleCalloutHeading => {
                game_settings.callout_heading = !game_settings.callout_heading;
            }
            MenuAction::AdjustCalloutMax { up } => {
                game_settings.callout_max = if up { (game_settings.callout_max + 1).min(6) } else { game_settings.callout_max.saturating_sub(1).max(1) };
            }

            MenuAction::ResetSettings => {
                *game_settings = GameSettings::default();
            }
        }
    }
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}
fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

// ============================================================================
// SYSTEMS — persistence & application
// ============================================================================

/// Load settings from disk at startup (falls back to defaults). Mutating the
/// resource here marks it changed, so the apply systems run on the first frame.
pub fn load_settings(mut settings: ResMut<GameSettings>) {
    if let Ok(data) = std::fs::read_to_string(SETTINGS_PATH) {
        if let Ok(loaded) = serde_json::from_str::<GameSettings>(&data) {
            *settings = loaded;
            info!("Loaded settings from {}", SETTINGS_PATH);
        }
    }
}

/// Persist settings whenever they change (only ever on discrete menu clicks).
pub fn save_settings(settings: Res<GameSettings>) {
    if !settings.is_changed() {
        return;
    }
    let _ = std::fs::create_dir_all("meta");
    if let Ok(data) = serde_json::to_string_pretty(settings.as_ref()) {
        let _ = std::fs::write(SETTINGS_PATH, data);
    }
}

/// Mirror master-volume/mute onto Bevy's `GlobalVolume` (scales all audio).
/// The per-bus scalars (sfx/music/ui) are applied inside `audio.rs`.
pub fn apply_audio_settings(settings: Res<GameSettings>, mut global: ResMut<GlobalVolume>) {
    if !settings.is_changed() {
        return;
    }
    // AFTERLIGHT_MUTE=1: automated runs are silent whatever the saved settings
    // say, so a test playing in the background never reaches the speakers.
    let forced = std::env::var("AFTERLIGHT_MUTE").ok().as_deref() == Some("1");
    let effective = if settings.muted || forced { 0.0 } else { settings.master_volume };
    global.volume = Volume::Linear(effective);
}

/// Push fullscreen / vsync / resolution / ui-scale onto the window. Guards each
/// write against its current value so an unrelated settings change (e.g. a
/// volume nudge) never needlessly reconfigures the render surface.
pub fn apply_display_settings(
    settings: Res<GameSettings>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
) {
    if !settings.is_changed() {
        return;
    }

    if (ui_scale.0 - settings.ui_scale).abs() > f32::EPSILON {
        ui_scale.0 = settings.ui_scale;
    }

    let Ok(mut win) = window.single_mut() else { return };

    let desired_mode = if settings.fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };
    if win.mode != desired_mode {
        win.mode = desired_mode;
    }

    // A perf run (perf.rs) keeps vsync off whatever the settings say, or it
    // measures the display's refresh instead of the frame.
    let desired_present = if settings.vsync && !crate::perf::perf_mode() {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
    if win.present_mode != desired_present {
        win.present_mode = desired_present;
    }

    // Resolution only makes sense windowed; borderless fullscreen tracks the
    // monitor. Applied when windowed (including right after leaving fullscreen).
    if !settings.fullscreen {
        let (w, h) = settings.resolution();
        let (w, h) = (w as f32, h as f32);
        if (win.resolution.width() - w).abs() > 0.5 || (win.resolution.height() - h).abs() > 0.5 {
            win.resolution.set(w, h);
        }
    }
}

/// Spawn/despawn the Settings overlay to match `SettingsMenu` (open + tab).
pub fn manage_settings_overlay(
    settings: Res<SettingsMenu>,
    existing: Query<Entity, With<SettingsOverlay>>,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    if settings.open {
        spawn_settings_overlay(&mut commands, settings.tab);
    }
}

/// Keep the dynamic Settings labels in sync every frame while the overlay is up.
pub fn update_settings_values(
    settings_menu: Res<SettingsMenu>,
    game_settings: Res<GameSettings>,
    mut query: Query<(&SettingsValue, &mut Text, &mut TextColor)>,
) {
    if !settings_menu.open {
        return;
    }
    for (kind, mut text, mut color) in query.iter_mut() {
        match kind {
            SettingsValue::Volume(bus) => {
                text.0 = format!("{}%", (game_settings.bus(*bus) * 100.0).round() as i32);
                color.0 = ThemeColors::ACCENT_BLUE;
            }
            SettingsValue::Mute => set_toggle(&mut text, &mut color, game_settings.muted, ThemeColors::ACCENT_ORANGE),
            SettingsValue::Fullscreen => set_toggle(&mut text, &mut color, game_settings.fullscreen, ThemeColors::ACCENT_GREEN),
            SettingsValue::Vsync => set_toggle(&mut text, &mut color, game_settings.vsync, ThemeColors::ACCENT_GREEN),
            SettingsValue::InvertAimY => set_toggle(&mut text, &mut color, game_settings.invert_aim_y, ThemeColors::ACCENT_GREEN),
            SettingsValue::CalloutWords => set_toggle(&mut text, &mut color, game_settings.callout_words, ThemeColors::ACCENT_GREEN),
            SettingsValue::CalloutHeading => set_toggle(&mut text, &mut color, game_settings.callout_heading, ThemeColors::ACCENT_GREEN),
            SettingsValue::CalloutStyle => {
                text.0 = ["FULL", "SMALL", "OFF"][(game_settings.callout_style % 3) as usize].to_string();
                color.0 = ThemeColors::ACCENT_BLUE;
            }
            SettingsValue::CalloutMax => {
                text.0 = game_settings.callout_max.to_string();
                color.0 = ThemeColors::ACCENT_BLUE;
            }
            SettingsValue::Resolution => {
                let (w, h) = game_settings.resolution();
                text.0 = format!("{}x{}", w, h);
                color.0 = ThemeColors::ACCENT_BLUE;
            }
            SettingsValue::UiScale => {
                text.0 = format!("{}%", (game_settings.ui_scale * 100.0).round() as i32);
                color.0 = ThemeColors::ACCENT_BLUE;
            }
            SettingsValue::AimDeadzone => {
                text.0 = format!("{}%", (game_settings.aim_deadzone * 100.0).round() as i32);
                color.0 = ThemeColors::ACCENT_BLUE;
            }
        }
    }
}

fn set_toggle(text: &mut Text, color: &mut TextColor, on: bool, on_color: Color) {
    text.0 = if on { "ON".into() } else { "OFF".into() };
    color.0 = if on { on_color } else { ThemeColors::TEXT_SECONDARY };
}

/// Force the Settings overlay closed on any state exit (safety net so it never
/// lingers into gameplay).
pub fn close_settings_on_exit(
    mut settings: ResMut<SettingsMenu>,
    existing: Query<Entity, With<SettingsOverlay>>,
    mut commands: Commands,
) {
    if settings.open {
        settings.open = false;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// SETTINGS OVERLAY LAYOUT
// ============================================================================

fn spawn_settings_overlay(commands: &mut Commands, tab: SettingsTab) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            // Modal scrim above the main menu (ZIndex 100) and the pause
            // screen (GlobalZIndex 150, over the HUD).
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(200),
            // Interaction (with no MenuButton) makes the scrim the topmost
            // pickable node, absorbing clicks so nothing behind the modal
            // reacts. The dispatch/visual systems ignore it (they require
            // MenuButton).
            Button,
            Interaction::default(),
            SettingsOverlay,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(ThemeSpacing::XXL)),
                    row_gap: Val::Px(ThemeSpacing::LG),
                    border: UiRect::all(Val::Px(1.0)),
                    min_width: Val::Px(500.0),
                    ..default()
                },
                BackgroundColor(ThemeColors::BG_PANEL),
                BorderColor::all(ThemeColors::BORDER_BRIGHT),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("SETTINGS"),
                    TextFont { font_size: FontSize::Px(ThemeFonts::H2), ..default() },
                    TextColor(ThemeColors::TEXT_TITLE),
                ));

                // ---- Tab bar ----
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(ThemeSpacing::SM),
                        margin: UiRect::vertical(Val::Px(ThemeSpacing::SM)),
                        ..default()
                    })
                    .with_children(|tabs| {
                        tab_button(tabs, "SOUND", SettingsTab::Sound, tab);
                        tab_button(tabs, "GRAPHICS", SettingsTab::Graphics, tab);
                        tab_button(tabs, "CONTROLS", SettingsTab::Controls, tab);
                    });

                divider(panel);

                // ---- Content (fixed-ish height so switching tabs doesn't jump) ----
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(ThemeSpacing::MD),
                        min_height: Val::Px(240.0),
                        width: Val::Percent(100.0),
                        ..default()
                    })
                    .with_children(|content| match tab {
                        SettingsTab::Sound => sound_tab(content),
                        SettingsTab::Graphics => graphics_tab(content),
                        SettingsTab::Controls => controls_tab(content),
                    });

                divider(panel);

                // ---- Footer: reset + back ----
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(ThemeSpacing::MD),
                        ..default()
                    })
                    .with_children(|footer| {
                        footer_button(footer, "RESET DEFAULTS", ThemeColors::TEXT_MUTED, MenuAction::ResetSettings);
                        footer_button(footer, "BACK  -  Esc", ThemeColors::TEXT_PRIMARY, MenuAction::CloseSettings);
                    });
            });
        });
}

// ---- Per-tab content ---------------------------------------------------------

fn sound_tab(content: &mut ChildSpawnerCommands) {
    stepper_row(content, "MASTER VOLUME", SettingsValue::Volume(AudioBus::Master), AudioBus::Master);
    stepper_row(content, "SFX VOLUME", SettingsValue::Volume(AudioBus::Sfx), AudioBus::Sfx);
    stepper_row(content, "MUSIC VOLUME", SettingsValue::Volume(AudioBus::Music), AudioBus::Music);
    stepper_row(content, "UI VOLUME", SettingsValue::Volume(AudioBus::Ui), AudioBus::Ui);
    toggle_row(content, "MUTE ALL", SettingsValue::Mute, MenuAction::ToggleMute);
}

fn graphics_tab(content: &mut ChildSpawnerCommands) {
    toggle_row(content, "FULLSCREEN", SettingsValue::Fullscreen, MenuAction::ToggleFullscreen);
    toggle_row(content, "VSYNC", SettingsValue::Vsync, MenuAction::ToggleVsync);
    generic_stepper_row(
        content,
        "RESOLUTION",
        SettingsValue::Resolution,
        MenuAction::CycleResolution { up: false },
        MenuAction::CycleResolution { up: true },
    );
    generic_stepper_row(
        content,
        "UI SCALE",
        SettingsValue::UiScale,
        MenuAction::AdjustUiScale { up: false },
        MenuAction::AdjustUiScale { up: true },
    );
    caption(content, "Resolution applies in windowed mode.");
    divider_thin(content);
    generic_stepper_row(
        content,
        "CALLOUT LINES",
        SettingsValue::CalloutStyle,
        MenuAction::CycleCalloutStyle { up: false },
        MenuAction::CycleCalloutStyle { up: true },
    );
    toggle_row(content, "WORDS ON THE LINES", SettingsValue::CalloutWords, MenuAction::ToggleCalloutWords);
    toggle_row(content, "HEADING ARROW", SettingsValue::CalloutHeading, MenuAction::ToggleCalloutHeading);
    generic_stepper_row(
        content,
        "MOST NOTES AT ONCE",
        SettingsValue::CalloutMax,
        MenuAction::AdjustCalloutMax { up: false },
        MenuAction::AdjustCalloutMax { up: true },
    );
    caption(content, "Callout lines are the notes pinned to your ship. Small puts a number on the ship and the words in the title block.");
}

fn controls_tab(content: &mut ChildSpawnerCommands) {
    // Read-only cheatsheet in two columns.
    content
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(ThemeSpacing::SECTION),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|cols| {
            reference_column(cols, "KEYBOARD", &[
                ("WASD", "Move"),
                ("Q / E", "Thrusters"),
                ("Shift", "Brake"),
                ("Space", "Fire"),
                ("Z", "Radar ping"),
                ("F", "Interact / dock"),
                ("B", "Build mode"),
                ("C", "Crew  -  M  Map"),
                ("Esc", "Pause"),
            ]);
            reference_column(cols, "GAMEPAD", &[
                ("L-Stick", "Throttle / strafe"),
                ("R-Stick", "Aim"),
                ("RT", "Fire  -  LT  Brake"),
                ("A", "Confirm  -  B  Cancel"),
                ("X", "Interact / dock"),
                ("Y", "Radar ping"),
                ("LB", "Cycle target"),
                ("RB", "Build mode"),
                ("Start", "Pause"),
            ]);
        });

    divider_thin(content);

    // Tunable gamepad options.
    toggle_row(content, "INVERT AIM Y (PAD)", SettingsValue::InvertAimY, MenuAction::ToggleInvertAimY);
    generic_stepper_row(
        content,
        "AIM DEADZONE (PAD)",
        SettingsValue::AimDeadzone,
        MenuAction::AdjustAimDeadzone { up: false },
        MenuAction::AdjustAimDeadzone { up: true },
    );
}

// ---- Row / element builders --------------------------------------------------

/// A `[−] value [+]` volume row.
fn stepper_row(parent: &mut ChildSpawnerCommands, label: &str, value: SettingsValue, bus: AudioBus) {
    generic_stepper_row(
        parent,
        label,
        value,
        MenuAction::AdjustVolume { bus, up: false },
        MenuAction::AdjustVolume { bus, up: true },
    );
}

/// A `[−] value [+]` stepper row with arbitrary down/up actions.
fn generic_stepper_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: SettingsValue,
    down: MenuAction,
    up: MenuAction,
) {
    settings_row(parent, label, |controls| {
        spawn_chip_button(controls, "-", ThemeColors::TEXT_PRIMARY, down);
        controls
            .spawn(Node {
                min_width: Val::Px(88.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|v| {
                v.spawn((
                    Text::new("--"),
                    TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() },
                    TextColor(ThemeColors::ACCENT_BLUE),
                    value,
                ));
            });
        spawn_chip_button(controls, "+", ThemeColors::TEXT_PRIMARY, up);
    });
}

/// A `label ....... [toggle]` row.
fn toggle_row(parent: &mut ChildSpawnerCommands, label: &str, value: SettingsValue, action: MenuAction) {
    settings_row(parent, label, |controls| {
        spawn_toggle_button(controls, value, action);
    });
}

/// One labelled settings row: "LABEL" on the left, caller-built controls right.
fn settings_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    controls: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            width: Val::Px(420.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(ThemeSpacing::LG),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() },
                TextColor(ThemeColors::TEXT_SECONDARY),
            ));
            row.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(ThemeSpacing::MD),
                ..default()
            })
            .with_children(controls);
        });
}

/// A tab-bar button. The active tab keeps a baked highlight (see
/// `ActiveTabButton`); inactive tabs get normal hover feedback.
fn tab_button(parent: &mut ChildSpawnerCommands, label: &str, tab: SettingsTab, current: SettingsTab) {
    let active = tab == current;
    let (bg, border, text) = if active {
        (ThemeColors::BG_PRESSED, ThemeColors::BORDER_ACTIVE, ThemeColors::ACCENT_BLUE)
    } else {
        (ThemeColors::BG_ELEVATED, ThemeColors::BORDER_DEFAULT, ThemeColors::TEXT_SECONDARY)
    };
    let mut ec = parent.spawn((
        Node {
            min_width: Val::Px(130.0),
            padding: UiRect::new(
                Val::Px(ThemeSpacing::LG),
                Val::Px(ThemeSpacing::LG),
                Val::Px(ThemeSpacing::MD),
                Val::Px(ThemeSpacing::MD),
            ),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(bg),
        BorderColor::all(border),
        Button,
        Interaction::default(),
        MenuButton { action: MenuAction::SelectSettingsTab(tab) },
    ));
    if active {
        ec.insert(ActiveTabButton);
    }
    ec.with_children(|b| {
        b.spawn((
            Text::new(label),
            TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() },
            TextColor(text),
        ));
    });
}

/// A footer button (reset / back) — compact, label-only.
fn footer_button(parent: &mut ChildSpawnerCommands, label: &str, accent: Color, action: MenuAction) {
    parent
        .spawn((
            Node {
                padding: UiRect::new(
                    Val::Px(ThemeSpacing::XL),
                    Val::Px(ThemeSpacing::XL),
                    Val::Px(ThemeSpacing::MD),
                    Val::Px(ThemeSpacing::MD),
                ),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ThemeColors::BG_ELEVATED),
            BorderColor::all(ThemeColors::BORDER_DEFAULT),
            Button,
            Interaction::default(),
            MenuButton { action },
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() },
                TextColor(accent),
            ));
        });
}

/// A column of read-only `KEY  action` reference rows under a header.
fn reference_column(parent: &mut ChildSpawnerCommands, header: &str, rows: &[(&str, &str)]) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(ThemeSpacing::SM),
            min_width: Val::Px(210.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(header),
                TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                TextColor(ThemeColors::TEXT_MUTED),
                Node { margin: UiRect::bottom(Val::Px(ThemeSpacing::XS)), ..default() },
            ));
            for (key, action) in rows {
                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(ThemeSpacing::MD),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Node { min_width: Val::Px(64.0), ..default() },
                    ))
                    .with_children(|k| {
                        k.spawn((
                            Text::new(*key),
                            TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() },
                            TextColor(ThemeColors::ACCENT_BLUE),
                        ));
                    });
                    row.spawn((
                        Text::new(*action),
                        TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() },
                        TextColor(ThemeColors::TEXT_SECONDARY),
                    ));
                });
            }
        });
}

fn caption(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
        TextColor(ThemeColors::TEXT_MUTED),
    ));
}

fn divider(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node { width: Val::Px(460.0), height: Val::Px(1.0), ..default() },
        BackgroundColor(ThemeColors::BORDER_DEFAULT),
    ));
}

fn divider_thin(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: Val::Px(420.0),
            height: Val::Px(1.0),
            margin: UiRect::vertical(Val::Px(ThemeSpacing::XS)),
            ..default()
        },
        BackgroundColor(ThemeColors::BORDER_SUBTLE),
    ));
}
