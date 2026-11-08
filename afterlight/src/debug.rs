use bevy::prelude::*;
use rand::Rng;

use crate::components::*;
use crate::events::*;
use crate::resources::*;
use crate::states::GameState;
use crate::ai_ship::components::{AiShip, AiShipState, AiShipType};

// ============================================================================
// DEBUG MENU — dev tooling, ` (backtick) to toggle, and only in a build
// launched with AFTERLIGHT_DEBUG=1. Not player-facing polish:
// spawn ships/wrecks on demand, grant credits, repair/refuel/rearm, clear an
// area, teleport, dump cargo, visualize hitboxes. Actions only respond while
// the panel is open so the hotkeys can't misfire during normal play.
//
// Opens alongside a second panel, the TUNING PANEL — live +/- multipliers
// for speed/damage/fire-rate plus god mode, infinite fuel, and a teleport
// button, for pushing the game's numbers past their normal range to see
// where things break. DebugTuning is read by ship/movement.rs (speed, fuel)
// and combat/new_projectiles.rs (damage, fire rate — the player's kinetic
// weapons specifically: Cannon/Railgun/Coilgun/Gatling; lasers and missiles
// aren't wired to it yet) and ship/damage.rs (god mode).
// ============================================================================

pub struct DebugPlugin;

/// Whether this run was launched with the debug menu enabled:
/// `AFTERLIGHT_DEBUG=1`, the house style of every other dev switch (demo.rs).
///
/// Backtick sits right beside 1, and with the menu open 1 used to reveal the
/// whole galaxy as Visited -- which also maxed the story's reach, so a
/// playtest run that brushed the key got the endgame mirror ship a minute
/// in. It reveals them as Located now, and a normal launch has no debug menu
/// at all.
pub fn debug_enabled() -> bool {
    std::env::var("AFTERLIGHT_DEBUG").ok().as_deref() == Some("1") || god_enabled()
}

/// `AFTERLIGHT_GOD=1`: a launch for playing the story through quickly. Nothing
/// hurts your ship, fuel never runs out, the debug menu is on, and the
/// clock can be run fast: `]` doubles the speed of time (up to x16), `[`
/// halves it, and Shift + `]` pushes the dark on a whole system.
pub fn god_enabled() -> bool {
    std::env::var("AFTERLIGHT_GOD").ok().as_deref() == Some("1")
}

/// How fast god mode's clock can run. Past this a frame's step outgrows what
/// Time<Virtual> will take (250 ms), and it just stops getting faster.
pub const TIME_SPEEDS: [f32; 5] = [1.0, 2.0, 4.0, 8.0, 16.0];

/// The next speed up or down from `now`.
pub fn step_speed(now: f32, up: bool) -> f32 {
    let i = TIME_SPEEDS.iter().position(|s| (*s - now).abs() < 0.01).unwrap_or(0);
    let j = if up { (i + 1).min(TIME_SPEEDS.len() - 1) } else { i.saturating_sub(1) };
    TIME_SPEEDS[j]
}

/// The time keys: `]` faster, `[` slower, Shift + `]` the dark on a system.
/// Not while building, where the brackets turn blocks.
fn time_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    build: Res<State<crate::states::BuildState>>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut push: ResMut<crate::narrative::front::DarkPush>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if *build.get() != crate::states::BuildState::Inactive {
        return;
    }
    let shift = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let now = virtual_time.relative_speed();
    let next = if keyboard.just_pressed(KeyCode::BracketRight) && shift {
        push.0 += crate::narrative::front::ONE_SYSTEM;
        notifications.write(ShowNotification { message: "[god] the dark moves on a system".into(), notification_type: NotificationType::Info, duration: 3.0 });
        return;
    } else if keyboard.just_pressed(KeyCode::BracketRight) {
        step_speed(now, true)
    } else if keyboard.just_pressed(KeyCode::BracketLeft) {
        step_speed(now, false)
    } else {
        return;
    };
    if next != now {
        virtual_time.set_relative_speed(next);
        notifications.write(ShowNotification { message: format!("[god] time x{next}"), notification_type: NotificationType::Info, duration: 2.0 });
    }
}

/// God mode keeps the ship whole after everything else has had its turn
/// this frame: every plate and block back to full, no hole venting, every
/// crew member well. Damage reaches the ship by many roads (rounds, beams,
/// blasts, fire, heat, the dark); this doesn't have to know them all.
fn god_keeps_the_ship(
    tuning: Res<DebugTuning>,
    ship: Query<Entity, With<Ship>>,
    children: Query<&Children>,
    mut hull: Query<&mut HullSegment, Without<HullDestroyed>>,
    mut modules: Query<&mut Module, Without<DestroyedModule>>,
    mut crew: Query<&mut CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
) {
    if !tuning.god_mode {
        return;
    }
    let Ok(ship) = ship.single() else { return };
    for block in children.iter_descendants(ship) {
        if let Ok(mut h) = hull.get_mut(block) {
            if h.health < h.max_health || h.is_depressurized {
                h.health = h.max_health;
                h.is_depressurized = false;
                h.depressurization_level = 0.0;
            }
        }
        if let Ok(mut m) = modules.get_mut(block) {
            if m.health < m.max_health {
                m.health = m.max_health;
            }
        }
    }
    for mut c in &mut crew {
        if c.health < c.max_health {
            c.health = c.max_health;
        }
    }
}

#[derive(Component)]
struct TimeChip;

/// "TIME x8" at the top of the screen whenever the clock isn't running true.
fn time_chip(mut commands: Commands, virtual_time: Res<Time<Virtual>>, mut chip: Query<(&mut Text, &mut Visibility), With<TimeChip>>) {
    let speed = virtual_time.relative_speed();
    let Ok((mut text, mut vis)) = chip.single_mut() else {
        commands.spawn((
            Text::new(""),
            TextFont { font_size: FontSize::Px(16.0), ..default() },
            TextColor(Color::srgb(1.0, 0.85, 0.4)),
            Node { position_type: PositionType::Absolute, top: Val::Px(72.0), left: Val::Percent(46.0), ..default() },
            ZIndex(50),
            Visibility::Hidden,
            TimeChip,
        ));
        return;
    };
    let want = if (speed - 1.0).abs() > 0.01 { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    let label = format!("TIME x{speed}");
    if text.0 != label {
        text.0 = label;
    }
}

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        // DebugTuning is read by movement, combat and damage whether or not
        // the menu exists; its defaults are all neutral.
        app.init_resource::<DebugMenu>()
            .init_resource::<DebugTuning>();
        if !debug_enabled() {
            return;
        }
        info!("AFTERLIGHT_DEBUG=1: debug menu on backtick");
        if god_enabled() {
            info!("AFTERLIGHT_GOD=1: god mode, ] and [ to run time faster or slower");
            app.add_systems(Update, (time_keys, time_chip).run_if(in_state(GameState::Exploring)))
                .add_systems(PostUpdate, god_keeps_the_ship);
        }
        app.add_systems(Update, toggle_debug_menu)
            .add_systems(
                Update,
                (
                    debug_actions,
                    debug_breach_hull,
                    debug_kill_flagged,
                    draw_hitboxes,
                )
                    .run_if(in_state(GameState::Exploring)),
            )
            .add_systems(
                Update,
                apply_debug_teleport.run_if(in_state(GameState::Exploring)),
            )
            .add_systems(
                Update,
                debug_kill_crew.run_if(in_state(GameState::Exploring)),
            )
            .add_systems(
                Update,
                (tuning_button_system, update_tuning_value_text)
                    .run_if(in_state(GameState::Exploring)),
            );
    }
}

#[derive(Resource, Default)]
pub struct DebugMenu {
    pub open: bool,
    pub show_hitboxes: bool,
}

/// Live gameplay multipliers/toggles for exploring the game's limits — see
/// module doc comment for exactly which systems read each field.
#[derive(Resource)]
pub struct DebugTuning {
    pub speed_mult: f32,
    pub damage_mult: f32,
    pub fire_rate_mult: f32,
    pub god_mode: bool,
    pub infinite_fuel: bool,
}

impl Default for DebugTuning {
    fn default() -> Self {
        Self {
            speed_mult: 1.0,
            damage_mult: 1.0,
            fire_rate_mult: 1.0,
            god_mode: god_enabled(),
            infinite_fuel: god_enabled(),
        }
    }
}

#[derive(Component)]
struct DebugMenuPanel;

#[derive(Component)]
struct TuningPanel;

/// Which field a tuning row's +/- buttons (or the row itself, for toggles
/// and actions) affects.
#[derive(Component, Clone, Copy, PartialEq)]
enum TuningButton {
    SpeedDec,
    SpeedInc,
    DamageDec,
    DamageInc,
    FireRateDec,
    FireRateInc,
    ToggleGodMode,
    ToggleInfiniteFuel,
    Teleport,
    ResetAll,
}

/// Tags a value-display Text so update_tuning_value_text knows what to
/// write into it each frame.
#[derive(Component, Clone, Copy)]
enum TuningValueText {
    Speed,
    Damage,
    FireRate,
    GodMode,
    InfiniteFuel,
}

/// Ships flagged for instant destruction ONE FRAME after spawning — the
/// block hierarchy must exist (commands flushed) before
/// ai_ship_death_system walks the children to build the wreck.
#[derive(Component)]
struct DebugKillNextFrame;

/// Factions the debug menu can spawn.
///
/// Note that two of these -- The Silence and Stellar Preserve -- are excluded by
/// `faction_fights`, so they will never shoot at anything. That is by design
/// for them, but it means the "spawn hostile ship" key has to filter them out
/// or it lies one time in four: you get a ship that closes, tracks you, and
/// then just watches.
const SPAWNABLE: [AiShipType; 8] = [
    AiShipType::TerranHegemony,
    AiShipType::GildedThrone,
    AiShipType::CorpseStars,
    AiShipType::TheSilence,
    AiShipType::BrokenChoir,
    AiShipType::SynthesisCollective,
    AiShipType::RecursiveKingdom,
    AiShipType::StellarPreserve,
];

const BOSSES: [AiShipType; 2] = [AiShipType::EternalHegemony, AiShipType::Shepherd];

fn toggle_debug_menu(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<DebugMenu>,
    mut commands: Commands,
    panel_query: Query<Entity, With<DebugMenuPanel>>,
    tuning_panel_query: Query<Entity, With<TuningPanel>>,
) {
    // Backquote as primary — F10 is a macOS media key unless Fn is held.
    if !keyboard.just_pressed(KeyCode::Backquote) && !keyboard.just_pressed(KeyCode::F10) {
        return;
    }
    menu.open = !menu.open;

    if menu.open {
        commands
            .spawn((
                DebugMenuPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    top: Val::Px(220.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.05, 0.08, 0.85)),
                GlobalZIndex(50),
            ))
            .with_children(|panel| {
                for line in [
                    "DEBUG  (` close)",
                    "7  +1000 credits",
                    "8  spawn hostile ship",
                    "9  spawn fresh wreck",
                    "2  clear nearby hostiles",
                    "3  teleport to system star",
                    "4  fill cargo (one of everything)",
                    "5  spawn a boss ship",
                    "0  reveal + target nearest system",
                    "1  reveal entire galaxy map",
                    "6  kill one crew member",
                    "^6 breach your own hull (shift)",
                    "H  toggle hitboxes",
                    "J  repair + refuel + rearm",
                ] {
                    panel.spawn((
                        Text::new(line),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(Color::srgb(0.75, 0.9, 1.0)),
                    ));
                }
            });

        spawn_tuning_panel(&mut commands);
    } else {
        for entity in panel_query.iter() {
            commands.entity(entity).despawn();
        }
        for entity in tuning_panel_query.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_tuning_panel(commands: &mut Commands) {
    commands
        .spawn((
            TuningPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(260.0),
                top: Val::Px(220.0),
                width: Val::Px(230.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.05, 0.08, 0.85)),
            GlobalZIndex(50),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("TUNING - see the limits"),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::srgb(0.75, 0.9, 1.0)),
            ));

            spawn_stepper_row(panel, "Speed", TuningValueText::Speed, TuningButton::SpeedDec, TuningButton::SpeedInc);
            spawn_stepper_row(panel, "Damage", TuningValueText::Damage, TuningButton::DamageDec, TuningButton::DamageInc);
            spawn_stepper_row(panel, "Fire Rate", TuningValueText::FireRate, TuningButton::FireRateDec, TuningButton::FireRateInc);

            spawn_toggle_row(panel, "God Mode", TuningValueText::GodMode, TuningButton::ToggleGodMode);
            spawn_toggle_row(panel, "Infinite Fuel", TuningValueText::InfiniteFuel, TuningButton::ToggleInfiniteFuel);

            spawn_action_row(panel, "Teleport to map target", TuningButton::Teleport);
            spawn_action_row(panel, "Reset all", TuningButton::ResetAll);
        });
}

fn tuning_button_style() -> (Node, BackgroundColor) {
    (
        Node {
            width: Val::Px(22.0),
            height: Val::Px(18.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.15, 0.2, 0.28, 1.0)),
    )
}

fn spawn_stepper_row(
    panel: &mut ChildSpawnerCommands,
    label: &str,
    value_tag: TuningValueText,
    dec: TuningButton,
    inc: TuningButton,
) {
    panel.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(4.0),
        ..default()
    }).with_children(|row| {
        row.spawn((
            Text::new(label),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(Color::srgb(0.7, 0.75, 0.8)),
            Node { width: Val::Px(70.0), ..default() },
        ));
        let (node, bg) = tuning_button_style();
        row.spawn((node.clone(), bg, Interaction::None, dec))
            .with_children(|b| { b.spawn((Text::new("-"), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(Color::WHITE))); });
        row.spawn((
            Text::new("1.00x"),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(Color::srgb(1.0, 0.9, 0.5)),
            Node { width: Val::Px(48.0), justify_content: JustifyContent::Center, ..default() },
            value_tag,
        ));
        row.spawn((node, bg, Interaction::None, inc))
            .with_children(|b| { b.spawn((Text::new("+"), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(Color::WHITE))); });
    });
}

fn spawn_toggle_row(panel: &mut ChildSpawnerCommands, label: &str, value_tag: TuningValueText, toggle: TuningButton) {
    panel.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.16, 0.22, 1.0)),
        Interaction::None,
        toggle,
    )).with_children(|row| {
        row.spawn((
            Text::new(label),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(Color::srgb(0.7, 0.75, 0.8)),
        ));
        row.spawn((
            Text::new("OFF"),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(Color::srgb(1.0, 0.4, 0.4)),
            value_tag,
        ));
    });
}

fn spawn_action_row(panel: &mut ChildSpawnerCommands, label: &str, action: TuningButton) {
    panel.spawn((
        Node {
            padding: UiRect::all(Val::Px(4.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.18, 0.14, 0.1, 1.0)),
        Interaction::None,
        action,
    )).with_children(|row| {
        row.spawn((
            Text::new(label),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(Color::srgb(0.9, 0.8, 0.6)),
        ));
    });
}

/// Step sizes chosen so a handful of clicks gets you somewhere absurd —
/// this panel exists to find the game's breaking points, not for fine
/// balance tuning (that's the customization/tuning window's job).
const SPEED_STEP: f32 = 0.25;
const DAMAGE_STEP: f32 = 0.5;
const FIRE_RATE_STEP: f32 = 0.25;

fn tuning_button_system(
    buttons: Query<(&TuningButton, &Interaction), Changed<Interaction>>,
    mut tuning: ResMut<DebugTuning>,
    mut ship_query: Query<(&mut Transform, &mut Velocity), With<Ship>>,
    pending: Res<crate::ui::PendingWarpTarget>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for (button, interaction) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            TuningButton::SpeedDec => tuning.speed_mult = (tuning.speed_mult - SPEED_STEP).max(0.0),
            TuningButton::SpeedInc => tuning.speed_mult = (tuning.speed_mult + SPEED_STEP).min(20.0),
            TuningButton::DamageDec => tuning.damage_mult = (tuning.damage_mult - DAMAGE_STEP).max(0.0),
            TuningButton::DamageInc => tuning.damage_mult = (tuning.damage_mult + DAMAGE_STEP).min(50.0),
            TuningButton::FireRateDec => tuning.fire_rate_mult = (tuning.fire_rate_mult - FIRE_RATE_STEP).max(0.05),
            TuningButton::FireRateInc => tuning.fire_rate_mult = (tuning.fire_rate_mult + FIRE_RATE_STEP).min(20.0),
            TuningButton::ToggleGodMode => tuning.god_mode = !tuning.god_mode,
            TuningButton::ToggleInfiniteFuel => tuning.infinite_fuel = !tuning.infinite_fuel,
            TuningButton::ResetAll => *tuning = DebugTuning::default(),
            TuningButton::Teleport => {
                let Some(target) = pending.0 else {
                    notifications.write(ShowNotification {
                        message: "[debug] no map target set - open the map (M) and click one first".into(),
                        notification_type: NotificationType::Warning,
                        duration: 2.5,
                    });
                    continue;
                };
                if let Ok((mut transform, mut velocity)) = ship_query.single_mut() {
                    transform.translation.x = target.x;
                    transform.translation.y = target.y;
                    velocity.0 = Vec2::ZERO;
                    notifications.write(ShowNotification {
                        message: "[debug] teleported to map target".into(),
                        notification_type: NotificationType::Info,
                        duration: 2.0,
                    });
                }
            }
        }
    }
}

fn update_tuning_value_text(
    tuning: Res<DebugTuning>,
    mut texts: Query<(&mut Text, &mut TextColor, &TuningValueText)>,
) {
    if !tuning.is_changed() {
        return;
    }
    for (mut text, mut color, tag) in texts.iter_mut() {
        match tag {
            TuningValueText::Speed => text.0 = format!("{:.2}x", tuning.speed_mult),
            TuningValueText::Damage => text.0 = format!("{:.2}x", tuning.damage_mult),
            TuningValueText::FireRate => text.0 = format!("{:.2}x", tuning.fire_rate_mult),
            TuningValueText::GodMode => {
                text.0 = if tuning.god_mode { "ON".into() } else { "OFF".into() };
                color.0 = if tuning.god_mode { Color::srgb(0.4, 1.0, 0.4) } else { Color::srgb(1.0, 0.4, 0.4) };
            }
            TuningValueText::InfiniteFuel => {
                text.0 = if tuning.infinite_fuel { "ON".into() } else { "OFF".into() };
                color.0 = if tuning.infinite_fuel { Color::srgb(0.4, 1.0, 0.4) } else { Color::srgb(1.0, 0.4, 0.4) };
            }
        }
    }
}

/// Bundled so debug_actions stays under Bevy's ~16-raw-parameter ceiling on
/// function systems — same pattern MapWorldData (ui/mod.rs) already uses
/// for the same reason.
#[derive(bevy::ecs::system::SystemParam)]
struct DebugGalaxyParams<'w> {
    galaxy_map: ResMut<'w, crate::celestial::resources::GalaxyMap>,
    pending_galaxy_target: ResMut<'w, crate::celestial::resources::PendingGalaxyWarpTarget>,
    streaming: Res<'w, crate::celestial::resources::SystemStreamingManager>,
}

#[derive(bevy::ecs::system::SystemParam)]
struct DebugHostileParams<'w, 's> {
    hostile_query: Query<'w, 's, (Entity, &'static GlobalTransform), With<AiShip>>,
    destroyed_events: MessageWriter<'w, AiShipDestroyed>,
    ai_state_query: Query<'w, 's, &'static mut AiShipState>,
    ai_type_query: Query<'w, 's, &'static AiShipType>,
}

fn debug_actions(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<DebugMenu>,
    mut commands: Commands,
    ship_query: Query<(Entity, &GlobalTransform), With<Ship>>,
    children_query: Query<&Children>,
    mut currency: ResMut<Currency>,
    mut fuel: ResMut<FuelState>,
    mut hull_query: Query<&mut HullSegment>,
    mut module_query: Query<&mut Module>,
    mut weapon_query: Query<&mut Weapon>,
    registry: Res<crate::building::ModuleRegistry>,
    asset_server: Res<AssetServer>,
    mut notifications: MessageWriter<ShowNotification>,
    mut galaxy: DebugGalaxyParams,
    mut inventory: ResMut<Inventory>,
    mut hostiles: DebugHostileParams,
) {
    if !menu.open {
        return;
    }
    let Ok((ship_entity, ship_gt)) = ship_query.single() else { return };
    let ship_pos = ship_gt.translation().truncate();
    let mut rng = rand::thread_rng();

    let mut notify = |notifications: &mut MessageWriter<ShowNotification>, msg: String| {
        notifications.write(ShowNotification {
            message: msg,
            notification_type: NotificationType::Info,
            duration: 2.0,
        });
    };

    if keyboard.just_pressed(KeyCode::Digit7) {
        currency.credits += 1000;
        notify(&mut notifications, "[debug] +1000 credits".into());
    }

    if keyboard.just_pressed(KeyCode::Digit8) {
        // "Hostile" has to mean hostile: pick only from factions that fight.
        let fighters: Vec<AiShipType> = SPAWNABLE
            .into_iter()
            .filter(|f| crate::ai_ship::components::faction_fights(*f))
            .collect();
        let ship_type = fighters[rng.gen_range(0..fighters.len())];
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let pos = ship_pos + Vec2::new(angle.cos(), angle.sin()) * 700.0;
        crate::ai_ship::spawner::spawn_ai_ship(ship_type, pos, &mut commands, &registry, &asset_server);
        notify(&mut notifications, format!("[debug] spawned {:?}", ship_type));
    }

    if keyboard.just_pressed(KeyCode::Digit9) {
        let ship_type = SPAWNABLE[rng.gen_range(0..SPAWNABLE.len())];
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let pos = ship_pos + Vec2::new(angle.cos(), angle.sin()) * 450.0;
        let entity = crate::ai_ship::spawner::spawn_ai_ship(ship_type, pos, &mut commands, &registry, &asset_server);
        commands.entity(entity).insert(DebugKillNextFrame);
        notify(&mut notifications, format!("[debug] spawning {:?} wreck", ship_type));
    }

    // Clears every real (spawned) hostile within 6000 units — for testing
    // an empty arena, or just cleaning up after a debug-spawn spree.
    if keyboard.just_pressed(KeyCode::Digit2) {
        let mut cleared = 0;
        for (entity, gt) in hostiles.hostile_query.iter() {
            if gt.translation().truncate().distance(ship_pos) > 6000.0 { continue; }
            let ship_type = hostiles.ai_type_query.get(entity).copied().unwrap_or(AiShipType::BrokenChoir);
            if let Ok(mut state) = hostiles.ai_state_query.get_mut(entity) {
                if !state.is_destroyed {
                    state.is_destroyed = true;
                    state.hull_integrity = 0.0;
                    hostiles.destroyed_events.write(AiShipDestroyed {
                        entity,
                        ship_type,
                        position: gt.translation().truncate(),
                        bounty_id: None,
                        cause: ShipDeathCause::Gutted,
                    });
                    cleared += 1;
                }
            }
        }
        notify(&mut notifications, format!("[debug] cleared {} nearby hostiles", cleared));
    }

    // Teleports to the current system's star — always a known, reachable
    // point regardless of how the local map looks.
    if keyboard.just_pressed(KeyCode::Digit3) {
        if let Some(def) = galaxy.streaming.loaded_system.and_then(|id| galaxy.galaxy_map.systems.iter().find(|s| s.id == id)) {
            let target = def.local_center + Vec2::new(80_000.0, 0.0);
            commands.entity(ship_entity).try_insert(DebugTeleportTo(target));
            notify(&mut notifications, format!("[debug] teleporting to {}'s star", def.name));
        }
    }

    // Fills cargo with one stack of every item type, for testing sell/buy
    // and weight-capacity behavior without a mining/looting grind.
    if keyboard.just_pressed(KeyCode::Digit4) {
        for item in [ItemType::ScrapMetal, ItemType::Crystal, ItemType::BioSample, ItemType::FuelCell, ItemType::RareAlloy, ItemType::AncientArtifact, ItemType::AmmoCrate] {
            inventory.add_item(item, 5);
        }
        notify(&mut notifications, "[debug] cargo filled with 5x of everything".into());
    }

    // Spawns a boss (Eternal Hegemony/Shepherd) for testing endgame-tier fights
    // without needing to actually travel to boss territory.
    if keyboard.just_pressed(KeyCode::Digit5) {
        let ship_type = BOSSES[rng.gen_range(0..BOSSES.len())];
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let pos = ship_pos + Vec2::new(angle.cos(), angle.sin()) * 1200.0;
        crate::ai_ship::spawner::spawn_ai_ship(ship_type, pos, &mut commands, &registry, &asset_server);
        notify(&mut notifications, format!("[debug] spawned boss {:?}", ship_type));
    }

    // Instantly reveals + targets the nearest not-yet-visited system for
    // interstellar warp testing — real discovery (scanning, star charts,
    // and the click-anywhere galaxy map) is wired now too, this just saves
    // clicking around for a quick test.
    if keyboard.just_pressed(KeyCode::Digit0) {
        use crate::celestial::resources::{GalaxyWarpTarget, SystemDiscovery};
        let current_pos = galaxy.streaming.current_galaxy_pos;
        let nearest = galaxy.galaxy_map.systems.iter_mut()
            .filter(|s| s.discovery != SystemDiscovery::Visited && s.galaxy_pos.distance(current_pos) > 1.0)
            .min_by(|a, b| a.galaxy_pos.distance(current_pos).partial_cmp(&b.galaxy_pos.distance(current_pos)).unwrap());
        if let Some(sys) = nearest {
            sys.discovery = SystemDiscovery::Located;
            galaxy.pending_galaxy_target.0 = Some(GalaxyWarpTarget::System(sys.id));
            notify(&mut notifications, format!("[debug] revealed + targeted {} for warp (G to jump)", sys.name));
        }
    }

    // Reveals every system on the galaxy map — for eyeballing the whole
    // layout at once, not a real gameplay mechanic. As Located, not Visited:
    // the story's reach counts Visited systems, so revealing them as visited
    // jumped a fresh run to the late game and sent the mirror ship a
    // minute in (a god-mode playtest, 2026-11-05).
    if keyboard.just_pressed(KeyCode::Digit1) {
        use crate::celestial::resources::SystemDiscovery;
        for sys in galaxy.galaxy_map.systems.iter_mut() {
            if sys.discovery == SystemDiscovery::Unknown {
                sys.discovery = SystemDiscovery::Located;
            }
        }
        notify(&mut notifications, "[debug] revealed entire galaxy map".into());
    }

    if keyboard.just_pressed(KeyCode::KeyH) {
        menu.show_hitboxes = !menu.show_hitboxes;
        notify(
            &mut notifications,
            format!("[debug] hitboxes {}", if menu.show_hitboxes { "on" } else { "off" }),
        );
    }

    if keyboard.just_pressed(KeyCode::KeyJ) {
        fuel.current_fuel = fuel.max_fuel;
        if let Ok(children) = children_query.get(ship_entity) {
            for child in children.iter() {
                if let Ok(mut hull) = hull_query.get_mut(child) {
                    hull.health = hull.max_health;
                    hull.is_depressurized = false;
                    hull.depressurization_level = 0.0;
                }
                if let Ok(mut module) = module_query.get_mut(child) {
                    if module.health > 0.0 {
                        module.health = module.max_health;
                    }
                }
                if let Ok(mut weapon) = weapon_query.get_mut(child) {
                    weapon.ammo = weapon.max_ammo;
                }
            }
        }
        notify(&mut notifications, "[debug] repaired + refueled + rearmed".into());
    }
}

/// One-frame marker: teleport this entity to the given position next frame.
/// Needed because debug_actions only has a GlobalTransform query for the
/// player ship (shared with other read-only lookups); a dedicated tiny
/// system below does the actual mutable Transform write.
#[derive(Component)]
struct DebugTeleportTo(Vec2);

fn apply_debug_teleport(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut Velocity, &DebugTeleportTo)>,
) {
    for (entity, mut transform, mut velocity, teleport) in query.iter_mut() {
        transform.translation.x = teleport.0.x;
        transform.translation.y = teleport.0.y;
        velocity.0 = Vec2::ZERO;
        commands.entity(entity).remove::<DebugTeleportTo>();
    }
}

/// Detonate ships flagged by the wreck-spawn action, one frame later.
fn debug_kill_flagged(
    mut commands: Commands,
    mut flagged: Query<(Entity, &mut AiShipState, &Transform, &AiShipType), With<DebugKillNextFrame>>,
    mut destroyed_events: MessageWriter<AiShipDestroyed>,
) {
    for (entity, mut state, transform, ship_type) in flagged.iter_mut() {
        if !state.is_destroyed {
            state.is_destroyed = true;
            state.hull_integrity = 0.0;
            destroyed_events.write(AiShipDestroyed {
                entity,
                ship_type: *ship_type,
                position: transform.translation.truncate(),
                bounty_id: None,
                cause: ShipDeathCause::Gutted,
            });
        }
        commands.entity(entity).try_remove::<DebugKillNextFrame>();
    }
}

/// Gizmo overlay: block bounds (green), creature bounds (orange),
/// missile blast radii (yellow). Sprite bounds ARE the hitboxes — combat
/// collision is distance-vs-block-cell math, no physics engine.
fn draw_hitboxes(
    menu: Res<DebugMenu>,
    mut gizmos: Gizmos,
    block_query: Query<(&GlobalTransform, &Sprite), Or<(With<Module>, With<HullSegment>)>>,
    creature_query: Query<(&GlobalTransform, &Sprite), With<Creature>>,
    missile_query: Query<(&GlobalTransform, &crate::combat::new_projectiles::MissileProjectile)>,
) {
    if !menu.show_hitboxes {
        return;
    }
    for (gt, sprite) in block_query.iter() {
        let size = sprite.custom_size.unwrap_or(Vec2::splat(66.0));
        let angle = gt.rotation().to_euler(EulerRot::XYZ).2;
        gizmos.rect_2d(
            Isometry2d::new(gt.translation().truncate(), Rot2::radians(angle)),
            size,
            Color::srgba(0.2, 1.0, 0.4, 0.5),
        );
    }
    for (gt, sprite) in creature_query.iter() {
        let radius = sprite.custom_size.map(|s| s.x.max(s.y) * 0.5).unwrap_or(20.0);
        gizmos.circle_2d(gt.translation().truncate(), radius, Color::srgba(1.0, 0.45, 0.2, 0.6));
    }
    for (gt, missile) in missile_query.iter() {
        gizmos.circle_2d(gt.translation().truncate(), missile.blast_radius, Color::srgba(1.0, 0.85, 0.2, 0.35));
    }
}

/// Debug menu, `Shift+6`: punch a hole in your own hull.
///
/// Shares the key with `debug_kill_crew`, which takes plain `6`. The two
/// belong together: hole the ship, then watch whether the air takes somebody
/// out through it.
///
/// Breaching means what `ship::air::mark_breached_hull` means by it: the plate
/// is holed and the air starts leaving. Setting health alone would not do it,
/// because a breach is an edge taken from a `HullBreached` event rather than a
/// standing condition read off health -- crew sealing has to be able to close a
/// hole in a plate that is still damaged.
///
/// Its own system rather than another arm of `debug_actions`, which is already
/// at Bevy's 16-parameter ceiling.
fn debug_breach_hull(
    keyboard: Res<ButtonInput<KeyCode>>,
    menu: Res<DebugMenu>,
    ship_query: Query<Entity, With<Ship>>,
    room_map: Res<crate::building::rooms::RoomMap>,
    mut hull_query: Query<(&mut HullSegment, &ChildOf)>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !menu.open
        || !keyboard.just_pressed(KeyCode::Digit6)
        || !(keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight))
    {
        return;
    }
    let Ok(player_ship) = ship_query.single() else { return };

    // Only plates that front a compartment are worth holing -- puncturing
    // outboard armour with nothing behind it vents nothing and looks broken.
    let mut candidates: Vec<IVec2> = Vec::new();
    for (hull, parent) in hull_query.iter() {
        if parent.parent() != player_ship || hull.is_depressurized {
            continue;
        }
        let pos = hull.grid_position;
        if room_map.tile_to_room.contains_key(&pos) {
            continue; // hallway: interior itself, not a wall to breach
        }
        if [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y]
            .iter()
            .any(|o| room_map.tile_to_room.contains_key(&(pos + *o)))
        {
            candidates.push(pos);
        }
    }

    if candidates.is_empty() {
        notifications.write(ShowNotification {
            message: "[debug] no hull plate borders a compartment".into(),
            notification_type: NotificationType::Warning,
            duration: 2.0,
        });
        return;
    }

    let target = candidates[rand::thread_rng().gen_range(0..candidates.len())];
    for (mut hull, parent) in hull_query.iter_mut() {
        if parent.parent() != player_ship || hull.grid_position != target {
            continue;
        }
        hull.health = 0.0;
        hull.is_depressurized = true;
        hull.depressurization_level = 1.0;
        break;
    }

    notifications.write(ShowNotification {
        message: format!("[debug] hull breached at {}, {}", target.x, target.y),
        notification_type: NotificationType::Danger,
        duration: 3.0,
    });
}

/// `6` — kill one living crew member outright.
///
/// A TEST HOOK, not a damage source. Everything downstream of a death — the
/// notification, the body left on the deck, a spare hand carrying it to the
/// airlock, the drifting corpse that stays in the system forever — is nearly
/// impossible to see in play, because as of writing only boarding parasites
/// and EVA blasts can injure anybody at all. This makes the whole chain
/// reachable in one keypress so it can be watched end to end.
///
/// Delete this once crew take damage from fire, breaches and shrapnel for
/// real; at that point the chain tests itself in any serious fight.
///
/// Its own system rather than another arm of `debug_actions`, which is already
/// at Bevy's 16-parameter ceiling.
fn debug_kill_crew(
    keyboard: Res<ButtonInput<KeyCode>>,
    menu: Res<DebugMenu>,
    mut crew: Query<
        (Entity, &mut CrewMember),
        Without<crate::ai_ship::components::OwnedByAiShip>,
    >,
    mut damage_events: MessageWriter<CrewDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    // Plain `6`. Shift+6 holes the hull instead, in `debug_breach_hull`.
    if !menu.open
        || !keyboard.just_pressed(KeyCode::Digit6)
        || keyboard.pressed(KeyCode::ShiftLeft)
        || keyboard.pressed(KeyCode::ShiftRight)
    {
        return;
    }

    let Some((entity, mut member)) = crew.iter_mut().find(|(_, m)| m.health > 0.0) else {
        notifications.write(ShowNotification {
            message: "[debug] nobody left alive aboard".into(),
            notification_type: NotificationType::Warning,
            duration: 2.0,
        });
        return;
    };

    let name = member.name.clone();
    let fatal = member.health;
    member.health = 0.0;

    // Go through the real damage event rather than only zeroing health, so the
    // death is reported with a cause exactly as a genuine one would be
    // (crew::report_crew_deaths reads these to name what killed someone).
    damage_events.write(CrewDamaged {
        crew: entity,
        amount: fatal,
        source: CrewDamageSource::Explosion,
    });

    notifications.write(ShowNotification {
        message: format!("[debug] killed {name}"),
        notification_type: NotificationType::Info,
        duration: 2.0,
    });
}

#[cfg(test)]
mod god_tests {
    use super::*;

    /// The clock steps through the speeds in order, and stops at both ends.
    #[test]
    fn time_steps_up_and_down_and_stops_at_the_ends() {
        assert_eq!(step_speed(1.0, true), 2.0);
        assert_eq!(step_speed(8.0, true), 16.0);
        assert_eq!(step_speed(16.0, true), 16.0);
        assert_eq!(step_speed(4.0, false), 2.0);
        assert_eq!(step_speed(1.0, false), 1.0);
        // A speed set from elsewhere (autoplay) starts again from the bottom.
        assert_eq!(step_speed(3.0, true), 2.0);
    }
}
