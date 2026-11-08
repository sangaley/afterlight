use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};
use std::collections::{VecDeque};

use crate::combat::shields::ShipShield;
use crate::components::*;

use super::components::{AiShip, AiShipType, OwnedByAiShip};

/// Per-ship power balance for AI ships — the AI-side mirror of the player's
/// PowerGraph/PowerState (ship/power.rs), computed independently per AI
/// ship root. Kept as a separate component rather than folding into a
/// shared refactor of the player's PowerState/PowerGraph: those are a
/// global Resource consumed by ~10 player-only combat/UI systems, and a
/// single shared instance can only ever represent one ship (that's the
/// documented reason build_power_graph/update_power_system are hard player-
/// scoped in the first place) — reworking them into per-entity data would
/// have touched every one of those consumers for a feature that only needs
/// AI ships to gain power simulation, not the player's to change shape.
#[derive(Component, Default)]
pub struct AiPowerState {
    pub power_balance: f32,
}

/// Rebuilds every AI ship's power graph and balance each frame: same BFS-
/// from-generators-through-conductive-tiles algorithm as build_power_graph
/// (ship/power.rs), looped per AI root instead of hard-coded to the
/// player's singleton Ship. Grid positions are ship-local, so each ship's
/// BFS must stay scoped to its own modules/hull — mixing them would corrupt
/// every ship's graph, exactly why the player's version is scoped too.
/// Which cells of an AI ship its reactors can reach, and the layout version
/// that was worked out from. The flood fill only reruns when the ship's
/// layout changes; the power balance on top of it is summed every frame,
/// because crew efficiency and is_active move all the time.
#[derive(Component, Default)]
pub struct AiPowerReach {
    built_from: Option<u64>,
    powered_tiles: bevy::platform::collections::HashSet<IVec2>,
}

pub fn update_ai_power(
    mut commands: Commands,
    mut ai_ships: Query<
        (
            Entity,
            &AiShipType,
            Option<&ShipShield>,
            &crate::building::layout::LayoutVersion,
            Option<&mut AiPowerReach>,
            Option<&mut AiPowerState>,
        ),
        With<AiShip>,
    >,
    module_query: Query<(&Module, &OwnedByAiShip, Option<&ModuleEfficiency>)>,
    hull_query: Query<(&HullSegment, &OwnedByAiShip)>,
) {
    let mut by_ship: HashMap<Entity, Vec<(&Module, Option<&ModuleEfficiency>)>> = HashMap::new();
    for (module, owned, eff) in module_query.iter() {
        by_ship.entry(owned.root).or_default().push((module, eff));
    }
    // Hull only matters to a ship whose reach is being rebuilt.
    let stale: HashSet<Entity> = ai_ships
        .iter()
        .filter(|(_, _, _, v, reach, _)| reach.as_ref().is_none_or(|r| r.built_from != Some(v.0)))
        .map(|(e, ..)| e)
        .collect();
    let mut hull_by_ship: HashMap<Entity, Vec<IVec2>> = HashMap::new();
    if !stale.is_empty() {
        for (hull, owned) in hull_query.iter() {
            if stale.contains(&owned.root) {
                hull_by_ship.entry(owned.root).or_default().push(hull.grid_position);
            }
        }
    }

    for (ai_root, ship_type, shield, version, mut reach, power_state) in ai_ships.iter_mut() {
        let Some(modules) = by_ship.get(&ai_root) else { continue };
        let mut fresh: Option<AiPowerReach> = None;
        if reach.as_ref().is_none_or(|r| r.built_from != Some(version.0)) {
            let tiles = flood_power(modules, hull_by_ship.get(&ai_root).map_or(&[][..], |h| &h[..]));
            match reach.as_mut() {
                Some(r) => {
                    r.built_from = Some(version.0);
                    r.powered_tiles = tiles;
                }
                None => fresh = Some(AiPowerReach { built_from: Some(version.0), powered_tiles: tiles }),
            }
        }
        let powered_tiles = match (&reach, &fresh) {
            (Some(r), _) => &r.powered_tiles,
            (None, Some(f)) => &f.powered_tiles,
            (None, None) => continue,
        };
        let mut total_generation = 0.0;
        let mut total_consumption = 0.0;
        // Shield upkeep — AI shields are always enabled (ShipShield's own
        // doc comment), so this is a flat tax whenever a shield exists.
        if shield.is_some_and(|s| s.enabled) {
            total_consumption += crate::combat::shields::SHIELD_UPKEEP_POWER;
        }
        for (module, eff) in modules {
            if !module.is_active {
                continue;
            }
            let efficiency = effective_efficiency(module, *eff);
            if module.power_generation > 0.0 {
                total_generation += module.power_generation * efficiency * super::components::power_output_multiplier(*ship_type);
                continue;
            }
            if powered_tiles.contains(&module.grid_position) {
                total_consumption += module.power_consumption * efficiency;
            }
        }

        // Guarantee every enemy ship is adequately powered: its balance is
        // never allowed to go negative, so an undercrewed reactor or one
        // taking fire can't brown out the whole ship and silence EVERY weapon
        // at once (the hard power gate in ai_ship/combat.rs). Power is no
        // longer a reason an enemy suddenly stops shooting — its utilities,
        // life support, and guns always have what they need. A genuine surplus
        // still shows through as a positive value; only real deficits are
        // clamped away. (This makes the weak-faction power_output_multiplier
        // derate purely cosmetic now — it thins surplus but can't dark a gun.)
        let power_balance = (total_generation - total_consumption).max(0.0);
        match power_state {
            Some(mut state) => state.power_balance = power_balance,
            None => { commands.entity(ai_root).try_insert(AiPowerState { power_balance }); }
        }
        if let Some(f) = fresh {
            commands.entity(ai_root).try_insert(f);
        }
    }
}

/// Cells reachable from any live reactor through live modules and hull.
fn flood_power(
    modules: &[(&Module, Option<&ModuleEfficiency>)],
    hull: &[IVec2],
) -> bevy::platform::collections::HashSet<IVec2> {
    let mut conductive_tiles: bevy::platform::collections::HashSet<IVec2> = hull.iter().copied().collect();
    let mut power_sources: Vec<IVec2> = Vec::new();
    for (module, _eff) in modules {
        if module.health <= 0.0 {
            continue;
        }
        let footprint = crate::building::footprints::footprint_override(module.module_type);
        let cells = crate::building::GridOccupancy::cells_for(
            module.grid_position, module.size, module.rotation, footprint,
        );
        for cell in &cells {
            conductive_tiles.insert(*cell);
        }
        if module.power_generation > 0.0 {
            power_sources.extend(cells);
        }
    }
    let mut visited: bevy::platform::collections::HashSet<IVec2> = bevy::platform::collections::HashSet::new();
    let mut queue: VecDeque<IVec2> = VecDeque::new();
    for pos in power_sources {
        if visited.insert(pos) {
            queue.push_back(pos);
        }
    }
    let mut powered_tiles = bevy::platform::collections::HashSet::new();
    while let Some(current) = queue.pop_front() {
        powered_tiles.insert(current);
        for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let neighbor = current + offset;
            if !visited.contains(&neighbor) && conductive_tiles.contains(&neighbor) {
                visited.insert(neighbor);
                queue.push_back(neighbor);
            }
        }
    }
    powered_tiles
}
