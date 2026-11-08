//! Crew walking about inside the ship.
//!
//! Split the way the AI ships are: `plan_crew_destinations` and
//! `plan_crew_paths` think on a timer, `walk_crew` moves every frame. A* over
//! a whole ship is far too expensive to redo per frame, and stepping has to be
//! smooth, so the two cannot share a cadence.
//!
//! Crew walk in ship-LOCAL space as children of the ship, so the hull can spin
//! and manoeuvre underneath them without any of it reaching this file.
//!
//! Deliberately does NOT touch `CrewMember::state`. `CrewState::Moving` exists
//! and is unused, but `crew_repair_system` and `crew_rebuild_system` both count
//! `Idle` crew as work capacity, so flipping walkers to `Moving` would quietly
//! slow repair and reconstruction. Whether walking should cost you that is a
//! real design question, and it gets answered when walking gains teeth — not
//! as a side effect of making crew visible. Until then the presence of
//! `CrewPath` is what "this person is walking" means.

use bevy::prelude::*;

use crate::ai_ship::components::OwnedByAiShip;
use crate::building::{footprints, grid_to_local, local_to_grid, ShipGrid};
use crate::components::*;
use crate::crew::eva_salvage::EvaSalvaging;
use crate::crew::navigation::{find_path, NavCell, NavGrid};

/// World units per second. A cell is 66 units, so this is a touch under a
/// cell a second — brisk enough not to look broken, slow enough that crossing
/// the ship is visibly a journey.
const CREW_WALK_SPEED: f32 = 50.0;

/// How close counts as standing on the waypoint.
const ARRIVE: f32 = 3.0;

/// Seconds between planning passes.
const PLAN_INTERVAL: f32 = 0.5;

/// Where this crew member is trying to get to, in ship-local cells.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrewDestination(pub IVec2);

/// The route there. Inserted when planned, removed on arrival — same idiom as
/// `MigrationPath`, so "is this person walking" is a component query rather
/// than another flag on `CrewMember`.
#[derive(Component)]
pub struct CrewPath {
    pub cells: Vec<IVec2>,
    /// Index of the cell currently being walked toward.
    pub index: usize,
    /// `NavGrid::version` this route was planned against. When the ship
    /// changes shape the route is replanned rather than trusted.
    pub nav_version: u32,
}

impl CrewPath {
    fn target(&self) -> Option<IVec2> {
        self.cells.get(self.index).copied()
    }

    fn destination(&self) -> Option<IVec2> {
        self.cells.last().copied()
    }
}

/// An engineer's rounds. One hand covers several nozzles by walking between
/// them, so the post is a circuit rather than a chair — standing motionless at
/// one engine while three others are "covered" reads as a bug, whatever the
/// efficiency numbers say.
#[derive(Component)]
pub struct CrewPatrol {
    pub stops: Vec<IVec2>,
    pub index: usize,
    /// Time spent working at a stop before moving to the next.
    pub dwell: Timer,
}

/// Seconds an engineer spends at each nozzle before walking to the next.
const PATROL_DWELL: f32 = 6.0;

#[derive(Resource)]
pub struct CrewPlanTimer(Timer);

impl Default for CrewPlanTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(PLAN_INTERVAL, TimerMode::Repeating))
    }
}

/// A crew member's post, as a cell they can actually stand on.
///
/// Multi-cell modules cover several cells and the origin is not guaranteed to
/// be one a person can occupy, so this picks the first passable cell of the
/// footprint rather than assuming.
fn station_cell(nav: &NavGrid, module: &Module) -> Option<IVec2> {
    let footprint = footprints::footprint_override(module.module_type);
    let cells = ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint);
    cells
        .iter()
        .copied()
        .find(|c| nav.passable(*c))
        .or_else(|| cells.first().copied().and_then(|c| nav.nearest_passable(c)))
}

/// Points each crew member at their post.
///
/// Runs after `auto_assign_crew`, reading the assignments it made rather than
/// reaching into it — that system's per-ship bucketing and priority ordering
/// is doing enough already, and destinations are derivable from its output.
///
/// Crew with no post keep whatever destination they had; sending idle hands
/// somewhere in particular is a role behaviour, not a movement one.
pub fn plan_crew_destinations(
    mut commands: Commands,
    stations: Query<(&Module, &CrewStation, &ChildOf), Without<OwnedByAiShip>>,
    crew: Query<
        (Entity, &ChildOf),
        (
            With<CrewMember>,
            Without<EvaSalvaging>,
            Without<OwnedByAiShip>,
            // Someone running for a closing door is not available for duty.
            Without<Fleeing>,
            // A hand on their way to the surgery is not walking back to a gun.
            Without<SeekingTreatment>,
            // Nor is one carrying a body to the lock.
            Without<crate::crew::burial::BurialDetail>,
            // Nor one on their way to the suit locker.
            Without<SuitingUp>,
            // Nor one suited up and working a hole.
            Without<DamageControl>,
        ),
    >,
    existing: Query<&CrewDestination>,
    patrols: Query<&CrewPatrol>,
    navs: Query<&NavGrid>,
) {
    for (module, station, parent) in stations.iter() {
        let Some(assigned) = station.assigned_crew else { continue };
        // The station and its operator must belong to the same ship. Crew
        // queries in this codebase have leaked across ships more than once.
        let Ok((entity, crew_parent)) = crew.get(assigned) else { continue };
        if crew_parent.parent() != parent.parent() {
            continue;
        }
        let Ok(nav) = navs.get(parent.parent()) else { continue };
        let Some(cell) = station_cell(nav, module) else { continue };

        // The engine room is walked, not sat in. Hand the engineer the whole
        // circuit and let `walk_engine_room_rounds` move them along it.
        if module.module_type.category() == ModuleCategory::Propulsion {
            if patrols.get(entity).is_err() {
                let mut stops: Vec<IVec2> = stations
                    .iter()
                    .filter(|(m, _, p)| {
                        p.parent() == parent.parent()
                            && m.module_type.category() == ModuleCategory::Propulsion
                    })
                    .filter_map(|(m, _, _)| station_cell(nav, m))
                    .collect();
                stops.sort_by_key(|c| (c.x, c.y));
                stops.dedup();
                if stops.is_empty() {
                    stops.push(cell);
                }
                let start = stops.iter().position(|c| *c == cell).unwrap_or(0);
                commands.entity(entity).try_insert((
                    CrewPatrol {
                        stops,
                        index: start,
                        dwell: Timer::from_seconds(PATROL_DWELL, TimerMode::Repeating),
                    },
                    CrewDestination(cell),
                ));
            }
            continue;
        }

        // Anyone who stopped being an engineer stops doing rounds.
        if patrols.get(entity).is_ok() {
            commands.entity(entity).try_remove::<CrewPatrol>();
        }
        if existing.get(entity).is_ok_and(|d| d.0 == cell) {
            continue;
        }
        commands.entity(entity).try_insert(CrewDestination(cell));
    }
}

/// Moves engineers along their rounds: work a nozzle for a while, walk to the
/// next, repeat. Only advances once they've actually arrived, so a long walk
/// never gets cut short by the timer.
pub fn walk_engine_room_rounds(
    mut commands: Commands,
    time: Res<Time>,
    mut patrols: Query<
        (Entity, &mut CrewPatrol, Option<&CrewPath>),
        (
            With<CrewMember>,
            Without<EvaSalvaging>,
            Without<OwnedByAiShip>,
            Without<SeekingTreatment>,
        ),
    >,
) {
    for (entity, mut patrol, path) in patrols.iter_mut() {
        if path.is_some() || patrol.stops.len() < 2 {
            continue; // still walking there, or there's only one nozzle
        }
        patrol.dwell.tick(time.delta());
        if !patrol.dwell.just_finished() {
            continue;
        }
        patrol.index = (patrol.index + 1) % patrol.stops.len();
        let next = patrol.stops[patrol.index];
        commands.entity(entity).try_insert(CrewDestination(next));
    }
}

/// Plans a route for anyone whose destination they haven't reached.
///
/// A crew member with no route to their post is left with no `CrewPath` and
/// simply stays put. That is a real answer, not a failure: the section they
/// need is cut off. Making it *matter* comes later; for now they just don't
/// walk through walls to get there.
pub fn plan_crew_paths(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<CrewPlanTimer>,
    navs: Query<&NavGrid>,
    crew: Query<
        (Entity, &Transform, &ChildOf, Option<&CrewDestination>, Option<&CrewPath>),
        (With<CrewMember>, Without<EvaSalvaging>, Without<OwnedByAiShip>),
    >,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    for (entity, transform, parent, destination, path) in crew.iter() {
        let Ok(nav) = navs.get(parent.parent()) else { continue };

        let here = local_to_grid(transform.translation.truncate());

        // The nearest place this person could actually be standing. Equal to
        // `here` while they are on deck, and something else the moment a plate
        // is shot out from under them.
        let footing = nav.nearest_passable(here);

        // Where they are trying to get to.
        //
        // Getting back onto deck outranks the post. A hand left hanging in the
        // gap where their plating used to be has exactly one errand, and for
        // anyone nobody assigned anywhere it is the ONLY errand they will ever
        // be given — `plan_crew_destinations` only speaks to crew manning a
        // station and `plan_repair_errands` stands down in combat, which is
        // precisely when the floor goes away. Without this they stand in open
        // space, motionless, for the rest of the game.
        let goal = match destination.map(|d| d.0) {
            Some(post) if nav.passable(post) => post,
            _ => match footing {
                Some(cell) if cell != here => cell,
                // On solid deck with nowhere to be, or adrift with no deck
                // left anywhere on the ship. Either way, no route to plan.
                _ => {
                    if path.is_some() {
                        commands.entity(entity).try_remove::<CrewPath>();
                    }
                    continue;
                }
            },
        };

        // A route that still ends where we want to go, planned against the
        // ship's current shape, is still good.
        if let Some(path) = path {
            if path.nav_version == nav.version && path.destination() == Some(goal) {
                continue;
            }
        }

        if here == goal {
            commands.entity(entity).try_remove::<CrewPath>();
            continue;
        }

        // Routes start from somewhere a person can stand, which is not
        // necessarily where they are.
        let Some(from) = footing else {
            commands.entity(entity).try_remove::<CrewPath>();
            continue;
        };

        match find_path(nav, from, goal) {
            Some(cells) => {
                commands
                    .entity(entity)
                    .try_insert(CrewPath { cells, index: 0, nav_version: nav.version });
            }
            None => {
                commands.entity(entity).try_remove::<CrewPath>();
            }
        }
    }
}

/// Steps everyone along their route. Per-frame and deliberately dumb — all
/// the thinking happened in the planner.
pub fn walk_crew(
    mut commands: Commands,
    time: Res<Time>,
    mut crew: Query<
        (Entity, &mut Transform, &mut CrewPath, &CrewMember),
        (Without<EvaSalvaging>, Without<OwnedByAiShip>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut path, member) in crew.iter_mut() {
        // The unconscious and the panicking don't report for duty.
        if member.health <= 0.0 || member.state == CrewState::Panicking {
            continue;
        }
        let Some(target_cell) = path.target() else {
            commands.entity(entity).try_remove::<CrewPath>();
            continue;
        };

        let target = grid_to_local(target_cell);
        let pos = transform.translation.truncate();
        let delta = target - pos;
        let distance = delta.length();

        if distance <= ARRIVE {
            transform.translation.x = target.x;
            transform.translation.y = target.y;
            path.index += 1;
            if path.index >= path.cells.len() {
                commands.entity(entity).try_remove::<CrewPath>();
            }
            continue;
        }

        let step = (CREW_WALK_SPEED * dt).min(distance);
        let dir = delta / distance;
        transform.translation.x += dir.x * step;
        transform.translation.y += dir.y * step;

        // Face the direction of travel.
        //
        // Crew art is drawn from straight down, and from that angle a person
        // turning really does rotate - so rotating the sprite is correct
        // rather than a cheat, and it means one sprite set covers every
        // heading instead of four or eight baked directions. (flip_x would be
        // the 3/4-view idiom and is wrong here.) Crew live in ship-local
        // space, so this composes with the hull's own rotation.
        //
        // The art faces +Y, hence the -FRAC_PI_2 to bring atan2's +X origin
        // into line - the same convention weapon sprites use.
        transform.rotation = Quat::from_rotation_z(
            dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2,
        );
    }
}

/// Z for crew sprites. Above modules (0.2, or 0.4 for overhanging barrels)
/// so a crew member crossing machinery is visible, and above the damage
/// overlay's 0.5 so they don't disappear under it when it's toggled on.
pub const CREW_Z: f32 = 0.6;

/// Every cell covered by the ship's crew quarters.
pub fn quarters_cells<'a>(modules: impl Iterator<Item = &'a Module>) -> Vec<IVec2> {
    let mut cells = Vec::new();
    for module in modules {
        let footprint = footprints::footprint_override(module.module_type);
        cells.extend(ShipGrid::cells_for(
            module.grid_position,
            module.size,
            module.rotation,
            footprint,
        ));
    }
    cells.sort_by_key(|c| (c.x, c.y));
    cells
}

/// Ship-local spawn point for the nth hand coming aboard: spread across the
/// bunks rather than stacked in one cell, so a fresh crew reads as a crew.
///
/// A ship with no quarters at all falls back to the origin; the walking
/// planner will pull them onto real deck on its next pass.
pub fn berth_position(quarters: &[IVec2], index: usize) -> Vec3 {
    match quarters.get(index % quarters.len().max(1)) {
        Some(&cell) => grid_to_local(cell).extend(CREW_Z),
        None => Vec3::new(0.0, 0.0, CREW_Z),
    }
}

#[cfg(test)]
mod walking_tests {
    use super::*;
    use crate::crew::navigation::rebuild_nav_grids;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn module_at(module_type: ModuleType, cell: IVec2) -> Module {
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
        }
    }

    fn hull_at(cell: IVec2) -> (HullSegment, Transform) {
        (
            HullSegment { grid_position: cell, hull_layer: HullLayer::Hallway, ..default() },
            Transform::from_translation(grid_to_local(cell).extend(0.1)),
        )
    }

    /// A one-row ship: hallway from (0,0) to (len-1, 0), crew at one end, a
    /// station at the other.
    fn corridor_ship(app: &mut App, len: i32) -> (Entity, Entity, IVec2) {
        let ship = app.world_mut().spawn(Ship).id();
        for x in 0..len {
            let cell = IVec2::new(x, 0);
            app.world_mut().spawn(hull_at(cell)).insert(ChildOf(ship));
        }

        let start = IVec2::new(0, 0);
        let post = IVec2::new(len - 1, 0);
        let crew = app
            .world_mut()
            .spawn((
                CrewMember {
                    name: "Chen".into(),
                    health: 100.0,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Idle,
                },
                Transform::from_translation(grid_to_local(start).extend(CREW_Z)),
            ))
            .insert(ChildOf(ship))
            .id();

        app.world_mut()
            .spawn((
                module_at(ModuleType::Gatling, post),
                CrewStation { priority: 6, assigned_crew: Some(crew), manually_assigned: false },
            ))
            .insert(ChildOf(ship));

        (ship, crew, post)
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(crate::building::layout::LayoutPlugin);
        app.add_plugins(MinimalPlugins);
        app.init_resource::<CrewPlanTimer>();
        app.add_systems(
            Update,
            (rebuild_nav_grids, plan_crew_destinations, plan_crew_paths, walk_crew).chain(),
        );
        app
    }

    /// Advances the clock so the planner's timer fires and the mover gets a
    /// real delta.
    ///
    /// `Time::advance_by` is no good here: `TimePlugin` rewrites `Time<()>`
    /// from the real clock at the top of every frame, so a manual advance is
    /// overwritten before any system sees it. `TimeUpdateStrategy` is the
    /// supported way to drive the clock by hand.
    fn tick(app: &mut App, seconds: f32) {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            seconds,
        )));
        app.update();
    }

    fn crew_cell(app: &App, crew: Entity) -> IVec2 {
        local_to_grid(crew_local(app, crew))
    }

    fn crew_local(app: &App, crew: Entity) -> Vec2 {
        app.world().get::<Transform>(crew).unwrap().translation.truncate()
    }

    /// Runs long enough for a destination to be inserted, the planner's timer
    /// to fire, and the resulting route to become visible. Each of those is a
    /// command insertion, so each needs a frame boundary — in play that lag is
    /// two frames of a walk that takes seconds.
    fn settle(app: &mut App) {
        for _ in 0..4 {
            tick(app, 0.3);
        }
    }

    #[test]
    fn a_crew_member_walks_to_their_post() {
        let mut app = test_app();
        let (_, crew, post) = corridor_ship(&mut app, 6);

        settle(&mut app);
        assert_eq!(
            app.world().get::<CrewDestination>(crew).map(|d| d.0),
            Some(post),
            "the assignment never became a destination"
        );
        assert!(app.world().get::<CrewPath>(crew).is_some(), "no route was planned");

        // Five cells at 66 units each, 50 units a second — about 6.6s.
        for _ in 0..100 {
            tick(&mut app, 0.1);
        }

        assert_eq!(crew_cell(&app, crew), post, "crew never reached their station");
        assert!(
            app.world().get::<CrewPath>(crew).is_none(),
            "the route should be dropped on arrival"
        );
    }

    /// The point of walking: it takes time. If a crew member is at their post
    /// on the first frame, nothing downstream can ever cost travel.
    #[test]
    fn arriving_is_not_instant() {
        let mut app = test_app();
        let (_, crew, post) = corridor_ship(&mut app, 6);
        let start_x = crew_local(&app, crew).x;
        let post_x = grid_to_local(post).x;

        settle(&mut app);
        tick(&mut app, 0.5);

        let x = crew_local(&app, crew).x;
        assert!(x > start_x, "never left the bunks");
        assert!(x < post_x, "crossed five cells before the walk could be seen");
    }

    /// A post with no route to it is not a crash and not a teleport — the
    /// crew member simply stays where they are.
    #[test]
    fn an_unreachable_post_strands_the_crew_where_they_stand() {
        let mut app = test_app();
        let ship = app.world_mut().spawn(Ship).id();

        // Two decks with no deck between them.
        for cell in [IVec2::new(0, 0), IVec2::new(4, 0)] {
            app.world_mut().spawn(hull_at(cell)).insert(ChildOf(ship));
        }

        let crew = app
            .world_mut()
            .spawn((
                CrewMember {
                    name: "Okafor".into(),
                    health: 100.0,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Idle,
                },
                Transform::from_translation(grid_to_local(IVec2::new(0, 0)).extend(CREW_Z)),
            ))
            .insert(ChildOf(ship))
            .id();

        app.world_mut()
            .spawn((
                module_at(ModuleType::Gatling, IVec2::new(4, 0)),
                CrewStation { priority: 6, assigned_crew: Some(crew), manually_assigned: false },
            ))
            .insert(ChildOf(ship));

        for _ in 0..20 {
            tick(&mut app, 0.6);
        }

        assert!(app.world().get::<CrewPath>(crew).is_none(), "routed through vacuum");
        assert_eq!(crew_cell(&app, crew), IVec2::new(0, 0), "crew moved without a route");
    }

    fn idle_ship(app: &mut App) -> (Entity, Entity) {
        let ship = app.world_mut().spawn((Ship, Transform::default())).id();
        for x in 0..6 {
            app.world_mut().spawn(hull_at(IVec2::new(x, 0))).insert(ChildOf(ship));
        }
        // Somewhere worth going.
        app.world_mut()
            .spawn(module_at(ModuleType::BasicQuarters, IVec2::new(5, 0)))
            .insert(ChildOf(ship));

        let crew = app
            .world_mut()
            .spawn((
                CrewMember {
                    name: "Bakare".into(),
                    health: 100.0,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Idle,
                },
                Transform::from_translation(grid_to_local(IVec2::ZERO).extend(CREW_Z)),
            ))
            .insert(ChildOf(ship))
            .id();
        (ship, crew)
    }

    fn off_duty_app() -> App {
        let mut app = App::new();
        app.add_plugins(crate::building::layout::LayoutPlugin);
        app.add_plugins(MinimalPlugins);
        app.init_resource::<CrewPlanTimer>();
        app.init_resource::<CrewOffDutyTimer>();
        app.add_systems(
            Update,
            (rebuild_nav_grids, plan_off_duty_errands, plan_crew_paths, walk_crew).chain(),
        );
        app
    }

    /// A hand nobody has posted should go and live somewhere, not stand where
    /// they were released. Stand-down made this visible: crew came off the
    /// guns in peacetime and simply froze mid-deck.
    #[test]
    fn an_off_duty_hand_goes_somewhere() {
        let mut app = off_duty_app();
        let (_, crew) = idle_ship(&mut app);
        let start = crew_cell(&app, crew);

        for _ in 0..120 {
            tick(&mut app, 0.1);
        }

        assert!(app.world().get::<OffDuty>(crew).is_some(), "nobody ever went off duty");
        assert_ne!(crew_cell(&app, crew), start, "the off-duty hand never moved");
    }

    /// And they must not settle permanently. The first version picked the spot
    /// from the entity id alone, which is the same answer every time: they
    /// walked to one bunk and stood in it for the rest of the game.
    #[test]
    fn they_move_on_rather_than_settling_forever() {
        let mut app = off_duty_app();
        let (_, crew) = idle_ship(&mut app);

        let mut seen = bevy::platform::collections::HashSet::new();
        // Long enough to outlast a bunk's dwell several times over.
        for _ in 0..2400 {
            tick(&mut app, 0.1);
            seen.insert(crew_cell(&app, crew));
        }

        assert!(
            seen.len() > 2,
            "an off-duty hand only ever occupied {seen:?} - they are not wandering"
        );
    }

    /// A ship with a working ward at one end and one hand at the other.
    fn ward_ship(app: &mut App, health: f32) -> (Entity, Entity, IVec2) {
        let ship = app.world_mut().spawn((Ship, Transform::default())).id();
        for x in 0..5 {
            app.world_mut().spawn(hull_at(IVec2::new(x, 0))).insert(ChildOf(ship));
        }

        let ward_cell = IVec2::new(4, 0);
        app.world_mut()
            .spawn((
                module_at(ModuleType::MedBay, ward_cell),
                CrewFacility { facility_type: FacilityType::MedBay },
                // MedBay is `crew_station: true` in the registry, so the real
                // one carries this too — which is what makes its cell a
                // NavCell::Post and therefore somewhere a patient can walk.
                CrewStation { priority: 3, assigned_crew: None, manually_assigned: false },
            ))
            .insert(ChildOf(ship));

        let crew = app
            .world_mut()
            .spawn((
                CrewMember {
                    name: "Bakare".into(),
                    health,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Idle,
                },
                Transform::from_translation(grid_to_local(IVec2::ZERO).extend(CREW_Z)),
            ))
            .insert(ChildOf(ship))
            .id();

        (ship, crew, ward_cell)
    }

    fn med_app() -> App {
        let mut app = App::new();
        app.add_plugins(crate::building::layout::LayoutPlugin);
        app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin));
        app.init_resource::<CrewPlanTimer>();
        app.init_resource::<CrewMedicalTimer>();
        app.add_systems(
            Update,
            (
                rebuild_nav_grids,
                plan_crew_destinations,
                plan_medical_errands,
                plan_crew_paths,
                walk_crew,
            )
                .chain(),
        );
        app
    }

    /// `settle` is cut to the 0.5s planning cadence. Medical errands run on a
    /// 1s clock, so they need a longer run-up before anything has happened.
    fn settle_med(app: &mut App) {
        for _ in 0..8 {
            tick(app, 0.3);
        }
    }

    fn hostile_at(app: &mut App, x: f32) {
        app.world_mut()
            .spawn((crate::ai_ship::components::AiShip, Transform::from_xyz(x, 0.0, 0.0)));
    }

    /// The whole point: a hurt crew member leaves their post and walks to the
    /// ward under their own steam. Nothing used to send anybody there, so the
    /// MedBay healed whoever happened to already be standing in it.
    #[test]
    fn the_wounded_walk_to_the_ward() {
        let mut app = med_app();
        let (_, crew, ward) = ward_ship(&mut app, 40.0);

        settle_med(&mut app);
        assert!(
            app.world().get::<SeekingTreatment>(crew).is_some(),
            "a wounded hand was never sent for treatment"
        );
        assert_eq!(app.world().get::<CrewDestination>(crew).map(|d| d.0), Some(ward));

        for _ in 0..120 {
            tick(&mut app, 0.1);
        }
        assert_eq!(crew_cell(&app, crew), ward, "never reached the ward");
    }

    /// Nobody strolls to the surgery while the ship is being shot at.
    #[test]
    fn treatment_is_cancelled_by_a_fight() {
        let mut app = med_app();
        let (_, crew, _) = ward_ship(&mut app, 40.0);
        hostile_at(&mut app, COMBAT_RANGE * 0.5);

        settle_med(&mut app);

        assert!(
            app.world().get::<SeekingTreatment>(crew).is_none(),
            "walked off to the ward mid-battle"
        );
    }

    /// The same hostile, out past the threshold, is not a fight.
    #[test]
    fn a_distant_hostile_does_not_cancel_treatment() {
        let mut app = med_app();
        let (_, crew, _) = ward_ship(&mut app, 40.0);
        hostile_at(&mut app, COMBAT_RANGE * 2.0);

        settle_med(&mut app);

        assert!(
            app.world().get::<SeekingTreatment>(crew).is_some(),
            "a hostile on the far side of the system stopped treatment"
        );
    }

    /// A crew member in one piece stays at their post.
    #[test]
    fn the_unhurt_do_not_go_to_the_ward() {
        let mut app = med_app();
        let (_, crew, _) = ward_ship(&mut app, 100.0);

        settle_med(&mut app);

        assert!(
            app.world().get::<SeekingTreatment>(crew).is_none(),
            "a healthy hand abandoned their post for the surgery"
        );
    }

    /// The floor gets shot out from under someone who has nowhere in
    /// particular to be. They must walk off the hole.
    ///
    /// The recovery in `plan_crew_paths` used to fire only as the START of a
    /// route to somewhere else, so an unassigned hand - or anyone already
    /// standing on their destination - was left hanging in the gap where
    /// their deck used to be, motionless, for the rest of the game.
    #[test]
    fn a_crew_member_walks_off_a_destroyed_deck_plate() {
        let mut app = test_app();
        let ship = app.world_mut().spawn(Ship).id();

        let mut plates = Vec::new();
        for x in 0..5 {
            let cell = IVec2::new(x, 0);
            plates.push((cell, app.world_mut().spawn(hull_at(cell)).insert(ChildOf(ship)).id()));
        }

        // Nobody has assigned them anything: no station, no destination.
        let stood_on = IVec2::new(2, 0);
        let crew = app
            .world_mut()
            .spawn((
                CrewMember {
                    name: "Vasquez".into(),
                    health: 100.0,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Idle,
                },
                Transform::from_translation(grid_to_local(stood_on).extend(CREW_Z)),
            ))
            .insert(ChildOf(ship))
            .id();

        settle(&mut app);
        assert_eq!(crew_cell(&app, crew), stood_on, "moved before anything happened");

        // A round takes out the plate they are standing on.
        let plate = plates.iter().find(|(c, _)| *c == stood_on).unwrap().1;
        app.world_mut().entity_mut(plate).insert(HullDestroyed);

        for _ in 0..40 {
            tick(&mut app, 0.2);
        }

        let ended = crew_cell(&app, crew);
        assert_ne!(ended, stood_on, "crew stayed standing on empty space");
        let nav = app.world().get::<NavGrid>(ship).unwrap();
        assert!(nav.passable(ended), "crew walked to {ended:?}, which is not deck");
    }
}

/// How near a hostile has to be before the crew stop doing chores and stay at
/// their posts. Generous on purpose — you should not have damage-control teams
/// strolling into the open while something is still shooting at you.
pub const COMBAT_RANGE: f32 = 24_000.0;

/// Is anything hostile close enough to count as a fight?
///
/// One definition, shared by everything that has to care: damage-control
/// errands, medical errands, and the ward itself. Two systems disagreeing
/// about whether the ship is in combat would show up as crew walking to the
/// surgery mid-battle, or sitting in it not healing.
pub fn under_threat(ship_pos: Vec2, hostiles: impl Iterator<Item = Vec2>) -> bool {
    hostiles.into_iter().any(|h| h.distance(ship_pos) < COMBAT_RANGE)
}

/// This crew member has left their post to get patched up.
///
/// A marker rather than a `CrewState`, because the state machine in
/// `crew::update_crew_ai` overwrites itself constantly and every consumer of
/// `CrewState` already treats anything that isn't `Idle` as unavailable — a
/// wounded hand walking to the ward is still available, just elsewhere.
#[derive(Component)]
pub struct SeekingTreatment;

#[derive(Resource)]
pub struct CrewErrandTimer(Timer);

impl Default for CrewErrandTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.0, TimerMode::Repeating))
    }
}

/// Medical errands keep their OWN clock. Two systems ticking one repeating
/// timer advance it twice a frame and then take it in turns to see
/// `just_finished`, so sharing `CrewErrandTimer` would have had damage control
/// and the sick parade each firing half as often as they read.
#[derive(Resource)]
pub struct CrewMedicalTimer(Timer);

impl Default for CrewMedicalTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.0, TimerMode::Repeating))
    }
}

/// Out of combat, hands with no post go and fix things.
///
/// Only crew nobody has assigned to a station take errands — the reactor keeps
/// its operator while the spare hands patch the hull. That is the whole reason
/// the engine room stopped needing five people: it freed the hands this uses.
///
/// Nothing here touches `CrewMember::state`. An idle crew member already mends
/// what's within reach at half rate (`IDLE_REPAIR_POWER`), so walking them to
/// the damage is enough to get breaches sealed and plating patched between
/// fights — basic damage control, not a full repair crew.
pub fn plan_repair_errands(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<CrewErrandTimer>,
    ships: Query<(Entity, &GlobalTransform, &NavGrid), (With<Ship>, Without<OwnedByAiShip>)>,
    hostiles: Query<&GlobalTransform, With<crate::ai_ship::components::AiShip>>,
    stations: Query<&CrewStation>,
    hulls: Query<(&HullSegment, &ChildOf), Without<HullDestroyed>>,
    modules: Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    crew: Query<
        (Entity, &Transform, &ChildOf),
        (
            With<CrewMember>,
            Without<EvaSalvaging>,
            Without<OwnedByAiShip>,
            // Don't send someone running from a closing bulkhead to go and
            // patch the very breach they are running from.
            Without<Fleeing>,
            Without<SeekingTreatment>,
            Without<crate::crew::burial::BurialDetail>,
            // Nor one on their way to the suit locker.
            Without<SuitingUp>,
            // Nor one suited up and working a hole.
            Without<DamageControl>,
        ),
    >,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }
    let Ok((ship, ship_gt, nav)) = ships.single() else { return };

    let ship_pos = ship_gt.translation().truncate();
    if under_threat(ship_pos, hostiles.iter().map(|h| h.translation().truncate())) {
        return;
    }

    // Breaches first — air is leaving. Then anything simply damaged.
    let mut jobs: Vec<(u8, IVec2)> = Vec::new();
    for (hull, parent) in hulls.iter() {
        if parent.parent() != ship {
            continue;
        }
        if hull.is_depressurized && hull.depressurization_level > 0.0 {
            jobs.push((0, hull.grid_position));
        } else if hull.health < hull.max_health {
            jobs.push((1, hull.grid_position));
        }
    }
    for (module, parent) in modules.iter() {
        if parent.parent() == ship && module.health < module.max_health && module.health > 0.0 {
            jobs.push((1, module.grid_position));
        }
    }
    if jobs.is_empty() {
        return;
    }

    let posted: bevy::platform::collections::HashSet<Entity> =
        stations.iter().filter_map(|s| s.assigned_crew).collect();

    // One job per hand, nearest first — otherwise every spare crew member
    // walks to the same breach and the rest of the damage goes untouched.
    let mut claimed: bevy::platform::collections::HashSet<IVec2> = bevy::platform::collections::HashSet::new();
    for (entity, transform, parent) in crew.iter() {
        if parent.parent() != ship || posted.contains(&entity) {
            continue;
        }
        let here = local_to_grid(transform.translation.truncate());

        let best = jobs
            .iter()
            .filter(|(_, cell)| !claimed.contains(cell))
            .filter_map(|(severity, cell)| {
                // Stand next to the damage, not inside it — a breached hull
                // cell is not somewhere a person fits.
                nav.nearest_passable(*cell).map(|stand| {
                    let d = (here - stand).abs();
                    (*severity, d.x + d.y, *cell, stand)
                })
            })
            .min_by_key(|(severity, dist, _, _)| (*severity, *dist));

        let Some((_, _, job_cell, stand)) = best else { continue };
        claimed.insert(job_cell);
        commands.entity(entity).try_insert(CrewDestination(stand));
    }
}

/// Out of combat, the wounded walk to the ward.
///
/// The MedBay has always healed whoever happened to be standing in its room,
/// and nothing has ever sent anybody there — destinations came from your post
/// or from damage control, and neither had heard of health. So the bay healed
/// by coincidence or not at all. This is the missing half.
///
/// Runs LAST of the destination setters on purpose: a patient's errand
/// outranks their gun, their bunk and the hull breach down the corridor.
/// Combat cancels it — nobody strolls to the surgery while the ship is being
/// shot at, they go back to their post and bleed.
pub fn plan_medical_errands(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<CrewMedicalTimer>,
    ships: Query<(Entity, &GlobalTransform, &NavGrid), (With<Ship>, Without<OwnedByAiShip>)>,
    hostiles: Query<&GlobalTransform, With<crate::ai_ship::components::AiShip>>,
    wards: Query<(&CrewFacility, &Module, &ChildOf), Without<DestroyedModule>>,
    crew: Query<
        (Entity, &CrewMember, &ChildOf, Has<SeekingTreatment>),
        (Without<EvaSalvaging>, Without<OwnedByAiShip>),
    >,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }
    let Ok((ship, ship_gt, nav)) = ships.single() else { return };

    let fighting = under_threat(
        ship_gt.translation().truncate(),
        hostiles.iter().map(|h| h.translation().truncate()),
    );

    // Nearest cell of any working ward. A MedBay is `crew_station: true`, so
    // it is `NavCell::Post` — somewhere a person can actually stand — which is
    // what makes "walk into the surgery" a route rather than a wish.
    let ward = wards
        .iter()
        .filter(|(facility, module, parent)| {
            facility.facility_type == FacilityType::MedBay
                && module.is_active
                && module.health > 0.0
                && parent.parent() == ship
        })
        .find_map(|(_, module, _)| station_cell(nav, module));

    for (entity, member, parent, treating) in crew.iter() {
        if parent.parent() != ship {
            continue;
        }

        // Fit for duty, no ward to go to, or there is shooting: back to work.
        let wants_care = member.health > 0.0 && member.health < member.max_health;
        let Some(ward) = ward.filter(|_| wants_care && !fighting) else {
            if treating {
                commands.entity(entity).try_remove::<SeekingTreatment>();
            }
            continue;
        };

        commands.entity(entity).try_insert((SeekingTreatment, CrewDestination(ward)));
    }
}

/// Somewhere an off-duty hand has taken themselves, and how long for.
#[derive(Component)]
pub struct OffDuty {
    /// Counts down only once they have ARRIVED. Ticking it while they walk
    /// would have anyone crossing the ship turn round before they got there.
    dwell: Timer,
    /// How many places they have been. Mixed into the next choice, because a
    /// pick derived from the entity alone is the SAME pick every time — they
    /// would walk to one bunk and stand in it for the rest of the game.
    visits: u32,
}

/// Seconds spent at a spot before wandering on. A bunk is worth lingering in,
/// a stretch of corridor is not.
fn dwell_for(module_type: Option<ModuleType>) -> f32 {
    match module_type {
        Some(ModuleType::BasicQuarters | ModuleType::Barracks | ModuleType::OfficerQuarters) => 45.0,
        Some(ModuleType::MessHall | ModuleType::GalleyMess) => 25.0,
        Some(ModuleType::RecRoom | ModuleType::WellnessHub) => 30.0,
        Some(_) => 18.0,
        // A corridor is a place you pass through, not one you stand in.
        None => 8.0,
    }
}

/// How often anyone bothers to think about where to be next.
const OFF_DUTY_INTERVAL: f32 = 1.5;

#[derive(Resource)]
pub struct CrewOffDutyTimer(Timer);

impl Default for CrewOffDutyTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(OFF_DUTY_INTERVAL, TimerMode::Repeating))
    }
}

/// Hands with nothing to do go and live somewhere.
///
/// Crew stand down from guns and engines in peacetime now
/// (`crew::auto_assign_crew`), which produced a new problem: the released
/// hands simply stopped dead wherever they happened to be standing, which
/// reads far worse than being nailed to a post did. So off-duty crew take
/// themselves to a bunk, the galley, the rec room, or just somewhere else
/// along the corridor, wait a while, and move on.
///
/// LOWEST priority of everything that hands out a destination — it runs first
/// in the chain so damage control, a trip to the ward and a burial detail all
/// overwrite it. Nothing here is a job.
pub fn plan_off_duty_errands(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<CrewOffDutyTimer>,
    ships: Query<(Entity, &NavGrid), (With<Ship>, Without<OwnedByAiShip>)>,
    modules: Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    stations: Query<&CrewStation>,
    mut crew: Query<
        (Entity, &Transform, &ChildOf, &CrewMember, Option<&mut OffDuty>, Option<&CrewPath>),
        (
            Without<EvaSalvaging>,
            Without<OwnedByAiShip>,
            Without<SeekingTreatment>,
            Without<crate::crew::burial::BurialDetail>,
            // Nor one on their way to the suit locker.
            Without<SuitingUp>,
            // Nor one suited up and working a hole.
            Without<DamageControl>,
        ),
    >,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }
    let Ok((ship, nav)) = ships.single() else { return };

    // Places worth being. Crew-category modules are the ship's living space —
    // bunks, the galley, the rec room. The med bay is excluded: it is for the
    // wounded, and healthy crew loitering in the surgery is not the read.
    let mut spots: Vec<(IVec2, Option<ModuleType>)> = Vec::new();
    for (module, parent) in modules.iter() {
        if parent.parent() != ship || module.module_type.category() != ModuleCategory::Crew {
            continue;
        }
        if matches!(module.module_type, ModuleType::MedBay | ModuleType::SurgicalBay) {
            continue;
        }
        let footprint = footprints::footprint_override(module.module_type);
        let cells =
            ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint);
        if let Some(cell) = cells.first().and_then(|c| nav.nearest_passable(*c)) {
            spots.push((cell, Some(module.module_type)));
        }
    }
    // Plus the corridors themselves, so the ship looks lived-in rather than
    // like everyone is queuing for the same four rooms.
    let mut halls: Vec<IVec2> = nav
        .cells
        .iter()
        .filter(|(_, kind)| **kind == NavCell::Hallway)
        .map(|(cell, _)| *cell)
        .collect();
    halls.sort_by_key(|c| (c.x, c.y));
    spots.extend(halls.iter().map(|c| (*c, None)));

    if spots.is_empty() {
        return;
    }

    let posted: bevy::platform::collections::HashSet<Entity> =
        stations.iter().filter_map(|s| s.assigned_crew).collect();

    for (entity, transform, parent, member, off_duty, path) in crew.iter_mut() {
        if parent.parent() != ship || member.health <= 0.0 {
            continue;
        }
        // On duty, or busy with something that matters.
        if posted.contains(&entity)
            || member.state == CrewState::Repairing
            || member.state == CrewState::Panicking
        {
            if off_duty.is_some() {
                commands.entity(entity).try_remove::<OffDuty>();
            }
            continue;
        }

        let visits = match off_duty {
            Some(mut rest) => {
                // Still walking there — the clock has not started.
                if path.is_some() {
                    continue;
                }
                rest.dwell.tick(time.delta());
                if !rest.dwell.is_finished() {
                    continue;
                }
                rest.visits.wrapping_add(1)
            }
            None => 0,
        };

        // Two things go into the choice: WHO (so a whole watch coming off duty
        // does not file into the same bunk) and HOW MANY PLACES THEY HAVE BEEN
        // (so the next one is somewhere else).
        let seed = (entity.to_bits() as usize)
            .wrapping_mul(2654435761)
            ^ (visits as usize).wrapping_mul(0x9E37_79B9);
        let here = local_to_grid(transform.translation.truncate());

        // Walk the list from that point until something that isn't here.
        let start = (seed >> 8) % spots.len();
        let Some((cell, kind)) = (0..spots.len())
            .map(|i| spots[(start + i) % spots.len()])
            .find(|(cell, _)| *cell != here)
        else {
            continue; // nowhere else on the ship to be
        };

        commands.entity(entity).try_insert((
            OffDuty { dwell: Timer::from_seconds(dwell_for(kind), TimerMode::Once), visits },
            CrewDestination(cell),
        ));
    }
}
