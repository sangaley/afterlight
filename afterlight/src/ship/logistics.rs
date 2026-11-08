//! Logistics systems: conveyor tubes and fuel processors.

use bevy::prelude::*;
use crate::components::*;
use crate::building::GridOccupancy;
use crate::resources::PowerGraph;

// ============================================================================
// CONVEYOR TUBE — transfers ammo between adjacent AmmoBay and weapon modules
// ============================================================================

/// Conveyor tubes auto-transfer ammo from adjacent AmmoBay modules to adjacent
/// weapon modules that are low on ammo.
pub fn update_conveyor_tubes(
    time: Res<Time>,
    conveyor_query: Query<(&ConveyorTubeComp, &Module), Without<DestroyedModule>>,
    mut ammo_bays: Query<(&mut AmmoStorage, &Module), (Without<ConveyorTubeComp>, Without<DestroyedModule>)>,
    occupancy: Res<GridOccupancy>,
    power_graph: Res<PowerGraph>,
) {
    let dt = time.delta_secs();

    for (conveyor, conv_module) in conveyor_query.iter() {
        if !conv_module.is_active { continue; }
        if !power_graph.powered_tiles.contains(&conv_module.grid_position) { continue; }

        let transfer_amount = (conveyor.speed * dt * 2.0) as u32; // ~2 ammo/sec at speed 1.0
        if transfer_amount == 0 { continue; }

        // Find adjacent cells
        let adjacent_positions: Vec<IVec2> = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y]
            .iter()
            .map(|&offset| conv_module.grid_position + offset)
            .collect();

        // Find adjacent AmmoBay entities with spare ammo, and weapon entities needing ammo
        let mut source_entities = Vec::new();
        let mut sink_entities = Vec::new();

        for &adj_pos in &adjacent_positions {
            if let Some(&adj_entity) = occupancy.cells.get(&adj_pos) {
                if let Ok((ammo_storage, module)) = ammo_bays.get(adj_entity) {
                    if !module.is_active { continue; }
                    if module.module_type == ModuleType::AmmoBay && ammo_storage.current > 0 {
                        source_entities.push(adj_entity);
                    } else if ammo_storage.current < ammo_storage.capacity {
                        // Weapon with room for ammo
                        sink_entities.push(adj_entity);
                    }
                }
            }
        }

        // Transfer from sources to sinks
        for &sink_entity in &sink_entities {
            for &source_entity in &source_entities {
                if source_entity == sink_entity { continue; }

                // Get both mutably using a combination query
                let Ok([mut source, mut sink]) = ammo_bays.get_many_mut([source_entity, sink_entity]) else {
                    continue;
                };

                let needed = sink.0.capacity.saturating_sub(sink.0.current);
                let transfer = transfer_amount.min(source.0.current).min(needed);
                if transfer > 0 {
                    source.0.current -= transfer;
                    sink.0.current += transfer;
                }
            }
        }
    }
}

// ============================================================================
// FUEL PROCESSOR — reduces fuel consumption of adjacent engines
// ============================================================================

/// Fuel processors reduce the effective fuel consumption of adjacent Engine modules.
/// Each processor reduces consumption by `(1.0 - 1.0/efficiency)` factor.
pub fn update_fuel_processor(
    fuel_processors: Query<(&FuelProcessorComp, &Module), Without<DestroyedModule>>,
    mut engines: Query<(&mut Engine, &Module), (Without<FuelProcessorComp>, Without<DestroyedModule>)>,
    power_graph: Res<PowerGraph>,
) {
    // Reset all engine fuel consumption to base (1.0) first
    for (mut engine, _) in engines.iter_mut() {
        engine.fuel_consumption = 1.0;
    }

    // Apply fuel processor efficiency to adjacent engines
    for (processor, proc_module) in fuel_processors.iter() {
        if !proc_module.is_active { continue; }
        if !power_graph.powered_tiles.contains(&proc_module.grid_position) { continue; }

        let reduction_factor = 1.0 / processor.efficiency; // e.g., 1.0/1.2 = 0.833

        for (mut engine, eng_module) in engines.iter_mut() {
            if !eng_module.is_active { continue; }

            let dist = (eng_module.grid_position - proc_module.grid_position).as_vec2().length();
            if dist <= 1.5 {
                engine.fuel_consumption *= reduction_factor;
            }
        }
    }
}
