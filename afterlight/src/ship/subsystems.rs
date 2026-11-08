//! Systems for companion components: Capacitor, FireSuppression,
//! RadiationShielding, DroneBay, AmmoAutoloader, and Phase B subsystems.

use bevy::prelude::*;
use crate::components::*;
use crate::resources::{PowerState, ResearchState, TargetingBonus};

// ============================================================================
// CAPACITOR — charges from surplus power, discharges on demand
// ============================================================================

/// Charges capacitors when there is surplus power (generation > consumption).
/// Surplus is divided evenly among all active capacitors that aren't full.
pub fn update_capacitors(
    time: Res<Time>,
    power_state: Res<PowerState>,
    mut capacitors: Query<(&mut CapacitorComp, &Module), Without<DestroyedModule>>,
) {
    let dt = time.delta_secs();
    let surplus = power_state.total_power_generation - power_state.total_power_consumption;

    if surplus > 0.0 {
        // Count active capacitors that can accept charge
        let hungry_count = capacitors.iter()
            .filter(|(cap, module)| module.is_active && cap.charge < cap.capacity)
            .count()
            .max(1) as f32;
        let per_cap_surplus = surplus / hungry_count;

        for (mut cap, module) in capacitors.iter_mut() {
            if !module.is_active { continue; }
            if cap.charge < cap.capacity {
                let charge_amount = cap.charge_rate.min(per_cap_surplus) * dt;
                cap.charge = (cap.charge + charge_amount).min(cap.capacity);
            }
        }
    } else {
        // Slowly drain when no surplus (standby loss)
        for (mut cap, module) in capacitors.iter_mut() {
            if !module.is_active { continue; }
            cap.charge = (cap.charge - 0.5 * dt).max(0.0);
        }
    }
}

// ============================================================================
// FIRE SUPPRESSION — auto-extinguishes fires in adjacent modules
// ============================================================================

/// Fire suppression modules reduce fire intensity on nearby burning modules.
/// Checks adjacency via grid position (within 2 cells).
pub fn update_fire_suppression(
    time: Res<Time>,
    suppressors: Query<(&FireSuppressionComp, &Module), Without<DestroyedModule>>,
    mut fires: Query<(&mut OnFire, &Module), (Without<FireSuppressionComp>, Without<DestroyedModule>)>,
) {
    let dt = time.delta_secs();

    for (suppressor, sup_module) in suppressors.iter() {
        if !sup_module.is_active || !suppressor.active {
            continue;
        }

        for (mut fire, fire_module) in fires.iter_mut() {
            let dist = (fire_module.grid_position - sup_module.grid_position).as_vec2().length();
            if dist <= 2.0 {
                fire.intensity -= suppressor.effectiveness * dt * 0.5;
                if fire.intensity <= 0.0 {
                    fire.intensity = 0.0;
                }
            }
        }
    }
}

// ============================================================================
// RADIATION SHIELDING — boosts radiation tolerance of adjacent hull segments
// ============================================================================

/// Radiation shielding modules increase the radiation_shielding of adjacent hull segments.
/// Applied additively each frame (idempotent because hull segments reset to base each tick).
pub fn update_radiation_shielding(
    reinforcements: Query<(&RadiationShieldingComp, &Module), Without<DestroyedModule>>,
    mut hull_segments: Query<&mut HullSegment>,
) {
    // First, reset all hull segments to their base radiation shielding
    for mut hull in hull_segments.iter_mut() {
        hull.radiation_shielding = hull.material.radiation_shielding();
    }

    // Then apply bonuses from active shielding modules
    for (reinforcement, module) in reinforcements.iter() {
        if !module.is_active {
            continue;
        }

        for mut hull in hull_segments.iter_mut() {
            let dist = (hull.grid_position - module.grid_position).as_vec2().length();
            if dist <= 2.5 {
                hull.radiation_shielding += reinforcement.shielding_bonus;
            }
        }
    }
}

// ============================================================================
// DRONE BAY — passive repair effect on nearby damaged modules
// ============================================================================

/// Drone bays provide a slow passive repair effect to nearby damaged modules,
/// simulating deployed repair drones working on the hull.
/// Repair rate scales with the bay module's own health ratio.
pub fn update_drone_bays(
    time: Res<Time>,
    drone_bays: Query<(&DroneBayComp, &Module), Without<DestroyedModule>>,
    mut damaged_modules: Query<&mut Module, (Without<DroneBayComp>, Without<DestroyedModule>)>,
) {
    let dt = time.delta_secs();

    for (drone_bay, bay_module) in drone_bays.iter() {
        if !bay_module.is_active || drone_bay.drone_count == 0 {
            continue;
        }

        // Repair rate scales with the bay's own health
        let health_ratio = if bay_module.max_health > 0.0 {
            (bay_module.health / bay_module.max_health).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let repair_rate = drone_bay.drone_count as f32 * 0.5 * health_ratio;

        for mut target in damaged_modules.iter_mut() {
            if target.health <= 0.0 || target.health >= target.max_health {
                continue;
            }

            let dist = (target.grid_position - bay_module.grid_position).as_vec2().length();
            if dist <= drone_bay.drone_range / 66.0 {
                target.health = (target.health + repair_rate * dt).min(target.max_health);
            }
        }
    }
}

// ============================================================================
// TORPEDO LOADER — boosts fire rate of adjacent torpedo tubes
// ============================================================================

/// AmmoAutoloader modules speed up the reload of adjacent torpedo weapons.
/// Finds torpedo tubes within 1 grid cell and multiplies their cooldown speed.
pub fn apply_torpedo_loader_bonus(
    loader_query: Query<(&AmmoAutoloaderComp, &Module), Without<DestroyedModule>>,
    mut weapon_query: Query<(&mut WeaponCooldown, &Module), (Without<AmmoAutoloaderComp>, Without<DestroyedModule>)>,
) {
    for (loader, loader_module) in loader_query.iter() {
        if !loader_module.is_active { continue; }

        for (mut cooldown, weapon_module) in weapon_query.iter_mut() {
            if !weapon_module.is_active { continue; }

            // Only boost torpedo tubes
            if !matches!(weapon_module.module_type, ModuleType::HeavyMissile | ModuleType::GuidedMissile | ModuleType::ClusterRocket) {
                continue;
            }

            let dist = (weapon_module.grid_position - loader_module.grid_position).as_vec2().length();
            if dist <= 1.5 {
                // Speed up the cooldown timer by the reload bonus factor
                let speedup = 1.0 + loader.reload_bonus;
                // Saturating: a duration shrunk under a running timer would
                // make Timer::remaining() panic. sync_weapon_cooldowns keeps
                // elapsed in range, but this reads every frame from a system
                // that can't afford to be wrong about it.
                let remaining = cooldown.timer.duration().as_secs_f32()
                    - cooldown.timer.elapsed_secs().min(cooldown.timer.duration().as_secs_f32());
                let new_remaining = remaining / speedup;
                let advance = remaining - new_remaining;
                let duration_secs = cooldown.timer.duration().as_secs_f32();
                let new_elapsed = (cooldown.timer.elapsed_secs() + advance).min(duration_secs);
                cooldown.timer.set_elapsed(std::time::Duration::from_secs_f32(new_elapsed));
            }
        }
    }
}

// ============================================================================
// WEAPON POWER ROUTING — routed power scales player reload speed
// ============================================================================

/// Scales the player's weapon reload speed by the routed weapons power. It
/// nudges each cooldown timer by the extra (or missing) fraction of a frame,
/// so effective fire rate ≈ base × weapons_mult: route power to guns and they
/// cycle up to 2× faster; starve them and reloads crawl until they're
/// effectively silent. Mirrors `apply_torpedo_loader_bonus`'s timer-advance
/// trick and runs once per weapon per frame, so the net effect is stable
/// regardless of ordering against the firing systems.
pub fn apply_weapon_power_scaling(
    time: Res<Time>,
    channels: Res<crate::resources::PowerChannels>,
    ship_query: Query<Entity, With<Ship>>,
    mut weapon_query: Query<(&mut WeaponCooldown, &ChildOf), Without<DestroyedModule>>,
) {
    let mult = channels.weapons_mult;
    // Nominal power → leave timers untouched.
    if (mult - 1.0).abs() < 0.01 {
        return;
    }
    let Ok(player) = ship_query.single() else { return };
    let dt = time.delta_secs();
    // >0 speeds the reload up, <0 slows it down.
    let adjust = (mult - 1.0) * dt;

    for (mut cooldown, parent) in weapon_query.iter_mut() {
        if parent.parent() != player {
            continue;
        }
        // A ready weapon has nothing to reload.
        if cooldown.timer.is_finished() {
            continue;
        }
        let duration = cooldown.timer.duration().as_secs_f32();
        let elapsed = (cooldown.timer.elapsed_secs() + adjust).clamp(0.0, duration);
        cooldown.timer.set_elapsed(std::time::Duration::from_secs_f32(elapsed));
    }
}

// ============================================================================
// TARGETING COMPUTER — boosts weapon accuracy
// ============================================================================

/// Sums accuracy bonus from all active targeting computers.
/// Uses diminishing returns: bonus = 1.0 - (1.0 - single_bonus)^count, capped at 0.3.
/// Writes result to TargetingBonus resource for combat systems to read.
pub fn apply_targeting_computer_bonus(
    targeting_query: Query<(&TargetingComputerComp, &Module), Without<DestroyedModule>>,
    mut targeting_bonus: ResMut<TargetingBonus>,
) {
    let mut active_count = 0u32;
    let mut single_bonus = 0.0f32;

    for (tc, module) in targeting_query.iter() {
        if !module.is_active { continue; }
        active_count += 1;
        single_bonus = tc.accuracy_bonus; // all targeting computers have same bonus
    }

    if active_count == 0 {
        targeting_bonus.accuracy_bonus = 0.0;
        return;
    }

    // Diminishing returns: 1 - (1 - bonus)^count
    let combined = 1.0 - (1.0 - single_bonus).powi(active_count as i32);
    targeting_bonus.accuracy_bonus = combined.min(0.3);
}

// ============================================================================
// AI COMBAT CORE — placeholder for threat prioritization
// ============================================================================

/// AI Combat Core boosts threat-based targeting. Currently a stub that will
/// integrate with future target prioritization logic.
pub fn update_ai_combat_core(
    _ai_query: Query<(&AICombatCoreComp, &Module), Without<DestroyedModule>>,
) {
    // Future: weight creature threat by priority_bonus in target selection.
    // For now this system exists so the component is queryable.
}

// ============================================================================
// RESEARCH LAB — generates research points from adjacent creature containment
// ============================================================================

/// Research labs generate research points when adjacent to CreatureContainment
/// modules that have stored cargo > 0.
pub fn update_research_lab(
    time: Res<Time>,
    lab_query: Query<(&ResearchLabComp, &Module), Without<DestroyedModule>>,
    cargo_query: Query<(&CargoHold, &Module), Without<DestroyedModule>>,
    mut research_state: ResMut<ResearchState>,
) {
    let dt = time.delta_secs();
    let mut total_rate = 0.0f32;

    for (lab, lab_module) in lab_query.iter() {
        if !lab_module.is_active { continue; }

        // Check adjacent CreatureContainment for specimens
        let mut specimen_count = 0.0f32;
        for (cargo, cargo_module) in cargo_query.iter() {
            if cargo_module.module_type != ModuleType::CreatureContainment { continue; }
            if !cargo_module.is_active { continue; }

            let dist = (cargo_module.grid_position - lab_module.grid_position).as_vec2().length();
            if dist <= 2.5 && cargo.current_weight > 0.0 {
                specimen_count += cargo.current_weight / 10.0; // normalize
            }
        }

        if specimen_count > 0.0 {
            total_rate += lab.research_speed * specimen_count;
        }
    }

    research_state.research_rate = total_rate;
    research_state.research_points += total_rate * dt;
}

// ============================================================================
// MAINTENANCE LOCKER — boosts repair rate of adjacent RepairSystem modules
// ============================================================================

/// Maintenance lockers boost the repair_rate of adjacent RepairSystem modules by 15%.
pub fn maintenance_locker_boost(
    locker_query: Query<&Module, Without<DestroyedModule>>,
    mut repair_query: Query<(&mut RepairSystem, &Module), Without<DestroyedModule>>,
) {
    // Collect locker positions
    let locker_positions: Vec<IVec2> = locker_query
        .iter()
        .filter(|m| m.module_type == ModuleType::MaintenanceLocker && m.is_active)
        .map(|m| m.grid_position)
        .collect();

    if locker_positions.is_empty() { return; }

    for (mut repair_sys, repair_module) in repair_query.iter_mut() {
        if !repair_module.is_active { continue; }

        let base_rate = repair_sys.repair_rate;
        let mut boost = 1.0f32;

        for &locker_pos in &locker_positions {
            let dist = (repair_module.grid_position - locker_pos).as_vec2().length();
            if dist <= 2.5 {
                boost += 0.15; // +15% per adjacent locker
            }
        }

        repair_sys.repair_rate = base_rate * boost;
    }
}
