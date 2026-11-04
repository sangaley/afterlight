use bevy::prelude::*;
use crate::components::*;
use crate::events::*;
use crate::resources::*;
use crate::building::{GridOccupancy, ModuleRegistry};
use crate::building::footprints;

// ============================================================================
// GHOST REBUILD — the ship remembers its own design.
//
// When a player block is destroyed (queued for removal), a ghost of what
// stood there is recorded: faint outline sprites on the hull, an entry in
// the RebuildQueue. Idle crew reconstruct ghosts one at a time in flight,
// paying ScrapMetal per block. Docking and rebuilding manually over a ghost
// cell simply clears it.
// ============================================================================

/// Seconds of work (at one idle crew) to rebuild a hull block.
const HULL_WORK: f32 = 5.0;
/// Seconds of work (at one idle crew) to rebuild a module.
const MODULE_WORK: f32 = 10.0;
/// ScrapMetal per hull block.
const HULL_SCRAP: u32 = 2;
/// Module scrap cost: max(2, cost / this divisor).
const MODULE_SCRAP_DIVISOR: u32 = 20;

#[derive(Clone, Copy, Debug)]
pub enum GhostKind {
    Hull { layer: HullLayer, material: HullMaterial },
    Module { module_type: ModuleType, rotation: Rotation },
}

pub struct GhostBlock {
    pub grid_position: IVec2,
    pub kind: GhostKind,
    /// Every grid cell the block covered (multi-cell modules included) —
    /// cleanup drops the ghost if ANY of them gets built over manually.
    pub cells: Vec<IVec2>,
    /// Seconds of crew work accumulated.
    pub progress: f32,
    pub work_needed: f32,
    pub scrap_cost: u32,
    /// Scrap is paid once, when work on this ghost begins.
    pub scrap_paid: bool,
    /// Ghost outline sprites (children of the ship, or of the turret a
    /// turret ghost stands on).
    pub sprites: Vec<Entity>,
    /// Stood on the turret over the ring at this hull cell; `grid_position`
    /// and `cells` are then in that turret's own grid.
    pub turret: Option<IVec2>,
    /// What the block was set to -- tuning, fire group, ammo, a gun's shell --
    /// so the crew rebuild the same gun, not a fresh one.
    pub extras: Option<crate::building::blueprint::ModuleExtras>,
}

/// A destroyed block's settings, read before it goes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct KeptSettings<'w, 's> {
    tuning: Query<'w, 's, &'static crate::building::customization::tuning::WeaponTuning>,
    group: Query<'w, 's, &'static crate::combat::targeting::fire_groups::FireGroup>,
    ammo: Query<'w, 's, &'static crate::building::customization::tuning::SelectedAmmo>,
    shell: Query<'w, 's, &'static crate::combat::shell::GunShell>,
    missile: Query<'w, 's, &'static crate::combat::missile_design::MissileLoadout>,
    pd: Query<'w, 's, (), With<crate::combat::targeting::fire_groups::InterceptMode>>,
    signature: Query<'w, 's, &'static crate::diplomacy::signatures::Signature>,
    turrets: Query<'w, 's, (&'static crate::combat::turret_deck::TurretBody, &'static ChildOf)>,
}

impl KeptSettings<'_, '_> {
    fn extras(&self, e: Entity, turret: Option<IVec2>) -> crate::building::blueprint::ModuleExtras {
        crate::building::blueprint::ModuleExtras {
            tuning: self.tuning.get(e).ok().copied().filter(|t| t.velocity != 1.0 || t.fire_rate != 1.0 || t.damage != 1.0),
            fire_group: self.group.get(e).ok().map(|g| g.group).filter(|&g| g != 0),
            ammo: self.ammo.get(e).ok().copied().filter(|a| a.0 != crate::combat::ammo_types::KineticAmmoType::AP),
            turret,
            shell: self.shell.get(e).ok().map(|s| s.0.clone()),
            missile: self.missile.get(e).ok().map(|m| m.0.clone()),
            point_defence: self.pd.contains(e),
            signature: self.signature.get(e).ok().map(|s| s.0.id.to_string()),
        }
    }
}

#[derive(Resource, Default)]
pub struct RebuildQueue {
    pub ghosts: Vec<GhostBlock>,
}

/// Marker for ghost outline sprites.
#[derive(Component)]
pub struct RebuildGhostSprite;

fn ghost_name(kind: &GhostKind) -> String {
    match kind {
        GhostKind::Hull { material, .. } => format!("{} hull", material.name()),
        GhostKind::Module { module_type, .. } => module_type.name().to_string(),
    }
}

/// Records a rebuild ghost for every PLAYER block queued for destruction
/// removal. AI ships get no ghosts — their wrecks are salvage, not repairs.
pub fn record_rebuild_ghosts(
    mut commands: Commands,
    fresh: Query<
        (Entity, &ChildOf, Option<&Module>, Option<&HullSegment>),
        Added<PendingRemoval>,
    >,
    ship_query: Query<Entity, With<Ship>>,
    registry: Res<ModuleRegistry>,
    mut queue: ResMut<RebuildQueue>,
    kept: KeptSettings,
) {
    let Ok(ship) = ship_query.single() else { return };

    for (entity, parent, module, hull) in fresh.iter() {
        // On the hull, or on one of our turrets' decks -- then the ghost
        // lives in that turret's grid and turns with it.
        let turret = if parent.parent() == ship {
            None
        } else {
            match kept.turrets.get(parent.parent()) {
                Ok((t, on)) if on.parent() == ship => Some(t.ring_cell),
                _ => continue,
            }
        };

        let (grid_position, kind, work_needed, scrap_cost, cells, color) =
            if let Some(module) = module {
                let def = registry.get(module.module_type);
                let footprint = footprints::footprint_override(module.module_type);
                let cells = GridOccupancy::cells_for(
                    module.grid_position, def.size, module.rotation, footprint,
                );
                (
                    module.grid_position,
                    GhostKind::Module {
                        module_type: module.module_type,
                        rotation: module.rotation,
                    },
                    MODULE_WORK,
                    (def.cost / MODULE_SCRAP_DIVISOR).max(2),
                    cells,
                    def.color,
                )
            } else if let Some(hull) = hull {
                let cells = GridOccupancy::cells_for(
                    hull.grid_position, IVec2::ONE, Rotation::North, None,
                );
                (
                    hull.grid_position,
                    GhostKind::Hull {
                        layer: hull.hull_layer,
                        material: hull.material,
                    },
                    HULL_WORK,
                    HULL_SCRAP,
                    cells,
                    Color::WHITE,
                )
            } else {
                continue;
            };

        // One ghost per origin cell — repeated destruction of the same spot
        // (e.g. rebuilt then shot off again) must not stack entries.
        if queue.ghosts.iter().any(|g| g.grid_position == grid_position && g.turret == turret) {
            continue;
        }
        let extras = module.map(|_| kept.extras(entity, turret)).filter(|x| !x.is_empty());

        // Faint outline where the block stood — subtle, reads as a memory
        let ghost_color = color.with_alpha(0.15);
        let mut sprites = Vec::new();
        for &cell in &cells {
            let sprite = commands.spawn((
                (Sprite {
                        color: ghost_color,
                        custom_size: Some(Vec2::splat(58.0)),
                        ..default()
                    }, Transform::from_xyz(
                        cell.x as f32 * 66.0,
                        // A turret's grid has no half-cell offset.
                        cell.y as f32 * 66.0 - if turret.is_some() { 0.0 } else { 33.0 },
                        if turret.is_some() { 0.5 } else { 0.05 },
                    )),
                RebuildGhostSprite,
                ChildOf(parent.parent()),
            )).id();
            sprites.push(sprite);
        }

        queue.ghosts.push(GhostBlock {
            grid_position,
            kind,
            cells: cells.to_vec(),
            progress: 0.0,
            work_needed,
            scrap_cost,
            scrap_paid: false,
            sprites,
            turret,
            extras,
        });
    }
}

/// Supply Crates speed up work this many blocks round them...
pub const CRATES_REACH: i32 = 5;
/// ...by this much (one stack or several, the same).
const CRATES_SPEED: f32 = 1.3;

/// How fast crew put a block back, against 1 for the ordinary case: twice
/// under an Armoured Hatch (combat::gun_house), twice for a Light Frame
/// (girders go up quick), and 30% quicker with Supply Crates to hand.
pub fn rebuild_rate(kind: &GhostKind, hatch: bool, crates_near: bool) -> f32 {
    let mut rate = 1.0;
    if hatch {
        rate *= 2.0;
    }
    if matches!(kind, GhostKind::Module { module_type: ModuleType::LightFrame, .. }) {
        rate *= 2.0;
    }
    if crates_near {
        rate *= CRATES_SPEED;
    }
    rate
}

/// Idle crew reconstruct ghosts one at a time (FIFO), paying scrap up front.
/// Runs while exploring — this is field construction, dock rebuilding stays
/// manual in build mode.
pub fn crew_rebuild_system(
    mut commands: Commands,
    time: Res<Time>,
    // The player's hands only. AI crew are CrewMembers too, spawned Idle and
    // never changed, so every enemy crew in the world was rebuilding our hull.
    crew_query: Query<(Entity, &CrewMember), Without<crate::ai_ship::components::OwnedByAiShip>>,
    stations: Query<&CrewStation>,
    mut queue: ResMut<RebuildQueue>,
    mut inventory: ResMut<Inventory>,
    mut place_hull_events: MessageWriter<PlaceHullRequest>,
    mut place_module_events: MessageWriter<PlaceModuleRequest>,
    mut notifications: MessageWriter<ShowNotification>,
    mut stall_notified: Local<bool>,
    turret_work: (
        Query<Entity, With<Ship>>,
        Query<(&crate::combat::turret_deck::TurretBody, &ChildOf, Option<&crate::combat::gun_house::RoofFittings>)>,
        MessageWriter<crate::combat::turret_deck::RebuildTurretPlate>,
        Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    ),
) {
    let (ship_query, turrets, mut plate_events, modules) = turret_work;
    // A turret ghost whose whole turret is gone has nowhere to go: laid
    // down anyway it would land on the hull at turret-grid cells.
    if let Ok(ship) = ship_query.single() {
        queue.ghosts.retain(|g| {
            let keep = g.turret.is_none_or(|rc| turrets.iter().any(|(t, p, _)| p.parent() == ship && t.ring_cell == rc));
            if !keep {
                for &sprite in &g.sprites {
                    commands.entity(sprite).try_despawn();
                }
            }
            keep
        });
    }
    if queue.ghosts.is_empty() {
        *stall_notified = false;
        return;
    }

    // Idle is not "free": a posted hand keeps CrewState::Idle (posting never
    // touches the state), so the reactor's operator was rebuilding too. Only
    // the unposted count.
    let posted: std::collections::HashSet<Entity> =
        stations.iter().filter_map(|s| s.assigned_crew).collect();
    let idle_crew = crew_query.iter()
        .filter(|(e, c)| c.state == CrewState::Idle && c.health > 0.0 && !posted.contains(e))
        .count();
    if idle_crew == 0 {
        return;
    }

    let ghost = &mut queue.ghosts[0];

    // Materials up front: no scrap, no reconstruction.
    if !ghost.scrap_paid {
        if !inventory.remove_item(ItemType::ScrapMetal, ghost.scrap_cost) {
            if !*stall_notified {
                *stall_notified = true;
                notifications.write(ShowNotification {
                    message: format!(
                        "Rebuild waiting on ScrapMetal ({} needed for {})",
                        ghost.scrap_cost,
                        ghost_name(&ghost.kind)
                    ),
                    notification_type: NotificationType::Warning,
                    duration: 4.0,
                });
            }
            return;
        }
        ghost.scrap_paid = true;
        *stall_notified = false;
    }

    // An Armoured Hatch on a turret's roof lets the crew in to the parts
    // under it: they go back twice as fast (combat::gun_house).
    let under_roof = matches!(ghost.kind, GhostKind::Module { module_type, .. } if !crate::combat::gun_house::is_roof_block(module_type));
    let hatch = under_roof
        && ship_query.single().is_ok_and(|ship| {
            ghost.turret.is_some_and(|rc| {
                turrets.iter().any(|(t, p, f)| p.parent() == ship && t.ring_cell == rc && f.is_some_and(|f| f.hatches > 0))
            })
        });
    // Supply Crates on the hull within reach (a turret part counts from its
    // ring).
    let at = ghost.turret.unwrap_or(ghost.grid_position);
    let crates_near = ship_query.single().is_ok_and(|ship| {
        modules.iter().any(|(m, p)| {
            p.parent() == ship
                && m.module_type == ModuleType::SupplyCrates
                && (m.grid_position - at).abs().max_element() <= CRATES_REACH
        })
    });
    ghost.progress += idle_crew as f32 * time.delta_secs() * rebuild_rate(&ghost.kind, hatch, crates_near);
    if ghost.progress < ghost.work_needed {
        return;
    }

    // Done — respawn the block. `free: true`: it was paid for in scrap.
    let ghost = queue.ghosts.remove(0);
    match ghost.kind {
        // A turret's floor tile is the turret's, not the hull's.
        GhostKind::Hull { .. } if ghost.turret.is_some() => {
            if let (Ok(ship), Some(ring_cell)) = (ship_query.single(), ghost.turret) {
                plate_events.write(crate::combat::turret_deck::RebuildTurretPlate { ship, ring_cell, cell: ghost.grid_position });
            }
        }
        GhostKind::Hull { layer, material } => {
            place_hull_events.write(PlaceHullRequest {
                layer,
                material,
                grid_position: ghost.grid_position,
                free: true,
            });
        }
        GhostKind::Module { module_type, rotation } => {
            place_module_events.write(PlaceModuleRequest {
                module_type,
                grid_position: ghost.grid_position,
                rotation,
                custom_name: None,
                subcomponents: None,
                extras: ghost.extras.clone(),
                free: true,
            });
        }
    }
    for sprite in ghost.sprites {
        commands.entity(sprite).try_despawn();
    }
    info!("Crew rebuilt {}{}", ghost_name(&ghost.kind), if ghost.turret.is_some() { " on a turret" } else { "" });
    notifications.write(ShowNotification {
        message: format!(
            "Crew rebuilt {} (-{} scrap)",
            ghost_name(&ghost.kind),
            ghost.scrap_cost
        ),
        notification_type: NotificationType::Success,
        duration: 2.5,
    });
}

/// While docked: drop any ghost whose origin cell got built over manually.
/// (Occupancy only rebuilds while docked, so this check is gated there.)
pub fn cleanup_built_over_ghosts(
    mut commands: Commands,
    occupancy: Res<GridOccupancy>,
    mut queue: ResMut<RebuildQueue>,
) {
    queue.ghosts.retain(|ghost| {
        // Turret ghosts are in a turret's grid, not the hull's.
        if ghost.turret.is_none() && ghost.cells.iter().any(|c| occupancy.cells.contains_key(c)) {
            for &sprite in &ghost.sprites {
                commands.entity(sprite).try_despawn();
            }
            false
        } else {
            true
        }
    });
}

#[cfg(test)]
mod rebuild_crew_tests {
    use super::*;
    use crate::ai_ship::components::OwnedByAiShip;
    use std::time::Duration;

    fn hand() -> CrewMember {
        CrewMember {
            name: "Ash".into(),
            health: 100.0,
            max_health: 100.0,
            oxygen: 100.0,
            morale: 100.0,
            state: CrewState::Idle,
        }
    }

    /// Only our unposted hands work the ghost: not an enemy's crew, and not
    /// the operator standing at a post (who still reads as Idle).
    #[test]
    fn only_our_free_hands_rebuild() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<Inventory>()
            .add_message::<PlaceHullRequest>()
            .add_message::<PlaceModuleRequest>()
            .add_message::<ShowNotification>()
            .add_message::<crate::combat::turret_deck::RebuildTurretPlate>()
            .add_systems(Update, crew_rebuild_system);
        app.insert_resource(RebuildQueue {
            ghosts: vec![GhostBlock {
                grid_position: IVec2::ZERO,
                kind: GhostKind::Hull { layer: HullLayer::Outer, material: HullMaterial::Steel },
                cells: vec![IVec2::ZERO],
                progress: 0.0,
                work_needed: 100.0,
                scrap_cost: 0,
                scrap_paid: true,
                sprites: Vec::new(),
                turret: None,
                extras: None,
            }],
        });

        let raider = app.world_mut().spawn_empty().id();
        app.world_mut().spawn(hand());
        let operator = app.world_mut().spawn(hand()).id();
        app.world_mut().spawn(CrewStation { priority: 5, assigned_crew: Some(operator), manually_assigned: false });
        for _ in 0..3 {
            app.world_mut().spawn((hand(), OwnedByAiShip { root: raider }));
        }

        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_secs(1));
        app.update();
        let progress = app.world().resource::<RebuildQueue>().ghosts[0].progress;
        assert!((progress - 1.0).abs() < 1e-4, "one free hand should give 1.0 work/s, got {progress}");
    }
}

#[cfg(test)]
mod turret_rebuild_tests {
    use super::*;
    use crate::combat::shell::{GunShell, ShellDesign, ShellHead};
    use crate::combat::turret_deck::{RebuildTurretPlate, TurretBody};
    use std::time::Duration;

    fn app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<Inventory>()
            .init_resource::<RebuildQueue>()
            .insert_resource(crate::building::registry::build_registry())
            .add_message::<PlaceHullRequest>()
            .add_message::<PlaceModuleRequest>()
            .add_message::<ShowNotification>()
            .add_message::<RebuildTurretPlate>()
            .add_systems(Update, (record_rebuild_ghosts, crew_rebuild_system).chain());
        let ship = app.world_mut().spawn(Ship).id();
        let body = app.world_mut().spawn((TurretBody::new(Entity::PLACEHOLDER, IVec2::new(16, 1), 3), ChildOf(ship))).id();
        app.world_mut().spawn(CrewMember {
            name: "Ash".into(), health: 100.0, max_health: 100.0, oxygen: 100.0, morale: 100.0, state: CrewState::Idle,
        });
        (app, ship, body)
    }

    fn part(t: ModuleType, x: i32, y: i32) -> Module {
        Module {
            module_type: t, health: 0.0, max_health: 100.0, power_consumption: 0.0, power_generation: 0.0,
            is_active: false, grid_position: IVec2::new(x, y), size: IVec2::ONE, rotation: Rotation::East,
        }
    }

    fn lose(app: &mut App, e: Entity) {
        app.world_mut().entity_mut(e).insert(PendingRemoval { timer: Timer::from_seconds(0.5, TimerMode::Once) });
    }

    fn work(app: &mut App, secs: f32) {
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_secs_f32(secs));
        app.update();
    }

    #[test]
    fn a_lost_turret_gun_is_rebuilt_on_its_turret_with_its_shell() {
        let (mut app, _, body) = app();
        let shell = ShellDesign { head: ShellHead::He, ..default() };
        let fp = app.world_mut().spawn((
            part(ModuleType::FiringPiece, 0, 0),
            GunShell(shell.clone()),
            ChildOf(body),
        )).id();
        lose(&mut app, fp);
        app.update();
        let q = app.world().resource::<RebuildQueue>();
        assert_eq!(q.ghosts.len(), 1);
        assert_eq!(q.ghosts[0].turret, Some(IVec2::new(16, 1)));
        let sprite = q.ghosts[0].sprites[0];
        assert_eq!(app.world().get::<ChildOf>(sprite).unwrap().parent(), body, "the outline turns with the turret");
        app.world_mut().resource_mut::<RebuildQueue>().ghosts[0].scrap_paid = true;
        work(&mut app, MODULE_WORK + 1.0);
        let placed: Vec<_> = app.world_mut().resource_mut::<Messages<PlaceModuleRequest>>().drain().collect();
        assert_eq!(placed.len(), 1);
        let extras = placed[0].extras.as_ref().expect("the gun's settings come back");
        assert_eq!(extras.turret, Some(IVec2::new(16, 1)));
        assert_eq!(extras.shell, Some(shell));
    }

    #[test]
    fn frames_go_up_quick_and_crates_help() {
        let plate = GhostKind::Module { module_type: ModuleType::ArmorPlate, rotation: Rotation::North };
        let frame = GhostKind::Module { module_type: ModuleType::LightFrame, rotation: Rotation::North };
        assert_eq!(rebuild_rate(&plate, false, false), 1.0);
        assert_eq!(rebuild_rate(&frame, false, false), 2.0);
        assert!((rebuild_rate(&plate, false, true) - 1.3).abs() < 1e-6);
        assert!((rebuild_rate(&frame, true, true) - 5.2).abs() < 1e-6, "everything stacks");
    }

    #[test]
    fn a_lost_turret_plate_comes_back_as_a_plate() {
        let (mut app, ship, body) = app();
        let plate = app.world_mut().spawn((
            HullSegment { grid_position: IVec2::new(1, -2), ..default() },
            crate::combat::turret_deck::TurretPlate,
            ChildOf(body),
        )).id();
        lose(&mut app, plate);
        app.update();
        app.world_mut().resource_mut::<RebuildQueue>().ghosts[0].scrap_paid = true;
        work(&mut app, HULL_WORK + 1.0);
        let plates: Vec<_> = app.world_mut().resource_mut::<Messages<RebuildTurretPlate>>().drain().collect();
        assert_eq!(plates.len(), 1);
        assert_eq!((plates[0].ship, plates[0].ring_cell, plates[0].cell), (ship, IVec2::new(16, 1), IVec2::new(1, -2)));
        assert!(app.world_mut().resource_mut::<Messages<PlaceHullRequest>>().drain().next().is_none(), "not hull plating");
    }

    #[test]
    fn with_its_turret_gone_a_turret_ghost_is_dropped() {
        let (mut app, _, body) = app();
        let part = app.world_mut().spawn((part(ModuleType::Loader, -1, 0), ChildOf(body))).id();
        lose(&mut app, part);
        app.update();
        assert_eq!(app.world().resource::<RebuildQueue>().ghosts.len(), 1);
        app.world_mut().entity_mut(body).despawn();
        app.update();
        assert!(app.world().resource::<RebuildQueue>().ghosts.is_empty());
    }
}
