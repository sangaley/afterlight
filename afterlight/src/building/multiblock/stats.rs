use bevy::prelude::*;
use crate::components::{Module, Weapon};
use crate::building::customization::parameters::ModuleCustomization;
use super::components::*;

// ============================================================================
// STAT CALCULATION FROM PHYSICAL LAYOUT
// Weapon stats emerge from the number and type of connected blocks.
// No magic numbers from a menu — your ship's physical design IS the stats.
// ============================================================================

/// Base stats per weapon core type (before block modifiers)
pub struct CoreBaseStats {
    pub damage: f32,
    pub range: f32,
    pub fire_rate: f32,
    pub heat_per_shot: f32,
    pub accuracy: f32,
}

/// How much each additional block of a type contributes
pub struct BlockContribution {
    /// Range gained per barrel block
    pub range_per_barrel: f32,
    /// Accuracy gained per barrel block (diminishing)
    pub accuracy_per_barrel: f32,
    /// Damage multiplier per barrel block (compound)
    pub damage_mult_per_barrel: f32,
    /// Fire rate multiplier per feed block
    pub fire_rate_mult_per_feed: f32,
    /// Heat capacity gained per cooling block
    pub heat_capacity_per_cooling: f32,
}

/// Default contributions for weapon types
fn weapon_block_contributions() -> BlockContribution {
    BlockContribution {
        range_per_barrel: 60.0,         // 60 range per barrel (was 80 — 4 barrels = +240 range)
        accuracy_per_barrel: 0.06,       // Diminishing returns keep max ~92%
        damage_mult_per_barrel: 0.12,    // 12% per barrel (4 barrels = +48% damage)
        fire_rate_mult_per_feed: 0.20,   // 20% per feed (2 feeds = +40% fire rate)
        heat_capacity_per_cooling: 40.0, // 40 per cooling block
    }
}

/// Recalculate machine stats from connected blocks.
/// Runs after connection detection.
pub fn calculate_machine_stats(
    mut core_query: Query<(Entity, &Module, &MachineBlock, &mut MachineStats)>,
    base_stats_query: Query<&BaseWeaponStats>,
    customization_query: Query<&ModuleCustomization>,
    _block_query: Query<(&MachineBlock, Option<&ModuleCustomization>)>,
) {
    let contrib = weapon_block_contributions();

    for (entity, _module, block, mut stats) in core_query.iter_mut() {
        if block.role != BlockRole::Core {
            continue;
        }

        // Base stats come from the spawn-time snapshot, never from the live
        // Weapon component — see BaseWeaponStats docs for why (this used to
        // read weapon.range here, which apply_machine_stats_to_weapons had
        // just overwritten with *last frame's* output, compounding every
        // frame toward zero).
        let base = if let Ok(base_stats) = base_stats_query.get(entity) {
            CoreBaseStats {
                damage: base_stats.damage,
                range: base_stats.range,
                fire_rate: base_stats.fire_rate,
                heat_per_shot: 10.0,
                accuracy: 0.7,
            }
        } else {
            continue;
        };

        // === Tier 3: Read core customization parameters ===
        let core_custom = customization_query.get(entity).ok();

        // Barrel parameter: bore_diameter affects damage, barrel_length affects range
        let bore_mult = core_custom
            .and_then(|c| c.parameter_values.get("Barrel.bore_diameter"))
            .map(|&bore| bore / 80.0)  // Normalize: 80mm = default = 1.0x
            .unwrap_or(1.0);

        let length_mult = core_custom
            .and_then(|c| c.parameter_values.get("Barrel.barrel_length"))
            .map(|&len| len / 1200.0)  // Normalize: 1200mm = default = 1.0x
            .unwrap_or(1.0);

        let rifling_bonus = core_custom
            .and_then(|c| c.parameter_values.get("Barrel.rifling_depth"))
            .map(|&r| r / 1.5 * 0.03)  // 1.5mm default = +3% accuracy
            .unwrap_or(0.03);

        // Ammo parameters
        let propellant_mult = core_custom
            .and_then(|c| c.parameter_values.get("Ammunition.propellant_charge"))
            .map(|&p| p / 50.0)  // 50g = default = 1.0x
            .unwrap_or(1.0);

        // Feed mechanism parameters
        let feed_speed_mult = core_custom
            .and_then(|c| c.parameter_values.get("Feed Mechanism.loader_speed"))
            .or_else(|| core_custom.and_then(|c| c.parameter_values.get("Feed Mechanism.belt_speed")))
            .map(|&s| s / 200.0)  // 200 = default = 1.0x
            .unwrap_or(1.0);

        // Cooling parameters
        let cooling_mult = core_custom
            .and_then(|c| c.parameter_values.get("Cooling.coolant_flow_rate"))
            .map(|&f| f / 8.0)  // 8 L/s = default = 1.0x
            .unwrap_or(1.0);

        // Mount parameters
        let stabilization = core_custom
            .and_then(|c| c.parameter_values.get("Mount.stabilization"))
            .map(|&s| s / 100.0 * 0.05)  // 50% default = +2.5% accuracy
            .unwrap_or(0.025);

        // Targeting parameters
        let tracking_bonus = core_custom
            .and_then(|c| c.parameter_values.get("Targeting.prediction_range"))
            .map(|&r| r / 800.0 * 0.04)  // 800 = default = +4% accuracy
            .unwrap_or(0.04);

        // === Calculate stats from physical layout + Tier 3 ===

        let barrels = stats.barrel_count as f32;
        let feeds = stats.feed_count as f32;
        let cooling = stats.cooling_count as f32;

        // Range: base * barrel_length_modifier + per_barrel
        stats.effective_range = (base.range * length_mult) + barrels * contrib.range_per_barrel;

        // Damage: base * bore * propellant * (1 + barrel bonus)
        stats.effective_damage = base.damage * bore_mult * propellant_mult
            * (1.0 + barrels * contrib.damage_mult_per_barrel);

        // Accuracy: base + barrel rifling + stabilization + tracking + diminishing barrel bonus
        let barrel_accuracy: f32 = (0..stats.barrel_count)
            .map(|i| contrib.accuracy_per_barrel / (1.0 + i as f32 * 0.5))
            .sum();
        stats.effective_accuracy = (base.accuracy + rifling_bonus + stabilization + tracking_bonus + barrel_accuracy).min(0.98);

        // Fire rate: base * feed_speed * (1 + feed blocks)
        stats.effective_fire_rate = base.fire_rate * feed_speed_mult
            * (1.0 + feeds * contrib.fire_rate_mult_per_feed);

        // Heat capacity: base + cooling blocks * cooling multiplier
        let base_heat = 100.0;
        stats.heat_capacity = (base_heat + cooling * contrib.heat_capacity_per_cooling) * cooling_mult;

        // === No blocks = core-only stats ===
        if stats.barrel_count == 0 {
            stats.effective_range *= 0.6;
            stats.effective_damage *= 0.7;
            stats.effective_accuracy *= 0.6;
        }
    }
}

/// Apply calculated machine stats back to the Weapon component
/// so existing combat systems use the emergent stats.
pub fn apply_machine_stats_to_weapons(
    core_query: Query<(Entity, &MachineStats, &MachineBlock), Changed<MachineStats>>,
    mut weapon_query: Query<&mut Weapon>,
) {
    for (entity, stats, block) in core_query.iter() {
        if block.role != BlockRole::Core {
            continue;
        }
        if let Ok(mut weapon) = weapon_query.get_mut(entity) {
            weapon.damage = stats.effective_damage;
            weapon.range = stats.effective_range;
            weapon.fire_rate = stats.effective_fire_rate;
        }
    }
}
