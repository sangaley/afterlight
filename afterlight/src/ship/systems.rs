use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::states::GameState;
use crate::events::*;

/// Updates depth and noise state.
/// engine_query is scoped to the player's own ship (via ChildOf) — Engine is
/// shared with AI ships, so unscoped this summed every AI ship's engine
/// noise in the world into the player's own NoiseState (the same
/// cross-ship-contamination pattern fixed everywhere else this session).
pub fn update_ship_state(
    ship_query: Query<(Entity, &Depth), With<Ship>>,
    engine_query: Query<(&Engine, &Module, &ChildOf)>,
    drill_query: Query<&super::drill::DrillRig>,
    mut depth_state: ResMut<DepthState>,
    mut noise_state: ResMut<NoiseState>,
) {
    let Ok((player_ship, depth)) = ship_query.single() else { return };

    // Update depth
    depth_state.current_depth = depth.0;

    // Calculate noise level from the player's own active engines
    let noise: f32 = engine_query
        .iter()
        .filter(|(_, module, parent)| module.is_active && parent.parent() == player_ship)
        .map(|(engine, _, _)| engine.noise_level)
        .sum();

    // A grinding Breaker Drill is louder than cruising engines — the
    // racket is the drill's price tag (creatures and AI hear it).
    let drill_noise: f32 = drill_query
        .iter()
        .filter(|rig| rig.target.is_some())
        .count() as f32
        * super::drill::DRILL_NOISE;

    noise_state.noise_level = noise + drill_noise;
}

/// Checks game over conditions
pub fn check_game_over(
    hull_state: Res<HullState>,
    // Scoped, like core_query below it. Unscoped, a single living AI crewman
    // anywhere in the system kept "all crew dead" permanently false — the
    // player's whole crew could die and the run continued as a ghost ship.
    crew_query: Query<&CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
    // Includes destroyed cores on purpose — see below.
    core_query: Query<
        Option<&DestroyedModule>,
        (With<MemoryCoreComp>, Without<crate::ai_ship::components::OwnedByAiShip>),
    >,
    mut death_cause: ResMut<DeathCause>,
    mut next_state: ResMut<NextState<GameState>>,
    mut notifications: MessageWriter<ShowNotification>,
    session_timer: Res<ExploringSessionTimer>,
    time: Res<Time>,
) {
    // Grace period: allow 3 seconds for all initialization systems to complete
    // (crew spawning, hull setup, etc. may take a frame to flush commands)
    if session_timer.elapsed < 3.0 {
        return;
    }

    // Cores are the player's own continuity, not just equipment. Losing every
    // one is a different death from losing the hull: the ship can still be
    // structurally fine and full of living people.
    //
    // Two things this must NOT do, both of which it did on the first attempt.
    //
    // It must not count `is_active`. That tracks whether a module is powered,
    // and a browned-out core is still a core — the first version ended the run
    // three seconds after launch, at full hull with nobody hurt, because the
    // cores had not been energised yet.
    //
    // And the query must include destroyed cores, or the condition erases
    // itself: filtering them out means that once the last one dies the query
    // is empty, "did this ship ever have cores" reads false, and the death
    // never fires at all.
    let cores_total = core_query.iter().count();
    let cores_alive = core_query.iter().filter(|destroyed| destroyed.is_none()).count();
    let had_cores = cores_total > 0;

    let crew_count = crew_query.iter().count();
    let all_crew_dead = crew_count == 0 || crew_query.iter().all(|c| c.health <= 0.0);
    let hull_destroyed = hull_state.hull_integrity <= 0.0;

    // Attribute the death to the most recent damage source if it was fresh
    // (within 20s) — "hull destroyed" alone doesn't tell the player anything.
    let recent_damage = death_cause.last_damage.as_ref()
        .filter(|(_, at)| time.elapsed_secs_f64() - at < 20.0)
        .map(|(desc, _)| desc.clone());
    let attribution = recent_damage
        .map(|d| format!(" Cause: {}.", d))
        .unwrap_or_default();

    if had_cores && cores_alive == 0 {
        death_cause.cause = Some(format!("Every memory core destroyed.{}", attribution));
        notifications.write(ShowNotification {
            message: "Last core gone. Whatever was running this ship is not running it now.".into(),
            notification_type: NotificationType::Danger,
            duration: 5.0,
        });
        next_state.set(GameState::GameOver);
    } else if all_crew_dead {
        death_cause.cause = Some(format!("All crew died.{}", attribution));
        notifications.write(ShowNotification {
            message: "All crew lost. The ship drifts silently into the void...".into(),
            notification_type: NotificationType::Danger,
            duration: 5.0,
        });
        next_state.set(GameState::GameOver);
    } else if hull_destroyed {
        death_cause.cause = Some(format!("Hull integrity reached zero.{}", attribution));
        notifications.write(ShowNotification {
            message: "Hull integrity zero! The ship is breaking apart!".into(),
            notification_type: NotificationType::Danger,
            duration: 5.0,
        });
        next_state.set(GameState::GameOver);
    }
}

/// Updates inventory max_capacity based on cargo hold modules
pub fn update_inventory_capacity(
    // Player holds only — unscoped, every wreck/AI cargo module in the
    // world inflated the player's max capacity (and made it jump around
    // as wrecks spawned/got dismantled).
    cargo_query: Query<(&CargoHold, &Module), Without<crate::ai_ship::components::OwnedByAiShip>>,
    mut inventory: ResMut<Inventory>,
) {
    let base_capacity = 100.0f32;
    // Every intact hold, switched "on" or not. Holds draw no power and don't
    // start switched on (spawner::starts_active), so gating on is_active left
    // the starter's 440-capacity bay out entirely: 0/100 at the start, then
    // 0/860 once a Repair Modules visit happened to switch everything on.
    let cargo_bonus: f32 = cargo_query
        .iter()
        .filter(|(_, module)| module.health > 0.0)
        .map(|(cargo, _)| cargo.capacity)
        .sum();
    inventory.max_capacity = base_capacity + cargo_bonus;
}

/// Fires the ending once the player holds the finale log.
///
/// The distance clause this used to carry is gone. It asked for 2,200 units
/// from the origin, a submarine-era measure that survived the conversion to
/// space and amounted to a few seconds of flight; it only ever looked like a
/// real condition because the log it was paired with could not spawn at all.
///
/// The finale entry only exists at the deepest log tier, which only exists in
/// the most dangerous systems, which sit at the far edge of the galaxy. So
/// "you have read the last thing out there" already means "you went all the
/// way out". One condition, and it is the one that means something.
pub fn check_victory(
    mut finale: ResMut<crate::narrative::FinaleFound>,
    mut victory_state: ResMut<VictoryState>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if victory_state.achieved {
        return;
    }

    // Keyed by name through narrative::logs::FINALE_TITLE rather than a
    // literal, because the last time this check owned its own copy of the
    // string, the entry it named could not spawn and nobody noticed.
    if finale.0 {
        finale.0 = false;
        victory_state.achieved = true;
        // Straight into the sequence. No toast: the ending opens with its own
        // victory screen, and a congratulatory popup in front of it would step
        // on the one beat that has to land cleanly.
        next_state.set(GameState::Truth);
    }
}

/// Resets victory state when returning to surface
pub fn reset_victory_state(
    mut victory_state: ResMut<VictoryState>,
    mut death_cause: ResMut<DeathCause>,
) {
    victory_state.achieved = false;
    death_cause.cause = None;
    death_cause.last_damage = None;
}

/// Tracks statistics during gameplay
pub fn update_statistics(
    time: Res<Time>,
    depth_state: Res<DepthState>,
    mut statistics: ResMut<Statistics>,
) {
    statistics.play_time_seconds += time.delta_secs();

    if depth_state.current_depth > statistics.max_depth_reached {
        statistics.max_depth_reached = depth_state.current_depth;
    }
}

/// Ticks the exploring session timer each frame
pub fn tick_session_timer(time: Res<Time>, mut timer: ResMut<ExploringSessionTimer>) {
    timer.elapsed += time.delta_secs();
}

/// Resets the exploring session timer (called on OnEnter(Exploring))
pub fn reset_session_timer(mut timer: ResMut<ExploringSessionTimer>) {
    timer.elapsed = 0.0;
}

/// Cleans up all game entities when returning to main menu (after game over or restart).
/// This prevents stale entities from accumulating across play sessions.
pub fn cleanup_game_entities(
    mut commands: Commands,
    ships: Query<Entity, With<Ship>>,
    creatures: Query<Entity, With<Creature>>,
    chunks: Query<Entity, With<Chunk>>,
    pois: Query<Entity, With<PointOfInterest>>,
    // The star system and everyone in it. None of this was cleared, so the
    // next run -- new or loaded -- started inside the last one's.
    world: (
        Query<Entity, With<crate::celestial::components::StarSystemMember>>,
        Query<Entity, With<crate::ai_ship::components::AiShip>>,
        ResMut<crate::celestial::resources::GalaxyState>,
        ResMut<crate::ai_ship::components::WorldSimulation>,
    ),
    mut chunk_manager: ResMut<ChunkManager>,
    mut roster: ResMut<CrewRoster>,
    mut depth_state: ResMut<DepthState>,
    mut oxygen_state: ResMut<OxygenState>,
    mut hull_state: ResMut<HullState>,
    mut fuel_state: ResMut<FuelState>,
    mut noise_state: ResMut<NoiseState>,
) {
    let (members, ai_ships, mut galaxy, mut sim) = world;
    // Despawn ship and all children (modules, hull segments, crew)
    for entity in ships.iter() {
        commands.entity(entity).despawn();
    }
    // Despawn creatures
    for entity in creatures.iter() {
        commands.entity(entity).despawn();
    }
    // Stars, planets, rocks and derelicts. GalaxyState still listing them is
    // what kept the next run from spawning its own: spawn_initial_system only
    // spawns into an empty list, so a new expedition flew the old system.
    for entity in members.iter() {
        commands.entity(entity).try_despawn();
    }
    *galaxy = crate::celestial::resources::GalaxyState::default();
    // AI ships, wrecks included -- a wreck keeps its AiShip. Before the POI
    // pass below, which also reaches wrecks (they carry PointOfInterest).
    // The simulation goes with them: left `initialized`, it never populated
    // the next run, and its ships still carried the old run's systems.
    for entity in ai_ships.iter() {
        commands.entity(entity).try_despawn();
    }
    *sim = crate::ai_ship::components::WorldSimulation::default();
    // Despawn chunks (and their children: POIs, decorations)
    for entity in chunks.iter() {
        commands.entity(entity).despawn();
    }
    // Despawn any orphaned POIs.
    //
    // try_despawn, not despawn: chunk despawn above is recursive, so every POI
    // parented to a chunk is already gone by the time this runs and only the
    // genuinely orphaned ones are left. Using the erroring variant here logged
    // a despawn warning per POI on every restart.
    for entity in pois.iter() {
        commands.entity(entity).try_despawn();
    }
    // Reset chunk manager
    chunk_manager.loaded_chunks.clear();
    // Reset crew roster
    roster.members.clear();
    // Reset resources to defaults
    depth_state.current_depth = 0.0;
    oxygen_state.current_oxygen = 1000.0;
    oxygen_state.max_oxygen = 1000.0;
    hull_state.hull_integrity = 1.0;
    hull_state.max_radiation_shielding = 0.0;
    fuel_state.current_fuel = fuel_state.max_fuel;
    noise_state.noise_level = 0.0;

    info!("Game entities cleaned up for restart");
}

#[cfg(test)]
mod cargo_capacity_tests {
    use super::*;

    fn hold(health: f32, active: bool) -> (CargoHold, Module) {
        (
            CargoHold { capacity: 440.0, current_weight: 0.0 },
            Module {
                module_type: ModuleType::BulkCargoHold,
                health,
                max_health: 100.0,
                power_consumption: 0.0,
                power_generation: 0.0,
                is_active: active,
                grid_position: IVec2::ZERO,
                size: IVec2::ONE,
                rotation: Rotation::default(),
            },
        )
    }

    #[test]
    fn an_intact_hold_counts_whether_or_not_it_is_switched_on() {
        let mut app = App::new();
        app.init_resource::<Inventory>();
        app.add_systems(Update, update_inventory_capacity);
        app.world_mut().spawn(hold(100.0, false));
        app.world_mut().spawn(hold(0.0, true)); // shot to pieces
        let raider = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((hold(100.0, true), crate::ai_ship::components::OwnedByAiShip { root: raider }));
        app.update();
        assert_eq!(app.world().resource::<Inventory>().max_capacity, 540.0);
    }
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    use crate::ai_ship::components::{AiShip, WorldSimulation};
    use crate::celestial::components::StarSystemMember;
    use crate::celestial::resources::GalaxyState;

    /// Back at the menu, nothing of the last run's world is left for the next
    /// one to start inside: not its system, not its ships or wrecks, and not
    /// the records that would stop the next run spawning its own.
    #[test]
    fn the_menu_clears_the_last_runs_world() {
        let mut app = App::new();
        app.init_resource::<ChunkManager>()
            .init_resource::<CrewRoster>()
            .init_resource::<DepthState>()
            .init_resource::<OxygenState>()
            .init_resource::<HullState>()
            .init_resource::<FuelState>()
            .init_resource::<NoiseState>()
            .init_resource::<GalaxyState>()
            .init_resource::<WorldSimulation>()
            .add_systems(Update, cleanup_game_entities);
        app.world_mut().resource_mut::<WorldSimulation>().initialized = true;
        app.world_mut().resource_mut::<GalaxyState>().current_system = 7;
        let star = app.world_mut().spawn(StarSystemMember { system_id: 7 }).id();
        let raider = app.world_mut().spawn(AiShip).id();
        let wreck = app.world_mut().spawn((AiShip, PointOfInterest { poi_type: PoiType::Wreck, discovered: true })).id();

        app.update();

        for (what, e) in [("star", star), ("raider", raider), ("wreck", wreck)] {
            assert!(app.world().get_entity(e).is_err(), "the {what} survived");
        }
        assert!(!app.world().resource::<WorldSimulation>().initialized, "the next run would never be populated");
        assert_eq!(app.world().resource::<GalaxyState>().current_system, 0);
    }
}
