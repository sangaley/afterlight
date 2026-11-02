//! Pressure suits, and the lock you fetch them from.
//!
//! The game has always said its crew are suited: `eva_salvage` sends a detail
//! onto a wreck's hull with no preparation at all ("the suit keeps them alive
//! out there"), and the tutorial tells the player "idle crew suit up, cross
//! over". That was fiction with nothing behind it. This makes it literal and
//! puts it behind a block, so the same rule covers a breach and a salvage run
//! instead of two contradictory ones.
//!
//! The suits hang in the `AirlockChamber`, which `crew::burial` already uses
//! as the lock the dead go out of. One block, both jobs.
//!
//! `CrewMember::oxygen` carries suit air. It has been a dead field since
//! personal oxygen was removed on 2026-07-15 -- written as 100.0 at every
//! spawn site, read by nothing, and still serialised into every save. Reusing
//! it costs no new field and no save migration.

use bevy::prelude::*;

use crate::ai_ship::components::OwnedByAiShip;
use crate::building::{footprints, local_to_grid, ShipGrid};
use crate::components::*;
use crate::crew::eva_salvage::EvaSalvaging;
use crate::crew::navigation::NavGrid;
use crate::crew::walking::CrewDestination;
use crate::events::*;
use crate::ship::air::AirField;

/// Pressure below which the air will not keep a person alive. Sits under
/// fire's 0.3 ignition floor: by the time a compartment will no longer carry a
/// flame it is already no good for lungs.
pub const VACUUM_AT: f32 = 0.25;

/// Seconds of vacuum a full suit covers. Long enough to cross the ship and
/// seal the hole, short enough that a suit is not a solution to the breach.
const SUIT_AIR_SECONDS: f32 = 90.0;

/// Seconds to top a suit back up once they are in air again.
const SUIT_REFILL_SECONDS: f32 = 20.0;

/// Health per second lost to vacuum with no suit on. Around eight seconds from
/// sound to dead -- time to see it happening and not much else.
const SUFFOCATION_DPS: f32 = 12.0;

/// How close counts as standing at the locker.
const REACH: f32 = 12.0;

/// Cadence for the suit-up planner. Matches the other errand planners rather
/// than running every frame: A* across the ship is not a per-frame cost.
#[derive(Resource)]
pub struct SuitErrandTimer(Timer);

impl Default for SuitErrandTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.5, TimerMode::Repeating))
    }
}

/// Standable cells of the working airlocks, and the suits racked across them.
fn lockers(nav: &NavGrid, airlocks: impl Iterator<Item = (Module, u32)>) -> Option<(Vec<IVec2>, u32)> {
    let (mut at, mut racked) = (Vec::new(), 0);
    for (module, suits) in airlocks {
        if !module.is_active || module.health <= 0.0 {
            continue;
        }
        let footprint = footprints::footprint_override(module.module_type);
        let cells =
            ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint);
        if let Some(cell) = cells.first().and_then(|c| nav.nearest_passable(*c)) {
            at.push(cell);
            racked += suits;
        }
    }
    (!at.is_empty()).then_some((at, racked))
}

/// The locker nearest someone standing at `from`.
fn nearest_locker(at: &[IVec2], from: Vec2) -> IVec2 {
    at.iter()
        .copied()
        .min_by(|a, b| crate::building::grid_to_local(*a).distance(from).total_cmp(&crate::building::grid_to_local(*b).distance(from)))
        .unwrap_or(IVec2::ZERO)
}

/// Sends unsuited hands to the locker while the hull is open.
///
/// Deliberately triggered by the ship being holed rather than by anyone
/// actually standing in vacuum: by the time a compartment is thin enough to
/// hurt, the walk to the locker is through the thing you needed the suit for.
pub fn plan_suit_errands(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<SuitErrandTimer>,
    air: Res<AirField>,
    ships: Query<(Entity, &NavGrid), (With<Ship>, Without<OwnedByAiShip>)>,
    airlocks: Query<(&Module, &AirlockComp, &ChildOf), Without<DestroyedModule>>,
    crew: Query<
        (Entity, &ChildOf, &Transform, Has<Suited>, Has<SuitingUp>),
        (With<CrewMember>, Without<EvaSalvaging>, Without<OwnedByAiShip>),
    >,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }
    let Ok((ship, nav)) = ships.single() else { return };

    let breached = !air.vents.is_empty();
    let Some((at, racked)) = lockers(
        nav,
        airlocks
            .iter()
            .filter(|(_, _, parent)| parent.parent() == ship)
            .map(|(module, rack, _)| (module.clone(), rack.suits)),
    ) else {
        // No lock, no suits. Anyone already told to fetch one is stood down
        // rather than left walking to a place that no longer exists.
        for (entity, _, _, _, going) in crew.iter() {
            if going {
                commands.entity(entity).try_remove::<SuitingUp>();
            }
        }
        return;
    };

    // Only as many go as there are suits left: the whole crew leaving its posts
    // for a rack of a dozen is how a holed ship stopped firing.
    let spoken_for = crew.iter().filter(|(_, parent, _, suited, going)| parent.parent() == ship && (*suited || *going)).count() as u32;
    let mut free = racked.saturating_sub(spoken_for);
    for (entity, parent, transform, suited, going) in crew.iter() {
        if parent.parent() != ship {
            continue;
        }
        if !breached || suited {
            if going {
                commands.entity(entity).try_remove::<SuitingUp>();
            }
            continue;
        }
        if !going && free > 0 {
            free -= 1;
            commands
                .entity(entity)
                .try_insert((SuitingUp, CrewDestination(nearest_locker(&at, transform.translation.truncate()))));
        }
    }
}

/// Hands out a suit to anyone who has reached the locker, while they last.
pub fn issue_suits(
    mut commands: Commands,
    ships: Query<(Entity, &NavGrid), (With<Ship>, Without<OwnedByAiShip>)>,
    airlocks: Query<(&Module, &AirlockComp, &ChildOf), Without<DestroyedModule>>,
    suited: Query<(), (With<Suited>, Without<OwnedByAiShip>)>,
    mut waiting: Query<
        (Entity, &Transform, &mut CrewMember),
        (With<SuitingUp>, Without<OwnedByAiShip>),
    >,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let Ok((ship, nav)) = ships.single() else { return };
    let Some((at, racked)) = lockers(
        nav,
        airlocks
            .iter()
            .filter(|(_, _, parent)| parent.parent() == ship)
            .map(|(module, rack, _)| (module.clone(), rack.suits)),
    ) else {
        return;
    };

    let mut worn = suited.iter().count() as u32;

    for (entity, transform, mut member) in waiting.iter_mut() {
        let here = transform.translation.truncate();
        if crate::building::grid_to_local(nearest_locker(&at, here)).distance(here) > REACH {
            continue;
        }
        if worn >= racked {
            // They made it and the rack was empty. A breach with more crew
            // than suits is a choice about who gets one.
            continue;
        }
        worn += 1;
        member.oxygen = 100.0;
        commands
            .entity(entity)
            .try_remove::<SuitingUp>()
            .try_insert(Suited);
        notifications.write(ShowNotification {
            message: format!("{} is suited.", member.name),
            notification_type: NotificationType::Info,
            duration: 1.5,
        });
    }
}

/// Seconds a suited hand spends in good air, suit topped up, before they hang
/// it back up.
const SUIT_STOW_SECONDS: f32 = 15.0;

/// Burns suit air in vacuum, refills it in atmosphere, and suffocates whoever
/// is caught without.
///
/// This is the first thing since 2026-07-15 that can kill a crew member with
/// air alone. Until now a breach could only take someone by throwing them out
/// of the hull; a compartment that merely emptied was survivable indefinitely,
/// which is why sealing one had no weight.
pub fn suit_air(
    mut commands: Commands,
    time: Res<Time>,
    air: Res<AirField>,
    mut crew: Query<
        (Entity, &Transform, &mut CrewMember, Has<Suited>),
        (Without<EvaSalvaging>, Without<OwnedByAiShip>),
    >,
    mut wounds: MessageWriter<CrewDamaged>,
    // Seconds each suited hand has been in good air with a full suit.
    mut settled: Local<std::collections::HashMap<Entity, f32>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for (entity, transform, mut member, suited) in crew.iter_mut() {
        if member.health <= 0.0 {
            continue;
        }
        let cell = local_to_grid(transform.translation.truncate());
        // A cell with no entry is somewhere the air model does not describe --
        // treat it as breathable rather than drowning people in the gaps.
        let pressure = air.pressure.get(&cell).copied().unwrap_or(1.0);
        let in_vacuum = pressure < VACUUM_AT;

        if !in_vacuum {
            if suited && member.oxygen < 100.0 {
                member.oxygen = (member.oxygen + (100.0 / SUIT_REFILL_SECONDS) * dt).min(100.0);
            }
            // Once the danger has passed, the suit goes back on the rack.
            // Nothing took one off short of running it dry, so suits issued
            // for a breach or an EVA stayed on their wearers for good and the
            // airlock emptied for the rest of the run.
            if suited && member.oxygen >= 100.0 {
                let held = settled.entry(entity).or_insert(0.0);
                *held += dt;
                if *held >= SUIT_STOW_SECONDS {
                    commands.entity(entity).try_remove::<Suited>();
                    settled.remove(&entity);
                }
            } else {
                settled.remove(&entity);
            }
            continue;
        }
        settled.remove(&entity);

        if suited && member.oxygen > 0.0 {
            member.oxygen = (member.oxygen - (100.0 / SUIT_AIR_SECONDS) * dt).max(0.0);
            if member.oxygen > 0.0 {
                continue;
            }
            // An empty suit is not the same as no suit -- take it off so the
            // planner sends them for a fresh one if they reach air again.
            commands.entity(entity).try_remove::<Suited>();
        }

        let hurt = SUFFOCATION_DPS * dt;
        member.health = (member.health - hurt).max(0.0);
        wounds.write(CrewDamaged {
            crew: entity,
            amount: hurt,
            source: CrewDamageSource::Suffocation,
        });
    }
}

#[cfg(test)]
mod suit_tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<AirField>();
        app.add_message::<CrewDamaged>();
        app.add_systems(Update, suit_air);
        app
    }

    /// Pressure at the one cell everybody in these tests stands on.
    fn set_air(app: &mut App, pressure: f32) {
        app.world_mut()
            .resource_mut::<AirField>()
            .pressure
            .insert(IVec2::new(0, 0), pressure);
    }

    fn spawn(app: &mut App, suited: bool, oxygen: f32) -> Entity {
        let mut e = app.world_mut().spawn((
            CrewMember {
                name: "Okonkwo".into(),
                health: 100.0,
                max_health: 100.0,
                oxygen,
                morale: 100.0,
                state: CrewState::Idle,
            },
            Transform::from_translation(crate::building::grid_to_local(IVec2::ZERO).extend(0.6)),
        ));
        if suited {
            e.insert(Suited);
        }
        e.id()
    }

    fn run(app: &mut App, seconds: f32, steps: u32) {
        for _ in 0..steps {
            let d = std::time::Duration::from_secs_f32(seconds / steps as f32);
            app.world_mut().resource_mut::<Time>().advance_by(d);
            app.update();
        }
    }

    fn health(app: &App, e: Entity) -> f32 {
        app.world().get::<CrewMember>(e).unwrap().health
    }
    fn air_left(app: &App, e: Entity) -> f32 {
        app.world().get::<CrewMember>(e).unwrap().oxygen
    }

    /// The point of the whole stage: an emptied compartment can now kill you.
    /// Before this, a breach could only take someone by throwing them out of
    /// the hull, so sealing a compartment had nothing riding on it.
    #[test]
    fn vacuum_kills_an_unsuited_crew_member() {
        let mut app = app();
        set_air(&mut app, 0.0);
        let hand = spawn(&mut app, false, 100.0);
        run(&mut app, 4.0, 40);

        assert!(
            health(&app, hand) < 60.0,
            "unsuited in hard vacuum for 4s and barely hurt: {}",
            health(&app, hand)
        );
    }

    /// And a suit is what makes it survivable, which is what the locker is for.
    #[test]
    fn a_suit_keeps_them_alive() {
        let mut app = app();
        set_air(&mut app, 0.0);
        let hand = spawn(&mut app, true, 100.0);
        run(&mut app, 4.0, 40);

        assert_eq!(health(&app, hand), 100.0, "a suited hand took vacuum damage");
        assert!(air_left(&app, hand) < 100.0, "the suit burned no air");
    }

    /// A suit is a reprieve, not immunity -- it has to run out, or there is no
    /// urgency to sealing the hull.
    #[test]
    fn an_empty_suit_stops_helping() {
        let mut app = app();
        set_air(&mut app, 0.0);
        let hand = spawn(&mut app, true, 2.0); // nearly out
        run(&mut app, 6.0, 60);

        assert!(
            app.world().get::<Suited>(hand).is_none(),
            "an empty suit is still counted as a suit"
        );
        assert!(
            health(&app, hand) < 100.0,
            "the suit emptied and they still took no harm"
        );
    }

    /// Back in atmosphere it refills, so one breach does not permanently spend
    /// every suit on the ship.
    #[test]
    fn atmosphere_refills_the_suit() {
        let mut app = app();
        set_air(&mut app, 1.0);
        let hand = spawn(&mut app, true, 10.0);
        run(&mut app, 5.0, 50);

        assert!(
            air_left(&app, hand) > 25.0,
            "suit did not refill in breathable air: {}",
            air_left(&app, hand)
        );
        assert_eq!(health(&app, hand), 100.0, "hurt while standing in good air");
    }

    /// The thing plan_repair_errands would not do: a crew member who belongs
    /// to a station still goes to the hole once they have a suit on.
    ///
    /// Seventeen of the starter ship's twenty hands are posted, so leaving
    /// breach work to the ordinary errand planner left three people to fix a
    /// hull full of holes -- and none at all while anything was shooting.
    #[test]
    fn a_suited_hand_leaves_their_post_for_the_hole() {
        use crate::crew::navigation::NavCell;

        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, suited_damage_control);

        let mut cells = bevy::platform::collections::HashMap::new();
        for x in 0..4 {
            cells.insert(IVec2::new(x, 0), NavCell::Hallway);
        }
        let ship = app.world_mut().spawn((Ship, NavGrid { cells, version: 1 })).id();

        // A holed plate just past the end of the corridor.
        app.world_mut().spawn((
            HullSegment {
                health: 0.0,
                max_health: 100.0,
                radiation_shielding: 0.0,
                is_depressurized: true,
                depressurization_level: 1.0,
                hull_layer: HullLayer::Inner,
                material: HullMaterial::Steel,
                grid_position: IVec2::new(4, 0),
            },
            ChildOf(ship),
        ));

        // Suited, and manning a post at the far end.
        let hand = app
            .world_mut()
            .spawn((
                CrewMember {
                    name: "Adeyemi".into(),
                    health: 100.0,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Working,
                },
                Transform::from_translation(
                    crate::building::grid_to_local(IVec2::ZERO).extend(0.6),
                ),
                Suited,
                ChildOf(ship),
            ))
            .id();

        app.update();

        let sent = app.world().get::<CrewDestination>(hand);
        assert!(sent.is_some(), "a suited hand was not sent to the breach at all");
        assert!(
            app.world().get::<DamageControl>(hand).is_some(),
            "sent, but not flagged -- their station will reclaim them next frame"
        );
        assert_eq!(
            sent.unwrap().0,
            IVec2::new(3, 0),
            "should stand beside the hole, not try to occupy it"
        );
        // Walking there is half the job. crew_repair_system skips any state
        // that is not Repairing or Idle, and seal_breach_system counts only
        // Repairing -- so arriving still flagged Working means standing at the
        // hole doing nothing, which is exactly how this first shipped.
        assert_eq!(
            app.world().get::<CrewMember>(hand).unwrap().state,
            CrewState::Repairing,
            "sent to the breach but not set to work on it"
        );
    }

    /// A cell the air model has no entry for is not vacuum. Ships have gaps the
    /// room fill never reaches, and treating those as hard vacuum would quietly
    /// suffocate anyone standing in one.
    #[test]
    fn an_undescribed_cell_is_not_vacuum() {
        let mut app = app();
        let hand = spawn(&mut app, false, 100.0);
        run(&mut app, 4.0, 40);

        assert_eq!(
            health(&app, hand),
            100.0,
            "suffocated on a cell the air field says nothing about"
        );
    }
}

/// Sends suited crew to work the breaches, post or no post.
///
/// `plan_repair_errands` will not do this. It skips anyone a station has
/// claimed, and the starter ship runs seventeen of its twenty hands on posts,
/// so a hull full of holes had three people eligible to fix it. It also stands
/// down entirely while hostiles are near -- reasonable for general repairs,
/// wrong for a hole letting the air out during the fight that made it.
///
/// Putting a suit on is the signal. Nobody suits up to go back to a gun.
pub fn suited_damage_control(
    mut commands: Commands,
    ships: Query<(Entity, &NavGrid), (With<Ship>, Without<OwnedByAiShip>)>,
    hulls: Query<(&HullSegment, &ChildOf), Without<HullDestroyed>>,
    mut crew: Query<
        (Entity, &Transform, &mut CrewMember, &ChildOf, Has<DamageControl>),
        (With<Suited>, Without<EvaSalvaging>, Without<OwnedByAiShip>),
    >,
) {
    let Ok((ship, nav)) = ships.single() else { return };

    // Open holes only. A merely dented plate is what the ordinary errand
    // planner is for; this is the job worth pulling someone off a gun.
    let breaches: Vec<IVec2> = hulls
        .iter()
        .filter(|(hull, parent)| {
            parent.parent() == ship && hull.is_depressurized && hull.depressurization_level > 0.0
        })
        .map(|(hull, _)| hull.grid_position)
        .collect();

    if breaches.is_empty() {
        for (entity, _, mut member, _, working) in crew.iter_mut() {
            if working {
                commands.entity(entity).try_remove::<DamageControl>();
                member.state = CrewState::Idle;
            }
        }
        return;
    }

    // One hole per hand, nearest first, so they spread out instead of all
    // walking to the same one.
    let mut claimed: std::collections::HashSet<IVec2> = std::collections::HashSet::new();
    for (entity, transform, mut member, parent, _) in crew.iter_mut() {
        if parent.parent() != ship {
            continue;
        }
        let here = local_to_grid(transform.translation.truncate());
        let best = breaches
            .iter()
            .filter(|cell| !claimed.contains(*cell))
            .filter_map(|cell| {
                // Stand beside the hole, not in it.
                nav.nearest_passable(*cell).map(|stand| {
                    let d = (here - stand).abs();
                    (d.x + d.y, *cell, stand)
                })
            })
            .min_by_key(|(dist, _, _)| *dist);

        let Some((_, hole, stand)) = best else { continue };
        claimed.insert(hole);
        commands
            .entity(entity)
            .try_insert((DamageControl, CrewDestination(stand)));

        // The state IS the work. crew_repair_system credits Repairing at 1.0
        // and Idle at 0.5 and skips everything else outright, and
        // seal_breach_system only counts Repairing -- so a hand pulled off a
        // post arrived at the hole still flagged Working and stood there
        // contributing precisely nothing.
        member.state = CrewState::Repairing;
    }
}

#[cfg(test)]
mod stow_tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<AirField>();
        app.add_message::<CrewDamaged>();
        app.add_systems(Update, suit_air);
        app.world_mut().resource_mut::<AirField>().pressure.insert(IVec2::ZERO, 1.0);
        app
    }

    fn suited_hand(app: &mut App, oxygen: f32) -> Entity {
        app.world_mut().spawn((
            CrewMember {
                name: "Okonkwo".into(),
                health: 100.0,
                max_health: 100.0,
                oxygen,
                morale: 100.0,
                state: CrewState::Idle,
            },
            Transform::from_translation(crate::building::grid_to_local(IVec2::ZERO).extend(0.6)),
            Suited,
        )).id()
    }

    fn run(app: &mut App, seconds: f32) {
        for _ in 0..(seconds * 10.0) as u32 {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
            app.update();
        }
    }

    /// Safe and topped up: the suit goes back on the rack.
    #[test]
    fn a_suit_is_stowed_once_the_danger_has_passed() {
        let mut app = app();
        let hand = suited_hand(&mut app, 100.0);
        run(&mut app, SUIT_STOW_SECONDS + 1.0);
        assert!(app.world().get::<Suited>(hand).is_none(), "suit never came off");
    }

    /// Not before it's been refilled and they've been safe a while.
    #[test]
    fn a_suit_stays_on_while_it_refills() {
        let mut app = app();
        let hand = suited_hand(&mut app, 20.0);
        run(&mut app, 5.0);
        assert!(app.world().get::<Suited>(hand).is_some());
    }
}
