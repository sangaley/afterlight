use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::celestial::resources::{GalaxyMap, GalaxyState, LootedPois, SystemStreamingManager, SystemDiscovery};
use crate::celestial::galaxy::{generate_galaxy_map, nearest_neighbors, nearest_neighbors_to_pos, WARM_NEIGHBOR_COUNT};
use crate::events::*;
use crate::states::GameState;

const SAVE_DIR: &str = "saves";
const MAX_SLOTS: u32 = 3;
const AUTO_SAVE_SLOT: u32 = 99; // Special slot number for auto-save

pub struct MetaPlugin;

impl Plugin for MetaPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<Unlocks>()
            .init_resource::<Statistics>()
            .init_resource::<Inventory>()
            .init_resource::<Currency>()
            .init_resource::<AutoSaveTimer>()
            .init_resource::<SaveLoadMenuState>()
            .init_resource::<PendingLoad>()
            .init_resource::<PendingEntityRebuild>()
            .add_systems(Startup, load_unlocks)
            .add_systems(OnEnter(GameState::GameOver), save_unlocks)
            // Ungated (like handle_load_request): it only acts when a
            // SaveGameRequest event is present, and quick-save (F5) fires that
            // during Exploring — a Paused-only gate here silently dropped those.
            // Must run before the state change lands, so the wipe is in place
            // by the time StationDocked spawns the starter ship and crew.
            .add_systems(Update, reset_for_new_game)
            .add_systems(Update, handle_save_request)
            .add_systems(Update, handle_load_request)
            .add_systems(Update, apply_pending_load)
            .add_systems(Update, rebuild_entities_from_save)
            .add_systems(Update, apply_module_health_overrides)
            .add_systems(Last, despawn_at_frame_end)
            .add_systems(
                Update,
                auto_save_system.run_if(
                    in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))
                ),
            );
    }
}

/// Load unlocks from file on startup
fn load_unlocks(mut unlocks: ResMut<Unlocks>) {
    let path = "meta/unlocks.json";
    if let Ok(data) = std::fs::read_to_string(path) {
        if let Ok(loaded) = serde_json::from_str::<Unlocks>(&data) {
            *unlocks = loaded;
            info!("Loaded unlocks from {}", path);
        }
    }
}

/// Save unlocks to file on game over
fn save_unlocks(unlocks: Res<Unlocks>) {
    let _ = std::fs::create_dir_all("meta");
    let path = "meta/unlocks.json";
    if let Ok(data) = serde_json::to_string_pretty(unlocks.as_ref()) {
        let _ = std::fs::write(path, data);
        info!("Saved unlocks to {}", path);
    }
}

// ============================================================================
// SAVE SYSTEM
// ============================================================================

/// The player's own ship, for saving: the `Ship` root and what hangs off it.
///
/// Blocks are kept by WHO THEY BELONG TO, not by what they lack. Enemy hull,
/// modules and crew carry the same components as the player's, and hull cells
/// are recorded from LOCAL transforms -- so an enemy plate at its own (2,3)
/// was written into the save as the player's (2,3), and came back bolted onto
/// the player's ship (one save held five blocks on a cell). Excluding
/// `OwnedByAiShip` fixed the enemies, then shot-off sections needed their own
/// exclusion; a child-of-the-ship rule covers both and whatever comes next.
///
/// Crew are the one exception, because the player's crew are not always
/// aboard: EVA salvagers are deliberately unparented, and a hand bought from
/// the hiring menu has no parent until `reconcile_hired_crew` runs. So crew
/// count if they are the ship's children, or nobody's and not an AI ship's.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerShipParts<'w, 's> {
    ship: Query<'w, 's, (Entity, &'static Transform), With<Ship>>,
    modules: Query<'w, 's, (
        &'static Module,
        Option<&'static CustomModule>,
        Option<&'static crate::building::customization::tuning::WeaponTuning>,
        Option<&'static crate::building::customization::tuning::SelectedAmmo>,
        &'static ChildOf,
        // Built guns, launchers and turrets: what they stand on and are set to.
        (
            Option<&'static crate::combat::turret_deck::OnTurret>,
            Option<&'static crate::combat::shell::GunShell>,
            Option<&'static crate::combat::missile_design::MissileLoadout>,
            Has<crate::combat::targeting::fire_groups::InterceptMode>,
            Option<&'static crate::diplomacy::signatures::Signature>,
        ),
    )>,
    hull: Query<'w, 's, (&'static HullSegment, &'static Transform, &'static ChildOf)>,
    /// The ship's turrets: their gun parts and roofs are the turret's
    /// children, one level down, and are the ship's as much as the hull is.
    /// A turret whose ring was shot out is saved with that ring at 1 HP.
    turrets: Query<'w, 's, (Entity, &'static crate::combat::turret_deck::TurretBody, &'static ChildOf)>,
    live_modules: Query<'w, 's, (), (With<Module>, Without<crate::components::DestroyedModule>)>,
    crew: Query<'w, 's, (
        &'static CrewMember,
        Option<&'static CrewDuty>,
        Option<&'static ChildOf>,
    ), Without<crate::ai_ship::components::OwnedByAiShip>>,
}

/// The player ship's blocks and crew, in save form.
struct ShipParts {
    position: Vec2,
    modules: Vec<ModuleData>,
    hull_segments: Vec<HullSaveData>,
    crew: Vec<CrewSaveData>,
}

impl PlayerShipParts<'_, '_> {
    /// `None` without exactly one player ship -- nothing to save, and writing
    /// an empty ship over the slot would be worse than not saving.
    fn collect(&self) -> Option<ShipParts> {
        let (ship, transform) = self.ship.single().ok()?;

        let on_ship: Vec<Entity> = self.turrets.iter().filter(|(_, _, p)| p.parent() == ship).map(|(e, _, _)| e).collect();
        let mut modules: Vec<ModuleData> = self.modules
            .iter()
            .filter(|(.., parent, _)| parent.parent() == ship || on_ship.contains(&parent.parent()))
            .map(|(module, custom, tuning, selected_ammo, _, (on_turret, shell, missile, point_defence, signature))| {
                let custom_data = custom.map(|c| CustomModuleData {
                    custom_name: c.custom_name.clone(),
                    subcomponents: Vec::new(),
                });
                ModuleData {
                    module_type: module.module_type,
                    grid_position: module.grid_position,
                    health: module.health,
                    rotation: module.rotation,
                    is_active: module.is_active,
                    custom_data,
                    customization_params: None, // Tier 3 params saved in future version
                    tuning: tuning.copied(),
                    selected_ammo: selected_ammo.map(|s| s.0),
                    turret: on_turret.map(|t| t.ring_cell),
                    shell: shell.map(|s| s.0.clone()),
                    missile: missile.map(|m| m.0.clone()),
                    point_defence,
                    signature: signature.map(|s| s.0.id.to_string()),
                }
            })
            .collect();
        // A turret whose ring is shot out still has its parts aboard, and
        // they're saved standing on it. A destroyed ring is gone from the
        // save like any destroyed block, so on load they'd wait for a ring
        // that never comes. Save the ring patched together at 1 HP instead.
        for (_, turret, parent) in self.turrets.iter() {
            if turret.jammed && parent.parent() == ship && !self.live_modules.contains(turret.ring) {
                modules.push(ModuleData {
                    module_type: crate::combat::turret_deck::ring_type(turret.size),
                    grid_position: turret.ring_cell,
                    health: 1.0,
                    rotation: turret.ring_rotation,
                    is_active: true,
                    custom_data: None,
                    customization_params: None,
                    tuning: None,
                    selected_ammo: None,
                    turret: None,
                    shell: None,
                    missile: None,
                    point_defence: false,
                    signature: None,
                });
            }
        }

        // Hull segments with material info
        let hull_segments = self.hull
            .iter()
            .filter(|(.., parent)| parent.parent() == ship)
            .map(|(seg, transform, _)| {
                let grid_x = (transform.translation.x / 66.0).round() as i32;
                let grid_y = ((transform.translation.y + 33.0) / 66.0).round() as i32;
                HullSaveData {
                    grid_position: IVec2::new(grid_x, grid_y),
                    health: seg.health,
                    max_health: seg.max_health,
                    radiation_shielding: seg.radiation_shielding,
                    material: seg.material,
                    hull_layer: seg.hull_layer,
                }
            })
            .collect();

        // Crew (auto-assign restores assignments on load)
        let crew = self.crew
            .iter()
            .filter(|(.., parent)| parent.is_none_or(|p| p.parent() == ship))
            .map(|(member, duty, _)| CrewSaveData {
                name: member.name.clone(),
                health: member.health,
                max_health: member.max_health,
                oxygen: member.oxygen,
                morale: member.morale,
                assigned_module_grid: None,
                duty: duty.copied().unwrap_or_default(),
            })
            .collect();

        Some(ShipParts {
            position: transform.translation.truncate(),
            modules,
            hull_segments,
            crew,
        })
    }
}

/// Get save file path for a given slot
fn save_path(slot: u32) -> String {
    if slot == AUTO_SAVE_SLOT {
        format!("{}/autosave.json", SAVE_DIR)
    } else {
        format!("{}/slot_{}.json", SAVE_DIR, slot)
    }
}

/// Read save slot info without loading the full save
pub fn read_slot_info(slot: u32) -> Option<SaveSlotInfo> {
    let path = save_path(slot);
    let data = std::fs::read_to_string(&path).ok()?;
    let save: SaveData = serde_json::from_str(&data).ok()?;
    Some(save.slot_info)
}

/// Check which slots have saves
pub fn get_save_slots() -> Vec<(u32, Option<SaveSlotInfo>)> {
    let mut slots = Vec::new();
    for i in 0..MAX_SLOTS {
        slots.push((i, read_slot_info(i)));
    }
    // Also check auto-save
    slots.push((AUTO_SAVE_SLOT, read_slot_info(AUTO_SAVE_SLOT)));
    slots
}

/// The state a load should drop you back into. Paused is never it: the
/// pause-menu save keys only work while paused, so a save made mid-flight
/// recorded "not exploring" and loaded you docked at a station, wherever you
/// had been. Paused saves record the state you paused from -- the same one
/// Escape resumes into, with the same fallback.
fn saved_flight_state(current: GameState, pre_pause: &PrePauseState) -> GameState {
    match current {
        GameState::Paused => pre_pause.0.unwrap_or(GameState::Exploring),
        other => other,
    }
}

/// Collect current game state into SaveData
fn collect_save_data(
    slot: u32,
    depth_state: &DepthState,
    hull_state: &HullState,
    statistics: &Statistics,
    inventory: &Inventory,
    currency: &Currency,
    unlocks: &Unlocks,
    discovered_locations: &DiscoveredLocations,
    world_state: &WorldState,
    ship_parts: ShipParts,
    flight_state: GameState,
    galaxy_map: &GalaxyMap,
    streaming: &SystemStreamingManager,
    drifting_dead: &crate::crew::burial::DriftingDead,
    looted: &LootedPois,
    fuel: &FuelState,
    contracts: &crate::contracts::ContractState,
    reputation: &crate::contracts::FactionReputation,
    standing: &crate::diplomacy::FactionStanding,
    sim: &crate::ai_ship::components::WorldSimulation,
    stations: &crate::diplomacy::stations::FactionStations,
    war: &crate::celestial::territory::BorderWar,
    contacts: &crate::diplomacy::contacts::Contacts,
    jobs: &crate::diplomacy::jobs::Jobs,
    galaxy_time: f64,
) -> SaveData {
    let ShipParts { position, modules, hull_segments, crew } = ship_parts;

    // Timestamp
    let timestamp = format!("Save {}", slot);

    SaveData {
        version: 1,
        slot_info: SaveSlotInfo {
            slot,
            timestamp,
            depth: depth_state.current_depth,
            play_time: statistics.play_time_seconds,
            hull_integrity: hull_state.hull_integrity,
        },
        ship: ShipBlueprint {
            hull_segments: Vec::new(), // Legacy field
            modules,
        },
        hull_segments,
        crew,
        inventory: inventory.clone(),
        currency: currency.clone(),
        unlocks: unlocks.clone(),
        statistics: statistics.clone(),
        discovered_locations: discovered_locations.clone(),
        position,
        current_depth: depth_state.current_depth,
        world_seed: world_state.seed,
        was_exploring: flight_state == GameState::Exploring,
        current_system_id: streaming.loaded_system.unwrap_or(0),
        galaxy_seed: galaxy_map.galaxy_seed,
        drifting_dead: drifting_dead.bodies.clone(),
        galaxy_systems: galaxy_map.systems.iter().map(|s| SystemSaveData {
            id: s.id,
            discovery: s.discovery.as_u8(),
            last_updated: s.last_updated,
            resource_fraction_remaining: s.resource_fraction_remaining,
            looted_pois: looted.for_system(s.id).to_vec(),
            held: Some(crate::resources::HeldBy { owner: s.faction, capital: s.capital }),
            dark: s.dark,
        }).collect(),
        uncharted_galaxy_pos: streaming.loaded_system.is_none().then_some(streaming.current_galaxy_pos),
        fuel: Some(fuel.current_fuel),
        contracts: contracts.clone(),
        reputation: reputation.clone(),
        standing: Some(standing.clone()),
        world: sim.initialized.then(|| sim.snapshot()),
        destroyed_stations: stations.destroyed.clone(),
        border_war: Some(war.clone()),
        contacts: Some(contacts.clone()),
        jobs: Some(jobs.clone()),
        galaxy_time,
    }
}

/// Write save data to disk
fn write_save(save_data: &SaveData, slot: u32) -> bool {
    let _ = std::fs::create_dir_all(SAVE_DIR);
    let path = save_path(slot);
    match serde_json::to_string_pretty(save_data) {
        Ok(json) => {
            match std::fs::write(&path, json) {
                Ok(_) => {
                    info!("Game saved to {}", path);
                    true
                }
                Err(e) => {
                    error!("Failed to write save file {}: {}", path, e);
                    false
                }
            }
        }
        Err(e) => {
            error!("Failed to serialize save data: {}", e);
            false
        }
    }
}

/// Handle save game requests
fn handle_save_request(
    mut save_events: MessageReader<SaveGameRequest>,
    mut saved_events: MessageWriter<GameSaved>,
    mut notify_events: MessageWriter<ShowNotification>,
    depth_state: Res<DepthState>,
    hull_state: Res<HullState>,
    statistics: Res<Statistics>,
    inventory: Res<Inventory>,
    currency: Res<Currency>,
    unlocks: Res<Unlocks>,
    discovered_locations: Res<DiscoveredLocations>,
    // Bundled to stay under Bevy's 16-param cap: local world seed + galaxy state
    // + the drifting dead, and the run state that isn't the ship (fuel,
    // contracts, standing). Bevy caps a system at 16 params; this one is at
    // 14, so a big addition still belongs in a tuple.
    world_galaxy: (
        Res<WorldState>,
        Res<GalaxyMap>,
        Res<SystemStreamingManager>,
        Res<crate::crew::burial::DriftingDead>,
        Res<LootedPois>,
        Res<FuelState>,
        Res<crate::contracts::ContractState>,
        Res<crate::contracts::FactionReputation>,
        Res<crate::diplomacy::FactionStanding>,
        Res<crate::ai_ship::components::WorldSimulation>,
        Res<crate::diplomacy::stations::FactionStations>,
        Res<crate::celestial::territory::BorderWar>,
        Res<crate::diplomacy::contacts::Contacts>,
        Res<crate::diplomacy::jobs::Jobs>,
        Res<GalaxyState>,
    ),
    current_state: Res<State<GameState>>,
    pre_pause: Res<PrePauseState>,
    player_ship: PlayerShipParts,
) {
    let (world_state, galaxy_map, streaming, drifting_dead, looted, fuel, contracts, reputation, standing, sim, faction_stations, war, people, work, spawned) = &world_galaxy;
    for event in save_events.read() {
        let Some(ship_parts) = player_ship.collect() else {
            error!("Save skipped: no single player ship to save");
            saved_events.write(GameSaved { slot: event.slot, success: false });
            notify_events.write(ShowNotification {
                message: "Save failed!".to_string(),
                notification_type: NotificationType::Danger,
                duration: 3.0,
            });
            continue;
        };
        let save_data = collect_save_data(
            event.slot,
            &depth_state,
            &hull_state,
            &statistics,
            &inventory,
            &currency,
            &unlocks,
            &discovered_locations,
            world_state,
            ship_parts,
            saved_flight_state(*current_state.get(), &pre_pause),
            galaxy_map,
            streaming,
            drifting_dead,
            looted,
            fuel,
            contracts,
            reputation,
            standing,
            sim,
            faction_stations,
            war,
            people,
            work,
            spawned.galaxy_time,
        );

        // Tier 3 customization is saved via customization_params field
        // (populated when ModuleCustomization query is added in a future refactor)

        let success = write_save(&save_data, event.slot);

        saved_events.write(GameSaved {
            slot: event.slot,
            success,
        });

        let msg = if success {
            if event.slot == AUTO_SAVE_SLOT {
                "Auto-save complete".to_string()
            } else {
                format!("Game saved to slot {}", event.slot + 1)
            }
        } else {
            "Save failed!".to_string()
        };

        notify_events.write(ShowNotification {
            message: msg,
            notification_type: if success { NotificationType::Success } else { NotificationType::Danger },
            duration: 3.0,
        });
    }
}

// ============================================================================
// LOAD SYSTEM
// ============================================================================

/// Pending load data resource - parsed save data waiting to be applied
#[derive(Resource, Default)]
struct PendingLoad(Option<(u32, SaveData)>);

/// Phase 1: Read save file and store in PendingLoad resource
fn handle_load_request(
    mut load_events: MessageReader<LoadGameRequest>,
    mut notify_events: MessageWriter<ShowNotification>,
    mut pending: ResMut<PendingLoad>,
) {
    for event in load_events.read() {
        let path = save_path(event.slot);
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to read save file {}: {}", path, e);
                notify_events.write(ShowNotification {
                    message: "Load failed: file not found".to_string(),
                    notification_type: NotificationType::Danger,
                    duration: 3.0,
                });
                continue;
            }
        };

        let save: SaveData = match serde_json::from_str(&data) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to parse save file: {}", e);
                notify_events.write(ShowNotification {
                    message: "Load failed: corrupted save".to_string(),
                    notification_type: NotificationType::Danger,
                    duration: 3.0,
                });
                continue;
            }
        };

        pending.0 = Some((event.slot, save));
    }
}

/// Phase 2: Apply pending load data to game state
fn apply_pending_load(
    mut pending: ResMut<PendingLoad>,
    mut depth_state: ResMut<DepthState>,
    mut hull_state: ResMut<HullState>,
    mut statistics: ResMut<Statistics>,
    mut inventory: ResMut<Inventory>,
    mut currency: ResMut<Currency>,
    mut unlocks: ResMut<Unlocks>,
    mut discovered_locations: ResMut<DiscoveredLocations>,
    mut world_state: ResMut<WorldState>,
    run: (
        ResMut<FuelState>,
        ResMut<crate::contracts::ContractState>,
        ResMut<crate::contracts::FactionReputation>,
        ResMut<crate::diplomacy::FactionStanding>,
    ),
    mut commands: Commands,
) {
    let (mut fuel, mut contracts, mut reputation, mut standing) = run;
    let Some((slot, save)) = pending.0.take() else { return };

    // ---- Restore resources ----
    depth_state.current_depth = save.current_depth;
    depth_state.target_depth = save.current_depth;
    *statistics = save.statistics.clone();
    *inventory = save.inventory.clone();
    *currency = save.currency.clone();
    *unlocks = save.unlocks.clone();
    *discovered_locations = save.discovered_locations.clone();
    world_state.seed = save.world_seed;
    hull_state.hull_integrity = save.slot_info.hull_integrity;
    // None of these were saved. A load from the menu came back with a full
    // tank (cleanup refills it) and an in-game one kept whatever was in it;
    // either way the contracts and standing were the session's, not the save's.
    // Not clamped to max_fuel: signature tanks raise the cap
    // (diplomacy::signatures::fuel_capacity), and they're respawned after this.
    fuel.current_fuel = save.fuel.unwrap_or(fuel.max_fuel).max(0.0);
    *contracts = save.contracts.clone();
    *reputation = save.reputation.clone();
    *standing = save.standing.clone().unwrap_or_default();

    // Store save data in a resource for entity rebuild (handled in phase 3)
    commands.insert_resource(PendingEntityRebuild {
        save_data: Some(save),
        slot,
    });
}

/// Save data waiting for entity rebuild
#[derive(Resource, Default)]
struct PendingEntityRebuild {
    save_data: Option<SaveData>,
    slot: u32,
}

/// Phase 3: Despawn old entities and respawn from save data
fn rebuild_entities_from_save(
    mut pending: ResMut<PendingEntityRebuild>,
    mut loaded_events: MessageWriter<GameLoaded>,
    mut notify_events: MessageWriter<ShowNotification>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    registry: Res<crate::building::registry::ModuleRegistry>,
    ship_query: Query<Entity, With<Ship>>,
    // Mirror of the save-side defect: unscoped, these despawned every live AI
    // ship's blocks and crew, gutting them into empty root entities on load.
    module_entities: Query<Entity, (With<Module>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    // A turret's plating goes with its turret body (despawned with it), and
    // an enemy's must stay: enemy ships carry on through a load.
    hull_entities: Query<Entity, (With<HullSegment>, Without<crate::ai_ship::components::OwnedByAiShip>, Without<crate::combat::turret_deck::TurretPlate>)>,
    crew_entities: Query<Entity, (With<CrewMember>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    mut galaxy_map: ResMut<GalaxyMap>,
    mut streaming: ResMut<SystemStreamingManager>,
    mut drifting_dead: ResMut<crate::crew::burial::DriftingDead>,
    // The spawned star system and its population. AT Bevy's 16-param cap:
    // anything new joins one of these tuples.
    world: (
        ResMut<GalaxyState>,
        ResMut<LootedPois>,
        ResMut<crate::ai_ship::components::WorldSimulation>,
        Res<crate::vfx::procedural_textures::CelestialTextures>,
        Res<Time>,
        // Not AI ships: a faction starport is a member of its system too, and
        // despawned here as well as tagged LeftByLoad below, every command
        // already queued for its blocks that frame hit nothing (16 warnings
        // per load beside a starport). It leaves with the other AI ships.
        Query<Entity, (With<crate::celestial::components::StarSystemMember>, Without<crate::ai_ship::components::AiShip>)>,
        Query<Entity, With<crate::ai_ship::components::AiShip>>,
        ResMut<crate::diplomacy::stations::FactionStations>,
        ResMut<crate::celestial::territory::BorderWar>,
        ResMut<crate::diplomacy::contacts::Contacts>,
        ResMut<crate::diplomacy::jobs::Jobs>,
    ),
    // State the old ship was in the middle of, that the loaded one is not.
    flight: (
        ResMut<crate::ship::rebuild::RebuildQueue>,
        ResMut<crate::ship::towing::Towing>,
        ResMut<crate::ui::PendingWarpTarget>,
        // The player's turret bodies only: an enemy's turrets, and the guns
        // standing on them, carry on with their ship.
        Query<(Entity, &ChildOf), With<crate::combat::turret_deck::TurretBody>>,
        Query<(), With<crate::ai_ship::components::AiShip>>,
    ),
) {
    let (mut galaxy, mut looted, mut sim, textures, time, members, ai_ships, mut faction_stations, mut border_war, mut people, mut work) = world;
    let (mut rebuild_queue, mut towing, mut dash_target, turret_bodies, ai_roots) = flight;
    let player_turrets: Vec<Entity> = turret_bodies.iter().filter(|(_, p)| !ai_roots.contains(p.parent())).map(|(e, _)| e).collect();
    let Some(save) = pending.save_data.take() else { return };
    let slot = pending.slot;

    // ---- Despawn existing ship entities ----
    // Hidden now, gone at the end of the frame (despawn_at_frame_end).
    // Despawned here, mid-frame, any system that had already queued a
    // command for one of them this frame crashed the load; which ones did
    // depended on scheduling order, so it came and went as systems were added.
    // Turret bodies too. One whose ring was shot out is kept waiting for its
    // ring to be rebuilt -- it would outlive the load, stripped of its plating
    // (a HullSegment, so it goes above), and the loaded ring would take it on
    // instead of building the turret fresh.
    for entity in module_entities.iter().chain(hull_entities.iter()).chain(crew_entities.iter()).chain(player_turrets.iter().copied()) {
        commands.entity(entity).try_insert((DespawnAtFrameEnd, Visibility::Hidden));
    }

    // ---- Respawn ship at saved position ----
    // The same root a new run gets (ship::ship_root_bundle). Over an existing
    // root it also zeroes the old ship's motion: only the Transform used to be
    // replaced, so a quick-load mid-burn arrived still moving, spinning and
    // throttled up.
    let root = crate::ship::ship_root_bundle(save.position);
    let ship_entity = if let Ok(existing) = ship_query.single() {
        commands
            .entity(existing)
            .insert(root)
            // A jump or dash charging when F9 was pressed is the old ship's.
            .remove::<(crate::celestial::components::WarpCharging, crate::ui::MapWarpCharging)>();
        existing
    } else {
        commands.spawn(root).id()
    };
    commands.entity(ship_entity).insert(Depth(save.current_depth));
    *towing = default();
    dash_target.0 = None;
    // Ghosts are the old ship's missing blocks; left queued, crew rebuilt
    // them onto the loaded one. Their outline sprites hang off the root,
    // which survives the load, so they go explicitly.
    for ghost in rebuild_queue.ghosts.drain(..) {
        for sprite in ghost.sprites {
            commands.entity(sprite).try_despawn();
        }
    }

    let roster_members = respawn_ship_parts(
        &mut commands,
        &asset_server,
        &registry,
        ship_entity,
        &save.hull_segments,
        &save.ship.modules,
        &save.crew,
    );
    commands.insert_resource(CrewRoster { members: roster_members });
    // Auto-assign will re-staff stations on next tick

    // ---- Restore the drifting dead ----
    // Positions are world-space per system; the entities for whichever system
    // the player lands in are spawned by burial::sync_drifting_dead on its
    // next run, so nothing is spawned here.
    drifting_dead.bodies = save.drifting_dead.clone();
    drifting_dead.reseed();

    // ---- Restore galaxy state ----
    // Regenerate the deterministic layout from the saved seed, then overlay the
    // persisted per-system mutable state (discovery / depletion) and re-point the
    // streaming manager at the system the player was in. Legacy saves have
    // galaxy_seed == 0 → leave the freshly-generated galaxy alone.
    if save.galaxy_seed != 0 {
        *galaxy_map = generate_galaxy_map(save.galaxy_seed);
        looted.0.clear();
        for sd in &save.galaxy_systems {
            if let Some(sys) = galaxy_map.systems.iter_mut().find(|s| s.id == sd.id) {
                sys.discovery = SystemDiscovery::from_u8(sd.discovery);
                sys.last_updated = sd.last_updated;
                sys.resource_fraction_remaining = sd.resource_fraction_remaining;
                sys.dark = sd.dark;
                if let Some(held) = sd.held {
                    sys.faction = held.owner;
                    sys.capital = held.capital;
                    sys.danger_tier = crate::celestial::territory::danger_of(sys);
                }
            }
            for &poi_slot in &sd.looted_pois {
                looted.mark(sd.id, poi_slot);
            }
        }
        if let Some(pos) = save.uncharted_galaxy_pos {
            // Saved after a blind jump into empty space: no system is Hot.
            streaming.loaded_system = None;
            streaming.current_galaxy_pos = pos;
            streaming.warm_systems = nearest_neighbors_to_pos(&galaxy_map, pos, None, WARM_NEIGHBOR_COUNT);
        } else {
            let cur = save.current_system_id;
            streaming.loaded_system = Some(cur);
            streaming.current_galaxy_pos = galaxy_map.systems.iter()
                .find(|s| s.id == cur)
                .map(|s| s.galaxy_pos)
                .unwrap_or(Vec2::ZERO);
            streaming.warm_systems = nearest_neighbors(&galaxy_map, cur, WARM_NEIGHBOR_COUNT);
        }

        // ---- The star system itself ----
        // This used to stop at pointing the streaming manager at the saved
        // system. Nothing despawned what was there or spawned what should be:
        // an in-game load kept the current system's bodies around a ship now
        // somewhere else, and a load from the menu got Haven whatever the
        // save said. Clear every system, then arrive in the saved one the way
        // a warp does.
        for entity in members.iter() {
            commands.entity(entity).try_despawn();
        }
        galaxy.systems.clear();
        // The orbital clock, which every planet is placed by, comes from the
        // save too. It was left alone: the way to the menu resets it to zero
        // and an in-game load kept this session's, so either way the planets
        // came back somewhere other than where the save left them.
        galaxy.galaxy_time = save.galaxy_time;
        if let Some(cur) = streaming.loaded_system {
            let now = time.elapsed_secs_f64();
            if let Some(info) = crate::celestial::galaxy::load_system(
                &mut commands, &asset_server, &textures, &mut galaxy_map, cur, now, &looted,
            ) {
                galaxy.current_system = cur;
                galaxy.next_system_id = cur + 1;
                galaxy.systems.push(info);
                galaxy.total_bodies = 1;
            }
        }
        // ...and its people, as the save left them: every fleet where it was,
        // as hurt as it was, the dead still dead. The ships on screen now
        // belong to the world being left; they go, and the saved ones
        // materialise from the restored simulation. An older save without a
        // world keeps the running one.
        if let Some(world) = &save.world {
            // Tagged, not despawned here: every other system this frame may
            // already hold a command for these ships, and a despawn in the
            // middle of the frame turned those into crashes. They go at the
            // end of the frame (ai_ship::clear_ships_left_by_load).
            for ship in ai_ships.iter() {
                commands.entity(ship).try_insert((
                    crate::ai_ship::components::LeftByLoad { frames: 3 },
                    Visibility::Hidden,
                    Transform::from_xyz(1.0e8, 1.0e8, 0.0),
                ));
            }
            sim.restore(world);
            faction_stations.live = None;
            faction_stations.destroyed = save.destroyed_stations.clone();
            *border_war = save.border_war.clone().unwrap_or_default();
            *people = save.contacts.clone().unwrap_or_default();
            *work = save.jobs.clone().unwrap_or_default();
        }
        // A running simulation populates the saved system
        // and its neighbours now (a no-op for any already populated). One not
        // yet started -- a load from the menu -- is left to
        // init_world_simulation, which does the same on entering Exploring.
        // Either way the restored bounties then need ships to point at:
        // contracts::tracking::reattach_bounties_after_load.
        if sim.initialized {
            if let Some(cur) = streaming.loaded_system {
                crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy_map, cur);
            }
            for warm_id in streaming.warm_systems.clone() {
                crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy_map, warm_id);
            }
        }
        info!(
            "Galaxy restored: seed={}, current system={:?}, {} systems",
            save.galaxy_seed, streaming.loaded_system, galaxy_map.systems.len()
        );
    }

    // ---- Set game state ----
    if save.was_exploring {
        next_state.set(GameState::Exploring);
    } else {
        next_state.set(GameState::StationDocked);
    }

    loaded_events.write(GameLoaded { slot, success: true });

    let slot_name = if slot == AUTO_SAVE_SLOT {
        "auto-save".to_string()
    } else {
        format!("slot {}", slot + 1)
    };
    notify_events.write(ShowNotification {
        message: format!("Game loaded from {}", slot_name),
        notification_type: NotificationType::Success,
        duration: 3.0,
    });

    info!("Game loaded (depth: {:.0}m)", save.current_depth);
}

/// Puts saved hull, modules and crew back onto `ship_entity` as its children.
/// Returns the crew, for the roster. Split out of `rebuild_entities_from_save`
/// so a save -> load round trip can be tested without the rest of the world.
fn respawn_ship_parts(
    commands: &mut Commands,
    asset_server: &AssetServer,
    registry: &crate::building::registry::ModuleRegistry,
    ship_entity: Entity,
    hull_segments: &[HullSaveData],
    modules: &[ModuleData],
    crew: &[CrewSaveData],
) -> Vec<Entity> {
    // ---- Respawn hull segments ----
    for hull_data in hull_segments {
        let hull_texture = asset_server.load(
            crate::sprite_map::hull_layer_sprite_path(hull_data.material, hull_data.hull_layer)
        );
        // Same tint per layer as blueprint::spawn_ship_from_design and
        // building::process_hull_placement, so a loaded hull matches a built one.
        let color = match hull_data.hull_layer {
            HullLayer::Outer | HullLayer::Hallway => Color::WHITE,
            HullLayer::Inner => Color::srgb(0.9, 0.9, 0.9),
            HullLayer::Void => Color::srgb(0.5, 0.5, 0.6),
            HullLayer::BulkheadDoor => Color::srgb(0.9, 0.8, 0.7),
        };
        commands.spawn((
            (Sprite {
                    image: hull_texture,
                    color,
                    // Full cell, matching blueprint.rs -- a loaded save used to
                    // come back with the old 2-unit gap between every plate.
                    custom_size: Some(Vec2::new(66.0, 66.0)),
                    ..default()
                }, Transform::from_xyz(
                    hull_data.grid_position.x as f32 * 66.0,
                    hull_data.grid_position.y as f32 * 66.0 - 33.0,
                    0.1,
                )),
            // Every other hull spawn carries these two. Without Block a plate
            // resolved as armour thickness 0 for the rest of the session, and
            // without the base colour damage tinting had nothing to darken from.
            BaseSpriteColor(color),
            crate::building::Block::hull(hull_data.grid_position, hull_data.material),
            BaseHullStats {
                max_health: hull_data.max_health,
                radiation_shielding: hull_data.radiation_shielding,
            },
            HullSegment {
                health: hull_data.health,
                max_health: hull_data.max_health,
                radiation_shielding: hull_data.radiation_shielding,
                is_depressurized: false,
                depressurization_level: 0.0,
                hull_layer: hull_data.hull_layer,
                material: hull_data.material,
                grid_position: hull_data.grid_position,
            },
        )).insert(ChildOf(ship_entity));
    }

    // ---- Respawn modules ----
    for module_data in modules {
        let entity = crate::ship::spawn_module_signed(
            commands,
            asset_server,
            ship_entity,
            module_data.module_type,
            module_data.grid_position,
            module_data.rotation,
            registry,
            module_data.signature.as_deref().and_then(crate::diplomacy::signatures::get),
        );
        // Restore saved health and active state (spawn_module uses registry defaults)
        commands.entity(entity).insert(ModuleHealthOverride {
            health: module_data.health,
            is_active: module_data.is_active,
        });
        // Restore stat tuning + loaded ammo — inserting counts as Changed, so
        // apply_weapon_tuning recomputes the live stats on the next frame.
        if let Some(tuning) = module_data.tuning {
            commands.entity(entity).insert(tuning);
        }
        if let Some(ammo) = module_data.selected_ammo {
            commands.entity(entity).insert(
                crate::building::customization::tuning::SelectedAmmo(ammo)
            );
        }
        if let Some(shell) = &module_data.shell {
            commands.entity(entity).insert(crate::combat::shell::GunShell(shell.clone()));
        }
        if let Some(missile) = &module_data.missile {
            commands.entity(entity).insert(crate::combat::missile_design::MissileLoadout(missile.clone()));
        }
        if module_data.point_defence {
            commands.entity(entity).insert(crate::combat::targeting::fire_groups::InterceptMode);
        }
        if let Some(ring_cell) = module_data.turret {
            commands.entity(entity).insert(crate::combat::turret_deck::PendingTurretMount(ring_cell));
        }
    }

    // ---- Respawn crew ----
    // On the deck, spread through the ship. They used to be laid out in a line
    // from the ship's middle, 20 units apart: fine for eight, but with a crew of
    // 240 most of them came back standing in open space.
    let deck: Vec<Vec2> = hull_segments
        .iter()
        .filter(|h| h.hull_layer == HullLayer::Hallway)
        .map(|h| Vec2::new(h.grid_position.x as f32 * 66.0, h.grid_position.y as f32 * 66.0 - 33.0))
        .collect();
    let mut roster_members = Vec::new();
    for (i, crew_data) in crew.iter().enumerate() {
        let crew_entity = commands.spawn((
            (Sprite {
                    color: Color::srgb(0.8, 0.6, 0.5),
                    custom_size: Some(Vec2::new(16.0, 16.0)),
                    ..default()
                }, Transform::from_translation(
                    // Above the modules, like every other crew spawn.
                    deck.get((i * 7919) % deck.len().max(1))
                        .copied()
                        .unwrap_or(Vec2::new((i as f32 - 3.5) * 20.0, 0.0))
                        .extend(crate::crew::walking::CREW_Z),
                )),
            crew_data.duty,
            CrewMember {
                name: crew_data.name.clone(),
                health: crew_data.health,
                max_health: crew_data.max_health,
                oxygen: crew_data.oxygen,
                morale: crew_data.morale,
                state: CrewState::Idle,
            },
        )).insert(ChildOf(ship_entity)).id();
        roster_members.push(crew_entity);
    }
    roster_members
}

// ============================================================================
// MODULE HEALTH OVERRIDE (applied once after load)
// ============================================================================

/// Applies saved health/active state to modules after load, then removes the marker.
fn apply_module_health_overrides(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Module, &ModuleHealthOverride)>,
) {
    for (entity, mut module, override_data) in query.iter_mut() {
        module.health = override_data.health;
        module.is_active = override_data.is_active;
        commands.entity(entity).remove::<ModuleHealthOverride>();
    }
}

// ============================================================================
// AUTO-SAVE
// ============================================================================

/// Auto-save every 2 minutes while exploring or at surface
fn auto_save_system(
    time: Res<Time>,
    mut auto_save: ResMut<AutoSaveTimer>,
    mut save_events: MessageWriter<SaveGameRequest>,
    current_state: Res<State<GameState>>,
) {
    if !auto_save.enabled {
        return;
    }

    // Only auto-save while exploring or at surface base
    match current_state.get() {
        GameState::Exploring | GameState::StationDocked => {}
        _ => return,
    }

    auto_save.timer.tick(time.delta());
    if auto_save.timer.just_finished() {
        info!("Auto-save triggered");
        save_events.write(SaveGameRequest { slot: AUTO_SAVE_SLOT });
    }
}

/// Wipe the previous run when a new expedition starts.
///
/// ROADMAP called this out as open: "New Expedition doesn't start a new
/// expedition. It sets the state and nothing else." Everything below was
/// inherited from the last run, which made a fresh start quietly not fresh.
///
/// It became load-bearing once the story landed. `Statistics.logs_found`
/// survived into the new run, and the ending keys on the finale entry, so a
/// player who finished once would start their next expedition and have the
/// ending fire immediately.
///
/// `Unlocks` is deliberately NOT reset: that is meta-progression across runs,
/// which is the one thing that is supposed to carry.
pub fn reset_for_new_game(
    mut requests: MessageReader<NewExpeditionRequest>,
    mut currency: ResMut<Currency>,
    mut inventory: ResMut<Inventory>,
    mut statistics: ResMut<Statistics>,
    mut contracts: ResMut<crate::contracts::ContractState>,
    mut reputation: ResMut<crate::contracts::FactionReputation>,
    mut standing: ResMut<crate::diplomacy::FactionStanding>,
    mut victory: ResMut<VictoryState>,
    mut death_cause: ResMut<DeathCause>,
    mut dead: ResMut<crate::crew::burial::DriftingDead>,
    mut finale: ResMut<crate::narrative::FinaleFound>,
    mut galaxy: ResMut<crate::celestial::resources::GalaxyMap>,
    mut streaming: ResMut<crate::celestial::resources::SystemStreamingManager>,
    world: (
        ResMut<GalaxyState>,
        ResMut<crate::ai_ship::components::WorldSimulation>,
        ResMut<LootedPois>,
        ResMut<crate::diplomacy::stations::FactionStations>,
        ResMut<crate::celestial::territory::BorderWar>,
        ResMut<crate::diplomacy::contacts::Contacts>,
        ResMut<crate::diplomacy::jobs::Jobs>,
    ),
) {
    let (mut spawned, mut sim, mut looted, mut faction_stations, mut border_war, mut people, mut work) = world;
    if requests.read().next().is_none() {
        return;
    }
    requests.clear();

    *currency = Currency::default();
    *inventory = Inventory::default();
    *statistics = Statistics::default();
    *contracts = crate::contracts::ContractState::default();
    *reputation = crate::contracts::FactionReputation::default();
    *standing = crate::diplomacy::FactionStanding::default();
    *victory = VictoryState::default();
    *death_cause = DeathCause::default();
    // The dead are permanent within a run, on purpose. They are not permanent
    // across runs: those were a different expedition's crew.
    *dead = crate::crew::burial::DriftingDead::default();
    finale.0 = false;

    // Emptying the systems list is the trigger: generate_galaxy_on_enter
    // early-returns when the galaxy is already populated, so clearing it is
    // what makes the next launch roll a fresh one.
    galaxy.systems.clear();
    *streaming = crate::celestial::resources::SystemStreamingManager::default();
    // The spawned side of the galaxy and its population. Their entities are
    // already gone (ship::cleanup_game_entities, on entering the menu); these
    // are the records that would otherwise make the next run skip spawning
    // its own -- spawn_initial_system only fills an empty GalaxyState, and an
    // `initialized` simulation never populates again.
    *spawned = GalaxyState::default();
    *sim = crate::ai_ship::components::WorldSimulation::default();
    *faction_stations = crate::diplomacy::stations::FactionStations::default();
    *border_war = crate::celestial::territory::BorderWar::default();
    *people = crate::diplomacy::contacts::Contacts::default();
    *work = crate::diplomacy::jobs::Jobs::default();
    // Derelicts stripped last run are full again in a new galaxy.
    *looted = LootedPois::default();

    info!("new expedition: previous run cleared");
}

/// Left behind by a load: hidden at once, despawned in `Last`, after every
/// system has applied what it queued this frame.
#[derive(Component)]
pub struct DespawnAtFrameEnd;

pub fn despawn_at_frame_end(mut commands: Commands, doomed: Query<Entity, With<DespawnAtFrameEnd>>) {
    for entity in doomed.iter() {
        commands.entity(entity).try_despawn();
    }
}

#[cfg(test)]
mod save_scope_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::ai_ship::components::{AiShip, AiShipType, AiShipWreck, OwnedByAiShip};
    use crate::building::registry::{build_registry, ModuleRegistry};
    use crate::combat::severance::DetachedSection;
    use crate::crew::eva_salvage::{EvaPhase, EvaSalvaging};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>();
        app.insert_resource(build_registry());
        app
    }

    fn hull_at(cell: IVec2, health: f32) -> (HullSegment, Transform) {
        (
            HullSegment { grid_position: cell, health, ..default() },
            Transform::from_xyz(cell.x as f32 * 66.0, cell.y as f32 * 66.0 - 33.0, 0.1),
        )
    }

    fn hand(name: &str, health: f32) -> CrewMember {
        CrewMember {
            name: name.into(),
            health,
            max_health: 100.0,
            oxygen: 90.0,
            morale: 70.0,
            state: CrewState::Idle,
        }
    }

    struct World3 {
        player: Entity,
        ai: Entity,
        wreck: Entity,
    }

    /// The player ship -- 3 hull, 2 modules, 2 crew aboard, one out on EVA and
    /// one just bought at the hiring menu -- beside an AI ship and a wreck
    /// whose blocks sit on the SAME local cells (grid cells are ship-local,
    /// which is how a save ended up with five blocks on one cell), plus a
    /// module the player just had shot off.
    fn populate(app: &mut App) -> World3 {
        let player = app.world_mut().spawn((Ship, Transform::from_xyz(120.0, -40.0, 0.0))).id();
        let ai = app.world_mut().spawn((AiShip, Transform::from_xyz(5000.0, 0.0, 0.0))).id();
        let wreck = app.world_mut().spawn((
            AiShipWreck { ship_type: AiShipType::StellarPreserve, loot_remaining: 3, intact_frac: 0.5 },
            Transform::from_xyz(-5000.0, 0.0, 0.0),
        )).id();

        app.world_mut().run_system_once(move |mut commands: Commands, assets: Res<AssetServer>,
            registry: Res<ModuleRegistry>| {
            for x in 0..3 {
                commands.spawn(hull_at(IVec2::new(x, 0), 40.0 + x as f32)).insert(ChildOf(player));
            }
            crate::ship::spawn_module(&mut commands, &assets, player, ModuleType::SmallReactor,
                IVec2::new(0, 1), Rotation::North, &registry);
            let battery = crate::ship::spawn_module(&mut commands, &assets, player,
                ModuleType::BatteryBank, IVec2::new(1, 1), Rotation::East, &registry);
            // Damaged and switched off, so a round trip has something to keep.
            commands.entity(battery).insert(ModuleHealthOverride { health: 37.0, is_active: false });
            commands.spawn((hand("Ines", 80.0), CrewDuty::default(), ChildOf(player)));
            commands.spawn((hand("Okafor", 55.0), ChildOf(player)));
            commands.spawn((hand("Duarte", 100.0), EvaSalvaging {
                wreck,
                grab_target: wreck,
                phase: EvaPhase::Outbound,
                carrying: None,
                recalled: false,
                home_local: Vec3::ZERO,
                base_color: Color::WHITE,
            }));
            commands.spawn(hand("Rask", 100.0));

            // Shot off: unparented in place, so its transform is world-space.
            let severed = crate::ship::spawn_module(&mut commands, &assets, player,
                ModuleType::SmallReactor, IVec2::new(2, 1), Rotation::North, &registry);
            commands.entity(severed).remove::<ChildOf>().insert(DetachedSection {
                has_reactor: true,
                has_ammo: false,
                has_fuel: false,
                meltdown_timer: Some(10.0),
                mass: 50.0,
                spin: 1.5,
                age: 0.0,
                ours: true,
            });

            for root in [ai, wreck] {
                for x in 0..4 {
                    commands.spawn(hull_at(IVec2::new(x, 0), 118.75))
                        .insert((OwnedByAiShip { root }, ChildOf(root)));
                }
                let m = crate::ship::spawn_module(&mut commands, &assets, root,
                    ModuleType::SmallReactor, IVec2::new(0, 1), Rotation::North, &registry);
                commands.entity(m).insert(OwnedByAiShip { root });
                commands.spawn((hand("Orlo", 100.0), OwnedByAiShip { root }, ChildOf(root)));
            }
        }).unwrap();
        app.world_mut().run_system_once(apply_module_health_overrides).unwrap();
        World3 { player, ai, wreck }
    }

    fn capture(app: &mut App) -> Option<ShipParts> {
        app.world_mut()
            .run_system_once(|parts: PlayerShipParts| parts.collect())
            .unwrap()
    }

    /// Order-free fingerprint of what a save would write for the ship.
    fn fingerprint(parts: &ShipParts) -> (Vec<String>, Vec<String>, Vec<String>) {
        fn rows<T: serde::Serialize>(items: &[T]) -> Vec<String> {
            let mut out: Vec<String> =
                items.iter().map(|i| serde_json::to_string(i).unwrap()).collect();
            out.sort();
            out
        }
        (rows(&parts.hull_segments), rows(&parts.modules), rows(&parts.crew))
    }

    fn children_of(app: &mut App, root: Entity) -> usize {
        app.world_mut()
            .query::<&ChildOf>()
            .iter(app.world())
            .filter(|p| p.parent() == root)
            .count()
    }

    #[test]
    fn save_keeps_only_the_player_ship() {
        let mut app = app();
        populate(&mut app);
        let parts = capture(&mut app).expect("one player ship");

        assert_eq!(parts.hull_segments.len(), 3, "foreign hull in the save");
        assert_eq!(parts.modules.len(), 2, "foreign or shot-off modules in the save");
        let mut names: Vec<&str> = parts.crew.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Duarte", "Ines", "Okafor", "Rask"],
            "the crew are the ones aboard, out on EVA and just hired -- no AI crew");

        let mut cells: Vec<IVec2> = parts.hull_segments.iter().map(|h| h.grid_position).collect();
        cells.sort_by_key(|c| (c.x, c.y));
        cells.dedup();
        assert_eq!(cells.len(), 3, "one hull block per cell");
        assert_eq!(parts.position, Vec2::new(120.0, -40.0));
    }

    #[test]
    fn no_player_ship_means_no_save() {
        let mut app = app();
        let world = populate(&mut app);
        app.world_mut().entity_mut(world.player).despawn();
        assert!(capture(&mut app).is_none(),
            "without a ship the save must be skipped, not written empty");
    }

    #[test]
    fn save_then_load_gives_back_the_same_ship() {
        let mut app = app();
        let world = populate(&mut app);
        let before = capture(&mut app).unwrap();
        let ai_blocks = children_of(&mut app, world.ai);
        let wreck_blocks = children_of(&mut app, world.wreck);

        // Through JSON, like the file on disk.
        let hull: Vec<HullSaveData> =
            serde_json::from_str(&serde_json::to_string(&before.hull_segments).unwrap()).unwrap();
        let modules: Vec<ModuleData> =
            serde_json::from_str(&serde_json::to_string(&before.modules).unwrap()).unwrap();
        let crew: Vec<CrewSaveData> =
            serde_json::from_str(&serde_json::to_string(&before.crew).unwrap()).unwrap();

        // The load's despawn set: every block and hand that isn't an AI ship's.
        let doomed: Vec<Entity> = app.world_mut()
            .query_filtered::<Entity, (
                Or<(With<Module>, With<HullSegment>, With<CrewMember>)>,
                Without<OwnedByAiShip>,
            )>()
            .iter(app.world())
            .collect();
        for e in doomed {
            app.world_mut().entity_mut(e).despawn();
        }

        let player = world.player;
        let roster = app.world_mut().run_system_once(move |mut commands: Commands,
            assets: Res<AssetServer>, registry: Res<ModuleRegistry>| {
            respawn_ship_parts(&mut commands, &assets, &registry, player, &hull, &modules, &crew)
        }).unwrap();
        app.world_mut().run_system_once(apply_module_health_overrides).unwrap();

        let after = capture(&mut app).unwrap();
        assert_eq!(fingerprint(&after), fingerprint(&before), "the ship changed across save/load");
        assert_eq!(roster.len(), 4);
        assert_eq!(children_of(&mut app, world.ai), ai_blocks, "load touched the AI ship");
        assert_eq!(children_of(&mut app, world.wreck), wreck_blocks, "load touched the wreck");
    }
}

#[cfg(test)]
mod save_state_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::state::app::StatesPlugin;

    /// Pause from `from` the way Escape does, then ask what a save made now
    /// would record.
    fn saved_after_pausing_from(from: GameState) -> GameState {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.init_resource::<PrePauseState>();

        app.world_mut().resource_mut::<NextState<GameState>>().set(from);
        app.update();
        app.world_mut().resource_mut::<PrePauseState>().0 = Some(from);
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Paused);
        app.update();
        assert_eq!(*app.world().resource::<State<GameState>>().get(), GameState::Paused);

        app.world_mut()
            .run_system_once(|state: Res<State<GameState>>, pre_pause: Res<PrePauseState>| {
                saved_flight_state(*state.get(), &pre_pause)
            })
            .unwrap()
    }

    #[test]
    fn paused_mid_flight_saves_as_flying() {
        assert_eq!(saved_after_pausing_from(GameState::Exploring), GameState::Exploring);
    }

    #[test]
    fn paused_at_a_station_saves_as_docked() {
        assert_eq!(saved_after_pausing_from(GameState::StationDocked), GameState::StationDocked);
    }

    #[test]
    fn unpaused_saves_record_the_state_they_are_in() {
        let none = PrePauseState(None);
        assert_eq!(saved_flight_state(GameState::Exploring, &none), GameState::Exploring);
        assert_eq!(saved_flight_state(GameState::StationDocked, &none), GameState::StationDocked);
        // Nothing remembered: same fallback as Escape's resume.
        assert_eq!(saved_flight_state(GameState::Paused, &none), GameState::Exploring);
    }
}
