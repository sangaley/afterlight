pub mod celestial_visuals;
pub mod particles;
pub mod screen_effects;
pub mod block_visuals;
pub mod hull_seams;
pub mod ship_roof;
pub mod far_view;
pub mod far_art;
pub mod block_height;
pub mod starfield;
pub mod procedural_textures;
pub mod debris;
pub mod effect_textures;

use bevy::prelude::*;
use crate::states::GameState;

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, (
                procedural_textures::generate_celestial_textures,
                effect_textures::load_effect_textures,
            ))
            .add_systems(
                Update,
                (
                    celestial_visuals::animate_star_glow,
                    celestial_visuals::animate_star_flare_buildup,
                    celestial_visuals::animate_black_hole_disk,
                    celestial_visuals::animate_planet_atmosphere,
                    particles::spawn_engine_particles,
                    particles::spawn_breach_particles,
                    particles::update_particles,
                    particles::update_blasts,
                    debris::spawn_block_debris,
                    debris::update_debris,
                    screen_effects::update_screen_effects,
                ).run_if(in_state(GameState::Exploring)),
            )
            // Spawn visual layers when celestial bodies are created
            .add_systems(
                Update,
                (
                    celestial_visuals::attach_star_visuals,
                    celestial_visuals::attach_planet_visuals,
                    celestial_visuals::attach_black_hole_visuals,
                    // After orbits move the planets, so the night side is
                    // aimed from where the planet is this frame, not last.
                    celestial_visuals::spin_and_light_planets
                        .after(crate::celestial::orbits::update_orbital_positions),
                ).run_if(in_state(GameState::Exploring)),
            )
            // Block visuals DISABLED: this bolted crude colored-rectangle
            // silhouettes (barrel stubs, cores, rails) onto every module to
            // give the old flat-placeholder art some shape. Every module now
            // has a real detailed sprite, so these overlays just clashed —
            // "old graphics on top of the new ones". Left in the tree
            // commented rather than deleted in case a per-module overlay is
            // wanted again later.
            // .add_systems(Update, block_visuals::attach_block_visuals)
            // Parallax starfield — menu, docked and exploring, so the void
            // reads as space and motion is always perceptible. The menu was
            // excluded and so opened on text over a flat colour, with the
            // starfield the game already draws sitting one state away.
            // Hull tiles pick their own seams from their neighbours. Visual
            // only, and gated internally on the live tile count, so it costs
            // an archetype scan on the frames where nothing was built or shot.
            // Zoomed out, the ship shows plating instead of its insides.
            .add_systems(
                Update,
                (ship_roof::update_ship_roof, ship_roof::raise_weapons_over_roof).run_if(in_state(GameState::Exploring)),
            )
            .add_systems(OnExit(GameState::Exploring), (ship_roof::clear_ship_roof, ship_roof::lower_weapons))
            // Far out, every enemy carries its faction and class.
            .add_systems(
                Update,
                (far_view::attach_far_view, far_view::measure_far_view, far_view::show_far_view)
                    .chain()
                    .run_if(in_state(GameState::Exploring)),
            )
            .register_required_components::<crate::components::Ship, ship_roof::Scars>()
            .register_required_components::<crate::ai_ship::components::AiShip, ship_roof::Scars>()
            .add_systems(Update, ship_roof::note_scars.before(ship_roof::update_ship_roof).run_if(in_state(GameState::Exploring)))
            .add_systems(Update, far_view::label_ships.run_if(in_state(GameState::Exploring)))
            .add_systems(OnExit(GameState::Exploring), far_view::clear_far_labels)
            // Further out still, each ship and turret is one baked picture
            // of itself instead of hundreds of tiny blocks.
            .add_systems(
                Update,
                (far_art::attach_far_art, far_art::bake_far_art, far_art::show_far_art)
                    .chain()
                    .run_if(in_state(GameState::Exploring)),
            )
            .add_systems(OnExit(GameState::Exploring), far_art::clear_far_art)
            // Tall blocks (towers, masts, guns) cast a shadow, so they read as
            // standing up off the hull. Wherever ships are drawn: the berth
            // and the builder show the ship too.
            .add_systems(
                PostUpdate,
                (block_height::attach_height_shadows, block_height::place_height_shadows)
                    .chain()
                    .before(bevy::transform::TransformSystems::Propagate),
            )
            .add_systems(
                Update,
                hull_seams::update_hull_seams.run_if(
                    in_state(GameState::Exploring).or_else(in_state(GameState::StationDocked))
                ),
            )
            .add_systems(
                Update,
                (
                    starfield::spawn_starfield,
                    // Scrolls by how far the camera moved, so it has to see
                    // this frame's move: run before the follow and the
                    // background lagged a frame and shuddered at speed.
                    starfield::update_starfield
                        .after(starfield::spawn_starfield)
                        .after(crate::camera::camera_follow_ship),
                ).run_if(
                    in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))
                        .or_else(in_state(GameState::MainMenu))
                ),
            );
    }
}
