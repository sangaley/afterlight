use bevy::prelude::*;
use rand::prelude::*;
use crate::components::*;
use crate::sprite_map;

/// How far from the world origin the chunk layer stops placing points of
/// interest. Covers the spawn berth and Haven's station beside it.
const SPAWN_KEEP_CLEAR: f32 = 1_400.0;

/// Generates a chunk at the given position
pub fn generate_chunk(
    commands: &mut Commands,
    asset_server: &AssetServer,
    chunk_pos: IVec2,
    seed: u64,
) -> Entity {
    let mut rng = StdRng::seed_from_u64(seed ^ (chunk_pos.x as u64) ^ ((chunk_pos.y as u64) << 32));

    // Position chunk entity at its world-space location so children are offset correctly
    let chunk_world_x = chunk_pos.x as f32 * 512.0;
    let chunk_world_y = chunk_pos.y as f32 * 512.0;

    let chunk = commands.spawn((
        Transform::from_xyz(chunk_world_x, chunk_world_y, 0.0),
        Chunk {
            position: chunk_pos,
            is_explored: false,
        },
    )).id();

    let depth_level = -chunk_pos.y; // Lower Y = deeper

    // Skip chunks above the surface or below max depth (500m = depth_level ~10)
    if depth_level < 0 || depth_level > 11 {
        return chunk;
    }

    // Depth 9+ used to be "seafloor" and got generated terrain; there is no
    // terrain any more (rock is celestial-layer asteroids now), but the band
    // is still a useful divider for WHICH points of interest appear: deep
    // chunks favour ruins and vents, shallow ones wrecks and caves.
    let deep_level = 9;
    let is_deep_chunk = depth_level >= deep_level;

    // Keep the berth clear. The ship spawns at roughly the world origin and
    // Haven's station sits just beside it, while chunk points of interest
    // land within a couple of hundred units of their chunk corner — so the
    // chunk under the spawn reliably put a wreck within the 500-unit log
    // pickup radius. The player was handed their first log before they had
    // touched a control, which is the opposite of the opening the story wants.
    let chunk_center = Vec2::new(chunk_world_x + 256.0, chunk_world_y + 256.0);
    if chunk_center.distance(crate::world::home_base::SPAWN_BERTH) < SPAWN_KEEP_CLEAR {
        return chunk;
    }
    // Nor inside or against Haven itself, now that it's kilometres across.
    let haven = crate::world::home_base::STATION_POS;
    if chunk_center.distance(haven) < crate::world::home_base::HAVEN_RADIUS + SPAWN_KEEP_CLEAR {
        return chunk;
    }

    // --- Settlements at fixed depth intervals ---
    if depth_level > 0 && depth_level % 4 == 0 && (chunk_pos.x.abs() % 3 == 0) {
        spawn_poi(commands, asset_server, chunk, PoiType::Settlement, depth_level, &mut rng);
    }

    // --- POIs scattered in open void ---
    if depth_level > 0 && depth_level < deep_level {
        let poi_chance = 0.20 + (depth_level as f32 * 0.02).min(0.2);
        if rng.gen::<f32>() < poi_chance {
            let poi_type = match depth_level {
                d if d < 3 => PoiType::Wreck,
                d if d < 6 => {
                    let roll = rng.gen::<f32>();
                    if roll < 0.4 { PoiType::Wreck }
                    else if roll < 0.7 { PoiType::Cave }
                    else { PoiType::ThermalVent }
                }
                _ => {
                    let roll = rng.gen::<f32>();
                    if roll < 0.3 { PoiType::Ruins }
                    else if roll < 0.6 { PoiType::Cave }
                    else if roll < 0.8 { PoiType::ThermalVent }
                    else { PoiType::Wreck }
                }
            };
            spawn_poi(commands, asset_server, chunk, poi_type, depth_level, &mut rng);
        }
    }

    // --- POIs in the deep band ---
    if is_deep_chunk {
        if rng.gen::<f32>() < 0.4 {
            let poi_type = if rng.gen::<f32>() < 0.5 { PoiType::Ruins } else { PoiType::ThermalVent };
            spawn_poi(commands, asset_server, chunk, poi_type, depth_level, &mut rng);
        }
    }

    // Drifting particulate, for a sense of motion between the big rocks.
    // The old seafloor branch (terrain columns, cliffs, cave mouths, spore
    // stalks anchored to a ground line) was submarine-era code generating a
    // seabed in open space; asteroids now come from the celestial layer
    // (celestial::spawning::spawn_asteroid_field), which already carries
    // collision, mining and per-system streaming.
    if depth_level > 0 {
        spawn_void_particles(commands, chunk, depth_level, &mut rng);
    }

    chunk
}

/// Spawns floating particles/debris in open void chunks for visual reference when moving
fn spawn_void_particles(
    commands: &mut Commands,
    parent: Entity,
    depth_level: i32,
    rng: &mut StdRng,
) {
    let particle_count = rng.gen_range(8..16);
    for _ in 0..particle_count {
        let x = rng.gen_range(-250.0..250.0_f32);
        let y = rng.gen_range(-250.0..250.0_f32);
        let size = rng.gen_range(2.0..6.0_f32);

        // Particles get dimmer with depth
        let brightness = (0.4 - depth_level as f32 * 0.03).max(0.08);
        let alpha = rng.gen_range(0.15..0.4_f32);

        commands.spawn((
            (Sprite {
                    color: Color::srgba(brightness, brightness + 0.05, brightness + 0.1, alpha),
                    custom_size: Some(Vec2::new(size, size)),
                    ..default()
                }, Transform::from_xyz(x, y, -0.1)),
            WorldDecoration { decoration_type: DecorationType::Dust },
        )).insert(ChildOf(parent));
    }

    // Occasional larger debris/silt clouds
    if rng.gen::<f32>() < 0.3 {
        let x = rng.gen_range(-200.0..200.0_f32);
        let y = rng.gen_range(-200.0..200.0_f32);
        let w = rng.gen_range(20.0..60.0_f32);
        let h = rng.gen_range(10.0..30.0_f32);
        let brightness = (0.25 - depth_level as f32 * 0.02).max(0.05);

        commands.spawn((
            (Sprite {
                    color: Color::srgba(brightness, brightness, brightness + 0.03, 0.15),
                    custom_size: Some(Vec2::new(w, h)),
                    ..default()
                }, Transform {
                    translation: Vec3::new(x, y, -0.1),
                    rotation: Quat::from_rotation_z(rng.gen_range(0.0..std::f32::consts::TAU)),
                    ..default()
                }),
            WorldDecoration { decoration_type: DecorationType::Dust },
        )).insert(ChildOf(parent));
    }
}

fn spawn_poi(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    poi_type: PoiType,
    depth_level: i32,
    rng: &mut StdRng,
) {
    let offset = Vec2::new(
        rng.gen_range(-200.0..200.0),
        rng.gen_range(-200.0..200.0),
    );

    let (color, size) = match poi_type {
        PoiType::Wreck => (Color::srgb(0.5, 0.4, 0.3), Vec2::new(400.0, 180.0)),
        PoiType::Cave => (Color::srgb(0.15, 0.15, 0.18), Vec2::new(350.0, 280.0)),
        PoiType::Ruins => (Color::srgb(0.35, 0.35, 0.45), Vec2::new(450.0, 300.0)),
        PoiType::ThermalVent => (Color::srgb(0.8, 0.3, 0.1), Vec2::new(200.0, 320.0)),
        PoiType::Settlement => (Color::srgb(0.3, 0.7, 0.4), Vec2::new(500.0, 350.0)),
    };

    let texture = asset_server.load(sprite_map::poi_sprite_path(poi_type));

    let mut entity_commands = commands.spawn((
        (Sprite {
                image: texture,
                color,
                custom_size: Some(size),
                ..default()
            }, Transform::from_xyz(offset.x, offset.y, -0.1)),
        PointOfInterest {
            poi_type,
            discovered: false,
        },
    ));

    // Wrecks get Wreck component for salvage
    if poi_type == PoiType::Wreck {
        entity_commands.insert(Wreck {
            loot_remaining: rng.gen_range(1..=4),
            is_explored: false,
        });
    }

    // ThermalVents get HazardZone component for damage
    if poi_type == PoiType::ThermalVent {
        entity_commands.insert(HazardZone {
            hazard_type: HazardType::ThermalVent,
            radius: 120.0,
            damage_per_second: 3.0 + depth_level as f32 * 0.5,
        });
    }

    // No expedition records on this layer. It only exists around Haven, and
    // rolling records onto its dense wrecks, ruins and caves handed a new
    // player half the demo's trail in three minutes, then nothing. Every
    // record now has one planned home on a system derelict (narrative::trail).


    entity_commands.insert(ChildOf(parent));
}
