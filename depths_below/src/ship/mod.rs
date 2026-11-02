use bevy::prelude::*;

pub mod movement;
mod systems;
pub mod towing;
mod power;
mod oxygen;
mod radiation;
mod hull;
mod spawner;
pub mod air;
pub mod decompression;
pub mod damage;
pub mod fire;
mod atmosphere;
mod subsystems;
mod heat;
mod logistics;
pub mod drill;
pub mod rebuild;
pub mod collision;

pub use movement::*;
pub use systems::*;
pub use power::*;
pub use oxygen::*;
pub use radiation::*;
pub use hull::*;
pub use spawner::*;
pub use decompression::*;
#[allow(unused_imports)]
pub use damage::*;
#[allow(unused_imports)]
pub use fire::*;
pub use atmosphere::*;

use crate::states::{GameState, ShipSet};
use crate::resources::{DepthState, PowerState, PowerAllocation, PowerChannels, OxygenState, HullState, NoiseState, GameConfig, FuelState, VictoryState, DeathCause, ExploringSessionTimer, PowerGraph, HeatNetworkState, ResearchState, AutopilotState, TargetingBonus};
use crate::crew::spawn_starter_crew;

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app
            // Breaker Drill: contact wreck salvage (see drill.rs)
            .add_systems(Update, drill::wreck_drill_system.run_if(in_state(GameState::Exploring)))
            // Physical collision (ships/stations/asteroids/planets). PostUpdate,
            // before transform propagation: runs after every mover (player, AI,
            // orbits, warp) has written positions for the frame, and its pushes
            // still land in this frame's GlobalTransforms.
            .init_resource::<collision::ColliderGrid>()
            .add_systems(
                PostUpdate,
                (
                    collision::attach_static_colliders,
                    collision::attach_knock,
                    collision::refresh_ship_colliders,
                    collision::integrate_drift.run_if(in_state(GameState::Exploring)),
                    collision::rebuild_collider_grid.run_if(in_state(GameState::Exploring)),
                    collision::shots_hit_terrain.run_if(in_state(GameState::Exploring)),
                    collision::resolve_collisions.run_if(in_state(GameState::Exploring)),
                )
                    .chain()
                    .before(bevy::transform::TransformSystems::Propagate),
            )
            // Resources
            .init_resource::<DepthState>()
            .init_resource::<PowerState>()
            .init_resource::<PowerAllocation>()
            .init_resource::<PowerChannels>()
            .init_resource::<OxygenState>()
            .init_resource::<air::AirField>()
            .init_resource::<HullState>()
            .init_resource::<NoiseState>()
            .init_resource::<GameConfig>()
            .init_resource::<FuelState>()
            .init_resource::<VictoryState>()
            .init_resource::<DeathCause>()
            .init_resource::<ExploringSessionTimer>()
            .init_resource::<PowerGraph>()
            .init_resource::<HeatNetworkState>()
            .init_resource::<ResearchState>()
            .init_resource::<AutopilotState>()
            .init_resource::<TargetingBonus>()
            .init_resource::<AtmosphereState>()

            // Configure set ordering
            .configure_sets(Update, ShipSet::Input.run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, ShipSet::Movement.after(ShipSet::Input).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, ShipSet::Physics.after(ShipSet::Movement).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, ShipSet::Power.after(ShipSet::Physics).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, ShipSet::Heat.after(ShipSet::Power).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, ShipSet::Oxygen.after(ShipSet::Heat).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, ShipSet::Hull.after(ShipSet::Oxygen).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, ShipSet::State.after(ShipSet::Hull).run_if(in_state(GameState::Exploring)))

            // Cargo capacity must also track while docked/building — the
            // State set only runs Exploring, so placing a Cargo Hold showed
            // no capacity change until you undocked.
            .add_systems(
                Update,
                update_inventory_capacity.run_if(
                    in_state(GameState::StationDocked).or_else(in_state(GameState::Docked)),
                ),
            )
            // The Power set is flight-only; this keeps PWR honest at the berth.
            .add_systems(Update, berth_power_estimate.run_if(in_state(GameState::StationDocked)))

            // Startup - spawn ship, flush commands, then spawn crew (crew needs ship entity)
            .add_systems(OnEnter(GameState::StationDocked), (
                (spawn_starter_ship, reset_victory_state),
                ApplyDeferred,
                spawn_starter_crew,
            ).chain())

            // Update systems that run while exploring
            .add_systems(
                Update,
                (
                    ship_input.in_set(ShipSet::Input),
                    ship_movement.in_set(ShipSet::Movement),
                    update_depth.in_set(ShipSet::Movement),
                    // check_radiation_damage removed — radiation mechanic disabled per request.
                    build_power_graph.in_set(ShipSet::Power),
                    update_power_system.after(build_power_graph).in_set(ShipSet::Power),
                    update_power_allocation.after(update_power_system).in_set(ShipSet::Power),
                    update_reactor_heat.after(update_power_system).in_set(ShipSet::Power),
                    update_fuel_consumption.after(update_power_system).in_set(ShipSet::Power),
                    update_oxygen_system.in_set(ShipSet::Oxygen),
                    update_ship_state.in_set(ShipSet::State),
                    check_game_over.in_set(ShipSet::State),
                    check_victory.in_set(ShipSet::State),
                    update_inventory_capacity.in_set(ShipSet::State),
                    update_statistics.in_set(ShipSet::State),
                    tick_session_timer.in_set(ShipSet::Input),
                ),
            )

            // Hull damage chain: damage → destruction → detonation → fire → cascade → integrity
            .add_systems(
                Update,
                (
                    damage::process_ship_damage,
                    hull::tint_damaged_hull.after(damage::process_ship_damage),
                    hull::tint_destroyed_hull.after(hull::tint_damaged_hull),
                    damage::tint_damaged_modules.after(damage::process_ship_damage),
                    damage::process_module_destruction.after(damage::tint_damaged_modules),
                    damage::queue_detonation.after(damage::process_module_destruction),
                    damage::process_detonations.after(damage::queue_detonation),
                    // AI-ship blasts resolve in world space against the ship's
                    // own blocks (GridOccupancy only knows the player's grid)
                    damage::queue_ai_detonation.after(damage::process_module_destruction),
                    damage::process_ai_detonations.after(damage::queue_ai_detonation),
                    damage::explosion_shockwaves.after(damage::process_ai_detonations),
                    fire::apply_fire_ignition.after(damage::process_detonations),
                    fire::update_fire.after(fire::apply_fire_ignition),
                    // Nested so the outer tuple stays inside Bevy's 20-element
                    // limit for system tuples -- past it, the whole block
                    // silently loses IntoSystemConfigs and the error lands on
                    // `.in_set` rather than on the systems that overflowed it.
                    (
                        fire::sync_fire_overlays.after(fire::update_fire),
                        fire::animate_fire_overlays.after(fire::sync_fire_overlays),
                    ),
                    hull::process_hull_cascade.after(damage::process_detonations),
                    update_hull_integrity.after(hull::process_hull_cascade).after(fire::update_fire),
                    // Air is a fluid: seed tiles, vent through holes, let
                    // neighbours equalise, then publish room air level. Every
                    // reader below (hull sync, oxygen drain, fire, crew
                    // dispatch) sees the result of the whole pass, never a
                    // half-diffused field.
                    (
                        air::mark_breached_hull.after(hull::process_hull_cascade),
                        air::sync_air_tiles.after(air::mark_breached_hull),
                        air::project_force_fields.after(air::sync_air_tiles),
                        air::vent_air_at_breaches.after(air::project_force_fields),
                        air::diffuse_air.after(air::vent_air_at_breaches),
                        air::tanks_feed_air.after(air::diffuse_air).before(air::sync_room_air),
                        air::sync_room_air.after(air::diffuse_air),
                        air::trace_air.after(air::sync_room_air),
                        update_decompression.after(air::sync_room_air),
                        seal_breach_system.after(update_decompression),
                        air::auto_containment.after(air::sync_room_air),
                        air::clear_fleeing.after(air::auto_containment),
                    ),
                    handle_bulkhead_toggle,
                    bulkhead_seal_input,
                ).in_set(ShipSet::Hull),
            )

            // Destroyed-block removal: a short delay after tint_destroyed_hull /
            // process_module_destruction gives every other Added<HullDestroyed>/
            // Added<DestroyedModule> reader across plugins (severance, chain
            // reactions, reactor meltdown, detonation queueing) a full frame to
            // react before the block actually disappears.
            .add_systems(
                Update,
                (
                    hull::queue_hull_removal.after(hull::tint_destroyed_hull),
                    damage::queue_module_removal.after(damage::process_module_destruction),
                    // Ghost recording must see the block's Module/HullSegment
                    // data before tick_pending_removal despawns it.
                    rebuild::record_rebuild_ghosts
                        .after(hull::queue_hull_removal)
                        .after(damage::queue_module_removal),
                    damage::tick_pending_removal.after(rebuild::record_rebuild_ghosts),
                ).in_set(ShipSet::Hull),
            )
            // Ghost rebuild: crew reconstruct destroyed blocks in flight;
            // manual dock rebuilding clears built-over ghosts.
            .init_resource::<rebuild::RebuildQueue>()
            .add_systems(
                Update,
                rebuild::crew_rebuild_system.run_if(in_state(GameState::Exploring)),
            )
            .add_systems(
                Update,
                rebuild::cleanup_built_over_ghosts.run_if(in_state(GameState::StationDocked)),
            )

            // Heat network (7 systems, chained)
            .add_systems(
                Update,
                (
                    heat::sync_module_temperatures,
                    heat::generate_heat.after(heat::sync_module_temperatures),
                    heat::diffuse_heat.after(heat::generate_heat),
                    heat::apply_cooling.after(heat::diffuse_heat),
                    heat::apply_heat_damage.after(heat::apply_cooling),
                    heat::sync_reactor_heat.after(heat::apply_heat_damage),
                    heat::sync_temperatures_back.after(heat::sync_reactor_heat),
                ).in_set(ShipSet::Heat),
            )

            // Subsystems (capacitors, fire suppression, radiation shielding, drones, torpedo loader, Phase B)
            .add_systems(
                Update,
                (
                    subsystems::update_capacitors,
                    subsystems::update_fire_suppression,
                    subsystems::update_radiation_shielding,
                    subsystems::update_drone_bays,
                    subsystems::apply_torpedo_loader_bonus,
                    subsystems::apply_weapon_power_scaling,
                    subsystems::apply_targeting_computer_bonus,
                    subsystems::update_ai_combat_core,
                    subsystems::update_research_lab,
                    subsystems::maintenance_locker_boost,
                    logistics::update_conveyor_tubes,
                    logistics::update_fuel_processor,
                ).in_set(ShipSet::State),
            )

            // Atmospheric events (ambient immersion)
            .add_systems(Update, atmospheric_event_system.run_if(in_state(GameState::Exploring)))

            // Reset session timer when launching into void
            .add_systems(OnEnter(GameState::Exploring), reset_session_timer)

            // Cleanup when returning to main menu (after game over / restart)
            .add_systems(OnEnter(GameState::MainMenu), cleanup_game_entities)

            ;
    }
}
