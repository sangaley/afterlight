use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::celestial::components::GravityAffected;
use super::components::*;

// ============================================================================
// ENHANCER SYSTEMS
// Each advanced module scans adjacent tiles and applies its effect.
// Effects are recalculated each frame (idempotent via the stat reset systems).
// ============================================================================

/// Applies all adjacency-based enhancer effects for weapons, hull, and structural modules.
/// Resets every weapon to its emergent MachineStats (block layout + Tier 2/3
/// customization, see multiblock::stats::calculate_machine_stats) and every
/// block's CascadeRisk to its default first, then applies this frame's
/// adjacency bonuses on top — previously each `*=`/`+=` below applied
/// directly to the live component with nothing ever resetting it, so a
/// weapon or block sitting next to an enhancer compounded that bonus every
/// single frame (e.g. range *= 1.40 forever) instead of applying it once.
///
/// Was resetting straight to the raw BaseWeaponStats snapshot instead of
/// MachineStats — since this system and calculate_machine_stats/
/// apply_machine_stats_to_weapons had no defined order, whichever ran last
/// in a given frame won outright, silently discarding the other's
/// contribution (block counts and Tier 2/3 customization would work for one
/// frame then vanish). Resetting to MachineStats.effective_* plus an
/// explicit `.after()` (see building/mod.rs registration) makes this the
/// final compose-on-top step instead of a competing overwrite.
///
/// Also scoped per-ship (via each entry's owning ship, from ChildOf): grid
/// positions are ship-local and small in range, so they collide constantly
/// across different ships. Unscoped, a player's MuzzleBrake could buff an
/// AI ship's cannon that happens to sit at a matching local (x, y) — same
/// cross-ship contamination pattern fixed everywhere else this session.
pub fn apply_weapon_enhancers(
    module_query: Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    mut weapon_query: Query<(
        &Module, &mut Weapon, Option<&BaseWeaponStats>, Option<&MachineStats>, &ChildOf,
        Option<&crate::building::customization::tuning::WeaponTuning>,
        Option<&crate::building::customization::tuning::SelectedAmmo>,
    ), Without<DestroyedModule>>,
    mut cascade_query: Query<(&Module, &mut CascadeRisk, &ChildOf)>,
) {
    for (_, mut weapon, base, machine, _, tuning, ammo) in weapon_query.iter_mut() {
        if let Some(machine) = machine {
            weapon.damage = machine.effective_damage;
            weapon.range = machine.effective_range;
            weapon.fire_rate = machine.effective_fire_rate;
        } else if let Some(base) = base {
            weapon.damage = base.damage;
            weapon.range = base.range;
            weapon.fire_rate = base.fire_rate;
        }
        if let Some(base) = base {
            weapon.max_ammo = base.max_ammo;
        }
        // Stat tuning (dock-side sliders) composes into the same reset —
        // applying it anywhere else gets overwritten by this reset next
        // frame. Heavier loaded ammo cycles slower.
        if let Some(tuning) = tuning {
            weapon.damage *= tuning.damage;
            let weight = ammo.map(|a| a.0.weight_mult()).unwrap_or(1.0);
            weapon.fire_rate = (weapon.fire_rate * tuning.fire_rate / weight).max(0.05);
        }
    }
    for (_, mut cascade, _) in cascade_query.iter_mut() {
        cascade.cascade_chance = CascadeRisk::default().cascade_chance;
    }

    // Collect enhancer positions, types, and owning ship
    let enhancers: Vec<(Entity, IVec2, ModuleType)> = module_query.iter()
        .filter(|(m, _)| m.is_active)
        .map(|(m, parent)| (parent.parent(), m.grid_position, m.module_type))
        .collect();

    for (ship, pos, module_type) in &enhancers {
        match module_type {
            // MuzzleBrake and RecoilAbsorber used to be adjacency enhancers
            // here. They are gun parts now (combat::gun_assembly).
            ModuleType::BoreEvacuator => {
                for (wm, mut weapon, _, _, wp, _, _) in weapon_query.iter_mut() {
                    if wp.parent() == *ship && is_adjacent(pos, &wm.grid_position)
                        && matches!(wm.module_type, ModuleType::Cannon | ModuleType::Railgun | ModuleType::Coilgun | ModuleType::Gatling)
                    {
                        weapon.fire_rate *= 1.20;
                    }
                }
            }
            ModuleType::MagneticAccelerator => {
                for (wm, mut weapon, _, _, wp, _, _) in weapon_query.iter_mut() {
                    if wp.parent() == *ship && is_adjacent(pos, &wm.grid_position)
                        && matches!(wm.module_type, ModuleType::Railgun | ModuleType::Coilgun)
                    {
                        weapon.range *= 1.40;
                    }
                }
            }
            ModuleType::FocusingArray => {
                for (wm, mut weapon, _, _, wp, _, _) in weapon_query.iter_mut() {
                    if wp.parent() == *ship && is_adjacent(pos, &wm.grid_position)
                        && matches!(wm.module_type, ModuleType::Laser | ModuleType::PlasmaCaster | ModuleType::IonDisruptor)
                    {
                        weapon.range *= 1.30;
                    }
                }
            }
            ModuleType::WarheadBay => {
                for (wm, mut weapon, _, _, wp, _, _) in weapon_query.iter_mut() {
                    if wp.parent() == *ship && is_adjacent(pos, &wm.grid_position)
                        && matches!(wm.module_type, ModuleType::HeavyMissile | ModuleType::GuidedMissile | ModuleType::ClusterRocket)
                    {
                        weapon.max_ammo += 8;
                    }
                }
            }
            ModuleType::VibrationDamper => {
                for (wm, mut weapon, _, _, wp, _, _) in weapon_query.iter_mut() {
                    if wp.parent() == *ship && is_adjacent(pos, &wm.grid_position) {
                        weapon.damage *= 1.10;
                    }
                }
            }
            ModuleType::ReinforcedJoint => {
                for (cm, mut cascade, cp) in cascade_query.iter_mut() {
                    if cp.parent() == *ship && is_adjacent(pos, &cm.grid_position) {
                        cascade.cascade_chance *= 0.60;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Applies hull and structural enhancers (radiation hardening, hull reinforce, structural brace).
/// Resets every hull tile to its BaseHullStats snapshot first (see that
/// component's doc comment), and scopes enhancers per-ship the same way
/// apply_weapon_enhancers does — grid positions collide across ships.
pub fn apply_hull_enhancers(
    module_query: Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    mut hull_query: Query<(&mut HullSegment, &BaseHullStats, &ChildOf)>,
) {
    for (mut hull, base, _) in hull_query.iter_mut() {
        hull.max_health = base.max_health;
        hull.radiation_shielding = base.radiation_shielding;
    }

    let enhancers: Vec<(Entity, IVec2, ModuleType)> = module_query.iter()
        .filter(|(m, _)| m.is_active)
        .map(|(m, parent)| (parent.parent(), m.grid_position, m.module_type))
        .collect();

    for (ship, pos, module_type) in &enhancers {
        match module_type {
            ModuleType::RadiationHardening => {
                for (mut hull, _, hp) in hull_query.iter_mut() {
                    if hp.parent() == *ship && is_adjacent(pos, &hull.grid_position) {
                        hull.radiation_shielding *= 1.50;
                    }
                }
            }
            ModuleType::HullReinforcePlate | ModuleType::StructuralBrace => {
                let bonus_mult = if *module_type == ModuleType::HullReinforcePlate { 1.30 } else { 1.25 };
                for (mut hull, _, hp) in hull_query.iter_mut() {
                    if hp.parent() == *ship && is_adjacent(pos, &hull.grid_position) {
                        hull.max_health *= bonus_mult;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Base fuel_consumption_rate before any FuelInjector bonus — matches
/// FuelState::default(), the only other place this value is set.
const BASE_FUEL_CONSUMPTION_RATE: f32 = 0.2;

/// Base ship mass before any InertialDampener/GravityCompensator bonus —
/// matches the GravityAffected the player ship spawns with (ship/spawner.rs).
const SHIP_BASE_MASS: f32 = 5000.0;

/// Passive utility effects: signal jammer, fuel injector, inertial dampener.
/// Resets fuel_consumption_rate and ship mass to their known base values
/// each frame before reapplying — both were previously live values with no
/// reset, so `*= reduction` / `*= resistance` compounded forever (fuel
/// consumption decaying toward zero, or mass climbing toward infinity,
/// within seconds of a single relevant module being active). noise_level
/// doesn't need the same treatment — update_ship_state already resets it
/// from scratch every frame; this system just needs to run after that (see
/// registration in building/mod.rs).
/// module_query is scoped to the player's own ship: NoiseState/FuelState/
/// the queried GravityAffected are all player-only, but the module scan
/// itself was unscoped and would count e.g. an AI ship's FuelInjector
/// toward the player's fuel savings.
pub fn apply_utility_enhancers(
    module_query: Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    ship_query: Query<Entity, With<Ship>>,
    mut noise_state: ResMut<NoiseState>,
    mut fuel_state: ResMut<FuelState>,
    mut gravity_query: Query<&mut GravityAffected, With<Ship>>,
) {
    let Ok(player_ship) = ship_query.single() else { return };

    fuel_state.fuel_consumption_rate = BASE_FUEL_CONSUMPTION_RATE;
    if let Ok(mut gravity) = gravity_query.single_mut() {
        gravity.mass = SHIP_BASE_MASS;
    }

    let mut has_signal_jammer = false;
    let mut fuel_injector_count = 0u32;
    let mut dampener_count = 0u32;

    for (module, parent) in module_query.iter() {
        if !module.is_active || parent.parent() != player_ship { continue; }
        match module.module_type {
            ModuleType::SignalJammer => has_signal_jammer = true,
            ModuleType::FuelInjector => fuel_injector_count += 1,
            ModuleType::InertialDampener => dampener_count += 1,
            ModuleType::GravityCompensator => dampener_count += 2, // Stronger version
            _ => {}
        }
    }

    if has_signal_jammer {
        noise_state.noise_level *= 0.60;
    }

    if fuel_injector_count > 0 {
        // Diminishing returns: first = 20%, second = 10%, etc.
        let reduction = 1.0 - (fuel_injector_count as f32 * 0.15).min(0.40);
        fuel_state.fuel_consumption_rate *= reduction;
    }

    if dampener_count > 0 {
        if let Ok(mut gravity) = gravity_query.single_mut() {
            // Higher mass = less acceleration from same force
            let resistance = 1.0 + dampener_count as f32 * 0.30;
            gravity.mass *= resistance;
        }
    }
}

/// Emergency shutdown: auto-SCRAM reactor before meltdown
pub fn emergency_shutdown_system(
    mut commands: Commands,
    reactor_query: Query<(Entity, &Module, &Reactor), Without<DestroyedModule>>,
    shutdown_query: Query<(Entity, &Module), Without<DestroyedModule>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for (_reactor_entity, reactor_module, reactor) in reactor_query.iter() {
        if reactor.heat < reactor.max_heat * 0.90 { continue; }

        for (shutdown_entity, shutdown_module) in shutdown_query.iter() {
            if shutdown_module.module_type != ModuleType::EmergencyShutdown || !shutdown_module.is_active { continue; }
            if !is_adjacent(&reactor_module.grid_position, &shutdown_module.grid_position) { continue; }

            // Consume the shutdown module (one-time use)
            commands.entity(shutdown_entity).try_insert(DestroyedModule {
                original_type: shutdown_module.module_type,
            });

            notifications.write(ShowNotification {
                message: "EMERGENCY SCRAM! Reactor cooled before meltdown!".into(),
                notification_type: NotificationType::Danger,
                duration: 5.0,
            });
            return;
        }
    }
}

/// Afterburner: Shift+W for temporary thrust boost
pub fn afterburner_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    module_query: Query<&Module, Without<DestroyedModule>>,
    mut engine_query: Query<&mut Engine>,
    mut notifications: MessageWriter<ShowNotification>,
    mut active_timer: Local<f32>,
    mut cooldown: Local<f32>,
) {
    let dt = time.delta_secs();

    if *active_timer > 0.0 { *active_timer -= dt; }
    if *cooldown > 0.0 { *cooldown -= dt; }

    let has_afterburner = module_query.iter()
        .any(|m| m.module_type == ModuleType::Afterburner && m.is_active);

    if !has_afterburner { return; }

    if keyboard.pressed(KeyCode::ShiftLeft) && keyboard.pressed(KeyCode::KeyW)
        && *cooldown <= 0.0 && *active_timer <= 0.0
    {
        *active_timer = 5.0;
        *cooldown = 30.0;
        notifications.write(ShowNotification {
            message: "AFTERBURNER ENGAGED!".into(),
            notification_type: NotificationType::Warning,
            duration: 2.0,
        });
    }

    if *active_timer > 0.0 {
        for mut engine in engine_query.iter_mut() {
            engine.thrust *= 3.0;
        }
    }
}

fn is_adjacent(a: &IVec2, b: &IVec2) -> bool {
    let diff = *a - *b;
    (diff.x.abs() + diff.y.abs()) == 1
}
