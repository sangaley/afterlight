use bevy::prelude::*;

use crate::ai_ship::components::{SimBehavior, WorldSimulation};
use crate::components::{CargoHold, Module, ModuleType, Ship};
use crate::events::*;
use crate::resources::{Currency, Inventory};
use crate::world::home_base;
use super::{Contract, ContractObjective, ContractState, ContractStatus, FactionReputation};

// ============================================================================
// KILL TRACKING
// ============================================================================

pub fn track_kill_contracts(
    mut state: ResMut<ContractState>,
    mut kills: MessageReader<CreatureKilled>,
) {
    for event in kills.read() {
        for contract in state.active_contracts.iter_mut() {
            if contract.status != ContractStatus::Active { continue; }
            if let ContractObjective::Kill { creature_type, current_count, .. } = &mut contract.objective {
                if *creature_type == event.creature_type {
                    *current_count += 1;
                }
            }
        }
    }
}

// ============================================================================
// DESTROY-SHIP TRACKING
// ============================================================================

pub fn track_destroy_ship_contracts(
    mut state: ResMut<ContractState>,
    mut destroyed: MessageReader<AiShipDestroyed>,
) {
    for event in destroyed.read() {
        let Some(bounty_id) = event.bounty_id else { continue };
        for contract in state.active_contracts.iter_mut() {
            if contract.status != ContractStatus::Active { continue; }
            if let ContractObjective::DestroyShip { target_id, destroyed, .. } = &mut contract.objective {
                if *target_id == bounty_id {
                    *destroyed = true;
                }
            }
        }
    }
}

// ============================================================================
// POI TRACKING
// ============================================================================

pub fn track_poi_contracts(
    mut state: ResMut<ContractState>,
    mut discoveries: MessageReader<PoiDiscovered>,
) {
    for event in discoveries.read() {
        for contract in state.active_contracts.iter_mut() {
            if contract.status != ContractStatus::Active { continue; }
            if let ContractObjective::ExplorePoi { poi_type, discovered } = &mut contract.objective {
                if *poi_type == event.poi_type {
                    *discovered = true;
                }
            }
        }
    }
}

// ============================================================================
// DEPTH TRACKING
// ============================================================================

pub fn track_depth_contracts(
    mut state: ResMut<ContractState>,
    ship: Query<&Transform, With<crate::components::Ship>>,
    stations: Res<crate::world::home_base::SystemStations>,
) {
    let Ok(tf) = ship.single() else { return };
    let out = crate::world::distance_from_safety(tf.translation.truncate(), &stations);

    for contract in state.active_contracts.iter_mut() {
        if contract.status != ContractStatus::Active { continue; }
        if let ContractObjective::ReachDepth { target_depth, reached } = &mut contract.objective {
            // Distance from the nearest berth. Against current_depth this
            // completed the instant the player warped anywhere, because that
            // measures distance from the world origin and every system but
            // Haven centres hundreds of thousands of units away.
            if out >= *target_depth {
                *reached = true;
            }
        }
    }
}

// ============================================================================
// SALVAGE TRACKING
// ============================================================================

pub fn track_salvage_contracts(
    mut state: ResMut<ContractState>,
    inventory: Res<Inventory>,
) {
    for contract in state.active_contracts.iter_mut() {
        if contract.status != ContractStatus::Active { continue; }
        if let ContractObjective::RetrieveSalvage { item_type, current_count, .. } = &mut contract.objective {
            let held = inventory.items.get(item_type).copied().unwrap_or(0);
            *current_count = held;
        }
    }
}

// ============================================================================
// SURVEY TRACKING
// ============================================================================

/// The world's own answer, not a second one.
///
/// This was a private copy using 200/500/1000/2000 -- the submarine depths
/// `world::depth_to_zone` abandoned when cruise speeds went up, and which a
/// ship at full burn crosses in a couple of seconds. It meant a five-star
/// survey ("spend time in Black Hole Proximity") was satisfied 2,000 units off
/// the berth, while the HUD called that Near Space.
use crate::world::depth_to_zone as current_zone;

pub fn track_survey_contracts(
    mut state: ResMut<ContractState>,
    ship: Query<&Transform, With<crate::components::Ship>>,
    stations: Res<crate::world::home_base::SystemStations>,
    time: Res<Time>,
) {
    let Ok(tf) = ship.single() else { return };
    let out = crate::world::distance_from_safety(tf.translation.truncate(), &stations);
    let zone = current_zone(out);

    for contract in state.active_contracts.iter_mut() {
        if contract.status != ContractStatus::Active { continue; }
        if let ContractObjective::SurveyZone { zone: target_zone, elapsed_seconds, .. } = &mut contract.objective {
            if zone == *target_zone {
                *elapsed_seconds += time.delta_secs();
            }
        }
    }
}

// ============================================================================
// CAPTURE TRACKING
// ============================================================================

pub fn track_capture_contracts(
    mut state: ResMut<ContractState>,
    containment_query: Query<(&CargoHold, &Module)>,
) {
    // Check if any CreatureContainment module has cargo > 0
    // This is a simplified check — in a full implementation you'd track
    // which creature type is in each containment unit.
    let has_captured_creature = containment_query.iter().any(|(cargo, module)| {
        module.module_type == ModuleType::CreatureContainment && cargo.current_weight > 0.0
    });

    if has_captured_creature {
        for contract in state.active_contracts.iter_mut() {
            if contract.status != ContractStatus::Active { continue; }
            if let ContractObjective::CaptureLive { captured, .. } = &mut contract.objective {
                *captured = true;
            }
        }
    }
}

// ============================================================================
// COMPLETION CHECK
// ============================================================================

pub fn check_contract_completion(
    mut state: ResMut<ContractState>,
    mut completed_events: MessageWriter<ContractCompleted>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for contract in state.active_contracts.iter_mut() {
        if contract.status != ContractStatus::Active { continue; }
        if contract.is_objective_complete() {
            contract.status = ContractStatus::Completed;
            completed_events.write(ContractCompleted { contract_id: contract.id });
            notifications.write(ShowNotification {
                message: format!("Contract complete: {}! Claim it at any station (F).", contract.title),
                notification_type: NotificationType::Success,
                duration: 5.0,
            });
        }
    }
}

// ============================================================================
// TURN IN — claimable at any station, not just the one that offered it
// ============================================================================

/// Pays out every completed active contract. Shared by the automatic
/// Haven-docking turn-in and the proximity-based turn-in at any station.
fn turn_in_active(
    state: &mut ContractState,
    rep: &mut FactionReputation,
    currency: &mut Currency,
    inventory: &mut Inventory,
    turned_in_events: &mut MessageWriter<ContractTurnedIn>,
    notifications: &mut MessageWriter<ShowNotification>,
) {
    let mut completed_count = 0u32;

    for contract in state.active_contracts.iter_mut() {
        if contract.status != ContractStatus::Completed { continue; }

        // A retrieval is a delivery: the goods leave the hold. "Completed"
        // only meant the hold held enough at some point in flight -- they may
        // have been sold or spent since -- so check again now, and send the
        // contract back to Active if the cargo is no longer there.
        if !take_delivery(contract, inventory) {
            if let ContractObjective::RetrieveSalvage { item_type, target_count, .. } = &contract.objective {
                notifications.write(ShowNotification {
                    message: format!(
                        "{}: deliver {} {} to claim it.",
                        contract.title, target_count, item_type.name()
                    ),
                    notification_type: NotificationType::Warning,
                    duration: 4.0,
                });
            }
            contract.status = ContractStatus::Active;
            continue;
        }

        // Reward plus the deposit back. The deposit is taken when the job is
        // accepted (contracts/ui.rs), so finishing has to return it or every
        // completed contract quietly costs the player its stake.
        currency.credits += contract.reward + contract.deposit;
        let rep_gain = contract.star_rating as f32 * 2.0;
        rep.add(&contract.faction, rep_gain);
        contract.status = ContractStatus::TurnedIn;

        turned_in_events.write(ContractTurnedIn {
            contract_id: contract.id,
            reward: contract.reward,
            faction: contract.faction,
        });
        notifications.write(ShowNotification {
            message: format!(
                "{} contract turned in! +{}c, +{:.0} {} rep",
                contract.title, contract.reward, rep_gain, contract.faction.name()
            ),
            notification_type: NotificationType::Success,
            duration: 5.0,
        });

        completed_count += 1;
    }

    state.contracts_completed_total += completed_count;
    state.active_contracts.retain(|c| c.status != ContractStatus::TurnedIn);
}

/// Removes what a retrieval contract asks for from the hold. True when the
/// contract can be paid: the items were there and are now gone, or it is not
/// a retrieval at all.
fn take_delivery(contract: &mut Contract, inventory: &mut Inventory) -> bool {
    let ContractObjective::RetrieveSalvage { item_type, target_count, current_count } = &mut contract.objective
    else { return true };
    if inventory.remove_item(*item_type, *target_count) { return true; }
    *current_count = inventory.items.get(item_type).copied().unwrap_or(0);
    false
}

/// Docking at Haven always settles up any completed bounties.
pub fn turn_in_contracts(
    mut state: ResMut<ContractState>,
    mut rep: ResMut<FactionReputation>,
    mut currency: ResMut<Currency>,
    mut inventory: ResMut<Inventory>,
    mut turned_in_events: MessageWriter<ContractTurnedIn>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    turn_in_active(&mut state, &mut rep, &mut currency, &mut inventory, &mut turned_in_events, &mut notifications);
}

/// Flying near any station (Haven or an outpost) and pressing F claims
/// completed contracts too — you don't need to fly all the way back to
/// wherever a bounty happened to be posted to collect it.
pub fn turn_in_at_station_proximity(
    mut press: ResMut<crate::resources::InteractPress>,
    stations: Res<home_base::SystemStations>,
    ship_query: Query<&Transform, With<Ship>>,
    mut state: ResMut<ContractState>,
    mut rep: ResMut<FactionReputation>,
    mut currency: ResMut<Currency>,
    mut inventory: ResMut<Inventory>,
    mut turned_in_events: MessageWriter<ContractTurnedIn>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !press.pending() { return; }
    if !state.active_contracts.iter().any(|c| c.status == ContractStatus::Completed) { return; }

    let Ok(transform) = ship_query.single() else { return };
    let pos = transform.translation.truncate();
    if stations.nearest_index(pos).is_none() { return; }

    // Last in the chain: docking already turns contracts in on arrival
    // (OnEnter(StationDocked)), so this is only the turn-in-without-docking
    // convenience and must never outrank the press's other meanings.
    if !press.claim() { return; }

    turn_in_active(&mut state, &mut rep, &mut currency, &mut inventory, &mut turned_in_events, &mut notifications);
}

// ============================================================================
// FAILURE (on entering GameOver)
// ============================================================================

pub fn handle_contract_failure(
    mut state: ResMut<ContractState>,
    mut currency: ResMut<Currency>,
    mut sim: ResMut<WorldSimulation>,
    mut failed_events: MessageWriter<ContractFailed>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let mut failed_count = 0u32;

    for contract in state.active_contracts.iter_mut() {
        if contract.status == ContractStatus::TurnedIn { continue; }

        if let ContractObjective::DestroyShip { target_id, .. } = &contract.objective {
            sim.untag_bounty(*target_id);
        }

        contract.status = ContractStatus::Failed;
        failed_count += 1;

        if contract.deposit > 0 {
            // Nothing is deducted here. The deposit was already taken on
            // accept; failing simply means it is not returned. Deducting again
            // would charge the player twice for the same stake.
            notifications.write(ShowNotification {
                message: format!("Contract failed: {}. Lost {}c deposit.", contract.title, contract.deposit),
                notification_type: NotificationType::Danger,
                duration: 5.0,
            });
        } else {
            notifications.write(ShowNotification {
                message: format!("Contract failed: {}.", contract.title),
                notification_type: NotificationType::Warning,
                duration: 3.0,
            });
        }

        failed_events.write(ContractFailed { contract_id: contract.id });
    }

    state.contracts_failed_total += failed_count;
    state.active_contracts.clear();
}

// ============================================================================
// BOUNTIES RESTORED FROM A SAVE
// ============================================================================

/// Points every bounty restored from a save at a ship that exists.
///
/// A save keeps the contracts but not the simulation their targets live in.
/// A load from the menu starts a fresh `WorldSimulation`, so a bounty's
/// `target_id` named a ship nobody carried: it could never be killed and
/// never complete, and an accepted one had already taken its deposit.
///
/// Re-tagged rather than refunded. As far as the player can tell a bounty is
/// on "a vessel of that faction" -- they never saw which hull -- and the
/// faction is still out there: the loaded system and its neighbours are
/// populated from the same galaxy. So each open bounty goes on a ship of its
/// faction in the active systems, picked the way a board picks one
/// (`tag_bounty_target`), under a fresh id and at the reward it was accepted
/// for. A tag that still names a living ship of the right faction stays put:
/// that is an in-game load, where the simulation survived.
///
/// Only when no ship of that faction is around is a contract dropped.
/// Accepted ones are returned, for the caller to refund and announce; offers
/// on the boards just go, since nothing was paid for them.
pub fn reattach_restored_bounties(
    state: &mut ContractState,
    sim: &mut WorldSimulation,
    active_systems: &[u32],
    rng: &mut impl rand::Rng,
) -> Vec<Contract> {
    let restored_ids = state.active_contracts.iter()
        .chain(state.available_by_station.iter().flatten())
        .filter_map(|c| match c.objective {
            ContractObjective::DestroyShip { target_id, .. } => Some(target_id),
            _ => None,
        });
    // Every id the save handed out is spent, finished bounties' included.
    if let Some(highest) = restored_ids.max() {
        sim.reserve_bounty_ids_through(highest);
    }

    let open = state.active_contracts.iter()
        .chain(state.available_by_station.iter().flatten())
        .filter_map(|c| match c.objective {
            ContractObjective::DestroyShip { ship_type, target_id, destroyed: false } => Some((ship_type, target_id)),
            _ => None,
        });
    let kept: std::collections::HashSet<u32> = open
        .filter(|&(faction, id)| sim.ships.iter().any(|s| {
            s.bounty_id == Some(id) && s.faction == faction && s.behavior != SimBehavior::Dead
        }))
        .map(|(_, id)| id)
        .collect();

    // Free every other tag, or a lone boss still carrying one from before the
    // load could never carry the bounty restored onto it. Not on a ship in
    // view: its live entity carries the tag too (BountyTarget), and
    // sync_simulation_entities finds its record by it when it leaves range.
    for ship in sim.ships.iter_mut().filter(|s| !s.spawned) {
        if ship.bounty_id.is_some_and(|id| !kept.contains(&id)) {
            ship.bounty_id = None;
        }
    }

    let mut retag = |contract: &mut Contract| -> bool {
        let ContractObjective::DestroyShip { ship_type, target_id, destroyed } = &mut contract.objective
        else { return true };
        if *destroyed || kept.contains(target_id) {
            return true;
        }
        match sim.tag_bounty_target(*ship_type, active_systems, rng) {
            Some((id, _distance)) => {
                *target_id = id;
                true
            }
            None => false,
        }
    };

    // Accepted bounties first: they are paid for, and a lone boss can only
    // carry one tag.
    let mut lost = Vec::new();
    let mut i = 0;
    while i < state.active_contracts.len() {
        if retag(&mut state.active_contracts[i]) {
            i += 1;
        } else {
            lost.push(state.active_contracts.remove(i));
        }
    }
    for board in state.available_by_station.iter_mut() {
        board.retain_mut(|c| retag(c));
    }
    lost
}

/// Runs `reattach_restored_bounties` once after each load, as soon as there is
/// a simulation to tag in. An in-game load still has one; a load from the menu
/// waits for init_world_simulation, which runs on entering Exploring -- so a
/// save made at a berth loads docked and gets its targets at launch.
pub fn reattach_bounties_after_load(
    mut loaded: MessageReader<GameLoaded>,
    mut pending: Local<bool>,
    mut state: ResMut<ContractState>,
    mut sim: ResMut<WorldSimulation>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut currency: ResMut<Currency>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if loaded.read().filter(|e| e.success).count() > 0 {
        *pending = true;
    }
    if !*pending || !sim.initialized {
        return;
    }
    *pending = false;

    let active_systems: Vec<u32> = streaming.loaded_system.into_iter()
        .chain(streaming.warm_systems.iter().copied())
        .collect();
    let lost = reattach_restored_bounties(&mut state, &mut sim, &active_systems, &mut rand::thread_rng());
    for contract in lost {
        // Taken on accept (contracts/ui.rs); nothing was hunted, so it goes back.
        currency.credits += contract.deposit;
        let refund = if contract.deposit > 0 {
            format!(" {}c deposit returned.", contract.deposit)
        } else {
            String::new()
        };
        notifications.write(ShowNotification {
            message: format!("{}: the target is gone. Contract void.{}", contract.title, refund),
            notification_type: NotificationType::Warning,
            duration: 5.0,
        });
    }
}

#[cfg(test)]
mod retrieval_delivery_tests {
    use super::*;
    use crate::components::ZoneType;
    use crate::resources::ItemType;
    use super::super::{ContractType, Faction};

    fn retrieval(target_count: u32) -> Contract {
        Contract {
            id: 1,
            faction: Faction::SalvageGuild,
            contract_type: ContractType::RetrieveSalvage,
            title: "Crystals".into(),
            description: String::new(),
            objective: ContractObjective::RetrieveSalvage { item_type: ItemType::Crystal, target_count, current_count: target_count },
            reward: 100,
            deposit: 0,
            star_rating: 1,
            depth_zone: ZoneType::NearOrbit,
            status: ContractStatus::Completed,
        }
    }

    #[test]
    fn delivering_takes_the_goods_out_of_the_hold() {
        let mut inv = Inventory::default();
        inv.add_item(ItemType::Crystal, 5);
        assert!(take_delivery(&mut retrieval(3), &mut inv));
        assert_eq!(inv.items.get(&ItemType::Crystal).copied(), Some(2));
    }

    /// Sold since the tracker ticked it off: no pay, and the count shows why.
    #[test]
    fn goods_no_longer_aboard_are_not_paid_for() {
        let mut inv = Inventory::default();
        inv.add_item(ItemType::Crystal, 1);
        let mut contract = retrieval(3);
        assert!(!take_delivery(&mut contract, &mut inv));
        assert_eq!(inv.items.get(&ItemType::Crystal).copied(), Some(1));
        assert!(matches!(contract.objective, ContractObjective::RetrieveSalvage { current_count: 1, .. }));
    }
}

#[cfg(test)]
mod bounty_restore_tests {
    use super::*;
    use crate::ai_ship::components::{AiShipType, SimulatedShip};
    use crate::celestial::resources::SystemStreamingManager;
    use crate::components::ZoneType;
    use crate::resources::SaveData;
    use super::super::{ContractType, Faction};
    use rand::SeedableRng;

    /// The system the save was made in.
    const HERE: u32 = 5;

    fn bounty(target_id: u32, ship_type: AiShipType, status: ContractStatus) -> Contract {
        Contract {
            id: target_id + 100,
            faction: Faction::Navy,
            contract_type: ContractType::DestroyShip,
            title: format!("Bounty: {ship_type:?} vessel"),
            description: String::new(),
            objective: ContractObjective::DestroyShip { ship_type, target_id, destroyed: false },
            reward: 900,
            deposit: 225,
            star_rating: 3,
            depth_zone: ZoneType::NearOrbit,
            status,
        }
    }

    /// Contracts as a load brings them back: through the save file's JSON.
    fn restored(active: Vec<Contract>, board: Vec<Contract>) -> ContractState {
        let mut save = SaveData::default();
        save.contracts.active_contracts = active;
        *save.contracts.board_mut(0) = board;
        let json = serde_json::to_string(&save).unwrap();
        serde_json::from_str::<SaveData>(&json).unwrap().contracts
    }

    /// The simulation a load from the menu starts: freshly populated, no tags.
    fn fresh_sim(ships: &[(u32, AiShipType)]) -> WorldSimulation {
        let mut sim = WorldSimulation::default();
        sim.initialized = true;
        for &(system, faction) in ships {
            let at = Vec2::new(40_000.0, 0.0);
            sim.ships.push(SimulatedShip::patrolling(system, faction, at, at, 15_000.0, 90.0));
        }
        sim
    }

    fn target_of(contract: &Contract) -> u32 {
        match contract.objective {
            ContractObjective::DestroyShip { target_id, .. } => target_id,
            _ => panic!("{} is not a bounty", contract.title),
        }
    }

    fn rng() -> rand::rngs::StdRng {
        rand::rngs::StdRng::seed_from_u64(7)
    }

    /// The report: a bounty accepted before saving could never be completed
    /// after loading it from the menu, because its target went with the
    /// simulation. Now a ship of its faction carries it, and killing that
    /// ship pays.
    #[test]
    fn a_restored_bounty_can_still_be_completed() {
        let mut state = restored(vec![bounty(3, AiShipType::GildedThrone, ContractStatus::Active)], vec![]);
        let mut sim = fresh_sim(&[(HERE, AiShipType::TheSilence), (HERE, AiShipType::GildedThrone)]);
        let lost = reattach_restored_bounties(&mut state, &mut sim, &[HERE], &mut rng());
        assert!(lost.is_empty());

        let id = target_of(&state.active_contracts[0]);
        let carrier = sim.ships.iter()
            .find(|s| s.bounty_id == Some(id))
            .expect("no ship carries the restored bounty");
        assert_eq!(carrier.faction, AiShipType::GildedThrone);

        // The tag rides onto the hull when it spawns (sync_simulation_entities)
        // and back out when it dies; this is the dying half.
        let mut app = App::new();
        app.add_message::<AiShipDestroyed>();
        app.add_message::<ContractCompleted>();
        app.add_message::<ShowNotification>();
        app.insert_resource(state);
        app.add_systems(Update, (track_destroy_ship_contracts, check_contract_completion).chain());
        app.world_mut().write_message(AiShipDestroyed {
            entity: Entity::PLACEHOLDER,
            ship_type: carrier.faction,
            position: carrier.position,
            bounty_id: carrier.bounty_id,
            cause: ShipDeathCause::Struck,
        });
        app.update();
        assert_eq!(
            app.world().resource::<ContractState>().active_contracts[0].status,
            ContractStatus::Completed
        );
    }

    /// A fresh simulation counts bounty ids from 1 again. The first bounty
    /// posted after a load must not share an id with a restored one, or one
    /// kill would pay out both.
    #[test]
    fn new_bounties_never_reuse_a_restored_id() {
        let mut state = restored(
            vec![bounty(1, AiShipType::GildedThrone, ContractStatus::Active)],
            vec![bounty(2, AiShipType::GildedThrone, ContractStatus::Available)],
        );
        let mut sim = fresh_sim(&[(HERE, AiShipType::GildedThrone); 4]);
        reattach_restored_bounties(&mut state, &mut sim, &[HERE], &mut rng());

        let held: Vec<u32> = state.active_contracts.iter()
            .chain(state.available_by_station.iter().flatten())
            .map(target_of)
            .collect();
        assert_eq!(held.len(), 2, "a bounty was dropped with ships to spare");
        let (posted, _) = sim.tag_bounty_target(AiShipType::GildedThrone, &[HERE], &mut rng()).unwrap();
        assert!(
            posted > 2 && !held.contains(&posted),
            "the next bounty got id {posted}, already used by the save (1, 2) or a restored contract {held:?}"
        );
    }

    /// Saved after leaving the only system the target's faction holds: there
    /// is nothing of it to hunt from here. The contract is voided and the
    /// deposit it took comes back; the board offer just goes.
    #[test]
    fn a_bounty_with_nothing_to_hunt_is_voided_and_refunded() {
        let mut app = App::new();
        app.add_message::<GameLoaded>();
        app.add_message::<ShowNotification>();
        app.insert_resource(restored(
            vec![bounty(3, AiShipType::Shepherd, ContractStatus::Active)],
            vec![bounty(4, AiShipType::Shepherd, ContractStatus::Available)],
        ));
        app.insert_resource(fresh_sim(&[(HERE, AiShipType::TheSilence), (HERE + 1, AiShipType::Shepherd)]));
        app.insert_resource(SystemStreamingManager { loaded_system: Some(HERE), ..default() });
        app.init_resource::<Currency>();
        app.add_systems(Update, reattach_bounties_after_load);
        let before = app.world().resource::<Currency>().credits;

        app.world_mut().write_message(GameLoaded { slot: 0, success: true });
        app.update();

        let state = app.world().resource::<ContractState>();
        assert!(state.active_contracts.is_empty(), "a bounty with no ship to hunt is still on the books");
        assert!(state.available_by_station.iter().all(|b| b.is_empty()));
        assert_eq!(app.world().resource::<Currency>().credits, before + 225);
    }

    /// An in-game load keeps the running simulation. A tag that still names a
    /// living ship of the bounty's faction is the same hunt and stays; one no
    /// restored contract names is freed, or a lone boss would stay locked to
    /// a bounty nobody holds.
    #[test]
    fn an_in_game_load_keeps_live_tags_and_frees_the_rest() {
        let mut state = restored(vec![bounty(3, AiShipType::GildedThrone, ContractStatus::Active)], vec![]);
        let mut sim = fresh_sim(&[(HERE, AiShipType::GildedThrone), (HERE, AiShipType::Shepherd)]);
        sim.ships[0].bounty_id = Some(3);
        // Tagged by a board posted after the save, which the load threw away.
        sim.ships[1].bounty_id = Some(9);
        sim.reserve_bounty_ids_through(9);

        reattach_restored_bounties(&mut state, &mut sim, &[HERE], &mut rng());
        assert_eq!(target_of(&state.active_contracts[0]), 3);
        assert_eq!(sim.ships[0].bounty_id, Some(3));
        assert_eq!(sim.ships[1].bounty_id, None);
    }
}
