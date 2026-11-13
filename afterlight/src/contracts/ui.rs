use bevy::prelude::*;

use crate::ai_ship::components::WorldSimulation;
use crate::components::Ship;
use crate::events::*;
use crate::resources::Currency;
use crate::world::home_base;
use super::generation;
use super::{
    ContractObjective, ContractState, ContractStatus, Faction, FactionReputation, MissionBoardOpen,
    ViewingStation,
};

/// Display name for a global station index (see world::home_base).
fn station_name(station: usize) -> String {
    home_base::station_display_name(station)
}

// ============================================================================
// MARKER COMPONENTS
// ============================================================================

#[derive(Component)]
pub struct MissionBoardPanel;

#[derive(Component)]
pub struct MissionBoardContent;

/// Tracks which contract index (in the combined available+active list) is selected.
#[derive(Resource, Default)]
pub struct MissionBoardSelection {
    index: usize,
}

impl MissionBoardSelection {
    /// The highlighted row (available contracts first, then active ones).
    pub fn index(&self) -> usize {
        self.index
    }
}

#[derive(Component)]
pub struct ContractHudRoot;

#[derive(Component)]
pub struct ContractHudText;

/// Whether the top-right active-contract tracker is drawn. Toggled with
/// Shift+J (plain J opens the mission board). With several bounties running at
/// once the list owns the corner of the screen for the whole trip, so it can be
/// tucked away without abandoning anything — the contracts still track.
#[derive(Resource)]
pub struct ContractHudVisible(pub bool);

impl Default for ContractHudVisible {
    fn default() -> Self { Self(true) }
}

// ============================================================================
// MISSION BOARD: TOGGLE
// ============================================================================

pub fn toggle_mission_board(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut board_open: ResMut<MissionBoardOpen>,
    existing: Query<Entity, With<MissionBoardPanel>>,
    stations: Res<home_base::SystemStations>,
    ship_query: Query<&Transform, With<Ship>>,
    mut viewing: ResMut<ViewingStation>,
    mut contract_state: ResMut<ContractState>,
    rep: Res<FactionReputation>,
    mut sim: ResMut<WorldSimulation>,
    mut notifications: MessageWriter<ShowNotification>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    standing: Res<crate::diplomacy::FactionStanding>,
) {
    if !keyboard.just_pressed(KeyCode::KeyJ) { return; }
    // Shift+J belongs to the HUD tracker toggle (see toggle_contract_hud).
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) { return; }

    if board_open.0 {
        board_open.0 = false;
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // The board shown is whichever station the ship is parked at or flying
    // near — every station offers its own bounties, and since docking now
    // parks the ship at that station's own berth, the same position lookup
    // covers being docked (it used to assume docked always meant Haven).
    let station = ship_query.single().ok()
        .and_then(|t| stations.nearest_index(t.translation.truncate()));

    let Some(station) = station else {
        notifications.write(ShowNotification {
            message: "No station in range - fly closer to view its bounty board.".into(),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
        return;
    };

    viewing.0 = station;
    let active_systems: Vec<u32> = streaming.loaded_system.into_iter().chain(streaming.warm_systems.iter().copied()).collect();
    // No bounties on your friends.
    let spare: Vec<_> = crate::diplomacy::FACTIONS
        .iter()
        .copied()
        .filter(|f| standing.tier(*f) >= crate::diplomacy::StandingTier::Friendly)
        .collect();
    generation::ensure_station_board(station, &mut contract_state, &rep, &mut sim, &active_systems, &spare);

    board_open.0 = true;
    commands.init_resource::<MissionBoardSelection>();
    // Spawn board root: a sheet down the right-hand side, like the map's
    // side panel, so the station stays in view.
    use crate::ui::theme::{Sheet, ThemeColors, ThemeFonts};
    commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(580.0),
                max_width: Val::Percent(70.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::left(Val::Px(1.0)),
                ..default()
            }, BackgroundColor(Sheet::BG.with_alpha(1.0)), BorderColor::all(Sheet::EDGE), ZIndex(100)),
        MissionBoardPanel,
    )).with_children(|parent| {
        parent.spawn((
            Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Sheet::EDGE),
        )).with_children(|strip| {
            strip.spawn((Text::new(format!("JOB BOARD - {}", station_name(station).to_uppercase())), TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() }, TextColor(Sheet::TITLE)));
            strip.spawn((Text::new("J close"), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(Sheet::LABEL)));
        });

        // Scrollable content area
        parent.spawn((
            (Node {
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip(),
                    flex_grow: 1.0,
                    ..default()
                }),
            MissionBoardContent,
        ));

        // Controls hint
        parent.spawn((
            Text::new("Up / Down  pick    Enter  take the job    Tab  give up an active one"),
            TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
            TextColor(Sheet::LABEL),
            Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)), border: UiRect::top(Val::Px(1.0)), ..default() },
            BorderColor::all(Sheet::RULE),
        ));
        let _ = ThemeColors::TEXT_PRIMARY;
    });
}

// ============================================================================
// MISSION BOARD: INPUT
// ============================================================================

pub fn mission_board_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    board_open: Res<MissionBoardOpen>,
    viewing: Res<ViewingStation>,
    mut state: ResMut<ContractState>,
    mut selection: ResMut<MissionBoardSelection>,
    mut currency: ResMut<Currency>,
    mut sim: ResMut<WorldSimulation>,
    mut accepted_events: MessageWriter<ContractAccepted>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !board_open.0 { return; }

    let avail_len = state.board_mut(viewing.0).len();
    let total_items = avail_len + state.active_contracts.len();
    if total_items == 0 { return; }

    // Navigate
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        if selection.index > 0 {
            selection.index -= 1;
        }
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        if selection.index + 1 < total_items {
            selection.index += 1;
        }
    }

    // Accept (Enter) — no cap on active contracts, take as many bounties as you want
    if keyboard.just_pressed(KeyCode::Enter) {
        let avail_len = state.board_mut(viewing.0).len();
        if selection.index < avail_len {
            let deposit = state.board_mut(viewing.0)[selection.index].deposit;
            if deposit > 0 && currency.credits < deposit {
                notifications.write(ShowNotification {
                    message: format!("Not enough credits for {}c deposit.", deposit),
                    notification_type: NotificationType::Warning,
                    duration: 3.0,
                });
                return;
            }

            // Actually take it. The check above existed but nothing was ever
            // deducted, so accepting a contract was a free option with a
            // downside only if you died — there was no reason not to take
            // every job on the board.
            currency.credits = currency.credits.saturating_sub(deposit);

            let mut contract = state.board_mut(viewing.0).remove(selection.index);
            contract.status = ContractStatus::Active;
            let id = contract.id;
            let title = contract.title.clone();
            state.active_contracts.push(contract);

            accepted_events.write(ContractAccepted { contract_id: id });
            notifications.write(ShowNotification {
                message: format!("Accepted: {}", title),
                notification_type: NotificationType::Success,
                duration: 3.0,
            });

            // Clamp selection
            let new_total = state.board_mut(viewing.0).len() + state.active_contracts.len();
            if selection.index >= new_total && new_total > 0 {
                selection.index = new_total - 1;
            }
        }
    }

    // Abandon active contract (Tab)
    if keyboard.just_pressed(KeyCode::Tab) {
        let avail_len = state.board_mut(viewing.0).len();
        let active_idx = selection.index.checked_sub(avail_len);
        if let Some(idx) = active_idx {
            if idx < state.active_contracts.len() {
                let contract = state.active_contracts.remove(idx);
                if let ContractObjective::DestroyShip { target_id, .. } = &contract.objective {
                    sim.untag_bounty(*target_id);
                }
                notifications.write(ShowNotification {
                    message: format!("Abandoned: {}", contract.title),
                    notification_type: NotificationType::Warning,
                    duration: 3.0,
                });
                let new_total = state.board_mut(viewing.0).len() + state.active_contracts.len();
                if selection.index >= new_total && new_total > 0 {
                    selection.index = new_total - 1;
                }
            }
        }
    }
}

// ============================================================================
// MISSION BOARD: DISPLAY UPDATE
// ============================================================================

pub fn update_mission_board_display(
    board_open: Res<MissionBoardOpen>,
    state: Res<ContractState>,
    viewing: Res<ViewingStation>,
    rep: Res<FactionReputation>,
    currency: Res<Currency>,
    selection: Option<Res<MissionBoardSelection>>,
    mut content_query: Query<(Entity, Option<&Children>), With<MissionBoardContent>>,
    mut commands: Commands,
) {
    if !board_open.0 { return; }

    let empty: Vec<super::Contract> = Vec::new();
    let board = state.available_by_station.get(viewing.0).unwrap_or(&empty);

    // MissionBoardContent is spawned with zero children (this system is what
    // populates it), so it starts out with no Children component at all —
    // Bevy only attaches one once a child actually exists. A bare `&Children`
    // in the query would never match that first-frame state and this system
    // would silently no-op forever, which is exactly what was happening: the
    // board always opened empty regardless of how many contracts existed.
    let Ok((content_entity, children)) = content_query.single_mut() else { return; };
    let sel_idx = selection.map(|s| s.index).unwrap_or(0);

    // Despawn old children
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    use crate::ui::theme::{Sheet, ThemeColors, ThemeFonts};
    // One ruled row: stars, the job, what it pays; the picked one in gold.
    fn job_row(parent: &mut ChildSpawnerCommands, picked: bool, stars: String, title: String, pay: String, pay_color: Color) {
        parent.spawn((
            Node {
                display: Display::Grid,
                grid_template_columns: vec![GridTrack::px(44.0), GridTrack::flex(1.0), GridTrack::auto()],
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect { left: Val::Px(2.0), bottom: Val::Px(1.0), ..default() },
                ..default()
            },
            BackgroundColor(if picked { Sheet::ROW_PICKED } else { Color::NONE }),
            BorderColor { left: if picked { Sheet::PICKED } else { Color::NONE }, bottom: Sheet::RULE, ..BorderColor::all(Color::NONE) },
        )).with_children(|row| {
            row.spawn((Text::new(stars), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(if picked { Sheet::PICKED } else { Sheet::LABEL })));
            row.spawn((Text::new(title), TextFont { font_size: FontSize::Px(12.5), ..default() }, TextColor(if picked { ThemeColors::TEXT_PRIMARY } else { ThemeColors::TEXT_SECONDARY })));
            row.spawn((Text::new(pay), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(pay_color)));
        });
    }
    fn section(parent: &mut ChildSpawnerCommands, words: String, aside: String, aside_color: Color) {
        parent.spawn((
            Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Baseline,
                padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(10.0), Val::Px(4.0)),
                ..default()
            },
        )).with_children(|h| {
            h.spawn((Text::new(words), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(ThemeColors::TEXT_PRIMARY)));
            h.spawn((Text::new(aside), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(aside_color)));
        });
    }

    commands.entity(content_entity).with_children(|parent| {
        // Credits and how many are taken (no cap: take as many as you like).
        parent.spawn((
            Node {
                column_gap: Val::Px(18.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Sheet::RULE),
        )).with_children(|r| {
            r.spawn((Text::new(format!("credits  {}", currency.credits)), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(ThemeColors::ACCENT_YELLOW)));
            r.spawn((Text::new(format!("taken  {}", state.active_contracts.len())), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(ThemeColors::TEXT_SECONDARY)));
        });

        let mut global_index: usize = 0;

        // Available contracts grouped by faction
        for faction in Faction::all() {
            let faction_contracts: Vec<_> = board.iter()
                .filter(|c| c.faction == *faction)
                .collect();

            if faction_contracts.is_empty() && !state.active_contracts.iter().any(|c| c.faction == *faction) {
                continue;
            }

            section(parent, faction.name().to_string(), format!("standing {:.0}  {}", rep.get(faction), rep.star_string(faction)), Sheet::LABEL);

            for contract in &faction_contracts {
                let deposit_text = if contract.deposit > 0 { format!("  ({} down)", contract.deposit) } else { String::new() };
                job_row(
                    parent,
                    global_index == sel_idx,
                    contract.star_display(),
                    contract.title.clone(),
                    format!("{}{}", contract.reward, deposit_text),
                    ThemeColors::ACCENT_YELLOW,
                );
                global_index += 1;
            }
        }

        // Active contracts section
        if !state.active_contracts.is_empty() {
            section(parent, "TAKEN".to_string(), String::new(), Sheet::LABEL);
            for contract in &state.active_contracts {
                let (status_text, status_color) = match contract.status {
                    ContractStatus::Completed => ("done".to_string(), ThemeColors::STATUS_OK),
                    ContractStatus::Active => (contract.progress_text(), ThemeColors::STATUS_WARN),
                    _ => (contract.progress_text(), Sheet::LABEL),
                };
                job_row(
                    parent,
                    global_index == sel_idx,
                    contract.star_display(),
                    format!("{}  ({})", contract.title, contract.reward),
                    status_text,
                    status_color,
                );
                global_index += 1;
            }
        }
    });
}

// ============================================================================
// CONTRACT HUD (top-right during Exploring)
// ============================================================================

/// Spawns the contract tracker at the top of the HUD's right-hand column, so
/// the notifications below it are laid out under it instead of on top of it.
/// (At top 12 on its own it was drawn over CRED/CREW/CARGO with the
/// notifications running underneath.)
pub fn spawn_contract_hud(
    mut commands: Commands,
    column: Query<Entity, With<crate::ui::RightColumn>>,
) {
    let tracker = commands.spawn((
        (Node {
                // Same width as the notifications under it. Unbounded, a few
                // bounty lines ("... vessel: Hunting - in Ashfall") ran left
                // across the flight-training card; long lines wrap.
                max_width: Val::Px(360.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), ZIndex(50)),
        ContractHudRoot,
    )).with_children(|parent| {
        parent.spawn((
            (Text::new(""), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::WHITE)),
            ContractHudText,
        ));
    }).id();
    if let Ok(column) = column.single() {
        commands.entity(column).insert_children(0, &[tracker]);
    }
}

pub fn despawn_contract_hud(
    mut commands: Commands,
    query: Query<Entity, With<ContractHudRoot>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Shift+J hides/shows the contract tracker. Plain J is the mission board.
pub fn toggle_contract_hud(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<ContractHudVisible>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if !shift || !keyboard.just_pressed(KeyCode::KeyJ) { return; }

    visible.0 = !visible.0;
    notifications.write(ShowNotification {
        message: if visible.0 {
            "Contract tracker shown (Shift+J)".into()
        } else {
            "Contract tracker hidden - Shift+J to bring it back".into()
        },
        notification_type: NotificationType::Info,
        duration: 2.5,
    });
}

/// Where a bounty's ship is, as the tracker says it: a range when it's in
/// this system, the system's name when it isn't. Tracked bounties used to
/// read "Hunting" and nothing else for as long as they were open -- three of
/// them sat that way through a whole playtest with no clue which way to go.
fn bounty_whereabouts(
    target_id: u32,
    live: Option<Vec2>,
    sim: &crate::ai_ship::components::WorldSimulation,
    here: Option<u32>,
    ship_pos: Vec2,
) -> Option<String> {
    if let Some(pos) = live {
        return Some(crate::ui::format_range_km(pos.distance(ship_pos)));
    }
    let ship = sim.ships.iter().find(|s| s.bounty_id == Some(target_id))?;
    if Some(ship.system_id) == here {
        Some(crate::ui::format_range_km(ship.position.distance(ship_pos)))
    } else {
        Some(format!("in {}", crate::celestial::galaxy::system_name(ship.system_id)))
    }
}

pub fn update_contract_hud(
    state: Res<ContractState>,
    visible: Res<ContractHudVisible>,
    mut root_query: Query<&mut Node, With<ContractHudRoot>>,
    mut text_query: Query<&mut Text, With<ContractHudText>>,
    sim: Res<crate::ai_ship::components::WorldSimulation>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    player: Query<&Transform, With<crate::components::Ship>>,
    live_bounties: Query<(&Transform, &crate::ai_ship::components::BountyTarget)>,
    jobs: Res<crate::diplomacy::jobs::Jobs>,
) {
    // Hide the whole panel — not just its text — when it's toggled off or
    // there's nothing to track, so its background box doesn't sit empty in the
    // corner.
    let show = visible.0 && (!state.active_contracts.is_empty() || jobs.active().next().is_some());
    if let Ok(mut node) = root_query.single_mut() {
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }

    let Ok(mut text) = text_query.single_mut() else { return; };

    if !show {
        text.0 = String::new();
        return;
    }

    let ship_pos = player.single().map(|t| t.translation.truncate()).unwrap_or(Vec2::ZERO);
    let mut lines = Vec::new();
    for contract in &state.active_contracts {
        let mut status = match contract.status {
            ContractStatus::Completed => "DONE".to_string(),
            _ => contract.progress_text(),
        };
        if let ContractObjective::DestroyShip { target_id, destroyed: false, .. } = &contract.objective {
            if contract.status != ContractStatus::Completed {
                let live = live_bounties
                    .iter()
                    .find(|(_, b)| b.0 == *target_id)
                    .map(|(t, _)| t.translation.truncate());
                if let Some(at) = bounty_whereabouts(*target_id, live, &sim, streaming.loaded_system, ship_pos) {
                    status = format!("{status} - {at}");
                }
            }
        }
        lines.push(format!(
            "{} {}: {}",
            contract.star_display(),
            contract.title,
            status,
        ));
    }
    // Work taken from contacts at the starports.
    for job in jobs.active() {
        let mut line = format!("> {}: {}", job.title, job.progress());
        if let Some(at) = job_whereabouts(job, &sim, streaming.loaded_system, ship_pos) {
            line = format!("{line} - {at}");
        }
        lines.push(line);
    }

    text.0 = lines.join("\n");
}

/// Where a contact job's business is: a range if it's in this system, the
/// system's name if not.
fn job_whereabouts(
    job: &crate::diplomacy::jobs::Job,
    sim: &crate::ai_ship::components::WorldSimulation,
    here: Option<u32>,
    ship_pos: Vec2,
) -> Option<String> {
    use crate::diplomacy::jobs::JobKind;
    let elsewhere = |id: u32| format!("in {}", crate::celestial::galaxy::system_name(id));
    match &job.kind {
        JobKind::Bounty { fleet, system } | JobKind::Raid { fleet, system } | JobKind::Escort { fleet, system, .. } => {
            if here != Some(*system) {
                return Some(elsewhere(*system));
            }
            sim.ships
                .iter()
                .filter(|s| s.fleet == *fleet && s.behavior != crate::ai_ship::components::SimBehavior::Dead)
                .map(|s| s.position.distance(ship_pos))
                .min_by(|a, b| a.total_cmp(b))
                .map(crate::ui::format_range_km)
        }
        JobKind::HoldTheLine { system, .. } | JobKind::Survey { system, .. } => (here != Some(*system)).then(|| elsewhere(*system)),
        JobKind::Strike { system, target, .. } => {
            if here != Some(*system) {
                return Some(elsewhere(*system));
            }
            sim.ships
                .iter()
                .filter(|s| s.system_id == *system && s.faction == *target && s.behavior != crate::ai_ship::components::SimBehavior::Dead)
                .map(|s| s.position.distance(ship_pos))
                .min_by(|a, b| a.total_cmp(b))
                .map(crate::ui::format_range_km)
        }
        JobKind::Handoff { system, .. } => Some(if here == Some(*system) { "wait here".to_string() } else { elsewhere(*system) }),
        JobKind::Theft { system, taken: false, .. } => (here != Some(*system)).then(|| elsewhere(*system)),
        JobKind::Theft { to_system, taken: true, .. } => Some(if here == Some(*to_system) { "dock to hand it over".to_string() } else { format!("to {}", crate::celestial::galaxy::system_name(*to_system)) }),
        JobKind::Delivery { to_system, .. } => Some(if here == Some(*to_system) { "dock to deliver".to_string() } else { format!("to {}", crate::celestial::galaxy::system_name(*to_system)) }),
    }
}

#[cfg(test)]
mod bounty_whereabouts_tests {
    use super::*;
    use crate::ai_ship::components::{AiShipType, SimulatedShip, WorldSimulation};

    fn sim_with(system_id: u32, position: Vec2) -> WorldSimulation {
        let mut sim = WorldSimulation::default();
        let mut ship = SimulatedShip::patrolling(system_id, AiShipType::RecursiveKingdom, position, position, 1000.0, 100.0);
        ship.bounty_id = Some(7);
        sim.ships.push(ship);
        sim
    }

    #[test]
    fn a_target_in_this_system_reads_as_a_range() {
        let sim = sim_with(0, Vec2::new(12_000.0, 0.0));
        assert_eq!(bounty_whereabouts(7, None, &sim, Some(0), Vec2::ZERO).as_deref(), Some("12 km"));
    }

    #[test]
    fn a_target_elsewhere_names_its_system() {
        let sim = sim_with(2, Vec2::ZERO);
        assert_eq!(bounty_whereabouts(7, None, &sim, Some(0), Vec2::ZERO).as_deref(), Some("in Calder"));
    }

    /// Once it's a real ship nearby, its live position wins.
    #[test]
    fn a_live_target_uses_where_it_actually_is() {
        let sim = sim_with(0, Vec2::new(50_000.0, 0.0));
        assert_eq!(bounty_whereabouts(7, Some(Vec2::new(3_000.0, 0.0)), &sim, Some(0), Vec2::ZERO).as_deref(), Some("3.0 km"));
    }
}

#[cfg(test)]
mod tracker_column_tests {
    use super::*;

    /// The tracker goes in the right-hand column, ahead of the notifications,
    /// so layout puts the notices under it.
    #[test]
    fn the_tracker_heads_the_right_column() {
        let mut app = App::new();
        app.add_systems(Update, spawn_contract_hud);
        let notices = app.world_mut().spawn(Node::default()).id();
        let column = app.world_mut().spawn((Node::default(), crate::ui::RightColumn)).id();
        app.world_mut().entity_mut(column).add_child(notices);
        app.update();
        let children: Vec<Entity> = app.world().get::<Children>(column).unwrap().iter().collect();
        assert_eq!(children.len(), 2);
        assert!(app.world().get::<ContractHudRoot>(children[0]).is_some(), "tracker isn't first");
        assert_eq!(children[1], notices);
    }
}
