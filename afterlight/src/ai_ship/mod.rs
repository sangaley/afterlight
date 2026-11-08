pub mod awareness;
pub mod traffic;
pub mod components;
mod layouts;
pub mod spawner;
mod ai_brain;
mod movement;
mod combat;
mod noise;
pub mod wreck;
mod scavenger;
pub mod simulation;
mod crew;
pub mod interior;
pub mod air;
pub mod livery;
mod power;
pub mod roster;
pub mod fleet;
pub mod standing_spawns;
pub mod preview;

use bevy::prelude::*;

use crate::states::{GameState, SpatialSet};

use components::*;
pub use layouts::design_slug;

pub struct AiShipPlugin;

impl Plugin for AiShipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Last, clear_ships_left_by_load);
        // A system changing hands moves its fleets (celestial::territory).
        app.add_systems(Update, simulation::fleets_change_hands);
        // Far ships drawn as pictures of their designs, until they materialise.
        app.init_resource::<preview::PreviewImages>().add_systems(
            Update,
            preview::sync_previews
                .after(simulation::sync_simulation_entities)
                .run_if(in_state(crate::states::GameState::Exploring)),
        );
        // Your standing sends hunters, escorts and war fleets.
        app.init_resource::<standing_spawns::StandingSpawns>().add_systems(
            Update,
            (
                standing_spawns::standing_spawns,
                standing_spawns::war_fleets,
                standing_spawns::tick_missions,
                standing_spawns::allies_defend_player,
                traffic::run_traffic,
            )
                .run_if(in_state(crate::states::GameState::Exploring)),
        );
        app
            .init_resource::<WorldSimulation>()
            .init_resource::<awareness::Battlefield>()
            .init_resource::<awareness::Mind>()
            .init_resource::<traffic::TradingFleets>()
            .register_required_components::<AiShip, awareness::Awareness>()
            .init_resource::<scavenger::ScavengerWaves>()
            // Ordered after the galaxy layout exists (CelestialPlugin) —
            // init_world_simulation reads GalaxyMap/SystemStreamingManager
            // to know which system(s) to populate.
            .add_systems(OnEnter(GameState::Exploring), simulation::init_world_simulation.after(crate::celestial::galaxy::generate_galaxy_on_enter))
            .add_systems(
                Update,
                (
                    simulation::tick_world_simulation,
                    (simulation::sync_simulation_entities, simulation::apply_spawn_integrity, simulation::track_live_ships).chain(),
                    // spawn_raider_waves removed — was periodically teleporting a
                    // hostile wing in right next to the player regardless of
                    // location, which was most of what made fights feel
                    // constant/unavoidable. Territories + roaming wanderers are
                    // the encounter source now; ships stay on their own patrols.
                    (awareness::survey_battlefield, ai_brain::ai_brain_system).chain(),
                    movement::ai_ship_movement_system,
                    movement::ai_thruster_system,
                    movement::ai_fuel_system,
                    power::update_ai_power,
                    // Grouped: this tuple is at Bevy's 20-element limit, so
                    // gunnery goes in as one nested pair rather than two slots.
                    (
                        combat::ai_weapon_fire_system.after(power::update_ai_power),
                        // Reads the ricochet tally projectile_collision keeps and
                        // changes what the guns are loading. After firing, so a
                        // switch lands on the next volley rather than mid-burst.
                        combat::ai_adapt_ammo.after(combat::ai_weapon_fire_system),
                        combat::ai_restock_between_fights.after(combat::ai_weapon_fire_system),
                    ),
                    combat::ai_distress_system,
                    combat::process_ai_ship_damage_system,
                    combat::check_ai_reactor_destruction,
                    noise::ai_ship_noise_system,
                    noise::ai_ship_noise_trail_system,
                    noise::ai_ship_radar_contact_decay,
                    wreck::ai_ship_death_system,
                    (wreck::update_death_rattle, wreck::paint_wrecks),
                    wreck::wreck_fire_consumes_loot,
                    scavenger::schedule_scavenger_waves.after(wreck::ai_ship_death_system),
                    scavenger::spawn_scavenger_waves,
                    scavenger::scavengers_feed,
                )
                    .after(SpatialSet::Update)
                    .run_if(in_state(GameState::Exploring)),
            )
            // Defeat conditions. Separate block only because the list above is
            // at Bevy's 20-system tuple ceiling.
            .add_systems(
                Update,
                (
                    // Runs after damage so a killing blow on the last gun is
                    // read the same frame it lands.
                    combat::check_ai_cripple.after(combat::process_ai_ship_damage_system),
                    combat::tick_reactor_meltdown,
                    combat::ai_chain_reactions,
                    // What an enemy shows of itself. Concealment runs first so
                    // a ship spawned this frame is already dark before anyone
                    // looks at it; the reveal runs after breaches are recorded
                    // so a hole opens its surroundings on the frame it lands.
                    interior::conceal_new_ai_interiors,
                    interior::record_ai_breaches
                        .after(combat::process_ai_ship_damage_system),
                    interior::reveal_breached_interior
                        .after(interior::record_ai_breaches)
                        .after(interior::conceal_new_ai_interiors),
                )
                    .run_if(in_state(GameState::Exploring)),
            )
            // The air aboard, and the crew breathing it. Before the cripple
            // check so a crew that dies this frame is counted this frame.
            .add_systems(
                Update,
                (
                    // Before concealment reads a block's true colour.
                    livery::apply_paint.before(interior::conceal_new_ai_interiors),
                    air::fill_ai_air,
                    air::breathe_ai_ships
                        .after(air::fill_ai_air)
                        .after(interior::record_ai_breaches)
                        .before(combat::check_ai_cripple),
                )
                    .run_if(in_state(GameState::Exploring)),
            );
    }
}

/// Despawn the AI ships a load left behind (see `LeftByLoad`) once the
/// messages that might still name them have run out.
fn clear_ships_left_by_load(mut commands: Commands, mut ships: Query<(Entity, &mut LeftByLoad)>) {
    for (ship, mut left) in ships.iter_mut() {
        if left.frames == 0 {
            commands.entity(ship).try_despawn();
        } else {
            left.frames -= 1;
        }
    }
}
