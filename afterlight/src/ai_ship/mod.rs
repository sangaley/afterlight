pub mod awareness;
pub mod doctrine;
pub mod tractor;
pub mod ecm;
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
mod roster_classes;
pub mod fleet;
pub mod standing_spawns;
pub mod preview;
pub mod nav;

use bevy::prelude::*;

use crate::states::{GameState, SpatialSet};

use components::*;
pub use layouts::design_slug;

/// What a ship's size is counted in: its hull and parts. A ship's children
/// are more than its blocks -- shadows, glows, its roof frame -- and once
/// every AI ship wore the full roof, counting them all made each one about
/// twice as heavy as it is: slower to speed up and turn, and bigger in every
/// other ship's eyes.
pub type Blocks<'w, 's> = Query<'w, 's, (), Or<(With<crate::components::HullSegment>, With<crate::components::Module>)>>;

/// How many of `children` are blocks.
pub fn block_count(children: &Children, blocks: &Blocks) -> usize {
    children.iter().filter(|c| blocks.contains(*c)).count()
}

#[cfg(test)]
mod block_count_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_ships_size_is_its_blocks_not_its_roof() {
        let mut world = World::new();
        let ship = world.spawn_empty().id();
        for x in 0..3 {
            world.spawn((crate::components::HullSegment { grid_position: IVec2::new(x, 0), ..default() }, ChildOf(ship)));
        }
        let frame = world.spawn((crate::vfx::ship_roof::RoofFrame, ChildOf(ship))).id();
        world.spawn((Sprite::default(), ChildOf(frame)));
        world.spawn((Sprite::default(), ChildOf(ship)));
        let n = world.run_system_once(move |ships: Query<&Children>, blocks: Blocks| block_count(ships.get(ship).unwrap(), &blocks)).unwrap();
        assert_eq!(n, 3);
    }
}

pub struct AiShipPlugin;

impl Plugin for AiShipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Last, clear_ships_left_by_load);
        // AFTERLIGHT_CENSUS: what the simulation holds, system by system.
        app.add_systems(Update, simulation::sim_census.run_if(in_state(crate::states::GameState::Exploring)));
        // AFTERLIGHT_NAV_TRACE: who is stuck, and who is inside something.
        app.add_systems(
            Update,
            nav::nav_census.run_if(nav::trace).run_if(in_state(crate::states::GameState::Exploring)),
        );
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
        app.init_resource::<standing_spawns::Alarm>().add_systems(
            Update,
            standing_spawns::raise_alarm.run_if(in_state(crate::states::GameState::Exploring)),
        );
        app.init_resource::<standing_spawns::StandingSpawns>().add_systems(
            Update,
            (
                standing_spawns::standing_spawns,
                standing_spawns::war_fleets,
                standing_spawns::tick_missions,
                standing_spawns::allies_defend_player,
                traffic::run_traffic,
                simulation::hold_guard_posts,
                nav::refresh_nav_bodies,
                simulation::fly_departures,
                simulation::show_jump_flashes,
            )
                .run_if(in_state(crate::states::GameState::Exploring)),
        );
        app
            .init_resource::<WorldSimulation>()
            .init_resource::<awareness::Battlefield>()
            .init_resource::<awareness::Mind>()
            .init_resource::<tractor::PlayerDrive>()
            .init_resource::<ecm::Ecm>()
            .init_resource::<traffic::TradingFleets>()
            .init_resource::<nav::NavBodies>()
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
                    // A catcher's beams pull after it has moved, so a catch is
                    // carried where the catcher goes.
                    (movement::ai_ship_movement_system, tractor::ai_tractor_system.after(movement::ai_ship_movement_system), ecm::ai_ecm_system),
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
