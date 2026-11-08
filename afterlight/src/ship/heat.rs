//! Heat network system: generation, diffusion, cooling, and damage.
//!
//! Heat spreads between adjacent modules on the grid. Reactors, engines, and
//! weapons generate heat; CoolingPumps and HeatVents remove it. Overheated
//! modules take damage and may catch fire.
//!
//! EVERY SHIP, not just the player's. The map is keyed by (ship, local cell),
//! so an AI reactor's heat stays on the AI ship instead of landing on the
//! player tile with the same local coordinates — which is what forced this to
//! be player-only before, and meant enemy weapons never overheated while
//! yours thermally throttled under sustained fire.

use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;

/// Sync ModuleTemperature components → HeatNetworkState HashMap.
/// On first encounter, seed the map from the component; thereafter the map
/// is authoritative (updated by diffusion/cooling) and writes back.
pub fn sync_module_temperatures(
    mut heat_state: ResMut<HeatNetworkState>,
    module_query: Query<(&Module, &ModuleTemperature, &ChildOf)>,
) {
    // Seed any new modules into the map
    for (module, temp, parent) in module_query.iter() {
        heat_state.temperatures
            .entry((parent.parent(), module.grid_position))
            .or_insert(temp.current);
    }
}

/// Heat generation: reactors, engines (when active), weapons (on cooldown).
pub fn generate_heat(
    time: Res<Time>,
    mut heat_state: ResMut<HeatNetworkState>,
    reactor_query: Query<(&Reactor, &Module, &ChildOf), Without<DestroyedModule>>,
    engine_query: Query<(&Engine, &Module, &ChildOf), Without<DestroyedModule>>,
    weapon_query: Query<(
        &WeaponCooldown, &Module, &ChildOf,
        Option<&crate::building::customization::tuning::WeaponTuning>,
    ), Without<DestroyedModule>>,
) {
    let dt = time.delta_secs();

    // Reactors generate heat proportional to output. Tuned against a
    // reactor's own ModuleTemperature.max_temp (100, set in spawner.rs) and
    // the 5.0/s flat ambient cooling in apply_cooling. Was 1.5 (itself
    // already lowered once from 8.0) — but at 1.5 a Standard Reactor (output
    // 500, the common case) still generated 7.5 heat/s against only 5/s of
    // passive removal, a steady +2.5/s climb with zero cooling infrastructure
    // and zero combat activity, maxing out and auto-shutting the reactor down
    // in well under a minute just for being turned on. 0.8 keeps every
    // current reactor (output ≤ 500) net-negative when idle — heat now only
    // becomes a real risk once weapon fire or engine thrust piles more on.
    for (reactor, module, parent) in reactor_query.iter() {
        if !module.is_active { continue; }
        let heat_gain = (reactor.output / 100.0) * 0.8 * dt;
        *heat_state.temperatures.entry((parent.parent(), module.grid_position)).or_insert(0.0) += heat_gain;
    }

    // Active engines generate some heat. Was 2.0 — a 400-thrust engine
    // (the current max) generated 8.0/s against 5.0/s ambient, another
    // source of passive-idle heat creep independent of the reactor. 1.0
    // keeps every current engine (thrust ≤ 400) net-negative when idle too.
    for (engine, module, parent) in engine_query.iter() {
        if !module.is_active { continue; }
        let heat_gain = (engine.thrust / 100.0) * 1.0 * dt;
        *heat_state.temperatures.entry((parent.parent(), module.grid_position)).or_insert(0.0) += heat_gain;
    }

    // Weapons generate heat while recently fired (cooldown running), scaled
    // by tuning — an overtuned gun outpaces ambient cooling, thermally
    // throttles at 95% max temp, and cooks itself past max. This is the
    // counterweight that keeps "max every slider" from being free; the
    // constant power draw alone barely registers against a mid-game reactor.
    for (cooldown, module, parent, tuning) in weapon_query.iter() {
        if !module.is_active { continue; }
        if !cooldown.timer.is_finished() {
            // Currently cooling = recently fired
            let factor = tuning.map(|t| t.power_factor()).unwrap_or(1.0);
            let heat_gain = crate::building::customization::tuning::weapon_heat_per_second(factor) * dt;
            *heat_state.temperatures.entry((parent.parent(), module.grid_position)).or_insert(0.0) += heat_gain;
        }
    }
}

/// Diffuse heat between adjacent grid tiles. Heat is conserved.
pub fn diffuse_heat(
    time: Res<Time>,
    mut heat_state: ResMut<HeatNetworkState>,
    temp_query: Query<(&Module, &ModuleTemperature, &ChildOf)>,
) {
    let dt = time.delta_secs();

    // Build conductivity map (player modules only — AI local coords collide)
    let mut conductivity_map: bevy::platform::collections::HashMap<(Entity, IVec2), f32> = bevy::platform::collections::HashMap::new();
    for (module, temp, parent) in temp_query.iter() {
        conductivity_map.insert((parent.parent(), module.grid_position), temp.conductivity);
    }

    // Snapshot current temperatures into prev for reading
    heat_state.prev_temperatures = heat_state.temperatures.clone();

    // Compute deltas into a separate map to avoid borrow conflicts
    let mut deltas: Vec<((Entity, IVec2), f32)> = Vec::new();
    let offsets = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];

    for (&(ship, pos), &temp) in heat_state.prev_temperatures.iter() {
        if temp <= 0.0 { continue; }
        let conductivity = conductivity_map.get(&(ship, pos)).copied().unwrap_or(0.5);
        let transfer_rate = conductivity * 0.1 * dt;

        for offset in &offsets {
            // Same ship only — heat crosses between adjacent tiles of one
            // hull, never between two ships that happen to share a local cell.
            let neighbor = (ship, pos + *offset);
            if let Some(&neighbor_temp) = heat_state.prev_temperatures.get(&neighbor) {
                let delta = (temp - neighbor_temp) * transfer_rate;
                if delta > 0.0 {
                    deltas.push(((ship, pos), -delta));
                    deltas.push((neighbor, delta));
                }
            }
        }
    }

    // Apply deltas
    for (pos, delta) in deltas {
        *heat_state.temperatures.entry(pos).or_insert(0.0) += delta;
    }
}

/// Apply cooling: CoolingPumps, HeatVents, and ambient environmental cooling.
/// Heat a Coolant Run draws out each second, shared over its own tile and
/// the four beside it (a Cooling Pump's is 80).
pub const COOLANT_RUN_RATE: f32 = 20.0;

pub fn apply_cooling(
    time: Res<Time>,
    depth_state: Res<DepthState>,
    mut heat_state: ResMut<HeatNetworkState>,
    cooling_query: Query<(&CoolingPumpComp, &Module, &ChildOf), Without<DestroyedModule>>,
    vent_query: Query<(&HeatVentComp, &Module, &ChildOf), Without<DestroyedModule>>,
    pipe_query: Query<(&Module, &ChildOf), (Without<DestroyedModule>, Without<CoolingPumpComp>, Without<HeatVentComp>)>,
) {
    let dt = time.delta_secs();
    let offsets = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];

    // CoolingPumps: remove heat from adjacent tiles
    for (pump, module, parent) in cooling_query.iter() {
        if !module.is_active { continue; }
        let cooling_per_neighbor = pump.cooling_rate * dt / 4.0;
        for offset in &offsets {
            let neighbor = (parent.parent(), module.grid_position + *offset);
            if let Some(temp) = heat_state.temperatures.get_mut(&neighbor) {
                *temp = (*temp - cooling_per_neighbor).max(0.0);
            }
        }
    }

    // HeatVents: dissipate own tile heat, scaled by distance (deeper void = better radiative cooling)
    for (vent, module, parent) in vent_query.iter() {
        if !module.is_active { continue; }
        let depth_bonus = 1.0 + (depth_state.current_depth / 500.0).min(2.0);
        let dissipation = vent.dissipation_rate * depth_bonus * dt;
        if let Some(temp) = heat_state.temperatures.get_mut(&(parent.parent(), module.grid_position)) {
            *temp = (*temp - dissipation).max(0.0);
        }
    }

    // Coolant Runs: unpowered pipework drawing heat out of their own tile
    // and the four beside it, a quarter of a pump.
    for (module, parent) in pipe_query.iter() {
        if module.module_type != ModuleType::CoolantRun { continue; }
        let share = COOLANT_RUN_RATE * dt / 5.0;
        for offset in [IVec2::ZERO, IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            if let Some(temp) = heat_state.temperatures.get_mut(&(parent.parent(), module.grid_position + offset)) {
                *temp = (*temp - share).max(0.0);
            }
        }
    }

    // Ambient environmental cooling: all tiles lose heat passively
    let ambient = 5.0 * dt;
    for temp in heat_state.temperatures.values_mut() {
        *temp = (*temp - ambient).max(0.0);
    }
}

/// Apply heat damage to overheated modules. Fire risk at extreme temps.
pub fn apply_heat_damage(
    time: Res<Time>,
    heat_state: Res<HeatNetworkState>,
    mut module_query: Query<(Entity, &mut Module, &ModuleTemperature, Option<&OnFire>, &ChildOf), Without<DestroyedModule>>,
    ship_query: Query<Entity, With<Ship>>,
    mut commands: Commands,
    mut notifications: MessageWriter<ShowNotification>,
    mut warned: Local<bool>,
) {
    let Ok(player_ship) = ship_query.single() else { return };
    let dt = time.delta_secs();

    for (entity, mut module, temp, on_fire, parent) in module_query.iter_mut() {
        let current = heat_state.temperatures
            .get(&(parent.parent(), module.grid_position))
            .copied()
            .unwrap_or(temp.current);

        if current <= temp.max_temp * 0.8 {
            continue;
        }

        if current > temp.max_temp {
            // Overheat damage
            let damage = (current - temp.max_temp) * 0.5 * dt;
            module.health = (module.health - damage).max(0.0);

            // Damage applies to every ship now; the WARNING is yours alone.
            // Without the check, an enemy cooking its own guns would tell you
            // to deploy cooling.
            if !*warned && parent.parent() == player_ship {
                *warned = true;
                notifications.write(ShowNotification {
                    message: "Module overheating! Deploy cooling systems.".into(),
                    notification_type: NotificationType::Warning,
                    duration: 3.0,
                });
            }
        }

        // Fire risk at 150% max_temp
        if current > temp.max_temp * 1.5 && on_fire.is_none() {
            let fire_chance = 0.1 * dt;
            let hash = (module.grid_position.x.wrapping_mul(73) ^ module.grid_position.y.wrapping_mul(179)) as f32;
            let pseudo_rand = ((hash + current * 100.0) % 1000.0) / 1000.0;
            if pseudo_rand < fire_chance {
                commands.entity(entity).insert(OnFire {
                    intensity: 0.5,
                    damage_per_second: 4.0,
                    spread_timer: Timer::from_seconds(5.0, TimerMode::Repeating),
                    duration: Timer::from_seconds(30.0, TimerMode::Once),
                });
            }
        }

        if current <= temp.max_temp {
            *warned = false;
        }
    }
}

/// Bridge: keep existing Reactor.heat in sync with the heat network.
/// Reactor warnings, shutdown, and explosion logic in power.rs reads reactor.heat,
/// so we write the heat network temperature back to it.
pub fn sync_reactor_heat(
    heat_state: Res<HeatNetworkState>,
    mut reactor_query: Query<(&mut Reactor, &Module, &ChildOf), Without<DestroyedModule>>,
) {
    for (mut reactor, module, parent) in reactor_query.iter_mut() {
        if let Some(&temp) = heat_state.temperatures.get(&(parent.parent(), module.grid_position)) {
            reactor.heat = temp;
        }
    }
}

/// Write final heat network temperatures back to ModuleTemperature components.
pub fn sync_temperatures_back(
    heat_state: Res<HeatNetworkState>,
    mut temp_query: Query<(&Module, &mut ModuleTemperature, &ChildOf)>,
) {
    for (module, mut temp, parent) in temp_query.iter_mut() {
        if let Some(&t) = heat_state.temperatures.get(&(parent.parent(), module.grid_position)) {
            temp.current = t;
        }
    }
}

#[cfg(test)]
mod heat_tests {
    use super::*;
    use crate::ai_ship::components::AiShip;

    fn reactor_module(cell: IVec2) -> Module {
        Module {
            module_type: ModuleType::StandardReactor,
            health: 100.0,
            max_health: 100.0,
            power_consumption: 0.0,
            power_generation: 500.0,
            is_active: true,
            grid_position: cell,
            size: IVec2::ONE,
            rotation: Rotation::North,
        }
    }

    /// The reason this simulation was player-only: grid coordinates are
    /// ship-local, so an AI reactor at ITS (0,0) and the player's module at
    /// (0,0) are different blocks that collided on one key. Heat generated on
    /// an enemy landed on your hull.
    ///
    /// Keying by (ship, cell) is what makes running it for everyone safe — and
    /// running it for everyone is what finally makes enemy guns overheat.
    #[test]
    fn a_coolant_run_draws_heat_from_itself_and_its_neighbours() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(1));
        world.insert_resource(time);
        world.insert_resource(DepthState::default());
        world.init_resource::<HeatNetworkState>();
        let ship = world.spawn(Ship).id();
        let pipe = Module { module_type: ModuleType::CoolantRun, power_generation: 0.0, ..reactor_module(IVec2::ZERO) };
        world.spawn((pipe, ChildOf(ship)));
        {
            let mut heat = world.resource_mut::<HeatNetworkState>();
            for c in [IVec2::ZERO, IVec2::X, IVec2::new(2, 0)] {
                heat.temperatures.insert((ship, c), 100.0);
            }
        }
        world.run_system_once(apply_cooling).unwrap();
        let heat = world.resource::<HeatNetworkState>();
        let ambient = 5.0;
        let share = COOLANT_RUN_RATE / 5.0;
        assert_eq!(heat.temperatures[&(ship, IVec2::ZERO)], 100.0 - share - ambient);
        assert_eq!(heat.temperatures[&(ship, IVec2::X)], 100.0 - share - ambient, "the block beside it");
        assert_eq!(heat.temperatures[&(ship, IVec2::new(2, 0))], 100.0 - ambient, "out of its reach");
    }

    #[test]
    fn heat_does_not_leak_between_ships_sharing_a_cell() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<HeatNetworkState>();
        app.add_systems(Update, generate_heat);

        let player = app.world_mut().spawn(Ship).id();
        let enemy = app.world_mut().spawn(AiShip).id();

        // Same LOCAL cell on both ships — the exact collision that forced the
        // player-only scoping.
        let cell = IVec2::ZERO;
        app.world_mut()
            .spawn((reactor_module(cell), Reactor { output: 500.0, heat: 0.0, max_heat: 100.0, explosion_risk: false }))
            .insert(ChildOf(enemy));

        app.update();

        let heat = app.world().resource::<HeatNetworkState>();
        assert!(heat.temperatures.contains_key(&(enemy, cell)),
            "the enemy's reactor should heat its OWN tile");
        assert!(!heat.temperatures.contains_key(&(player, cell)),
            "enemy heat landed on the player's hull - the leak is back");
    }

    /// And an enemy reactor really does generate, which is the point of
    /// unscoping: their weapons can now cook themselves like yours do.
    #[test]
    fn enemy_ships_generate_heat_at_all() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<HeatNetworkState>();
        app.add_systems(Update, generate_heat);

        let enemy = app.world_mut().spawn(AiShip).id();
        app.world_mut()
            .spawn((reactor_module(IVec2::new(3, 1)), Reactor { output: 500.0, heat: 0.0, max_heat: 100.0, explosion_risk: false }))
            .insert(ChildOf(enemy));

        app.update();
        app.update();

        let heat = app.world().resource::<HeatNetworkState>();
        let t = heat.temperatures.get(&(enemy, IVec2::new(3, 1))).copied().unwrap_or(0.0);
        assert!(t > 0.0, "an active enemy reactor produced no heat");
    }
}
