pub mod components;
pub mod resources;
pub mod events;
pub mod gravity;
pub mod orbits;
pub mod stars;
pub mod black_holes;
pub mod spawning;
pub mod warp;
pub mod poi;
pub mod galaxy;
pub mod territory;
pub mod capital_worlds;

use bevy::prelude::*;
use crate::states::GameState;

pub struct CelestialPlugin;

/// System set for ordering celestial mechanics
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CelestialSet {
    Orbits,
    Gravity,
    Forces,
    StarLogic,
    BlackHoles,
    Cleanup,
}

impl Plugin for CelestialPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((territory::TerritoryPlugin, capital_worlds::CapitalWorldsPlugin))
            .init_resource::<resources::GalaxyState>()
            .init_resource::<resources::CelestialConfig>()
            .init_resource::<resources::GalaxyMap>()
            .init_resource::<resources::SystemStreamingManager>()
            .init_resource::<resources::PendingGalaxyWarpTarget>()
            .init_resource::<resources::LootedPois>()
            // Events
            .add_message::<events::RadiationFlare>()
            .add_message::<events::StarDestroyed>()
            .add_message::<events::PlanetConsumed>()
            .add_message::<events::BodyConsumed>()
            .add_message::<events::GravityWarning>()
            .add_message::<events::SupernovaShockwave>()
            .add_message::<events::WarpJumpStarted>()
            .add_message::<events::WarpJumpCompleted>()
            // System set ordering
            .configure_sets(Update, CelestialSet::Orbits.run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, CelestialSet::Gravity.after(CelestialSet::Orbits).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, CelestialSet::Forces.after(CelestialSet::Gravity).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, CelestialSet::StarLogic.after(CelestialSet::Forces).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, CelestialSet::BlackHoles.after(CelestialSet::StarLogic).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, CelestialSet::Cleanup.after(CelestialSet::BlackHoles).run_if(in_state(GameState::Exploring)))
            // Orbital mechanics
            .add_systems(Update, (
                orbits::update_orbital_positions,
                orbits::update_free_flight,
                // After the positions move, so the dotted path and the planet
                // on it are drawn from the same frame's state.
                orbits::draw_orbit_paths.after(orbits::update_orbital_positions),
            ).in_set(CelestialSet::Orbits))
            // Gravity accumulation
            .add_systems(Update,
                gravity::accumulate_gravity.in_set(CelestialSet::Gravity),
            )
            // Force application
            .add_systems(Update, (
                gravity::apply_gravity_to_velocity,
                gravity::apply_gravity_to_ship,
            ).in_set(CelestialSet::Forces))
            // Star logic
            // stars::star_flare_buildup / apply_flare_radiation removed —
            // radiation mechanic disabled per request. (Their damage events
            // were already routed to a dead end: process_ship_damage skips
            // any DamageSource::Radiation event, having been written for the
            // old check_radiation_damage's direct-application model — so
            // this only ever cost a misleading "radiation spike" warning.)
            .add_systems(Update, (
                stars::star_death_check,
                stars::apply_supernova_damage,
                orbits::destabilize_orbits.after(stars::star_death_check),
            ).in_set(CelestialSet::StarLogic))
            // Black hole logic
            .add_systems(Update, (
                black_holes::check_event_horizon,
                black_holes::process_consumption.after(black_holes::check_event_horizon),
                black_holes::grow_black_hole.after(black_holes::process_consumption),
            ).in_set(CelestialSet::BlackHoles))
            // Warp system (runs during exploring)
            .add_systems(Update, (
                warp::warp_input_system,
                warp::execute_warp_jump.after(warp::warp_input_system),
                warp::on_warp_complete.after(warp::execute_warp_jump),
                poi::mining_system,
                poi::loot_derelict_system,
                poi::loot_prompt_system,
                galaxy::passive_proximity_discovery_system,
            ).run_if(in_state(GameState::Exploring)))
            // Spawn initial star system on entering Exploring — chained so
            // the galaxy layout always exists before Haven tries to read
            // its assigned StarSystemDef from it.
            .add_systems(OnEnter(GameState::Exploring), (galaxy::generate_galaxy_on_enter, spawn_initial_system).chain())
        ;
    }
}

/// Spawn the system the player is in when they start exploring: Haven on a
/// new run, whichever system a save was made in on a load.
///
/// It always spawned `systems[0]`. A save made anywhere else came back with
/// Haven's star and planets around a ship parked in another system's space,
/// and a later jump to Haven spawned it a second time on top -- two sets of
/// solid bodies and two gravity wells.
///
/// Goes through galaxy::load_system, the same path a warp uses.
fn spawn_initial_system(
    mut commands: Commands,
    mut galaxy: ResMut<resources::GalaxyState>,
    mut galaxy_map: ResMut<resources::GalaxyMap>,
    streaming: Res<resources::SystemStreamingManager>,
    looted: Res<resources::LootedPois>,
    textures: Res<crate::vfx::procedural_textures::CelestialTextures>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    // Only spawn if no systems exist yet
    if !galaxy.systems.is_empty() {
        return;
    }
    if galaxy_map.systems.is_empty() {
        return; // galaxy::generate_galaxy_on_enter hasn't run yet this frame
    }
    // Uncharted space (a save made after a blind jump): nothing to spawn.
    let Some(id) = streaming.loaded_system else { return };

    let now = time.elapsed_secs_f64();
    let Some(system_info) = galaxy::load_system(&mut commands, &asset_server, &textures, &mut galaxy_map, id, now, &looted) else {
        return;
    };

    galaxy.current_system = id;
    galaxy.next_system_id = id + 1;
    galaxy.systems.push(system_info);
    galaxy.total_bodies = 1;
}

#[cfg(test)]
mod initial_system_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use components::StarSystemMember;

    /// The systems spawned when exploring starts with `loaded` as the Hot
    /// system, by id.
    fn spawned_with(loaded: Option<u32>) -> Vec<u32> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Image>();
        app.init_resource::<resources::GalaxyState>()
            .init_resource::<resources::LootedPois>()
            .insert_resource(galaxy::generate_galaxy_map(42))
            .insert_resource(resources::SystemStreamingManager { loaded_system: loaded, ..default() })
            .insert_resource(crate::vfx::procedural_textures::CelestialTextures {
                solid: default(),
                glow: default(),
                ring: default(),
            });
        app.world_mut().run_system_once(spawn_initial_system).unwrap();
        let world = app.world_mut();
        let mut ids: Vec<u32> = world.query::<&StarSystemMember>().iter(world).map(|m| m.system_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// A save made in another system loads into that system, not Haven.
    #[test]
    fn it_spawns_the_system_the_player_is_in() {
        assert_eq!(spawned_with(Some(0)), vec![0]);
        assert_eq!(spawned_with(Some(5)), vec![5]);
    }

    /// Uncharted space has nothing in it, and certainly not Haven.
    #[test]
    fn uncharted_space_spawns_nothing() {
        assert!(spawned_with(None).is_empty());
    }
}
