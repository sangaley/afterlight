pub mod build_ui;
pub mod damage_overlay;
pub mod pressure_overlay;
pub mod windows;
pub mod theme;
pub mod cursor;
pub mod menu_buttons;
pub mod offscreen_markers;

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::input::InputSystems;
use crate::states::{GameState, BuildState};
use crate::resources::*;
use crate::events::*;
use crate::components::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<pressure_overlay::FlowTrails>()
            .init_resource::<PrePauseState>()
            .init_resource::<OpenDutyDropdown>()
            .init_resource::<CustomizationState>()
            .init_resource::<ComponentPlacementState>()
            .init_resource::<PieceCustomizationState>()
            .init_resource::<windows::framework::WindowZCounter>()
            .init_resource::<windows::power_routing::PowerSliderDrag>()
            .init_resource::<windows::tooltip::TooltipState>()
            .init_resource::<windows::notification_log::NotificationHistory>()
            .init_resource::<PendingWarpTarget>()
            .init_resource::<MapViewMode>()
            .init_resource::<MapZoom>()
            .init_resource::<menu_buttons::GameSettings>()
            .init_resource::<menu_buttons::SettingsMenu>()
            // UiScale is normally inserted by Bevy's UiPlugin; init defensively
            // so apply_display_settings can always drive it.
            .init_resource::<bevy::ui::UiScale>()
            // Load persisted settings before the apply systems' first run.
            .add_systems(Startup, (setup_ui, cursor::setup_custom_cursor, menu_buttons::load_settings))
            // Clickable menu buttons + Settings overlay (main/pause/game-over).
            // These run in every state — the queries are empty unless a menu
            // is on screen, so there's no per-frame cost during play.
            .add_systems(
                Update,
                (
                    menu_buttons::menu_button_visuals,
                    menu_buttons::menu_button_dispatch,
                    menu_buttons::apply_audio_settings,
                    menu_buttons::apply_display_settings,
                    menu_buttons::save_settings,
                    menu_buttons::manage_settings_overlay,
                    menu_buttons::update_settings_values,
                ),
            )
            // Never let the Settings overlay leak into gameplay.
            .add_systems(OnExit(GameState::MainMenu), menu_buttons::close_settings_on_exit)
            .add_systems(OnExit(GameState::Paused), menu_buttons::close_settings_on_exit)
            // HUD toolbar: synthesize the key while a button is held (PreUpdate
            // after InputSystems, like the gamepad bridge); recolor on hover.
            .add_systems(PreUpdate, hud_action_button_press.after(InputSystems).run_if(
                in_state(GameState::Exploring).or_else(in_state(GameState::StationDocked))))
            .add_systems(Update, (
                hud_action_button_hover,
                toggle_flight_toolbar_visibility,
                rebuild_action_toolbar,
                weapon_rack_visibility,
                update_weapon_rack.run_if(in_state(GameState::Exploring)),
                pin_below_top_bar,
            ))
            .add_systems(
                Update,
                (
                    cursor::update_custom_cursor,
                    update_hud,
                    update_hud_secondary,
                    update_celestial_hud,
                    handle_notifications,
                    update_notifications,
                    handle_menu_input,
                    // Floating window systems
                    windows::framework::window_drag_system,
                    windows::framework::window_close_system,
                    windows::framework::window_collapse_system,
                    windows::framework::window_button_hover_system,
                    windows::framework::scroll_on_wheel,
                    // Tooltip
                    windows::tooltip::tooltip_system,
                    windows::tooltip::tooltip_position_system,
                    // Notification log
                    windows::notification_log::record_notifications,
                ),
            )
            // Main menu
            .add_systems(OnEnter(GameState::MainMenu), (spawn_main_menu, hide_hud))
            .add_systems(OnEnter(GameState::Exploring), show_hud)
            .add_systems(
                Update,
                offscreen_markers::update_offscreen_markers.run_if(in_state(GameState::Exploring)),
            )
            .add_systems(OnExit(GameState::Exploring), (offscreen_markers::clear_offscreen_markers, clear_hull_warning_overlay))
            .add_systems(OnEnter(GameState::StationDocked), show_hud)
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
            // Game Over screen
            .add_systems(OnEnter(GameState::GameOver), spawn_game_over_screen)
            .add_systems(OnExit(GameState::GameOver), despawn_game_over_screen)
            .add_systems(
                Update,
                game_over_input.run_if(in_state(GameState::GameOver)),
            )
            // Pause menu
            .add_systems(OnEnter(GameState::Paused), spawn_pause_menu)
            .add_systems(OnExit(GameState::Paused), despawn_pause_menu)
            .add_systems(
                Update,
                (
                    toggle_module_panel,
                    module_panel_input,
                    save_load_input,
                ).run_if(in_state(GameState::Paused)),
            )
            // Quick-save (F5) / quick-load (F9) during normal play — no menu needed
            .add_systems(
                Update,
                quick_save_load_input.run_if(
                    in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))
                        .or_else(in_state(GameState::Paused)),
                ),
            )
            // Docked state
            .add_systems(OnEnter(GameState::Docked), spawn_docking_menu)
            .add_systems(OnExit(GameState::Docked), despawn_docking_menu)
            // The same services at a station berth, on U (the toolbar's Shop).
            .init_resource::<StationServicesRequested>()
            .add_systems(
                Update,
                (
                    station_services_key,
                    spawn_docking_menu.run_if(|asked: Res<StationServicesRequested>| asked.0),
                    clear_station_services_request,
                )
                    .chain()
                    .run_if(in_state(GameState::StationDocked)),
            )
            .add_systems(OnExit(GameState::StationDocked), despawn_docking_menu)
            .add_systems(
                Update,
                (docking_menu_input, highlight_dock_rows)
                    .chain()
                    .run_if(in_state(GameState::Docked).or_else(in_state(GameState::StationDocked))),
            )
            // Game event notifications (while exploring)
            .add_systems(
                Update,
                (
                    handle_game_event_notifications,
                    update_hull_warning_overlay,
                    // Floating windows (exploring)
                    windows::minimap::toggle_minimap,
                    windows::minimap::update_minimap,
                    windows::notification_log::toggle_notification_log,
                    // Power routing window (Weapons/Shields/Engines)
                    windows::power_routing::toggle_power_window,
                    windows::power_routing::power_slider_drag,
                    windows::power_routing::power_preset_click,
                    windows::power_routing::power_button_hover,
                    windows::power_routing::power_window_refresh,
                    // Radial menu
                    windows::radial_menu::spawn_radial_on_right_click,
                    windows::radial_menu::update_radial_menu,
                    windows::radial_menu::radial_menu_input,
                ).run_if(in_state(GameState::Exploring)),
            )
            // Inspection & customization — the window is opened while docked/building,
            // so its buttons must stay responsive in both StationDocked and Exploring
            // (it can be left open across the undock transition).
            .add_systems(
                Update,
                (
                    windows::inspection::slot_button_click,
                    windows::inspection::slot_button_hover,
                    windows::inspection::customize_button_hover,
                    windows::inspection::preset_button_click,
                    windows::inspection::preset_button_hover,
                    windows::inspection::custom_preset_button_click,
                    windows::inspection::custom_preset_button_hover,
                    windows::inspection::save_build_button_click,
                    windows::inspection::save_build_button_hover,
                    windows::customization::slider_click_system,
                    windows::customization::undo_button_hover,
                ).run_if(
                    in_state(GameState::Exploring).or_else(in_state(GameState::StationDocked))
                ),
            )
            // Weapon tuning — dock-side workshop only. Windows close on undock.
            .init_resource::<windows::tuning::ActiveSliderDrag>()
            .add_systems(
                Update,
                (
                    windows::tuning::right_click_open_tuning,
                    windows::tuning::tuning_slider_drag,
                    windows::tuning::ammo_button_click,
                    windows::tuning::reset_tuning_click,
                    windows::tuning::tuning_window_refresh,
                    windows::shell_designer::shell_designer_click,
                    windows::shell_designer::shell_designer_refresh,
                    windows::shell_designer::save_gun_design_click,
                    windows::missile_designer::missile_designer_click,
                    windows::shell_designer::point_defence_click,
                    windows::shell_designer::point_defence_refresh,
                    windows::missile_designer::missile_designer_refresh,
                ).run_if(in_state(GameState::StationDocked)),
            )
            .add_systems(OnExit(GameState::StationDocked), windows::tuning::despawn_tuning_windows)
            // Cargo hold panel — Haven has no trade menu (that's remote
            // outposts, GameState::Docked) and the M inventory overlay is
            // flying-only, so the itemized hold was unreadable at the home
            // station. This shows it top-left while docked/building.
            .add_systems(OnEnter(GameState::StationDocked), spawn_station_cargo_panel)
            .add_systems(OnExit(GameState::StationDocked), despawn_station_cargo_panel)
            .add_systems(
                Update,
                update_station_cargo_panel.run_if(in_state(GameState::StationDocked)),
            )
            // Damage overlay (while exploring) — chained for correct ordering
            .add_systems(
                Update,
                (
                    damage_overlay::toggle_damage_overlay,
                    damage_overlay::spawn_overlay_legend.after(damage_overlay::toggle_damage_overlay),
                    damage_overlay::despawn_overlay_legend.after(damage_overlay::toggle_damage_overlay),
                    damage_overlay::update_damage_overlay.after(damage_overlay::spawn_overlay_legend),
                    damage_overlay::cleanup_damage_overlay.after(damage_overlay::toggle_damage_overlay),
                    // Pressure view rides the same shape as the damage view
                    // and turns it off when it comes up -- both tint blocks.
                    pressure_overlay::toggle_pressure_overlay,
                    pressure_overlay::update_pressure_overlay
                        .after(pressure_overlay::toggle_pressure_overlay),
                    pressure_overlay::draw_pressure_flow
                        .after(pressure_overlay::update_pressure_overlay),
                ).run_if(in_state(GameState::Exploring)),
            )
            // Clean up overlay legend/sprites on state transitions
            .add_systems(OnEnter(GameState::GameOver), (damage_overlay::cleanup_overlay_on_exit, pressure_overlay::cleanup_pressure_overlay_on_exit))
            .add_systems(OnEnter(GameState::MainMenu), (damage_overlay::cleanup_overlay_on_exit, pressure_overlay::cleanup_pressure_overlay_on_exit))
            .add_systems(OnEnter(GameState::GameOver), windows::framework::close_all_windows)
            .add_systems(OnEnter(GameState::MainMenu), windows::framework::close_all_windows)
            .add_systems(OnEnter(GameState::StationDocked), (
                windows::framework::close_flight_windows,
                damage_overlay::overlays_off_at_berth,
                damage_overlay::cleanup_overlay_on_exit,
                pressure_overlay::cleanup_pressure_overlay_on_exit,
            ))
            // Crew menu toggle (while exploring)
            .add_systems(
                Update,
                (
                    toggle_crew_menu,
                    toggle_map_overlay,
                    toggle_galaxy_view,
                    crew_duty_click,
                    crew_duty_option_click,
                    sync_crew_duty_dropdowns,
                    refresh_crew_duty_labels,
                ).run_if(
                    in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked)),
                ),
            )
            // Map-click warp destination + the G warp dash (while exploring)
            .add_systems(
                Update,
                (
                    map_click_system,
                    galaxy_map_click_system,
                    map_scroll_system,
                    warp_dash_input,
                    execute_warp_dash,
                ).run_if(in_state(GameState::Exploring)),
            )
            // Build UI: ghost preview
            .add_systems(OnEnter(BuildState::Placing), build_ui::spawn_build_ghost)
            .add_systems(OnExit(BuildState::Placing), build_ui::despawn_build_ghost)
            // Build UI: delete highlight
            .add_systems(OnEnter(BuildState::Deleting), build_ui::spawn_delete_highlight)
            .add_systems(OnExit(BuildState::Deleting), build_ui::despawn_delete_highlight)
            // Build UI: info panel (spawn when entering build mode, despawn when leaving)
            .add_systems(OnExit(BuildState::Inactive), (
                build_ui::spawn_build_panel,
                build_ui::spawn_build_grid_lines,
                build_ui::spawn_module_outlines,
                build_ui::spawn_power_indicators,
            ))
            .add_systems(OnEnter(BuildState::Inactive), (
                build_ui::despawn_build_panel,
                build_ui::despawn_build_grid_lines,
                build_ui::despawn_module_outlines,
                build_ui::despawn_power_indicators,
                crate::building::build_info::despawn_build_overlays,
            ))
            // Launching leaves build mode. Nothing used to say so: BuildState
            // stayed Placing when the player flew off, so none of the teardown
            // above ever ran and the grid, the outlines, the power indicators
            // and the balance marker were all still sitting beside the station
            // afterwards.
            .add_systems(OnExit(GameState::StationDocked), (leave_build_mode, close_station_panels))
            .add_systems(Update, hide_hud_while_building)
            // Build UI: update systems
            .add_systems(
                Update,
                (
                    build_ui::update_build_ghost.run_if(in_state(BuildState::Placing)),
                    build_ui::update_delete_highlight.run_if(in_state(BuildState::Deleting)),
                    build_ui::update_build_panel.run_if(
                        in_state(BuildState::Placing)
                            .or_else(in_state(BuildState::Deleting)),
                    ),
                    build_ui::build_panel_click.run_if(
                        in_state(BuildState::Placing)
                            .or_else(in_state(BuildState::Deleting)),
                    ),
                    build_ui::scroll_item_slots.run_if(
                        in_state(BuildState::Placing)
                            .or_else(in_state(BuildState::Deleting)),
                    ),
                    build_ui::update_build_info.run_if(
                        in_state(BuildState::Placing)
                            .or_else(in_state(BuildState::Deleting)),
                    ),
                    build_ui::update_controls_help.run_if(
                        in_state(GameState::StationDocked).or_else(in_state(GameState::Exploring))
                    ),
                    build_ui::update_module_tooltip.run_if(in_state(BuildState::Placing)),
                ),
            )
            // Customization panel systems
            .add_systems(
                Update,
                (
                    build_ui::spawn_customization_panel,
                    build_ui::update_customization_panel,
                    build_ui::handle_customization_input,
                ).run_if(in_state(GameState::StationDocked)),
            )
            // Component placement panel systems
            .add_systems(
                Update,
                (
                    build_ui::spawn_component_placement_panel,
                    build_ui::handle_component_placement_input,
                    build_ui::update_component_palette_visuals,
                    build_ui::update_component_grid_visuals,
                    build_ui::update_context_menu_visuals,
                    build_ui::handle_component_placement_keyboard,
                    build_ui::show_piece_context_menu,
                    build_ui::handle_context_menu_input,
                    build_ui::spawn_piece_customization_panel,
                    build_ui::handle_piece_customization_keyboard,
                ).run_if(in_state(BuildState::PlacingComponent)),
            );
    }
}

#[derive(Component)]
/// The flight HUD's root. Public so the ending can take the whole instrument
/// panel off screen: it is telling the player the run is over, and a live fuel
/// gauge behind that argues with it.
pub struct HudRoot;

/// A clickable HUD toolbar button that stands in for a keyboard shortcut.
/// While pressed, hud_action_button_press synthesizes `key` onto the shared
/// ButtonInput<KeyCode> (same trick gamepad.rs uses), so the existing toggle/
/// action systems fire with no changes.
#[derive(Component)]
struct HudActionButton {
    key: KeyCode,
    /// Held alongside `key` for chorded shortcuts (Ctrl+Z). Synthesized the
    /// same way, so Undo can be a button instead of folklore.
    modifier: Option<KeyCode>,
}

/// One entry in the contextual action bar.
struct ToolbarAction {
    label: &'static str,
    key_disp: &'static str,
    key: KeyCode,
    modifier: Option<KeyCode>,
}

const fn act(label: &'static str, key_disp: &'static str, key: KeyCode) -> ToolbarAction {
    ToolbarAction { label, key_disp, key, modifier: None }
}

const fn act_mod(label: &'static str, key_disp: &'static str, key: KeyCode,
                 modifier: KeyCode) -> ToolbarAction {
    ToolbarAction { label, key_disp, key, modifier: Some(modifier) }
}

/// What the action bar offers right now.
///
/// Every keyboard shortcut a player would otherwise have to memorize appears
/// here as a labelled button. Only continuous inputs (move, aim, fire, brake)
/// stay keyboard-only, because a button cannot express "hold".
fn toolbar_actions(game: &GameState, build: &BuildState) -> &'static [ToolbarAction] {
    use KeyCode as K;
    const FLYING: [ToolbarAction; 11] = [
        act("Map", "M", K::KeyM),
        act("Warp", "G", K::KeyG),
        act("Systems", "N", K::KeyN),
        act("Radar", "Tab", K::Tab),
        act("Crew", "C", K::KeyC),
        act("Power", "U", K::KeyU),
        act("Log", "L", K::KeyL),
        act("Ping", "Z", K::KeyZ),
        act("Dock", "F", K::KeyF),
        act("Tow", "Y", K::KeyY),
        act("Diplomacy", "X", K::KeyX),
    ];
    // Crew belongs here as much as Hire does: the tutorial's second-to-last
    // step tells the player to manage crew while docked, and until this button
    // existed there was nothing on screen to press and the C key did nothing
    // in this state either — a dead end one step before the payoff line.
    const DOCKED: [ToolbarAction; 6] = [
        act("Build", "B", K::KeyB),
        act("Shop", "U", K::KeyU),
        act("Jobs", "J", K::KeyJ),
        act("Crew", "C", K::KeyC),
        act("Hire", "H", K::KeyH),
        act("Launch", "Enter", K::Enter),
    ];
    const PLACING: [ToolbarAction; 8] = [
        act("Category", "Tab", K::Tab),
        act("Rotate", "R", K::KeyR),
        act("Material", "M", K::KeyM),
        act("Erase", "X", K::KeyX),
        act_mod("Undo", "Ctrl+Z", K::KeyZ, K::ControlLeft),
        act("Costs", "I", K::KeyI),
        act("Power", "F2", K::F2),
        act("Done", "Esc", K::Escape),
    ];
    const DELETING: [ToolbarAction; 3] = [
        act_mod("Undo", "Ctrl+Z", K::KeyZ, K::ControlLeft),
        act("Place", "X", K::KeyX),
        act("Done", "Esc", K::Escape),
    ];
    const COMPONENT: [ToolbarAction; 2] = [
        act("Finalize", "Enter", K::Enter),
        act("Cancel", "Esc", K::Escape),
    ];

    match (game, build) {
        (GameState::Exploring, _) => &FLYING,
        (GameState::StationDocked, BuildState::Inactive) => &DOCKED,
        (GameState::StationDocked, BuildState::Placing) => &PLACING,
        (GameState::StationDocked, BuildState::Deleting) => &DELETING,
        (GameState::StationDocked, BuildState::PlacingComponent)
        | (GameState::StationDocked, BuildState::CustomizingPiece) => &COMPONENT,
        (GameState::StationDocked, _) => &DOCKED,
        _ => &[],
    }
}

/// The in-flight action toolbar (Map/Sys/Radar/Crew). Only shown while flying
/// (GameState::Exploring) — hidden when docked or in menus, since those panels
/// aren't the right controls there. See toggle_flight_toolbar_visibility.
#[derive(Component)]
struct FlightToolbar;

#[derive(Component)]
pub struct DepthText;

#[derive(Component)]
pub struct PowerText;

#[derive(Component)]
pub struct OxygenText;

#[derive(Component)]
pub struct HullText;

#[derive(Component)]
pub struct FuelText;

#[derive(Component)]
pub struct ThrusterText;

#[derive(Component)]
pub struct AmmoText;

/// Column container in the AMMO HUD slot — holds one AmmoLineText child per weapon.
/// A single Text node with embedded "\n" wasn't a reliable way to show
/// multiple weapons in a fixed-height flex row; separate stacked nodes are.
#[derive(Component)]
pub struct AmmoLinesContainer;

#[derive(Component)]
pub struct AmmoLineText;

#[derive(Component)]
pub struct NoiseText;

#[derive(Component)]
pub struct CreditsText;

#[derive(Component)]
pub struct CrewText;

#[derive(Component)]
pub struct CargoText;

/// Root of the Haven cargo-hold panel (top-left while StationDocked).
#[derive(Component)]
struct StationCargoPanel;
/// The itemized cargo list text inside the Haven cargo-hold panel.
#[derive(Component)]
struct StationCargoBodyText;
/// The "used/capacity hold" total line inside the Haven cargo-hold panel.
#[derive(Component)]
struct StationCargoTotalText;

/// Marker for a HUD bar fill element
#[derive(Component)]
pub struct HudBar {
    pub kind: HudBarKind,
}

#[derive(Clone, Copy, PartialEq)]
pub enum HudBarKind {
    Hull,
    Oxygen,
    Fuel,
    Power,
    Thrust,
}

/// Marker for the depth zone indicator
#[derive(Component)]
pub struct DepthZoneText;

/// Marker for star system info display
#[derive(Component)]
pub struct SystemInfoText;

/// Marker for gravity pull indicator
#[derive(Component)]
pub struct GravityIndicatorText;

/// Marker for map/inventory overlay
#[derive(Component)]
pub struct MapOverlay;

/// The clickable world-map square within the overlay — tagged so the click
/// handler can find it and convert cursor position to world coordinates.
#[derive(Component)]
struct MapPanel;

/// The clickable galaxy-scale map square — same idea as MapPanel, but for
/// the galaxy view. Separate marker since only one of the two is ever
/// spawned at a time (Local vs. Galaxy view) and their coordinate
/// conversions are entirely different scales.
#[derive(Component)]
struct GalaxyMapPanel;

/// Gold crosshair marking the currently selected warp destination.
#[derive(Component)]
struct PendingWarpMarker;

/// World position the player last clicked on the map — persists across
/// opening/closing the map (it's a resource, not tied to the overlay's
/// entities) so the selection sticks until they pick a new one.
#[derive(Resource, Default)]
pub struct PendingWarpTarget(pub Option<Vec2>);

/// Which panel the M-key overlay is currently showing — Local (the existing
/// current-system tactical view) or Galaxy (the new galaxy-scale starmap).
/// Always resets to Local when the overlay is freshly opened; Tab flips it
/// while open.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum MapViewMode {
    #[default]
    Local,
    Galaxy,
}

/// How far in the galaxy map is zoomed, and where it looks. Whole galaxy at 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GalaxyView {
    pub zoom: f32,
    pub center: Vec2,
}

impl Default for GalaxyView {
    fn default() -> Self {
        Self { zoom: 1.0, center: Vec2::ZERO }
    }
}

impl GalaxyView {
    /// Half the width of what's on the panel, in galaxy units.
    fn span(self) -> f32 {
        crate::celestial::galaxy::GALAXY_RADIUS / self.zoom
    }

    /// A panel position (-0.5..0.5, as `ComputedNode::normalize_point` gives
    /// it) to the galaxy position drawn there.
    fn at(self, norm: Vec2) -> Vec2 {
        self.center + Vec2::new(norm.x, -norm.y) * 2.0 * self.span()
    }

    /// Never looks past the galaxy's edge, so zooming out always ends on the
    /// whole of it.
    fn framed(self) -> Self {
        let room = (crate::celestial::galaxy::GALAXY_RADIUS - self.span()).max(0.0);
        Self { center: self.center.clamp(Vec2::splat(-room), Vec2::splat(room)), ..self }
    }

    /// Zoom by wheel notches, keeping what's under the cursor under the cursor.
    fn scrolled(self, notches: f32, norm: Vec2) -> Self {
        let zoom = (self.zoom * MAP_ZOOM_PER_NOTCH.powf(notches)).clamp(1.0, GALAXY_MAX_ZOOM);
        let anchor = self.at(norm);
        let next = Self { zoom, center: Vec2::ZERO };
        Self { center: anchor - next.at(norm), ..next }.framed()
    }

    /// Fully zoomed in on one spot: where scrolling out of a system lands.
    fn close_on(pos: Vec2) -> Self {
        Self { zoom: GALAXY_MAX_ZOOM, center: pos }.framed()
    }
}

/// How much one wheel notch zooms the galaxy map.
const MAP_ZOOM_PER_NOTCH: f32 = 1.4;
/// Past this, scrolling in leaves the galaxy for the system under the cursor.
const GALAXY_MAX_ZOOM: f32 = 6.0;
/// From here in, every system you know gets its name, not only hostile ones.
const GALAXY_NAMES_ALL_ZOOM: f32 = 2.0;
/// Pixels of trackpad scroll that make one wheel notch.
const MAP_PIXELS_PER_NOTCH: f32 = 40.0;
/// A swipe keeps sending scroll after it crosses from one map to the next.
/// Ignored for this long, in real seconds, so it doesn't cross another.
const MAP_CROSSING_HOLD: f64 = 0.35;
/// Scrolled this far out with the camera already at its limit opens the map,
/// so reaching the limit in a fight doesn't pause the game.
const FLIGHT_NOTCHES_TO_MAP: f32 = 3.0;

/// Where the scroll wheel has taken the map. Reset each time M opens it.
#[derive(Resource, Default)]
pub struct MapZoom {
    /// Looking inside a system other than the one you're in.
    pub peek: Option<u32>,
    pub galaxy: GalaxyView,
    /// Scroll toward the next crossing, in notches, signed: in is positive.
    pushed: f32,
    /// When the wheel last moved, and when it next counts (MAP_CROSSING_HOLD).
    last_scroll: f64,
    hold_until: f64,
}

/// The known system nearest the cursor, if any is on the panel: what
/// scrolling in on the galaxy map goes into. You can't look inside a system
/// you don't know is there.
fn system_to_enter(map: &crate::celestial::resources::GalaxyMap, view: GalaxyView, norm: Vec2) -> Option<u32> {
    use crate::celestial::resources::SystemDiscovery;
    let cursor = view.at(norm);
    let on_panel = |pos: Vec2| (pos - view.center).abs().max_element() <= view.span();
    map.systems.iter()
        .filter(|s| s.discovery != SystemDiscovery::Unknown && on_panel(s.galaxy_pos))
        .min_by(|a, b| a.galaxy_pos.distance(cursor).total_cmp(&b.galaxy_pos.distance(cursor)))
        .map(|s| s.id)
}

/// On the ship while a long-range warp dash is charging. Target position and
/// fuel cost are locked in at charge-start; press G again to cancel.
#[derive(Component)]
pub struct MapWarpCharging {
    pub charge_timer: Timer,
    pub target_pos: Vec2,
    pub fuel_cost: f32,
}

const WARP_DASH_FUEL_PER_1000: f32 = 1.0;
/// Spin-up before a local dash moves you at all.
///
/// Was 2.0, which was most of the cost of any short hop and made the drive
/// feel like a worse pair of engines. The local dash is a reposition inside
/// one system -- crossing to a planet, a belt, the next station -- and it
/// should feel like using a tool, not committing to something.
const WARP_DASH_BASE_CHARGE: f32 = 0.5;
/// Distance covered per second of charge. Was 15,000, so the hop out to a
/// planet at 29,000 cost about four seconds of standing still. At 80,000 the
/// same hop is under a second, and crossing the whole system is a few.
const WARP_DASH_DISTANCE_PER_SECOND: f32 = 80_000.0;
/// Ceiling on a local dash, however far across the system it goes.
///
/// Without it the drives invert at the extremes: the longest in-system hop
/// is to a station 420,000 out or more (`home_base::station_sites` starts
/// them there, and nudges some further off a planet's path), which on the
/// distance term alone charges 5.75s and up -- longer than leaving for another star
/// entirely, which starts at 4s. Crossing a room must not cost more than
/// crossing the galaxy, so the local drive is bounded rather than linear
/// forever.
const WARP_DASH_MAX_CHARGE: f32 = 3.0;
/// Stop short of the exact clicked point - avoids ever materializing inside
/// whatever's sitting there (a station, a boss hull, etc).
///
/// This used to be a flat 3000, which made the drive useless at exactly the
/// ranges you most want it: it refused any hop under 3000 AND parked you 3000
/// out on the ones it did take, so everything from ~3000 to ~6000 bought you
/// almost no ground for a two-second spin-up. Scaled now, so a short hop gets
/// a short buffer and only long jumps keep the full standoff.
const WARP_DASH_MIN_BUFFER: f32 = 800.0;
const WARP_DASH_MAX_BUFFER: f32 = 3000.0;
const WARP_DASH_BUFFER_FRACTION: f32 = 0.15;
/// Below this, fly it. The buffer alone would eat most of the distance: at
/// 1200 the minimum 800 standoff leaves 400 units of travel for a two-second
/// spin-up, which is a worse deal than the thrusters.
const WARP_DASH_MIN_DISTANCE: f32 = 2000.0;

fn warp_dash_arrival_buffer(distance: f32) -> f32 {
    (distance * WARP_DASH_BUFFER_FRACTION).clamp(WARP_DASH_MIN_BUFFER, WARP_DASH_MAX_BUFFER)
}

fn warp_dash_fuel_cost(distance: f32) -> f32 {
    (distance / 1000.0) * WARP_DASH_FUEL_PER_1000
}

fn warp_dash_charge_time(distance: f32) -> f32 {
    (WARP_DASH_BASE_CHARGE + distance / WARP_DASH_DISTANCE_PER_SECOND).min(WARP_DASH_MAX_CHARGE)
}

/// Helper to spawn a HUD bar (background + fill)
fn spawn_hud_bar(parent: &mut ChildSpawnerCommands, kind: HudBarKind, width: f32, color: Color) {
    parent.spawn((Node {
            width: Val::Px(width),
            height: Val::Px(4.0),
            ..default()
        }, BackgroundColor(Color::srgba(0.10, 0.12, 0.18, 0.8)))).with_children(|bar_bg| {
        bar_bg.spawn((
            (Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                }, BackgroundColor(color)),
            HudBar { kind },
        ));
    });
}

/// Short per-weapon label for the ammo HUD breakdown — full module names
/// ("Heavy Missile Launcher") don't fit next to a clip count.
fn ammo_hud_abbrev(module_type: ModuleType) -> &'static str {
    match module_type {
        ModuleType::Railgun => "RG",
        ModuleType::Cannon => "CN",
        ModuleType::Coilgun => "CG",
        ModuleType::Gatling => "GT",
        ModuleType::Laser => "LS",
        ModuleType::IonDisruptor => "ION",
        ModuleType::HeavyMissile => "HM",
        ModuleType::GuidedMissile => "GM",
        ModuleType::ClusterRocket => "CR",
        ModuleType::LaunchPad => "LP",
        ModuleType::RailLauncher => "RL",
        ModuleType::VlsSilo => "VLS",
        ModuleType::EMPPulse => "EMP",
        _ => "WPN",
    }
}

/// Helper to spawn a HUD group with label — uses theme colors
fn spawn_hud_group(parent: &mut ChildSpawnerCommands, label: &str, label_color: Color, children: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent.spawn((Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(2.0), Val::Px(2.0)),
            row_gap: Val::Px(1.0),
            ..default()
        })).with_children(|group| {
        group.spawn((Text::new(label), TextFont { font_size: FontSize::Px(theme::ThemeFonts::TINY), ..default() }, TextColor(label_color)));
        children(group);
    });
}

/// Helper to spawn a HUD separator
fn spawn_hud_separator(parent: &mut ChildSpawnerCommands) {
    parent.spawn((Node { width: Val::Px(1.0), height: Val::Px(28.0), ..default() }, BackgroundColor(theme::ThemeColors::HUD_SEPARATOR)));
}

/// Status colour for a 0..1 reading where LOW is bad (hull, fuel).
///
/// Neutral until it matters. ART_BRIEF's rule -- saturation is reserved for
/// meaning -- applies to the interface as much as the sprites: if every meter
/// is permanently a different bright colour, none of them can raise an alarm.
fn severity_low_bad(frac: f32) -> Color {
    use theme::ThemeColors as C;
    if frac < 0.20 {
        C::STATUS_CRITICAL
    } else if frac < 0.50 {
        C::STATUS_WARN
    } else {
        C::TEXT_PRIMARY
    }
}

/// Slow pulse for a critical reading, between the danger colour and a dim
/// version of it. The old version snapped between pure red and near-black,
/// which read as a rendering fault rather than an alarm.
fn critical_pulse(t: f32) -> Color {
    use theme::ThemeColors as C;
    let k = 0.5 + 0.5 * (t * 3.0).sin();
    C::STATUS_CRITICAL.mix(&C::BG_ELEVATED, 0.55 * (1.0 - k))
}

/// A vital meter for the redesigned top bar: a "LABEL  value" row over a thin
/// severity-colored bar. `value` spawns the value node (so the caller attaches
/// the right marker, e.g. HullText); the bar fill carries HudBar so the existing
/// update systems drive its width/color.
fn spawn_meter(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    bar_kind: HudBarKind,
    bar_color: Color,
    value: impl FnOnce(&mut ChildSpawnerCommands),
) {
    use theme::*;
    parent.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(3.0),
        min_width: Val::Px(90.0),
        ..default()
    }).with_children(|m| {
        m.spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        }).with_children(|row| {
            row.spawn((Text::new(label), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(ThemeColors::TEXT_MUTED)));
            value(row);
        });
        m.spawn((Node { width: Val::Percent(100.0), height: Val::Px(5.0), ..default() },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.11, 0.9)))).with_children(|track| {
            track.spawn((
                (Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(bar_color)),
                HudBar { kind: bar_kind },
            ));
        });
    });
}

/// A label-over-value stack for the nav / resources clusters. `value` spawns the
/// value line(s) so the caller attaches the right marker.
fn spawn_stack(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    align_end: bool,
    value: impl FnOnce(&mut ChildSpawnerCommands),
) {
    use theme::*;
    parent.spawn(Node {
        flex_direction: FlexDirection::Column,
        align_items: if align_end { AlignItems::FlexEnd } else { AlignItems::FlexStart },
        row_gap: Val::Px(1.0),
        ..default()
    }).with_children(|g| {
        g.spawn((Text::new(label), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(ThemeColors::TEXT_MUTED)));
        value(g);
    });
}

/// Clear the HUD out of the way while the player is building.
///
/// Build mode is the one screen where the ship itself is the content, and the
/// flight HUD was competing with it: the cargo readout and the expedition line
/// stacked on each other in the top-left corner, and the action toolbar
/// repeated, as buttons across the bottom, the same keys the build panel now
/// lists down the right-hand side.
fn hide_hud_while_building(
    build: Res<State<BuildState>>,
    mut cargo: Query<&mut Visibility, (With<StationCargoPanel>, Without<FlightToolbar>)>,
    mut toolbar: Query<&mut Visibility, (With<FlightToolbar>, Without<StationCargoPanel>)>,
) {
    let want = if *build.get() == BuildState::Inactive {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in cargo.iter_mut() {
        if *v != want {
            *v = want;
        }
    }
    for mut v in toolbar.iter_mut() {
        if *v != want {
            *v = want;
        }
    }
}

/// Shut the station's own windows when the ship leaves.
///
/// The hiring board and the mission board were both left standing on launch:
/// nothing closed them, and both keep running while Exploring so that a ship
/// flying within DOCK_RANGE of a berth can still use them. Open one, press
/// Enter, and it came along, fully interactive, while the station shrank
/// behind you.
///
/// This closes the windows. It does NOT change the reach rule -- pressing H or
/// J near a station is deliberate, and `SystemStations::nearest_index` already
/// limits that to DOCK_RANGE.
fn close_station_panels(
    mut commands: Commands,
    mut hiring_open: ResMut<crate::crew::hiring::HiringBoardOpen>,
    mut board_open: ResMut<crate::contracts::MissionBoardOpen>,
    hiring: Query<Entity, With<crate::crew::hiring::HiringPanel>>,
    board: Query<Entity, With<crate::contracts::ui::MissionBoardPanel>>,
) {
    hiring_open.0 = false;
    board_open.0 = false;
    for entity in hiring.iter().chain(board.iter()) {
        commands.entity(entity).despawn();
    }
}

/// Drop out of build mode, so every OnEnter(Inactive) teardown fires.
fn leave_build_mode(
    current: Res<State<BuildState>>,
    mut next: ResMut<NextState<BuildState>>,
) {
    if *current.get() != BuildState::Inactive {
        next.set(BuildState::Inactive);
    }
}

/// Sets up the UI — themed, clean layout
fn setup_ui(mut commands: Commands) {
    use theme::*;

    commands.spawn((
        (Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            }),
        // Starts hidden. `setup_ui` runs in Startup and the game opens on the
        // MainMenu, so the HUD existed and was visible behind the menu from
        // the first frame -- hull, power, fuel, credits and the flight control
        // hints, all over a title screen. Nobody saw it because the menu's
        // backdrop was 98% opaque; the moment it was thinned to let the
        // starfield through, a whole instrument panel came with it.
        // `show_hud` reveals it on entering flight or a berth.
        Visibility::Hidden,
        HudRoot,
    )).with_children(|parent| {
        // ===== TOP BAR — Ship Vitals =====
        // Credits used to also show as a separate top-right floating counter
        // (removed — same currency.credits value shown twice on screen at
        // once; the CRED group below is now the one place to look).
        parent.spawn((Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(ThemeSpacing::LG), Val::Px(ThemeSpacing::LG), Val::Px(ThemeSpacing::SM), Val::Px(ThemeSpacing::SM)),
                column_gap: Val::Px(ThemeSpacing::XS),
                align_items: AlignItems::Center,
                ..default()
            }, BackgroundColor(ThemeColors::HUD_BG_SOLID), HudTopBar)).with_children(|top_bar| {
            // Three scannable clusters: ship vitals (severity meters) · nav
            // (system / depth / noise) · resources (credits / crew / cargo).
            // The nav cluster grows to push resources to the right edge.

            // ---- CLUSTER: ship vitals ----
            top_bar.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                ..default()
            }).with_children(|c| {
                spawn_meter(c, "HULL", HudBarKind::Hull, ThemeColors::STATUS_OK, |r| {
                    r.spawn((Text::new("100%"), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::TEXT_PRIMARY), HullText));
                });
                spawn_meter(c, "PWR", HudBarKind::Power, ThemeColors::ACCENT_CYAN, |r| {
                    r.spawn((Text::new("0/0"), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::TEXT_PRIMARY), PowerText));
                });
                spawn_meter(c, "FUEL", HudBarKind::Fuel, ThemeColors::ACCENT_CYAN, |r| {
                    r.spawn((Text::new("100%"), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::TEXT_PRIMARY), FuelText));
                });
                spawn_meter(c, "THRS", HudBarKind::Thrust, ThemeColors::ACCENT_CYAN, |r| {
                    r.spawn((Text::new("0%"), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::TEXT_PRIMARY), ThrusterText));
                });
            });

            // ---- CLUSTER: navigation (grows to push resources right) ----
            top_bar.spawn((Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(22.0),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(6.0)),
                margin: UiRect::left(Val::Px(6.0)),
                border: UiRect::left(Val::Px(1.0)),
                ..default()
            }, BorderColor::all(ThemeColors::BORDER_SUBTLE))).with_children(|c| {
                spawn_stack(c, "SYS", false, |g| {
                    g.spawn((Text::new("Haven"), TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::TEXT_TITLE), SystemInfoText));
                    g.spawn((Text::new("Station Orbit"), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY), DepthZoneText));
                });
                spawn_stack(c, "HAVEN", false, |g| {
                    g.spawn((Text::new("0.0 km"), TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::TEXT_PRIMARY), DepthText));
                    g.spawn((Text::new(""), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY), GravityIndicatorText));
                });
                spawn_stack(c, "NOISE", false, |g| {
                    g.spawn((Text::new("0"), TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY), NoiseText));
                });
            });

            // ---- CLUSTER: resources (right) ----
            top_bar.spawn((Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(18.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                border: UiRect::left(Val::Px(1.0)),
                ..default()
            }, BorderColor::all(ThemeColors::BORDER_SUBTLE))).with_children(|c| {
                spawn_stack(c, "CRED", true, |g| {
                    g.spawn((Text::new("500"), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::ACCENT_YELLOW), CreditsText));
                });
                spawn_stack(c, "CREW", true, |g| {
                    g.spawn((Text::new("0/0"), TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::TEXT_PRIMARY), CrewText));
                });
                spawn_stack(c, "CARGO", true, |g| {
                    g.spawn((Text::new("0/0"), TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::ACCENT_CYAN), CargoText));
                });
            });
        });

        // ===== RIGHT COLUMN: contract tracker, then notifications =====
        // One column so layout stacks them. They used to be placed apart and
        // the notifications pushed down by the tracker's measured height,
        // which lagged and missed wrapped lines: notices kept landing on the
        // tracker's last line. The tracker joins this column when it spawns
        // (contracts::ui::spawn_contract_hud), ahead of the notifications.
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(ThemeSpacing::LG),
                top: Val::Px(RIGHT_COLUMN_TOP),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(ThemeSpacing::SM),
                max_width: Val::Px(360.0),
                ..default()
            },
            RightColumn,
            BelowTopBar,
        )).with_children(|column| {
            column.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(ThemeSpacing::SM),
                    ..default()
                },
                NotificationContainer,
            ));
        });

        // ===== BOTTOM BAR — Controls =====
        parent.spawn((Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                padding: UiRect::new(Val::Px(ThemeSpacing::XL), Val::Px(ThemeSpacing::XL), Val::Px(ThemeSpacing::SM), Val::Px(ThemeSpacing::SM)),
                align_items: AlignItems::Center,
                ..default()
            }, BackgroundColor(ThemeColors::HUD_BG_SOLID))).with_children(|bar| {
            bar.spawn((
                // Immediately overwritten every frame by build_ui::update_controls_help
                // once GameState resolves — this is just the pre-first-frame fallback.
                (Text::new("Mouse: Aim | WASD: Move | Space: Fire | B: Build | ESC: Pause"), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(ThemeColors::TEXT_MUTED)),
                build_ui::ControlsHelpText,
            ));
        });

        // Contextual action bar. Every shortcut worth knowing shows up here as
        // a labelled button for the current state, so the player never has to
        // memorize a key. Buttons synthesize their KeyCode (see
        // hud_action_button_press), so existing handlers fire unchanged.
        // Children are rebuilt by rebuild_action_toolbar on state change.
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(30.0),
                left: Val::Px(12.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            // The strip had no background at all: only the buttons did, so the
            // gaps between them were holes onto the play area and the station
            // showed through the row.
            BackgroundColor(ThemeColors::HUD_BG_SOLID),
            FlightToolbar,
        ));

        // ===== WEAPON RACK (bottom-center, flight only) =====
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                right: Val::Px(12.0),
                width: Val::Px(420.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(ThemeColors::HUD_BG_SOLID),
            BorderColor::all(ThemeColors::BORDER_DEFAULT),
            WeaponRackPanel,
        )).with_children(|rack| {
            rack.spawn((Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            }, BorderColor::all(ThemeColors::BORDER_SUBTLE))).with_children(|h| {
                h.spawn((Text::new("WEAPONS"), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY)));
            });
            rack.spawn((Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                row_gap: Val::Px(2.0),
                ..default()
            }, WeaponRackRoot));
        });
    });
}

/// Turn a HUD toolbar button press into a synthesized key press. Mirrors
/// gamepad.rs's bridge: build the set of keys whose button is currently held,
/// release ones no longer held, press the rest (idempotent). Runs in PreUpdate
/// after InputSystems so the synthesized `just_pressed` is seen by the toggle
/// systems that same frame.
fn hud_action_button_press(
    buttons: Query<(&Interaction, &HudActionButton)>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut emulated: Local<HashSet<KeyCode>>,
) {
    let mut desired: HashSet<KeyCode> = HashSet::new();
    for (interaction, btn) in buttons.iter() {
        if *interaction == Interaction::Pressed {
            desired.insert(btn.key);
            // Chorded shortcuts need the modifier held in the same frame, or
            // the handler sees a bare Z and undoes nothing.
            if let Some(m) = btn.modifier {
                desired.insert(m);
            }
        }
    }
    for &key in emulated.iter() {
        if !desired.contains(&key) {
            keyboard.release(key);
        }
    }
    for &key in desired.iter() {
        keyboard.press(key);
    }
    *emulated = desired;
}

/// Hover/press color feedback for the HUD toolbar buttons.
fn hud_action_button_hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (With<HudActionButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        *bg = theme::button_color_for_interaction(interaction).into();
    }
}

/// Show the action bar wherever it has something to offer — flying, docked,
/// and throughout build mode. Hidden only in menus and on the pause/death
/// screens, where it would sit under a fullscreen overlay.
fn toggle_flight_toolbar_visibility(
    state: Res<State<GameState>>,
    mut panels: Query<&mut Node, With<FlightToolbar>>,
) {
    if !state.is_changed() {
        return;
    }
    let show = matches!(*state.get(), GameState::Exploring | GameState::StationDocked);
    for mut node in panels.iter_mut() {
        node.display = if show { Display::Flex } else { Display::None };
    }
}

/// Rebuild the action bar's buttons whenever the relevant state changes.
///
/// The set of useful actions differs sharply between flying, sitting docked,
/// and each build sub-mode, so the bar is rebuilt rather than filtered.
fn rebuild_action_toolbar(
    mut commands: Commands,
    game_state: Res<State<GameState>>,
    build_state: Res<State<BuildState>>,
    bars: Query<(Entity, Option<&Children>), With<FlightToolbar>>,
) {
    use theme::*;
    if !game_state.is_changed() && !build_state.is_changed() {
        return;
    }
    let actions = toolbar_actions(game_state.get(), build_state.get());
    for (bar, children) in bars.iter() {
        if let Some(children) = children {
            for child in children.iter() {
                commands.entity(child).despawn();
            }
        }
        commands.entity(bar).with_children(|toolbar| {
            for a in actions {
                toolbar.spawn((
                    Node {
                        padding: UiRect::new(Val::Px(11.0), Val::Px(11.0),
                                             Val::Px(6.0), Val::Px(5.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(ThemeColors::BG_ELEVATED),
                    BorderColor::all(ThemeColors::BORDER_SUBTLE),
                    Button,
                    Interaction::default(),
                    HudActionButton { key: a.key, modifier: a.modifier },
                )).with_children(|b| {
                    b.spawn((
                        Text::new(a.label),
                        TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() },
                        TextColor(ThemeColors::TEXT_PRIMARY),
                    ));
                    // The key stays visible but recessive: available to learn,
                    // never required to act.
                    b.spawn((
                        Text::new(a.key_disp),
                        TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
                        TextColor(ThemeColors::TEXT_MUTED),
                    ));
                });
            }
        });
    }
}

/// Weapon rack visibility: shown while flying, hidden when docked, and manually
/// toggleable with K (so it can be tucked away). Only writes when the effective
/// visibility changes, to avoid dirtying the layout every frame.
fn weapon_rack_visibility(
    keyboard: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut hidden: Local<bool>,
    mut last: Local<Option<bool>>,
    mut rack: Query<&mut Node, With<WeaponRackPanel>>,
) {
    if keyboard.just_pressed(KeyCode::KeyK) {
        *hidden = !*hidden;
    }
    let show = *state.get() == GameState::Exploring && !*hidden;
    if *last != Some(show) {
        *last = Some(show);
        for mut node in rack.iter_mut() {
            node.display = if show { Display::Flex } else { Display::None };
        }
    }
}

// ===== WEAPON RACK =====

/// The bottom-center weapon rack panel — a flight HUD element gated to Exploring
/// (shares toggle_flight_toolbar_visibility with the toolbar).
#[derive(Component)]
struct WeaponRackPanel;
/// Container the per-weapon rows are spawned into.
#[derive(Component)]
struct WeaponRackRoot;
/// One rack row: every weapon of one kind in one fire group, caching its
/// dynamic children.
#[derive(Component)]
struct WeaponRackRow {
    key: (u8, &'static str),
    name: Entity,
    bar: Entity,
    ammo: Entity,
    state: Entity,
}
/// The reload-progress bar fill inside a rack row.
#[derive(Component)]
struct WeaponRackBarFill;
/// Marks the ammo/state text nodes so the update query stays narrow.
#[derive(Component)]
struct WeaponRackDynText;

fn weapon_rack_name(m: ModuleType) -> &'static str {
    match m {
        ModuleType::Railgun => "Railgun",
        ModuleType::Cannon => "Cannon",
        ModuleType::Coilgun => "Coilgun",
        ModuleType::Gatling => "Gatling",
        ModuleType::Laser => "Laser",
        ModuleType::PlasmaCaster => "Plasma Caster",
        ModuleType::IonDisruptor => "Ion Disruptor",
        ModuleType::HeavyMissile => "Heavy Missile",
        ModuleType::GuidedMissile => "Guided Missile",
        ModuleType::ClusterRocket => "Cluster Rocket",
        ModuleType::LaunchPad => "Launch Pad",
        ModuleType::RailLauncher => "Rail Launcher",
        ModuleType::VlsSilo => "VLS Silo",
        ModuleType::EMPPulse => "EMP Pulse",
        _ => "Weapon",
    }
}

/// A gun built from parts is named by its gauge. Gauge only comes in nine
/// sizes (60 mm + 55 mm per increaser, up to 500), so each has a fixed name
/// and guns of one size stack in the rack like any other weapon.
fn built_gun_rack_name(gauge_mm: f32) -> &'static str {
    // Increasers and reducers together make any bore from 20 to 500 mm, so
    // the names are made as they turn up, once each.
    use std::sync::{Mutex, OnceLock};
    static NAMES: OnceLock<Mutex<HashMap<u32, &'static str>>> = OnceLock::new();
    let mm = gauge_mm.round().clamp(0.0, 9999.0) as u32;
    *NAMES
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(mm)
        .or_insert_with(|| Box::leak(format!("{mm} mm Gun").into_boxed_str()))
}

/// What one rack row says about a stack of identical weapons.
#[derive(Debug, Clone, PartialEq)]
struct RackLine {
    name: String,
    frac: f32,
    ammo: String,
    state: String,
    state_color: Color,
}

/// One weapon as the rack sees it.
struct RackWeapon {
    frac: f32,
    ready: bool,
    remaining: f32,
    ammo: u32,
    max: u32,
    alive: bool,
    /// A turret gun, loaded, still swinging onto what it's aimed at: it
    /// holds fire until it's on.
    turning: bool,
    /// On point defence: it fires by itself at missiles, not when you do.
    auto: bool,
    /// Fed by power, not a magazine (beams carry 999 rounds as a stand-in).
    /// A magazine of 20 mm shells holds over 999 real ones.
    power: bool,
    /// On point defence with no Missile Warner left aboard: it can't see a
    /// missile to shoot.
    blind: bool,
}

/// Folds a stack of identical weapons into one line. A starter carrying four
/// cannons and four gatlings used to get eight near-identical rows; the rack
/// filled the lower right of the screen and said less than "Cannon x4 READY".
fn rack_line(name: &str, weapons: &[RackWeapon]) -> RackLine {
    use theme::ThemeColors;
    let n = weapons.len();
    let live: Vec<&RackWeapon> = weapons.iter().filter(|w| w.alive).collect();
    let label = if n > 1 { format!("{name} x{n}") } else { name.to_string() };
    if live.is_empty() {
        return RackLine { name: label, frac: 0.0, ammo: "-".into(), state: "DOWN".into(), state_color: ThemeColors::STATUS_DANGER };
    }
    let frac = live.iter().map(|w| w.frac).sum::<f32>() / live.len() as f32;
    // Beams and other power-fed guns carry 999 as a stand-in for "no
    // magazine"; printing it read as a real count.
    let ammo = if live.iter().all(|w| w.power) {
        "power".to_string()
    } else {
        format!("{}/{}", live.iter().map(|w| w.ammo).sum::<u32>(), live.iter().map(|w| w.max).sum::<u32>())
    };
    // A dry gun has finished its cooldown and still can't fire. With ammo
    // burning, READY over an empty magazine sent captains into fights with
    // nothing to shoot.
    let has_rounds = |w: &RackWeapon| w.power || w.ammo > 0;
    if !live.iter().any(|w| has_rounds(w)) {
        return RackLine { name: label, frac: 0.0, ammo, state: "EMPTY".into(), state_color: ThemeColors::STATUS_DANGER };
    }
    // Point defence isn't yours to fire: say so, rather than READY over a
    // gun that won't answer Space.
    if live.iter().all(|w| w.auto) {
        if live.iter().any(|w| w.blind) {
            return RackLine { name: label, frac: 0.0, ammo, state: "BLIND".into(), state_color: ThemeColors::STATUS_DANGER };
        }
        return RackLine { name: label, frac: 1.0, ammo, state: "AUTO PD".into(), state_color: ThemeColors::ACCENT_CYAN };
    }
    let ready = live.iter().filter(|w| w.ready && has_rounds(w) && !w.turning).count();
    let turning = live.iter().any(|w| w.ready && has_rounds(w) && w.turning);
    let (state, state_color) = if ready == live.len() {
        ("READY".to_string(), ThemeColors::ACCENT_GREEN)
    } else if ready > 0 {
        (format!("{ready}/{} RDY", live.len()), ThemeColors::ACCENT_YELLOW)
    } else if turning {
        // Loaded, but its turret hasn't come round to the target yet. Without
        // this it read READY while holding fire, and looked broken.
        ("TURNING".to_string(), ThemeColors::ACCENT_YELLOW)
    } else {
        let soonest = live.iter().filter(|w| has_rounds(w)).map(|w| w.remaining).fold(f32::INFINITY, f32::min);
        // Round up: the last tenth of a reload printed as an orange "0.0s",
        // which reads as stuck rather than about to fire.
        // (The epsilon keeps 0.3 from ceiling to 0.4 on f32 noise.)
        let shown = (soonest * 10.0 - 0.001).ceil().max(1.0) / 10.0;
        (format!("{shown:.1}s"), ThemeColors::ACCENT_ORANGE)
    };
    RackLine { name: label, frac, ammo, state, state_color }
}

/// Rebuilds/updates the weapon rack: one row per kind of weapon in each fire
/// group, with a reload bar, the group tag, combined ammo, and READY /
/// partly ready / countdown. Rows are reconciled by (group, kind), so adding
/// or losing a gun in the yard updates the rack on next entry to flight.
fn update_weapon_rack(
    rack_root: Query<Entity, With<WeaponRackRoot>>,
    ship_query: Query<(Entity, Option<&crate::combat::missile_defence::MissileWarning>), With<Ship>>,
    weapon_query: Query<(Entity, &Module, &Weapon, Option<&WeaponCooldown>, Option<&crate::combat::targeting::fire_groups::FireGroup>, &ChildOf, Option<&ModuleTemperature>)>,
    assemblies: Query<&crate::combat::gun_assembly::GunAssembly>,
    // A turret gun's barrel and where its turret is aiming.
    aiming: (Query<&crate::combat::gun_assembly::GunMuzzle>, Query<&crate::combat::turret_deck::TurretBody>),
    point_defence: Query<(), With<crate::combat::targeting::fire_groups::InterceptMode>>,
    // Lasers built from parts (combat::lasers): emitters, their banks, and
    // the turret an emitter may stand on.
    lasers: (
        Query<(Entity, &crate::combat::lasers::LaserEmitter, &Module, &ChildOf, Option<&crate::combat::targeting::fire_groups::FireGroup>)>,
        Query<&crate::combat::lasers::LaserBank>,
        Query<&ChildOf, With<crate::combat::turret_deck::TurretBody>>,
    ),
    rows: Query<(Entity, &WeaponRackRow)>,
    mut texts: Query<(&mut Text, &mut TextColor), With<WeaponRackDynText>>,
    mut bars: Query<(&mut Node, &mut BackgroundColor), With<WeaponRackBarFill>>,
    mut commands: Commands,
    mut notifications: MessageWriter<ShowNotification>,
    mut dry: Local<HashSet<(u8, &'static str)>>,
) {
    use theme::*;
    let Ok(root) = rack_root.single() else { return };
    let Ok((player, warning)) = ship_query.single() else { return };
    let blind = warning.is_some_and(|w| w.warners == 0);

    let mut stacks: std::collections::BTreeMap<(u8, &'static str), Vec<RackWeapon>> = std::collections::BTreeMap::new();
    for (entity, module, weapon, cd, fg, parent, temp) in weapon_query.iter() {
        // A turret gun's parent is its turret; its assembly knows the ship.
        let owner = assemblies.get(entity).ok().and_then(|a| a.ship).unwrap_or(parent.parent());
        if owner != player { continue; }
        // The tractor beam carries a Weapon for its reach, but it's the tow
        // tool (Y), not a gun -- it sat in the rack as a row called "Weapon".
        if module.module_type == ModuleType::TractorBeam { continue; }
        let (frac, ready, remaining) = if module.module_type == ModuleType::Laser {
            // A continuous beam fires through its own path and never runs a
            // WeaponCooldown, so the timer sat at its initial "1.0s" forever.
            // What can stop a beam is heat.
            let hot = temp.is_some_and(|t| t.current >= t.max_temp * 0.95);
            (if hot { 0.0 } else { 1.0 }, !hot, 0.0)
        } else {
            match cd {
                Some(cd) => (cd.timer.fraction(), cd.timer.is_finished(), cd.timer.remaining_secs()),
                None => (1.0, true, 0.0),
            }
        };
        stacks
            .entry((
                fg.map(|f| f.group).unwrap_or(0),
                match assemblies.get(entity) {
                    Ok(a) => built_gun_rack_name(a.stats.gauge_mm),
                    Err(_) => weapon_rack_name(module.module_type),
                },
            ))
            .or_default()
            .push(RackWeapon {
                frac, ready, remaining, ammo: weapon.ammo, max: weapon.max_ammo, alive: module.health > 0.0,
                turning: match (aiming.0.get(entity), aiming.1.get(parent.parent()).ok().and_then(|t| t.aim)) {
                    (Ok(m), Some(aim)) => m.dir.angle_to((aim - m.world).normalize_or_zero()).abs()
                        > (crate::combat::gun_assembly::SPINAL_ARC_DEG / 2.0).to_radians(),
                    _ => false,
                },
                auto: module.module_type == ModuleType::FiringPiece && point_defence.contains(entity),
                power: weapon.max_ammo >= 999 && assemblies.get(entity).is_err(),
                blind: blind && module.module_type == ModuleType::FiringPiece && point_defence.contains(entity),
            });
    }
    // Each laser emitter, reading its bank's charge: ready once a tenth of
    // the bank is in.
    for (emitter, link, module, parent, fg) in lasers.0.iter() {
        let owner = lasers.2.get(parent.parent()).map(|p| p.parent()).unwrap_or(parent.parent());
        if owner != player { continue; }
        let Ok(bank) = lasers.1.get(link.bank) else { continue };
        let fill = bank.fill();
        let ready = fill >= 0.1;
        let pumping = crate::combat::lasers::PUMP_RATE * bank.pumps.len() as f32;
        let remaining = if ready || pumping <= 0.0 { 0.0 } else { (0.1 * bank.stats.storage - bank.charge) / pumping };
        // On point defence it fires itself, at what the warners see.
        let auto = point_defence.contains(emitter);
        let name = if auto { "Laser (PD)" } else if bank.stats.shots == 0 { "Laser (beam)" } else { "Laser (pulse)" };
        stacks
            .entry((fg.map(|f| f.group).unwrap_or(0), name))
            .or_default()
            .push(RackWeapon {
                frac: fill, ready, remaining, ammo: 0, max: 999, alive: module.health > 0.0, turning: false,
                auto, power: true, blind: blind && auto,
            });
    }
    let lines: Vec<((u8, &'static str), RackLine)> =
        stacks.iter().map(|(key, ws)| (*key, rack_line(key.1, ws))).collect();

    // Say it once when a stack runs dry. The rack turns red, but mid-fight
    // nobody is reading the lower right corner -- the guns just go quiet.
    for (key, line) in &lines {
        if line.state == "EMPTY" {
            if dry.insert(*key) {
                notifications.write(ShowNotification {
                    message: format!("{} out of ammo - rearm at a station", line.name),
                    notification_type: NotificationType::Warning,
                    duration: 4.0,
                });
            }
        } else {
            dry.remove(key);
        }
    }

    // Update existing rows; despawn rows whose stack is gone.
    let mut have: HashSet<(u8, &'static str)> = HashSet::new();
    for (row_e, row) in rows.iter() {
        let Some((_, line)) = lines.iter().find(|(k, _)| *k == row.key) else {
            commands.entity(row_e).despawn();
            continue;
        };
        have.insert(row.key);
        if let Ok((mut node, mut bg)) = bars.get_mut(row.bar) {
            node.width = Val::Percent((line.frac * 100.0).clamp(0.0, 100.0));
            *bg = line.state_color.into();
        }
        for (entity, text) in [(row.name, &line.name), (row.ammo, &line.ammo)] {
            if let Ok((mut t, _)) = texts.get_mut(entity) {
                if t.0 != *text { t.0 = text.clone(); }
            }
        }
        if let Ok((mut t, mut col)) = texts.get_mut(row.state) {
            if t.0 != line.state { t.0 = line.state.clone(); }
            col.0 = line.state_color;
        }
    }
    // Spawn rows for stacks that don't have one yet.
    for (key, line) in lines.iter().filter(|(k, _)| !have.contains(k)) {
        let grp = commands.spawn((
            Text::new(format!("{}", key.0 + 1)),
            Node { min_width: Val::Px(16.0), ..default() },
            TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
            TextColor(ThemeColors::TEXT_MUTED),
        )).id();
        let name = commands.spawn((
            Text::new(line.name.clone()),
            Node { flex_grow: 1.0, ..default() },
            TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() },
            TextColor(ThemeColors::TEXT_PRIMARY),
            WeaponRackDynText,
        )).id();
        let bar_fill = commands.spawn((
            (Node { width: Val::Percent(line.frac * 100.0), height: Val::Percent(100.0), ..default() },
             BackgroundColor(line.state_color)),
            WeaponRackBarFill,
        )).id();
        let bar_track = commands.spawn((
            Node { width: Val::Px(64.0), height: Val::Px(4.0), ..default() },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.11, 0.9)),
        )).id();
        commands.entity(bar_track).add_child(bar_fill);
        let ammo = commands.spawn((
            Text::new(line.ammo.clone()),
            Node { min_width: Val::Px(56.0), ..default() },
            TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
            TextColor(ThemeColors::TEXT_SECONDARY),
            WeaponRackDynText,
        )).id();
        let state = commands.spawn((
            Text::new(line.state.clone()),
            Node { min_width: Val::Px(52.0), ..default() },
            TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
            TextColor(line.state_color),
            WeaponRackDynText,
        )).id();
        let row = commands.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                ..default()
            },
            WeaponRackRow { key: *key, name, bar: bar_fill, ammo, state },
        )).id();
        commands.entity(row).add_children(&[grp, name, bar_track, ammo, state]);
        commands.entity(root).add_child(row);
    }
}

/// Updates celestial HUD elements: system name, gravity pull, nearest star distance
pub fn update_celestial_hud(
    galaxy: Res<crate::celestial::resources::GalaxyState>,
    ship_query: Query<&Transform, With<Ship>>,
    star_query: Query<(&Transform, &crate::celestial::components::CelestialBody), With<crate::celestial::components::Star>>,
    bh_query: Query<(&Transform, &crate::celestial::components::CelestialBody), With<crate::celestial::components::BlackHole>>,
    gravity_query: Query<&crate::celestial::components::GravityForce, With<Ship>>,
    mut system_text_query: Query<(&mut Text, &mut TextColor), (With<SystemInfoText>, Without<GravityIndicatorText>)>,
    mut gravity_text_query: Query<(&mut Text, &mut TextColor), (With<GravityIndicatorText>, Without<SystemInfoText>)>,
    place: (Res<crate::celestial::resources::SystemStreamingManager>, Res<crate::celestial::resources::GalaxyMap>, Res<crate::diplomacy::FactionStanding>),
) {
    // System name, and whether it wants you dead.
    if let Ok((mut text, mut color)) = system_text_query.single_mut() {
        let (streaming, galaxy_map, standing) = &place;
        let name = crate::celestial::galaxy::system_name(galaxy.current_system);
        let hostile = streaming.loaded_system
            .and_then(|id| galaxy_map.systems.iter().find(|s| s.id == id))
            .is_some_and(|s| standing.system_hostile(s));
        text.0 = if hostile { format!("{name} - HOSTILE") } else { name };
        color.0 = if hostile { HOSTILE_TEXT } else { theme::ThemeColors::TEXT_TITLE };
    }

    let ship_pos = ship_query.single()
        .map(|t| t.translation.truncate())
        .unwrap_or(Vec2::ZERO);

    // Gravity indicator
    if let Ok((mut text, mut text_color)) = gravity_text_query.single_mut() {
        let gravity_force = gravity_query.single()
            .map(|gf| gf.0.length())
            .unwrap_or(0.0);

        if gravity_force > 10.0 {
            // Find what's pulling us
            let nearest_star = star_query.iter()
                .map(|(t, body)| (t.translation.truncate().distance(ship_pos), &body.name))
                .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let nearest_bh = bh_query.iter()
                .map(|(t, body)| (t.translation.truncate().distance(ship_pos), &body.name))
                .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let source_name = match (nearest_star, nearest_bh) {
                (Some((sd, sn)), Some((bd, bn))) => if bd < sd { bn.as_str() } else { sn.as_str() },
                (Some((_, n)), None) => n.as_str(),
                (None, Some((_, n))) => n.as_str(),
                _ => "Unknown",
            };

            // In u/s², against the starter's ~180 of thrust. These were sized
            // for the old force-over-mass scale, where nothing short of a black
            // hole ever left "Weak" -- Haven's spawn now pulls at ~31, which is
            // a real tug and should say so.
            use crate::celestial::gravity::{GRAVITY_NOTICE, GRAVITY_SEVERE};
            let intensity = if gravity_force > GRAVITY_SEVERE {
                "EXTREME"
            } else if gravity_force > GRAVITY_NOTICE {
                "Strong"
            } else if gravity_force > GRAVITY_NOTICE * 0.35 {
                "Moderate"
            } else {
                "Weak"
            };

            text.0 = format!("Grav: {} ({})", intensity, source_name);
            text_color.0 = if gravity_force > GRAVITY_SEVERE {
                Color::srgb(1.0, 0.0, 0.0)
            } else if gravity_force > GRAVITY_NOTICE {
                Color::srgb(1.0, 1.0, 0.0)
            } else {
                Color::srgb(0.8, 0.4, 0.3)
            };
        } else {
            text.0 = String::new();
        }
    }
}

/// Formats a world-unit range as kilometres. Every "depth" number in this game
/// is really the ship's radial distance from Haven Station (see
/// movement::update_depth) — it's a space game, not a submarine one, so the
/// readouts say how far out you are, in km. One decimal close in, where 100m
/// of drift still matters; whole km once you're far enough out that it doesn't.
pub fn format_range_km(units: f32) -> String {
    let km = units / 1000.0;
    if km < 10.0 { format!("{:.1} km", km) } else { format!("{:.0} km", km) }
}

/// Returns the space zone name for a given distance
// Thresholds must match world::depth_to_zone (radial distance rings)
fn depth_zone_name(depth: f32) -> &'static str {
    if depth < 600.0 {
        "Station Orbit"
    } else {
        crate::world::zone_display_name(crate::world::depth_to_zone(depth))
    }
}

/// Updates HUD text and bars
pub fn update_hud(
    depth_state: Res<DepthState>,
    power_state: Res<PowerState>,
    hull_state: Res<HullState>,
    time: Res<Time>,
    mut depth_query: Query<(&mut Text, &mut TextColor), (With<DepthText>, Without<PowerText>, Without<OxygenText>, Without<HullText>, Without<DepthZoneText>)>,
    mut depth_zone_query: Query<&mut Text, (With<DepthZoneText>, Without<DepthText>, Without<PowerText>, Without<OxygenText>, Without<HullText>)>,
    mut power_query: Query<(&mut Text, &mut TextColor), (With<PowerText>, Without<DepthText>, Without<OxygenText>, Without<HullText>, Without<DepthZoneText>)>,
    mut hull_query: Query<(&mut Text, &mut TextColor), (With<HullText>, Without<DepthText>, Without<PowerText>, Without<OxygenText>, Without<DepthZoneText>)>,
    mut bar_query: Query<(&HudBar, &mut Node, &mut BackgroundColor)>,
    local: (Res<crate::world::home_base::SystemStations>, Query<&Transform, With<Ship>>),
) {
    use theme::*;
    let (stations, ship) = local;
    // Range from Haven Station
    if let Ok((mut text, mut text_color)) = depth_query.single_mut() {
        text.0 = format_range_km(depth_state.current_depth);
        text_color.0 = if depth_state.current_depth > 1000.0 {
            Color::srgb(1.0, 0.4, 0.4)
        } else if depth_state.current_depth > 500.0 {
            Color::srgb(0.7, 0.7, 1.0)
        } else {
            Color::WHITE
        };
    }
    if let Ok(mut text) = depth_zone_query.single_mut() {
        // From the nearest station in this system, not from Haven.
        let range = ship
            .single()
            .ok()
            .and_then(|t| stations.local_range(t.translation.truncate()))
            .unwrap_or(depth_state.current_depth);
        let name = depth_zone_name(range);
        if text.0 != name {
            text.0 = name.to_string();
        }
    }

    // Power
    if let Ok((mut text, mut text_color)) = power_query.single_mut() {
        let gen = power_state.total_power_generation;
        let con = power_state.total_power_consumption;
        // Used / available, in that order. Printing generation first read as
        // "1000 out of 484" -- nonsense against the universal X/Y convention,
        // and backwards from the bar below it, which fills with con/gen.
        text.0 = format!("{:.0}/{:.0}", con, gen);
        text_color.0 = if power_state.power_balance < 0.0 {
            critical_pulse(time.elapsed_secs())
        } else {
            ThemeColors::TEXT_PRIMARY
        };
    }

    // Hull
    let hull_pct = hull_state.hull_integrity;
    let hull_pct_i = (hull_pct * 100.0) as i32;
    if let Ok((mut text, mut text_color)) = hull_query.single_mut() {
        text.0 = format!("{}%", hull_pct_i);
        text_color.0 = if hull_pct_i < 20 {
            critical_pulse(time.elapsed_secs())
        } else {
            severity_low_bad(hull_pct)
        };
    }

    // Update HUD bars
    for (bar, mut style, mut bg) in bar_query.iter_mut() {
        let (pct, color) = match bar.kind {
            HudBarKind::Hull => {
                let c = if hull_pct < 0.3 {
                    ThemeColors::STATUS_DANGER
                } else if hull_pct < 0.6 {
                    ThemeColors::STATUS_WARN
                } else {
                    ThemeColors::STATUS_OK
                };
                (hull_pct, c)
            }
            HudBarKind::Power => {
                // Fill = consumption as a fraction of generation (how much of
                // the budget is used); red when in deficit, yellow otherwise.
                let gen = power_state.total_power_generation.max(0.001);
                let frac = (power_state.total_power_consumption / gen).clamp(0.0, 1.0);
                let c = if power_state.power_balance < 0.0 {
                    ThemeColors::STATUS_DANGER
                } else {
                    ThemeColors::ACCENT_CYAN
                };
                (frac, c)
            }
            // Oxygen bar removed with crew O2 (its HUD group no longer spawns)
            HudBarKind::Oxygen => continue,
            HudBarKind::Fuel | HudBarKind::Thrust => continue, // handled in update_hud_secondary
        };
        style.width = Val::Percent(pct * 100.0);
        *bg = color.into();
    }
}

/// Updates secondary HUD elements: Fuel, Thrusters, Ammo, Noise, Credits, Crew
pub fn update_hud_secondary(
    fuel_state: Res<FuelState>,
    noise_state: Res<NoiseState>,
    currency: Res<Currency>,
    staffing_state: Res<StaffingState>,
    time: Res<Time>,
    ship_query: Query<(Entity, &ShipPhysics), With<Ship>>,
    weapon_query: Query<(&Weapon, &Module, &ChildOf, Option<&crate::combat::gun_assembly::GunAssembly>)>,
    mut fuel_query: Query<(&mut Text, &mut TextColor), (With<FuelText>, Without<ThrusterText>, Without<AmmoText>, Without<NoiseText>, Without<CreditsText>, Without<CrewText>)>,
    mut thruster_text_query: Query<(&mut Text, &mut TextColor), (With<ThrusterText>, Without<FuelText>, Without<AmmoText>, Without<NoiseText>, Without<CreditsText>, Without<CrewText>)>,
    mut ammo_ui: (
        Query<(Entity, Option<&Children>), With<AmmoLinesContainer>>,
        Query<&AmmoLineText>,
        Commands,
        Local<Vec<(u32, u32)>>,
    ),
    mut noise_query: Query<(&mut Text, &mut TextColor), (With<NoiseText>, Without<FuelText>, Without<ThrusterText>, Without<AmmoText>, Without<CreditsText>, Without<CrewText>)>,
    mut credits_query: Query<&mut Text, (With<CreditsText>, Without<FuelText>, Without<ThrusterText>, Without<AmmoText>, Without<NoiseText>, Without<CrewText>)>,
    mut crew_query_hud: Query<(&mut Text, &mut TextColor), (With<CrewText>, Without<FuelText>, Without<ThrusterText>, Without<AmmoText>, Without<NoiseText>, Without<CreditsText>)>,
    mut bar_query: Query<(&HudBar, &mut Node, &mut BackgroundColor)>,
    // Bundled into one tuple param — this system is already at Bevy's
    // 16-param ceiling (see the ammo_ui tuple above for the same reason).
    mut cargo_ui: (Res<Inventory>, Query<(&mut Text, &mut TextColor), (With<CargoText>, Without<FuelText>, Without<ThrusterText>, Without<AmmoText>, Without<NoiseText>, Without<CreditsText>, Without<CrewText>)>),
) {
    use theme::*;
    let (ammo_container_query, ammo_line_query, mut commands, mut last_ammo_snapshot) = ammo_ui;
    let (inventory, mut cargo_query) = cargo_ui;
    let Ok((player_ship, physics)) = ship_query.single() else { return };

    // Fuel
    let fuel_pct = if fuel_state.max_fuel > 0.0 {
        fuel_state.current_fuel / fuel_state.max_fuel
    } else { 1.0 };
    let fuel_pct_i = (fuel_pct * 100.0) as i32;
    if let Ok((mut text, mut text_color)) = fuel_query.single_mut() {
        text.0 = format!("{}%", fuel_pct_i);
        text_color.0 = if fuel_pct_i < 15 {
            critical_pulse(time.elapsed_secs())
        } else {
            severity_low_bad(fuel_pct_i as f32 / 100.0)
        };
    }

    // Main-drive throttle for the THRS meter + text. This used to read the
    // Q/E vertical thrusters, which are gone — so it sat at 0% (or "N/A") all
    // game. Now it's the W/S throttle the ship actually flies on: -1 full
    // reverse .. +1 full ahead, eased by ship_movement.
    let throttle = physics.throttle.clamp(-1.0, 1.0);
    let thrust_avg = throttle.abs();

    // Update fuel + thrust meter bars
    for (bar, mut style, mut bg) in bar_query.iter_mut() {
        match bar.kind {
            HudBarKind::Fuel => {
                style.width = Val::Percent(fuel_pct * 100.0);
                *bg = if fuel_pct < 0.25 { Color::srgb(1.0, 0.0, 0.0) } else { Color::srgb(1.0, 0.6, 0.2) }.into();
            }
            HudBarKind::Thrust => {
                style.width = Val::Percent((thrust_avg * 100.0).clamp(0.0, 100.0));
                // Amber while backing down, so reverse reads at a glance.
                *bg = if throttle < -0.02 { Color::srgb(1.0, 0.6, 0.2) } else { Color::srgb(0.3, 0.5, 1.0) }.into();
            }
            _ => {}
        }
    }

    // Throttle
    if let Ok((mut text, mut text_color)) = thruster_text_query.single_mut() {
        let pct = (thrust_avg * 100.0).round() as i32;
        if throttle < -0.02 {
            text.0 = format!("{}% REV", pct);
            text_color.0 = ThemeColors::ACCENT_ORANGE;
        } else {
            text.0 = format!("{}%", pct);
            text_color.0 = ThemeColors::TEXT_PRIMARY;
        }
    }

    // Ammo — one line per weapon on the player's own ship, not a
    // world-wide total (this used to sum every ship's weapons, player and
    // AI alike, into one misleading number). Rendered as separate stacked
    // UI nodes rather than "\n" inside one Text — a single Text node in
    // this fixed-height top-bar row wasn't reliably showing more than the
    // first line for a 7-weapon loadout.
    if let Ok((container, children)) = ammo_container_query.single() {
        let mut entries: Vec<(ModuleType, u32, u32)> = Vec::new();
        for (weapon, module, parent, gun) in weapon_query.iter() {
            if gun.and_then(|g| g.ship).unwrap_or(parent.parent()) != player_ship { continue; }
            entries.push((module.module_type, weapon.ammo, weapon.max_ammo));
        }
        let snapshot: Vec<(u32, u32)> = entries.iter().map(|(_, a, m)| (*a, *m)).collect();

        if snapshot != *last_ammo_snapshot {
            *last_ammo_snapshot = snapshot;

            // Clear old lines
            if let Some(children) = children {
                for child in children.iter() {
                    if ammo_line_query.get(child).is_ok() {
                        commands.entity(child).despawn();
                    }
                }
            }

            if entries.is_empty() {
                commands.entity(container).with_children(|c| {
                    c.spawn((
                        (Text::new("N/A"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(Color::srgb(0.5, 0.5, 0.5))),
                        AmmoLineText,
                    ));
                });
            } else {
                commands.entity(container).with_children(|c| {
                    for (module_type, ammo, max_ammo) in entries {
                        let pct = if max_ammo > 0 { ammo as f32 / max_ammo as f32 } else { 1.0 };
                        let color = if pct < 0.3 { Color::srgb(1.0, 0.0, 0.0) } else { Color::srgb(0.9, 0.7, 0.3) };
                        c.spawn((
                            (Text::new(format!("{} {}/{}", ammo_hud_abbrev(module_type), ammo, max_ammo)),
                                TextFont { font_size: FontSize::Px(theme::ThemeFonts::TINY), ..default() }, TextColor(color)),
                            AmmoLineText,
                        ));
                    }
                });
            }
        }
    }

    // Noise
    if let Ok((mut text, mut text_color)) = noise_query.single_mut() {
        let noise = noise_state.noise_level as i32;
        text.0 = format!("{}", noise);
        text_color.0 = if noise > 80 {
            Color::srgb(1.0, 0.0, 0.0)
        } else if noise > 50 {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgb(0.5, 0.5, 0.5)
        };
    }

    // Credits
    if let Ok(mut text) = credits_query.single_mut() {
        text.0 = format!("{}", currency.credits);
    }

    // Crew staffing
    if let Ok((mut text, mut text_color)) = crew_query_hud.single_mut() {
        text.0 = format!("{}/{}", staffing_state.total_crew, staffing_state.total_berths);
        text_color.0 = if staffing_state.total_crew > staffing_state.total_berths {
            Color::srgb(1.0, 0.0, 0.0)
        } else {
            Color::srgb(0.7, 0.9, 0.7)
        };
    }

    // Cargo hold — weight-based (matches Inventory.max_capacity, not an
    // item count), the same numbers the Map overlay's Inventory section and
    // sell/buy pricing already use.
    if let Ok((mut text, mut text_color)) = cargo_query.single_mut() {
        text.0 = format!("{:.0}/{:.0}", inventory.current_weight, inventory.max_capacity);
        text_color.0 = if inventory.current_weight >= inventory.max_capacity {
            Color::srgb(1.0, 0.0, 0.0)
        } else if inventory.max_capacity > 0.0 && inventory.current_weight / inventory.max_capacity > 0.85 {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgb(0.6, 0.8, 1.0)
        };
    }
}

/// Maximum number of notifications visible at once
const MAX_NOTIFICATIONS: usize = 6;
/// Minimum seconds between duplicate notifications
const NOTIFICATION_DEDUP_SECS: f32 = 3.0;

/// Spawns toast notifications from events, with deduplication and cap
fn handle_notifications(
    mut commands: Commands,
    mut notification_events: MessageReader<ShowNotification>,
    container_query: Query<(Entity, Option<&Children>), With<NotificationContainer>>,
    existing_toasts: Query<(), With<NotificationToast>>,
    mut recent_messages: Local<Vec<(String, f32)>>,
    time: Res<Time>,
) {
    let Ok((container, children)) = container_query.single() else { return };

    // Clean up expired dedup entries
    let now = time.elapsed_secs();
    recent_messages.retain(|(_, t)| now - *t < NOTIFICATION_DEDUP_SECS);

    // Live toasts, oldest first (the column's child order). Taking
    // `existing_toasts.iter().next()` per event evicted the SAME toast over
    // and over in a same-frame burst -- one missile's worth of "destroyed!"
    // notices -- so the stack ran on past the cap down the screen.
    let mut live: std::collections::VecDeque<Entity> = children
        .map(|c| c.iter().filter(|&e| existing_toasts.contains(e)).collect())
        .unwrap_or_default();

    for event in notification_events.read() {
        // Skip duplicate messages within the dedup window
        if recent_messages.iter().any(|(msg, _)| msg == &event.message) {
            continue;
        }

        // Cap max visible notifications - remove oldest if at limit
        while live.len() >= MAX_NOTIFICATIONS {
            if let Some(oldest) = live.pop_front() {
                commands.entity(oldest).despawn();
            }
        }

        let (color, bg_color, prefix) = match event.notification_type {
            NotificationType::Danger => (
                theme::ThemeColors::STATUS_DANGER,
                theme::ThemeColors::NOTIF_DANGER_BG,
                "! ",
            ),
            NotificationType::Warning => (
                theme::ThemeColors::STATUS_WARN,
                theme::ThemeColors::NOTIF_WARN_BG,
                "* ",
            ),
            NotificationType::Success => (
                theme::ThemeColors::ACCENT_GREEN,
                theme::ThemeColors::NOTIF_SUCCESS_BG,
                "+ ",
            ),
            NotificationType::Info => (
                theme::ThemeColors::TEXT_PRIMARY,
                theme::ThemeColors::NOTIF_INFO_BG,
                "",
            ),
        };
        let msg = format!("{}{}", prefix, event.message);
        let toast = commands.spawn((
            Text::new(&msg), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(color), Node { margin: UiRect::bottom(Val::Px(theme::ThemeSpacing::XS)),
                padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(5.0), Val::Px(5.0)),
                max_width: Val::Px(340.0),
                ..default() }, BackgroundColor(bg_color),
            NotificationToast { timer: Timer::from_seconds(event.duration, TimerMode::Once) },
            ToastBackground(bg_color),
        )).insert(ChildOf(container)).id();

        recent_messages.push((event.message.clone(), now));
        live.push_back(toast);
    }
}

/// A toast's own background colour, so the fade can scale it from full.
#[derive(Component)]
struct ToastBackground(Color);

/// Fades and despawns notification toasts
fn update_notifications(
    mut commands: Commands,
    time: Res<Time>,
    mut toast_query: Query<(Entity, &mut NotificationToast, &mut TextColor, Option<(&mut BackgroundColor, &ToastBackground)>)>,
) {
    for (entity, mut toast, mut text_color, background) in toast_query.iter_mut() {
        toast.timer.tick(time.delta());
        let remaining = toast.timer.remaining_secs() / toast.timer.duration().as_secs_f32();
        if remaining < 0.3 {
            let alpha = remaining / 0.3;
            text_color.0.set_alpha(alpha);
            // The slab fades with its text; it used to stay solid and the
            // last moments of every notice were an empty coloured box.
            if let Some((mut bg, base)) = background {
                bg.0 = base.0.with_alpha(base.0.alpha() * alpha);
            }
        }
        if toast.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

// ============================================================================
// HAVEN CARGO-HOLD PANEL
// A small itemized cargo list shown top-left while docked at the home station,
// where the trade menu (remote outposts) and the flying-only M inventory
// overlay both leave the hold unreadable. Single Text nodes updated per frame,
// same as the HUD readouts.
// ============================================================================

fn spawn_station_cargo_panel(mut commands: Commands) {
    use theme::*;
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ThemeSpacing::LG),
                // Below the EXPEDITION RECORDS tracker (narrative::expedition, top 88),
                // which this used to sit on top of at every berth.
                top: Val::Px(128.0),
                width: Val::Px(200.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(ThemeSpacing::XS),
                padding: UiRect::all(Val::Px(ThemeSpacing::MD)),
                ..default()
            },
            BackgroundColor(ThemeColors::BG_PANEL),
            ZIndex(40),
            StationCargoPanel,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("CARGO HOLD"),
                TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                TextColor(ThemeColors::TEXT_MUTED),
            ));
            panel.spawn((
                Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(ThemeSpacing::XS)), ..default() },
                BackgroundColor(ThemeColors::BORDER_SUBTLE),
            ));
            panel.spawn((
                Text::new("- empty -"),
                TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() },
                TextColor(ThemeColors::TEXT_SECONDARY),
                StationCargoBodyText,
            ));
            panel.spawn((
                Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(ThemeSpacing::XS)), ..default() },
                BackgroundColor(ThemeColors::BORDER_SUBTLE),
            ));
            panel.spawn((
                Text::new("0/0 hold"),
                TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                TextColor(ThemeColors::ACCENT_BLUE),
                StationCargoTotalText,
            ));
            // Salvage only becomes money at a station, and nothing anywhere
            // said so: crew haul loot aboard, the hold fills up, and the one
            // place it converts to credits is a row part-way down the services
            // list. Name it here, next to the cargo it applies to.
            panel.spawn((
                Text::new("Sell in station services (Sell Cargo)"),
                TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
                TextColor(ThemeColors::TEXT_MUTED),
            ));
        });
}

fn update_station_cargo_panel(
    inventory: Res<Inventory>,
    mut body_q: Query<&mut Text, (With<StationCargoBodyText>, Without<StationCargoTotalText>)>,
    mut total_q: Query<(&mut Text, &mut TextColor), (With<StationCargoTotalText>, Without<StationCargoBodyText>)>,
) {
    // Rebuilt every frame (cheap — a handful of items on a cold station
    // screen); the `!=` guards below keep it from touching the Text unless the
    // hold actually changed. Sorted so the list is stable frame-to-frame
    // (HashMap iteration order is not).
    let mut lines: Vec<String> = inventory
        .items
        .iter()
        .filter(|(_, &count)| count > 0)
        .map(|(item, count)| format!("{}  x{}", item.name(), count))
        .collect();
    lines.sort();
    let body = if lines.is_empty() { "- empty -".to_string() } else { lines.join("\n") };

    if let Ok(mut text) = body_q.single_mut() {
        if text.0 != body {
            text.0 = body;
        }
    }
    if let Ok((mut text, mut color)) = total_q.single_mut() {
        text.0 = format!("{:.0}/{:.0} hold", inventory.current_weight, inventory.max_capacity);
        color.0 = if inventory.max_capacity > 0.0 && inventory.current_weight >= inventory.max_capacity {
            theme::ThemeColors::STATUS_DANGER
        } else {
            theme::ThemeColors::ACCENT_BLUE
        };
    }
}

fn despawn_station_cargo_panel(
    mut commands: Commands,
    query: Query<Entity, With<StationCargoPanel>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Handles menu input
fn handle_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<GameState>>,
    build_state: Res<State<BuildState>>,
    customization_state: Res<CustomizationState>,
    // Grouped to stay under Bevy's 16-param cap: every board whose own Enter
    // must not also launch the ship.
    boards: (Res<crate::contracts::MissionBoardOpen>, Res<crate::crew::hiring::HiringBoardOpen>),
    mut next_state: ResMut<NextState<GameState>>,
    mut pre_pause: ResMut<PrePauseState>,
    mut load_events: MessageWriter<LoadGameRequest>,
    mut tutorial: ResMut<crate::tutorial::Tutorial>,
    mut new_expedition: MessageWriter<NewExpeditionRequest>,
    mut settings_menu: ResMut<menu_buttons::SettingsMenu>,
    mut commands: Commands,
    overlays: (Query<Entity, With<ModulePanelOverlay>>, Query<Entity, With<DockingOverlay>>),
    floating_windows: Query<(Entity, &windows::framework::FloatingWindow)>,
) {
    let (mission_board_open, hiring_board_open) = boards;
    let (module_panel, services) = overlays;
    // Settings overlay is modal: while it's open, Escape closes it (and nothing
    // else) so it never falls through to resume/close the underlying menu.
    if settings_menu.open {
        if keyboard.just_pressed(KeyCode::Escape) {
            settings_menu.open = false;
        }
        return;
    }

    // The deep customization window has no toggle key, so Escape closes it here
    // (complementing its × button) before Escape can fall through to the pause
    // menu. Its floating-window id is prefixed "deep_" (see customization.rs).
    if keyboard.just_pressed(KeyCode::Escape) {
        let mut closed_any = false;
        for (entity, window) in floating_windows.iter() {
            if window.id.starts_with("deep_") {
                commands.entity(entity).despawn();
                closed_any = true;
            }
        }
        if closed_any {
            return;
        }
    }

    // Station services close on Escape like any menu, rather than the press
    // falling through and pausing the game behind them.
    if keyboard.just_pressed(KeyCode::Escape)
        && *current_state.get() == GameState::StationDocked
        && !services.is_empty()
    {
        for entity in &services {
            commands.entity(entity).despawn();
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        match current_state.get() {
            // While build mode is active, Escape backs out of build layers
            // (paste → selection → build mode, see clipboard_input) instead
            // of opening the pause menu.
            GameState::StationDocked if *build_state.get() != BuildState::Inactive => {}
            GameState::Exploring | GameState::StationDocked => {
                pre_pause.0 = Some(*current_state.get());
                next_state.set(GameState::Paused);
            }
            GameState::Docked => {
                next_state.set(GameState::Exploring);
            }
            GameState::Paused => {
                let target = pre_pause.0.unwrap_or(GameState::Exploring);
                next_state.set(target);
            }
            _ => {}
        }
    }

    // Load from main menu: L+1/2/3/0
    if *current_state.get() == GameState::MainMenu && keyboard.pressed(KeyCode::KeyL) {
        if keyboard.just_pressed(KeyCode::Digit1) {
            load_events.write(LoadGameRequest { slot: 0 });
        }
        if keyboard.just_pressed(KeyCode::Digit2) {
            load_events.write(LoadGameRequest { slot: 1 });
        }
        if keyboard.just_pressed(KeyCode::Digit3) {
            load_events.write(LoadGameRequest { slot: 2 });
        }
        if keyboard.just_pressed(KeyCode::Digit0) {
            load_events.write(LoadGameRequest { slot: 99 });
        }
    }

    // Don't process Enter for state transitions while module panel, building,
    // customizing, or the mission board is active — the mission board also
    // binds Enter to "accept contract", and without this guard accepting a
    // contract simultaneously launched the ship out of the station.
    let is_building = *build_state.get() != BuildState::Inactive;
    let is_customizing = customization_state.active;

    // The hiring board and station services bind Enter too (hire / buy), and
    // neither was in this list: hiring a hand at a station launched the ship.
    if keyboard.just_pressed(KeyCode::Enter)
        && module_panel.is_empty()
        && !is_building
        && !is_customizing
        && !mission_board_open.0
        && !hiring_board_open.0
        && services.is_empty()
    {
        match current_state.get() {
            GameState::MainMenu => {
                // New expedition (not a load) — arm the guided tutorial and
                // wipe the previous run. Both of those must also happen on the
                // button path; see menu_buttons::menu_button_dispatch.
                tutorial.begin();
                new_expedition.write(NewExpeditionRequest);
                next_state.set(GameState::StationDocked);
            }
            GameState::StationDocked => next_state.set(GameState::Exploring),
            _ => {}
        }
    }
}

// ============================================================================
// GAME EVENT NOTIFICATIONS
// ============================================================================

/// Damage a crew member must take before it's worth a notice.
const HURT_NOTICE_MIN: f32 = 5.0;
/// Shortest gap between two hurt notices for the same person.
const HURT_NOTICE_EVERY: f32 = 8.0;

/// Reads from currently-silent events and sends ShowNotification
fn handle_game_event_notifications(
    mut power_events: MessageReader<PowerStateChanged>,
    mut oxygen_events: MessageReader<OxygenStateChanged>,
    mut breach_events: MessageReader<HullBreached>,
    mut crew_damage_events: MessageReader<CrewDamaged>,
    // Our crew only. AI ships' crew raise CrewDamaged too, and a raider
    // venting its own compartments read out as "Wren taking damage!" --
    // names nobody aboard has, about people the player was shooting at.
    crew_query: Query<&CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
    weapon_query: Query<&Weapon>,
    mut notifications: MessageWriter<ShowNotification>,
    mut low_ammo_warned: Local<bool>,
    time: Res<Time>,
    // Per crew member: damage not yet reported, and when they last got a notice.
    mut hurt_ledger: Local<HashMap<Entity, (f32, f32)>>,
) {
    // Power state changes
    for event in power_events.read() {
        if event.is_critical {
            notifications.write(ShowNotification {
                message: "WARNING: Power deficit! Systems failing!".into(),
                notification_type: NotificationType::Danger,
                duration: 4.0,
            });
        } else {
            notifications.write(ShowNotification {
                message: "Power restored. Systems nominal.".into(),
                notification_type: NotificationType::Success,
                duration: 3.0,
            });
        }
    }

    // Hull breaches
    // One notice per hit, not per plate: a single impact can open several
    // segments in the same frame, and each used to get its own banner.
    let (mut breaches, mut worst) = (0usize, 0.0f32);
    for event in breach_events.read() {
        breaches += 1;
        worst = worst.max(event.severity);
    }
    if breaches > 0 {
        let count = if breaches > 1 { format!(" x{breaches}") } else { String::new() };
        notifications.write(ShowNotification {
            message: format!("HULL BREACH{count}! Decompression in progress! (Severity: {:.0}%)", worst * 100.0),
            notification_type: NotificationType::Danger,
            duration: 4.0,
        });
    }

    // Crew damage
    let now = time.elapsed_secs();
    for event in crew_damage_events.read() {
        if let Ok(crew) = crew_query.get(event.crew) {
            // A fatal wound gets the death notice from report_crew_deaths;
            // "taking damage, -100" on top of it undersold what happened.
            if crew.health <= 0.0 {
                hurt_ledger.remove(&event.crew);
                continue;
            }
            // Slow harm (thin air, smoke) arrives as a sliver every frame.
            // Reported per event that was "hurt - asphyxiation (-0)" over two
            // thousand times in one run. Add it up, and say so once it's real
            // and not more than every few seconds per person.
            let entry = hurt_ledger.entry(event.crew).or_insert((0.0, f32::NEG_INFINITY));
            entry.0 += event.amount;
            if entry.0 < HURT_NOTICE_MIN || now - entry.1 < HURT_NOTICE_EVERY {
                continue;
            }
            let amount = entry.0;
            *entry = (0.0, now);
            notifications.write(ShowNotification {
                message: format!("{} hurt - {} (-{:.0})", crew.name, event.source.describe(), amount),
                notification_type: NotificationType::Warning,
                duration: 2.5,
            });
        }
    }

    // Low ammo warning (Phase 3.2)
    let any_low_ammo = weapon_query.iter().any(|w| {
        w.max_ammo > 0 && w.ammo > 0 && (w.ammo as f32) < (w.max_ammo as f32) * 0.25
    });
    if any_low_ammo && !*low_ammo_warned {
        *low_ammo_warned = true;
        notifications.write(ShowNotification {
            message: "Low ammo! Weapons below 25% capacity.".into(),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
    } else if !any_low_ammo {
        *low_ammo_warned = false;
    }
}

// ============================================================================
// CREW MANAGEMENT MENU (C key)
// ============================================================================

/// Toggles crew management overlay with C key
fn toggle_crew_menu(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    existing_menu: Query<(Entity, &windows::framework::FloatingWindow)>,
    // Without<OwnedByAiShip>: player-only — AI ships carry CrewMember/
    // CrewStation now too (see ai_ship::crew), unscoped this menu would show
    // AI crew mixed into the player's own crew roster.
    crew_query: Query<(Entity, &CrewMember, Option<&CrewDuty>), Without<crate::ai_ship::components::OwnedByAiShip>>,
    station_query: Query<(&CrewStation, &Module), Without<crate::ai_ship::components::OwnedByAiShip>>,
    // What each hand is actually doing. `CrewMember::state` cannot answer
    // this: `walking` deliberately never writes to it and `CrewState::Working`
    // is assigned nowhere in the codebase, so it reads `Idle` for almost
    // everyone no matter what they are up to. The components ARE the truth.
    activity: Query<
        (
            Entity,
            Has<crate::crew::walking::CrewPath>,
            Has<crate::crew::walking::SeekingTreatment>,
            Has<crate::crew::burial::BurialDetail>,
            Has<crate::crew::eva_salvage::EvaSalvaging>,
        ),
        With<CrewMember>,
    >,
    staffing_state: Res<StaffingState>,
    mut open_dropdown: ResMut<OpenDutyDropdown>,
) {
    // Ctrl/Cmd+C is the build clipboard's copy; it used to pop the roster
    // open over the yard every time a selection was copied.
    let modifier = keyboard.any_pressed([
        KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight,
    ]);
    if !keyboard.just_pressed(KeyCode::KeyC) || modifier {
        return;
    }

    const WINDOW_ID: &str = "crew_menu";

    // Toggle off if already open — despawn the window ROOT (found via
    // FloatingWindow.id), not the content entity spawn_floating_window
    // returns, so the title bar/border don't linger orphaned on screen.
    if let Some((entity, _)) = existing_menu.iter().find(|(_, w)| w.id == WINDOW_ID) {
        commands.entity(entity).despawn();
        open_dropdown.0 = None;
        return;
    }
    // Opening fresh: never inherit an order list left open from last time.
    open_dropdown.0 = None;

    let doing: std::collections::HashMap<Entity, (bool, bool, bool, bool)> = activity
        .iter()
        .map(|(e, path, care, burial, eva)| (e, (path, care, burial, eva)))
        .collect();

    // Build a map: crew entity -> the post they hold. Named, not numbered:
    // "On station - Railgun" tells you something, "-> (7,2)" does not.
    let mut crew_assignments: std::collections::HashMap<Entity, ModuleType> = std::collections::HashMap::new();
    for (cs, module) in station_query.iter() {
        if let Some(crew_entity) = cs.assigned_crew {
            crew_assignments.insert(crew_entity, module.module_type);
        }
    }

    let content = windows::framework::spawn_floating_window(
        &mut commands,
        WINDOW_ID,
        "Crew Management",
        Vec2::new(460.0, 340.0),
        Vec2::new(10.0, 60.0),
    );

    commands.entity(content).with_children(|parent| {
        parent.spawn((
            Text::new(format!("{}/{} berths - {}/{} stations staffed",
                staffing_state.total_crew, staffing_state.total_berths,
                staffing_state.staffed_stations, staffing_state.total_stations)),
            TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() },
            TextColor(theme::ThemeColors::TEXT_TITLE),
            Node { margin: UiRect::bottom(Val::Px(theme::ThemeSpacing::SM)), ..default() },
        ));
    });
    theme::spawn_divider(&mut commands, content);

    commands.entity(content).with_children(|parent| {
        parent.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(theme::ThemeSpacing::SM),
            ..default()
        }).with_children(|list| {
            for (entity, crew, duty) in crew_query.iter() {
                let duty = duty.copied().unwrap_or_default();
                let (walking, to_medbay, burial, eva) = doing
                    .get(&entity)
                    .copied()
                    .unwrap_or((false, false, false, false));

                let (status, dot_color) = if crew.health <= 0.0 {
                    ("Dead".to_string(), theme::ThemeColors::STATUS_DANGER)
                } else if crew.state == CrewState::Panicking {
                    ("Panicking".to_string(), theme::ThemeColors::STATUS_WARN)
                } else if eva {
                    ("Outside - salvaging".to_string(), theme::ThemeColors::STATUS_WARN)
                } else if burial {
                    ("Burial detail".to_string(), theme::ThemeColors::STATUS_WARN)
                } else if to_medbay {
                    ("Wounded - to the med bay".to_string(), theme::ThemeColors::STATUS_WARN)
                } else if crew.state == CrewState::Repairing {
                    ("Damage control".to_string(), theme::ThemeColors::STATUS_OK)
                } else if let Some(post) = crew_assignments.get(&entity) {
                    let label = if walking {
                        format!("Walking to the {}", post.name())
                    } else {
                        format!("On station - {}", post.name())
                    };
                    (label, theme::ThemeColors::STATUS_OK)
                } else if walking {
                    ("Moving".to_string(), theme::ThemeColors::TEXT_MUTED)
                } else {
                    // Genuinely nothing to do — with stand-down, a peacetime
                    // gunner is OFF DUTY rather than idle-because-broken.
                    ("Off duty".to_string(), theme::ThemeColors::TEXT_MUTED)
                };

                list.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(theme::ThemeSpacing::MD),
                        padding: UiRect::all(Val::Px(theme::ThemeSpacing::SM)),
                        ..default()
                    },
                    BackgroundColor(theme::ThemeColors::BG_CARD),
                )).with_children(|row| {
                    // Status dot — one glance instead of parsing text
                    row.spawn((
                        Node { width: Val::Px(8.0), height: Val::Px(8.0), ..default() },
                        BackgroundColor(dot_color),
                    ));
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        row_gap: Val::Px(theme::ThemeSpacing::XS),
                        ..default()
                    }).with_children(|info| {
                        info.spawn((
                            Text::new(format!("{}  -  {}", crew.name, status)),
                            TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                            TextColor(if crew.health <= 0.0 { theme::ThemeColors::TEXT_MUTED } else { theme::ThemeColors::TEXT_PRIMARY }),
                        ));
                        info.spawn((
                            Text::new(format!("HP {:.0}   Morale {:.0}", crew.health, crew.morale)),
                            TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                            TextColor(theme::ThemeColors::TEXT_MUTED),
                        ));
                    });

                    // Standing order, as a real dropdown. Cycling meant
                    // circling all six to get back to the one you wanted.
                    //
                    // The list is a SIBLING of the chip, not a child: nested
                    // inside the button, a click on an option would land on
                    // the chip as well and reopen what it just closed.
                    row.spawn(Node {
                        position_type: PositionType::Relative,
                        width: Val::Px(96.0),
                        ..default()
                    }).with_children(|slot| {
                        slot.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(theme::ThemeSpacing::XS)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(theme::ThemeColors::BG_ELEVATED),
                            BorderColor::all(theme::ThemeColors::BORDER_DEFAULT),
                            Button,
                            Interaction::default(),
                            CrewDutyButton { crew: entity },
                        )).with_children(|chip| {
                            chip.spawn((
                                Text::new(duty.label()),
                                TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                                TextColor(if duty == CrewDuty::Auto {
                                    theme::ThemeColors::TEXT_MUTED
                                } else {
                                    theme::ThemeColors::TEXT_TITLE
                                }),
                                CrewDutyLabel { crew: entity },
                            ));
                        });

                        // Spawned hidden and toggled, rather than spawned on
                        // open: no despawn race with the click that opened it.
                        slot.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Percent(100.0),
                                left: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                border: UiRect::all(Val::Px(1.0)),
                                display: Display::None,
                                ..default()
                            },
                            BackgroundColor(theme::ThemeColors::BG_VOID),
                            BorderColor::all(theme::ThemeColors::BORDER_DEFAULT),
                            GlobalZIndex(60),
                            CrewDutyDropdown { crew: entity },
                        )).with_children(|list| {
                            for option in CrewDuty::ALL {
                                list.spawn((
                                    Node {
                                        justify_content: JustifyContent::Center,
                                        padding: UiRect::all(Val::Px(theme::ThemeSpacing::XS)),
                                        ..default()
                                    },
                                    BackgroundColor(theme::ThemeColors::BG_ELEVATED),
                                    Button,
                                    Interaction::default(),
                                    CrewDutyOption { crew: entity, duty: option },
                                )).with_children(|o| {
                                    o.spawn((
                                        Text::new(option.label()),
                                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                                        TextColor(theme::ThemeColors::TEXT_PRIMARY),
                                    ));
                                });
                            }
                        });
                    });
                });
            }
        });
    });
}

/// Stub for crew assignment input — press 1 to manually assign idle crew to first unstaffed weapon
/// Marks the clickable duty chip on a crew row.
#[derive(Component)]
pub struct CrewDutyButton {
    pub crew: Entity,
}

/// Marks the chip's text so it can be refreshed in place.
#[derive(Component)]
pub struct CrewDutyLabel {
    pub crew: Entity,
}

/// The option list hanging under one crew member's chip.
#[derive(Component)]
pub struct CrewDutyDropdown {
    pub crew: Entity,
}

/// One row of that list.
#[derive(Component)]
pub struct CrewDutyOption {
    pub crew: Entity,
    pub duty: CrewDuty,
}

/// Which crew member's dropdown is open, if any. At most one at a time —
/// several open lists would overlap each other down the roster.
#[derive(Resource, Default)]
pub struct OpenDutyDropdown(pub Option<Entity>);

/// Clicking a chip opens its list, or closes it if it was already open.
fn crew_duty_click(
    buttons: Query<(&Interaction, &CrewDutyButton), Changed<Interaction>>,
    mut open: ResMut<OpenDutyDropdown>,
) {
    for (interaction, button) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        open.0 = if open.0 == Some(button.crew) { None } else { Some(button.crew) };
    }
}

/// Picking an option sets the standing order and closes the list.
fn crew_duty_option_click(
    mut commands: Commands,
    options: Query<(&Interaction, &CrewDutyOption), Changed<Interaction>>,
    mut crew_query: Query<(&CrewMember, Option<&mut CrewDuty>)>,
    mut station_query: Query<&mut CrewStation>,
    mut open: ResMut<OpenDutyDropdown>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for (interaction, option) in options.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok((crew, duty)) = crew_query.get_mut(option.crew) else { continue };
        let changed = duty.as_deref().copied().unwrap_or_default() != option.duty;
        match duty {
            Some(mut d) => *d = option.duty,
            None => {
                commands.entity(option.crew).try_insert(option.duty);
            }
        }
        open.0 = None;

        if !changed {
            continue;
        }
        // Drop the post they were holding so the next auto-assign pass can
        // honour the new order instead of leaving them where they were.
        for mut station in station_query.iter_mut() {
            if station.assigned_crew == Some(option.crew) {
                station.assigned_crew = None;
                station.manually_assigned = false;
            }
        }
        notifications.write(ShowNotification {
            message: format!("{} - {}", crew.name, option.duty.label()),
            notification_type: NotificationType::Info,
            duration: 1.5,
        });
    }
}

/// Shows exactly the open list and hides the rest.
fn sync_crew_duty_dropdowns(
    open: Res<OpenDutyDropdown>,
    mut lists: Query<(&CrewDutyDropdown, &mut Node)>,
) {
    for (dropdown, mut node) in lists.iter_mut() {
        let wanted = if open.0 == Some(dropdown.crew) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != wanted {
            node.display = wanted;
        }
    }
}

/// Keeps the chips reading true while the window stays open.
fn refresh_crew_duty_labels(
    duties: Query<&CrewDuty>,
    mut labels: Query<(&CrewDutyLabel, &mut Text, &mut TextColor)>,
) {
    for (label, mut text, mut color) in labels.iter_mut() {
        let duty = duties.get(label.crew).copied().unwrap_or_default();
        let wanted = duty.label();
        if text.0 != wanted {
            text.0 = wanted.to_string();
        }
        color.0 = if duty == CrewDuty::Auto {
            theme::ThemeColors::TEXT_MUTED
        } else {
            theme::ThemeColors::TEXT_TITLE
        };
    }
}

// ============================================================================
// MAP / INVENTORY OVERLAY (M key)
// ============================================================================

/// World-units-to-map-pixels scale. Covers -MAP_WORLD_RANGE..MAP_WORLD_RANGE
/// on each axis around `center` — big enough to fit every faction territory
/// (25k-175k out) and a star system (star at ~50k-100k out from its own
/// local_center, planets orbiting another 25k-45k+ beyond that).
const MAP_HOSTILE: Color = Color::srgb(1.0, 0.15, 0.15);
const MAP_NEUTRAL: Color = Color::srgb(0.62, 0.66, 0.72);
/// Words about a hostile system: MAP_HOSTILE is too dark to read as text.
const HOSTILE_TEXT: Color = Color::srgb(1.0, 0.38, 0.35);
const MAP_TRADER: Color = Color::srgb(0.35, 0.85, 0.75);
const MAP_WORLD_RANGE: f32 = 600_000.0;

/// Converts a world position to a pixel offset within a square map panel of
/// the given size (top-left origin, Y flipped since world +Y is up but UI
/// +Y is down). `center` is the map's fixed reference point — NOT the
/// player (see map_click_system's doc comment on why the frame doesn't
/// recenter every frame) but the CURRENT SYSTEM's local_center. Every
/// system's local_center lives somewhere in a multi-million-unit box (see
/// celestial::galaxy::generate_galaxy_map's LOCAL_RANGE), not clustered near
/// world origin like only-Haven-ever-existing used to guarantee — hardcoding
/// origin here left every non-Haven system's contents clamped to whichever
/// panel edge was closest, reading as a "glitch."
fn world_to_map_px(world_pos: Vec2, center: Vec2, panel_size: f32, range: f32) -> (f32, f32) {
    let (x, y) = world_to_map_px_raw(world_pos, center, panel_size, range);
    (x.clamp(0.0, panel_size), y.clamp(0.0, panel_size))
}

/// Unclamped. For drawing a curve, where a point off the map must be left
/// out — clamping it would stack every off-map point of an orbit along the
/// panel's border as a solid line.
fn world_to_map_px_raw(world_pos: Vec2, center: Vec2, panel_size: f32, range: f32) -> (f32, f32) {
    let rel = world_pos - center;
    let half = panel_size / 2.0;
    (half + (rel.x / range) * half, half - (rel.y / range) * half)
}

/// The inverse of `world_to_map_px`, from a click's normalised panel position
/// (-0.5..0.5 on each axis, as `ComputedNode::normalize_point` gives it).
///
/// One function for both directions' scale, so a click lands where it was
/// drawn. If the drawing and the click handler ever disagreed on `range`,
/// clicking a planet would warp you somewhere else on the map entirely.
fn map_norm_to_world(norm: Vec2, center: Vec2, range: f32) -> Vec2 {
    center + Vec2::new(norm.x * 2.0 * range, -norm.y * 2.0 * range)
}

/// The map's square. It has a header above and a legend below on top of the
/// window's padding, so 85% of the height alone pushed the title off the top
/// of a 720p window.
fn map_panel_size(win_w: f32, win_h: f32) -> f32 {
    (win_w.min(win_h) * 0.85).min(win_h - 170.0).max(200.0)
}

/// Samples around one orbit on the map. Half are drawn, so this many over
/// two dots per orbit -- enough to read as a curve at map scale without
/// spawning hundreds of UI nodes per planet.
const MAP_ORBIT_SAMPLES: usize = 120;

/// The planet markers' own blue, faded. A path belongs to its planet, so it
/// should read as the same family, and quieter than the thing on it.
const MAP_ORBIT_COLOR: Color = Color::srgba(0.5, 0.6, 0.8, 0.45);

/// Fraction of headroom beyond the outermost orbit, so the furthest planet's
/// path doesn't graze the panel edge.
const MAP_FIT_MARGIN: f32 = 1.08;

/// Half-width of the local map in world units, fitted to the system.
///
/// `MAP_WORLD_RANGE` alone is 600,000, which planets have outgrown: with
/// bodies twice their old size and 80,000-140,000 between them, the median
/// system's outermost planet now sits near 700,000 and one in ten past
/// 1,000,000. At a fixed range more than half of all systems would draw
/// their outer orbits off the edge of the map.
///
/// Fitting each system rather than raising the constant keeps small systems
/// as readable as before -- 600,000 is the floor, never zoomed further in --
/// and only widens the ones that need it.
///
/// Measured from each orbit's APOAPSIS, `a(1+e)`, plus the body's radius:
/// the farthest that orbit ever reaches. Using where planets currently are
/// would make the scale drift as they move, and the map is drawn on open and
/// clicked seconds later.
fn current_map_range(world_data: &MapWorldData) -> f32 {
    let furthest = world_data
        .planet_orbits
        .iter()
        .map(|(orbit, body)| orbit.semi_major_axis * (1.0 + orbit.eccentricity) + body.radius)
        .fold(0.0f32, f32::max);
    (furthest * MAP_FIT_MARGIN).max(MAP_WORLD_RANGE)
}

/// Bundles the map's world-data queries into one SystemParam — Bevy caps how
/// many parameters a single system function can take (16), and
/// toggle_map_overlay's own params plus these pushed it past that.
#[derive(bevy::ecs::system::SystemParam)]
struct MapWorldData<'w, 's> {
    ai_ship_query: Query<'w, 's, (&'static Transform, &'static crate::ai_ship::components::AiShipType, Has<crate::ai_ship::fleet::CivilianHull>), With<crate::ai_ship::components::AiShip>>,
    faction_standing: Res<'w, crate::diplomacy::FactionStanding>,
    sim: Res<'w, crate::ai_ship::components::WorldSimulation>,
    star_query: Query<'w, 's, &'static Transform, With<crate::celestial::components::Star>>,
    planet_query: Query<'w, 's, &'static Transform, With<crate::celestial::components::Planet>>,
    // The orbit itself, not where the planet currently is on it. Both the
    // drawn ellipse and the map's scale come from these, and they are fixed,
    // so the map does not rescale between opening it and clicking on it.
    planet_orbits: Query<'w, 's, (&'static crate::celestial::components::OrbitalPath, &'static crate::celestial::components::CelestialBody), With<crate::celestial::components::Planet>>,
    bounty_ship_query: Query<'w, 's, (&'static Transform, &'static crate::ai_ship::components::BountyTarget), With<crate::ai_ship::components::AiShip>>,
    contract_state: Res<'w, crate::contracts::ContractState>,
    streaming: Res<'w, crate::celestial::resources::SystemStreamingManager>,
    galaxy_map: Res<'w, crate::celestial::resources::GalaxyMap>,
    stations: Res<'w, crate::world::home_base::SystemStations>,
    war: Res<'w, crate::celestial::territory::BorderWar>,
}

/// What the sidebar says about a system you're looking into from elsewhere.
struct SystemPeek {
    /// `system_brief`, plus what's in it.
    lines: Vec<(String, Color, bool)>,
    charge: f32,
    fuel: f32,
    /// It's already the jump target.
    targeted: bool,
}

/// Plain-data snapshot of everything the map needs to render — decoupled
/// from SystemParams so both toggle_map_overlay (open) and map_click_system
/// (re-render after picking a destination) can build the exact same UI
/// without duplicating the layout code.
struct MapSnapshot {
    panel_size: f32,
    player_pos: Vec2,
    // Fixed reference point the local map is drawn around — see
    // world_to_map_px's doc comment.
    map_center: Vec2,
    pending_target: Option<Vec2>,
    current_fuel: f32,
    /// This system's stations — position, name and accent color. Every
    /// system has its own (see world::home_base::station_sites), so unlike
    /// the old fixed Haven-only list these are always the right ones for
    /// wherever the player actually is.
    stations: Vec<(Vec2, String, Color)>,
    /// Half-width of the map in world units, fitted to this system. See
    /// `current_map_range`.
    map_range: f32,
    stars: Vec<Vec2>,
    planets: Vec<Vec2>,
    /// Each planet's orbit as (star position, semi-major axis, eccentricity).
    orbits: Vec<(Vec2, f32, f32)>,
    /// Every ship in the system, coloured by what it is to you.
    hostiles: Vec<(Vec2, Color)>,
    bounties: Vec<Vec2>,
    wrecks_found: usize,
    caves_found: usize,
    settlements_found: usize,
    inventory_items: Vec<(String, u32)>,
    inventory_weight: (f32, f32),
    logs_found: Vec<String>,
    /// Name of the system being shown, for the map header.
    system_name: String,
    /// Does this system attack you? Said under the name in the header.
    system_hostile: bool,
    /// Set when this is another system, seen from here: you aren't on it and
    /// can't dash to a point in it, only jump to the system.
    peek: Option<SystemPeek>,
    /// Nearest station name + range, so the header answers "where am I?"
    /// without hunting for the green dot.
    nearest_station: Option<(String, f32)>,
}

fn spawn_map_overlay(commands: &mut Commands, snap: &MapSnapshot) {
    let panel_size = snap.panel_size;
    commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(20.0),
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            }, BackgroundColor(theme::ThemeColors::BG_VOID), ZIndex(50)),
        MapOverlay,
    )).with_children(|parent| {
        // Left column: header, map panel, legend.
        parent.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(theme::ThemeSpacing::MD),
            flex_shrink: 0.0,
            ..default()
        }).with_children(|col| {
        // Header strip: which system this is, plus where the nearest station
        // sits. The map used to open with no title at all — nothing on screen
        // said which of thirty systems you were looking at.
        col.spawn((
            Node {
                width: Val::Px(panel_size),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(theme::ThemeSpacing::LG), Val::Px(theme::ThemeSpacing::MD)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::ThemeColors::BG_PANEL),
            BorderColor::all(theme::ThemeColors::BORDER_DEFAULT),
        )).with_children(|head| {
            head.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(theme::ThemeSpacing::XS),
                ..default()
            }).with_children(|left| {
                left.spawn((
                    Text::new(format!("{} - {}", if snap.peek.is_some() { "SYSTEM" } else { "LOCAL MAP" }, snap.system_name.to_uppercase())),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::H2), ..default() },
                    TextColor(theme::ThemeColors::TEXT_TITLE),
                ));
                if snap.system_hostile {
                    left.spawn((
                        Text::new("HOSTILE SPACE - they attack on sight"),
                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() },
                        TextColor(HOSTILE_TEXT),
                    ));
                }
                let sub = match &snap.nearest_station {
                    _ if snap.peek.is_some() => "From your charts - you aren't here".to_string(),
                    Some((name, dist)) => format!("Nearest station: {} - {}", name, format_range_km(*dist)),
                    None => "No station in this system".to_string(),
                };
                left.spawn((
                    Text::new(sub),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                    TextColor(theme::ThemeColors::TEXT_SECONDARY),
                ));
            });
            head.spawn((
                Text::new(if snap.peek.is_some() {
                    "SCROLL: ZOOM  -  CLICK: JUMP  -  M: CLOSE"
                } else {
                    "SCROLL: ZOOM  -  CLICK: WARP  -  M: CLOSE"
                }),
                TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                TextColor(theme::ThemeColors::TEXT_MUTED),
            ));
        });
        // Solar system map: fixed world-anchored frame (not recentered on
        // the player) so position relative to the whole map is legible.
        // Clickable — click anywhere to set a warp destination (see
        // map_click_system); Interaction is what makes Bevy track hover/press
        // state on this node at all.
        col.spawn((
            Node {
                width: Val::Px(panel_size),
                height: Val::Px(panel_size),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::ThemeColors::HUD_BG),
            BorderColor::all(theme::ThemeColors::BORDER_DEFAULT),
            Interaction::None,
            MapPanel,
        )).with_children(|map| {
            // Star(s)
            for star_pos in &snap.stars {
                let (x, y) = world_to_map_px(*star_pos, snap.map_center, panel_size, snap.map_range);
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - 7.0),
                        top: Val::Px(y - 7.0),
                        width: Val::Px(14.0),
                        height: Val::Px(14.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.9, 0.4)),
                ));
            }
            // Orbits, drawn before the planets so each planet's marker sits
            // on top of its own path. Sampled from the same
            // `r = a(1-e²)/(1+e·cos θ)` that `celestial::orbits` moves the
            // planet along, so the dots are where the planet will be.
            for &(star, a, e) in &snap.orbits {
                for i in 0..MAP_ORBIT_SAMPLES {
                    // Every other sample: dotted, not solid.
                    if i % 2 == 1 {
                        continue;
                    }
                    let theta = i as f32 / MAP_ORBIT_SAMPLES as f32 * std::f32::consts::TAU;
                    let r = a * (1.0 - e * e) / (1.0 + e * theta.cos());
                    let point = star + Vec2::new(r * theta.cos(), r * theta.sin());
                    let (x, y) = world_to_map_px_raw(point, snap.map_center, panel_size, snap.map_range);
                    // Skip, never clamp -- see world_to_map_px_raw.
                    if x < 0.0 || y < 0.0 || x > panel_size || y > panel_size {
                        continue;
                    }
                    map.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x - 1.0),
                            top: Val::Px(y - 1.0),
                            width: Val::Px(2.0),
                            height: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(MAP_ORBIT_COLOR),
                    ));
                }
            }
            // Planets
            for planet_pos in &snap.planets {
                let (x, y) = world_to_map_px(*planet_pos, snap.map_center, panel_size, snap.map_range);
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - 3.5),
                        top: Val::Px(y - 3.5),
                        width: Val::Px(7.0),
                        height: Val::Px(7.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.5, 0.6, 0.8)),
                ));
            }
            // Stations of the system currently loaded — labelled, since two
            // per system means each one is a distinct destination worth
            // recognising rather than an anonymous green dot.
            for (station_pos, name, accent) in &snap.stations {
                let (x, y) = world_to_map_px(*station_pos, snap.map_center, panel_size, snap.map_range);
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - 5.0),
                        top: Val::Px(y - 5.0),
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(*accent),
                    BorderColor::all(Color::srgba(0.9, 1.0, 0.95, 0.85)),
                ));
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x + 9.0),
                        top: Val::Px(y - 7.0),
                        ..default()
                    },
                    Text::new(name.clone()),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::TINY), ..default() },
                    TextColor(Color::srgba(0.75, 0.95, 0.8, 0.9)),
                ));
            }
            // Hostiles: real (in render range) + still-off-screen simulated
            for (hostile_pos, dot_color) in &snap.hostiles {
                let (x, y) = world_to_map_px(*hostile_pos, snap.map_center, panel_size, snap.map_range);
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - 2.0),
                        top: Val::Px(y - 2.0),
                        width: Val::Px(4.0),
                        height: Val::Px(4.0),
                        ..default()
                    },
                    BackgroundColor(*dot_color),
                ));
            }
            // Active bounty targets, highlighted on top of the generic red
            // hostile dot at the same spot — this is specifically "your" hunt.
            for bounty_pos in &snap.bounties {
                let (x, y) = world_to_map_px(*bounty_pos, snap.map_center, panel_size, snap.map_range);
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - 4.0),
                        top: Val::Px(y - 4.0),
                        width: Val::Px(8.0),
                        height: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.2, 0.85)),
                ));
            }

            // Pending warp destination, if one is selected — gold crosshair
            if let Some(target) = snap.pending_target {
                let (x, y) = world_to_map_px(target, snap.map_center, panel_size, snap.map_range);
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - 6.0),
                        top: Val::Px(y - 1.0),
                        width: Val::Px(12.0),
                        height: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.85, 0.1)),
                    PendingWarpMarker,
                ));
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - 1.0),
                        top: Val::Px(y - 6.0),
                        width: Val::Px(2.0),
                        height: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.85, 0.1)),
                    PendingWarpMarker,
                ));
            }

            // Player marker on top, slightly bigger so it's easy to find
            if snap.peek.is_none() {
                let (px, py) = world_to_map_px(snap.player_pos, snap.map_center, panel_size, snap.map_range);
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(px - 3.5),
                        top: Val::Px(py - 3.5),
                        width: Val::Px(7.0),
                        height: Val::Px(7.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.3, 0.9, 1.0)),
                ));
            }
        });

        // Color key legend, directly under the map
        col.spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(14.0),
            row_gap: Val::Px(4.0),
            width: Val::Px(panel_size),
            ..default()
        }).with_children(|legend| {
            let entries: &[(Color, &str)] = &[
                (Color::srgb(0.3, 0.9, 1.0), "You"),
                (Color::srgb(0.25, 1.0, 0.35), "Station"),
                (MAP_HOSTILE, "Hostile"),
                (MAP_NEUTRAL, "Neutral"),
                (MAP_TRADER, "Trader"),
                (Color::srgb(1.0, 0.2, 0.85), "Bounty target"),
                (Color::srgb(1.0, 0.9, 0.4), "Star"),
                (Color::srgb(0.5, 0.6, 0.8), "Planet"),
                (Color::srgb(1.0, 0.85, 0.1), "Warp target"),
            ];
            for (color, label) in entries {
                legend.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    ..default()
                }).with_children(|row| {
                    row.spawn((
                        Node { width: Val::Px(9.0), height: Val::Px(9.0), ..default() },
                        BackgroundColor(*color),
                    ));
                    row.spawn((Text::new(*label), TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() }, TextColor(theme::ThemeColors::TEXT_SECONDARY)));
                });
            }
        });
        });

        // Sidebar: warp plan, cargo hold, survey log. Rebuilt around cards
        // with real hierarchy — it used to be one flat run of text lines
        // where the warp cost, the cargo list and the log entries all looked
        // exactly alike.
        parent.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Px(360.0),
                height: Val::Percent(92.0),
                padding: UiRect::all(Val::Px(theme::ThemeSpacing::LG)),
                row_gap: Val::Px(theme::ThemeSpacing::MD),
                overflow: Overflow::clip_y(),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::ThemeColors::BG_PANEL),
            BorderColor::all(theme::ThemeColors::BORDER_DEFAULT),
        )).with_children(|parent| {
            let section_header = |parent: &mut ChildSpawnerCommands, label: &str| {
                parent.spawn((
                    Text::new(label.to_uppercase()),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                    TextColor(theme::ThemeColors::TEXT_MUTED),
                    Node { margin: UiRect::top(Val::Px(theme::ThemeSpacing::SM)), ..default() },
                ));
            };

            // ---- ANOTHER SYSTEM ----
            if let Some(peek) = &snap.peek {
                section_header(parent, "This System");
                parent.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(theme::ThemeSpacing::XS),
                        padding: UiRect::all(Val::Px(theme::ThemeSpacing::MD)),
                        border: UiRect::left(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(theme::ThemeColors::BG_CARD),
                    BorderColor::all(if snap.system_hostile { MAP_HOSTILE } else { theme::ThemeColors::BORDER_SUBTLE }),
                )).with_children(|card| {
                    for (text, color, big) in &peek.lines {
                        let size = if *big { theme::ThemeFonts::BODY } else { theme::ThemeFonts::BODY_SMALL };
                        card.spawn((Text::new(text.clone()), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(*color)));
                    }
                });
                section_header(parent, "Jump");
                let can_afford = snap.current_fuel >= peek.fuel;
                parent.spawn((
                    Text::new(format!("Charge: {:.0}s   Fuel: {:.0} / {:.0}", peek.charge, peek.fuel, snap.current_fuel)),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                    TextColor(if can_afford { theme::ThemeColors::TEXT_PRIMARY } else { theme::ThemeColors::ACCENT_RED }),
                ));
                parent.spawn((
                    Text::new(if !can_afford {
                        "Not enough fuel for this jump."
                    } else if peek.targeted {
                        "Jump target set. Press G to jump."
                    } else {
                        "Click the map to make this your jump target."
                    }),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                    TextColor(if !can_afford {
                        theme::ThemeColors::ACCENT_RED
                    } else if peek.targeted {
                        theme::ThemeColors::ACCENT_GREEN
                    } else {
                        theme::ThemeColors::TEXT_MUTED
                    }),
                ));
            }

            // ---- WARP PLAN ----
            if snap.peek.is_none() {
            section_header(parent, "Warp Plan");
            parent.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(theme::ThemeSpacing::XS),
                    padding: UiRect::all(Val::Px(theme::ThemeSpacing::MD)),
                    border: UiRect::left(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(theme::ThemeColors::BG_CARD),
                BorderColor::all(if snap.pending_target.is_some() {
                    theme::ThemeColors::ACCENT_YELLOW
                } else {
                    theme::ThemeColors::BORDER_SUBTLE
                }),
            )).with_children(|card| {
                let Some(target) = snap.pending_target else {
                    card.spawn((
                        Text::new("No destination set"),
                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() },
                        TextColor(theme::ThemeColors::TEXT_SECONDARY),
                    ));
                    card.spawn((
                        Text::new("Click anywhere on the map to plot a jump."),
                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                        TextColor(theme::ThemeColors::TEXT_MUTED),
                    ));
                    return;
                };

                let dist = snap.player_pos.distance(target);
                // Same arithmetic the jump itself uses, so the preview can't
                // quote a price the drive won't charge.
                let jump_dist = (dist - warp_dash_arrival_buffer(dist)).max(0.0);
                let fuel_cost = warp_dash_fuel_cost(jump_dist);
                let charge_time = warp_dash_charge_time(jump_dist);
                let can_afford = snap.current_fuel >= fuel_cost;

                card.spawn((
                    Text::new(format_range_km(dist)),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::H2), ..default() },
                    TextColor(theme::ThemeColors::TEXT_PRIMARY),
                ));

                let mut stat = |card: &mut ChildSpawnerCommands, label: &str, value: String, color: Color| {
                    card.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    }).with_children(|row| {
                        row.spawn((
                            Text::new(label),
                            TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                            TextColor(theme::ThemeColors::TEXT_SECONDARY),
                        ));
                        row.spawn((
                            Text::new(value),
                            TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() },
                            TextColor(color),
                        ));
                    });
                };
                stat(card, "Fuel", format!("{:.0} / {:.0}", fuel_cost, snap.current_fuel),
                    if can_afford { theme::ThemeColors::TEXT_PRIMARY } else { theme::ThemeColors::ACCENT_RED });
                stat(card, "Charge", format!("{:.0}s", charge_time), theme::ThemeColors::TEXT_PRIMARY);

                card.spawn((
                    Text::new(if can_afford {
                        "Close the map (M), then press G to jump."
                    } else {
                        "Not enough fuel for this jump."
                    }),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                    TextColor(if can_afford { theme::ThemeColors::ACCENT_GREEN } else { theme::ThemeColors::ACCENT_RED }),
                    Node { margin: UiRect::top(Val::Px(theme::ThemeSpacing::XS)), ..default() },
                ));
            });
            }

            // ---- CARGO ----
            section_header(parent, "Cargo Hold");
            let (weight, capacity) = snap.inventory_weight;
            let fill = if capacity > 0.0 { (weight / capacity).clamp(0.0, 1.0) } else { 0.0 };
            let fill_color = if fill > 0.9 {
                theme::ThemeColors::ACCENT_RED
            } else if fill > 0.7 {
                theme::ThemeColors::ACCENT_ORANGE
            } else {
                theme::ThemeColors::ACCENT_CYAN
            };
            parent.spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            }).with_children(|row| {
                row.spawn((
                    Text::new(format!("{:.0} / {:.0}", weight, capacity)),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() },
                    TextColor(theme::ThemeColors::TEXT_PRIMARY),
                ));
                row.spawn((
                    Text::new(format!("{:.0}% full", fill * 100.0)),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                    TextColor(theme::ThemeColors::TEXT_SECONDARY),
                ));
            });
            // Capacity bar — the old sidebar printed the raw weight numbers
            // and nothing else, so "nearly full" never registered until a
            // pickup silently failed.
            parent.spawn((
                Node { width: Val::Percent(100.0), height: Val::Px(6.0), ..default() },
                BackgroundColor(theme::ThemeColors::BG_INPUT),
            )).with_children(|track| {
                track.spawn((
                    Node { width: Val::Percent(fill * 100.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(fill_color),
                ));
            });

            if snap.inventory_items.is_empty() {
                parent.spawn((
                    Text::new("Hold is empty"),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                    TextColor(theme::ThemeColors::TEXT_MUTED),
                ));
            } else {
                parent.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(theme::ThemeSpacing::XS),
                    margin: UiRect::top(Val::Px(theme::ThemeSpacing::XS)),
                    ..default()
                }).with_children(|list| {
                    for (name, count) in &snap.inventory_items {
                        list.spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(theme::ThemeSpacing::MD), Val::Px(theme::ThemeSpacing::SM)),
                                ..default()
                            },
                            BackgroundColor(theme::ThemeColors::BG_CARD),
                        )).with_children(|row| {
                            row.spawn((
                                Text::new(name.clone()),
                                TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() },
                                TextColor(theme::ThemeColors::TEXT_PRIMARY),
                            ));
                            row.spawn((
                                Text::new(format!("{}", count)),
                                TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() },
                                TextColor(theme::ThemeColors::ACCENT_CYAN),
                            ));
                        });
                    }
                });
            }

            // ---- SURVEY ----
            section_header(parent, "Survey");
            parent.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(theme::ThemeSpacing::MD),
                ..default()
            }).with_children(|row| {
                let tiles = [
                    ("WRECKS", snap.wrecks_found),
                    ("CAVES", snap.caves_found),
                    ("SETTLEMENTS", snap.settlements_found),
                ];
                for (label, count) in tiles {
                    row.spawn((
                        Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(theme::ThemeSpacing::XS),
                            padding: UiRect::axes(Val::Px(theme::ThemeSpacing::SM), Val::Px(theme::ThemeSpacing::MD)),
                            ..default()
                        },
                        BackgroundColor(theme::ThemeColors::BG_CARD),
                    )).with_children(|tile| {
                        tile.spawn((
                            Text::new(format!("{}", count)),
                            TextFont { font_size: FontSize::Px(theme::ThemeFonts::H3), ..default() },
                            TextColor(theme::ThemeColors::TEXT_PRIMARY),
                        ));
                        tile.spawn((
                            Text::new(label),
                            TextFont { font_size: FontSize::Px(theme::ThemeFonts::TINY), ..default() },
                            TextColor(theme::ThemeColors::TEXT_MUTED),
                        ));
                    });
                }
            });

            // ---- LOGS ----
            if !snap.logs_found.is_empty() {
                section_header(parent, "Recovered Logs");
                for log in &snap.logs_found {
                    parent.spawn((
                        Text::new(log.clone()),
                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                        TextColor(theme::ThemeColors::TEXT_SECONDARY),
                    ));
                }
            }

        });
    });
}

/// The local map's fixed reference point — the current system's
/// local_center, or the player's own position if there's no real system
/// loaded (a blind warp landed in empty space, see execute_warp_jump).
/// Shared by build_map_snapshot (render) and map_click_system (the inverse
/// screen-to-world conversion) so both agree on the same frame.
fn current_map_center(world_data: &MapWorldData, player_pos: Vec2) -> Vec2 {
    world_data.streaming.loaded_system
        .and_then(|id| world_data.galaxy_map.systems.iter().find(|s| s.id == id))
        .map(|s| s.local_center)
        .unwrap_or(player_pos)
}

/// A ship's dot on the system map: what it is to you.
fn map_ship_dot(standing: &crate::diplomacy::FactionStanding, faction: crate::ai_ship::components::AiShipType, civilian: bool) -> Color {
    if civilian {
        MAP_TRADER
    } else if standing.attacks_player(faction) {
        MAP_HOSTILE
    } else {
        MAP_NEUTRAL
    }
}

/// Whose a system is, what they think of you, and what haunts it: the lines
/// the galaxy map's sidebar and a look inside the system both give. The flag
/// marks the one that should be read first.
fn system_brief(
    sys: &crate::celestial::resources::StarSystemDef,
    galaxy_map: &crate::celestial::resources::GalaxyMap,
    standing: &crate::diplomacy::FactionStanding,
    war: &crate::celestial::territory::BorderWar,
) -> Vec<(String, Color, bool)> {
    use crate::ai_ship::components::{faction_display_name, faction_map_color};
    let mut lines = Vec::new();
    lines.push(if standing.system_hostile(sys) {
        ("HOSTILE - they attack on sight".to_string(), HOSTILE_TEXT, true)
    } else {
        ("Not hostile".to_string(), theme::ThemeColors::ACCENT_GREEN, true)
    });
    match sys.faction {
        None => lines.push(("Nobody's space".to_string(), theme::ThemeColors::TEXT_SECONDARY, false)),
        Some(owner) => {
            let what = if sys.capital { "capital" } else { "space" };
            lines.push((format!("{} {what}", faction_display_name(owner)), faction_map_color(owner), false));
            lines.push((format!("They count you {}", standing.tier(owner).label()), theme::ThemeColors::TEXT_SECONDARY, false));
        }
    }
    // A capital is a world: its name, its people, what it makes.
    if let (true, Some(owner)) = (sys.capital, sys.faction) {
        let (people, makes) = crate::celestial::capital_worlds::capital_blurb(owner);
        lines.push((
            format!("{}: {people}; makes {makes}", crate::celestial::capital_worlds::capital_world_name(&sys.name)),
            theme::ThemeColors::TEXT_SECONDARY,
            false,
        ));
    }
    let borders = crate::celestial::territory::border_factions(galaxy_map, sys.id);
    if !borders.is_empty() {
        let names: Vec<&str> = borders.iter().map(|f| faction_display_name(*f)).collect();
        lines.push((format!("Borders: {}", names.join(", ")), theme::ThemeColors::TEXT_MUTED, false));
    }
    if let Some(boss) = sys.haunt {
        lines.push((format!("Haunted by {}", faction_display_name(boss)), Color::srgb(0.95, 0.3, 0.3), false));
    }
    if let Some(contest) = war.contest(sys.id) {
        for (claimant, share) in contest.claims.iter().filter(|(_, share)| *share >= 1.0) {
            lines.push((
                format!("Contested: {} holds {:.0}%", faction_display_name(*claimant), share),
                Color::srgb(0.95, 0.75, 0.2),
                false,
            ));
        }
    }
    lines
}

/// Turn a snapshot of the system you're in into a look at another one. Its
/// star, planets and stations come from its seed, the same rolls that place
/// them when it loads, so what you see is what you'd arrive at. Ships are
/// whatever the simulation is running there: it doesn't run far-off systems,
/// and the sidebar says so rather than showing them as empty.
fn look_into_system(
    snap: &mut MapSnapshot,
    sys: &crate::celestial::resources::StarSystemDef,
    world_data: &MapWorldData,
    galaxy_target: Option<crate::celestial::resources::GalaxyWarpTarget>,
) {
    use crate::celestial::resources::GalaxyWarpTarget;
    let standing = &*world_data.faction_standing;
    let roll = crate::celestial::galaxy::system_roll(sys);
    let center = sys.local_center;
    let stations = crate::world::home_base::station_sites(sys);
    let furthest = roll.planets.iter()
        .map(|p| p.orbit_distance * (1.0 + p.eccentricity) + p.radius)
        .fold(0.0f32, f32::max);
    let trading: std::collections::HashSet<u32> =
        world_data.sim.fleets.iter().filter(|f| f.trade.is_some()).map(|f| f.id).collect();
    let ships: Vec<(Vec2, Color)> = world_data.sim.ships.iter()
        .filter(|s| s.system_id == sys.id && s.behavior != crate::ai_ship::components::SimBehavior::Dead)
        .map(|s| (s.position, map_ship_dot(standing, s.faction, trading.contains(&s.fleet))))
        .collect();

    let mut lines = system_brief(sys, &world_data.galaxy_map, standing, &world_data.war);
    let plain = theme::ThemeColors::TEXT_SECONDARY;
    lines.push((format!("{:?} star, {} planets", roll.star_class, roll.planets.len()), plain, false));
    for site in &stations {
        lines.push((format!("Station: {}", site.name), plain, false));
    }
    lines.push(if ships.is_empty() {
        ("No ship reports from here".to_string(), theme::ThemeColors::TEXT_MUTED, false)
    } else {
        (format!("{} ships on long-range sensors", ships.len()), plain, false)
    });

    let t = world_data.streaming.current_galaxy_pos.distance(sys.galaxy_pos) / crate::celestial::galaxy::GALAXY_RADIUS;
    snap.peek = Some(SystemPeek {
        lines,
        charge: crate::celestial::warp::interstellar_charge_time(t),
        fuel: crate::celestial::warp::interstellar_fuel_cost(t),
        targeted: galaxy_target == Some(GalaxyWarpTarget::System(sys.id)),
    });
    snap.map_center = center;
    snap.map_range = (furthest * MAP_FIT_MARGIN).max(MAP_WORLD_RANGE);
    snap.stars = vec![center];
    snap.planets = roll.planets.iter()
        .map(|p| {
            let r = crate::celestial::orbits::orbit_radius(p.orbit_distance, p.eccentricity, p.phase);
            center + Vec2::new(p.phase.cos(), p.phase.sin()) * r
        })
        .collect();
    snap.orbits = roll.planets.iter().map(|p| (center, p.orbit_distance, p.eccentricity)).collect();
    snap.stations = stations.iter()
        .map(|s| (s.pos, s.name.clone(), crate::world::home_base::station_accent(s.kind)))
        .collect();
    snap.hostiles = ships;
    snap.bounties = Vec::new();
    snap.pending_target = None;
    snap.system_name = sys.name.clone();
    snap.system_hostile = standing.system_hostile(sys);
    snap.nearest_station = None;
}

fn build_map_snapshot(
    windows: &Query<&Window>,
    player_pos: Vec2,
    pending_target: Option<Vec2>,
    fuel_state: &FuelState,
    discovered: &DiscoveredLocations,
    inventory: &Inventory,
    statistics: &Statistics,
    world_data: &MapWorldData,
) -> MapSnapshot {
    let (win_w, win_h) = windows.single().map(|w| (w.width(), w.height())).unwrap_or((1280.0, 800.0));
    let panel_size = map_panel_size(win_w, win_h);
    let current_system = world_data.streaming.loaded_system;
    let map_center = current_map_center(world_data, player_pos);

    // sim.ships now spans every Hot/Warm system, not just "the one system
    // that exists" like before the galaxy map — an off-screen ship ticking
    // in a Warm neighbor is real but belongs to a totally different local
    // space, so plotting its raw position on THIS system's local map would
    // be meaningless (or just pile up at the map edge). Only this system's
    // own off-screen ships belong on the local tactical view.
    let standing = &world_data.faction_standing;
    let dot = |faction: crate::ai_ship::components::AiShipType, civilian: bool| map_ship_dot(standing, faction, civilian);
    let trading: std::collections::HashSet<u32> =
        world_data.sim.fleets.iter().filter(|f| f.trade.is_some()).map(|f| f.id).collect();
    let mut hostiles: Vec<(Vec2, Color)> = world_data.ai_ship_query.iter()
        .map(|(t, f, civilian)| (t.translation.truncate(), dot(*f, civilian)))
        .collect();
    hostiles.extend(
        world_data.sim.ships.iter()
            .filter(|s| !s.spawned && s.behavior != crate::ai_ship::components::SimBehavior::Dead && Some(s.system_id) == current_system)
            .map(|s| (s.position, dot(s.faction, trading.contains(&s.fleet))))
    );

    let bounties: Vec<Vec2> = crate::contracts::bounty_nav::active_bounty_positions_with_id(
        &world_data.contract_state, &world_data.sim, &world_data.bounty_ship_query,
    ).into_iter()
        .filter(|(_, id)| world_data.sim.ships.iter().find(|s| s.bounty_id == Some(*id)).is_none_or(|s| Some(s.system_id) == current_system))
        .map(|(pos, _)| pos).collect();

    MapSnapshot {
        panel_size,
        player_pos,
        map_center,
        pending_target,
        current_fuel: fuel_state.current_fuel,
        stations: world_data.stations.sites.iter()
            .map(|s| (s.pos, s.name.clone(), crate::world::home_base::station_accent(s.kind)))
            .collect(),
        map_range: current_map_range(&world_data),
        stars: world_data.star_query.iter().map(|t| t.translation.truncate()).collect(),
        planets: world_data.planet_query.iter().map(|t| t.translation.truncate()).collect(),
        orbits: world_data.planet_orbits.iter()
            .filter_map(|(orbit, _)| {
                let star = world_data.star_query.get(orbit.parent).ok()?.translation.truncate();
                Some((star, orbit.semi_major_axis, orbit.eccentricity))
            })
            .collect(),
        hostiles,
        bounties,
        wrecks_found: discovered.wrecks.len(),
        caves_found: discovered.caves.len(),
        settlements_found: discovered.settlements.len(),
        inventory_items: inventory.items.iter().map(|(item_type, count)| (item_type.name().to_string(), *count)).collect(),
        inventory_weight: (inventory.current_weight, inventory.max_capacity),
        logs_found: statistics.logs_found.clone(),
        system_name: current_system
            .and_then(|id| world_data.galaxy_map.systems.iter().find(|s| s.id == id))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "Uncharted space".to_string()),
        system_hostile: current_system
            .and_then(|id| world_data.galaxy_map.systems.iter().find(|s| s.id == id))
            .is_some_and(|s| standing.system_hostile(s)),
        peek: None,
        // From the station's edge, as docking measures it: the centre of a
        // station kilometres across read "3.5 km" while you were 1.4 km off
        // its hull, and the number said nothing about whether F would dock.
        nearest_station: world_data.stations.closest(player_pos)
            .map(|s| (s.name.clone(), (player_pos.distance(s.pos) - s.radius()).max(0.0))),
    }
}

fn toggle_map_overlay(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    existing: Query<Entity, With<MapOverlay>>,
    discovered: Res<DiscoveredLocations>,
    inventory: Res<Inventory>,
    statistics: Res<Statistics>,
    player_query: Query<&Transform, With<Ship>>,
    world_data: MapWorldData,
    pending: Res<PendingWarpTarget>,
    fuel_state: Res<FuelState>,
    windows: Query<&Window>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut view_mode: ResMut<MapViewMode>,
    build_state: Res<State<BuildState>>,
    mut zoom: ResMut<MapZoom>,
) {
    if !keyboard.just_pressed(KeyCode::KeyM) {
        return;
    }

    if let Ok(entity) = existing.single() {
        commands.entity(entity).despawn();
        virtual_time.unpause();
        return;
    }
    // In the yard M cycles hull material. It also opened the full-screen map
    // over the builder and paused time, every press.
    if *build_state.get() != BuildState::Inactive {
        return;
    }

    // Always opens on the local view — Tab switches to the galaxy view
    // while open (see toggle_galaxy_view).
    *view_mode = MapViewMode::Local;
    *zoom = MapZoom::default();

    // Full-screen map pauses the simulation — ships, timers, damage, fuel
    // burn etc. all read Time<Virtual>, so this freezes everything except
    // UI input (which doesn't depend on Time) with no extra state juggling.
    virtual_time.pause();

    let player_pos = player_query.single().map(|t| t.translation.truncate()).unwrap_or(Vec2::ZERO);
    let snapshot = build_map_snapshot(&windows, player_pos, pending.0, &fuel_state, &discovered, &inventory, &statistics, &world_data);
    spawn_map_overlay(&mut commands, &snapshot);
}

/// Tab, while the M-key overlay is open, flips between the local tactical
/// view and the galaxy-scale starmap — rebuilds whichever view is now
/// active from scratch, same pattern as toggle_map_overlay.
fn toggle_galaxy_view(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    existing: Query<Entity, With<MapOverlay>>,
    mut view_mode: ResMut<MapViewMode>,
    galaxy_map: Res<crate::celestial::resources::GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    pending_galaxy: Res<crate::celestial::resources::PendingGalaxyWarpTarget>,
    discovered: Res<DiscoveredLocations>,
    inventory: Res<Inventory>,
    statistics: Res<Statistics>,
    player_query: Query<&Transform, With<Ship>>,
    world_data: MapWorldData,
    pending: Res<PendingWarpTarget>,
    fuel_state: Res<FuelState>,
    windows: Query<&Window>,
    // Bundled: this system is at Bevy's 16-param cap.
    politics: (Res<crate::diplomacy::FactionStanding>, Res<crate::celestial::territory::BorderWar>, ResMut<MapZoom>),
) {
    let (standing, war, mut zoom) = politics;
    let Ok(entity) = existing.single() else { return };
    if !keyboard.just_pressed(KeyCode::Tab) {
        return;
    }

    commands.entity(entity).despawn();
    *zoom = MapZoom::default();
    *view_mode = match *view_mode {
        MapViewMode::Local => MapViewMode::Galaxy,
        MapViewMode::Galaxy => MapViewMode::Local,
    };

    match *view_mode {
        MapViewMode::Local => {
            let player_pos = player_query.single().map(|t| t.translation.truncate()).unwrap_or(Vec2::ZERO);
            let snapshot = build_map_snapshot(&windows, player_pos, pending.0, &fuel_state, &discovered, &inventory, &statistics, &world_data);
            spawn_map_overlay(&mut commands, &snapshot);
        }
        MapViewMode::Galaxy => {
            let (win_w, win_h) = windows.single().map(|w| (w.width(), w.height())).unwrap_or((1280.0, 800.0));
            let panel_size = map_panel_size(win_w, win_h);
            spawn_galaxy_map_overlay(&mut commands, &galaxy_map, &streaming, pending_galaxy.0, &fuel_state, &standing, &war, panel_size, zoom.galaxy);
        }
    }
}

/// World-to-pixel projection for the galaxy view, using StarSystemDef's
/// abstract galaxy_pos (map-only, not a real Transform coordinate — see its
/// doc comment) instead of world_to_map_px's real-Transform local scale.
///
/// Unclamped: zoomed in, a system off the panel is left out by the panel's
/// clip, not stacked along its edge.
fn galaxy_to_map_px(galaxy_pos: Vec2, panel_size: f32, view: GalaxyView) -> (f32, f32) {
    let half = panel_size / 2.0;
    let rel = (galaxy_pos - view.center) / view.span();
    (half + rel.x * half, half - rel.y * half)
}

/// Galaxy-scale starmap. Unknown systems render nothing at all (true fog of
/// war); Located systems show as a dim unlabeled pip (revealed passively by
/// proximity — celestial::galaxy::passive_proximity_discovery — or by a
/// blind warp landing near one); Visited systems show colored by
/// danger_tier. Clickable ANYWHERE, not just on discovered pips — one
/// continuous space, not a picker limited to what you've already found;
/// see galaxy_map_click_system.
fn spawn_galaxy_map_overlay(
    commands: &mut Commands,
    galaxy_map: &crate::celestial::resources::GalaxyMap,
    streaming: &crate::celestial::resources::SystemStreamingManager,
    pending: Option<crate::celestial::resources::GalaxyWarpTarget>,
    fuel_state: &FuelState,
    standing: &crate::diplomacy::FactionStanding,
    war: &crate::celestial::territory::BorderWar,
    panel_size: f32,
    view: GalaxyView,
) {
    use crate::celestial::resources::SystemDiscovery;
    use crate::ai_ship::components::{faction_display_name, faction_map_color};

    commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(20.0),
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            }, BackgroundColor(theme::ThemeColors::BG_VOID), ZIndex(50)),
        MapOverlay,
    )).with_children(|parent| {
        parent.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            flex_shrink: 0.0,
            ..default()
        }).with_children(|col| {
            col.spawn((
                Node {
                    width: Val::Px(panel_size),
                    height: Val::Px(panel_size),
                    position_type: PositionType::Relative,
                    overflow: Overflow::clip(),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(theme::ThemeColors::HUD_BG),
                Interaction::None,
                GalaxyMapPanel,
            )).with_children(|map| {
                for sys in &galaxy_map.systems {
                    // Colored by WHOSE territory it is, not a generic danger
                    // bucket (faction_map_color). The charts know who holds
                    // a system you've only located, so its pip carries the
                    // owner's colour too, dimmed; a visited one is full size.
                    let owner_color = match sys.faction {
                        None => Color::srgb(0.3, 0.9, 1.0), // Haven
                        Some(faction) => faction_map_color(faction),
                    };
                    let (size, color) = match sys.discovery {
                        SystemDiscovery::Unknown => continue,
                        SystemDiscovery::Located => (7.0, owner_color.with_alpha(0.45)),
                        SystemDiscovery::Visited => (10.0, owner_color),
                    };
                    let (x, y) = galaxy_to_map_px(sys.galaxy_pos, panel_size, view);
                    if x < -30.0 || y < -30.0 || x > panel_size + 30.0 || y > panel_size + 30.0 {
                        continue;
                    }
                    // Who holds a system is its colour; whether it shoots at
                    // you is this: a red glow behind the pip, and its name.
                    // Nothing else on the map gets a name, so red words are
                    // the list of places to be careful about.
                    let hostile = standing.system_hostile(sys);
                    if hostile {
                        let glow = size + 12.0;
                        map.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(x - glow / 2.0),
                                top: Val::Px(y - glow / 2.0),
                                width: Val::Px(glow),
                                height: Val::Px(glow),
                                ..default()
                            },
                            BackgroundColor(MAP_HOSTILE.with_alpha(0.4)),
                        ));
                    }
                    // A capital wears a ring in its owner's colour; a haunted
                    // system a red one, wider -- the boss is what matters there.
                    // Contested: a rival holds a real share (a quarter or more).
                    let contested = war.contest(sys.id).is_some_and(|c| c.claims.iter().any(|(_, share)| *share >= 25.0));
                    for (ring, ring_color, show) in [
                        (size + 8.0, owner_color, sys.capital),
                        (size + 14.0, Color::srgb(0.95, 0.15, 0.15), sys.haunt.is_some()),
                        (size + 20.0, Color::srgb(0.95, 0.75, 0.2), contested),
                    ] {
                        if show {
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(x - ring / 2.0),
                                    top: Val::Px(y - ring / 2.0),
                                    width: Val::Px(ring),
                                    height: Val::Px(ring),
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BorderColor::all(ring_color),
                            ));
                        }
                    }
                    map.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x - size / 2.0),
                            top: Val::Px(y - size / 2.0),
                            width: Val::Px(size),
                            height: Val::Px(size),
                            ..default()
                        },
                        BackgroundColor(color),
                    ));
                    if hostile || view.zoom >= GALAXY_NAMES_ALL_ZOOM {
                        // Past the glow, on whichever side has the room.
                        let gap = size / 2.0 + 10.0;
                        let mut label = Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(y - 8.0),
                            ..default()
                        };
                        if x > panel_size * 0.8 {
                            label.right = Val::Px(panel_size - x + gap);
                        } else {
                            label.left = Val::Px(x + gap);
                        }
                        map.spawn((
                            Text::new(sys.name.clone()),
                            TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                            TextColor(if hostile { HOSTILE_TEXT } else { theme::ThemeColors::TEXT_SECONDARY }),
                            label,
                        ));
                    }
                }

                // "You are here" — current position, whether it's a real
                // system or an open patch of blind-warped space.
                let (px, py) = galaxy_to_map_px(streaming.current_galaxy_pos, panel_size, view);
                map.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(px - 4.0),
                        top: Val::Px(py - 4.0),
                        width: Val::Px(8.0),
                        height: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.3, 0.9, 1.0)),
                ));

                // Pending target crosshair, wherever it resolves to —
                // a known system's position, or the raw blind-click point.
                if let Some(target) = pending {
                    if let Some(pos) = crate::celestial::warp::target_galaxy_pos(galaxy_map, target) {
                        let (tx, ty) = galaxy_to_map_px(pos, panel_size, view);
                        map.spawn((
                            Node { position_type: PositionType::Absolute, left: Val::Px(tx - 6.0), top: Val::Px(ty - 1.0), width: Val::Px(12.0), height: Val::Px(2.0), ..default() },
                            BackgroundColor(Color::srgb(1.0, 0.85, 0.1)),
                        ));
                        map.spawn((
                            Node { position_type: PositionType::Absolute, left: Val::Px(tx - 1.0), top: Val::Px(ty - 6.0), width: Val::Px(2.0), height: Val::Px(12.0), ..default() },
                            BackgroundColor(Color::srgb(1.0, 0.85, 0.1)),
                        ));
                    }
                }
            });
            col.spawn((Text::new("GALAXY MAP - Scroll: zoom, in on a system to look inside | Tab: local view | M: close"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() }, TextColor(theme::ThemeColors::TEXT_MUTED)));

            // Faction color reference — Visited systems are colored by
            // whose territory they are (see faction_map_color); Located
            // ones show as a dim pip since the faction isn't known yet.
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(12.0),
                row_gap: Val::Px(4.0),
                width: Val::Px(panel_size),
                ..default()
            }).with_children(|legend| {
                // Labels come from faction_display_name so the legend can never
                // drift out of step with the roster. Only the factions that
                // hold ground have a colour on the map; the bosses haunt.
                let mut entries: Vec<(Color, &str)> = vec![
                    (MAP_HOSTILE.with_alpha(0.4), "Red glow, named: HOSTILE"),
                    (Color::srgb(0.3, 0.9, 1.0), "Haven"),
                ];
                entries.extend(
                    crate::celestial::territory::TERRITORIAL.iter().map(|f| (faction_map_color(*f), faction_display_name(*f))),
                );
                entries.push((Color::WHITE.with_alpha(0.45), "Located (dim)"));
                let entries: &[(Color, &str)] = &entries;
                for (color, label) in entries {
                    legend.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.0),
                        ..default()
                    }).with_children(|row| {
                        row.spawn((
                            Node { width: Val::Px(9.0), height: Val::Px(9.0), ..default() },
                            BackgroundColor(*color),
                        ));
                        row.spawn((Text::new(*label), TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() }, TextColor(theme::ThemeColors::TEXT_SECONDARY)));
                    });
                }
            });
        });

        // Sidebar: discovery progress, the warp target's owner and what it
        // thinks of you, and who is at war along which border.
        parent.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Px(260.0),
                height: Val::Percent(90.0),
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(6.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme::ThemeColors::BG_PANEL),
        )).with_children(|parent| {
            parent.spawn((Text::new("GALAXY"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::H2), ..default() }, TextColor(theme::ThemeColors::TEXT_TITLE)));

            let visited = galaxy_map.systems.iter().filter(|s| s.discovery == SystemDiscovery::Visited).count();
            let located = galaxy_map.systems.iter().filter(|s| s.discovery == SystemDiscovery::Located).count();
            let total = galaxy_map.systems.len();
            let unknown = total - visited - located;

            for (label, count, color) in [
                ("Visited", visited, theme::ThemeColors::ACCENT_GREEN),
                ("Located", located, theme::ThemeColors::TEXT_SECONDARY),
                ("Unknown", unknown, theme::ThemeColors::TEXT_MUTED),
            ] {
                parent.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() })
                    .with_children(|row| {
                        row.spawn((Text::new(label), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(color)));
                        row.spawn((Text::new(format!("{}", count)), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(theme::ThemeColors::TEXT_PRIMARY)));
                    });
            }

            // Cost/charge preview for the pending target, if any — same
            // numbers celestial::warp::warp_input_system will actually use.
            if let Some(target) = pending {
                if let Some(pos) = crate::celestial::warp::target_galaxy_pos(galaxy_map, target) {
                    let dist = streaming.current_galaxy_pos.distance(pos);
                    let t = dist / crate::celestial::galaxy::GALAXY_RADIUS;
                    let charge = crate::celestial::warp::interstellar_charge_time(t);
                    let fuel = crate::celestial::warp::interstellar_fuel_cost(t);
                    let name = match target {
                        crate::celestial::resources::GalaxyWarpTarget::System(id) => galaxy_map.systems.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default(),
                        crate::celestial::resources::GalaxyWarpTarget::BlindPoint(_) => "Uncharted space".to_string(),
                    };
                    parent.spawn((
                        Text::new("WARP TARGET"),
                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                        TextColor(theme::ThemeColors::TEXT_MUTED),
                        Node { margin: UiRect::top(Val::Px(theme::ThemeSpacing::SM)), ..default() },
                    ));
                    parent.spawn((Text::new(name), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(theme::ThemeColors::TEXT_PRIMARY)));
                    let fuel_color = if fuel_state.current_fuel < fuel { theme::ThemeColors::ACCENT_ORANGE } else { theme::ThemeColors::TEXT_SECONDARY };
                    parent.spawn((Text::new(format!("Charge: {:.0}s   Fuel: {:.0}", charge, fuel)), TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() }, TextColor(fuel_color)));
                    parent.spawn((Text::new("Press G to jump"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() }, TextColor(theme::ThemeColors::TEXT_MUTED)));
                }
            }

            // The warp target's place in the galaxy: whose it is, whether
            // it's their capital, who it borders, and what haunts it.
            if let Some(crate::celestial::resources::GalaxyWarpTarget::System(id)) = pending {
                if let Some(sys) = galaxy_map.systems.iter().find(|s| s.id == id) {
                    for (text, color, big) in system_brief(sys, galaxy_map, standing, war) {
                        let size = if big { theme::ThemeFonts::BODY } else { theme::ThemeFonts::CAPTION };
                        parent.spawn((Text::new(text), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color)));
                    }
                }
            }

            // Wars along the borders you know of: two factions at war whose
            // territories touch at a system you've at least located.
            let mut wars: Vec<(crate::ai_ship::components::AiShipType, crate::ai_ship::components::AiShipType)> = Vec::new();
            for sys in galaxy_map.systems.iter().filter(|s| s.discovery != SystemDiscovery::Unknown) {
                let Some(owner) = sys.faction else { continue };
                for other in crate::celestial::territory::border_factions(galaxy_map, sys.id) {
                    let pair = if (owner as u8) < (other as u8) { (owner, other) } else { (other, owner) };
                    if crate::ai_ship::components::factions_hostile(owner, other) && !wars.contains(&pair) {
                        wars.push(pair);
                    }
                }
            }
            if !wars.is_empty() {
                parent.spawn((
                    Text::new("AT WAR"),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                    TextColor(theme::ThemeColors::TEXT_MUTED),
                    Node { margin: UiRect::top(Val::Px(theme::ThemeSpacing::SM)), ..default() },
                ));
                for (a, b) in wars.iter().take(10) {
                    parent.spawn((
                        Text::new(format!("{} - {}", faction_display_name(*a), faction_display_name(*b))),
                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                        TextColor(theme::ThemeColors::TEXT_SECONDARY),
                    ));
                }
            }
            // Systems that changed hands lately, on your charts.
            let news: Vec<String> = war
                .news
                .iter()
                .rev()
                .filter_map(|(id, _, to)| {
                    let sys = galaxy_map.systems.iter().find(|s| s.id == *id)?;
                    (sys.discovery != SystemDiscovery::Unknown)
                        .then(|| format!("{} fell to the {}", sys.name, faction_display_name(*to)))
                })
                .take(5)
                .collect();
            if !news.is_empty() {
                parent.spawn((
                    Text::new("RECENT"),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                    TextColor(theme::ThemeColors::TEXT_MUTED),
                    Node { margin: UiRect::top(Val::Px(theme::ThemeSpacing::SM)), ..default() },
                ));
                for line in news {
                    parent.spawn((
                        Text::new(line),
                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                        TextColor(theme::ThemeColors::TEXT_SECONDARY),
                    ));
                }
            }
            parent.spawn((
                Text::new("Ring: capital   Red ring: haunted   Amber ring: contested"),
                TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                TextColor(theme::ThemeColors::TEXT_MUTED),
                Node { margin: UiRect::top(Val::Px(theme::ThemeSpacing::SM)), ..default() },
            ));

            parent.spawn((Text::new("Press M to close"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() }, TextColor(theme::ThemeColors::TEXT_MUTED)));
        });
    });
}

/// Handles clicks on the map panel: converts cursor position to world
/// coordinates, sets that as the pending warp destination, and rebuilds the
/// entire overlay so the sidebar's cost/charge-time preview and the
/// crosshair both update in the same frame — patching just the crosshair
/// left the sidebar (the main feedback the player actually looks at) frozen
/// on "click the map to set a destination" forever, which read as "clicking
/// does nothing."
fn map_click_system(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    map_panel: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<MapPanel>>,
    existing: Query<Entity, With<MapOverlay>>,
    windows: Query<&Window>,
    mut pending: ResMut<PendingWarpTarget>,
    // One warp key: the latest pick, on either map, is where it goes.
    mut pending_galaxy: ResMut<crate::celestial::resources::PendingGalaxyWarpTarget>,
    player_query: Query<&Transform, With<Ship>>,
    discovered: Res<DiscoveredLocations>,
    inventory: Res<Inventory>,
    statistics: Res<Statistics>,
    world_data: MapWorldData,
    fuel_state: Res<FuelState>,
    mut notifications: MessageWriter<ShowNotification>,
    zoom: Res<MapZoom>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok((node, transform)) = map_panel.single() else { return };
    let Ok(window) = windows.single() else { return };
    // window.cursor_position() is logical pixels; ComputedNode/UiGlobalTransform
    // are physical — on a Retina Mac (2x) that alone silently broke every
    // click. Also: this was querying the classic 2D/3D `GlobalTransform`,
    // which Bevy 0.19 no longer keeps in sync for UI nodes at all (UI now
    // uses its own dedicated `UiGlobalTransform` — see picking_backend.rs)
    // — so panel_center was reading a stale/default value regardless of the
    // pixel-scale bug. Both are fixed by using UiGlobalTransform plus Bevy's
    // own `ComputedNode::normalize_point` hit-test helper instead of manual
    // rectangle math.
    let Some(cursor_pos) = window.cursor_position().map(|p| p * window.scale_factor()) else { return };

    let Some(norm) = node.normalize_point(*transform, cursor_pos) else { return };
    if norm.x.abs() > 0.5 || norm.y.abs() > 0.5 {
        return; // click landed outside the map panel (e.g. on the sidebar)
    }

    let player_pos = player_query.single().map(|t| t.translation.truncate()).unwrap_or(Vec2::ZERO);

    // Looking into another system: there's no dashing to a point in a place
    // you aren't. The click picks the system to jump to.
    if let Some(sys) = zoom.peek.and_then(|id| world_data.galaxy_map.systems.iter().find(|s| s.id == id)) {
        pending_galaxy.0 = Some(crate::celestial::resources::GalaxyWarpTarget::System(sys.id));
        pending.0 = None;
        notifications.write(ShowNotification {
            message: format!("Jump target set - {}.", sys.name),
            notification_type: NotificationType::Info,
            duration: 2.5,
        });
        if let Ok(entity) = existing.single() {
            commands.entity(entity).despawn();
        }
        let mut snapshot = build_map_snapshot(&windows, player_pos, pending.0, &fuel_state, &discovered, &inventory, &statistics, &world_data);
        look_into_system(&mut snapshot, sys, &world_data, pending_galaxy.0);
        spawn_map_overlay(&mut commands, &snapshot);
        return;
    }

    let map_center = current_map_center(&world_data, player_pos);
    let target = map_norm_to_world(norm, map_center, current_map_range(&world_data));
    pending.0 = Some(target);
    pending_galaxy.0 = None;

    let dist = player_pos.distance(target);
    notifications.write(ShowNotification {
        message: format!("Warp target set - {:.0} units away.", dist),
        notification_type: NotificationType::Info,
        duration: 2.5,
    });

    // Full rebuild so the sidebar preview and crosshair are consistent.
    if let Ok(entity) = existing.single() {
        commands.entity(entity).despawn();
    }
    let snapshot = build_map_snapshot(&windows, player_pos, pending.0, &fuel_state, &discovered, &inventory, &statistics, &world_data);
    spawn_map_overlay(&mut commands, &snapshot);
}

/// Handles clicks on the galaxy map panel — clickable ANYWHERE, not just on
/// discovered pips (one continuous space, see spawn_galaxy_map_overlay's
/// doc comment). If the click lands within SNAP_TOLERANCE of a system
/// that's already Located/Visited (i.e. a pip you can actually see), it
/// targets that system precisely; otherwise it's a blind point — whether
/// anything is actually there is only revealed on arrival
/// (celestial::warp::execute_warp_jump).
fn galaxy_map_click_system(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    map_panel: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<GalaxyMapPanel>>,
    existing: Query<Entity, With<MapOverlay>>,
    windows: Query<&Window>,
    mut pending: ResMut<crate::celestial::resources::PendingGalaxyWarpTarget>,
    mut dash_target: ResMut<PendingWarpTarget>,
    galaxy_map: Res<crate::celestial::resources::GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    fuel_state: Res<FuelState>,
    mut notifications: MessageWriter<ShowNotification>,
    standing: Res<crate::diplomacy::FactionStanding>,
    war: Res<crate::celestial::territory::BorderWar>,
    zoom: Res<MapZoom>,
) {
    use crate::celestial::resources::{GalaxyWarpTarget, SystemDiscovery};

    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok((node, transform)) = map_panel.single() else { return };
    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position().map(|p| p * window.scale_factor()) else { return };
    let Some(norm) = node.normalize_point(*transform, cursor_pos) else { return };
    if norm.x.abs() > 0.5 || norm.y.abs() > 0.5 {
        return; // click landed outside the map panel (e.g. on the sidebar)
    }

    let clicked_pos = zoom.galaxy.at(norm);

    // Snap onto a VISIBLE pip if the click is close to one — precision aid
    // for known systems. Undiscovered systems don't get this treatment
    // (there's no pip to aim at), so clicking near one still just sets a
    // blind point; the arrival-time snap in execute_warp_jump is what
    // reveals it as a surprise.
    let snapped = galaxy_map.systems.iter()
        .filter(|s| s.discovery != SystemDiscovery::Unknown && s.galaxy_pos.distance(clicked_pos) <= crate::celestial::galaxy::SNAP_TOLERANCE)
        .min_by(|a, b| a.galaxy_pos.distance(clicked_pos).partial_cmp(&b.galaxy_pos.distance(clicked_pos)).unwrap());

    let (target, desc) = match snapped {
        Some(sys) => (GalaxyWarpTarget::System(sys.id), sys.name.clone()),
        None => (GalaxyWarpTarget::BlindPoint(clicked_pos), "uncharted space".to_string()),
    };
    pending.0 = Some(target);
    dash_target.0 = None;

    let dist = streaming.current_galaxy_pos.distance(clicked_pos);
    notifications.write(ShowNotification {
        message: format!("Warp target set - {} ({:.0} units away).", desc, dist),
        notification_type: NotificationType::Info,
        duration: 2.5,
    });

    if let Ok(entity) = existing.single() {
        commands.entity(entity).despawn();
    }
    let (win_w, win_h) = windows.single().map(|w| (w.width(), w.height())).unwrap_or((1280.0, 800.0));
    let panel_size = map_panel_size(win_w, win_h);
    spawn_galaxy_map_overlay(&mut commands, &galaxy_map, &streaming, pending.0, &fuel_state, &standing, &war, panel_size, zoom.galaxy);
}

/// The scroll wheel walks the map: from your ship out to the system, out to
/// the galaxy, and back in again into whichever system is under the cursor.
/// Plus and minus do the same, a notch a press.
///
/// The map was reached with M and flipped with Tab, which still work. Each
/// step rebuilds the overlay from scratch, like everything else on the map.
fn map_scroll_system(
    mut commands: Commands,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    existing: Query<Entity, With<MapOverlay>>,
    galaxy_panel: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<GalaxyMapPanel>>,
    state: (ResMut<MapViewMode>, ResMut<MapZoom>, ResMut<Time<Virtual>>),
    flight: (Res<crate::camera::CameraState>, Res<State<BuildState>>, Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<crate::ui::windows::framework::ScrollsOnWheel>>),
    records: (Res<DiscoveredLocations>, Res<Inventory>, Res<Statistics>),
    player_query: Query<&Transform, With<Ship>>,
    world_data: MapWorldData,
    targets: (Res<PendingWarpTarget>, Res<crate::celestial::resources::PendingGalaxyWarpTarget>),
    fuel_state: Res<FuelState>,
    windows: Query<&Window>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let (mut view_mode, mut zoom, mut virtual_time) = state;
    let (camera, build_state, scroll_lists) = flight;
    let (discovered, inventory, statistics) = records;
    let (pending, pending_galaxy) = targets;

    let mut notches: f32 = scroll.read().map(|e| match e.unit {
        MouseScrollUnit::Line => e.y,
        MouseScrollUnit::Pixel => e.y / MAP_PIXELS_PER_NOTCH,
    }).sum();
    if existing.single().is_ok() {
        if keyboard.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) { notches += 1.0; }
        if keyboard.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) { notches -= 1.0; }
    }
    let now = time.elapsed_secs_f64();
    // Let go of the wheel, or turn back, and the push toward a crossing is off.
    if now - zoom.last_scroll > 0.5 || notches * zoom.pushed < 0.0 {
        zoom.pushed = 0.0;
    }
    if notches == 0.0 {
        return;
    }
    zoom.last_scroll = now;
    if now < zoom.hold_until {
        return;
    }

    let window = windows.single().ok();
    let cursor = window.and_then(|w| w.cursor_position().map(|p| p * w.scale_factor()));
    let player_pos = player_query.single().map(|t| t.translation.truncate()).unwrap_or(Vec2::ZERO);
    let (win_w, win_h) = window.map(|w| (w.width(), w.height())).unwrap_or((1280.0, 800.0));
    let panel_size = map_panel_size(win_w, win_h);

    let spawn_system = |commands: &mut Commands, peek: Option<u32>| {
        let mut snapshot = build_map_snapshot(&windows, player_pos, pending.0, &fuel_state, &discovered, &inventory, &statistics, &world_data);
        if let Some(sys) = peek.and_then(|id| world_data.galaxy_map.systems.iter().find(|s| s.id == id)) {
            look_into_system(&mut snapshot, sys, &world_data, pending_galaxy.0);
        }
        spawn_map_overlay(commands, &snapshot);
    };
    let spawn_galaxy = |commands: &mut Commands, view: GalaxyView| {
        spawn_galaxy_map_overlay(commands, &world_data.galaxy_map, &world_data.streaming, pending_galaxy.0, &fuel_state, &world_data.faction_standing, &world_data.war, panel_size, view);
    };

    // In flight: past the camera's limit, the system map.
    let Ok(overlay) = existing.single() else {
        let over_list = crate::ui::windows::framework::cursor_over_scroll_list(&scroll_lists, cursor);
        if notches > 0.0 || over_list || camera.zoom < camera.max_zoom || *build_state.get() != BuildState::Inactive {
            zoom.pushed = 0.0;
            return;
        }
        zoom.pushed += notches;
        if zoom.pushed > -FLIGHT_NOTCHES_TO_MAP {
            return;
        }
        *zoom = MapZoom { hold_until: now + MAP_CROSSING_HOLD, last_scroll: now, ..default() };
        *view_mode = MapViewMode::Local;
        virtual_time.pause();
        spawn_system(&mut commands, None);
        return;
    };

    match *view_mode {
        MapViewMode::Galaxy => {
            let norm = galaxy_panel.single().ok()
                .and_then(|(node, transform)| cursor.and_then(|c| node.normalize_point(*transform, c)))
                .filter(|n| n.abs().max_element() <= 0.5)
                .unwrap_or(Vec2::ZERO);
            // Already all the way in: the rest of the scroll is into a system.
            if notches > 0.0 && zoom.galaxy.zoom >= GALAXY_MAX_ZOOM {
                zoom.pushed += notches;
                if zoom.pushed < 1.0 {
                    return;
                }
                let Some(id) = system_to_enter(&world_data.galaxy_map, zoom.galaxy, norm) else { return };
                commands.entity(overlay).despawn();
                zoom.pushed = 0.0;
                zoom.hold_until = now + MAP_CROSSING_HOLD;
                // Into the one you're in is the live map, not the charts.
                zoom.peek = (world_data.streaming.loaded_system != Some(id)).then_some(id);
                *view_mode = MapViewMode::Local;
                let peek = zoom.peek;
                spawn_system(&mut commands, peek);
                return;
            }
            let view = zoom.galaxy.scrolled(notches, norm);
            if view != zoom.galaxy {
                zoom.galaxy = view;
                commands.entity(overlay).despawn();
                spawn_galaxy(&mut commands, view);
            }
        }
        MapViewMode::Local => {
            zoom.pushed += notches;
            if zoom.pushed <= -1.0 {
                // Out to the galaxy, still close on the system you were in.
                let here = zoom.peek
                    .and_then(|id| world_data.galaxy_map.systems.iter().find(|s| s.id == id))
                    .map_or(world_data.streaming.current_galaxy_pos, |s| s.galaxy_pos);
                commands.entity(overlay).despawn();
                zoom.peek = None;
                zoom.pushed = 0.0;
                zoom.hold_until = now + MAP_CROSSING_HOLD;
                zoom.galaxy = GalaxyView::close_on(here);
                *view_mode = MapViewMode::Galaxy;
                let view = zoom.galaxy;
                spawn_galaxy(&mut commands, view);
            } else if zoom.pushed >= 1.0 {
                // In from your own system's map is back to your ship. Inside
                // another system there's nowhere further in to go.
                zoom.pushed = 0.0;
                if zoom.peek.is_none() {
                    commands.entity(overlay).despawn();
                    virtual_time.unpause();
                }
            }
        }
    }
}

/// The warp key: G, with V kept as a second key for the same thing.
///
/// There used to be two -- G dashed inside the system, V jumped to another
/// star -- and picking the wrong one did nothing but complain. Now one key
/// warps to wherever you last pointed: a galaxy-map pick jumps between
/// stars (celestial::warp::warp_input_system), anything else dashes across
/// this system (warp_dash_input). Picking on either map clears the other.
pub(crate) fn warp_key_pressed(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.any_just_pressed([KeyCode::KeyG, KeyCode::KeyV])
}

/// The warp key, dashing within the system: toward the destination clicked on
/// the system map, or failing that whatever is targeted. Press to start the
/// charge, press again to cancel -- the same as a jump between stars, which
/// this leaves alone (a galaxy-map destination is set, or that drive is
/// already charging).
fn warp_dash_input(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    ship_query: Query<(Entity, &Transform, Has<crate::celestial::components::WarpCharging>), (With<Ship>, Without<MapWarpCharging>)>,
    mut charging_query: Query<(Entity, &mut MapWarpCharging), With<Ship>>,
    pending: Res<PendingWarpTarget>,
    pending_galaxy: Res<crate::celestial::resources::PendingGalaxyWarpTarget>,
    selection: Res<crate::combat::targeting::selection::TargetSelection>,
    target_pos_query: Query<&GlobalTransform>,
    fuel_state: Res<FuelState>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if warp_key_pressed(&keyboard) {
        if let Ok((entity, charging)) = charging_query.single() {
            if !charging.charge_timer.is_finished() {
                commands.entity(entity).remove::<MapWarpCharging>();
                notifications.write(ShowNotification {
                    message: "Warp dash cancelled.".into(),
                    notification_type: NotificationType::Info,
                    duration: 2.0,
                });
            }
            return;
        }
        let Ok((entity, transform, jump_charging)) = ship_query.single() else { return };
        if jump_charging || pending_galaxy.0.is_some() {
            return;
        }
        let ship_pos = transform.translation.truncate();

        // Fall back to whatever's currently targeted (\\ cycles contacts).
        // The dash used to be reachable ONLY by opening the map and clicking,
        // so a player on the keyboard pressed G, got told to go and find a
        // mouse, and never used the drive again.
        let target = pending.0.or_else(|| {
            selection
                .target
                .and_then(|e| target_pos_query.get(e).ok())
                .map(|gt| gt.translation().truncate())
        });
        let Some(target) = target else {
            notifications.write(ShowNotification {
                message: "No warp destination - click the map (M), or target something (\\).".into(),
                notification_type: NotificationType::Warning,
                duration: 3.0,
            });
            return;
        };

        let dist = ship_pos.distance(target);
        if dist < WARP_DASH_MIN_DISTANCE {
            notifications.write(ShowNotification {
                message: "Already at the warp destination.".into(),
                notification_type: NotificationType::Info,
                duration: 2.5,
            });
            return;
        }

        let buffer = warp_dash_arrival_buffer(dist);
        let jump_dist = dist - buffer;
        let fuel_cost = warp_dash_fuel_cost(jump_dist);
        if fuel_state.current_fuel < fuel_cost {
            notifications.write(ShowNotification {
                message: format!("Not enough fuel for the jump ({:.0} needed, {:.0} available).", fuel_cost, fuel_state.current_fuel),
                notification_type: NotificationType::Warning,
                duration: 3.0,
            });
            return;
        }

        let dir = (target - ship_pos).normalize_or_zero();
        let target_pos = target - dir * buffer;
        let charge_time = warp_dash_charge_time(jump_dist);

        commands.entity(entity).insert(MapWarpCharging {
            charge_timer: Timer::from_seconds(charge_time, TimerMode::Once),
            target_pos,
            fuel_cost,
        });

        notifications.write(ShowNotification {
            message: format!("Warp dash charging: {:.0} fuel, {:.0}s - G again to cancel.", fuel_cost, charge_time),
            notification_type: NotificationType::Info,
            duration: charge_time + 1.0,
        });
        return;
    }

    if let Ok((_, mut charging)) = charging_query.single_mut() {
        charging.charge_timer.tick(time.delta());
    }
}

/// Where a dash meant for `target` may actually end: outside every immovable
/// body, with room for the ship.
///
/// The arrival buffer is measured from the clicked point, and a planet is
/// 10-60k across and a star 40-150k, so clicking one dropped the ship inside
/// its solid collider -- deep in a gravity well that hammered it with crash
/// damage every frame while collision dragged it back out. Bodies are
/// checked when the dash fires, not when it starts charging, because planets
/// move while it charges.
fn dash_arrival_clear_of_bodies(target: Vec2, ship_radius: f32, bodies: &[(Vec2, f32)]) -> Vec2 {
    const MARGIN: f32 = 400.0;
    let mut point = target;
    // A few passes settle a point pushed out of one circle into another.
    for _ in 0..4 {
        let mut moved = false;
        for &(centre, radius) in bodies {
            let clear = radius + ship_radius + MARGIN;
            let offset = point - centre;
            if offset.length_squared() < clear * clear {
                let dir = offset.try_normalize().unwrap_or(Vec2::X);
                point = centre + dir * clear;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    point
}

/// Completes the jump once the charge finishes: teleport, kill momentum,
/// spend the fuel locked in at charge-start, clear the destination.
fn execute_warp_dash(
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut ship_query: Query<(Entity, &mut Transform, &mut Velocity, &MapWarpCharging, Option<&crate::ship::collision::Collider>), With<Ship>>,
    bodies: Query<(&GlobalTransform, &crate::ship::collision::Collider), Without<Ship>>,
    mut fuel_state: ResMut<FuelState>,
    mut pending: ResMut<PendingWarpTarget>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let Ok((entity, mut transform, mut velocity, charging, ship_collider)) = ship_query.single_mut() else { return };
    if !charging.charge_timer.is_finished() {
        return;
    }

    let immovable: Vec<(Vec2, f32)> = bodies
        .iter()
        .filter(|(_, c)| c.mass.is_infinite())
        .flat_map(|(gt, c)| {
            let gt = *gt;
            c.circles.iter().map(move |&(local, r)| (gt.transform_point(local.extend(0.0)).truncate(), r))
        })
        .collect();
    let ship_radius = ship_collider.map(|c| c.bound_center.length() + c.bound_radius).unwrap_or(1000.0);
    let arrival = dash_arrival_clear_of_bodies(charging.target_pos, ship_radius, &immovable);

    // Both ends of a dash are inside the same loaded system, so unlike an
    // interstellar jump the departure flash is genuinely visible -- it marks
    // where you left, which is the only cue that you moved rather than that
    // the camera cut.
    let from = transform.translation.truncate();
    crate::vfx::particles::spawn_warp_flash(
        &mut commands, &fx, from, 150.0, Color::srgb(0.45, 0.70, 1.0),
    );

    transform.translation.x = arrival.x;
    transform.translation.y = arrival.y;
    velocity.0 = Vec2::ZERO;

    crate::vfx::particles::spawn_warp_flash(
        &mut commands, &fx, arrival, 150.0, Color::srgb(0.55, 0.78, 1.0),
    );
    fuel_state.current_fuel = (fuel_state.current_fuel - charging.fuel_cost).max(0.0);
    pending.0 = None;

    commands.entity(entity).remove::<MapWarpCharging>();

    notifications.write(ShowNotification {
        message: "Warp dash complete.".into(),
        notification_type: NotificationType::Success,
        duration: 4.0,
    });
}

// ============================================================================
// MAIN MENU SCREEN
// ============================================================================

fn spawn_main_menu(mut commands: Commands) {
    use theme::*;

    commands.spawn((
        (Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(ThemeSpacing::SECTION),
                ..default()
            }, BackgroundColor(ThemeColors::BG_MENU), ZIndex(100)),
        MainMenuOverlay,
    )).with_children(|parent| {
        // Title container
        parent.spawn((Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::new(Val::Px(80.0), Val::Px(80.0), Val::Px(ThemeSpacing::XXL), Val::Px(ThemeSpacing::XXL)),
                row_gap: Val::Px(ThemeSpacing::MD),
                ..default()
            })).with_children(|title_box| {
            // Top accent line
            title_box.spawn((Node { width: Val::Px(240.0), height: Val::Px(1.0), margin: UiRect::bottom(Val::Px(ThemeSpacing::LG)), ..default() }, BackgroundColor(ThemeColors::BORDER_BRIGHT)));

            title_box.spawn((Text::new("DEPTHS BELOW"), TextFont { font_size: FontSize::Px(ThemeFonts::DISPLAY), ..default() }, TextColor(ThemeColors::ACCENT_BLUE)));

            title_box.spawn((Text::new("Into the Void"), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY)));

            // Bottom accent line
            title_box.spawn((Node { width: Val::Px(240.0), height: Val::Px(1.0), margin: UiRect::top(Val::Px(ThemeSpacing::LG)), ..default() }, BackgroundColor(ThemeColors::BORDER_BRIGHT)));
        });

        // Actions container — clickable buttons (keyboard shortcuts still work:
        // Enter = New Expedition, L+1/2/3/0 = Load).
        parent.spawn((Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(ThemeSpacing::MD),
                ..default()
            })).with_children(|actions| {
            use menu_buttons::{spawn_menu_button, MenuAction};

            spawn_menu_button(actions, "NEW EXPEDITION", Some("Enter"), ThemeColors::ACCENT_BLUE, MenuAction::NewGame);

            // One Load button per existing save slot.
            let slots = crate::meta::get_save_slots();
            for (slot, info) in &slots {
                if let Some(info) = info {
                    let name = if *slot == 99 { "Auto".to_string() } else { format!("Slot {}", slot + 1) };
                    let key = if *slot == 99 { "L+0" } else { match slot { 0 => "L+1", 1 => "L+2", 2 => "L+3", _ => "L+?" } };
                    let time_min = (info.play_time / 60.0) as i32;
                    let time_sec = (info.play_time % 60.0) as i32;
                    let label = format!("LOAD - {} ({} - {}:{:02})", name, format_range_km(info.depth), time_min, time_sec);
                    spawn_menu_button(actions, &label, Some(key), ThemeColors::ACCENT_GREEN, MenuAction::LoadSlot(*slot));
                }
            }

            spawn_menu_button(actions, "SETTINGS", None, ThemeColors::ACCENT_PURPLE, MenuAction::OpenSettings);
            spawn_menu_button(actions, "QUIT", None, ThemeColors::ACCENT_RED, MenuAction::QuitToDesktop);
        });

        // Tagline
        parent.spawn((Text::new("An expedition went out before you. Find out how far it got."), TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::TEXT_MUTED)));

        // Version / flavor
        parent.spawn((Text::new("They left records the whole way. Follow them."), TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() }, TextColor(Color::srgba(0.25, 0.28, 0.35, 0.6))));

        // Required attribution. assets/audio/CREDITS.md puts most of the sound
        // under CC-BY 3.0, which obliges a visible credit, and the game had
        // none anywhere — an outstanding licence breach rather than an
        // oversight. On the title screen rather than buried behind a button,
        // because that is the page everyone actually sees.
        parent.spawn((
            Text::new("Sound effects by Little Robot Sound Factory (littlerobotsoundfactory.com), CC-BY 3.0"),
            TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
            TextColor(Color::srgba(0.22, 0.25, 0.31, 0.55)),
            Node { margin: UiRect::top(Val::Px(ThemeSpacing::MD)), ..default() },
        ));
    });
}

/// The flight HUD has no business on the title screen.
///
/// It was faintly visible behind the menu -- a hull bar and an ammo count for
/// a ship that does not exist yet -- because the menu's background sits at 98%
/// opacity and nothing ever hid the instruments.
fn hide_hud(mut hud: Query<&mut Visibility, With<HudRoot>>) {
    for mut v in hud.iter_mut() {
        *v = Visibility::Hidden;
    }
}

fn show_hud(mut hud: Query<&mut Visibility, With<HudRoot>>) {
    for mut v in hud.iter_mut() {
        *v = Visibility::Inherited;
    }
}

fn despawn_main_menu(
    mut commands: Commands,
    query: Query<Entity, With<MainMenuOverlay>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// GAME OVER SCREEN
// ============================================================================

fn spawn_game_over_screen(
    mut commands: Commands,
    statistics: Res<Statistics>,
    victory_state: Res<VictoryState>,
    death_cause: Res<crate::resources::DeathCause>,
) {
    use theme::*;

    let is_victory = victory_state.achieved;

    commands.spawn((
        (Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(ThemeSpacing::XXL),
                ..default()
            }, BackgroundColor(ThemeColors::BG_VOID)),
        // Above the floating windows and the pause menu (ZIndex 100).
        ZIndex(200),
        GameOverOverlay,
    )).with_children(|parent| {
        // Title
        if is_victory {
            parent.spawn((Text::new("VICTORY"), TextFont { font_size: FontSize::Px(ThemeFonts::DISPLAY), ..default() }, TextColor(ThemeColors::ACCENT_GREEN)));
            parent.spawn((Text::new("You reached the deepest void and uncovered the truth."), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::TEXT_TITLE)));
        } else {
            parent.spawn((Text::new("LOST IN SPACE"), TextFont { font_size: FontSize::Px(ThemeFonts::DISPLAY), ..default() }, TextColor(ThemeColors::ACCENT_RED)));
            parent.spawn((Text::new("The void claims another vessel."), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY)));

            // What actually killed the player — the single most useful line
            // on this screen.
            if let Some(cause) = &death_cause.cause {
                parent.spawn((
                    Text::new(cause.clone()),
                    TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() },
                    TextColor(ThemeColors::ACCENT_ORANGE),
                ));
            }
        }

        // Stats panel
        parent.spawn((Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(ThemeSpacing::XXL)),
                row_gap: Val::Px(ThemeSpacing::MD),
                ..default()
            }, BackgroundColor(ThemeColors::BG_CARD))).with_children(|stats| {
            stats.spawn((Text::new("EXPEDITION LOG"), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(ThemeColors::TEXT_MUTED)));

            stats.spawn((Node { width: Val::Px(200.0), height: Val::Px(1.0), ..default() }, BackgroundColor(ThemeColors::BORDER_SUBTLE)));

            let time_min = (statistics.play_time_seconds / 60.0) as i32;
            let time_sec = (statistics.play_time_seconds % 60.0) as i32;

            let stat_items = [
                (format!("Max Distance     {}", format_range_km(statistics.max_depth_reached)), ThemeColors::ACCENT_BLUE),
                (format!("Time Survived    {}:{:02}", time_min, time_sec), ThemeColors::TEXT_PRIMARY),
                (format!("Creatures Slain  {}", statistics.creatures_killed), ThemeColors::ACCENT_ORANGE),
                (format!("Crew Lost        {}", statistics.crew_lost), ThemeColors::ACCENT_RED),
            ];

            for (text, color) in stat_items {
                stats.spawn((Text::new(text), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(color)));
            }

            if !statistics.logs_found.is_empty() {
                stats.spawn((Text::new(format!("Logs Found       {}", statistics.logs_found.len())), TextFont { font_size: FontSize::Px(ThemeFonts::H3), ..default() }, TextColor(ThemeColors::ACCENT_CYAN)));
            }
        });

        // Return button (Enter also works — see game_over_input).
        parent.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            }).with_children(|actions| {
            menu_buttons::spawn_menu_button(
                actions,
                "RETURN TO MAIN MENU",
                Some("Enter"),
                ThemeColors::ACCENT_BLUE,
                menu_buttons::MenuAction::ReturnToMainMenu,
            );
        });
    });
}

fn despawn_game_over_screen(
    mut commands: Commands,
    query: Query<Entity, With<GameOverOverlay>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn game_over_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(GameState::MainMenu);
    }
}

// ============================================================================
// PAUSE MENU
// ============================================================================

fn spawn_pause_menu(
    mut commands: Commands,
    depth_state: Res<DepthState>,
    power_state: Res<PowerState>,
    oxygen_state: Res<OxygenState>,
    hull_state: Res<HullState>,
    // The player's modules only -- an AI ship nearby was counted in too.
    module_query: Query<&Module, Without<crate::ai_ship::components::OwnedByAiShip>>,
) {
    // Count modules per category that are doing their job. `is_active` is a
    // power switch, and only machines start with it on; plating, storage and
    // most control blocks never have one, so counting the flag put
    // "Structural: 0/42" in red on every pause of a perfectly sound ship.
    // A passive block is working while it's intact.
    let mut cat_total: HashMap<ModuleCategory, usize> = HashMap::new();
    let mut cat_active: HashMap<ModuleCategory, usize> = HashMap::new();
    for module in module_query.iter() {
        let cat = module.module_type.category();
        *cat_total.entry(cat).or_insert(0) += 1;
        let working = if crate::ship::starts_active(module.module_type) {
            module.is_active && module.health > 0.0
        } else {
            module.health > 0.0
        };
        if working {
            *cat_active.entry(cat).or_insert(0) += 1;
        }
    }

    info!("Spawning pause menu, modules found: {}", module_query.iter().count());

    commands.spawn((
        (Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            }, BackgroundColor(theme::ThemeColors::BG_VOID), ZIndex(100)),
        PauseMenuOverlay,
    )).with_children(|parent| {
        // Header
        parent.spawn((Text::new("PAUSED"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::H1), ..default() }, TextColor(theme::ThemeColors::TEXT_TITLE)));

        // Vitals line
        let hull_pct = (hull_state.hull_integrity * 100.0) as i32;
        parent.spawn((Text::new(format!(
                "Haven: {}  Hull: {}%  Power: {:.0}/{:.0}",
                format_range_km(depth_state.current_depth), hull_pct,
                // Used / available, like the HUD's PWR readout.
                power_state.total_power_consumption, power_state.total_power_generation,
            )), TextFont { font_size: FontSize::Px(18.0), ..default() }, TextColor(Color::srgb(0.8, 0.8, 0.8))));

        // Module counts by category
        for cat in ModuleCategory::ALL {
            let total = cat_total.get(cat).copied().unwrap_or(0);
            if total == 0 { continue; }
            let active = cat_active.get(cat).copied().unwrap_or(0);
            let color = if active == total {
                Color::srgb(0.0, 1.0, 0.0)
            } else if active > 0 {
                Color::srgb(1.0, 1.0, 0.0)
            } else {
                Color::srgb(1.0, 0.0, 0.0)
            };
            parent.spawn((Text::new(format!("  {}: {}/{} working", cat.name(), active, total)), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(color)));
        }

        use menu_buttons::{spawn_chip_button, spawn_menu_button, MenuAction};

        // Spacer between the status readout and the buttons.
        parent.spawn(Node { height: Val::Px(theme::ThemeSpacing::MD), ..default() });

        // Button column (all keyboard shortcuts below still work too).
        parent.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(theme::ThemeSpacing::MD),
                ..default()
            }).with_children(|actions| {
            use theme::ThemeColors;

            spawn_menu_button(actions, "RESUME", Some("Esc"), ThemeColors::ACCENT_GREEN, MenuAction::Resume);

            let slots = crate::meta::get_save_slots();

            // Save row: one chip per slot (1/2/3).
            actions.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(theme::ThemeSpacing::MD),
                    ..default()
                }).with_children(|row| {
                row.spawn((Text::new("SAVE"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY)));
                for slot in 0u32..3 {
                    spawn_chip_button(row, &format!("{}", slot + 1), ThemeColors::ACCENT_YELLOW, MenuAction::SaveSlot(slot));
                }
            });

            // Load row: one chip per slot that actually has a save.
            let has_saves = slots.iter().any(|(_, info)| info.is_some());
            if has_saves {
                actions.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(theme::ThemeSpacing::MD),
                        ..default()
                    }).with_children(|row| {
                    row.spawn((Text::new("LOAD"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY)));
                    for (slot, info) in &slots {
                        if info.is_some() {
                            let label = if *slot == 99 { "Auto".to_string() } else { format!("{}", slot + 1) };
                            spawn_chip_button(row, &label, ThemeColors::ACCENT_BLUE, MenuAction::LoadSlot(*slot));
                        }
                    }
                });
            }

            spawn_menu_button(actions, "SETTINGS", None, ThemeColors::ACCENT_PURPLE, MenuAction::OpenSettings);
            spawn_menu_button(actions, "QUIT TO MAIN MENU", None, ThemeColors::ACCENT_RED, MenuAction::QuitToMainMenu);
        });

        // Hint
        parent.spawn((Text::new("Esc: Resume  *  P: Modules  *  F1-F3 / L+1-3: quick save & load"), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.5, 0.5, 0.5)), Node { margin: UiRect::top(Val::Px(theme::ThemeSpacing::MD)), ..default() }));
    });
}

fn despawn_pause_menu(
    mut commands: Commands,
    query: Query<Entity, With<PauseMenuOverlay>>,
    panel_query: Query<Entity, With<ModulePanelOverlay>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in panel_query.iter() {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// SAVE/LOAD INPUT (while paused)
// ============================================================================

/// Universal quick-save / quick-load: F5 saves, F9 loads (slot 0). Unlike
/// `save_load_input` (multi-slot, pause-menu only) this runs during normal play
/// so you never have to open a menu — the common-case save/reload loop.
fn quick_save_load_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut save_events: MessageWriter<SaveGameRequest>,
    mut load_events: MessageWriter<LoadGameRequest>,
) {
    if keyboard.just_pressed(KeyCode::F5) {
        save_events.write(SaveGameRequest { slot: 0 });
    }
    if keyboard.just_pressed(KeyCode::F9) {
        load_events.write(LoadGameRequest { slot: 0 });
    }
}

/// Handle F1-F3 to save, L+1-3 to load (also L+0 for auto-save)
fn save_load_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut save_events: MessageWriter<SaveGameRequest>,
    mut load_events: MessageWriter<LoadGameRequest>,
) {
    let l_held = keyboard.pressed(KeyCode::KeyL);

    // Save: F1, F2, F3
    if !l_held {
        if keyboard.just_pressed(KeyCode::F1) {
            save_events.write(SaveGameRequest { slot: 0 });
        }
        if keyboard.just_pressed(KeyCode::F2) {
            save_events.write(SaveGameRequest { slot: 1 });
        }
        if keyboard.just_pressed(KeyCode::F3) {
            save_events.write(SaveGameRequest { slot: 2 });
        }
    }

    // Load: L+1, L+2, L+3, L+0 (auto-save)
    if l_held {
        if keyboard.just_pressed(KeyCode::Digit1) {
            load_events.write(LoadGameRequest { slot: 0 });
        }
        if keyboard.just_pressed(KeyCode::Digit2) {
            load_events.write(LoadGameRequest { slot: 1 });
        }
        if keyboard.just_pressed(KeyCode::Digit3) {
            load_events.write(LoadGameRequest { slot: 2 });
        }
        if keyboard.just_pressed(KeyCode::Digit0) {
            load_events.write(LoadGameRequest { slot: 99 }); // Auto-save slot
        }
    }
}

// ============================================================================
// MODULE MANAGEMENT PANEL (P key while paused)
// ============================================================================

/// Toggles the module management panel on/off with P key
fn toggle_module_panel(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    existing_panel: Query<Entity, With<ModulePanelOverlay>>,
    // Without<OwnedByAiShip>: player-only — AI ships carry Module entities
    // too, unscoped this panel would list AI ship modules alongside the
    // player's own.
    module_query: Query<(Entity, &Module), Without<crate::ai_ship::components::OwnedByAiShip>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyP) {
        return;
    }

    info!("P pressed - toggling module panel");

    // Toggle off if already open
    if let Ok(entity) = existing_panel.single() {
        info!("Closing module panel");
        commands.entity(entity).despawn();
        return;
    }

    // Collect modules grouped by category
    let mut by_cat: HashMap<ModuleCategory, Vec<(Entity, &Module)>> = HashMap::new();
    for (entity, module) in module_query.iter() {
        by_cat.entry(module.module_type.category()).or_default().push((entity, module));
    }

    info!("Opening module panel, {} modules found", module_query.iter().count());

    commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(60.0),
                width: Val::Px(400.0),
                max_height: Val::Percent(80.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(4.0),
                ..default()
            }, BackgroundColor(theme::ThemeColors::BG_PANEL), ZIndex(110)),
        ModulePanelOverlay,
        ModuleListSelection(0),
    )).with_children(|parent| {
        parent.spawn((Text::new("MODULE MANAGEMENT"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::H2), ..default() }, TextColor(theme::ThemeColors::TEXT_TITLE)));

        let mut row_index: usize = 0;
        for cat in ModuleCategory::ALL {
            let Some(modules) = by_cat.get(cat) else { continue };

            // Category header
            parent.spawn((Text::new(format!("--- {} ---", cat.name())), TextFont { font_size: FontSize::Px(theme::ThemeFonts::H3), ..default() }, TextColor(theme::ThemeColors::ACCENT_YELLOW)));

            for &(entity, module) in modules {
                let status = if module.is_active { "[ON] " } else { "[OFF]" };
                let pwr = if module.power_generation > 0.0 {
                    format!("Pwr:+{:.0}", module.power_generation)
                } else if module.power_consumption > 0.0 {
                    format!("Pwr:-{:.0}", module.power_consumption)
                } else {
                    "Pwr:0".to_string()
                };
                let cursor = if row_index == 0 { "> " } else { "  " };
                let text = format!(
                    "{}{} {} - HP:{:.0}/{:.0} {}",
                    cursor, status, module.module_type.name(),
                    module.health, module.max_health, pwr,
                );
                let color = if module.is_active {
                    theme::ThemeColors::STATUS_OK
                } else {
                    theme::ThemeColors::TEXT_MUTED
                };

                parent.spawn((
                    Node {
                        padding: UiRect::new(Val::Px(theme::ThemeSpacing::SM), Val::Px(theme::ThemeSpacing::SM), Val::Px(theme::ThemeSpacing::XS), Val::Px(theme::ThemeSpacing::XS)),
                        margin: UiRect::bottom(Val::Px(theme::ThemeSpacing::XS)),
                        ..default()
                    },
                    BackgroundColor(if row_index == 0 { theme::ThemeColors::BG_ELEVATED } else { theme::ThemeColors::BG_CARD }),
                )).with_children(|card| {
                    card.spawn((
                        (Text::new(&text), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(color)),
                        ModuleListItem(entity),
                    ));
                });
                row_index += 1;
            }
        }

        if row_index == 0 {
            parent.spawn((Text::new("No modules installed"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(theme::ThemeColors::TEXT_MUTED)));
        }

        parent.spawn((Text::new("Up/Down: Select  Enter: Toggle  P: Close"), TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() }, TextColor(theme::ThemeColors::TEXT_MUTED)));
    });
}

/// Handles Up/Down/Enter input on the module panel
fn module_panel_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel_query: Query<&mut ModuleListSelection, With<ModulePanelOverlay>>,
    mut item_query: Query<(&ModuleListItem, &mut Text, &mut TextColor, &ChildOf)>,
    mut card_query: Query<&mut BackgroundColor>,
    mut module_query: Query<&mut Module>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let Ok(mut selection) = panel_query.single_mut() else { return };

    let items: Vec<Entity> = item_query.iter().map(|(item, ..)| item.0).collect();
    let count = items.len();
    if count == 0 { return; }

    let old_idx = selection.0;
    let mut changed = false;

    if keyboard.just_pressed(KeyCode::ArrowUp) {
        selection.0 = if old_idx == 0 { count - 1 } else { old_idx - 1 };
        changed = true;
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        selection.0 = if old_idx + 1 >= count { 0 } else { old_idx + 1 };
        changed = true;
    }

    // Toggle is_active on Enter
    if keyboard.just_pressed(KeyCode::Enter) {
        let target_entity = items[selection.0];
        if let Ok(mut module) = module_query.get_mut(target_entity) {
            module.is_active = !module.is_active;
            let state_str = if module.is_active { "ON" } else { "OFF" };
            notifications.write(ShowNotification {
                message: format!("{} turned {}", module.module_type.name(), state_str),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
            changed = true;
        }
    }

    if !changed { return; }

    // Rebuild text for all rows, and highlight the selected row's card background
    let new_idx = selection.0;
    for (i, (item, mut text, mut text_color, card)) in item_query.iter_mut().enumerate() {
        let Ok(module) = module_query.get(item.0) else { continue };
        let cursor = if i == new_idx { "> " } else { "  " };
        let status = if module.is_active { "[ON] " } else { "[OFF]" };
        let pwr = if module.power_generation > 0.0 {
            format!("Pwr:+{:.0}", module.power_generation)
        } else if module.power_consumption > 0.0 {
            format!("Pwr:-{:.0}", module.power_consumption)
        } else {
            "Pwr:0".to_string()
        };
        text.0 = format!(
            "{}{} {} - HP:{:.0}/{:.0} {}",
            cursor, status, module.module_type.name(),
            module.health, module.max_health, pwr,
        );
        text_color.0 = if module.is_active {
            theme::ThemeColors::STATUS_OK
        } else {
            theme::ThemeColors::TEXT_MUTED
        };
        if let Ok(mut bg) = card_query.get_mut(card.0) {
            bg.0 = if i == new_idx { theme::ThemeColors::BG_ELEVATED } else { theme::ThemeColors::BG_CARD };
        }
    }
}

// ============================================================================
// DOCKING / TRADING MENU (GameState::Docked)
// ============================================================================

/// Service definitions for the docking menu
struct DockingService {
    name: &'static str,
    description: String,
    cost: u32,
    available: bool,
}

/// Every tradeable item, in stable menu order.
const TRADE_GOODS: [ItemType; 7] = [
    ItemType::ScrapMetal,
    ItemType::Crystal,
    ItemType::BioSample,
    ItemType::FuelCell,
    ItemType::RareAlloy,
    ItemType::AncientArtifact,
    ItemType::AmmoCrate,
];

/// Credits per point of module damage at a station.
const MODULE_REPAIR_RATE: f32 = 5.0;

/// Repair order: what keeps the ship alive and moving before what fights.
fn repair_priority(category: ModuleCategory) -> u8 {
    match category {
        ModuleCategory::Power => 0,
        ModuleCategory::LifeSupport => 1,
        ModuleCategory::Propulsion => 2,
        ModuleCategory::Control => 3,
        ModuleCategory::Weapons => 4,
        _ => 5,
    }
}

/// The Repair Modules row: what the credits on hand will buy.
fn repair_modules_row(damage: f32, credits: u32) -> String {
    let full = damage * MODULE_REPAIR_RATE;
    if damage < 0.1 {
        "All modules at full health".to_string()
    } else if credits as f32 >= full {
        format!("Restore all {:.0} damaged HP", damage)
    } else if credits as f32 >= MODULE_REPAIR_RATE {
        format!(
            "All {}c buys {:.0} of {:.0} HP - vital systems first",
            credits,
            credits as f32 / MODULE_REPAIR_RATE,
            damage
        )
    } else {
        format!("{:.0} HP damaged - {:.0}c per HP", damage, MODULE_REPAIR_RATE)
    }
}

/// How many HP to restore on each module, given (priority, damage) per module
/// in query order, the credits on hand and the price per HP. Spends no more
/// than the credits cover, highest priority first.
fn plan_module_repair(damage: &[(u8, f32)], credits: u32, rate: f32) -> Vec<f32> {
    let mut budget = credits as f32 / rate;
    let mut order: Vec<usize> = (0..damage.len()).collect();
    order.sort_by_key(|&i| damage[i].0);
    let mut plan = vec![0.0; damage.len()];
    for i in order {
        if budget <= 0.0 {
            break;
        }
        let fix = damage[i].1.min(budget);
        plan[i] = fix;
        budget -= fix;
    }
    plan
}

/// Sell-cargo choices at a station: index 0 = everything, then one entry
/// per item stack actually held (stable order). Lets the player dump
/// scrap at a Trade Hub while holding artifacts back for the Research
/// Outpost, instead of the old all-or-nothing dump.
fn sell_choices(inventory: &Inventory) -> Vec<Option<ItemType>> {
    let mut choices = vec![None];
    for item in TRADE_GOODS {
        if inventory.items.get(&item).copied().unwrap_or(0) > 0 {
            choices.push(Some(item));
        }
    }
    choices
}

/// Description + unit cost for the current buy-goods choice.
fn buy_row(inventory: &Inventory, station_idx: usize, choice_idx: usize, market: &MarketEvents) -> (String, u32, ItemType) {
    let item = TRADE_GOODS[choice_idx % TRADE_GOODS.len()];
    let price = crate::resources::live_item_buy_price(market, station_idx, item);
    let held = inventory.items.get(&item).copied().unwrap_or(0);
    (
        format!("{} @ {}c each (hold: {})  (Left/Right: browse, Enter: buy 1)", item.name(), price, held),
        price,
        item,
    )
}

/// Description + value for the current sell-cargo choice at this station.
fn sell_row(inventory: &Inventory, station_idx: usize, choice: Option<ItemType>, market: &MarketEvents) -> (String, u32) {
    match choice {
        None => {
            let mut total = 0u32;
            for (item, count) in &inventory.items {
                total += crate::resources::live_item_price(market, station_idx, *item) * count;
            }
            (format!("ALL cargo - {}c  (Left/Right: pick a single stack)", total), total)
        }
        Some(item) => {
            let count = inventory.items.get(&item).copied().unwrap_or(0);
            let price = crate::resources::live_item_price(market, station_idx, item);
            let value = price * count;
            let tag = if market.multiplier(station_idx, item) > 1.0 { "  * SHORTAGE" } else { "" };
            (format!("{}x {} @ {}c each = {}c{}  (Left/Right: cycle)", count, item.name(), price, value, tag), value)
        }
    }
}

/// Price multiplier for a station, from how far its system sits from Haven.
///
/// station_idx is system_id * STATIONS_PER_SYSTEM + slot, so the system is
/// recoverable from the index alone.
fn distance_price_multiplier_for(
    station_idx: usize,
    galaxy: &crate::celestial::resources::GalaxyMap,
) -> f32 {
    let system_id = (station_idx / crate::world::home_base::STATIONS_PER_SYSTEM) as u32;
    galaxy
        .systems
        .iter()
        .find(|s| s.id == system_id)
        .map(|s| crate::world::station_types::distance_price_multiplier(s.galaxy_pos))
        .unwrap_or(1.0)
}

/// One of the player's guns, as the rearm service sees it.
#[derive(Clone, Copy)]
struct RackAmmo {
    entity: Entity,
    ammo: u32,
    max: u32,
    loaded: Option<crate::combat::ammo_types::KineticAmmoType>,
    kind: ModuleType,
    /// Damage per round, which prices a gun built from parts: its shell grows
    /// with the square of its gauge, and so does what it does.
    damage: f32,
}

/// Credits per point of a built gun's shell damage: a 170 mm shell ~2c,
/// 280 mm ~5c, 500 mm ~18c, next to a cannon shell's 1.5c.
const BUILT_SHELL_PRICE_PER_DAMAGE: f32 = 0.02;

impl RackAmmo {
    /// Rounds short of a full magazine. Power-fed guns have no magazine to buy.
    fn short(&self) -> u32 {
        if self.max >= crate::combat::ammo_types::POWER_FED_AMMO { 0 } else { self.max.saturating_sub(self.ammo) }
    }
    /// What one round for this gun costs here, its load and the station's
    /// discount included.
    fn round_cost(&self, discount: f32) -> f32 {
        if self.kind == ModuleType::FiringPiece {
            let type_mult = crate::combat::ammo_types::rearm_price(ModuleType::Cannon, self.loaded, 1)
                / crate::combat::ammo_types::rearm_price(ModuleType::Cannon, None, 1);
            return self.damage * BUILT_SHELL_PRICE_PER_DAMAGE * type_mult * discount;
        }
        crate::combat::ammo_types::rearm_price(self.kind, self.loaded, 1) * discount
    }
}

/// Rounds missing across the rack, and what refilling them all costs.
fn rearm_bill(rack: &[RackAmmo], discount: f32) -> (u32, u32) {
    let rounds = rack.iter().map(RackAmmo::short).sum();
    let cost: f32 = rack.iter().map(|w| w.short() as f32 * w.round_cost(discount)).sum();
    (rounds, cost.ceil() as u32)
}

/// What pressing Rearm will do.
#[derive(Debug, Default, PartialEq)]
struct RearmPlan {
    /// Rounds missing in total.
    wanted: u32,
    /// Rounds that will go in.
    loaded: u32,
    crates_used: u32,
    from_crates: u32,
    cost: u32,
    /// Rounds per gun.
    fills: Vec<(Entity, u32)>,
    /// Cheapest round on the rack, for the "can't afford" message.
    cheapest_round: f32,
}

/// Crates first, each worth 10 rounds and spent on the dearest rounds
/// (a crate is worth more against antimatter than against brass, which is
/// the right way round); then credits, spread across every gun in proportion
/// so a short wallet tops up the whole battery rather than one gun.
///
/// The service used to be all or nothing: below the full bill you got
/// "Not enough credits" and not a single round, which with ammo actually
/// burning would strand a broke captain with empty guns at a station that
/// sells shells.
fn plan_rearm(rack: &[RackAmmo], discount: f32, crates: u32, credits: u32) -> RearmPlan {
    const ROUNDS_PER_CRATE: u32 = 10;
    let mut need: Vec<(Entity, u32, f32)> = rack
        .iter()
        .filter(|w| w.short() > 0)
        .map(|w| (w.entity, w.short(), w.round_cost(discount)))
        .collect();
    let wanted: u32 = need.iter().map(|n| n.1).sum();
    let cheapest_round = need.iter().map(|n| n.2).fold(f32::INFINITY, f32::min);
    let mut plan = RearmPlan { wanted, cheapest_round, ..default() };
    if wanted == 0 {
        return plan;
    }
    let mut fills: std::collections::HashMap<Entity, u32> = std::collections::HashMap::new();

    // Crates, dearest rounds first.
    need.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    plan.crates_used = wanted.div_ceil(ROUNDS_PER_CRATE).min(crates);
    let mut crate_rounds = (plan.crates_used * ROUNDS_PER_CRATE).min(wanted);
    plan.from_crates = crate_rounds;
    for (entity, short, _) in need.iter_mut() {
        let take = crate_rounds.min(*short);
        *short -= take;
        crate_rounds -= take;
        *fills.entry(*entity).or_default() += take;
    }

    // Credits for the rest, the same fraction of every gun's shortfall.
    let bill: f32 = need.iter().map(|(_, short, price)| *short as f32 * price).sum();
    if bill > 0.0 {
        let share = (credits as f32 / bill).min(1.0);
        let mut spent = 0.0f32;
        for (entity, short, price) in &need {
            let rounds = if share >= 1.0 { *short } else { (*short as f32 * share).floor() as u32 };
            spent += rounds as f32 * price;
            *fills.entry(*entity).or_default() += rounds;
        }
        plan.cost = (spent.ceil() as u32).min(credits);
    }

    plan.fills = fills.into_iter().filter(|(_, r)| *r > 0).collect();
    plan.loaded = plan.fills.iter().map(|f| f.1).sum();
    plan
}

fn get_docking_services(
    hull_state: &HullState,
    oxygen_state: &OxygenState,
    fuel_state: &FuelState,
    weapon_query: &Query<(Entity, &Weapon, Option<&crate::building::customization::tuning::SelectedAmmo>, &Module), (Without<Creature>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    crew_count: usize,
    total_berths: u32,
    inventory: &Inventory,
    station_idx: usize,
    market: &MarketEvents,
    far: f32,
) -> Vec<DockingService> {
    // Station identity: repairs/fuel/ammo are cheaper at the right outpost
    // type (Mining/Refuel/Military — see world::station_types). These same
    // multipliers must be applied when charging in docking_menu_input.
    let discounts = crate::world::station_types::service_discounts(
        crate::world::station_types::station_type(station_idx),
    );

    let hull_damage = 1.0 - hull_state.hull_integrity;
    let hull_repair_full_cost = (hull_damage * 500.0 * discounts.hull_repair * far) as u32;
    let scrap_have = inventory.items.get(&ItemType::ScrapMetal).copied().unwrap_or(0);
    let scrap_usable = (hull_repair_full_cost / 50).min(scrap_have);
    let hull_repair_cost = hull_repair_full_cost.saturating_sub(scrap_usable * 50);

    let o2_missing = oxygen_state.max_oxygen - oxygen_state.current_oxygen;
    let o2_cost = (o2_missing * 2.0) as u32;

    // Count weapons that need ammo, and price each gun's refill by the gun and
    // what it has loaded -- a railgun full of antimatter costs multiples of
    // the same railgun full of AP (combat::ammo_types::rearm_price).
    let rack: Vec<RackAmmo> = weapon_query
        .iter()
        .map(|(entity, w, sel, module)| RackAmmo { entity, ammo: w.ammo, max: w.max_ammo, loaded: sel.map(|a| a.0), kind: module.module_type, damage: w.damage })
        .collect();
    let (ammo_needed, ammo_cost) = rearm_bill(&rack, discounts.ammo);

    let hire_full_cost = ((200 + (crew_count as u32) * 50) as f32 * far) as u32;
    let bio_have = inventory.items.get(&ItemType::BioSample).copied().unwrap_or(0);
    let bio_usable = (hire_full_cost / 60).min(bio_have);
    let hire_cost = hire_full_cost.saturating_sub(bio_usable * 60);

    // Sell value: count total sellable items at this station's prices
    let mut sell_value = 0u32;
    for (item_type, count) in &inventory.items {
        sell_value += crate::resources::live_item_price(market, station_idx, *item_type) * count;
    }

    let fuel_missing = fuel_state.max_fuel - fuel_state.current_fuel;
    let fuel_cost = (fuel_missing * 0.5 * discounts.fuel * far) as u32;

    vec![
        DockingService {
            name: "Repair Hull",
            description: format!("Restore hull to 100% (Damage: {:.0}%) - ScrapMetal used first", hull_damage * 100.0),
            cost: hull_repair_cost,
            available: hull_damage > 0.01,
        },
        DockingService {
            name: "Refill Oxygen",
            description: format!("Refill O2 tanks ({:.0}/{:.0})", oxygen_state.current_oxygen, oxygen_state.max_oxygen),
            cost: o2_cost,
            available: o2_missing > 1.0,
        },
        DockingService {
            name: "Refuel",
            description: format!("Fill fuel tanks ({:.0}/{:.0}) - FuelCells used first", fuel_state.current_fuel, fuel_state.max_fuel),
            cost: fuel_cost,
            available: fuel_missing > 1.0,
        },
        DockingService {
            name: "Rearm Weapons",
            description: format!("Resupply {} rounds - AmmoCrates used first", ammo_needed),
            cost: ammo_cost,
            available: ammo_needed > 0,
        },
        DockingService {
            name: "Hire Crew",
            description: format!("Recruit crew ({}/{} berths) - BioSample used first", crew_count, total_berths),
            cost: hire_cost,
            available: (crew_count as u32) < total_berths,
        },
        DockingService {
            name: "Sell Cargo",
            description: sell_row(inventory, station_idx, None, market).0,
            cost: 0,
            available: sell_value > 0,
        },
        DockingService {
            name: "Repair Modules",
            description: "Restore damaged modules - vital systems first if credits run short".to_string(),
            cost: 0, // Calculated dynamically in the input handler
            available: true, // Checked dynamically
        },
        DockingService {
            name: "Buy Goods",
            description: buy_row(inventory, station_idx, 0, market).0,
            cost: 0, // Shown per-choice in the description
            available: true,
        },
        DockingService {
            name: "Undock",
            description: "Return to exploring".to_string(),
            cost: 0,
            available: true,
        },
    ]
}

fn spawn_docking_menu(
    mut commands: Commands,
    hull_state: Res<HullState>,
    oxygen_state: Res<OxygenState>,
    fuel_state: Res<FuelState>,
    weapon_query: Query<(Entity, &Weapon, Option<&crate::building::customization::tuning::SelectedAmmo>, &Module), (Without<Creature>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    crew_query: Query<&CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
    inventory: Res<Inventory>,
    currency: Res<Currency>,
    staffing_state: Res<StaffingState>,
    ship_query: Query<&Transform, With<Ship>>,
    market: Res<MarketEvents>,
    stations: Res<crate::world::home_base::SystemStations>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
    state: Res<State<GameState>>,
) {
    let crew_count = crew_query.iter().count();
    // Escape leaves a settlement berth, but at a station it only closes this
    // menu (handle_menu_input) -- the hint has to say which.
    let escape_hint = if *state.get() == GameState::StationDocked { "Esc  close" } else { "Esc  undock" };
    let station_idx = ship_query.single().ok()
        .and_then(|t| stations.nearest_index(t.translation.truncate()))
        .unwrap_or(0);
    let far = distance_price_multiplier_for(station_idx, &galaxy) * market.service_multiplier();
    let services = get_docking_services(&hull_state, &oxygen_state, &fuel_state, &weapon_query, crew_count, staffing_state.total_berths, &inventory, station_idx, &market, far);

    // Who the station belongs to decides its colours and whether it has the
    // Bar and Comm directory tabs (a faction starport does; a free port has
    // only its market).
    let owner = market.dock_terms.as_ref().and_then(|t| t.faction);
    let accent = owner.map_or(theme::ThemeColors::ACCENT_CYAN, crate::ai_ship::components::faction_map_color);
    let s_type = crate::world::station_types::station_type(station_idx);
    let name = market.dock_terms.as_ref().map_or_else(
        || crate::world::home_base::station_display_name(station_idx),
        |t| t.name.clone(),
    );
    let kind = crate::world::station_types::station_type_name(s_type);
    let subtitle = match (owner, &market.dock_terms) {
        (Some(f), _) => format!("{} starport", crate::ai_ship::components::faction_display_name(f)),
        (None, Some(_)) => format!("Free port  |  {kind}"),
        (None, None) => kind.to_string(),
    };
    let standing_chip: Option<(String, Color)> = market.dock_terms.as_ref().map(|t| {
        let color = match t.standing {
            "Allied" => theme::ThemeColors::ACCENT_GREEN,
            "Friendly" => theme::ThemeColors::STATUS_OK,
            "Neutral" => theme::ThemeColors::TEXT_SECONDARY,
            "Unfriendly" => theme::ThemeColors::ACCENT_ORANGE,
            "Free Port" => theme::ThemeColors::ACCENT_CYAN,
            _ => theme::ThemeColors::ACCENT_RED,
        };
        (t.standing.to_uppercase(), color)
    });

    // Station identity: discounts were baked silently into prices; say them.
    let discounts = crate::world::station_types::service_discounts(s_type);
    let mut perks = Vec::new();
    if let Some(terms) = &market.dock_terms {
        perks.push(if terms.faction.is_some() {
            format!("You pay x{:.2}, they pay you x{:.2}", terms.buy, terms.sell)
        } else if terms.buy <= 1.0 && terms.sell >= 1.0 {
            "Ordinary prices for now; they rise after your first hours out".to_string()
        } else {
            format!("No one's protection: you pay x{:.2}, they pay you x{:.2}", terms.buy, terms.sell)
        });
    }
    if discounts.fuel < 1.0 { perks.push(format!("Fuel -{:.0}%", (1.0 - discounts.fuel) * 100.0)); }
    if discounts.hull_repair < 1.0 { perks.push(format!("Repairs -{:.0}%", (1.0 - discounts.hull_repair) * 100.0)); }
    if discounts.ammo < 1.0 { perks.push(format!("Ammo -{:.0}%", (1.0 - discounts.ammo) * 100.0)); }

    let caption = |text: String| (
        Text::new(text),
        TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
        TextColor(theme::ThemeColors::TEXT_MUTED),
    );

    // A dimmed backdrop (the ship and the station stay visible behind it),
    // and the window centred on it.
    commands.spawn((
        (Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            }, BackgroundColor(Color::srgba(0.0, 0.01, 0.03, 0.72)), ZIndex(100)),
        DockingOverlay,
        DockingMenuSelection(0, 0),
        DockTabState { tab: DockTab::Market, tabs: owner.is_some(), contact: 0 },
    )).with_children(|backdrop| {
        backdrop.spawn((
            Node {
                width: Val::Px(1040.0),
                // A fixed size: the window used to shrink and grow to each
                // tab's contents and jump on screen as you switched.
                height: Val::Px(620.0),
                max_width: Val::Percent(94.0),
                max_height: Val::Percent(88.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(theme::ThemeBorders::DEFAULT)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme::ThemeColors::BG_PANEL),
            BorderColor::all(theme::ThemeColors::BORDER_BRIGHT),
        )).with_children(|window| {
            // The owner's colour, across the top.
            window.spawn((Node { height: Val::Px(3.0), width: Val::Percent(100.0), ..default() }, BackgroundColor(accent)));

            // Header: name and owner; standing and credits.
            window.spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::new(Val::Px(18.0), Val::Px(18.0), Val::Px(12.0), Val::Px(10.0)),
                column_gap: Val::Px(16.0),
                ..default()
            }).with_children(|header| {
                header.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_shrink: 1.0, ..default() })
                    .with_children(|names| {
                        names.spawn((Text::new(name.to_uppercase()), TextFont { font_size: FontSize::Px(theme::ThemeFonts::H2), ..default() }, TextColor(theme::ThemeColors::TEXT_TITLE)));
                        names.spawn((Text::new(subtitle), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() }, TextColor(theme::ThemeColors::TEXT_SECONDARY)));
                    });
                header.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(14.0), flex_shrink: 0.0, ..default() })
                    .with_children(|right| {
                        if let Some((label, color)) = &standing_chip {
                            right.spawn((
                                Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                                BackgroundColor(color.with_alpha(0.12)),
                                BorderColor::all(color.with_alpha(0.7)),
                            )).with_children(|chip| {
                                chip.spawn((Text::new(label.clone()), TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() }, TextColor(*color)));
                            });
                        }
                        right.spawn((
                            Text::new(format!("Credits: {}", currency.credits)),
                            TextFont { font_size: FontSize::Px(theme::ThemeFonts::H3), ..default() },
                            TextColor(theme::ThemeColors::ACCENT_YELLOW),
                            DockingCreditsText,
                        ));
                    });
            });

            // Tabs, at a faction starport.
            if owner.is_some() {
                window.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FlexEnd,
                        column_gap: Val::Px(2.0),
                        padding: UiRect::horizontal(Val::Px(14.0)),
                        border: UiRect::bottom(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(theme::ThemeColors::BORDER_DEFAULT),
                )).with_children(|bar| {
                    for tab in DockTab::ALL {
                        bar.spawn((
                            Node { padding: UiRect::axes(Val::Px(16.0), Val::Px(7.0)), border: UiRect::bottom(Val::Px(2.0)), ..default() },
                            BackgroundColor(theme::ThemeColors::BG_CARD),
                            BorderColor::all(Color::NONE),
                            DockTabButton(tab),
                        )).with_children(|b| {
                            b.spawn((Text::new(tab.label()), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY), ..default() }, TextColor(theme::ThemeColors::TEXT_SECONDARY)));
                        });
                    }
                    bar.spawn((Node { margin: UiRect::left(Val::Auto), padding: UiRect::bottom(Val::Px(8.0)), ..default() }, ))
                        .with_children(|hint| { hint.spawn(caption("Q / E  switch tab".into())); });
                });
            }

            // Body: the open tab, and your ship beside it.
            window.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                padding: UiRect::all(Val::Px(14.0)),
                min_height: Val::Px(0.0),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                overflow: Overflow::clip_y(),
                ..default()
            }).with_children(|body| {
                // The market: the service rows, as cards. Row order and
                // numbering must stay exactly as-is -- docking_menu_input
                // matches on these indices throughout.
                body.spawn((
                    Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, row_gap: Val::Px(5.0), ..default() },
                    DockTabPanel(DockTab::Market),
                )).with_children(|list| {
                    for (group_start, group_label) in [(0usize, "Services"), (5, "Trade & upkeep"), (8, "")] {
                        if !group_label.is_empty() {
                            list.spawn((
                                Text::new(group_label.to_uppercase()),
                                TextFont { font_size: FontSize::Px(theme::ThemeFonts::CAPTION), ..default() },
                                TextColor(theme::ThemeColors::TEXT_MUTED),
                                Node { margin: UiRect::top(Val::Px(if group_start == 0 { 0.0 } else { 6.0 })), ..default() },
                            ));
                        }
                        let group_end = match group_start { 0 => 5, 5 => 8, _ => services.len() };
                        for i in group_start..group_end {
                            let service = &services[i];
                            let color = if !service.available { theme::ThemeColors::TEXT_MUTED } else { theme::ThemeColors::TEXT_PRIMARY };
                            list.spawn((
                                Node {
                                    padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(6.0), Val::Px(6.0)),
                                    border: UiRect::left(Val::Px(3.0)),
                                    ..default()
                                },
                                BackgroundColor(theme::ThemeColors::BG_CARD),
                                BorderColor::all(Color::NONE),
                                DockingRowCard(i),
                            )).with_children(|card| {
                                card.spawn((
                                    Text::new(service_title(service.name, service.cost, service.available)),
                                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                                    TextColor(color),
                                    DockingServiceItem(i),
                                )).with_children(|section| {
                                    section.spawn((
                                        TextSpan::new(service_detail(&service.description)),
                                        TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                                        TextColor(theme::ThemeColors::TEXT_SECONDARY),
                                    ));
                                });
                            });
                        }
                    }
                });
                // The Bar and the Comm directory: filled by diplomacy::dock_tabs.
                if owner.is_some() {
                    for tab in [DockTab::Bar, DockTab::Comm] {
                        body.spawn((
                            Node { display: Display::None, flex_direction: FlexDirection::Column, flex_grow: 1.0, row_gap: Val::Px(8.0), ..default() },
                            DockTabPanel(tab),
                        ));
                    }
                }

                // Your ship, and what's in the hold.
                body.spawn(Node { flex_direction: FlexDirection::Column, width: Val::Px(270.0), flex_shrink: 0.0, row_gap: Val::Px(10.0), ..default() })
                    .with_children(|side| {
                        let card = |side: &mut ChildSpawnerCommands, title: &str, lines: Vec<(String, Color)>| {
                            side.spawn((
                                Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), padding: UiRect::all(Val::Px(10.0)), ..default() },
                                BackgroundColor(theme::ThemeColors::BG_CARD),
                            )).with_children(|c| {
                                c.spawn(caption(title.to_uppercase()));
                                for (line, color) in lines {
                                    c.spawn((Text::new(line), TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() }, TextColor(color)));
                                }
                            });
                        };
                        let pct = |v: f32| format!("{:.0}%", v * 100.0);
                        card(side, "Your ship", vec![
                            (format!("Hull       {}", pct(hull_state.hull_integrity)), if hull_state.hull_integrity < 0.5 { theme::ThemeColors::ACCENT_RED } else { theme::ThemeColors::ACCENT_GREEN }),
                            (format!("Fuel       {:.0} / {:.0}", fuel_state.current_fuel, fuel_state.max_fuel), theme::ThemeColors::ACCENT_ORANGE),
                            (format!("Oxygen     {:.0} / {:.0}", oxygen_state.current_oxygen, oxygen_state.max_oxygen), theme::ThemeColors::ACCENT_CYAN),
                            (format!("Crew       {} / {}", crew_count, staffing_state.total_berths), theme::ThemeColors::TEXT_PRIMARY),
                        ]);
                        // Filled by diplomacy::dock_tabs::refresh_dock_hold, so
                        // what you buy or are handed shows straight away.
                        side.spawn((
                            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), padding: UiRect::all(Val::Px(10.0)), ..default() },
                            BackgroundColor(theme::ThemeColors::BG_CARD),
                        )).with_children(|c| {
                            c.spawn(caption("CARGO HOLD".to_string()));
                            c.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }, DockHoldList));
                        });
                        if !perks.is_empty() {
                            card(side, "Terms", perks.iter().map(|p| (p.clone(), theme::ThemeColors::TEXT_SECONDARY)).collect());
                        }
                    });
            });

            // Keys.
            window.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)),
                    border: UiRect::top(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme::ThemeColors::BG_CARD),
                BorderColor::all(theme::ThemeColors::BORDER_SUBTLE),
            )).with_children(|footer| {
                footer.spawn((
                    Text::new(format!("Up/Down  select    Left/Right  cargo choice    Enter  buy    {escape_hint}")),
                    TextFont { font_size: FontSize::Px(theme::ThemeFonts::BODY_SMALL), ..default() },
                    TextColor(theme::ThemeColors::TEXT_SECONDARY),
                    DockFooterHint,
                ));
            });
        });
    });
}

/// A service card's first line: its name, and its price when it has one.
fn service_title(name: &str, cost: u32, available: bool) -> String {
    if cost > 0 && available {
        format!("{name}  -  {cost} cr")
    } else {
        name.to_string()
    }
}

/// A service card's second line.
fn service_detail(description: &str) -> String {
    format!("\n{description}")
}

/// The selected service card is lit; the rest sit back.
fn highlight_dock_rows(
    menu: Query<&DockingMenuSelection, With<DockingOverlay>>,
    mut cards: Query<(&DockingRowCard, &mut BackgroundColor, &mut BorderColor)>,
) {
    let Ok(selection) = menu.single() else { return };
    for (card, mut bg, mut border) in cards.iter_mut() {
        let on = card.0 == selection.0;
        let want_bg = if on { theme::ThemeColors::BG_ELEVATED } else { theme::ThemeColors::BG_CARD };
        if bg.0 != want_bg {
            bg.0 = want_bg;
            *border = BorderColor::all(if on { theme::ThemeColors::ACCENT_CYAN } else { Color::NONE });
        }
    }
}

/// Top of the right-hand HUD column (contract tracker, then notifications):
/// just below the top bar. Only the first frame's guess -- `pin_below_top_bar`
/// replaces it with the bar's measured height once layout has run.
pub const RIGHT_COLUMN_TOP: f32 = 72.0;

/// Gap left between the top bar's lower edge and anything pinned under it.
const BELOW_TOP_BAR_GAP: f32 = 6.0;

/// The cursor in UI units -- what a `Val::Px` offset means.
///
/// `Window::cursor_position` is logical pixels, but every `Val::Px` is
/// multiplied by `UiScale`, so at any UI scale but 100% a node placed at the
/// raw cursor drew somewhere else: the crosshair, tooltips, dragged windows
/// and the radial menu all ran away from the mouse at the Settings' 120%.
pub fn cursor_in_ui(window: &Window, ui_scale: &bevy::ui::UiScale) -> Option<Vec2> {
    window.cursor_position().map(|p| p / ui_scale.0.max(0.01))
}

/// The vitals/nav/resources strip across the top of the HUD.
#[derive(Component)]
pub struct HudTopBar;

/// An absolutely positioned panel that should sit just under the top bar.
///
/// The bar's height is not a constant: its nav cluster stacks a label, a
/// value and a caption, and it grew to 65px when the caption lines landed.
/// The right column (pinned at 48) and the training card (at 46) were never
/// moved, so both covered the bar's bottom line -- the zone name, the gravity
/// readout -- for as long as either was on screen.
#[derive(Component)]
pub struct BelowTopBar;

/// Keeps every `BelowTopBar` panel under the top bar's measured lower edge.
fn pin_below_top_bar(
    bar: Query<&ComputedNode, With<HudTopBar>>,
    mut pinned: Query<&mut Node, With<BelowTopBar>>,
) {
    let Ok(bar) = bar.single() else { return };
    // ComputedNode is physical pixels; Node offsets are logical.
    let height = bar.size.y * bar.inverse_scale_factor;
    if height <= 0.0 {
        return;
    }
    let top = Val::Px(height + BELOW_TOP_BAR_GAP);
    for mut node in &mut pinned {
        if node.top != top {
            node.top = top;
        }
    }
}

/// The right-hand column the contract tracker and the notifications stack in.
#[derive(Component)]
pub struct RightColumn;

/// Selling cargo, refuelling, rearming and repairs all live in the services
/// menu, and only a settlement berth ever opened it. A station -- where the
/// tutorial sends you to sell your haul, and whose cargo panel says "Sell in
/// station services" -- had a Shop button on its toolbar wired to U, and U
/// had opened nothing there since the old upgrade shop was removed. Set by
/// `station_services_key`, read by `spawn_docking_menu`'s run condition.
#[derive(Resource, Default)]
struct StationServicesRequested(bool);

/// U at a station berth opens the services menu, or closes it if it's up.
fn station_services_key(
    keyboard: Res<ButtonInput<KeyCode>>,
    build_state: Res<State<BuildState>>,
    mut commands: Commands,
    open: Query<Entity, With<DockingOverlay>>,
    mut requested: ResMut<StationServicesRequested>,
) {
    if !keyboard.just_pressed(KeyCode::KeyU) || *build_state.get() != BuildState::Inactive {
        return;
    }
    if open.is_empty() {
        requested.0 = true;
    } else {
        for entity in &open {
            commands.entity(entity).despawn();
        }
    }
}

fn clear_station_services_request(mut requested: ResMut<StationServicesRequested>) {
    requested.0 = false;
}

fn despawn_docking_menu(
    mut commands: Commands,
    query: Query<Entity, With<DockingOverlay>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn docking_menu_input(
    // Bevy caps a system at 16 params and this one was exactly at it, so the
    // additions are grouped -- same trick `econ_state` below already uses.
    crew_art: (Res<AssetServer>, Res<crate::crew::animation::CrewAtlases>),
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut menu_query: Query<(&mut DockingMenuSelection, Option<&DockTabState>), With<DockingOverlay>>,
    // Grouped: this system is at Bevy's 16-param cap (see the note above), so
    // the credits readout rides along with the service rows it sits beside.
    mut menu_text: (
        Query<(&DockingServiceItem, &mut Text, &mut TextColor, &Children)>,
        Query<&mut Text, (With<DockingCreditsText>, Without<DockingServiceItem>)>,
    ),
    mut span_query: Query<&mut TextSpan>,
    econ_state: (ResMut<HullState>, ResMut<OxygenState>, ResMut<FuelState>, ResMut<Currency>, ResMut<Inventory>),
    mut weapon_query: Query<(Entity, &mut Weapon, Option<&crate::building::customization::tuning::SelectedAmmo>), (Without<Creature>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    // All three MUST exclude AI ships. Unscoped, the docking menu counted
    // enemy crew into the hire price and the berth check (a live run showed
    // "Recruit crew (25/20 berths)" at 1450c with twenty crew aboard),
    // full-healed every AI ship in the system on the player's credits when
    // they bought Repair Hull, and charged them for enemy battle damage under
    // Repair Modules.
    crew_query: Query<&CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
    mut notifications: MessageWriter<ShowNotification>,
    mut next_state: ResMut<NextState<GameState>>,
    mut hull_query: Query<&mut HullSegment, Without<crate::ai_ship::components::OwnedByAiShip>>,
    staffing_state: Res<StaffingState>,
    mut module_query: Query<&mut Module, Without<crate::ai_ship::components::OwnedByAiShip>>,
    ship_query: Query<&Transform, With<Ship>>,
    world_ctx: (
        Res<MarketEvents>,
        Res<crate::world::home_base::SystemStations>,
        Res<crate::celestial::resources::GalaxyMap>,
    ),
) {
    let (assets, crew_atlases) = crew_art;
    let (market, stations, galaxy) = world_ctx;
    let (mut hull_state, mut oxygen_state, mut fuel_state, mut currency, mut inventory) = econ_state;

    // Keep the menu's own credit readout honest. It is spawned once and
    // everything below can spend, so without this it reports the balance from
    // whenever the menu happened to open.
    if let Ok(mut text) = menu_text.1.single_mut() {
        let want = format!("Credits: {}", currency.credits);
        if **text != want {
            **text = want;
        }
    }

    let Ok((mut selection, tabs)) = menu_query.single_mut() else { return };
    // The Bar and the Comm directory have their own keys (dock_tabs).
    if tabs.is_some_and(|t| t.tab != DockTab::Market) {
        return;
    }

    let station_idx = ship_query.single().ok()
        .and_then(|t| stations.nearest_index(t.translation.truncate()))
        .unwrap_or(0);
    // Must match the multipliers used for the displayed costs in
    // get_docking_services / the refresh block below. That now includes the
    // distance multiplier: quote one price and charge another and the menu is
    // lying, which is worse than it being cheap.
    let far = distance_price_multiplier_for(station_idx, &galaxy) * market.service_multiplier();
    let discounts = crate::world::station_types::service_discounts(
        crate::world::station_types::station_type(station_idx),
    );

    let service_count = 9usize;
    let old_idx = selection.0;
    let mut changed = false;

    if keyboard.just_pressed(KeyCode::ArrowUp) {
        selection.0 = if old_idx == 0 { service_count - 1 } else { old_idx - 1 };
        changed = true;
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        selection.0 = if old_idx + 1 >= service_count { 0 } else { old_idx + 1 };
        changed = true;
    }

    // Left/Right cycles the cargo choice while Sell Cargo (5) or Buy
    // Goods (7) is selected — what to sell (ALL or one held stack) or
    // which good to buy, both priced for this station.
    if selection.0 == 5 || selection.0 == 7 {
        let len = if selection.0 == 5 {
            sell_choices(&inventory).len()
        } else {
            TRADE_GOODS.len()
        };
        if selection.1 >= len {
            selection.1 = 0;
        }
        if keyboard.just_pressed(KeyCode::ArrowRight) {
            selection.1 = (selection.1 + 1) % len;
            changed = true;
        }
        if keyboard.just_pressed(KeyCode::ArrowLeft) {
            selection.1 = (selection.1 + len - 1) % len;
            changed = true;
        }
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        let crew_count = crew_query.iter().count();
        let weapon_read_query_hack: Vec<RackAmmo> = weapon_query.iter()
            .filter_map(|(entity, w, sel)| {
                let kind = module_query.get(entity).ok()?.module_type;
                Some(RackAmmo { entity, ammo: w.ammo, max: w.max_ammo, loaded: sel.map(|a| a.0), kind, damage: w.damage })
            })
            .collect();

        match selection.0 {
            0 => {
                // Repair Hull — ScrapMetal offsets cost (50c value each)
                // before credits, same pattern as Refuel/Rearm's FuelCell/
                // AmmoCrate offset. Checked atomically (compute scrap+credit
                // split, verify affordable, THEN consume both) rather than
                // spending scrap first — repair is all-or-nothing, unlike
                // fuel/ammo's partial fill, so a failed attempt must not
                // waste resources the player can't get back.
                let hull_damage = 1.0 - hull_state.hull_integrity;
                let full_cost = (hull_damage * 500.0 * discounts.hull_repair * far) as u32;
                // Half a percent is where the HUD's rounding flips to 100%.
                // At 0.01 a hull reading 99% was refused as already whole.
                if hull_damage < 0.005 {
                    notifications.write(ShowNotification {
                        message: "Hull already at full integrity".into(),
                        notification_type: NotificationType::Info,
                        duration: 2.0,
                    });
                } else {
                    const SCRAP_VALUE: u32 = 50;
                    let scrap_have = inventory.items.get(&ItemType::ScrapMetal).copied().unwrap_or(0);
                    let scrap_used = (full_cost / SCRAP_VALUE).min(scrap_have);
                    let cost = full_cost.saturating_sub(scrap_used * SCRAP_VALUE);
                    if currency.credits >= cost {
                        if scrap_used > 0 {
                            inventory.remove_item(ItemType::ScrapMetal, scrap_used);
                        }
                        currency.credits -= cost;
                        hull_state.hull_integrity = 1.0;
                        // Also repair all hull segments
                        for mut segment in hull_query.iter_mut() {
                            segment.health = segment.max_health;
                            segment.is_depressurized = false;
                            segment.depressurization_level = 0.0;
                        }
                        let message = if scrap_used > 0 {
                            format!("Hull repaired! Used {} ScrapMetal (-{}c)", scrap_used, cost)
                        } else {
                            format!("Hull repaired! (-{}c)", cost)
                        };
                        notifications.write(ShowNotification {
                            message,
                            notification_type: NotificationType::Success,
                            duration: 3.0,
                        });
                        changed = true;
                    } else {
                        notifications.write(ShowNotification {
                            message: format!("Not enough credits (need {}c, have {}c)", cost, currency.credits),
                            notification_type: NotificationType::Warning,
                            duration: 2.0,
                        });
                    }
                }
            }
            1 => {
                // Refill Oxygen
                let o2_missing = oxygen_state.max_oxygen - oxygen_state.current_oxygen;
                let cost = (o2_missing * 2.0) as u32;
                if o2_missing < 1.0 {
                    notifications.write(ShowNotification {
                        message: "Oxygen tanks are full".into(),
                        notification_type: NotificationType::Info,
                        duration: 2.0,
                    });
                } else if currency.credits >= cost {
                    currency.credits -= cost;
                    oxygen_state.current_oxygen = oxygen_state.max_oxygen;
                    notifications.write(ShowNotification {
                        message: format!("Oxygen refilled! (-{}c)", cost),
                        notification_type: NotificationType::Success,
                        duration: 3.0,
                    });
                    changed = true;
                } else {
                    notifications.write(ShowNotification {
                        message: format!("Not enough credits (need {}c, have {}c)", cost, currency.credits),
                        notification_type: NotificationType::Warning,
                        duration: 2.0,
                    });
                }
            }
            2 => {
                // Refuel - first consume FuelCells from inventory (free), then charge for rest
                let fuel_missing = fuel_state.max_fuel - fuel_state.current_fuel;
                if fuel_missing < 1.0 {
                    notifications.write(ShowNotification {
                        message: "Fuel tanks are full".into(),
                        notification_type: NotificationType::Info,
                        duration: 2.0,
                    });
                } else {
                    let mut fuel_added = 0.0f32;
                    // Each FuelCell provides 50 fuel
                    let fuel_cells = inventory.items.get(&ItemType::FuelCell).copied().unwrap_or(0);
                    let cells_needed = ((fuel_missing / 50.0).ceil() as u32).min(fuel_cells);
                    if cells_needed > 0 {
                        let fuel_from_cells = (cells_needed as f32 * 50.0).min(fuel_missing);
                        fuel_state.current_fuel += fuel_from_cells;
                        fuel_added += fuel_from_cells;
                        inventory.remove_item(ItemType::FuelCell, cells_needed);
                        notifications.write(ShowNotification {
                            message: format!("Used {} FuelCells (+{:.0} fuel)", cells_needed, fuel_from_cells),
                            notification_type: NotificationType::Info,
                            duration: 2.0,
                        });
                    }

                    let remaining_missing = fuel_state.max_fuel - fuel_state.current_fuel;
                    if remaining_missing > 1.0 {
                        // `far` too: the services list posts the price with
                        // it, and the bill has to match the price posted.
                        let per_unit = 0.5 * discounts.fuel * far;
                        let cost = (remaining_missing * per_unit) as u32;
                        // Whatever the credits stretch to, not all or nothing.
                        // A captain 100c short of a full tank used to get no
                        // fuel at all, and a playtest sat at one station with
                        // 436c and a near-empty tank for fifteen minutes.
                        let affordable = (currency.credits as f32 / per_unit).floor();
                        if currency.credits >= cost {
                            currency.credits -= cost;
                            fuel_state.current_fuel = fuel_state.max_fuel;
                            notifications.write(ShowNotification {
                                message: format!("Fuel tanks refilled! (-{}c)", cost),
                                notification_type: NotificationType::Success,
                                duration: 3.0,
                            });
                        } else if affordable >= 1.0 {
                            let bought = affordable.min(remaining_missing);
                            let paid = ((bought * per_unit) as u32).min(currency.credits);
                            currency.credits -= paid;
                            fuel_state.current_fuel += bought;
                            notifications.write(ShowNotification {
                                message: format!(
                                    "Bought {:.0} fuel (-{}c) - a full tank is {}c",
                                    bought, paid, cost
                                ),
                                notification_type: NotificationType::Warning,
                                duration: 3.0,
                            });
                        } else {
                            notifications.write(ShowNotification {
                                message: format!("No credits for fuel (a full tank is {}c)", cost),
                                notification_type: NotificationType::Warning,
                                duration: 2.0,
                            });
                        }
                    } else if fuel_added > 0.0 {
                        notifications.write(ShowNotification {
                            message: "Fuel tanks full from FuelCells!".into(),
                            notification_type: NotificationType::Success,
                            duration: 2.0,
                        });
                    }
                    changed = true;
                }
            }
            3 => {
                // Rearm Weapons: AmmoCrates first (10 rounds each, free), then
                // credits for as much of the rest as the wallet covers.
                let plan = plan_rearm(
                    &weapon_read_query_hack,
                    discounts.ammo,
                    inventory.items.get(&ItemType::AmmoCrate).copied().unwrap_or(0),
                    currency.credits,
                );
                if plan.wanted == 0 {
                    notifications.write(ShowNotification {
                        message: "All weapons fully loaded".into(),
                        notification_type: NotificationType::Info,
                        duration: 2.0,
                    });
                } else if plan.loaded == 0 {
                    notifications.write(ShowNotification {
                        message: format!("Not enough credits to rearm (rounds from {:.1}c)", plan.cheapest_round),
                        notification_type: NotificationType::Warning,
                        duration: 2.5,
                    });
                } else {
                    if plan.crates_used > 0 {
                        inventory.remove_item(ItemType::AmmoCrate, plan.crates_used);
                        notifications.write(ShowNotification {
                            message: format!("Used {} AmmoCrates (+{} rounds)", plan.crates_used, plan.from_crates),
                            notification_type: NotificationType::Info,
                            duration: 2.0,
                        });
                    }
                    currency.credits -= plan.cost;
                    for (entity, mut weapon, _) in weapon_query.iter_mut() {
                        if weapon.max_ammo >= crate::combat::ammo_types::POWER_FED_AMMO {
                            // Power-fed: topped up free, never on the bill.
                            weapon.ammo = weapon.max_ammo;
                        } else if let Some(&(_, rounds)) = plan.fills.iter().find(|(e, _)| *e == entity) {
                            weapon.ammo = (weapon.ammo + rounds).min(weapon.max_ammo);
                        }
                    }
                    let (message, kind) = if plan.loaded < plan.wanted {
                        (
                            format!("Partly rearmed: {} of {} rounds (-{}c) - credits ran out", plan.loaded, plan.wanted, plan.cost),
                            NotificationType::Warning,
                        )
                    } else if plan.cost > 0 {
                        (format!("Weapons rearmed! {} rounds (-{}c)", plan.loaded, plan.cost), NotificationType::Success)
                    } else {
                        (format!("Weapons rearmed from AmmoCrates! {} rounds", plan.loaded), NotificationType::Success)
                    };
                    notifications.write(ShowNotification { message, notification_type: kind, duration: 3.0 });
                    changed = true;
                }
            }
            4 => {
                // Hire Crew — gated by available berths
                let total_berths = staffing_state.total_berths as usize;
                if crew_count >= total_berths {
                    notifications.write(ShowNotification {
                        message: "No available berths! Build more quarters.".into(),
                        notification_type: NotificationType::Warning,
                        duration: 2.0,
                    });
                } else {
                    // BioSample offsets hiring cost first (60c value each —
                    // medical/ration supplies for the new hire) — same
                    // atomic check-then-spend pattern as Repair Hull's
                    // ScrapMetal offset, since hiring is all-or-nothing too.
                    let full_cost = ((200 + (crew_count as u32) * 50) as f32 * far) as u32;
                    const BIOSAMPLE_VALUE: u32 = 60;
                    let bio_have = inventory.items.get(&ItemType::BioSample).copied().unwrap_or(0);
                    let bio_used = (full_cost / BIOSAMPLE_VALUE).min(bio_have);
                    let cost = full_cost.saturating_sub(bio_used * BIOSAMPLE_VALUE);
                    if currency.credits >= cost {
                        if bio_used > 0 {
                            inventory.remove_item(ItemType::BioSample, bio_used);
                        }
                        currency.credits -= cost;
                        let crew_names = ["Morgan", "Rivera", "Chen", "Volkov", "Okafor", "Tanaka", "Andersen", "Reyes",
                                          "Park", "Santos", "Becker", "Ito", "Larsen", "Novak", "Gupta", "Patel"];
                        let name = crew_names[crew_count % crew_names.len()].to_string();

                        // Spawn with SpriteBundle; reconcile_hired_crew system
                        // will parent to ship and add to CrewRoster
                        commands.spawn((
                            crate::crew::animation::crew_sprite(&assets, &crew_atlases),
                            Transform::from_xyz(
                                (crew_count as f32 - 3.5) * 20.0,
                                0.0,
                                crate::crew::walking::CREW_Z,
                            ),
                            CrewMember {
                                name: name.clone(),
                                health: 100.0,
                                max_health: 100.0,
                                oxygen: 100.0,
                                morale: 80.0,
                                state: CrewState::Idle,
                            },
                        ));

                        let message = if bio_used > 0 {
                            format!("{} joined the crew! Used {} BioSample (-{}c) ({}/{} berths)",
                                name, bio_used, cost, crew_count + 1, total_berths)
                        } else {
                            format!("{} joined the crew! (-{}c) ({}/{} berths)",
                                name, cost, crew_count + 1, total_berths)
                        };
                        notifications.write(ShowNotification {
                            message,
                            notification_type: NotificationType::Success,
                            duration: 3.0,
                        });
                        changed = true;
                    } else {
                        notifications.write(ShowNotification {
                            message: format!("Not enough credits (need {}c, have {}c)", cost, currency.credits),
                            notification_type: NotificationType::Warning,
                            duration: 2.0,
                        });
                    }
                }
            }
            5 => {
                // Sell Cargo — whatever the Left/Right choice says:
                // everything, or one specific stack.
                let choices = sell_choices(&inventory);
                let choice = choices.get(selection.1).copied().flatten();
                match choice {
                    None => {
                        let mut total_value = 0u32;
                        for (item_type, count) in &inventory.items {
                            total_value += crate::resources::live_item_price(&market, station_idx, *item_type) * count;
                        }
                        if total_value == 0 {
                            notifications.write(ShowNotification {
                                message: "No cargo to sell".into(),
                                notification_type: NotificationType::Info,
                                duration: 2.0,
                            });
                        } else {
                            currency.credits += total_value;
                            inventory.items.clear();
                            inventory.current_weight = 0.0;
                            notifications.write(ShowNotification {
                                message: format!("Sold all cargo for {}c!", total_value),
                                notification_type: NotificationType::Success,
                                duration: 3.0,
                            });
                            changed = true;
                        }
                    }
                    Some(item) => {
                        let count = inventory.items.get(&item).copied().unwrap_or(0);
                        if count == 0 {
                            notifications.write(ShowNotification {
                                message: format!("No {} in the hold.", item.name()),
                                notification_type: NotificationType::Info,
                                duration: 2.0,
                            });
                        } else {
                            let price = crate::resources::live_item_price(&market, station_idx, item);
                            let value = price * count;
                            inventory.remove_item(item, count);
                            currency.credits += value;
                            selection.1 = 0;
                            notifications.write(ShowNotification {
                                message: format!("Sold {}x {} for {}c ({}c each)", count, item.name(), value, price),
                                notification_type: NotificationType::Success,
                                duration: 3.0,
                            });
                            changed = true;
                        }
                    }
                }
            }
            6 => {
                // Repair Modules. As much as the credits cover, most vital
                // systems first -- it was all-or-nothing, so a captain a few
                // hundred short of the full bill couldn't fix a single reactor.
                let damage: Vec<(u8, f32)> = module_query
                    .iter()
                    .map(|m| (repair_priority(m.module_type.category()), (m.max_health - m.health).max(0.0)))
                    .collect();
                let total_damage: f32 = damage.iter().map(|(_, d)| d).sum();
                let plan = plan_module_repair(&damage, currency.credits, MODULE_REPAIR_RATE);
                let repaired: f32 = plan.iter().sum();
                if total_damage < 0.1 {
                    notifications.write(ShowNotification {
                        message: "All modules at full health".into(),
                        notification_type: NotificationType::Info,
                        duration: 2.0,
                    });
                } else if repaired < 1.0 {
                    notifications.write(ShowNotification {
                        message: format!("Not enough credits to start repairs ({}c per HP)", MODULE_REPAIR_RATE),
                        notification_type: NotificationType::Warning,
                        duration: 2.0,
                    });
                } else {
                    let cost = ((repaired * MODULE_REPAIR_RATE).ceil() as u32).min(currency.credits);
                    currency.credits -= cost;
                    for (mut module, amount) in module_query.iter_mut().zip(plan.iter()) {
                        if *amount <= 0.0 {
                            continue;
                        }
                        let was_wrecked = module.health <= 0.0;
                        module.health = (module.health + amount).min(module.max_health);
                        // Back on only if this repair is what revived it. A
                        // module the captain switched off stays off.
                        if was_wrecked && module.health > 0.0 {
                            module.is_active = true;
                        }
                    }
                    let left = total_damage - repaired;
                    notifications.write(ShowNotification {
                        message: if left < 0.5 {
                            format!("All modules repaired! (-{}c)", cost)
                        } else {
                            format!("Repaired {:.0} HP, vital systems first (-{}c). {:.0} HP still damaged.", repaired, cost, left)
                        },
                        notification_type: NotificationType::Success,
                        duration: 3.5,
                    });
                    changed = true;
                }
            }
            7 => {
                // Buy Goods — one unit of the Left/Right choice
                let (_, price, item) = buy_row(&inventory, station_idx, selection.1, &market);
                if currency.credits < price {
                    notifications.write(ShowNotification {
                        message: format!("Not enough credits (need {}c, have {}c)", price, currency.credits),
                        notification_type: NotificationType::Warning,
                        duration: 2.0,
                    });
                } else if !inventory.add_item(item, 1) {
                    notifications.write(ShowNotification {
                        message: "Cargo hold full!".into(),
                        notification_type: NotificationType::Warning,
                        duration: 2.0,
                    });
                } else {
                    currency.credits -= price;
                    notifications.write(ShowNotification {
                        message: format!("Bought 1x {} (-{}c)", item.name(), price),
                        notification_type: NotificationType::Success,
                        duration: 2.0,
                    });
                    changed = true;
                }
            }
            8 => {
                // Undock
                next_state.set(GameState::Exploring);
                notifications.write(ShowNotification {
                    message: "Undocking...".into(),
                    notification_type: NotificationType::Info,
                    duration: 2.0,
                });
                return;
            }
            _ => {}
        }
    }

    if !changed { return; }

    // Rebuild menu text to reflect updated state
    let crew_count = crew_query.iter().count();
    let weapon_data: Vec<RackAmmo> = weapon_query.iter()
        .filter_map(|(entity, w, sel)| {
            let kind = module_query.get(entity).ok()?.module_type;
            Some(RackAmmo { entity, ammo: w.ammo, max: w.max_ammo, loaded: sel.map(|a| a.0), kind, damage: w.damage })
        })
        .collect();

    let hull_damage = 1.0 - hull_state.hull_integrity;
    let hull_repair_full_cost = (hull_damage * 500.0 * discounts.hull_repair * far) as u32;
    let scrap_have = inventory.items.get(&ItemType::ScrapMetal).copied().unwrap_or(0);
    let scrap_usable = (hull_repair_full_cost / 50).min(scrap_have);
    let hull_repair_cost = hull_repair_full_cost.saturating_sub(scrap_usable * 50);
    let o2_missing = oxygen_state.max_oxygen - oxygen_state.current_oxygen;
    let o2_cost = (o2_missing * 2.0) as u32;
    let (ammo_needed, ammo_cost) = rearm_bill(&weapon_data, discounts.ammo);
    let hire_full_cost = ((200 + (crew_count as u32) * 50) as f32 * far) as u32;
    let bio_have = inventory.items.get(&ItemType::BioSample).copied().unwrap_or(0);
    let bio_usable = (hire_full_cost / 60).min(bio_have);
    let hire_cost = hire_full_cost.saturating_sub(bio_usable * 60);
    let (sell_desc, sell_total) = {
        let choices = sell_choices(&inventory);
        let choice = choices.get(selection.1).copied().flatten();
        sell_row(&inventory, station_idx, choice, &market)
    };

    let fuel_missing = fuel_state.max_fuel - fuel_state.current_fuel;
    let fuel_cost = (fuel_missing * 0.5 * discounts.fuel * far) as u32;

    let new_idx = selection.0;
    let service_info: Vec<(&str, String, u32, bool)> = vec![
        ("Repair Hull", format!("Restore hull to 100% (Damage: {:.0}%) - ScrapMetal used first", hull_damage * 100.0), hull_repair_cost, hull_damage > 0.01),
        ("Refill Oxygen", format!("Refill O2 tanks ({:.0}/{:.0})", oxygen_state.current_oxygen, oxygen_state.max_oxygen), o2_cost, o2_missing > 1.0),
        ("Refuel", format!("Fill fuel tanks ({:.0}/{:.0}) - FuelCells used first", fuel_state.current_fuel, fuel_state.max_fuel), fuel_cost, fuel_missing > 1.0),
        ("Rearm Weapons", format!("Resupply {} rounds - AmmoCrates used first", ammo_needed), ammo_cost, ammo_needed > 0),
        ("Hire Crew", format!("Recruit crew ({}/{} berths) - BioSample used first", crew_count, staffing_state.total_berths), hire_cost, (crew_count as u32) < staffing_state.total_berths),
        ("Sell Cargo", sell_desc, 0, sell_total > 0),
        ("Repair Modules", {
            let mut total_module_damage = 0.0f32;
            for module in module_query.iter() {
                if module.health < module.max_health {
                    total_module_damage += module.max_health - module.health;
                }
            }
            // Say what pressing it will do. Short of the full bill it spends
            // everything on hand, and without this the first a captain knew
            // of that was the wallet reading zero.
            repair_modules_row(total_module_damage, currency.credits)
        }, {
            let mut total_module_damage = 0.0f32;
            for module in module_query.iter() {
                if module.health < module.max_health {
                    total_module_damage += module.max_health - module.health;
                }
            }
            (total_module_damage * MODULE_REPAIR_RATE) as u32
        }, module_query.iter().any(|m| m.health < m.max_health)),
        ("Buy Goods", buy_row(&inventory, station_idx, selection.1, &market).0, 0, true),
        ("Undock", "Return to exploring".to_string(), 0, true),
    ];

    for (item, mut text, mut text_color, children) in menu_text.0.iter_mut() {
        let idx = item.0;
        if idx >= service_info.len() { continue; }
        let (name, desc, cost, available) = &service_info[idx];

        // The selected card is lit by highlight_dock_rows; the text only
        // says what the row is and what it costs.
        let color = if !available {
            theme::ThemeColors::TEXT_MUTED
        } else if idx == new_idx {
            Color::WHITE
        } else {
            theme::ThemeColors::TEXT_PRIMARY
        };

        text.0 = service_title(name, *cost, *available);
        text_color.0 = color;
        for child in children.iter() {
            if let Ok(mut span) = span_query.get_mut(child) {
                span.0 = service_detail(desc);
            }
        }
    }
}

// ============================================================================
// LOW HULL WARNING OVERLAY
// ============================================================================

/// Marker for the hull warning overlay
#[derive(Component)]
struct HullWarningOverlay;

/// Pulses a red screen tint when hull integrity drops below 30%.
///
/// A UI node, not a world sprite. It used to be a fixed 2560x1440 world
/// rectangle following the camera, which at the framing zoom (~5.8) covered
/// about a third of each screen axis: a hard-edged red box in the middle of
/// the view rather than a tint. It also outlived the flight state -- docking
/// or dying below 30% left it frozen in the world.
fn update_hull_warning_overlay(
    mut commands: Commands,
    time: Res<Time>,
    hull_state: Res<HullState>,
    mut overlay_query: Query<(Entity, &mut BackgroundColor), With<HullWarningOverlay>>,
) {
    let critical = hull_state.hull_integrity < 0.3;

    if critical {
        let alpha = 0.1 + 0.05 * (time.elapsed_secs() * 6.0).sin();
        if let Ok((_, mut bg)) = overlay_query.single_mut() {
            bg.0 = Color::srgba(1.0, 0.0, 0.0, alpha);
        } else {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.0, 0.0, alpha)),
                // Under every panel and menu, and never in the way of a click.
                GlobalZIndex(-10),
                bevy::picking::Pickable::IGNORE,
                HullWarningOverlay,
            ));
        }
    } else {
        for (entity, _) in overlay_query.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn clear_hull_warning_overlay(mut commands: Commands, overlay_query: Query<Entity, With<HullWarningOverlay>>) {
    for entity in &overlay_query {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod rearm_tests {
    use super::*;

    fn gun(n: u32, kind: ModuleType, ammo: u32, max: u32) -> RackAmmo {
        RackAmmo { entity: Entity::from_raw_u32(n).unwrap(), ammo, max, loaded: None, kind, damage: 50.0 }
    }

    fn starter_empty() -> Vec<RackAmmo> {
        vec![
            gun(1, ModuleType::Cannon, 0, 30),
            gun(2, ModuleType::Gatling, 0, 300),
            gun(3, ModuleType::HeavyMissile, 0, 12),
            gun(4, ModuleType::Laser, 0, 999),
        ]
    }

    #[test]
    fn power_fed_guns_are_never_on_the_bill() {
        let (rounds, _) = rearm_bill(&[gun(4, ModuleType::Laser, 0, 999)], 1.0);
        assert_eq!(rounds, 0);
    }

    #[test]
    fn a_full_wallet_fills_everything() {
        let rack = starter_empty();
        let (rounds, cost) = rearm_bill(&rack, 1.0);
        let plan = plan_rearm(&rack, 1.0, 0, 10_000);
        assert_eq!((plan.wanted, plan.loaded, plan.cost), (rounds, rounds, cost));
    }

    /// Short of the bill you still get rounds -- in every gun, not one --
    /// and are never charged more than you have.
    #[test]
    fn a_built_guns_shells_cost_by_their_damage() {
        let mut small = gun(1, ModuleType::FiringPiece, 0, 10);
        small.damage = 87.0;
        let mut big = gun(2, ModuleType::FiringPiece, 0, 10);
        big.damage = 909.0;
        assert!((small.round_cost(1.0) - 1.74).abs() < 0.01);
        assert!(big.round_cost(1.0) > 10.0 * small.round_cost(1.0) * 0.9);
    }

    #[test]
    fn a_short_wallet_buys_a_partial_rearm() {
        let rack = starter_empty();
        let (_, cost) = rearm_bill(&rack, 1.0);
        let plan = plan_rearm(&rack, 1.0, 0, cost / 2);
        assert!(plan.loaded > 0 && plan.loaded < plan.wanted, "{plan:?}");
        assert!(plan.cost <= cost / 2, "{plan:?}");
        assert_eq!(plan.fills.len(), 3, "every gun gets some: {plan:?}");
    }

    #[test]
    fn crates_go_to_the_dearest_rounds_first() {
        let rack = vec![gun(1, ModuleType::Gatling, 0, 300), gun(2, ModuleType::Railgun, 5, 15)];
        let plan = plan_rearm(&rack, 1.0, 1, 0);
        assert_eq!(plan.from_crates, 10);
        assert_eq!(plan.fills, vec![(Entity::from_raw_u32(2).unwrap(), 10)]);
        assert_eq!(plan.cost, 0);
    }

    #[test]
    fn broke_and_no_crates_loads_nothing() {
        let plan = plan_rearm(&starter_empty(), 1.0, 0, 0);
        assert_eq!((plan.loaded, plan.cost), (0, 0));
    }
}

#[cfg(test)]
mod dash_arrival_tests {
    use super::*;

    #[test]
    fn open_space_arrivals_are_left_alone() {
        let bodies = [(Vec2::new(100_000.0, 0.0), 20_000.0)];
        assert_eq!(dash_arrival_clear_of_bodies(Vec2::new(0.0, 5_000.0), 1000.0, &bodies), Vec2::new(0.0, 5_000.0));
    }

    #[test]
    fn a_dash_into_a_planet_stops_outside_it() {
        let centre = Vec2::new(100_000.0, 0.0);
        let bodies = [(centre, 20_000.0)];
        // Aimed at a point just short of the centre, from the left.
        let got = dash_arrival_clear_of_bodies(Vec2::new(97_000.0, 0.0), 1000.0, &bodies);
        assert!(got.distance(centre) >= 20_000.0 + 1000.0, "{got}");
        assert!(got.x < centre.x, "pushed out on the near side, got {got}");
    }
}

#[cfg(test)]
mod one_warp_key_tests {
    use super::*;
    use crate::celestial::components::WarpCharging;
    use crate::celestial::resources::{GalaxyWarpTarget, PendingGalaxyWarpTarget};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<PendingWarpTarget>()
            .init_resource::<PendingGalaxyWarpTarget>()
            .init_resource::<crate::combat::targeting::selection::TargetSelection>()
            .init_resource::<FuelState>()
            .init_resource::<crate::celestial::resources::GalaxyMap>()
            .init_resource::<crate::celestial::resources::SystemStreamingManager>()
            .add_message::<ShowNotification>()
            .add_message::<crate::celestial::events::WarpJumpStarted>()
            .add_systems(Update, (warp_dash_input, crate::celestial::warp::warp_input_system));
        app.world_mut().spawn((Ship, Transform::default()));
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
        keys.press(key);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
    }

    fn charging(app: &mut App) -> (bool, bool) {
        let dash = app.world_mut().query_filtered::<(), With<MapWarpCharging>>().iter(app.world()).count() == 1;
        let jump = app.world_mut().query_filtered::<(), With<WarpCharging>>().iter(app.world()).count() == 1;
        (dash, jump)
    }

    /// A point on the system map dashes; a pick on the galaxy map jumps.
    /// Either way it is the same key, and a second press cancels.
    #[test]
    fn one_key_goes_wherever_you_last_pointed() {
        let mut app = app();
        app.world_mut().resource_mut::<PendingWarpTarget>().0 = Some(Vec2::new(30_000.0, 0.0));
        press(&mut app, KeyCode::KeyG);
        assert_eq!(charging(&mut app), (true, false), "system-map point: a dash");
        press(&mut app, KeyCode::KeyG);
        assert_eq!(charging(&mut app), (false, false), "second press cancels");

        app.world_mut().resource_mut::<PendingGalaxyWarpTarget>().0 =
            Some(GalaxyWarpTarget::BlindPoint(Vec2::new(200_000.0, 0.0)));
        press(&mut app, KeyCode::KeyG);
        assert_eq!(charging(&mut app), (false, true), "galaxy pick: a jump, not a dash");
        press(&mut app, KeyCode::KeyV);
        assert_eq!(charging(&mut app), (false, false), "V is the same key");
    }
}

#[cfg(test)]
mod warp_pacing_tests {
    use super::*;
    use crate::celestial::warp::interstellar_charge_time;

    /// Representative in-system distances at the scale systems actually have
    /// (see `celestial::galaxy`): a planet from the spawn, the star, and a
    /// far station.
    const TO_PLANET: f32 = 29_000.0;
    const TO_STAR: f32 = 95_000.0;
    const TO_FAR_STATION: f32 = 420_000.0;

    /// The point of having two drives. A reposition inside one system must
    /// stay cheaper than the cheapest jump between stars, or there is no
    /// reason to feel differently about the two, and both have now been
    /// retuned in opposite directions at least once.
    #[test]
    fn crossing_a_system_is_quicker_than_leaving_one() {
        let shortest_jump = interstellar_charge_time(0.0);
        for distance in [TO_PLANET, TO_STAR, TO_FAR_STATION] {
            let local = warp_dash_charge_time(distance);
            assert!(
                local < shortest_jump,
                "a {distance}-unit local dash charges for {local}s, no quicker than the \
                 shortest interstellar jump at {shortest_jump}s"
            );
        }
    }

    /// Reaching the things the generator puts near the spawn should feel like
    /// using a tool. At the old 2.0s base and 15,000/s this was about four
    /// seconds of standing still to cross to a planet.
    #[test]
    fn reaching_a_nearby_planet_is_near_instant() {
        let t = warp_dash_charge_time(TO_PLANET);
        assert!(t < 1.0, "charging {t}s to cross to a planet is a pause, not a dash");
    }

    /// And leaving a system should cost something. At 1-4s it was no more
    /// than crossing a room.
    #[test]
    fn leaving_the_system_is_a_decision() {
        let near = interstellar_charge_time(0.0);
        let far = interstellar_charge_time(1.0);
        assert!(near >= 3.0, "the shortest jump between stars charges for only {near}s");
        assert!(far >= 12.0, "a jump to the far edge of the galaxy charges for only {far}s");
        assert!(far <= 30.0, "{far}s is a loading screen, which is what 6-60s was");
    }

    /// Both drives must cost more for more distance, or the cheapest route is
    /// to pick the furthest target. Never LESS -- the local dash flattens at
    /// `WARP_DASH_MAX_CHARGE` rather than continuing to climb, so the top end
    /// is checked for non-decreasing rather than strictly increasing.
    #[test]
    fn further_never_costs_less() {
        assert!(warp_dash_charge_time(TO_PLANET) < warp_dash_charge_time(TO_STAR));
        assert!(warp_dash_charge_time(TO_STAR) <= warp_dash_charge_time(TO_FAR_STATION));
        assert!(interstellar_charge_time(0.0) < interstellar_charge_time(0.5));
        assert!(interstellar_charge_time(0.5) < interstellar_charge_time(1.0));
    }

    /// The cap is what keeps the two drives from inverting, so it has to stay
    /// under the cheapest jump between stars.
    #[test]
    fn the_local_cap_stays_under_an_interstellar_jump() {
        assert!(
            WARP_DASH_MAX_CHARGE < interstellar_charge_time(0.0),
            "local dashes cap at {WARP_DASH_MAX_CHARGE}s, at or above the {}s shortest jump",
            interstellar_charge_time(0.0)
        );
        // Even an absurd distance stays bounded.
        assert!(warp_dash_charge_time(50_000_000.0) <= WARP_DASH_MAX_CHARGE);
    }
}

#[cfg(test)]
mod map_projection_tests {
    use super::*;

    const PANEL: f32 = 600.0;

    /// Where on the panel a world point is drawn, as the normalised position
    /// `ComputedNode::normalize_point` would report for a click there.
    fn drawn_at(world: Vec2, center: Vec2, range: f32) -> Vec2 {
        let (x, y) = world_to_map_px_raw(world, center, PANEL, range);
        Vec2::new(x / PANEL - 0.5, y / PANEL - 0.5)
    }

    /// The property that matters most. Click the dot you see, and the warp
    /// target must be the world point that dot was drawn for -- at every
    /// scale the map can be fitted to. Draw and click each used to compute
    /// the scale for themselves; if they ever disagree, clicking a planet
    /// sends you somewhere else on the map entirely.
    #[test]
    fn a_click_lands_where_the_dot_was_drawn() {
        let center = Vec2::new(31_000.0, -68_400.0);
        for range in [MAP_WORLD_RANGE, 900_000.0, 1_150_000.0] {
            for world in [
                center,
                center + Vec2::new(250_000.0, 0.0),
                center + Vec2::new(-410_000.0, 330_000.0),
                center + Vec2::new(0.0, -range * 0.9),
            ] {
                let back = map_norm_to_world(drawn_at(world, center, range), center, range);
                assert!(
                    back.distance(world) < 1.0,
                    "range {range}: drew {world}, a click there resolves to {back}"
                );
            }
        }
    }

    /// Up is up. Screen y grows downward and world y upward; flip one side and
    /// not the other and the whole map is mirrored top to bottom.
    #[test]
    fn north_is_drawn_above_the_centre() {
        let c = Vec2::ZERO;
        let (_, y_north) = world_to_map_px_raw(Vec2::new(0.0, 100_000.0), c, PANEL, MAP_WORLD_RANGE);
        let (_, y_centre) = world_to_map_px_raw(c, c, PANEL, MAP_WORLD_RANGE);
        assert!(y_north < y_centre, "a point north of centre was drawn below it");
    }

    /// Orbit dots off the panel must be dropped, not clamped: clamping stacks
    /// every off-map point of an orbit along the border as a solid line. The
    /// raw projection has to actually report "off the map" for that to work.
    #[test]
    fn an_off_map_point_reads_as_off_the_map() {
        let c = Vec2::ZERO;
        let far = Vec2::new(MAP_WORLD_RANGE * 3.0, 0.0);
        let (x, _) = world_to_map_px_raw(far, c, PANEL, MAP_WORLD_RANGE);
        assert!(x > PANEL, "a point three map-widths out was placed at x={x}, on the panel");

        // While the clamped version, used for single markers, pins it to the edge.
        let (xc, _) = world_to_map_px(far, c, PANEL, MAP_WORLD_RANGE);
        assert_eq!(xc, PANEL);
    }

    /// Widening the map for a large system must shrink things, never grow
    /// them. Inverted, a fitted map would zoom IN on the systems that most
    /// need zooming out.
    #[test]
    fn a_wider_map_draws_things_closer_to_centre() {
        let c = Vec2::ZERO;
        let p = Vec2::new(400_000.0, 0.0);
        let (narrow, _) = world_to_map_px_raw(p, c, PANEL, MAP_WORLD_RANGE);
        let (wide, _) = world_to_map_px_raw(p, c, PANEL, 1_150_000.0);
        assert!(
            (wide - PANEL / 2.0) < (narrow - PANEL / 2.0),
            "the wider map drew a point further from centre than the narrow one"
        );
    }
}

#[cfg(test)]
mod crew_hurt_notice_tests {
    use super::*;
    use crate::ai_ship::components::OwnedByAiShip;

    fn hand(name: &str, health: f32) -> CrewMember {
        CrewMember {
            name: name.into(),
            health,
            max_health: 100.0,
            oxygen: 100.0,
            morale: 100.0,
            state: CrewState::Idle,
        }
    }

    fn notices(app: &App) -> Vec<String> {
        app.world()
            .resource::<Messages<ShowNotification>>()
            .iter_current_update_messages()
            .map(|n| n.message.clone())
            .collect()
    }

    /// Slivers of suffocation every frame make one notice, not one per frame.
    #[test]
    fn slow_harm_is_summed_and_rate_limited() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_message::<PowerStateChanged>()
            .add_message::<OxygenStateChanged>()
            .add_message::<HullBreached>()
            .add_message::<CrewDamaged>()
            .add_message::<ShowNotification>()
            .add_systems(Update, handle_game_event_notifications);
        let crew = app.world_mut().spawn(hand("Stross", 80.0)).id();
        let mut seen = 0;
        for _ in 0..300 {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(16));
            app.world_mut().write_message(CrewDamaged { crew, amount: 0.05, source: CrewDamageSource::Suffocation });
            app.update();
            seen += notices(&app).len();
        }
        // 15 HP over ~4.8s: summed into one notice, the rest held back by the gap.
        assert_eq!(seen, 1);
    }

    #[test]
    fn only_our_living_crew_get_a_hurt_notice() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_message::<PowerStateChanged>()
            .add_message::<OxygenStateChanged>()
            .add_message::<HullBreached>()
            .add_message::<CrewDamaged>()
            .add_message::<ShowNotification>()
            .add_systems(Update, handle_game_event_notifications);

        let ours = app.world_mut().spawn(hand("Reyes", 70.0)).id();
        let dead = app.world_mut().spawn(hand("Falk", 0.0)).id();
        let raider = app.world_mut().spawn(Ship).id();
        let theirs = app.world_mut().spawn((hand("Wren", 0.0), OwnedByAiShip { root: raider })).id();
        for (crew, amount) in [(ours, 30.0), (dead, 100.0), (theirs, 100.0)] {
            app.world_mut().write_message(CrewDamaged { crew, amount, source: CrewDamageSource::Decompression });
        }
        app.update();

        assert_eq!(notices(&app), vec!["Reyes hurt - decompression (-30)".to_string()]);
    }
}

#[cfg(test)]
mod station_services_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn press(app: &mut App, key: KeyCode) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
        keys.press(key);
        app.update();
    }

    fn key_app(build: BuildState) -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.insert_state(build);
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<StationServicesRequested>();
        app.add_systems(Update, station_services_key);
        app
    }

    #[test]
    fn u_asks_for_the_services_when_none_are_up() {
        let mut app = key_app(BuildState::Inactive);
        press(&mut app, KeyCode::KeyU);
        assert!(app.world().resource::<StationServicesRequested>().0);
    }

    #[test]
    fn u_closes_the_services_when_they_are_up() {
        let mut app = key_app(BuildState::Inactive);
        let overlay = app.world_mut().spawn(DockingOverlay).id();
        press(&mut app, KeyCode::KeyU);
        assert!(!app.world().resource::<StationServicesRequested>().0);
        assert!(app.world().get_entity(overlay).is_err(), "still open");
    }

    /// Build mode owns the keyboard; U there is not a trip to the shop.
    #[test]
    fn u_in_build_mode_does_nothing() {
        let mut app = key_app(BuildState::Placing);
        press(&mut app, KeyCode::KeyU);
        assert!(!app.world().resource::<StationServicesRequested>().0);
    }

    fn berth_app() -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.insert_state(GameState::StationDocked);
        app.init_state::<BuildState>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<CustomizationState>();
        app.init_resource::<crate::contracts::MissionBoardOpen>();
        app.init_resource::<crate::crew::hiring::HiringBoardOpen>();
        app.init_resource::<PrePauseState>();
        app.init_resource::<crate::tutorial::Tutorial>();
        app.init_resource::<menu_buttons::SettingsMenu>();
        app.add_message::<LoadGameRequest>();
        app.add_message::<NewExpeditionRequest>();
        app.add_systems(Update, handle_menu_input);
        app
    }

    fn pending(app: &App) -> Option<GameState> {
        match app.world().resource::<NextState<GameState>>() {
            NextState::Pending(s) => Some(*s),
            _ => None,
        }
    }

    #[test]
    fn enter_at_a_bare_berth_launches() {
        let mut app = berth_app();
        press(&mut app, KeyCode::Enter);
        assert_eq!(pending(&app), Some(GameState::Exploring));
    }

    /// Hiring binds Enter to "hire". It must not also launch the ship.
    #[test]
    fn hiring_with_enter_does_not_launch() {
        let mut app = berth_app();
        app.world_mut().resource_mut::<crate::crew::hiring::HiringBoardOpen>().0 = true;
        press(&mut app, KeyCode::Enter);
        assert_eq!(pending(&app), None);
    }

    /// Nor does buying in the station services.
    #[test]
    fn buying_with_enter_does_not_launch() {
        let mut app = berth_app();
        app.world_mut().spawn(DockingOverlay);
        press(&mut app, KeyCode::Enter);
        assert_eq!(pending(&app), None);
    }

    /// Escape takes the services down instead of pausing behind them.
    #[test]
    fn escape_closes_the_services_without_pausing() {
        let mut app = berth_app();
        let overlay = app.world_mut().spawn(DockingOverlay).id();
        press(&mut app, KeyCode::Escape);
        assert_eq!(pending(&app), None);
        assert!(app.world().get_entity(overlay).is_err());
    }
}

#[cfg(test)]
mod weapon_rack_tests {
    use super::*;

    #[test]
    fn a_turret_gun_still_coming_round_says_so() {
        let turning = RackWeapon { frac: 1.0, ready: true, remaining: 0.0, ammo: 30, max: 30, alive: true, turning: true, auto: false, power: false, blind: false };
        assert_eq!(rack_line("170 mm Gun", std::slice::from_ref(&turning)).state, "TURNING");
        let on = RackWeapon { turning: false, ..turning };
        assert_eq!(rack_line("170 mm Gun", &[on]).state, "READY");
    }

    fn gun(ready: bool, remaining: f32, alive: bool) -> RackWeapon {
        RackWeapon { frac: if ready { 1.0 } else { 0.5 }, ready, remaining, ammo: 30, max: 30, alive, turning: false, auto: false, power: false, blind: false }
    }

    #[test]
    fn point_defence_guns_say_they_fire_themselves() {
        let pd = RackWeapon { auto: true, ..gun(false, 0.02, true) };
        assert_eq!(rack_line("20 mm Gun", &[pd]).state, "AUTO PD");
        let dry = RackWeapon { auto: true, ammo: 0, ..gun(true, 0.0, true) };
        assert_eq!(rack_line("20 mm Gun", &[dry]).state, "EMPTY");
        let blind = RackWeapon { auto: true, blind: true, ..gun(true, 0.0, true) };
        assert_eq!(rack_line("20 mm Gun", &[blind]).state, "BLIND");
    }

    #[test]
    fn a_stack_reads_as_one_line() {
        let line = rack_line("Cannon", &[gun(true, 0.0, true), gun(true, 0.0, true), gun(true, 0.0, true), gun(true, 0.0, true)]);
        assert_eq!((line.name.as_str(), line.ammo.as_str(), line.state.as_str()), ("Cannon x4", "120/120", "READY"));
    }

    #[test]
    fn partly_reloaded_says_how_many_can_fire() {
        let line = rack_line("Cannon", &[gun(true, 0.0, true), gun(false, 0.7, true), gun(false, 0.3, true)]);
        assert_eq!(line.state, "1/3 RDY");
    }

    #[test]
    fn none_ready_counts_down_to_the_soonest() {
        let line = rack_line("Gatling", &[gun(false, 0.7, true), gun(false, 0.3, true)]);
        assert_eq!(line.state, "0.3s");
    }

    /// A dry magazine is EMPTY, not READY, and a half-dry stack can't all fire.
    #[test]
    fn dry_guns_are_not_ready() {
        let dry = RackWeapon { frac: 1.0, ready: true, remaining: 0.0, ammo: 0, max: 30, alive: true, turning: false, auto: false, power: false, blind: false };
        assert_eq!(rack_line("Cannon", &[dry]).state, "EMPTY");
        let dry = RackWeapon { frac: 1.0, ready: true, remaining: 0.0, ammo: 0, max: 30, alive: true, turning: false, auto: false, power: false, blind: false };
        assert_eq!(rack_line("Cannon", &[gun(true, 0.0, true), dry]).state, "1/2 RDY");
    }

    /// The last sliver of a reload reads 0.1s, never a stuck-looking 0.0s.
    #[test]
    fn countdown_never_reads_zero() {
        assert_eq!(rack_line("Gatling", &[gun(false, 0.02, true)]).state, "0.1s");
    }

    /// Dead guns don't count as loaded or ready; a stack of only dead guns is DOWN.
    #[test]
    fn destroyed_guns_drop_out() {
        assert_eq!(rack_line("Cannon", &[gun(true, 0.0, true), gun(false, 0.0, false)]).state, "READY");
        assert_eq!(rack_line("Cannon", &[gun(false, 0.0, false)]).state, "DOWN");
    }

    /// A power-fed gun has no magazine to count.
    #[test]
    fn power_weapons_show_no_fake_ammo() {
        let beam = RackWeapon { frac: 1.0, ready: true, remaining: 0.0, ammo: 999, max: 999, alive: true, turning: false, auto: false, power: true, blind: false };
        assert_eq!(rack_line("Laser", &[beam]).ammo, "power");
    }
}

#[cfg(test)]
mod module_repair_tests {
    use super::*;

    #[test]
    fn enough_credits_fixes_everything() {
        let plan = plan_module_repair(&[(4, 100.0), (0, 50.0)], 10_000, 5.0);
        assert_eq!(plan, vec![100.0, 50.0]);
    }

    /// Short of the full bill: power first, then whatever is left.
    #[test]
    fn short_credits_fix_vital_systems_first() {
        // 400c at 5c/HP buys 80 HP: the reactor's 50 first, then 30 of the gun's 100.
        let plan = plan_module_repair(&[(4, 100.0), (0, 50.0)], 400, 5.0);
        assert_eq!(plan, vec![30.0, 50.0]);
    }

    /// Short of the full bill, the row says the whole balance goes.
    #[test]
    fn the_row_says_what_the_money_buys() {
        assert_eq!(repair_modules_row(100.0, 10_000), "Restore all 100 damaged HP");
        assert_eq!(repair_modules_row(610.0, 1_688), "All 1688c buys 338 of 610 HP - vital systems first");
        assert_eq!(repair_modules_row(0.0, 50), "All modules at full health");
    }

    #[test]
    fn no_credits_fixes_nothing() {
        assert!(plan_module_repair(&[(0, 50.0)], 0, 5.0).iter().all(|a| *a == 0.0));
    }
}

#[cfg(test)]
mod galaxy_map_tests {
    use super::*;
    use crate::celestial::resources::{GalaxyWarpTarget, SystemDiscovery, SystemStreamingManager};

    fn texts(app: &mut App) -> Vec<String> {
        let mut q = app.world_mut().query::<&Text>();
        q.iter(app.world()).map(|t| t.0.clone()).collect()
    }

    /// The map tells you whose space a system is, what they think of you,
    /// who it borders and what haunts it, and lists the wars it knows of.
    #[test]
    fn the_galaxy_map_shows_territory() {
        let mut map = crate::celestial::galaxy::generate_galaxy_map(42);
        for s in &mut map.systems {
            s.discovery = SystemDiscovery::Located;
        }
        let haunted = map.systems.iter().find(|s| s.haunt.is_some()).unwrap().id;
        let owner = map.systems.iter().find(|s| s.id == haunted).unwrap().faction.unwrap();
        let standing = crate::diplomacy::FactionStanding::default();

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let streaming = SystemStreamingManager::default();
        let fuel = FuelState::default();
        {
            let world = app.world_mut();
            let mut commands = world.commands();
            let war = crate::celestial::territory::BorderWar::default();
            spawn_galaxy_map_overlay(&mut commands, &map, &streaming, Some(GalaxyWarpTarget::System(haunted)), &fuel, &standing, &war, 600.0, GalaxyView::default());
        }
        app.update();
        let all = texts(&mut app);
        let has = |needle: &str| all.iter().any(|t| t.contains(needle));
        let name = crate::ai_ship::components::faction_display_name(owner);
        assert!(has(name), "owner {name} not shown: {all:?}");
        assert!(has("They count you"), "standing not shown: {all:?}");
        assert!(has("Haunted by"), "haunt not shown: {all:?}");
        assert!(has("AT WAR"), "no wars listed: {all:?}");
        // One capital ring per territorial faction, and a red ring per haunt.
        let mut rings = app.world_mut().query::<&Node>();
        let ringed = rings.iter(app.world()).filter(|n| n.border.left == Val::Px(2.0)).count();
        let haunts = map.systems.iter().filter(|s| s.haunt.is_some()).count();
        assert_eq!(ringed, crate::celestial::territory::TERRITORIAL.len() + haunts);
    }

    /// Every system that would shoot at you is named in red on the map, the
    /// others aren't, and it follows your standing, not a fixed list.
    #[test]
    fn the_galaxy_map_names_what_is_hostile() {
        let mut map = crate::celestial::galaxy::generate_galaxy_map(42);
        for s in &mut map.systems {
            s.discovery = SystemDiscovery::Located;
        }
        let draw = |standing: &crate::diplomacy::FactionStanding, target: Option<GalaxyWarpTarget>| {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            {
                let world = app.world_mut();
                let mut commands = world.commands();
                let war = crate::celestial::territory::BorderWar::default();
                spawn_galaxy_map_overlay(&mut commands, &map, &SystemStreamingManager::default(), target, &FuelState::default(), standing, &war, 600.0, GalaxyView::default());
            }
            app.update();
            texts(&mut app)
        };

        let mut standing = crate::diplomacy::FactionStanding::default();
        let all = draw(&standing, None);
        let (hostile, safe): (Vec<_>, Vec<_>) = map.systems.iter().partition(|s| standing.system_hostile(s));
        assert!(!hostile.is_empty() && !safe.is_empty());
        for s in &hostile {
            assert!(all.contains(&s.name), "hostile {} not named: {all:?}", s.name);
        }
        // Haven is in the legend whatever the map says about it.
        for s in safe.iter().filter(|s| s.id != 0) {
            assert!(!all.contains(&s.name), "{} is not hostile but named: {all:?}", s.name);
        }

        // Clicking one says so outright, and making peace takes the mark off.
        let enemy = hostile.iter().find(|s| s.haunt.is_none() && s.faction.is_some()).unwrap();
        let picked = draw(&standing, Some(GalaxyWarpTarget::System(enemy.id)));
        assert!(picked.iter().any(|t| t.starts_with("HOSTILE")), "no verdict: {picked:?}");
        standing.change(enemy.faction.unwrap(), 200.0, "peace");
        let at_peace = draw(&standing, Some(GalaxyWarpTarget::System(enemy.id)));
        assert!(at_peace.iter().any(|t| t == "Not hostile"), "still hostile: {at_peace:?}");
    }

    /// Scrolling keeps what's under the cursor under the cursor, and scrolling
    /// back out always ends on the whole galaxy, wherever you went in.
    #[test]
    fn the_galaxy_zooms_about_the_cursor_and_back_out() {
        let norm = Vec2::new(0.3, -0.2);
        let whole = GalaxyView::default();
        let target = whole.at(norm);
        let close = whole.scrolled(3.0, norm);
        assert!(close.zoom > 2.0);
        assert!(close.at(norm).distance(target) < 1.0, "{:?} drifted from {target:?}", close.at(norm));

        let mut view = close;
        for _ in 0..40 {
            view = view.scrolled(-1.0, Vec2::new(-0.4, 0.4));
        }
        assert_eq!(view, whole);
    }

    /// A drawn system is where a click reads it to be, zoomed or not.
    #[test]
    fn a_zoomed_galaxy_map_clicks_where_it_draws() {
        let view = GalaxyView::close_on(Vec2::new(1_200_000.0, -800_000.0));
        let pos = view.center + Vec2::new(150_000.0, 90_000.0);
        let (x, y) = galaxy_to_map_px(pos, 600.0, view);
        let norm = Vec2::new(x / 600.0 - 0.5, y / 600.0 - 0.5);
        assert!(view.at(norm).distance(pos) < 1.0);
    }

    /// Scrolling in goes to a system you know of and can see, never one you
    /// don't, and never one off the panel.
    #[test]
    fn scrolling_in_only_enters_known_systems_on_the_panel() {
        let mut map = crate::celestial::galaxy::generate_galaxy_map(42);
        for s in &mut map.systems {
            s.discovery = SystemDiscovery::Unknown;
        }
        let whole = GalaxyView::default();
        assert_eq!(system_to_enter(&map, whole, Vec2::ZERO), None);

        let id = map.systems[3].id;
        let pos = map.systems[3].galaxy_pos;
        map.systems[3].discovery = SystemDiscovery::Located;
        assert_eq!(system_to_enter(&map, whole, Vec2::ZERO), Some(id));
        assert_eq!(system_to_enter(&map, GalaxyView::close_on(pos), Vec2::ZERO), Some(id));
        let elsewhere = GalaxyView::close_on(-pos.normalize_or_zero() * 4_000_000.0);
        assert_eq!(system_to_enter(&map, elsewhere, Vec2::ZERO), None);
    }
}

#[cfg(test)]
mod built_gun_name_tests {
    use super::*;

    #[test]
    fn a_built_gun_is_named_by_its_gauge() {
        assert_eq!(built_gun_rack_name(60.0), "60 mm Gun");
        assert_eq!(built_gun_rack_name(280.0), "280 mm Gun");
        assert_eq!(built_gun_rack_name(500.0), "500 mm Gun");
    }
}
