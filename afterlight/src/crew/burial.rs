//! Burial at space.
//!
//! When the shooting stops, a free hand carries the dead to an airlock and puts
//! them out. The body does not despawn and is never cleaned up: it drifts, and
//! it is still drifting if you come back to that system a hundred hours later.
//!
//! That permanence is the whole point, so the bodies do NOT live only as
//! entities — entities die with the system when you warp out of it. They live
//! in `DriftingDead`, which is saved with the game, and entities are spawned
//! from it whenever their system is the one you're standing in. Same shape as
//! `sync_station_entities`: the resource is the truth, the entities are a view
//! of whichever slice of it is currently loaded.
//!
//! Nothing here removes a body. A scavenger/cleaner faction is the intended
//! answer to that, and it is a faction problem rather than a crew one.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ai_ship::components::{AiShip, OwnedByAiShip};
use crate::building::{footprints, grid_to_local, local_to_grid, ShipGrid};
use crate::celestial::resources::SystemStreamingManager;
use crate::components::*;
use crate::events::{NotificationType, ShowNotification};
use crate::crew::animation::{CrewAtlases, CrewCorpse, CREW_SIZE};
use crate::crew::eva_salvage::EvaSalvaging;
use crate::crew::navigation::NavGrid;
use crate::crew::walking::{under_threat, CrewDestination, CrewPath, SeekingTreatment};

/// Speed a body leaves the lock at, on top of the ship's own motion. Slow on
/// purpose: it should still be near the place it died when you come back, not
/// halfway across the system.
const EJECT_SPEED: f32 = 9.0;

/// Slow tumble, radians per second. Nothing out there will ever stop it.
const EJECT_SPIN: f32 = 0.22;

/// Z for a drifting body — below ships, above the starfield.
const DRIFT_Z: f32 = 0.35;

/// Close enough to a cell to count as standing on it.
const REACH: f32 = 10.0;

/// One of the dead, adrift. Serialised with the save, so it outlives the
/// entity, the system and the session.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DriftingBody {
    /// Monotonic id, so a loaded entity can find its own registry row again.
    pub id: u64,
    /// The star system this body is drifting in.
    pub system_id: u32,
    pub position: Vec2,
    pub velocity: Vec2,
    pub rotation: f32,
    /// Who they were. Nothing reads it yet; it is the hook for a memorial, a
    /// recovery contract, or a scavenger who can tell you whose ship this was.
    pub name: String,
}

/// Every body ever put out of an airlock, across every system.
///
/// Deliberately uncapped. A body is a few dozen bytes and a couple of floats;
/// the day this needs pruning is the day a cleaner faction exists to do the
/// pruning in fiction rather than in a `Vec::truncate`.
#[derive(Resource, Default, Serialize, Deserialize)]
pub struct DriftingDead {
    pub bodies: Vec<DriftingBody>,
    next_id: u64,
}

impl DriftingDead {
    /// Register a body. Public because the lock is not the only way out: air
    /// venting through a hull breach puts people into space too, and those
    /// belong on the same register (see `ship::air::crew_suction`).
    pub fn add(&mut self, system_id: u32, position: Vec2, velocity: Vec2, rotation: f32, name: String) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.bodies.push(DriftingBody { id, system_id, position, velocity, rotation, name });
        id
    }

    /// Rebuilds the id counter after a load, so new bodies can't collide with
    /// restored ones.
    pub fn reseed(&mut self) {
        self.next_id = self.bodies.iter().map(|b| b.id + 1).max().unwrap_or(0);
    }
}

/// Which leg of the errand a pallbearer is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BurialPhase {
    /// Walking to where the body fell.
    Fetch,
    /// Carrying it to the lock.
    Carry,
}

/// A crew member seeing one of the dead out of the ship.
#[derive(Component)]
pub struct BurialDetail {
    pub body: Entity,
    pub phase: BurialPhase,
}

/// A body in world space, no longer attached to any ship.
#[derive(Component)]
pub struct DriftingCorpse {
    /// Row in `DriftingDead` this entity is a view of.
    pub id: u64,
    pub velocity: Vec2,
    pub spin: f32,
}

/// The cell a pallbearer should stand in to work the lock. Airlocks are not
/// crew stations, so their own cell is solid — you stand beside one.
fn airlock_cell(nav: &NavGrid, modules: impl Iterator<Item = Module>) -> Option<IVec2> {
    for module in modules {
        if !matches!(module.module_type, ModuleType::AirlockChamber | ModuleType::DockingPort) {
            continue;
        }
        if !module.is_active || module.health <= 0.0 {
            continue;
        }
        let footprint = footprints::footprint_override(module.module_type);
        let cells = ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint);
        if let Some(cell) = cells.first().and_then(|c| nav.nearest_passable(*c)) {
            return Some(cell);
        }
    }
    None
}

/// Has this crew member got where they were sent?
fn arrived(transform: &Transform, path: Option<&CrewPath>, target: IVec2) -> bool {
    path.is_none() && transform.translation.truncate().distance(grid_to_local(target)) <= REACH
}

/// Out of combat, details off spare hands to carry the dead to the lock.
///
/// Only hands nobody has posted, for the same reason damage control uses
/// them: the reactor keeps its operator while the spare crew do the chores.
/// No airlock means no burials — the bodies simply stay where they fell,
/// which is a fair thing for a ship with nowhere to put them.
#[allow(clippy::too_many_arguments)]
pub fn plan_burial_detail(
    mut commands: Commands,
    ships: Query<(Entity, &GlobalTransform, &NavGrid), (With<Ship>, Without<OwnedByAiShip>)>,
    hostiles: Query<&GlobalTransform, With<AiShip>>,
    modules: Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    stations: Query<&CrewStation>,
    corpses: Query<(Entity, &Transform, &ChildOf), With<CrewCorpse>>,
    carriers: Query<&BurialDetail>,
    crew: Query<
        (Entity, &Transform, &ChildOf),
        (
            With<CrewMember>,
            Without<EvaSalvaging>,
            Without<OwnedByAiShip>,
            Without<SeekingTreatment>,
            Without<BurialDetail>,
        ),
    >,
) {
    let Ok((ship, ship_gt, nav)) = ships.single() else { return };
    if under_threat(
        ship_gt.translation().truncate(),
        hostiles.iter().map(|h| h.translation().truncate()),
    ) {
        return;
    }

    let Some(lock) = airlock_cell(
        nav,
        modules.iter().filter(|(_, p)| p.parent() == ship).map(|(m, _)| m.clone()),
    ) else {
        return;
    };

    // Bodies already spoken for by someone else's detail.
    let claimed: std::collections::HashSet<Entity> =
        carriers.iter().map(|d| d.body).collect();
    let posted: std::collections::HashSet<Entity> =
        stations.iter().filter_map(|s| s.assigned_crew).collect();

    let mut waiting: Vec<(Entity, IVec2)> = corpses
        .iter()
        .filter(|(entity, _, parent)| parent.parent() == ship && !claimed.contains(entity))
        .map(|(entity, transform, _)| (entity, local_to_grid(transform.translation.truncate())))
        .collect();
    if waiting.is_empty() {
        return;
    }
    // Deterministic order, so the same run assigns the same details.
    waiting.sort_by_key(|(_, cell)| (cell.x, cell.y));

    for (entity, transform, parent) in crew.iter() {
        if parent.parent() != ship || posted.contains(&entity) {
            continue;
        }
        let here = local_to_grid(transform.translation.truncate());
        // Nearest body to this hand, so two pallbearers don't cross the ship
        // past each other.
        let Some(best) = waiting
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, cell))| (here - *cell).abs().element_sum())
            .map(|(i, _)| i)
        else {
            break;
        };
        let (body, cell) = waiting.remove(best);

        commands.entity(entity).try_insert((
            BurialDetail { body, phase: BurialPhase::Fetch },
            CrewDestination(nav.nearest_passable(cell).unwrap_or(lock)),
        ));

        if waiting.is_empty() {
            break;
        }
    }
}

/// Walks a detail through its two legs and puts the body out at the end.
#[allow(clippy::too_many_arguments)]
pub fn advance_burial(
    mut commands: Commands,
    streaming: Res<SystemStreamingManager>,
    mut dead: ResMut<DriftingDead>,
    ships: Query<(Entity, &GlobalTransform, &Velocity, &NavGrid), (With<Ship>, Without<OwnedByAiShip>)>,
    modules: Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    mut carriers: Query<
        (Entity, &Transform, &mut BurialDetail, Option<&CrewPath>),
        (With<CrewMember>, Without<CrewCorpse>),
    >,
    mut bodies: Query<(&mut Transform, Option<&CrewCorpse>), Without<CrewMember>>,
    cascade: Res<crate::narrative::CascadeState>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    let Ok((ship, ship_gt, ship_vel, nav)) = ships.single() else { return };
    let Some(lock) = airlock_cell(
        nav,
        modules.iter().filter(|(_, p)| p.parent() == ship).map(|(m, _)| m.clone()),
    ) else {
        return;
    };

    for (carrier, carrier_transform, mut detail, path) in carriers.iter_mut() {
        // The body was destroyed under them (the block it lay on was shot off
        // and took it with it). Nothing to bury.
        if bodies.get(detail.body).is_err() {
            commands.entity(carrier).try_remove::<BurialDetail>();
            continue;
        }

        match detail.phase {
            BurialPhase::Fetch => {
                let Ok((body_transform, _)) = bodies.get(detail.body) else { continue };
                let body_cell = local_to_grid(body_transform.translation.truncate());
                let stand = nav.nearest_passable(body_cell).unwrap_or(body_cell);
                if arrived(carrier_transform, path, stand) {
                    detail.phase = BurialPhase::Carry;
                    commands.entity(carrier).try_insert(CrewDestination(lock));
                }
            }
            BurialPhase::Carry => {
                // Carried: the body rides on the shoulder, in ship-local space
                // still, so it turns with the hull like everything else aboard.
                if let Ok((mut body_transform, _)) = bodies.get_mut(detail.body) {
                    body_transform.translation.x = carrier_transform.translation.x;
                    body_transform.translation.y = carrier_transform.translation.y;
                    body_transform.rotation = carrier_transform.rotation;
                }

                if !arrived(carrier_transform, path, lock) {
                    continue;
                }

                // Out of the lock. Ship-local becomes world, and it keeps the
                // ship's own motion — you cannot drop something from a moving
                // vessel and have it stop.
                let world = ship_gt.transform_point(carrier_transform.translation).truncate();
                let facing = ship_gt.compute_transform().rotation;
                let push = (facing * Vec3::Y).truncate().normalize_or_zero() * EJECT_SPEED;
                let velocity = ship_vel.0 + push;

                let name = bodies
                    .get(detail.body)
                    .ok()
                    .and_then(|(_, corpse)| corpse.map(|c| c.name.clone()))
                    .unwrap_or_default();
                let name_for_line = name.clone();
                let id = dead.add(
                    streaming.loaded_system.unwrap_or(u32::MAX),
                    world,
                    velocity,
                    0.0,
                    name,
                );

                // Say something as they go. Near Haven it is the plain fact.
                // Further out the ship starts adding things nobody asked it to
                // add, which is the only place the story ever states its own
                // premise outright — and it takes going a long way to hear it.
                let who = if name_for_line.is_empty() { "A hand".to_string() } else { name_for_line.clone() };
                let line = match cascade.ring {
                    0 => format!("{who} committed to space."),
                    1 => format!("{who} committed to space. The roster is updated."),
                    2 => format!("{who} committed to space. Their station logs are retained."),
                    _ => format!("{who} committed to space. The body, anyway."),
                };
                notifications.write(ShowNotification {
                    message: line,
                    notification_type: NotificationType::Info,
                    duration: 5.0,
                });

                commands
                    .entity(detail.body)
                    .try_remove::<ChildOf>()
                    .try_insert((
                        Transform::from_translation(world.extend(DRIFT_Z)),
                        DriftingCorpse { id, velocity, spin: EJECT_SPIN },
                    ));
                commands.entity(carrier).try_remove::<BurialDetail>();
            }
        }
    }
}

/// Moves the drifting dead, and keeps their saved rows in step.
///
/// No drag and nothing to slow them: this is the one place in the game where
/// "momentum is forever" is taken completely literally.
pub fn drift_dead(
    time: Res<Time>,
    mut dead: ResMut<DriftingDead>,
    mut drifting: Query<(&mut Transform, &DriftingCorpse)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut transform, corpse) in drifting.iter_mut() {
        transform.translation.x += corpse.velocity.x * dt;
        transform.translation.y += corpse.velocity.y * dt;
        transform.rotate_z(corpse.spin * dt);

        // Write the position back so warping out and returning finds them
        // where they actually got to, not where they were let go.
        if let Some(row) = dead.bodies.iter_mut().find(|b| b.id == corpse.id) {
            row.position = transform.translation.truncate();
            row.rotation = transform.rotation.to_euler(EulerRot::ZYX).0;
        }
    }
}

/// Spawns the bodies belonging to the system you're in, and despawns the rest.
///
/// The same contract `sync_station_entities` has: the resource is the truth,
/// entities are only the slice of it that is currently loaded. Warping away
/// does not bury anybody twice or lose them.
pub fn sync_drifting_dead(
    mut commands: Commands,
    assets: Res<AssetServer>,
    atlases: Res<CrewAtlases>,
    streaming: Res<SystemStreamingManager>,
    dead: Res<DriftingDead>,
    existing: Query<(Entity, &DriftingCorpse)>,
) {
    let Some(system) = streaming.loaded_system else {
        for (entity, _) in existing.iter() {
            commands.entity(entity).try_despawn();
        }
        return;
    };

    let mut present: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for (entity, corpse) in existing.iter() {
        let belongs = dead.bodies.iter().any(|b| b.id == corpse.id && b.system_id == system);
        if belongs {
            present.insert(corpse.id);
        } else {
            commands.entity(entity).try_despawn();
        }
    }

    for body in dead.bodies.iter().filter(|b| b.system_id == system) {
        if present.contains(&body.id) {
            continue;
        }
        commands.spawn((
            Sprite {
                image: assets.load(crate::crew::animation::CrewAnimState::Dead.sheet()),
                custom_size: Some(Vec2::splat(CREW_SIZE)),
                texture_atlas: Some(TextureAtlas { layout: atlases.dead.clone(), index: 0 }),
                ..default()
            },
            Transform::from_translation(body.position.extend(DRIFT_Z))
                .with_rotation(Quat::from_rotation_z(body.rotation)),
            DriftingCorpse { id: body.id, velocity: body.velocity, spin: EJECT_SPIN },
        ));
    }
}

#[cfg(test)]
mod burial_tests {
    use super::*;
    use crate::crew::navigation::rebuild_nav_grids;
    use crate::crew::walking::{plan_crew_paths, walk_crew, CrewPlanTimer, CREW_Z};
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    const SYSTEM: u32 = 3;

    fn tick(app: &mut App, seconds: f32) {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f32(seconds),
        ));
        app.update();
    }

    /// Corridor from (0,0) to (4,0) with an airlock on the end, one body on
    /// the deck at one end and one spare hand in the middle.
    fn ship_with_a_body(app: &mut App) -> (Entity, Entity, Entity) {
        let ship = app
            .world_mut()
            .spawn((Ship, Transform::default(), Velocity(Vec2::ZERO)))
            .id();

        for x in 0..5 {
            let cell = IVec2::new(x, 0);
            app.world_mut()
                .spawn((
                    HullSegment {
                        grid_position: cell,
                        hull_layer: HullLayer::Hallway,
                        ..default()
                    },
                    Transform::from_translation(grid_to_local(cell).extend(0.1)),
                ))
                .insert(ChildOf(ship));
        }

        app.world_mut()
            .spawn(Module {
                module_type: ModuleType::AirlockChamber,
                health: 100.0,
                max_health: 100.0,
                power_consumption: 0.0,
                power_generation: 0.0,
                is_active: true,
                grid_position: IVec2::new(4, 0),
                size: IVec2::ONE,
                rotation: Rotation::North,
            })
            .insert(ChildOf(ship));

        let body = app
            .world_mut()
            .spawn((
                CrewCorpse { name: "Ferreira".into() },
                Transform::from_translation(grid_to_local(IVec2::ZERO).extend(CREW_Z)),
            ))
            .insert(ChildOf(ship))
            .id();

        let hand = app
            .world_mut()
            .spawn((
                CrewMember {
                    name: "Ferreira".into(),
                    health: 100.0,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Idle,
                },
                Transform::from_translation(grid_to_local(IVec2::new(2, 0)).extend(CREW_Z)),
            ))
            .insert(ChildOf(ship))
            .id();

        (ship, hand, body)
    }

    fn burial_app() -> App {
        let mut app = App::new();
        app.add_plugins(crate::building::layout::LayoutPlugin);
        app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin));
        app.init_resource::<CrewPlanTimer>();
        app.init_resource::<DriftingDead>();
        // advance_burial reads the cascade level to decide what it says as a
        // body goes out. A fresh, quiet run is the right default here.
        app.init_resource::<crate::narrative::CascadeState>();
        app.add_message::<ShowNotification>();
        app.insert_resource(SystemStreamingManager {
            loaded_system: Some(SYSTEM),
            ..default()
        });
        app.add_systems(
            Update,
            (
                rebuild_nav_grids,
                plan_burial_detail,
                advance_burial,
                plan_crew_paths,
                walk_crew,
            )
                .chain(),
        );
        app
    }

    /// The scene, end to end: a spare hand walks to the body, carries it to
    /// the lock, and puts it out. Once it is out it belongs to the world, not
    /// to the ship — which is what lets it stay behind when you leave.
    #[test]
    fn a_spare_hand_carries_the_dead_to_the_lock_and_puts_them_out() {
        let mut app = burial_app();
        let (_, hand, body) = ship_with_a_body(&mut app);

        for _ in 0..250 {
            tick(&mut app, 0.1);
        }

        assert!(
            app.world().get::<DriftingCorpse>(body).is_some(),
            "the body was never put out of the lock"
        );
        assert!(
            app.world().get::<ChildOf>(body).is_none(),
            "the body is still parented to the ship after ejection"
        );
        assert!(
            app.world().get::<BurialDetail>(hand).is_none(),
            "the pallbearer never finished the errand"
        );

        let dead = app.world().resource::<DriftingDead>();
        assert_eq!(dead.bodies.len(), 1, "the body was not recorded as drifting");
        assert_eq!(dead.bodies[0].system_id, SYSTEM, "recorded in the wrong system");
        assert!(dead.bodies[0].velocity.length() > 0.0, "left with no drift at all");
        assert_eq!(dead.bodies[0].name, "Ferreira", "the drifting dead lost their name");
    }

    /// A ship with nowhere to put anyone keeps its dead. That is a fair cost
    /// for not building an airlock, not a bug.
    #[test]
    fn without_an_airlock_the_dead_stay_aboard() {
        let mut app = burial_app();
        let (ship, _, body) = ship_with_a_body(&mut app);

        // Take the lock away.
        let airlock = app
            .world_mut()
            .query_filtered::<Entity, With<Module>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().entity_mut(airlock).despawn();
        let _ = ship;

        for _ in 0..120 {
            tick(&mut app, 0.1);
        }

        assert!(app.world().get::<DriftingCorpse>(body).is_none(), "ejected with no airlock");
        assert!(app.world().get::<ChildOf>(body).is_some(), "the body left the ship anyway");
        assert!(app.world().resource::<DriftingDead>().bodies.is_empty());
    }

    /// Nobody carries a body across the deck while the ship is being shot at.
    #[test]
    fn burials_wait_for_the_shooting_to_stop() {
        let mut app = burial_app();
        let (_, hand, _) = ship_with_a_body(&mut app);
        app.world_mut().spawn((
            AiShip,
            Transform::from_xyz(crate::crew::walking::COMBAT_RANGE * 0.5, 0.0, 0.0),
        ));

        for _ in 0..120 {
            tick(&mut app, 0.1);
        }

        assert!(
            app.world().get::<BurialDetail>(hand).is_none(),
            "a burial detail was sent out mid-battle"
        );
        assert!(app.world().resource::<DriftingDead>().bodies.is_empty());
    }

    #[test]
    fn ids_never_collide_with_restored_ones() {
        let mut dead = DriftingDead::default();
        dead.bodies.push(DriftingBody {
            id: 7,
            system_id: 0,
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            rotation: 0.0,
            name: "Loaded".into(),
        });
        // A save restores rows without the counter that produced them.
        dead.reseed();
        let fresh = dead.add(0, Vec2::ZERO, Vec2::ZERO, 0.0, "New".into());
        assert_eq!(fresh, 8, "a new body reused a restored id");
    }

    /// The dead are never tidied away. If this test ever has to change, it
    /// should be because a scavenger faction exists to do it in fiction.
    #[test]
    fn nothing_prunes_the_dead() {
        let mut dead = DriftingDead::default();
        for i in 0..500 {
            dead.add(i % 4, Vec2::splat(i as f32), Vec2::ZERO, 0.0, String::new());
        }
        assert_eq!(dead.bodies.len(), 500);
    }
}
