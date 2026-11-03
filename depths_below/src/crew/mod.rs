use bevy::prelude::*;
use crate::states::GameState;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::building::rooms::RoomMap;

pub mod animation;
pub mod burial;
pub mod suits;
pub mod eva_salvage;
pub mod hiring;
pub mod names;
pub mod navigation;
pub mod walking;
use eva_salvage::EvaSalvaging;

pub struct CrewPlugin;

impl Plugin for CrewPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CrewRoster>()
            .init_resource::<StaffingState>()
            .init_resource::<AutoAssignTimer>()
            .init_resource::<RepairScrapPool>()
            // Interior navigation. Rebuilt in every state crew act in, and at
            // dock especially — the whole point of build mode is changing the
            // shape of the ship they have to walk through.
            .add_systems(
                Update,
                navigation::rebuild_nav_grids.run_if(
                    in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))
                        .or_else(in_state(GameState::Docked)),
                ),
            )
            // Crew walking. Same states as staffing: they should walk to a
            // newly-placed station while docked, not teleport there the
            // instant the player leaves. Planning is chained after
            // auto_assign_crew because destinations are read off the
            // assignments it just made.
            .init_resource::<walking::CrewPlanTimer>()
            .init_resource::<walking::CrewErrandTimer>()
            .init_resource::<suits::SuitErrandTimer>()
            .init_resource::<walking::CrewMedicalTimer>()
            .init_resource::<walking::CrewOffDutyTimer>()
            .init_resource::<burial::DriftingDead>()
            .add_systems(Startup, animation::setup_crew_atlases)
            .add_systems(
                Update,
                animation::animate_crew_sprites
                    .run_if(in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))
                        .or_else(in_state(GameState::Docked))),
            )
            .add_systems(
                Update,
                (
                    walking::plan_crew_destinations,
                    walking::walk_engine_room_rounds,
                    // Lowest priority of the destination setters: everything
                    // below is a job and overwrites it. Nothing here is.
                    walking::plan_off_duty_errands,
                    // After destinations: posted crew keep their post, and
                    // only the hands nobody assigned get sent to the damage.
                    walking::plan_repair_errands,
                    // Last of the destination setters: a wounded hand's errand
                    // outranks their post and the breach down the corridor.
                    walking::plan_medical_errands,
                    // The dead go last of the destination setters and, like
                    // damage control, only spare hands carry them.
                    burial::plan_burial_detail,
                    burial::advance_burial.after(burial::plan_burial_detail),
                    // Before path planning: the locker is a destination like
                    // any other, and has to be set before routes are laid.
                    suits::plan_suit_errands,
                    // After the locker errand: once they have a suit on, the
                    // hole outranks whatever post they came from.
                    suits::suited_damage_control,
                    walking::plan_crew_paths,
                    walking::walk_crew,
                    suits::issue_suits,
                    suits::suit_air,
                    // Last in the chain: the draught is applied on top of
                    // whatever step walking just took, so a crew member can
                    // walk against a weak one and lose to a strong one.
                    crate::ship::air::crew_suction,
                )
                    .chain()
                    .after(auto_assign_crew)
                    .run_if(in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))
                        .or_else(in_state(GameState::Docked))),
            )
            // Staffing / efficiency systems run at both StationDocked and Exploring
            // so the HUD shows correct crew/station counts at the surface.
            .add_systems(
                Update,
                (
                    compute_module_efficiency,
                    update_staffing_state,
                    auto_assign_crew,
                    reconcile_hired_crew,
                    crew_arrive_with_quarters,
                )
                    .run_if(in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))
                        .or_else(in_state(GameState::Docked))),
            )
            // Gameplay crew systems only run while Exploring
            .add_systems(
                Update,
                (
                    update_crew_needs,
                    update_crew_room_location,
                    crew_emergency_dispatch.after(update_crew_room_location),
                    update_crew_ai.after(crew_emergency_dispatch),
                    crew_fire_suppression.after(update_crew_ai),
                    crew_repair_system.after(update_crew_ai),
                    report_crew_deaths,
                    handle_crew_death.after(report_crew_deaths),
                    burial::drift_dead,
                    burial::sync_drifting_dead,

                    medbay_healing,
                    messhall_morale,
                    recroom_morale_floor,
                    training_room_boost,
                    engineering_station_boost,
                )
                    .run_if(in_state(GameState::Exploring)),
            )
            // EVA salvage: crew ferry loot from wrecks (see eva_salvage.rs)
            .add_systems(
                Update,
                (
                    eva_salvage::order_salvage_detail,
                    eva_salvage::run_salvage_detail,
                    eva_salvage::eva_blast_damage,
                )
                    .chain()
                    .run_if(in_state(GameState::Exploring)),
            )
            // Hiring board (H near/at a station) — see hiring.rs
            .init_resource::<hiring::HiringBoardOpen>()
            .init_resource::<hiring::HiringPool>()
            .init_resource::<hiring::HiringSelection>()
            .add_systems(
                Update,
                (
                    hiring::toggle_hiring_board,
                    hiring::hiring_board_input,
                    hiring::update_hiring_display,
                )
                    .chain()
                    .run_if(in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))),
            );
        // The detail comes home when the flight ends, not when it pauses.
        eva_salvage::schedule_recall(app);
    }
}

/// Timer for periodic auto-assignment
#[derive(Resource)]
pub struct AutoAssignTimer {
    pub timer: Timer,
}

impl Default for AutoAssignTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        }
    }
}

/// INTERIM CREW SUPPLY — until station hiring exists, crew come WITH
/// the bunks: placing a quarters module (Barracks etc.) during a refit
/// spawns its berths' worth of new hands, as if they signed on with the
/// accommodation. Starter crew are unaffected (the initial ship spawn
/// doesn't emit ModulePlaced). Berths come from the registry def, not
/// the entity — the companion component may not be flushed yet in the
/// frame the placement event fires.
///
/// Station refits only. In flight the crew's scrap rebuild re-places a
/// shot-off quarters through this same event, and filling its bunks then
/// brought the dead back — crew loss has to stay permanent between docks.
fn crew_arrive_with_quarters(
    state: Res<State<GameState>>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    crew_atlases: Res<animation::CrewAtlases>,
    mut placed_events: MessageReader<ModulePlaced>,
    registry: Res<crate::building::ModuleRegistry>,
    ship_query: Query<Entity, With<Ship>>,
    quarters_query: Query<(&Quarters, &Module, &ChildOf)>,
    // Player crew only. AI crew are CrewMembers too, and counting them as
    // "alive aboard" shrank the number of empty bunks to fill.
    crew_query: Query<&CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
    mut roster: ResMut<CrewRoster>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    // Drained rather than left unread: a reader that skips frames would see
    // a rebuild from the last frames of the flight on arrival at the dock.
    if *state.get() != GameState::StationDocked {
        placed_events.clear();
        return;
    }

    let Ok(ship) = ship_query.single() else { return };
    for event in placed_events.read() {
        let crate::building::registry::CompanionData::Quarters { berths: new_berths } =
            registry.get(event.module_type).companion
        else {
            continue;
        };

        // Fill EVERY empty bunk, not just the new module's — otherwise
        // adding a barracks moves crew and capacity in lockstep and the
        // staffing gap never closes (22/30 became 30/38 instead of 38/38).
        // The just-placed module's Quarters companion isn't flushed yet
        // this frame, so its berths come from the registry def.
        //
        // Ship-scoped: AI ships carry Quarters modules too, and an unscoped
        // count let every enemy's bunks inflate the player's capacity — the
        // same leak OwnedByAiShip was introduced to close elsewhere. The
        // ChildOf needed for berth placement makes the fix free.
        let ours = || {
            quarters_query
                .iter()
                .filter(|(_, _, parent)| parent.parent() == ship)
        };
        let existing_berths: u32 = ours()
            .filter(|(_, module, _)| module.is_active && module.health > 0.0)
            .map(|(quarters, _, _)| quarters.berths)
            .sum();
        let capacity = existing_berths + new_berths;
        let alive = crew_query.iter().filter(|c| c.health > 0.0).count() as u32;
        let to_spawn = capacity.saturating_sub(alive);

        let berths = walking::quarters_cells(ours().map(|(_, module, _)| module));

        let mut rng = rand::thread_rng();
        let mut aboard: Vec<String> = crew_query.iter().map(|c| c.name.clone()).collect();
        for i in 0..to_spawn {
            let name = names::unused_name(&aboard, &mut rng);
            aboard.push(name.clone());
            let crew = commands
                .spawn((
                    animation::crew_sprite(&assets, &crew_atlases),
                    Transform::from_translation(walking::berth_position(&berths, alive as usize + i as usize)),
                    CrewMember {
                        name,
                        health: 100.0,
                        max_health: 100.0,
                        oxygen: 100.0,
                        morale: 100.0,
                        state: CrewState::Idle,
                    },
                ))
                .insert(ChildOf(ship))
                .id();
            roster.members.push(crew);
        }

        notifications.write(ShowNotification {
            message: format!(
                "{} crew signed on - bunks full ({}/{}).",
                to_spawn,
                alive + to_spawn,
                capacity
            ),
            notification_type: NotificationType::Success,
            duration: 3.0,
        });
    }
}

/// Spawns the initial crew, one per berth, no skills.
///
/// Was a fixed roster of 8 regardless of the hull. That silently undercrewed
/// the ship the moment `weapon_is_crewed` started gating the guns: the starter
/// carries 20 posts, so eight hands left twelve stations — most of the battery
/// among them — dark, with no signal beyond "the guns don't fire".
pub fn spawn_starter_crew(
    mut commands: Commands,
    assets: Res<AssetServer>,
    crew_atlases: Res<animation::CrewAtlases>,
    ship_query: Query<Entity, With<Ship>>,
    // Must exclude AI crew. This runs on EVERY dock, not just startup, and
    // unscoped it meant any living enemy crewman anywhere suppressed the
    // player's replacement crew — so a wiped-out run could not recover by
    // docking either.
    existing_crew: Query<Entity, (With<CrewMember>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    quarters_query: Query<(&Quarters, &Module, &ChildOf)>,
    mut roster: ResMut<CrewRoster>,
) {
    // Guard: don't spawn duplicate crew
    if !existing_crew.is_empty() {
        return;
    }
    let Ok(ship) = ship_query.single() else {
        return;
    };

    // Sail with a full complement: every berth the hull provides is filled.

    let complement = quarters_query
        .iter()
        .filter(|(_, module, parent)| {
            parent.parent() == ship && module.is_active && module.health > 0.0
        })
        .map(|(quarters, _, _)| quarters.berths)
        .sum::<u32>() as usize;

    // Start them in the bunks. They walk to their posts from there, which is
    // both how a watch actually changes and a free demonstration that the
    // pathing works on the ship the player is looking at.
    let berths = walking::quarters_cells(
        quarters_query
            .iter()
            .filter(|(_, _, parent)| parent.parent() == ship)
            .map(|(_, module, _)| module),
    );

    for i in 0..complement {
        // Past the written names, hands are numbered rather than repeated —
        // two crew called Jones on the same roster reads as a bug.
        let name = names::starting_name(i);
        let crew = commands.spawn((
            animation::crew_sprite(&assets, &crew_atlases),
            Transform::from_translation(walking::berth_position(&berths, i)),
            CrewMember {
                name,
                health: 100.0,
                max_health: 100.0,
                oxygen: 100.0,
                morale: 100.0,
                state: CrewState::Idle,
            },
        )).insert(ChildOf(ship)).id();

        roster.members.push(crew);
    }

    info!("Spawned {} crew members to fill {} berths", complement, complement);
}

/// How many nozzles one engineer can keep running by walking between them.
pub const ENGINES_PER_OPERATOR: usize = 3;

/// Is this crew member available to work a post right now?
fn crew_on_duty(crew: &CrewMember) -> bool {
    crew.health > 0.0
        && crew.state != CrewState::Panicking
        && crew.state != CrewState::Unconscious
        && crew.state != CrewState::Salvaging
}

/// Computes ModuleEfficiency for all modules with a CrewStation.
/// staffing_factor: 0.0 unstaffed, 1.0 staffed (crew alive, aboard, and
/// not panicking/unconscious) — a station nobody operates DOES NOT RUN.
/// That's the teeth behind crew scarcity: send everyone out on salvage
/// and the unmanned reactors/engines/guns go dark until they're back.
/// value = damage_efficiency * staffing_factor
/// How much of a station a ship can work with nobody standing at it, given
/// how many Memory Cores it has alive.
///
/// Diminishing, same shape as apply_targeting_computer_bonus: each core is
/// worth less than the last, and no number of them replaces a crew. The cap
/// is the important half — a ship can never run itself outright, or crew stop
/// being a thing you need and the whole staffing layer goes quiet.
pub fn autonomy_from_cores(core_autonomy: &[f32]) -> f32 {
    let mut combined = 0.0f32;
    for a in core_autonomy {
        combined = 1.0 - (1.0 - combined) * (1.0 - a.clamp(0.0, 1.0));
    }
    combined.min(MAX_AUTONOMY)
}

/// Ceiling on self-operation. Deliberately well under half: the ship covering
/// for a missing hand is a different thing from the ship not needing hands.
pub const MAX_AUTONOMY: f32 = 0.45;

fn compute_module_efficiency(
    mut commands: Commands,
    mut station_query: Query<(Entity, &Module, &mut CrewStation, &ChildOf)>,
    crew_query: Query<&CrewMember>,
    core_query: Query<(&MemoryCoreComp, &Module, &ChildOf), Without<DestroyedModule>>,
) {
    // Cores are counted PER SHIP. This system is globally unscoped — it has
    // always run over AI ship modules too — so a single global autonomy figure
    // derived from the player's cores would quietly hand every enemy in the
    // system the same benefit. Same bug shape as the crew and power leaks that
    // turned up when AI ships first got real crews.
    let mut cores_by_ship: bevy::platform::collections::HashMap<Entity, Vec<f32>> =
        bevy::platform::collections::HashMap::new();
    for (core, module, parent) in core_query.iter() {
        if !module.is_active {
            continue;
        }
        cores_by_ship.entry(parent.parent()).or_default().push(core.autonomy);
    }
    // One engineer walks a bank of ENGINES_PER_OPERATOR nozzles. Not one hand
    // each (five engines ate five of eight crew and the guns stayed dark), and
    // not one hand for the whole ship however big it gets — a hundred-engine
    // hull should still cost you a black gang.
    //
    // Which engines a short-handed room keeps running is resolved by grid
    // position so it's stable frame to frame; an engineer's own nozzle is
    // always covered first.
    let mut covered_propulsion: bevy::platform::collections::HashSet<Entity> = bevy::platform::collections::HashSet::new();
    {
        let mut by_ship: bevy::platform::collections::HashMap<Entity, (Vec<(IVec2, Entity)>, usize)> =
            bevy::platform::collections::HashMap::new();
        for (entity, module, station, parent) in station_query.iter() {
            if module.module_type.category() != ModuleCategory::Propulsion {
                continue;
            }
            let slot = by_ship.entry(parent.parent()).or_default();
            let manned = station
                .assigned_crew
                .is_some_and(|c| crew_query.get(c).is_ok_and(crew_on_duty));
            if manned {
                slot.1 += 1;
                covered_propulsion.insert(entity);
            } else {
                slot.0.push((module.grid_position, entity));
            }
        }
        for (_, (mut unmanned, operators)) in by_ship {
            let capacity = operators * ENGINES_PER_OPERATOR;
            let spare = capacity.saturating_sub(operators);
            unmanned.sort_by_key(|(cell, _)| (cell.x, cell.y));
            for (_, entity) in unmanned.into_iter().take(spare) {
                covered_propulsion.insert(entity);
            }
        }
    }

    for (entity, module, mut station, parent) in station_query.iter_mut() {
        let ratio = if module.max_health > 0.0 { module.health / module.max_health } else { 1.0 };
        let damage_eff = ModuleDamageState::from_health_ratio(ratio).efficiency();

        let manned = match station.assigned_crew {
            Some(crew_entity) => match crew_query.get(crew_entity) {
                Ok(crew) if crew_on_duty(crew) => true,
                // Dead/panicking/unconscious/EVA crew, or an entity that no
                // longer exists — clear the assignment either way.
                _ => {
                    station.assigned_crew = None;
                    false
                }
            },
            None => false,
        };

        let staffing_factor = if manned {
            1.0
        } else if covered_propulsion.contains(&entity) {
            // Within an engineer's rounds.
            1.0
        } else {
            // Nobody is at this post. What the ship can still do for itself
            // depends on how much of its mind is intact.
            cores_by_ship
                .get(&parent.parent())
                .map(|c| autonomy_from_cores(c))
                .unwrap_or(0.0)
        };

        // try_insert: wreck modules carry CrewStation too, and a drill or
        // EVA detail may have dismantled (despawned) this block this frame
        commands.entity(entity).try_insert(ModuleEfficiency {
            value: damage_eff * staffing_factor,
            staffing_factor,
        });
    }
}

/// Counts total berths, crew, staffed/total stations. Writes to StaffingState.
/// Player-scoped: this feeds the player's own crew HUD, and both Quarters/
/// CrewStation/CrewMember exist on AI ships too now — an unscoped count
/// here would blend every AI ship in render range into the player's own
/// crew numbers (see OwnedByAiShip usage below, same marker the AI-vs-AI
/// projectile-ownership work already established for this exact class of
/// "unscoped query silently picks up AI ship data" bug).
fn update_staffing_state(
    quarters_query: Query<(&Quarters, &Module, Option<&crate::ai_ship::components::OwnedByAiShip>)>,
    station_query: Query<(&CrewStation, Option<&crate::ai_ship::components::OwnedByAiShip>)>,
    crew_query: Query<(&CrewMember, Option<&crate::ai_ship::components::OwnedByAiShip>)>,
    mut staffing: ResMut<StaffingState>,
) {
    let mut total_berths = 0u32;
    for (quarters, module, owned) in quarters_query.iter() {
        if owned.is_some() { continue; }
        if module.is_active && module.health > 0.0 {
            total_berths += quarters.berths;
        }
    }

    let mut staffed = 0u32;
    let mut total = 0u32;
    for (station, owned) in station_query.iter() {
        if owned.is_some() { continue; }
        total += 1;
        if let Some(crew_entity) = station.assigned_crew {
            // Only count as staffed if the crew is alive and still exists
            if let Ok((crew, _)) = crew_query.get(crew_entity) {
                if crew.health > 0.0 {
                    staffed += 1;
                }
            }
        }
    }

    // Count living player crew
    let alive_crew = crew_query.iter()
        .filter(|(c, owned)| c.health > 0.0 && owned.is_none())
        .count() as u32;

    staffing.total_berths = total_berths;
    staffing.total_crew = alive_crew;
    staffing.staffed_stations = staffed;
    staffing.total_stations = total;
}

/// Priority-based auto-assignment of crew to stations. Grouped per owning
/// ship (player ship or a specific AI ship root) via OwnedByAiShip — AI
/// ships now carry real CrewStation/CrewMember data too (see ai_ship::crew),
/// and without this grouping a single global pool would happily staff one
/// ship's idle crew onto a completely different ship's open stations the
/// next tick this system runs.
/// Guns keep their crews this long after the last hostile leaves range.
/// Long enough that a contact drifting across the range edge does not have the
/// battery manning and standing down repeatedly, short enough that the crew
/// are free to get on with something else soon after a fight.
const GUNS_STAND_DOWN_AFTER: f32 = 10.0;

/// Engineers hold the bank this long after the throttle goes quiet, so
/// feathering the throttle does not empty and refill the engine room.
const ENGINES_STAND_DOWN_AFTER: f32 = 10.0;

/// How long the ship has been out of a fight and off the throttle.
#[derive(Default)]
pub struct StandDown {
    since_hostile: f32,
    since_thrust: f32,
}

/// Does this post need a body on it right now?
///
/// A gun with nothing to shoot at and an engine with no throttle are not jobs,
/// they are furniture. Keeping hands nailed to them in peacetime meant a ship
/// with twenty berths and twenty stations had nobody spare to do anything —
/// no damage control between fights, and nobody to carry the dead to the lock.
/// Everything else (power, air, sensors, the medbay) runs continuously and is
/// always wanted.
fn post_is_wanted(category: ModuleCategory, at_battle_stations: bool, under_thrust: bool) -> bool {
    match category {
        ModuleCategory::Weapons => at_battle_stations,
        ModuleCategory::Propulsion => under_thrust,
        _ => true,
    }
}

fn auto_assign_crew(
    time: Res<Time>,
    mut timer: ResMut<AutoAssignTimer>,
    mut station_query: Query<(Entity, &Module, &mut CrewStation, Has<KeepManned>, Option<&crate::ai_ship::components::OwnedByAiShip>)>,
    crew_query: Query<(
        Entity,
        &CrewMember,
        Option<&CrewDuty>,
        Option<&crate::ai_ship::components::OwnedByAiShip>,
    )>,
    // Identity and state are separate queries ON PURPOSE. Folding the
    // transform and physics into this one made a ship that has neither stop
    // being recognised as the player's at all, and every post on it went
    // unstaffed — the ship is the fallback owner for anything without
    // OwnedByAiShip, so losing it loses the whole assignment pass.
    ship_query: Query<Entity, With<Ship>>,
    ship_motion: Query<(&GlobalTransform, &ShipPhysics), With<Ship>>,
    hostiles: Query<&GlobalTransform, With<crate::ai_ship::components::AiShip>>,
    keeping_range: Option<Res<crate::ship::keep_range::KeepRange>>,
    mut quiet: Local<StandDown>,
) {
    // Accumulated every frame, deliberately BEFORE the timer gate — the body
    // below runs on a slow cadence and would otherwise measure nothing.
    // With no motion data the counters stay at zero, which reads as "in a
    // fight and under way": posts stay manned, the safe default.
    let dt = time.delta_secs();
    if let Ok((ship_gt, physics)) = ship_motion.single() {
        if crate::crew::walking::under_threat(
            ship_gt.translation().truncate(),
            hostiles.iter().map(|h| h.translation().truncate()),
        ) {
            quiet.since_hostile = 0.0;
        } else {
            quiet.since_hostile += dt;
        }
        // Holding range on a ship (ship::keep_range) is the helm at work even
        // while it sits still: it needs the engines the moment the gap moves,
        // and with no throttle on it the engineers stood down mid-fight.
        let helm_busy = keeping_range.is_some_and(|k| k.ship.is_some());
        if physics.throttle.abs() > 0.01 || physics.rudder.abs() > 0.01 || helm_busy {
            quiet.since_thrust = 0.0;
        } else {
            quiet.since_thrust += dt;
        }
    }

    timer.timer.tick(time.delta());
    if !timer.timer.just_finished() {
        return;
    }

    let at_battle_stations = quiet.since_hostile < GUNS_STAND_DOWN_AFTER;
    let under_thrust = quiet.since_thrust < ENGINES_STAND_DOWN_AFTER;

    let player_ship = ship_query.single().ok();
    // Absent OwnedByAiShip => player-owned (the only other kind of ship).
    let owner_of = |owned: Option<&crate::ai_ship::components::OwnedByAiShip>| -> Option<Entity> {
        owned.map(|o| o.root).or(player_ship)
    };

    // Collect all crew currently assigned to any station
    let mut assigned_crew: bevy::platform::collections::HashSet<Entity> = bevy::platform::collections::HashSet::new();
    for (_, _, station, _, _) in station_query.iter() {
        if let Some(crew_entity) = station.assigned_crew {
            assigned_crew.insert(crew_entity);
        }
    }

    // Stand down posts with nothing to do. Player ship only: AI crews are not
    // simulated walking anywhere, and an enemy that stood its battery down
    // would simply be worse at fighting.
    //
    // A pinned post (KeepManned) is the player's standing order and outranks
    // this — if you have told someone to hold that seat, they hold it.
    for (_, module, mut station, pinned, owned) in station_query.iter_mut() {
        if pinned || station.manually_assigned || owner_of(owned) != player_ship {
            continue;
        }
        if post_is_wanted(module.module_type.category(), at_battle_stations, under_thrust) {
            continue;
        }
        if let Some(freed) = station.assigned_crew.take() {
            assigned_crew.remove(&freed);
        }
    }

    // Clean up dead/despawned crew from stations
    for (_, _, mut station, _, _) in station_query.iter_mut() {
        if let Some(crew_entity) = station.assigned_crew {
            if let Ok((_, crew, _, _)) = crew_query.get(crew_entity) {
                if crew.health <= 0.0 {
                    station.assigned_crew = None;
                    assigned_crew.remove(&crew_entity);
                }
            } else {
                // Entity no longer exists
                station.assigned_crew = None;
                assigned_crew.remove(&crew_entity);
            }
        }
    }

    // One engineer per ENGINES_PER_OPERATOR nozzles (see
    // compute_module_efficiency). Work out how many the room actually wants,
    // then release anyone beyond that so a big bank stops eating the crew.
    // Pinned posts are honoured first — a KeepManned engine keeps its hand.
    let mut engine_berths_wanted: bevy::platform::collections::HashMap<Entity, usize> =
        bevy::platform::collections::HashMap::new();
    for (_, module, _, _, owned) in station_query.iter() {
        if module.module_type.category() == ModuleCategory::Propulsion {
            if let Some(ship) = owner_of(owned) {
                *engine_berths_wanted.entry(ship).or_insert(0) += 1;
            }
        }
    }
    for wanted in engine_berths_wanted.values_mut() {
        *wanted = wanted.div_ceil(ENGINES_PER_OPERATOR);
    }

    let mut engine_berths_used: bevy::platform::collections::HashMap<Entity, usize> =
        bevy::platform::collections::HashMap::new();
    for pinned_pass in [true, false] {
        for (_, module, mut station, pinned, owned) in station_query.iter_mut() {
            if module.module_type.category() != ModuleCategory::Propulsion || pinned != pinned_pass {
                continue;
            }
            let Some(ship) = owner_of(owned) else { continue };
            if station.assigned_crew.is_none() {
                continue;
            }
            let used = engine_berths_used.entry(ship).or_insert(0);
            if *used < engine_berths_wanted.get(&ship).copied().unwrap_or(0) {
                *used += 1;
                continue;
            }
            if let Some(freed) = station.assigned_crew.take() {
                assigned_crew.remove(&freed);
            }
        }
    }

    // Collect unfilled stations (priority > 0, not manually assigned), bucketed by owning ship
    let mut unfilled_by_ship: bevy::platform::collections::HashMap<Entity, Vec<(Entity, u8, bool, ModuleCategory)>> = bevy::platform::collections::HashMap::new();
    for (entity, module, station, pinned, owned) in station_query.iter() {
        if station.priority > 0 && !station.manually_assigned && station.assigned_crew.is_none() {
            if let Some(ship) = owner_of(owned) {
                // Nothing to shoot at, or no throttle: leave the seat empty.
                if Some(ship) == player_ship
                    && !pinned
                    && !post_is_wanted(
                        module.module_type.category(),
                        at_battle_stations,
                        under_thrust,
                    )
                {
                    continue;
                }
                // Don't offer more engine berths than the room needs.
                if module.module_type.category() == ModuleCategory::Propulsion {
                    let used = engine_berths_used.entry(ship).or_insert(0);
                    if *used >= engine_berths_wanted.get(&ship).copied().unwrap_or(0) {
                        continue;
                    }
                    *used += 1;
                }
                unfilled_by_ship
                    .entry(ship)
                    .or_default()
                    .push((entity, station.priority, pinned, module.module_type.category()));
            }
        }
    }

    // Collect available crew (alive, not panicking/unconscious, not assigned), bucketed by owning ship
    let mut available_by_ship: bevy::platform::collections::HashMap<Entity, Vec<(Entity, CrewDuty)>> = bevy::platform::collections::HashMap::new();
    for (entity, crew, duty, owned) in crew_query.iter() {
        if crew.health > 0.0
            && crew.state != CrewState::Panicking
            && crew.state != CrewState::Unconscious
            && crew.state != CrewState::Salvaging
            && !assigned_crew.contains(&entity)
        {
            if let Some(ship) = owner_of(owned) {
                available_by_ship
                    .entry(ship)
                    .or_default()
                    .push((entity, duty.copied().unwrap_or_default()));
            }
        }
    }

    // Assign in order, independently per ship
    for (ship, mut unfilled) in unfilled_by_ship {
        let Some(available_crew) = available_by_ship.get(&ship) else { continue };

        // Pinned (keep-manned) posts staff first, then by priority descending
        unfilled.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));

        // Standing orders narrow who each post can draw on. Taking the first
        // WILLING hand rather than the first hand full stop is the whole
        // difference: a gunner is passed over for the reactor and is still
        // there when the post they were told to take comes up.
        let mut taken: bevy::platform::collections::HashSet<Entity> = bevy::platform::collections::HashSet::new();
        for (station_entity, _priority, _pinned, category) in unfilled {
            let Some(&(crew_entity, _)) = available_crew
                .iter()
                .find(|(e, duty)| !taken.contains(e) && duty.allows(category))
            else {
                continue;
            };
            taken.insert(crew_entity);

            if let Ok((_, _, mut station, _, _)) = station_query.get_mut(station_entity) {
                station.assigned_crew = Some(crew_entity);
            }
        }
    }
}

/// Updates crew needs (morale). Personal oxygen was removed by design
/// call 2026-07-15 — crew no longer consume O2 or suffocate; room air
/// and decompression stay as pure physics (vent thrust, breach sealing).
fn update_crew_needs(
    time: Res<Time>,
    depth_state: Res<DepthState>,
    cascade: Res<crate::narrative::CascadeState>,
    // EVA crew are on suit systems — needs frozen while outside.
    // The player's crew only: the dread is this run's distance from safety,
    // and AI crews sharing it drifted into panic late in a run unless the
    // PLAYER had built a Rec Room -- which no longer reaches them.
    mut crew_query: Query<
        &mut CrewMember,
        (Without<EvaSalvaging>, Without<crate::ai_ship::components::OwnedByAiShip>),
    >,
) {
    for mut crew in crew_query.iter_mut() {
        if crew.health <= 0.0 {
            continue;
        }

        if depth_state.current_depth > 500.0 {
            // Distance erodes morale, and how far it can erode depends on how
            // far along the run is.
            //
            // The floor used to be a flat 35, above both the panic threshold
            // (20) and the recovery threshold (30), for a good reason: letting
            // it reach zero locked every deep-zone crew into permanent panic
            // and nobody could man a station or crew a salvage detail out
            // where the wrecks actually are. That failure is still real and
            // this does not undo it.
            //
            // What changes is that the floor slides with the cascade. Early it
            // is the old 35 exactly. At the far edge it dips just under the
            // panic threshold, so an unmitigated crew finally does start to
            // break — and the answer is a thing the player can build. A Rec
            // Room hard-floors morale at 30 and a Mess Hall pushes it back up,
            // so going out that far becomes a question of whether the ship was
            // designed for it, rather than an unavoidable loss.
            let floor = DREAD_FLOOR_NEAR
                + (DREAD_FLOOR_FAR - DREAD_FLOOR_NEAR) * cascade.level.clamp(0.0, 1.0);
            crew.morale = (crew.morale - 5.0 * time.delta_secs()).max(floor);
        } else {
            crew.morale = (crew.morale + 1.0 * time.delta_secs()).min(100.0);
        }
    }
}

/// Morale floor from distance alone, at the start of a run and at the end of
/// one. FAR sits just under the panic threshold of 20 on purpose: reachable,
/// but only at the edge, and answerable by building for it.
const DREAD_FLOOR_NEAR: f32 = 35.0;
const DREAD_FLOOR_FAR: f32 = 18.0;

/// Maps each crew member's world position to a grid position and room via RoomMap.
fn update_crew_room_location(
    mut commands: Commands,
    // LOCAL transform, not global. Crew are children of their ship and every
    // room id in `RoomMap` is a ship-local cell (see rooms::transform_to_grid,
    // which reads hull segments' own local transforms). Measuring a crew
    // member against the world instead put them off by the ship's entire
    // displacement the moment it left the origin — a hand standing in the
    // MedBay reported a cell hundreds of tiles away, so their room never
    // matched anything and every room-scoped behaviour downstream (medical
    // treatment, fire and breach dispatch, room-local repair) silently did
    // nothing in flight. `local_to_grid`'s own doc says it: world-space
    // callers must undo the ship transform first.
    mut crew_query: Query<(Entity, &Transform, Option<&mut CrewRoomLocation>), (With<CrewMember>, Without<EvaSalvaging>)>,
    room_map: Res<RoomMap>,
) {
    for (entity, transform, location) in crew_query.iter_mut() {
        let grid = crate::building::local_to_grid(transform.translation.truncate());
        let room_id = room_map.tile_to_room.get(&grid).copied();

        if let Some(mut loc) = location {
            loc.room_id = room_id;
            loc.grid_position = grid;
        } else {
            // try_insert: the crew member may die and despawn this frame
            commands.entity(entity).try_insert(CrewRoomLocation {
                room_id,
                grid_position: grid,
            });
        }
    }
}

/// Scans for rooms with decompression or fire and dispatches idle crew to handle emergencies.
/// Temporarily clears non-manual CrewStation assignments for dispatched crew.
/// Room-scoped both ways: only crew IN an emergency room get flagged
/// (repair/suppression power is room-local and crew can't walk between
/// rooms, so flagging distant crew just locked them in Repairing doing
/// nothing — which starved every other job, e.g. salvage details), and
/// Repairing crew whose room is calm get released back to Idle.
fn crew_emergency_dispatch(
    ship_query: Query<Entity, With<Ship>>,
    child_query: Query<&ChildOf>,
    mut crew_query: Query<
        (Entity, &mut CrewMember, Option<&CrewRoomLocation>),
        Without<DamageControl>,
    >,
    fire_query: Query<(Entity, &Module), With<OnFire>>,
    room_map: Res<RoomMap>,
    mut station_query: Query<(Entity, &mut CrewStation)>,
    mut dispatch_events: MessageWriter<CrewDispatched>,
) {
    // Build priority list of emergency rooms: decompression first, then fire
    let mut emergency_rooms: Vec<(usize, DispatchReason)> = Vec::new();

    for room in room_map.rooms.iter() {
        if room.is_breached && room.air_level < 1.0 {
            emergency_rooms.push((room.id, DispatchReason::Decompression));
        }
    }

    // Check for rooms with fire — our modules only; a burning wreck's
    // grid positions can phantom-match our room map's tiles.
    let ship = ship_query.single().ok();
    for (entity, module) in fire_query.iter() {
        if child_query.get(entity).ok().map(|p| p.0) != ship {
            continue;
        }
        if let Some(&room_id) = room_map.tile_to_room.get(&module.grid_position) {
            if !emergency_rooms.iter().any(|(id, _)| *id == room_id) {
                emergency_rooms.push((room_id, DispatchReason::Fire));
            }
        }
    }

    // Collect crew assigned to stations (to know who to pull)
    let mut station_assignments: bevy::platform::collections::HashMap<Entity, Entity> = bevy::platform::collections::HashMap::new();
    for (station_entity, station) in station_query.iter() {
        if let Some(crew_entity) = station.assigned_crew {
            if !station.manually_assigned {
                station_assignments.insert(crew_entity, station_entity);
            }
        }
    }

    for (entity, mut crew, location) in crew_query.iter_mut() {
        if crew.health <= 0.0 {
            continue;
        }
        let room = location.and_then(|l| l.room_id);
        let emergency_here = room.and_then(|r| {
            emergency_rooms.iter().find(|(id, _)| *id == r).copied()
        });

        match (crew.state, emergency_here) {
            (CrewState::Idle, Some((room_id, reason))) => {
                crew.state = CrewState::Repairing;

                // Clear station assignment if not manually assigned
                if let Some(station_entity) = station_assignments.get(&entity) {
                    if let Ok((_, mut station)) = station_query.get_mut(*station_entity) {
                        station.assigned_crew = None;
                    }
                }

                dispatch_events.write(CrewDispatched {
                    crew: entity,
                    room_id,
                    reason,
                });
            }
            (CrewState::Repairing, None) => {
                crew.state = CrewState::Idle;
            }
            _ => {}
        }
    }
}

/// Updates crew AI behavior — now aware of both decompression and fires.
fn update_crew_ai(
    ship_query: Query<Entity, With<Ship>>,
    child_query: Query<&ChildOf>,
    hull_query: Query<(Entity, &HullSegment, &Transform)>,
    fire_query: Query<Entity, With<OnFire>>,
    // EVA crew's state machine is owned by eva_salvage while they're out
    // DamageControl crew have their state set by suits::suited_damage_control;
    // resetting Repairing here would stop them sealing the hole they were sent to.
    mut crew_query: Query<&mut CrewMember, (Without<EvaSalvaging>, Without<DamageControl>)>,
) {
    let Ok(ship) = ship_query.single() else { return };
    // Danger must be OUR danger — unscoped, any holed/burning wreck
    // drifting nearby kept the crew stuck in Repairing forever.
    let has_depressurized = hull_query.iter().any(|(entity, hull, _)| {
        hull.is_depressurized && child_query.get(entity).is_ok_and(|p| p.0 == ship)
    });
    let has_fires = fire_query
        .iter()
        .any(|entity| child_query.get(entity).is_ok_and(|p| p.0 == ship));
    let has_danger = has_depressurized || has_fires;

    for mut crew in crew_query.iter_mut() {
        if crew.health <= 0.0 {
            continue;
        }

        match crew.state {
            CrewState::Repairing => {
                // Return to idle when no more danger
                if !has_danger {
                    crew.state = CrewState::Idle;
                }
            }
            CrewState::Panicking => {
                if crew.morale > 30.0 {
                    crew.state = CrewState::Idle;
                }
            }
            _ => {}
        }

        if crew.morale < 20.0 {
            crew.state = CrewState::Panicking;
        }
    }
}

/// Crew in Repairing state suppress fires in their room.
/// Since skills are removed, each crew member contributes a flat suppression value.
fn crew_fire_suppression(
    time: Res<Time>,
    mut commands: Commands,
    ship_query: Query<Entity, With<Ship>>,
    child_query: Query<&ChildOf>,
    crew_query: Query<(&CrewMember, &CrewRoomLocation)>,
    mut fire_query: Query<(Entity, &mut OnFire, &Module, &mut Sprite), Without<DestroyedModule>>,
    room_map: Res<RoomMap>,
    mut extinguish_events: MessageWriter<FireExtinguished>,
) {
    let dt = time.delta_secs();

    // Build per-room suppression power from repairing crew (flat 1.0 per crew)
    let mut room_suppression: bevy::platform::collections::HashMap<usize, f32> = bevy::platform::collections::HashMap::new();
    for (crew, location) in crew_query.iter() {
        if crew.state != CrewState::Repairing || crew.health <= 0.0 {
            continue;
        }
        if let Some(room_id) = location.room_id {
            *room_suppression.entry(room_id).or_insert(0.0) += 0.8;
        }
    }

    if room_suppression.is_empty() {
        return;
    }

    // Apply suppression to fires — our modules only (see dispatch note)
    let ship = ship_query.single().ok();
    for (entity, mut fire, module, mut sprite) in fire_query.iter_mut() {
        if child_query.get(entity).ok().map(|p| p.0) != ship {
            continue;
        }
        let Some(&room_id) = room_map.tile_to_room.get(&module.grid_position) else {
            continue;
        };
        let Some(&suppression) = room_suppression.get(&room_id) else {
            continue;
        };

        fire.intensity -= suppression * 0.03 * dt;
        fire.damage_per_second = 8.0 * fire.intensity.max(0.0);

        if fire.intensity < 0.05 {
            commands.entity(entity).remove::<OnFire>();
            sprite.color = Color::srgb(0.2, 0.2, 0.2);
            extinguish_events.write(FireExtinguished {
                module: entity,
                cause: FireExtinguishCause::CrewSuppressed,
            });
        }
    }
}

/// HP restored per ScrapMetal consumed by field repair.
const HP_PER_SCRAP: f32 = 25.0;
/// Idle crew patch at half the rate of crew actively dispatched to an
/// emergency.
const IDLE_REPAIR_POWER: f32 = 0.5;

/// Field-repair material budget. Hull/module HP healing draws from this;
/// when the credit runs dry it converts ScrapMetal from the hold, and when
/// there's no scrap left, patching stalls (breach sealing and fire
/// suppression stay free — damage control needs hands, not plates).
#[derive(Resource, Default)]
pub struct RepairScrapPool {
    hp_credit: f32,
}

impl RepairScrapPool {
    /// Grant up to `want` HP of repair, converting scrap as needed.
    /// Returns how much was actually granted.
    fn draw(&mut self, want: f32, inventory: &mut Inventory) -> f32 {
        let mut granted = 0.0;
        let mut remaining = want;
        while remaining > 0.0 {
            if self.hp_credit <= 0.0 {
                if !inventory.remove_item(ItemType::ScrapMetal, 1) {
                    break;
                }
                self.hp_credit += HP_PER_SCRAP;
            }
            let take = remaining.min(self.hp_credit);
            self.hp_credit -= take;
            granted += take;
            remaining -= take;
        }
        granted
    }
}

/// Room-local crew repair system with RepairBay boost.
/// Crew contribute flat repair power (no skills). Emergency-dispatched
/// (Repairing) crew work at full power; IDLE crew in a damaged room pitch
/// in at half power — the ship self-heals in the field as long as there's
/// ScrapMetal aboard to feed the patches.
fn crew_repair_system(
    time: Res<Time>,
    ship_query: Query<(Entity, &crate::building::ShipGrid), With<Ship>>,
    crew_query: Query<(&CrewMember, &Transform, &ChildOf)>,
    repair_bays: Query<(&Module, &RepairSystem, &ChildOf), Without<DestroyedModule>>,
    mut hull_query: Query<&mut HullSegment>,
    mut module_query: Query<&mut Module, (Without<DestroyedModule>, Without<RepairSystem>)>,
    room_map: Res<RoomMap>,
    mut inventory: ResMut<Inventory>,
    mut pool: ResMut<RepairScrapPool>,
    mut notifications: MessageWriter<ShowNotification>,
    mut repaired_notified: Local<bool>,
    mut stall_notified: Local<bool>,
) {
    let dt = time.delta_secs();
    let Ok((player_ship, grid)) = ship_query.single() else { return };

    // Repair power is where the crew are STANDING, not which room they were
    // filed under. Rooms were the right model when nobody could move; now that
    // they walk, a hand mends what's within arm's reach — the cell under them
    // and its four neighbours. It also drops a dependency on GridOccupancy,
    // which only rebuilds at dock and is stale for the whole flight.
    let mut cell_power: bevy::platform::collections::HashMap<IVec2, f32> = bevy::platform::collections::HashMap::new();
    for (crew, transform, parent) in crew_query.iter() {
        if crew.health <= 0.0 || parent.parent() != player_ship {
            continue;
        }
        let power = match crew.state {
            CrewState::Repairing => 1.0,
            CrewState::Idle => IDLE_REPAIR_POWER,
            _ => continue,
        };
        let here = crate::building::local_to_grid(transform.translation.truncate());
        for reach in [IVec2::ZERO, IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            *cell_power.entry(here + reach).or_insert(0.0) += power;
        }
    }

    // Our bays only. Room ids are the player's, and a bay's grid_position is
    // local to whichever ship carries it, so an enemy bay at the same cell
    // numbers was boosting -- and passively mending -- our room.
    let our_bays: Vec<(&Module, &RepairSystem)> = repair_bays
        .iter()
        .filter(|(_, _, parent)| parent.parent() == player_ship)
        .map(|(module, repair_sys, _)| (module, repair_sys))
        .collect();

    // Build per-room RepairBay boost
    let mut room_repair_boost: bevy::platform::collections::HashMap<usize, f32> = bevy::platform::collections::HashMap::new();
    for &(module, repair_sys) in &our_bays {
        if let Some(&room_id) = room_map.tile_to_room.get(&module.grid_position) {
            let boost = room_repair_boost.entry(room_id).or_insert(0.0);
            *boost += repair_sys.repair_rate;
        }
    }

    let mut any_repaired = false;
    let mut repair_stalled = false;

    // Repair whatever is under each crew member's hands
    for (&tile, crew_power) in cell_power.iter() {
        let boost = room_map
            .tile_to_room
            .get(&tile)
            .and_then(|id| room_repair_boost.get(id))
            .copied()
            .unwrap_or(0.0);
        let total_power = crew_power + boost;

        {
            {
                let Some(entity) = grid.get(tile) else { continue };
                if let Ok(mut hull) = hull_query.get_mut(entity) {
                    // Breach sealing is free — damage control, not materials
                    if hull.is_depressurized && hull.depressurization_level > 0.0 {
                        let repair_rate = total_power * 0.05 * dt;
                        hull.depressurization_level = (hull.depressurization_level - repair_rate).max(0.0);
                        if hull.depressurization_level <= 0.0 {
                            hull.is_depressurized = false;
                            any_repaired = true;
                        }
                    }
                    // Patch hull health if damaged and not depressurized —
                    // costs ScrapMetal via the pool
                    if hull.health < hull.max_health && !hull.is_depressurized {
                        let want = (total_power * 2.0 * dt).min(hull.max_health - hull.health);
                        let granted = pool.draw(want, &mut inventory);
                        hull.health += granted;
                        if granted < want {
                            repair_stalled = true;
                        }
                    }
                } else if let Ok(mut module) = module_query.get_mut(entity) {
                    // Crew also patch damaged modules in their room (new —
                    // previously only the RepairBay touched module health)
                    if module.health < module.max_health && module.health > 0.0 {
                        let want = (total_power * 2.0 * dt).min(module.max_health - module.health);
                        let granted = pool.draw(want, &mut inventory);
                        module.health += granted;
                        if granted < want {
                            repair_stalled = true;
                        }
                    }
                }
            }
        }
    }

    // RepairBay passive module repair (even without crew, repair_rate * dt)
    for &(bay_module, repair_sys) in &our_bays {
        if let Some(&room_id) = room_map.tile_to_room.get(&bay_module.grid_position) {
            if let Some(room) = room_map.rooms.get(room_id) {
                for &tile in &room.tiles {
                    if let Some(entity) = grid.get(tile) {
                        if let Ok(mut module) = module_query.get_mut(entity) {
                            if module.health < module.max_health && module.health > 0.0 {
                                module.health = (module.health + repair_sys.repair_rate * dt).min(module.max_health);
                            }
                        }
                    }
                }
            }
        }
    }

    if any_repaired && !*repaired_notified {
        *repaired_notified = true;
        notifications.write(ShowNotification {
            message: "Crew repaired a hull breach!".into(),
            notification_type: NotificationType::Success,
            duration: 3.0,
        });
    }
    if !any_repaired {
        *repaired_notified = false;
    }

    // Out of scrap with damage still waiting — tell the player once, and
    // again if it happens again after resupplying.
    if repair_stalled && !*stall_notified {
        *stall_notified = true;
        notifications.write(ShowNotification {
            message: "Field repairs stalled - no ScrapMetal aboard".into(),
            notification_type: NotificationType::Warning,
            duration: 4.0,
        });
    }
    if !repair_stalled {
        *stall_notified = false;
    }
}

/// Declares death for anyone whose health has reached zero, however they got
/// there.
///
/// A sweep rather than a line at each damage site, because there were three of
/// those and only one — the EVA blast — ever remembered to raise the event.
/// Radiation and boarders quietly left a zero-HP crew member in the roster
/// indefinitely: never announced, never buried, still counted as a hand, still
/// holding a bunk, still drawn on the deck as a body nobody had been told
/// about. Anything added later that can hurt a person is covered for free.
///
/// AI-ship crew are excluded. They are casualties on somebody else's ship —
/// counting them in `Statistics::crew_lost` would report the player losing a
/// hand every time they gutted an enemy's engine room.
fn report_crew_deaths(
    crew: Query<(Entity, &CrewMember), Without<crate::ai_ship::components::OwnedByAiShip>>,
    mut damage_events: MessageReader<CrewDamaged>,
    mut deaths: MessageWriter<CrewDied>,
    mut wounds: Local<bevy::platform::collections::HashMap<Entity, CrewDamageSource>>,
) {
    // Remember the last thing that hurt each person. Kept across frames on
    // purpose: the killing blow and the frame this notices the body are not
    // reliably the same one, and a death that cannot name its cause is a
    // worse notification than one that can.
    for event in damage_events.read() {
        wounds.insert(event.crew, event.source.clone());
    }

    for (entity, member) in crew.iter() {
        if member.health > 0.0 {
            continue;
        }
        deaths.write(CrewDied {
            crew: entity,
            name: member.name.clone(),
            cause: wounds.remove(&entity).unwrap_or(CrewDamageSource::Unknown),
        });
    }

    // Anyone back to full health has no open wound to report.
    wounds.retain(|entity, _| {
        crew.get(*entity).is_ok_and(|(_, m)| m.health < m.max_health)
    });
}

/// Handles crew death events - despawn and update roster.
/// Also clears any CrewStation assignments for the dead crew.
fn handle_crew_death(
    mut commands: Commands,
    assets: Res<AssetServer>,
    crew_atlases: Res<animation::CrewAtlases>,
    corpse_pos: Query<(&Transform, Option<&ChildOf>), With<CrewMember>>,
    mut death_events: MessageReader<CrewDied>,
    mut roster: ResMut<CrewRoster>,
    mut statistics: ResMut<Statistics>,
    mut notifications: MessageWriter<ShowNotification>,
    mut station_query: Query<&mut CrewStation>,
) {
    for event in death_events.read() {
        roster.members.retain(|&e| e != event.crew);
        statistics.crew_lost += 1;

        // Clear station assignments for this crew
        for mut station in station_query.iter_mut() {
            if station.assigned_crew == Some(event.crew) {
                station.assigned_crew = None;
            }
        }

        notifications.write(ShowNotification {
            message: format!("{} is dead: {}.", event.name, event.cause.describe()),
            notification_type: NotificationType::Danger,
            duration: 6.0,
        });

        // Leave the body where they fell. The entity itself has to go -- every
        // staffing, oxygen and routing system would otherwise have to learn to
        // skip a corpse -- so a stripped-down stand-in takes its place.
        if let Ok((transform, parent)) = corpse_pos.get(event.crew) {
            animation::spawn_corpse(
                &mut commands,
                &assets,
                &crew_atlases,
                *transform,
                parent.map(|p| p.parent()),
                event.name.clone(),
            );
        }
        commands.entity(event.crew).try_despawn();
    }
}

// ============================================================================
// CREW FACILITY SYSTEMS (Phase 7)
// ============================================================================

/// HP per second in a working ward. Slow on purpose: recovery is something
/// you spend quiet time on between fights, not a between-volleys top-up.
const MEDBAY_HEAL_RATE: f32 = 1.0;

/// The MedBay treats whoever is in its room, while nobody is shooting.
///
/// Scaled by the ward's DAMAGE only, deliberately not by
/// `effective_efficiency`. A MedBay is `crew_station: true`, so that helper
/// reports 0 for any bay without a crew member posted to it — which meant the
/// bay healed nobody at all unless you had spent a hand standing in it. The
/// patient walking through the door is the staffing that matters here; a
/// shot-up ward still treats people, just worse.
///
/// `crew_query` needs `CrewRoomLocation`, which only exists on crew that have
/// a `Transform` — so AI-ship crew fall out of this for free.
fn medbay_healing(
    time: Res<Time>,
    facility_query: Query<(&CrewFacility, &Module, &ChildOf), Without<DestroyedModule>>,
    room_map: Res<RoomMap>,
    ships: Query<(Entity, &GlobalTransform), (With<Ship>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    hostiles: Query<&GlobalTransform, With<crate::ai_ship::components::AiShip>>,
    mut crew_query: Query<(&mut CrewMember, &CrewRoomLocation)>,
) {
    let Ok((ship, ship_gt)) = ships.single() else { return };
    // Same definition of "in a fight" that sent them here, so they cannot be
    // walked to the ward by one system and refused treatment by another.
    if walking::under_threat(
        ship_gt.translation().truncate(),
        hostiles.iter().map(|h| h.translation().truncate()),
    ) {
        return;
    }

    let dt = time.delta_secs();

    for (facility, module, parent) in facility_query.iter() {
        if facility.facility_type != FacilityType::MedBay || !module.is_active {
            continue;
        }
        // Ours only: the room lookup below is in the player's room map, so an
        // enemy ward would otherwise treat whoever stands at its cell numbers.
        if parent.parent() != ship {
            continue;
        }

        let ratio = if module.max_health > 0.0 { module.health / module.max_health } else { 1.0 };
        let condition = ModuleDamageState::from_health_ratio(ratio).efficiency();
        if condition <= 0.0 {
            continue;
        }

        let Some(&room_id) = room_map.tile_to_room.get(&module.grid_position) else {
            continue;
        };

        let heal_rate = MEDBAY_HEAL_RATE * condition * dt;

        for (mut crew, location) in crew_query.iter_mut() {
            if crew.health <= 0.0 || crew.health >= crew.max_health {
                continue;
            }
            if location.room_id == Some(room_id) {
                crew.health = (crew.health + heal_rate).min(crew.max_health);
            }
        }
    }
}

/// Facility rows for the morale systems: whose ship each one sits on.
type FacilityRow<'a> = (
    &'a CrewFacility,
    &'a Module,
    &'a ChildOf,
    Option<&'a crate::ai_ship::components::OwnedByAiShip>,
);

/// Ships carrying an active, intact facility of this kind. A facility serves
/// its own ship's crew: the player's by parentage, an AI ship's by
/// OwnedByAiShip, and anything else (a derelict) nobody. Unscoped, one enemy
/// mess hall anywhere in the world lifted the player's morale, and the
/// player's lifted every enemy crew's.
fn ships_with_facility<'a>(
    kind: FacilityType,
    facilities: impl Iterator<Item = FacilityRow<'a>>,
    player: Option<Entity>,
) -> bevy::platform::collections::HashSet<Entity> {
    facilities
        .filter(|(f, m, _, _)| f.facility_type == kind && m.is_active && m.health > 0.0)
        .filter_map(|(_, _, parent, owned)| match owned {
            Some(owned) => Some(owned.root),
            None => Some(parent.parent()).filter(|p| Some(*p) == player),
        })
        .collect()
}

/// The ship a crew member belongs to. Not ChildOf: a hand out on EVA is
/// unparented and still ours.
fn crew_ship(owned: Option<&crate::ai_ship::components::OwnedByAiShip>, player: Option<Entity>) -> Option<Entity> {
    owned.map(|o| o.root).or(player)
}

/// Active MessHall gives +2 morale/s to its ship's crew (passive).
fn messhall_morale(
    time: Res<Time>,
    ship_query: Query<Entity, With<Ship>>,
    facility_query: Query<FacilityRow>,
    mut crew_query: Query<(&mut CrewMember, Option<&crate::ai_ship::components::OwnedByAiShip>)>,
) {
    let dt = time.delta_secs();
    let player = ship_query.single().ok();
    let fed = ships_with_facility(FacilityType::MessHall, facility_query.iter(), player);

    if fed.is_empty() {
        return;
    }

    for (mut crew, owned) in crew_query.iter_mut() {
        if crew.health > 0.0 && crew_ship(owned, player).is_some_and(|s| fed.contains(&s)) {
            crew.morale = (crew.morale + 2.0 * dt).min(100.0);
        }
    }
}

/// Active RecRoom prevents its ship's crew morale from dropping below 30.
fn recroom_morale_floor(
    ship_query: Query<Entity, With<Ship>>,
    facility_query: Query<FacilityRow>,
    mut crew_query: Query<(&mut CrewMember, Option<&crate::ai_ship::components::OwnedByAiShip>)>,
) {
    let player = ship_query.single().ok();
    let floored = ships_with_facility(FacilityType::RecRoom, facility_query.iter(), player);

    if floored.is_empty() {
        return;
    }

    for (mut crew, owned) in crew_query.iter_mut() {
        if crew.health > 0.0
            && crew.morale < 30.0
            && crew_ship(owned, player).is_some_and(|s| floored.contains(&s))
        {
            crew.morale = 30.0;
        }
    }
}

/// Active TrainingRoom gives its ship's crew +1 morale/s and raises the panic
/// threshold from 20 to 10. Trained crew hold it together longer under stress.
fn training_room_boost(
    time: Res<Time>,
    ship_query: Query<Entity, With<Ship>>,
    facility_query: Query<FacilityRow>,
    mut crew_query: Query<(&mut CrewMember, Option<&crate::ai_ship::components::OwnedByAiShip>)>,
) {
    let dt = time.delta_secs();
    let player = ship_query.single().ok();
    let trained = ships_with_facility(FacilityType::TrainingRoom, facility_query.iter(), player);

    if trained.is_empty() {
        return;
    }

    for (mut crew, owned) in crew_query.iter_mut() {
        if crew.health > 0.0 && crew_ship(owned, player).is_some_and(|s| trained.contains(&s)) {
            // Morale boost (half of MessHall rate)
            crew.morale = (crew.morale + 1.0 * dt).min(100.0);
            // Trained crew resist panic better: recover from panicking at lower morale
            if crew.state == CrewState::Panicking && crew.morale > 15.0 {
                crew.state = CrewState::Idle;
            }
        }
    }
}

/// Active EngineeringStation boosts repair rate of nearby modules (+25%).
/// When staffed, RepairBay and HullPatch modules within 3 cells get a repair speed bonus.
fn engineering_station_boost(
    time: Res<Time>,
    facility_query: Query<(&CrewFacility, &Module, Option<&ModuleEfficiency>, &ChildOf), Without<RepairSystem>>,
    mut repair_query: Query<(&mut Module, &RepairSystem, &ChildOf), Without<DestroyedModule>>,
) {
    let dt = time.delta_secs();

    // Collect active engineering station positions with their efficiency,
    // and the ship each sits on: grid positions are ship-local, so "within 3
    // cells" only means anything between blocks of the same ship.
    let stations: Vec<(Entity, IVec2, f32)> = facility_query.iter()
        .filter(|(f, m, _, _)| {
            f.facility_type == FacilityType::EngineeringStation && m.is_active && m.health > 0.0
        })
        .map(|(_, m, eff, parent)| {
            let efficiency = effective_efficiency(m, eff);
            (parent.parent(), m.grid_position, efficiency)
        })
        .collect();

    if stations.is_empty() {
        return;
    }

    // Boost nearby repair modules
    for (mut repair_module, repair_sys, parent) in repair_query.iter_mut() {
        if !repair_module.is_active { continue; }

        for &(station_ship, station_pos, efficiency) in &stations {
            if station_ship != parent.parent() { continue; }
            let dist = (repair_module.grid_position - station_pos).as_vec2().length();
            if dist <= 3.0 {
                // +25% repair rate bonus scaled by efficiency
                let bonus_heal = repair_sys.repair_rate * 0.25 * efficiency * dt;
                // Apply to the repair module's own health as a small self-maintenance effect
                if repair_module.health < repair_module.max_health {
                    repair_module.health = (repair_module.health + bonus_heal).min(repair_module.max_health);
                }
            }
        }
    }
}

/// Finds crew members that aren't in the roster or parented to the ship
/// and fixes them. This handles crew hired at docking stations.
fn reconcile_hired_crew(
    mut commands: Commands,
    // EVA crew are deliberately un-parented — don't "fix" them mid-flight
    crew_query: Query<(Entity, Option<&ChildOf>), (With<CrewMember>, Without<EvaSalvaging>)>,
    ship_query: Query<Entity, With<Ship>>,
    mut roster: ResMut<CrewRoster>,
) {
    let Ok(ship) = ship_query.single() else { return };

    for (crew_entity, parent) in crew_query.iter() {
        // Add to roster if missing
        if !roster.members.contains(&crew_entity) {
            roster.members.push(crew_entity);
        }

        // Parent to ship if orphaned
        if parent.is_none() {
            commands.entity(crew_entity).insert(ChildOf(ship));
        }
    }
}

#[cfg(test)]
mod engine_room_tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn post(module_type: ModuleType, cell: IVec2, priority: u8) -> (Module, CrewStation) {
        (
            Module {
                module_type,
                health: 100.0,
                max_health: 100.0,
                power_consumption: 0.0,
                power_generation: 0.0,
                is_active: true,
                grid_position: cell,
                size: IVec2::ONE,
                rotation: Rotation::North,
            },
            CrewStation { priority, assigned_crew: None, manually_assigned: false },
        )
    }

    fn hand(name: &str) -> CrewMember {
        CrewMember {
            name: name.into(),
            health: 100.0,
            max_health: 100.0,
            oxygen: 100.0,
            morale: 100.0,
            state: CrewState::Idle,
        }
    }

    /// A ship at rest with nothing to shoot at, carrying enough hands for
    /// every post.
    fn peacetime_ship(app: &mut App) -> (Entity, Entity, Entity) {
        let ship = app
            .world_mut()
            .spawn((
                Ship,
                Transform::default(),
                GlobalTransform::default(),
                ShipPhysics::default(),
            ))
            .id();
        let gun = app
            .world_mut()
            .spawn(post(ModuleType::Gatling, IVec2::new(2, 0), 6))
            .insert(ChildOf(ship))
            .id();
        let engine = app
            .world_mut()
            .spawn(post(ModuleType::StandardEngine, IVec2::new(-2, 0), 9))
            .insert(ChildOf(ship))
            .id();
        let reactor = app
            .world_mut()
            .spawn(post(ModuleType::StandardReactor, IVec2::ZERO, 10))
            .insert(ChildOf(ship))
            .id();
        for name in ["Adeyemi", "Vasquez", "Ferreira", "Okonkwo"] {
            app.world_mut().spawn(hand(name)).insert(ChildOf(ship));
        }
        (gun, engine, reactor)
    }

    fn stand_down_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AutoAssignTimer>();
        app.add_systems(Update, auto_assign_crew);
        app
    }

    fn settle(app: &mut App) {
        // Time<Virtual> clamps steps over 250ms, so one big jump would be
        // silently shortened and the 2s auto-assign tick would never fire.
        // 150 x 200ms = 30s of sim, comfortably past both stand-down delays.
        // Keep this well clear of them: at 8s the battery is still correctly
        // manned and an impatient test reads that as a broken feature.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(200)));
        for _ in 0..150 {
            app.update();
        }
    }

    fn manned(app: &App, station: Entity) -> bool {
        app.world().get::<CrewStation>(station).unwrap().assigned_crew.is_some()
    }

    /// The point of the whole thing: a gun with nothing to shoot at and an
    /// engine with no throttle are furniture, not jobs. Nailing hands to them
    /// in peacetime left a ship with as many posts as berths no spare crew at
    /// all — no damage control between fights and nobody to carry the dead to
    /// the airlock.
    #[test]
    fn guns_and_engines_stand_down_in_peacetime() {
        let mut app = stand_down_app();
        let (gun, engine, reactor) = peacetime_ship(&mut app);

        settle(&mut app);

        assert!(!manned(&app, gun), "a gunner sat at the gun with nothing to shoot");
        assert!(!manned(&app, engine), "an engineer held the bank with no throttle");
        assert!(manned(&app, reactor), "the reactor was left unattended - it runs regardless");
    }

    /// Throttle up and the engine room fills again.
    #[test]
    fn the_engine_room_fills_when_the_throttle_opens() {
        let mut app = stand_down_app();
        let (_, engine, _) = peacetime_ship(&mut app);
        let ship = app.world_mut().query_filtered::<Entity, With<Ship>>()
            .iter(app.world()).next().unwrap();
        app.world_mut().get_mut::<ShipPhysics>(ship).unwrap().throttle = 1.0;

        settle(&mut app);

        assert!(manned(&app, engine), "engines got no operator under full throttle");
    }

    /// A hostile inside COMBAT_RANGE calls the crew to battle stations.
    #[test]
    fn a_hostile_in_range_mans_the_guns() {
        let mut app = stand_down_app();
        let (gun, _, _) = peacetime_ship(&mut app);
        app.world_mut().spawn((
            crate::ai_ship::components::AiShip,
            Transform::from_xyz(crate::crew::walking::COMBAT_RANGE * 0.5, 0.0, 0.0),
            GlobalTransform::from_xyz(crate::crew::walking::COMBAT_RANGE * 0.5, 0.0, 0.0),
        ));

        settle(&mut app);

        assert!(manned(&app, gun), "the battery stayed cold with a hostile in range");
    }

    /// A pinned post is the player's standing order and outranks stand-down.
    #[test]
    fn a_pinned_post_keeps_its_operator_in_peacetime() {
        let mut app = stand_down_app();
        let (gun, _, _) = peacetime_ship(&mut app);
        app.world_mut().entity_mut(gun).insert(KeepManned);

        settle(&mut app);

        assert!(manned(&app, gun), "a pinned gun lost its operator anyway");
    }

    /// An AI ship's guns must actually get manned.
    ///
    /// AI crew and AI stations both carry `OwnedByAiShip`, and `auto_assign_crew`
    /// resolves ownership through it — but the player ship is the fallback owner
    /// for anything WITHOUT that marker. If AI attribution ever regresses, the
    /// symptom is silent: enemy ships still spawn, still manoeuvre, and simply
    /// never shoot, because `ai_weapon_fire_system` gates on efficiency > 0.
    #[test]
    fn ai_ship_crew_man_their_own_guns() {
        use crate::ai_ship::components::OwnedByAiShip;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AutoAssignTimer>();
        app.add_systems(Update, (auto_assign_crew, compute_module_efficiency).chain());

        // A player ship must exist: it is the fallback owner, and its presence
        // is what makes mis-attribution possible in the first place.
        app.world_mut().spawn(Ship);

        let ai = app.world_mut().spawn_empty().id();
        let reactor = app.world_mut()
            .spawn(post(ModuleType::StandardReactor, IVec2::new(0, 0), 10))
            .insert((ChildOf(ai), OwnedByAiShip { root: ai })).id();
        let gun = app.world_mut()
            .spawn(post(ModuleType::Gatling, IVec2::new(2, 0), 6))
            .insert((ChildOf(ai), OwnedByAiShip { root: ai })).id();
        for name in ["Vex", "Sorrel"] {
            app.world_mut().spawn(hand(name))
                .insert((ChildOf(ai), OwnedByAiShip { root: ai }));
        }

        // Same cadence as the engine-bank test: Time<Virtual> clamps steps
        // over 250ms, so one big jump would be silently shortened and the
        // 2s auto-assign tick would never fire.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(200)));
        for _ in 0..14 {
            app.update();
        }

        let manned = |e: Entity, app: &App| {
            app.world().get::<CrewStation>(e).unwrap().assigned_crew.is_some()
        };
        assert!(manned(reactor, &app), "AI reactor left unmanned");
        assert!(manned(gun, &app), "AI gun left unmanned - enemies would never fire");

        let eff = app.world().get::<ModuleEfficiency>(gun)
            .expect("AI gun never got a ModuleEfficiency");
        assert!(eff.value > 0.0,
            "AI gun efficiency {} - ai_weapon_fire_system skips at <= 0", eff.value);
    }

    /// Propulsion outranks everything but power, so on the starter's five-engine
    /// bank an eight-hand crew put seven people on reactors and nozzles and left
    /// the guns dark. One operator runs the bank now; the rest of the ship gets
    /// the hands back.
    #[test]
    fn one_operator_runs_the_whole_engine_bank() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AutoAssignTimer>();
        app.add_systems(Update, (auto_assign_crew, compute_module_efficiency).chain());

        let ship = app.world_mut().spawn(Ship).id();
        let engines: Vec<Entity> = (0..3)
            .map(|i| {
                app.world_mut()
                    .spawn(post(ModuleType::StandardEngine, IVec2::new(-i, 0), 9))
                    .insert(ChildOf(ship))
                    .id()
            })
            .collect();
        let gun = app
            .world_mut()
            .spawn(post(ModuleType::Gatling, IVec2::new(3, 0), 6))
            .insert(ChildOf(ship))
            .id();
        for name in ["Chen", "Okafor"] {
            app.world_mut().spawn(hand(name)).insert(ChildOf(ship));
        }

        // Drive past the 2s auto-assign tick. Steps stay under 250ms because
        // Time<Virtual> clamps anything larger, so one big jump would be
        // silently shortened and the timer would never fire.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(200)));
        for _ in 0..14 {
            app.update();
        }

        let manned = engines
            .iter()
            .filter(|e| {
                app.world().get::<CrewStation>(**e).unwrap().assigned_crew.is_some()
            })
            .count();
        let gun_manned = app.world().get::<CrewStation>(gun).unwrap().assigned_crew.is_some();
        assert_eq!(
            manned, 1,
            "three nozzles is one engineer's round; took {manned} hands (gun manned: {gun_manned})"
        );

        // ...and the whole bank still runs on that one operator.
        for engine in &engines {
            let eff = app.world().get::<ModuleEfficiency>(*engine).unwrap();
            assert_eq!(
                eff.staffing_factor, 1.0,
                "an engine went dark despite the room being manned"
            );
        }

        // The hand that would have been a second nozzle-minder is on the gun.
        assert!(
            app.world().get::<CrewStation>(gun).unwrap().assigned_crew.is_some(),
            "freed crew never reached the weapon"
        );
    }

    /// A hand covers three nozzles, not the whole ship however big it gets.
    /// Four engines is one too many for one engineer, so the room wants two.
    #[test]
    fn a_bank_bigger_than_one_round_costs_a_second_engineer() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AutoAssignTimer>();
        app.add_systems(Update, (auto_assign_crew, compute_module_efficiency).chain());

        let ship = app.world_mut().spawn(Ship).id();
        let engines: Vec<Entity> = (0..4)
            .map(|i| {
                app.world_mut()
                    .spawn(post(ModuleType::StandardEngine, IVec2::new(-i, 0), 9))
                    .insert(ChildOf(ship))
                    .id()
            })
            .collect();
        for name in ["Chen", "Okafor", "Rivera"] {
            app.world_mut().spawn(hand(name)).insert(ChildOf(ship));
        }

        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(200)));
        for _ in 0..14 {
            app.update();
        }

        let manned = engines
            .iter()
            .filter(|e| app.world().get::<CrewStation>(**e).unwrap().assigned_crew.is_some())
            .count();
        assert_eq!(manned, 2, "four nozzles need two engineers, not {manned}");
        for engine in &engines {
            let eff = app.world().get::<ModuleEfficiency>(*engine).unwrap();
            assert_eq!(eff.staffing_factor, 1.0, "an engine went dark with the room fully crewed");
        }
    }

    /// A standing order has to beat priority, or it isn't an order. The
    /// reactor outranks the gun 10 to 6, so an unconstrained hand always takes
    /// it; a gunner must be passed over for it and still be there for the gun.
    #[test]
    fn a_standing_order_outranks_station_priority() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AutoAssignTimer>();
        app.add_systems(Update, auto_assign_crew);

        let ship = app.world_mut().spawn(Ship).id();
        let reactor = app
            .world_mut()
            .spawn(post(ModuleType::StandardReactor, IVec2::new(0, 0), 10))
            .insert(ChildOf(ship))
            .id();
        let gun = app
            .world_mut()
            .spawn(post(ModuleType::Gatling, IVec2::new(2, 0), 6))
            .insert(ChildOf(ship))
            .id();
        app.world_mut()
            .spawn((hand("Rivera"), CrewDuty::Guns))
            .insert(ChildOf(ship));

        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(200)));
        for _ in 0..14 {
            app.update();
        }

        assert!(
            app.world().get::<CrewStation>(gun).unwrap().assigned_crew.is_some(),
            "a gunner never reached the gun"
        );
        assert!(
            app.world().get::<CrewStation>(reactor).unwrap().assigned_crew.is_none(),
            "a gunner was drafted onto the reactor despite their orders"
        );
    }

    /// Damage control means damage control: they hold no post at all, which is
    /// how you commit someone to repairs on a ship with more posts than crew.
    #[test]
    fn damage_control_crew_take_no_post() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AutoAssignTimer>();
        app.add_systems(Update, auto_assign_crew);

        let ship = app.world_mut().spawn(Ship).id();
        let reactor = app
            .world_mut()
            .spawn(post(ModuleType::StandardReactor, IVec2::new(0, 0), 10))
            .insert(ChildOf(ship))
            .id();
        app.world_mut()
            .spawn((hand("Okafor"), CrewDuty::DamageControl))
            .insert(ChildOf(ship));

        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(200)));
        for _ in 0..14 {
            app.update();
        }

        assert!(
            app.world().get::<CrewStation>(reactor).unwrap().assigned_crew.is_none(),
            "a damage-control hand was drafted onto a post"
        );
    }

    /// Losing the engineer must still cost you the whole bank — otherwise
    /// sharing the post has quietly removed the reason crew matter.
    #[test]
    fn an_unmanned_engine_room_stops_every_engine() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AutoAssignTimer>();
        app.add_systems(Update, compute_module_efficiency);

        let ship = app.world_mut().spawn(Ship).id();
        let engines: Vec<Entity> = (0..3)
            .map(|i| {
                app.world_mut()
                    .spawn(post(ModuleType::StandardEngine, IVec2::new(-i, 0), 9))
                    .insert(ChildOf(ship))
                    .id()
            })
            .collect();

        app.update();
        app.update();

        for engine in &engines {
            let eff = app.world().get::<ModuleEfficiency>(*engine).unwrap();
            assert_eq!(eff.staffing_factor, 0.0, "an unmanned engine room still produced thrust");
        }
    }
}

#[cfg(test)]
mod death_tests {
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

    fn test_app() -> App {
        let mut app = App::new();
        app.add_message::<CrewDamaged>();
        app.add_message::<CrewDied>();
        app.add_systems(Update, report_crew_deaths);
        app
    }

    fn deaths(app: &mut App) -> Vec<(Entity, String, String)> {
        let messages = app.world().resource::<Messages<CrewDied>>();
        let mut cursor = messages.get_cursor();
        cursor
            .read(messages)
            .map(|d| (d.crew, d.name.clone(), d.cause.describe().to_string()))
            .collect()
    }

    /// The bug this exists to stop: two of the three things that can kill a
    /// crew member never raised `CrewDied`, so the player lost a hand and was
    /// never told. Death is now decided by the body, not by the weapon.
    #[test]
    fn a_crew_member_at_zero_health_is_reported_dead() {
        let mut app = test_app();
        let crew = app.world_mut().spawn(hand("Okonkwo", 0.0)).id();

        app.update();

        let reported = deaths(&mut app);
        assert_eq!(reported.len(), 1, "nobody was reported dead: {reported:?}");
        assert_eq!(reported[0].0, crew);
        assert_eq!(reported[0].1, "Okonkwo");
    }

    /// The killing blow and the frame the body is noticed are not reliably the
    /// same one, so the cause has to outlive the frame its event was written in.
    #[test]
    fn a_death_names_what_caused_it_even_a_frame_later() {
        let mut app = test_app();
        let crew = app.world_mut().spawn(hand("Sowande", 40.0)).id();

        app.world_mut().write_message(CrewDamaged {
            crew,
            amount: 60.0,
            source: CrewDamageSource::Boarders,
        });
        app.update();
        assert!(deaths(&mut app).is_empty(), "reported dead while still standing");

        // The wound lands; the sweep notices on a later frame.
        app.world_mut().get_mut::<CrewMember>(crew).unwrap().health = 0.0;
        app.update();

        let reported = deaths(&mut app);
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].2, "parasites", "the cause was forgotten between frames");
    }

    /// Casualties on somebody else's ship are not the player's losses.
    /// `handle_crew_death` increments `Statistics::crew_lost` for every event
    /// it reads, so letting enemy crew through here would report a loss every
    /// time the player gutted an enemy engine room.
    #[test]
    fn enemy_crew_are_not_the_players_dead() {
        let mut app = test_app();
        app.world_mut().spawn((hand("Vance", 0.0), OwnedByAiShip { root: Entity::PLACEHOLDER }));

        app.update();

        assert!(deaths(&mut app).is_empty(), "an enemy casualty was counted as ours");
    }
}

#[cfg(test)]
mod room_location_tests {
    use super::*;
    use crate::building::grid_to_local;
    use crate::crew::walking::CREW_Z;

    /// Room ids are ship-LOCAL, and so is everything that produces them. A
    /// crew member's cell has to be read off their local transform, not their
    /// world position — otherwise the answer is right only while the ship sits
    /// exactly on the origin, and wrong by the ship's whole displacement the
    /// moment it moves. Nothing downstream can tell the difference: the room
    /// simply stops matching and the MedBay quietly treats nobody.
    #[test]
    fn a_crew_members_room_is_read_in_ship_space_not_world_space() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin));
        app.init_resource::<RoomMap>();
        app.add_systems(Update, update_crew_room_location);

        // A ship a long way from home, which is the normal case.
        let ship = app
            .world_mut()
            .spawn((Ship, Transform::from_xyz(48_000.0, 31_500.0, 0.0)))
            .id();

        let cell = IVec2::new(2, 3);
        let crew = app
            .world_mut()
            .spawn((
                CrewMember {
                    name: "Adeyemi".into(),
                    health: 100.0,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Idle,
                },
                Transform::from_translation(grid_to_local(cell).extend(CREW_Z)),
            ))
            .insert(ChildOf(ship))
            .id();

        app.update();

        let location = app.world().get::<CrewRoomLocation>(crew).unwrap();
        assert_eq!(
            location.grid_position, cell,
            "crew cell was measured against the world, not the ship"
        );
    }
}

#[cfg(test)]
mod autonomy_tests {
    use super::*;

    /// No cores, no self-operation. An unstaffed post must still produce
    /// nothing on a ship with nothing to think with — that is the baseline the
    /// whole staffing layer rests on.
    #[test]
    fn no_cores_means_no_autonomy() {
        assert_eq!(autonomy_from_cores(&[]), 0.0);
    }

    /// Each core is worth less than the one before, and they never add up to a
    /// crew. If this cap ever reached 1.0, crew would stop mattering and an
    /// entire system of the game would go quiet without anything failing.
    #[test]
    fn cores_diminish_and_never_replace_a_crew() {
        let one = autonomy_from_cores(&[0.16]);
        let two = autonomy_from_cores(&[0.16, 0.16]);
        let three = autonomy_from_cores(&[0.16, 0.16, 0.16]);

        assert!(one > 0.0);
        assert!(two > one && three > two, "more cores should do more");
        assert!(two - one < one, "the second core must be worth less than the first");
        assert!(three - two < two - one, "returns must keep diminishing");

        let many = autonomy_from_cores(&[0.16; 50]);
        assert!(many <= MAX_AUTONOMY, "autonomy ran past its cap: {many}");
        assert!(MAX_AUTONOMY < 1.0, "a ship must never fully run itself");
    }

    /// Order must not change the result, or the same ship reports different
    /// autonomy depending on the order its blocks happened to be queried in.
    #[test]
    fn order_does_not_matter() {
        let a = autonomy_from_cores(&[0.16, 0.30, 0.05]);
        let b = autonomy_from_cores(&[0.05, 0.16, 0.30]);
        assert!((a - b).abs() < 1e-6, "{a} vs {b}");
    }
}

#[cfg(test)]
mod dread_tests {
    use super::*;

    fn floor_at(level: f32) -> f32 {
        DREAD_FLOOR_NEAR + (DREAD_FLOOR_FAR - DREAD_FLOOR_NEAR) * level.clamp(0.0, 1.0)
    }

    /// Early on the floor must be exactly what it always was. The old value
    /// was load-bearing: below it, deep-zone crews locked into permanent panic
    /// and nobody could man a station where the wrecks are.
    #[test]
    fn a_fresh_run_keeps_the_old_floor() {
        assert_eq!(floor_at(0.0), 35.0);
        assert!(floor_at(0.0) > 30.0, "must stay above the panic-recovery threshold");
    }

    /// At the far edge, distance alone finally can break someone — otherwise
    /// the dread is decorative.
    #[test]
    fn the_far_edge_can_actually_break_a_crew() {
        assert!(floor_at(1.0) < 20.0, "should dip under the panic threshold");
    }

    /// But never to nothing. A Rec Room hard-floors morale at 30, so the
    /// player always has an answer; what must not exist is a floor so low that
    /// no amount of building helps.
    #[test]
    fn it_never_bottoms_out() {
        assert!(floor_at(1.0) > 0.0);
        assert!(floor_at(1.0) < floor_at(0.0), "it has to actually get worse");
        for l in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert!(floor_at(l) >= DREAD_FLOOR_FAR, "level {l} went under the cap");
        }
    }
}

#[cfg(test)]
mod facility_scope_tests {
    use super::*;
    use crate::ai_ship::components::OwnedByAiShip;

    fn rec_room() -> (CrewFacility, Module) {
        (
            CrewFacility { facility_type: FacilityType::RecRoom },
            Module {
                module_type: ModuleType::RecRoom,
                health: 100.0,
                max_health: 100.0,
                power_consumption: 0.0,
                power_generation: 0.0,
                is_active: true,
                grid_position: IVec2::ZERO,
                size: IVec2::ONE,
                rotation: Rotation::default(),
            },
        )
    }

    fn low(name: &str) -> CrewMember {
        CrewMember {
            name: name.into(),
            health: 100.0,
            max_health: 100.0,
            oxygen: 100.0,
            morale: 10.0,
            state: CrewState::Idle,
        }
    }

    fn morale(app: &App, e: Entity) -> f32 {
        app.world().get::<CrewMember>(e).unwrap().morale
    }

    /// Each ship's Rec Room floors its own crew's morale and nobody else's.
    #[test]
    fn a_rec_room_serves_its_own_ship() {
        let mut app = App::new();
        app.add_systems(Update, recroom_morale_floor);
        let ours = app.world_mut().spawn(Ship).id();
        let raider = app.world_mut().spawn_empty().id();
        let bare = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((rec_room(), ChildOf(raider), OwnedByAiShip { root: raider }));

        let mine = app.world_mut().spawn((low("Ash"), ChildOf(ours))).id();
        let theirs = app.world_mut().spawn((low("Vex"), ChildOf(raider), OwnedByAiShip { root: raider })).id();
        let other = app.world_mut().spawn((low("Kade"), ChildOf(bare), OwnedByAiShip { root: bare })).id();
        app.update();
        assert_eq!(morale(&app, mine), 10.0, "an enemy's rec room lifted our crew");
        assert_eq!(morale(&app, theirs), 30.0, "their own rec room didn't serve them");

        app.world_mut().spawn((rec_room(), ChildOf(ours)));
        app.update();
        assert_eq!(morale(&app, mine), 30.0);
        assert_eq!(morale(&app, other), 10.0, "our rec room lifted a third ship's crew");
    }
}
