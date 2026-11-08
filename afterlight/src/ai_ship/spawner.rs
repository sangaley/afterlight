use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::building::blueprint::{self, apply_module_extras, Blueprint, BlueprintHullCell};
use crate::building::registry::{CompanionData, ModuleRegistry};
use crate::components::*;
use crate::ship::spawn_module;

use super::components::*;
use super::layouts;
use super::roster;

/// Faction designs, loaded once per run. designs/factions/<slug>.json wins;
/// missing files are converted from the built-in layouts and self-exported
/// so every faction ship exists as editable JSON after the first encounter.
static DESIGN_CACHE: OnceLock<Mutex<HashMap<AiShipType, Blueprint>>> = OnceLock::new();

/// Class designs (designs/factions/<slug>/<key>.json), loaded once per run.
/// `None` records a class whose file is missing, so it is reported once and
/// then quietly flies the faction's original design.
static CLASS_CACHE: OnceLock<Mutex<HashMap<(AiShipType, &'static str), Option<Blueprint>>>> = OnceLock::new();

/// The design for one class of a faction, or its original design when the
/// class is None or its file can't be read. Never writes a file: a class that
/// fails to load is a mistake to fix, not something to paper over on disk.
fn class_design(ship_type: AiShipType, class: Option<&'static str>) -> Blueprint {
    // AFTERLIGHT_MOVETEST_ENEMY_DESIGN=<path>: in a movement test, every enemy
    // flies this design file (to watch an AI fight with a given ship).
    if crate::demo::skip_ai_ship_spawn() {
        if let Some(bp) = std::env::var("AFTERLIGHT_MOVETEST_ENEMY_DESIGN").ok().and_then(blueprint::load_design_file) {
            return bp;
        }
    }
    let Some(key) = class else { return faction_design(ship_type) };
    let cache = CLASS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let loaded = {
        let mut cache = cache.lock().unwrap();
        cache
            .entry((ship_type, key))
            .or_insert_with(|| {
                let path = format!("designs/factions/{}/{}.json", layouts::design_slug(ship_type), key);
                let design = blueprint::load_design_file(&path);
                if design.is_none() {
                    warn!("Ship class {} could not be loaded; flying the faction's original design", path);
                }
                design
            })
            .clone()
    };
    let design = loaded.unwrap_or_else(|| faction_design(ship_type));
    if crate::diplomacy::stations::station_plan(ship_type).is_some_and(|p| p.key == key) {
        return station_design(ship_type, key, &design);
    }
    design
}

/// How many times larger than its design file a faction station is built.
/// A station should dwarf the ships round it; the files now hold stations
/// at three times the size they were drawn (tools/ship-scale, armed with the
/// new weapons), so they're built as they stand. AFTERLIGHT_STATION_SCALE grows
/// them further.
pub fn station_scale() -> i32 {
    static V: OnceLock<i32> = OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_STATION_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1).clamp(1, 6))
}

/// A station's design at station scale, built once per run.
fn station_design(ship_type: AiShipType, key: &'static str, drawn: &Blueprint) -> Blueprint {
    static SCALED: OnceLock<Mutex<HashMap<(AiShipType, &'static str), Blueprint>>> = OnceLock::new();
    static SIZES: OnceLock<HashMap<crate::components::ModuleType, IVec2>> = OnceLock::new();
    let sizes = SIZES.get_or_init(|| {
        crate::building::registry::build_registry().defs.iter().map(|(t, d)| (*t, d.size)).collect()
    });
    SCALED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .entry((ship_type, key))
        .or_insert_with(|| blueprint::scale_design(drawn, station_scale(), |t| sizes.get(&t).copied().unwrap_or(IVec2::ONE)))
        .clone()
}

/// A copy of the player's own ship, held aside for the one enemy that is meant
/// to be built from it.
///
/// Set at launch (see narrative::doppelganger). Consulted only for the ship
/// type the story uses as its host, and only when armed; every other spawn
/// goes through the ordinary faction design and is unaffected.
static MIRROR_DESIGN: OnceLock<Mutex<Option<Blueprint>>> = OnceLock::new();

fn mirror_slot() -> &'static Mutex<Option<Blueprint>> {
    MIRROR_DESIGN.get_or_init(|| Mutex::new(None))
}

/// Hand the spawner the player's current design.
pub fn set_mirror_design(design: Option<Blueprint>) {
    if let Ok(mut slot) = mirror_slot().lock() {
        *slot = design;
    }
}

/// Is there a captured design waiting to be worn?
pub fn has_mirror_design() -> bool {
    mirror_slot().lock().map(|s| s.is_some()).unwrap_or(false)
}

/// Spawn one ship that is built from the player's own design.
///
/// Takes the design rather than cloning it into the faction cache, so the
/// next ordinary ship of that faction is still an ordinary ship. The point is
/// that one of them is wrong, not that the faction changes.
pub fn spawn_mirror_ship(
    ship_type: AiShipType,
    position: Vec2,
    commands: &mut Commands,
    registry: &ModuleRegistry,
    asset_server: &AssetServer,
) -> Option<Entity> {
    let design = mirror_slot().lock().ok()?.clone()?;
    let root = spawn_ai_ship_with_design(ship_type, position, commands, registry, asset_server, Some(design));
    commands.entity(root).insert(MirrorShip);
    Some(root)
}

fn faction_design(ship_type: AiShipType) -> Blueprint {
    let cache = DESIGN_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap();
    cache
        .entry(ship_type)
        .or_insert_with(|| {
            let slug = layouts::design_slug(ship_type);
            let path = format!("designs/factions/{}.json", slug);
            blueprint::load_design_file(&path).unwrap_or_else(|| {
                let design = layouts::get_layout(ship_type).to_design(slug);
                if let Err(e) = blueprint::write_design_file(&path, &design) {
                    warn!("Could not export faction design {}: {}", slug, e);
                }
                design
            })
        })
        .clone()
}

/// Root sprite bounds from the design's hull extent.
fn design_body_size(design: &Blueprint) -> Vec2 {
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for cell in &design.hull_cells {
        min_x = min_x.min(cell.grid_pos.x);
        max_x = max_x.max(cell.grid_pos.x);
        min_y = min_y.min(cell.grid_pos.y);
        max_y = max_y.max(cell.grid_pos.y);
    }
    if design.hull_cells.is_empty() {
        return Vec2::splat(66.0);
    }
    Vec2::new(
        (max_x - min_x + 1) as f32 * 66.0,
        (max_y - min_y + 1) as f32 * 66.0,
    )
}

/// How much tougher and harder-hitting a ship gets based on its faction's
/// fixed danger tier (faction_power, 0.1..8.0) — replaces the old raw-
/// distance-from-Haven scaling now that danger is a per-system fixed
/// assignment (see StarSystemDef.danger_tier, celestial/galaxy.rs) instead
/// of a continuous function of how far you've traveled. Same output shape
/// as before (health up to 2.5x, damage up to 1.8x) so existing combat
/// balance doesn't need re-tuning from scratch — only the INPUT changed.
fn faction_difficulty(ship_type: AiShipType) -> (f32, f32) {
    let t = (super::components::faction_power(ship_type) / 8.0).clamp(0.0, 1.0);
    let health_mult = 1.0 + t * 1.5; // up to 2.5x hull HP for the toughest factions
    let damage_mult = 1.0 + t * 0.8; // up to 1.8x weapon damage for the toughest factions
    (health_mult, damage_mult * filming_damage())
}

/// AFTERLIGHT_AI_DAMAGE=<k>: a filming switch -- AI weapons hit k times harder, so
/// a battle filmed for a few seconds has ships going down in it. The shots
/// and explosions are the game's own; only how many it takes changes. 1 in play.
fn filming_damage() -> f32 {
    static V: OnceLock<f32> = OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_AI_DAMAGE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0))
}

/// Color tint per AI ship type
fn ship_tint(ship_type: AiShipType) -> Color {
    match ship_type {
        AiShipType::Shepherd => Color::srgb(0.75, 0.6, 0.1),     // molten gold — apex boss
        AiShipType::EternalHegemony => Color::srgb(0.5, 0.08, 0.08),  // deep crimson — mega-battleship
        AiShipType::StellarPreserve => Color::srgb(0.2, 0.6, 0.5),    // teal - creature riders
        AiShipType::SynthesisCollective => Color::srgb(0.4, 0.15, 0.5),  // purple - bio-organic cult
        AiShipType::BrokenChoir => Color::srgb(0.35, 0.4, 0.35),     // ghostly gray-green
        AiShipType::CorpseStars => Color::srgb(0.15, 0.1, 0.25),// dark violet - deep lords
        AiShipType::TheSilence => Color::srgb(0.85, 0.88, 0.9),    // white/translucent
        AiShipType::TerranHegemony => Color::srgb(0.45, 0.45, 0.5),    // steel gray - battleship
        AiShipType::GildedThrone => Color::srgb(0.2, 0.2, 0.25),   // dark tactical
        AiShipType::RecursiveKingdom => Color::srgb(0.7, 0.4, 0.15),    // rusty orange
        AiShipType::MerchantPrinces => Color::srgb(0.4, 0.55, 0.35),    // guild green
        AiShipType::FreeHaulersCompact => Color::srgb(0.6, 0.35, 0.22), // rust
    }
}

/// Spawns a complete AI ship of a class picked from the faction's roster.
/// Returns the root entity.
pub fn spawn_ai_ship(
    ship_type: AiShipType,
    position: Vec2,
    commands: &mut Commands,
    registry: &ModuleRegistry,
    asset_server: &AssetServer,
) -> Entity {
    let class = roster::roll(ship_type, &mut rand::thread_rng(), |_| true).map(|c| c.key);
    spawn_ai_ship_class(ship_type, class, position, commands, registry, asset_server)
}

/// Spawns one ship of a given class (None = the faction's original design).
pub fn spawn_ai_ship_class(
    ship_type: AiShipType,
    class: Option<&'static str>,
    position: Vec2,
    commands: &mut Commands,
    registry: &ModuleRegistry,
    asset_server: &AssetServer,
) -> Entity {
    spawn_ai_ship_wounded(ship_type, class, None, position, commands, registry, asset_server)
}

/// The design a ship of this class is built from (None = the faction's own).
pub fn built_design(ship_type: AiShipType, class: Option<&'static str>) -> Blueprint {
    class_design(ship_type, class)
}

/// Hull cells + modules in a class's design: what one of these costs the
/// live-ship budget. Cached; called for every spawn candidate.
pub fn design_blocks(ship_type: AiShipType, class: Option<&'static str>) -> usize {
    static SIZES: OnceLock<Mutex<HashMap<(AiShipType, Option<&'static str>), usize>>> = OnceLock::new();
    let sizes = SIZES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(&n) = sizes.lock().unwrap().get(&(ship_type, class)) {
        return n;
    }
    let design = class_design(ship_type, class);
    let n = design.hull_cells.len() + design.modules.len();
    sizes.lock().unwrap().insert((ship_type, class), n);
    n
}

/// How far a class's hull reaches from the ship's position, in world units:
/// its farthest cell, plus one. Cells sit at grid_pos x 66 from the root,
/// uncentred, so this is from the root, not the hull's middle. What a
/// formation spaces ships by. Cached.
pub fn design_extent(ship_type: AiShipType, class: Option<&'static str>) -> f32 {
    static EXTENTS: OnceLock<Mutex<HashMap<(AiShipType, Option<&'static str>), f32>>> = OnceLock::new();
    let extents = EXTENTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(&r) = extents.lock().unwrap().get(&(ship_type, class)) {
        return r;
    }
    let design = class_design(ship_type, class);
    let r = if design.hull_cells.is_empty() {
        300.0
    } else {
        design.hull_cells.iter().map(|c| c.grid_pos.as_vec2().length()).fold(0.0_f32, f32::max) * 66.0 + 66.0
    };
    extents.lock().unwrap().insert((ship_type, class), r);
    r
}

/// A class's design with the blocks it lost before going off-screen taken out.
pub fn wounded_design(built: &Blueprint, wounds: &Wounds) -> Blueprint {
    let mut design = built.clone();
    design.hull_cells.retain(|c| !wounds.hull.contains(&c.grid_pos));
    design.modules.retain(|m| match m.extras.as_ref().and_then(|e| e.turret) {
        Some(ring) => !wounds.turret_parts.contains(&(ring, m.grid_pos)),
        None => !wounds.modules.contains(&m.grid_pos),
    });
    design
}

/// Spawns a ship of a class carrying the damage it left with. Every "how
/// much is left" ratio still divides by the ship as BUILT, so a battleship
/// that came back with half its guns reads as half-armed, not fully armed.
pub fn spawn_ai_ship_wounded(
    ship_type: AiShipType,
    class: Option<&'static str>,
    wounds: Option<&Wounds>,
    position: Vec2,
    commands: &mut Commands,
    registry: &ModuleRegistry,
    asset_server: &AssetServer,
) -> Entity {
    let built = class_design(ship_type, class);
    let wounds = wounds.filter(|w| !w.is_empty());
    let design = wounds.map_or_else(|| built.clone(), |w| wounded_design(&built, w));
    let root = spawn_ai_ship_with_design(ship_type, position, commands, registry, asset_server, Some(design));
    if let Some(w) = wounds {
        let (health_mult, _) = faction_difficulty(ship_type);
        commands.entity(root).insert((built_loadout(&built, health_mult, registry), SpawnIntegrity(w.integrity)));
    }
    if let Some(key) = class {
        commands.entity(root).insert(AiShipClass(key));
        if super::roster::find(ship_type, key).is_some_and(|c| c.role == super::roster::ClassRole::Civilian) {
            commands.entity(root).insert(super::fleet::CivilianHull);
        }
    }
    root
}

/// The loadout of a design as built: guns, engines, and the hull health it
/// starts with (the same per-cell health spawn_ai_hull gives).
fn built_loadout(design: &Blueprint, health_mult: f32, registry: &ModuleRegistry) -> AiLoadout {
    let hull_max = design
        .hull_cells
        .iter()
        .map(|c| c.material.health_multiplier() * 100.0 * health_mult)
        .sum();
    loadout_of(design, hull_max, registry)
}

/// What a design carries that a ship can be beaten by losing: its guns,
/// engines, command and power.
fn loadout_of(design: &Blueprint, hull_max: f32, registry: &ModuleRegistry) -> AiLoadout {
    let count = |class: fn(ModuleType) -> bool| design.modules.iter().filter(|m| class(m.module_type)).count() as u32;
    AiLoadout {
        guns: count(super::combat::is_weapon_type),
        engines: count(super::combat::is_engine_type),
        command: count(super::combat::is_command_type),
        power_mw: design.modules.iter().map(|m| registry.get(m.module_type).power_generation).sum(),
        hull_max,
    }
}

/// As `spawn_ai_ship`, but optionally wearing a design that is not this
/// faction's own.
pub fn spawn_ai_ship_with_design(
    ship_type: AiShipType,
    position: Vec2,
    commands: &mut Commands,
    registry: &ModuleRegistry,
    asset_server: &AssetServer,
    override_design: Option<Blueprint>,
) -> Entity {
    let design = override_design.unwrap_or_else(|| faction_design(ship_type));
    let body_size = design_body_size(&design);

    let mut rng = rand::thread_rng();

    // Generate patrol waypoints around spawn position. Wider than the old
    // 300-600 range so a rendered ship visibly roams across the screen
    // instead of pacing a tight little loop right where it spawned.
    let waypoints: Vec<Vec2> = (0..4)
        .map(|i| {
            let angle = std::f32::consts::TAU * (i as f32 / 4.0) + rng.gen_range(-0.3..0.3);
            let dist = rng.gen_range(500.0..1000.0);
            position + Vec2::new(angle.cos() * dist, angle.sin() * dist)
        })
        .collect();

    let initial_behavior = match ship_type {
        AiShipType::Shepherd => AiShipBehavior::Patrolling,
        AiShipType::EternalHegemony => AiShipBehavior::Patrolling,
        AiShipType::StellarPreserve => AiShipBehavior::Patrolling,
        AiShipType::SynthesisCollective => AiShipBehavior::Patrolling,
        AiShipType::BrokenChoir => AiShipBehavior::Patrolling,
        AiShipType::CorpseStars => AiShipBehavior::Patrolling,
        AiShipType::TheSilence => AiShipBehavior::FollowingTradeRoute,
        AiShipType::TerranHegemony => AiShipBehavior::Patrolling,
        AiShipType::GildedThrone => AiShipBehavior::Patrolling,
        AiShipType::RecursiveKingdom => AiShipBehavior::Patrolling,
        AiShipType::MerchantPrinces | AiShipType::FreeHaulersCompact => AiShipBehavior::Patrolling,
    };

    let fuel = match ship_type {
        AiShipType::Shepherd => 3000.0,
        AiShipType::EternalHegemony => 2000.0,
        AiShipType::StellarPreserve => 400.0,
        AiShipType::SynthesisCollective => 600.0,
        AiShipType::BrokenChoir => 200.0,
        AiShipType::CorpseStars => 800.0,
        AiShipType::TheSilence => 500.0,
        AiShipType::TerranHegemony => 1000.0,
        AiShipType::GildedThrone => 700.0,
        AiShipType::RecursiveKingdom => 150.0,
        AiShipType::MerchantPrinces => 900.0,
        AiShipType::FreeHaulersCompact => 1100.0,
    };

    // Compute initial rotation: face toward first waypoint (or default to 0 = facing right)
    let initial_rotation = if let Some(&first_wp) = waypoints.first() {
        let dir = first_wp - position;
        if dir.length_squared() > 0.01 {
            dir.y.atan2(dir.x)
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Spawn root entity (body sprite invisible — hull cells provide the visual)
    let root = commands.spawn((
        (Sprite {
                image: asset_server.load(crate::sprite_map::effect_sprite_path("ship_body")),
                color: Color::NONE,
                custom_size: Some(body_size),
                ..default()
            }, Transform {
                translation: Vec3::new(position.x, position.y, 0.05),
                rotation: Quat::from_rotation_z(initial_rotation),
                ..default()
            }),
        AiShip,
        ship_type,
        AiShipState {
            fuel,
            max_fuel: fuel,
            ..default()
        },
        initial_behavior,
        AiShipNav {
            waypoints: waypoints.clone(),
            destination: waypoints.first().copied(),
            ..default()
        },
        AiShipDecisionTimer::default(),
        AiShipTarget::default(),
        AiGunneryLog::default(),
        Velocity(Vec2::ZERO),
        Depth(position.y.abs() / 10.0),
    )).id();

    let (health_mult, damage_mult) = faction_difficulty(ship_type);

    // Enemy ships get a deck, the same way the player's does.
    //
    // Every faction hull shipped as solid Inner fill with zero hallway cells,
    // which had two consequences. Visually, an enemy was a flat slab of
    // plating with no inside to see. Mechanically, `rebuild_nav_grids` builds
    // a grid for every AI ship and only hallways are walkable, so every one of
    // those grids came out empty -- work producing nothing.
    //
    // Derived at spawn rather than baked into designs/factions/*.json, so it
    // applies to every source of a hull: the authored layouts, the exported
    // files, and a mirror design wearing the player's own ship.
    let mut hull_cells = design.hull_cells.clone();
    crate::ship::lay_hallways_with(registry, &mut hull_cells, &design.modules);

    // Spawn hull segments as children
    let paint = super::livery::livery(ship_type);
    let hull_max = spawn_ai_hull(commands, asset_server, root, &hull_cells, health_mult, &paint);
    let (lo, hi) = hull_cells.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| (lo.min(c.grid_pos), hi.max(c.grid_pos)));

    // The loadout as built. Every "how much is left" ratio divides by this,
    // because dead blocks are despawned and can't be counted later.
    commands.entity(root).insert(loadout_of(&design, hull_max, registry));

    // Spawn modules as children, reusing existing spawn_module
    for mp in &design.modules {
        let module_entity = spawn_module(
            commands,
            asset_server,
            root,
            mp.module_type,
            mp.grid_pos,
            mp.rotation,
            registry,
        );
        commands.entity(module_entity).insert(OwnedByAiShip { root });
        // Armour plates wear the faction's paint, like the hull they sit on.
        if super::livery::is_plating(mp.module_type) {
            let accent = paint.accent_at(mp.grid_pos, lo, hi);
            commands.entity(module_entity).insert(super::livery::Paint(paint.paint(Color::srgb(0.6, 0.6, 0.65), accent)));
        }

        // Faction design state — a design file can ship tuned guns, fire
        // groups, ammo choices, and they apply here like on the player ship
        if let Some(extras) = &mp.extras {
            apply_module_extras(commands, module_entity, extras);
        }

        // Tougher factions hit harder too — scale weapon damage by the same
        // danger factor (see faction_difficulty doc comment).
        if damage_mult != 1.0 {
            if let CompanionData::Weapon { damage, range, fire_rate, ammo, .. } = &registry.get(mp.module_type).companion {
                commands.entity(module_entity).insert(Weapon {
                    damage: damage * damage_mult,
                    range: *range,
                    fire_rate: *fire_rate,
                    ammo: *ammo,
                    max_ammo: *ammo,
                });
            }
        }
    }

    // Crew: count this design's crew-eligible stations (Reactor/Engine/
    // Weapon/etc — whatever the registry marks crew_station:true) and staff
    // a per-faction fraction of them. auto_assign_crew (crew::mod.rs) does
    // the actual station assignment on its own timer, same as the player,
    // by priority: Power(10) > Propulsion(9) > LifeSupport(8) > Weapons(6).
    //
    // Floor: a small ship's non-weapon stations alone can eat its entire
    // complement before priority ever reaches a gun — Recursive Kingdom (2 crew for
    // Reactor+Engine+2 Gatlings) staffed Reactor+Engine and left BOTH guns
    // permanently dark, not "half dark" as crew_fill_fraction intended.
    // Guarantee at least one weapon slot for any faction that has weapons,
    // so "understaffed" always means fewer guns firing, never zero.
    //
    // The floor scales with weapon-station count, not a flat +1: a flat
    // floor barely dents a weapon-heavy design (Broken Choir's 5 weapon stations
    // vs Recursive Kingdom's 2) — same fraction, wildly different dark-gun ratio
    // (Broken Choir ended up at 1-of-5 firing, effectively silent, while
    // Recursive Kingdom's 1-of-2 matched its 0.6 fraction reasonably well).
    let non_weapon_stations = design.modules.iter()
        .filter(|m| {
            let def = registry.get(m.module_type);
            def.crew_station && def.category != crate::components::ModuleCategory::Weapons
        })
        .count();
    let total_stations = design.modules.iter()
        .filter(|m| registry.get(m.module_type).crew_station)
        .count();
    let weapon_stations = total_stations.saturating_sub(non_weapon_stations);
    let has_weapons = weapon_stations > 0;
    let fraction_complement = ((total_stations as f32) * super::components::crew_fill_fraction(ship_type)).round() as u32;
    let min_floor = if has_weapons {
        let weapon_floor = ((weapon_stations as f32) * super::components::crew_fill_fraction(ship_type)).round().max(1.0) as u32;
        (non_weapon_stations as u32 + weapon_floor).min(total_stations as u32)
    } else {
        1
    };
    // crew_fill_fraction is a per-faction TARGET, not a fixed headcount —
    // every Recursive Kingdom ship landing on exactly the same crew size read as
    // robotic. Roll each spawn's actual complement from a triangular
    // distribution: `typical` (the old deterministic value) is the peak, so
    // most ships land near it, but `min_floor` (skeleton crew) and
    // `full_complement` (every station manned) are both reachable, rarely.
    let typical_complement = fraction_complement.max(min_floor);
    let full_complement = typical_complement.max(total_stations as u32);
    let crew_complement = roll_crew_complement(&mut rng, min_floor, typical_complement, full_complement);
    super::crew::spawn_ai_crew(commands, root, crew_complement);
    commands.entity(root).insert(super::air::AiCrewComplement(crew_complement));

    root
}

/// Samples a crew headcount from a triangular distribution — `floor` and
/// `ceiling` are the hard bounds, `typical` is the peak (most spawns land
/// near it). Standard inverse-transform sampling for a triangular
/// distribution; degenerates to a flat `floor` if there's no room to roll.
fn roll_crew_complement(rng: &mut impl Rng, floor: u32, typical: u32, ceiling: u32) -> u32 {
    if ceiling <= floor {
        return floor;
    }
    let (min, mode, max) = (floor as f32, typical.clamp(floor, ceiling) as f32, ceiling as f32);
    let u: f32 = rng.gen_range(0.0..1.0);
    let split = (mode - min) / (max - min);
    let sample = if u < split {
        min + (u * (max - min) * (mode - min)).sqrt()
    } else {
        max - ((1.0 - u) * (max - min) * (max - mode)).sqrt()
    };
    sample.round() as u32
}

/// The tint an AI hull block's texture is drawn with (and its far-off
/// preview, see preview::cell_colour).
pub fn hull_tint(material: HullMaterial) -> Color {
    match material {
        HullMaterial::Steel => Color::srgb(0.55, 0.55, 0.6),
        HullMaterial::Titanium => Color::srgb(0.7, 0.7, 0.75),
        HullMaterial::Composite => Color::srgb(0.45, 0.62, 0.5),
        HullMaterial::AbyssalAlloy => Color::srgb(0.35, 0.28, 0.45),
    }
}

/// Spawns hull segment children for the AI ship. Returns the summed max
/// health of everything it spawned -- the ship's hull as built.
fn spawn_ai_hull(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    hull_cells: &[BlueprintHullCell],
    health_mult: f32,
    paint: &super::livery::Livery,
) -> f32 {
    let mut hull_max = 0.0;
    let (lo, hi) = hull_cells.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| (lo.min(c.grid_pos), hi.max(c.grid_pos)));
    for cell in hull_cells {
        let health = cell.material.health_multiplier() * 100.0 * health_mult;
        let radiation_shielding = cell.material.radiation_shielding();

        let x = cell.grid_pos.x as f32 * 66.0;
        let y = cell.grid_pos.y as f32 * 66.0 - 33.0;

        // The outer plating wears the faction's paint (ai_ship::livery);
        // what's inside keeps the bare metal.
        let hull_color = if cell.layer == HullLayer::Outer {
            paint.paint(hull_tint(cell.material), paint.accent_at(cell.grid_pos, lo, hi))
        } else {
            hull_tint(cell.material)
        };

        let hull_entity = commands.spawn((
            (Sprite {
                    image: asset_server.load(
                        crate::sprite_map::hull_layer_sprite_path(cell.material, cell.layer),
                    ),
                    color: hull_color,
                    // Full cell. Enemy hulls were drawn at 60 in a 66-unit grid,
                    // so every AI ship had a 9% gap between its blocks too.
                    custom_size: Some(Vec2::splat(66.0)),
                    ..default()
                }, Transform::from_xyz(x, y, 0.1)),
            BaseSpriteColor(hull_color),
            BaseHullStats {
                max_health: health,
                radiation_shielding,
            },
            HullSegment {
                health,
                max_health: health,
                radiation_shielding,
                is_depressurized: false,
                depressurization_level: 0.0,
                hull_layer: cell.layer,
                material: cell.material,
                grid_position: cell.grid_pos,
            },
            crate::building::Block::hull(cell.grid_pos, cell.material),
            OwnedByAiShip { root: parent },
        )).id();

        commands.entity(hull_entity).insert(ChildOf(parent));
        hull_max += health;
    }
    hull_max
}

#[cfg(test)]
mod wound_tests {
    use super::*;

    /// A wounded ship is its design minus exactly the blocks it lost.
    #[test]
    fn a_wounded_design_is_missing_only_what_was_lost() {
        let built = built_design(AiShipType::TerranHegemony, crate::ai_ship::roster::intern("bulwark"));
        let lost_cell = built.hull_cells[5].grid_pos;
        let lost_module = built.modules[3].grid_pos;
        let wounds = Wounds { hull: vec![lost_cell], modules: vec![lost_module], turret_parts: vec![], integrity: 0.9 };
        let hurt = wounded_design(&built, &wounds);
        assert_eq!(hurt.hull_cells.len(), built.hull_cells.len() - 1);
        assert!(hurt.modules.len() < built.modules.len());
        assert!(hurt.hull_cells.iter().all(|c| c.grid_pos != lost_cell));
        assert!(hurt.modules.iter().all(|m| m.grid_pos != lost_module));
    }

    /// Turret gun parts stand on their turret's own cells, so they are lost
    /// by ring and cell, and a hull block lost at the same numbers is not one
    /// of them. (Every turret gun used to come back from a load missing.)
    #[test]
    fn turret_parts_are_wounded_by_ring_and_cell() {
        let built = built_design(AiShipType::EternalHegemony, crate::ai_ship::roster::intern("vigil"));
        let ring = |m: &blueprint::BlueprintModule| m.extras.as_ref().and_then(|e| e.turret);
        let parts = built.modules.iter().filter(|m| ring(m).is_some()).count();
        assert!(parts > 40, "the Vigil's turret guns are in its design ({parts} parts)");
        let barrel = built.modules.iter().find(|m| m.module_type == ModuleType::GunBarrel && ring(m).is_some()).unwrap();
        let wounds = Wounds {
            hull: vec![],
            modules: vec![barrel.grid_pos],
            turret_parts: vec![(ring(barrel).unwrap(), barrel.grid_pos)],
            integrity: 0.9,
        };
        let hurt = wounded_design(&built, &wounds);
        assert_eq!(hurt.modules.iter().filter(|m| ring(m).is_some()).count(), parts - 1);
        assert!(!hurt.modules.iter().any(|m| ring(m) == ring(barrel) && m.grid_pos == barrel.grid_pos));
    }

    #[test]
    fn the_budget_counts_blocks_not_ships() {
        let skiff = design_blocks(AiShipType::Shepherd, crate::ai_ship::roster::intern("lamb"));
        let boss = design_blocks(AiShipType::Shepherd, crate::ai_ship::roster::intern("the_shepherd"));
        assert!(boss > skiff * 10, "a boss costs far more than an attendant ({boss} vs {skiff})");
    }
}
