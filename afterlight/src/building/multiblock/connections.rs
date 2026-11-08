use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};
use std::collections::{VecDeque};
use crate::components::Module;
use super::components::*;

// ============================================================================
// CONNECTION DETECTION SYSTEM
// Two-pass approach to avoid borrow conflicts.
// Pass 1: BFS to determine connections (read-only positions).
// Pass 2: Apply connection data (mutable writes).
// ============================================================================

/// Connection data computed during BFS
struct ConnectionResult {
    entity: Entity,
    core: Entity,
    distance: u32,
    prev: Option<Entity>,
    next: Option<Entity>,
    role: BlockRole,
}

/// Rebuild a ship's machine connections when its layout changes.
///
/// Per ship and only on change (`building::layout`). It used to rebuild every
/// machine on every ship every frame -- five maps, a BFS per core, and an
/// insert-or-remove command per machine block -- whether or not anything had
/// moved. FtD-style guns will be many blocks each, so this has to scale with
/// what changed, not with what exists.
///
/// The position lookup is keyed by (SHIP ROOT, grid cell), not grid cell
/// alone — grid coordinates are ship-local, and every ship/wreck reuses
/// the same small numbers. A global map chained the player's machine
/// cores into machine blocks on wrecks and AI ships that happened to be
/// grid-adjacent in their OWN coordinates: kilometers-long connection
/// lines on screen, and nearby wrecks silently changing the player's
/// composed weapon stats.
pub fn rebuild_machine_connections(
    mut commands: Commands,
    ships: Query<(Entity, &crate::building::layout::LayoutVersion, Option<&Children>)>,
    mut machine_query: Query<(&Module, &mut MachineBlock)>,
    mut core_stats: Query<&mut MachineStats>,
    disconnected: Query<(), With<Disconnected>>,
    epoch: Res<crate::building::layout::LayoutEpoch>,
    mut seen_epoch: Local<Option<crate::building::layout::LayoutEpoch>>,
    mut built: Local<crate::building::layout::BuiltFrom>,
) {
    if *seen_epoch == Some(*epoch) {
        return;
    }
    *seen_epoch = Some(*epoch);

    for (ship, version, children) in ships.iter() {
        if !built.is_stale(ship, version) {
            continue;
        }
        built.mark(ship, version);
        let Some(children) = children else { continue };
        rebuild_ship_machines(&mut commands, children, &mut machine_query, &mut core_stats, &disconnected);
    }
    built.retain(|e| ships.contains(e));
}

fn rebuild_ship_machines(
    commands: &mut Commands,
    children: &Children,
    machine_query: &mut Query<(&Module, &mut MachineBlock)>,
    core_stats: &mut Query<&mut MachineStats>,
    disconnected: &Query<(), With<Disconnected>>,
) {
    // This ship's machine blocks, by cell. Grid cells are ship-local, which
    // is why this is per ship: one global map once chained the player's
    // cores into blocks on wrecks that sat at the same local numbers --
    // kilometres-long connection lines on screen.
    let mut pos_to_entity: HashMap<IVec2, Entity> = HashMap::new();
    let mut entity_roles: HashMap<Entity, BlockRole> = HashMap::new();
    let mut cores: Vec<(Entity, IVec2)> = Vec::new();
    for child in children.iter() {
        let Ok((module, block)) = machine_query.get(child) else { continue };
        pos_to_entity.insert(module.grid_position, child);
        entity_roles.insert(child, block.role);
        if block.role == BlockRole::Core {
            cores.push((child, module.grid_position));
        }
    }
    if entity_roles.is_empty() {
        return;
    }

    // === PASS 1: BFS to compute connections ===
    let mut connections: Vec<ConnectionResult> = Vec::new();
    let mut connected: HashSet<Entity> = HashSet::new();
    let mut core_counts: HashMap<Entity, MachineStats> = HashMap::new();

    for &(core_entity, core_pos) in &cores {
        connected.insert(core_entity);
        core_counts.insert(core_entity, MachineStats::default());
        connections.push(ConnectionResult {
            entity: core_entity,
            core: core_entity,
            distance: 0,
            prev: None,
            next: None,
            role: BlockRole::Core,
        });

        let mut queue: VecDeque<(Entity, IVec2, u32, Entity)> = VecDeque::new();
        let mut visited: HashSet<Entity> = HashSet::new();
        visited.insert(core_entity);

        for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let adj_pos = core_pos + offset;
            if let Some(&adj_entity) = pos_to_entity.get(&adj_pos) {
                if adj_entity != core_entity {
                    queue.push_back((adj_entity, adj_pos, 1, core_entity));
                }
            }
        }

        while let Some((entity, pos, distance, prev_entity)) = queue.pop_front() {
            if !visited.insert(entity) { continue; }
            connected.insert(entity);

            let role = entity_roles.get(&entity).copied().unwrap_or(BlockRole::Core);
            connections.push(ConnectionResult {
                entity,
                core: core_entity,
                distance,
                prev: Some(prev_entity),
                next: None, // set in pass 2
                role,
            });

            if let Some(stats) = core_counts.get_mut(&core_entity) {
                match role {
                    BlockRole::Barrel => stats.barrel_count += 1,
                    BlockRole::AmmoFeed => stats.feed_count += 1,
                    BlockRole::Cooling => stats.cooling_count += 1,
                    BlockRole::FuelRod => stats.fuel_rod_count += 1,
                    BlockRole::Nozzle => stats.nozzle_count += 1,
                    BlockRole::ShieldEmitter => stats.emitter_count += 1,
                    BlockRole::Core => {}
                }
            }

            // Chainable blocks (barrels, nozzles) extend the machine.
            if role.can_chain() {
                for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                    let adj_pos = pos + offset;
                    if let Some(&adj_entity) = pos_to_entity.get(&adj_pos) {
                        if !visited.contains(&adj_entity) {
                            queue.push_back((adj_entity, adj_pos, distance + 1, entity));
                        }
                    }
                }
            }
        }
    }

    let mut prev_to_next: HashMap<Entity, Entity> = HashMap::new();
    for conn in &connections {
        if let Some(prev) = conn.prev {
            prev_to_next.insert(prev, conn.entity);
        }
    }

    // === PASS 2: Apply connection data ===
    for conn in &connections {
        if let Ok((_, mut block)) = machine_query.get_mut(conn.entity) {
            block.connected_core = Some(conn.core);
            block.chain_distance = conn.distance;
            block.prev_in_chain = conn.prev;
            block.next_in_chain = prev_to_next.get(&conn.entity).copied();
        }
    }

    // Block counts on each core. calculate_machine_stats derives the
    // effective_* numbers from these every frame, from scratch.
    for (core_entity, counts) in &core_counts {
        if let Ok(mut stats) = core_stats.get_mut(*core_entity) {
            stats.barrel_count = counts.barrel_count;
            stats.feed_count = counts.feed_count;
            stats.cooling_count = counts.cooling_count;
            stats.fuel_rod_count = counts.fuel_rod_count;
            stats.nozzle_count = counts.nozzle_count;
            stats.emitter_count = counts.emitter_count;
        }
    }

    // Mark disconnected blocks -- and only touch the marker when it changes.
    for (&entity, _) in &entity_roles {
        let is_disconnected = disconnected.contains(entity);
        if !connected.contains(&entity) {
            if let Ok((_, mut block)) = machine_query.get_mut(entity) {
                block.connected_core = None;
                block.chain_distance = 0;
                block.next_in_chain = None;
                block.prev_in_chain = None;
            }
            if !is_disconnected {
                commands.entity(entity).try_insert(Disconnected);
            }
        } else if is_disconnected {
            commands.entity(entity).try_remove::<Disconnected>();
        }
    }
}

/// Calculate barrel stress — blocks closer to core bear more load
pub fn calculate_barrel_stress(
    block_query: Query<(Entity, &MachineBlock)>,
    mut stress_query: Query<&mut BarrelStress>,
    cascade_query: Query<&CascadeRisk>,
) {
    // Group barrels by core, find max distance
    let mut core_max_distance: HashMap<Entity, u32> = HashMap::new();
    for (_, block) in block_query.iter() {
        if block.role == BlockRole::Barrel {
            if let Some(core) = block.connected_core {
                let max = core_max_distance.entry(core).or_insert(0);
                *max = (*max).max(block.chain_distance);
            }
        }
    }

    for (entity, block) in block_query.iter() {
        if block.role != BlockRole::Barrel || block.connected_core.is_none() { continue; }

        let Ok(mut stress) = stress_query.get_mut(entity) else { continue };
        let Some(core_entity) = block.connected_core else { continue; };
        let max_dist = core_max_distance.get(&core_entity).copied().unwrap_or(1);

        stress.load = max_dist.saturating_sub(block.chain_distance) + 1;

        let base_chance = cascade_query.get(entity)
            .map(|c| c.cascade_chance)
            .unwrap_or(0.15);

        let stress_mult = 1.0 + (stress.load as f32 - 1.0) * 0.5;
        stress.effective_cascade_chance = (base_chance * stress_mult).min(0.8);
    }
}

#[cfg(test)]
mod connection_tests {
    use super::*;
    use crate::components::{ModuleType, Rotation, Ship};

    fn module_at(module_type: ModuleType, x: i32) -> Module {
        Module {
            module_type,
            health: 100.0,
            max_health: 100.0,
            power_consumption: 0.0,
            power_generation: 0.0,
            is_active: true,
            grid_position: IVec2::new(x, 0),
            size: IVec2::ONE,
            rotation: Rotation::North,
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(crate::building::layout::LayoutPlugin);
        app.add_systems(Update, rebuild_machine_connections);
        app
    }

    /// A cannon at x=0 with barrels at x=1..=3, on one ship.
    fn cannon(app: &mut App) -> (Entity, Entity, Vec<Entity>) {
        let ship = app.world_mut().spawn(Ship).id();
        let core = app.world_mut().spawn((
            module_at(ModuleType::Cannon, 0),
            MachineBlock { role: BlockRole::Core, ..default() },
            MachineStats::default(),
            ChildOf(ship),
        )).id();
        let barrels = (1..=3).map(|x| app.world_mut().spawn((
            module_at(ModuleType::BarrelExtension, x),
            MachineBlock { role: BlockRole::Barrel, ..default() },
            ChildOf(ship),
        )).id()).collect();
        (ship, core, barrels)
    }

    #[test]
    fn a_core_counts_the_barrels_chained_to_it() {
        let mut app = app();
        let (_, core, barrels) = cannon(&mut app);
        app.update();
        assert_eq!(app.world().get::<MachineStats>(core).unwrap().barrel_count, 3);
        let tip = app.world().get::<MachineBlock>(barrels[2]).unwrap();
        assert_eq!(tip.connected_core, Some(core));
        assert_eq!(tip.chain_distance, 3);
    }

    #[test]
    fn losing_a_middle_barrel_cuts_off_the_one_beyond_it() {
        let mut app = app();
        let (_, core, barrels) = cannon(&mut app);
        app.update();
        app.world_mut().entity_mut(barrels[1]).despawn();
        app.update();
        app.update(); // the Disconnected marker lands with the command flush
        assert_eq!(app.world().get::<MachineStats>(core).unwrap().barrel_count, 1);
        assert!(app.world().get::<Disconnected>(barrels[2]).is_some(), "the tip is now orphaned");
        assert_eq!(app.world().get::<MachineBlock>(barrels[2]).unwrap().connected_core, None);
        assert!(app.world().get::<Disconnected>(barrels[0]).is_none());
    }

    #[test]
    fn an_unchanged_ship_is_not_rebuilt() {
        let mut app = app();
        let (_, core, _) = cannon(&mut app);
        app.update();
        // Scribble on the result; with no layout change it must survive.
        app.world_mut().get_mut::<MachineStats>(core).unwrap().barrel_count = 99;
        app.update();
        assert_eq!(app.world().get::<MachineStats>(core).unwrap().barrel_count, 99);
    }
}
