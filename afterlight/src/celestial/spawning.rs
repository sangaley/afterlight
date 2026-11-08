use bevy::prelude::*;
use rand::Rng;
use super::components::*;
use super::resources::*;
use super::poi::{SpacePoi, SpacePoiType, MineableResource, ResourceNodeType};
use crate::vfx::procedural_textures::CelestialTextures;

/// Generate a star system at a given center position.
/// Returns the StarSystemInfo for tracking.
/// Real planet art (Kenney "Planets" pack, CC0 — see
/// assets/sprites/celestial/CREDITS.txt). The candidate list per type lives
/// on `PlanetType::sprites`, matched to what each image actually depicts.
fn planet_sprite_path(planet_type: PlanetType, rng: &mut impl Rng) -> String {
    let variants = planet_type.sprites();
    let idx = variants[rng.gen_range(0..variants.len())];
    format!("sprites/celestial/planets/planet{:02}.png", idx)
}

/// Clear space between one planet's surface and the next planet's.
///
/// The minimum is not a feel number: the asteroid field is a disc of radius
/// `FIELD_SPREAD` (30,000) that has to sit in one of these gaps without
/// reaching the bodies on either side, so the narrowest gap must exceed
/// twice that. 80,000 leaves 20,000 of margin.
pub(crate) const ORBIT_GAP_MIN: f32 = 80_000.0;
const ORBIT_GAP_MAX: f32 = 140_000.0;

/// Advances the orbit frontier past one more planet.
///
/// `frontier` is the outer edge of everything placed so far (starting at the
/// star's own surface), `prev_radius` the last planet's radius, and the
/// result is the new planet's orbit distance — which also becomes the next
/// frontier. Clearing both neighbours' radii plus a gap is what keeps solid
/// bodies from intersecting at any size.
fn next_orbit(frontier: f32, prev_radius: f32, radius: f32, gap: f32) -> f32 {
    frontier + prev_radius + radius + gap
}

/// Seconds for one full orbit at `distance` from the star.
///
/// Kepler's third law, `T ∝ a^1.5`, so inner worlds go round faster than
/// outer ones the way they should. The old period was a flat 60-300s
/// regardless of distance, which at today's orbits put a planet 300,000 out
/// on a 60s orbit moving at about 31,000 u/s — twenty-six times faster than
/// a missile. Bodies tens of thousands of units across were whipping past.
///
/// The constant is chosen for orbital SPEED rather than period: about 150
/// u/s at 300,000 out. That is slower than the ship cruises, so you can
/// catch a planet and circle it, while still visibly moving along its
/// dotted path if you watch.
fn orbital_period(distance: f32) -> f32 {
    const KEPLER_K: f32 = 7.65e-5;
    KEPLER_K * distance.max(1.0).powf(1.5)
}

/// Which temperature band the `i`th of `count` planets falls in.
///
/// By position rather than by roll, so a system reads as a system: scorched
/// worlds close to the star, ice and giants at the back. Every system gets at
/// least one hot and one cold slot, since the minimum planet count is two.
fn band_for_orbit(i: usize, count: usize) -> OrbitBand {
    let t = if count <= 1 { 0.0 } else { i as f32 / (count - 1) as f32 };
    if t < 0.34 {
        OrbitBand::Hot
    } else if t < 0.67 {
        OrbitBand::Temperate
    } else {
        OrbitBand::Cold
    }
}

/// Per-planet brightness wobble, so two worlds of one type in one system are
/// not the same image twice. Brightness only -- the sprites carry their own
/// palette and a hue shift turns detailed pixel art into a flat wash.
fn vary(tint: Color, rng: &mut impl Rng) -> Color {
    let k = rng.gen_range(0.88f32..1.08);
    let c = tint.to_srgba();
    Color::srgb(
        (c.red * k).clamp(0.0, 1.0),
        (c.green * k).clamp(0.0, 1.0),
        (c.blue * k).clamp(0.0, 1.0),
    )
}

/// One planet as `roll_star_system` rolls it: everything about it the seed
/// decides, before any entity exists.
pub struct PlanetRoll {
    pub planet_type: PlanetType,
    pub radius: f32,
    pub mass: f32,
    pub orbit_distance: f32,
    pub period: f32,
    pub eccentricity: f32,
    pub phase: f32,
    pub clockwise: bool,
    sprite: String,
    color: Color,
    spin_rate: f32,
    has_atmosphere: bool,
    has_rings: bool,
    resource_richness: f32,
}

impl PlanetRoll {
    /// The tube this planet sweeps round its star.
    pub fn path(&self) -> super::orbits::PlanetPath {
        super::orbits::PlanetPath {
            semi_major_axis: self.orbit_distance,
            eccentricity: self.eccentricity,
            radius: self.radius,
        }
    }
}

/// A star system as its seed decides it, before any entity exists.
pub struct SystemRoll {
    pub star_class: StarSizeClass,
    flare_threshold: f32,
    pub planets: Vec<PlanetRoll>,
}

impl SystemRoll {
    pub fn star_radius(&self) -> f32 {
        self.star_class.radius()
    }

    pub fn planet_paths(&self) -> Vec<super::orbits::PlanetPath> {
        self.planets.iter().map(PlanetRoll::path).collect()
    }
}

/// Every draw `spawn_star_system` makes, in the order it has always made
/// them, with no Bevy world involved.
///
/// Split out so a system's planets can be known without spawning it.
/// Stations are placed by `world::home_base::station_sites`, which runs long
/// before (and long after) the system is loaded -- for the map, docking,
/// contracts and the warp arrival point -- and a station that doesn't know
/// where the planets go gets run over by one: in the seed-42 galaxy 16 of 31
/// systems had a station on a planet's path.
///
/// The ORDER is load-bearing. The spawner's stream carries on into the
/// asteroid field and the points of interest, so one draw moved or retyped
/// here (an f64 where an f32 was, a range changed) reshuffles every system in
/// the galaxy. Cosmetic rolls are kept even though only the spawner uses
/// them, for the same reason.
pub fn roll_star_system(rng: &mut impl Rng) -> SystemRoll {
    // Pick star class based on seed
    let star_class = match rng.gen_range(0..10) {
        0..=3 => StarSizeClass::Dwarf,
        4..=7 => StarSizeClass::Main,
        8 => StarSizeClass::Giant,
        _ => StarSizeClass::Supergiant,
    };
    let flare_threshold = rng.gen_range(0.7..0.95);

    // Generate 2-6 planets
    let planet_count = rng.gen_range(2..=6);
    let mut planets = Vec::with_capacity(planet_count);

    // Walking frontier for orbit placement: the outer edge of what has been
    // placed so far, starting at the star's own surface.
    let mut orbit_frontier = star_class.radius();
    let mut prev_radius = 0.0f32;

    for i in 0..planet_count {
        let band = band_for_orbit(i, planet_count);
        let candidates = PlanetType::for_band(band);
        let planet_type = candidates[rng.gen_range(0..candidates.len())];
        let (r_min, r_max) = planet_type.radius_range();
        let (m_min, m_max) = planet_type.mass_range();

        let planet_radius = rng.gen_range(r_min..r_max);
        let planet_mass = rng.gen_range(m_min..m_max);

        // Orbits are walked outward from the star with real clearance,
        // rather than `star_radius * 2 + (i + 1) * step`. That formula knew
        // nothing about how big the planets actually are, so with the sizes
        // doubled -- a gas giant is now 60,000 in radius against orbit steps
        // of 25,000-45,000 -- neighbouring worlds would simply overlap, and
        // they are solid colliders. Each orbit now clears the previous
        // planet's surface, this planet's own radius, and a gap.
        orbit_frontier = next_orbit(
            orbit_frontier,
            prev_radius,
            planet_radius,
            rng.gen_range(ORBIT_GAP_MIN..ORBIT_GAP_MAX),
        );
        let orbit_distance = orbit_frontier;
        prev_radius = planet_radius;
        let period = orbital_period(orbit_distance) * rng.gen_range(0.88..1.12);
        let eccentricity = rng.gen_range(0.0..0.3);
        let phase = rng.gen_range(0.0..std::f32::consts::TAU);
        let clockwise = rng.gen_bool(0.5);
        let sprite = planet_sprite_path(planet_type, rng);
        let color = vary(planet_type.tint(), rng);
        // One turn every one to three minutes, either way round. Slow
        // enough to read as a world rather than a top, quick enough
        // that a feature visibly crosses the disc while you watch.
        let spin_rate = std::f32::consts::TAU / rng.gen_range(60.0f32..180.0)
            * if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let has_atmosphere = rng.gen_bool(planet_type.atmosphere_chance());
        let has_rings = rng.gen_bool(planet_type.ring_chance());
        let resource_richness = rng.gen_range(0.1..1.0);

        planets.push(PlanetRoll {
            planet_type,
            radius: planet_radius,
            mass: planet_mass,
            orbit_distance,
            period,
            eccentricity,
            phase,
            clockwise,
            sprite,
            color,
            spin_rate,
            has_atmosphere,
            has_rings,
            resource_richness,
        });
    }

    SystemRoll { star_class, flare_threshold, planets }
}

pub fn spawn_star_system(
    commands: &mut Commands,
    asset_server: &AssetServer,
    center: Vec2,
    system_id: u32,
    rng: &mut impl Rng,
    textures: &CelestialTextures,
) -> StarSystemInfo {
    let roll = roll_star_system(rng);
    let star_class = roll.star_class;
    let star_radius = roll.star_radius();
    let star_mass = star_class.mass();

    // Spawn star
    let star_entity = commands.spawn((
        (Sprite {
                image: textures.solid.clone(),
                color: match star_class {
                    StarSizeClass::Dwarf => Color::srgb(1.0, 0.6, 0.3),
                    StarSizeClass::Main => Color::srgb(1.0, 0.95, 0.8),
                    StarSizeClass::Giant => Color::srgb(1.0, 0.8, 0.4),
                    StarSizeClass::Supergiant => Color::srgb(0.7, 0.8, 1.0),
                },
                custom_size: Some(Vec2::splat(star_radius * 2.0)),
                ..default()
            }, Transform::from_xyz(center.x, center.y, -1.0)),
        CelestialBody {
            body_type: CelestialBodyType::Star,
            mass: star_mass,
            radius: star_radius,
            name: super::galaxy::system_name(system_id),
        },
        Star {
            luminosity: star_class.radiation_multiplier(),
            radiation_output: star_class.radiation_multiplier() * 10.0,
            size_class: star_class,
            flare_buildup: 0.0,
            flare_threshold: roll.flare_threshold,
            is_dying: false,
            death_timer: 10.0, // 10 second countdown when dying starts
        },
        GravityWell {
            // Same derivation as planets: inverse-square lands on the chosen
            // surface acceleration when strength is that acceleration times
            // the radius squared.
            strength: star_class.surface_gravity() * star_radius * star_radius,
            // Six radii. Haven's spawn sits under two radii out, where this
            // still pulls at roughly a quarter of surface strength -- the
            // star should be felt from the moment you launch.
            influence_radius: star_radius * 6.0,
            falloff: GravityFalloff::InverseSquare,
        },
        StarSystemMember { system_id },
    )).id();

    let planet_paths = roll.planet_paths();
    let mut planet_entities = Vec::new();

    for (i, planet) in roll.planets.into_iter().enumerate() {
        let planet_type = planet.planet_type;
        let planet_radius = planet.radius;

        // Initial position on orbit
        let initial_x = center.x + planet.orbit_distance * planet.phase.cos();
        let initial_y = center.y + planet.orbit_distance * planet.phase.sin();

        let planet_entity = commands.spawn((
            (Sprite {
                    image: asset_server.load(planet.sprite),
                    color: planet.color,
                    custom_size: Some(Vec2::splat(planet_radius * 2.0)),
                    ..default()
                }, Transform::from_xyz(initial_x, initial_y, -0.9)),
            CelestialBody {
                body_type: CelestialBodyType::Planet,
                mass: planet.mass,
                radius: planet_radius,
                name: format!("{} {}", super::galaxy::system_name(system_id), super::galaxy::roman(i as u32 + 1)),
            },
            PlanetSpin { rate: planet.spin_rate },
            Planet {
                planet_type,
                has_atmosphere: planet.has_atmosphere,
                has_rings: planet.has_rings,
                resource_richness: planet.resource_richness,
            },
            OrbitalPath {
                parent: star_entity,
                semi_major_axis: planet.orbit_distance,
                eccentricity: planet.eccentricity,
                phase: planet.phase,
                period: planet.period,
                clockwise: planet.clockwise,
            },
            GravityWell {
                // Inverse-square gravity is `strength / distance²`, so to get
                // a chosen acceleration AT THE SURFACE the strength has to be
                // that acceleration times the radius squared. Deriving it
                // from mass instead (the old `mass * 100`) ignored the body's
                // size entirely and produced a surface pull of ~0.0009 u/s².
                strength: planet_type.surface_gravity() * planet_radius * planet_radius,
                // Out to five radii rather than three: at three the pull
                // switches off while still an eighth of surface strength,
                // which reads as a wall rather than a well.
                influence_radius: planet_radius * 5.0,
                falloff: GravityFalloff::InverseSquare,
            },
            StarSystemMember { system_id },
        )).id();

        planet_entities.push(planet_entity);
    }

    StarSystemInfo {
        id: system_id,
        star_entity: Some(star_entity),
        planet_entities,
        center,
        is_alive: true,
        star_radius,
        planet_paths,
    }
}

/// Real rock sprites (Kenney "Space Shooter Redux", CC0 — see
/// assets/sprites/celestial/CREDITS.txt). Picked independent of world size —
/// custom_size scales whatever image lands here to the rolled asteroid size,
/// so a "tiny" sprite blown up to a big asteroid's size looks fine.
/// Shape variants per (size class, ore type). See tools/art/pixel/asteroids.py.
const ASTEROID_VARIANTS: usize = 3;

/// Sprite for one rock.
///
/// The art is procedural pixel work (tools/art/pixel/asteroids.py): granular
/// grain-by-grain rock with ore running through it as thin seams. Resolution
/// tracks size class so a PIXEL is ~4 world units at every size -- a big rock
/// is more grains, not bigger grains.
///
/// Ore is baked into the sprite, so what a rock is worth mining for is
/// legible from the rock itself instead of from a flat colour wash.
fn asteroid_sprite(size: f32, resource: ResourceNodeType, variant: usize) -> String {
    let class = if size < 350.0 {
        "small"
    } else if size < 550.0 {
        "medium"
    } else {
        "large"
    };
    let ore = match resource {
        ResourceNodeType::MetalOre => "metal",
        ResourceNodeType::RareCrystal => "crystal",
        ResourceNodeType::FuelDeposit => "fuel",
        ResourceNodeType::ExoticMatter => "exotic",
    };
    format!(
        "sprites/celestial/asteroids/ast_{}_{}_{}.png",
        class, ore, variant % ASTEROID_VARIANTS
    )
}

/// Spawn asteroid field at a position (decorative gravity bodies, each
/// mineable — see MineableResource). Every asteroid the player actually
/// flies past is a resource node; there's no separate invisible "node"
/// system to stumble into, and since this is the one function both the
/// initial system and every warp jump call, mining works everywhere for
/// free instead of only in the system you started in.
/// Pack centres for one field, spread over its area.
///
/// Separated from the spawner so the layout can be tested without a Bevy
/// world — `spawn_asteroid_field` needs Commands and an AssetServer, which
/// is why nothing ever checked the old field's density.
fn pack_centers(
    center: Vec2,
    spread: f32,
    keep_clear: Option<(Vec2, f32)>,
    rng: &mut impl Rng,
) -> Vec<Vec2> {
    (0..PACK_COUNT)
        .map(|_| {
            let mut pos = center;
            for _attempt in 0..16 {
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                // sqrt keeps the packs spread evenly over the field's AREA
                // rather than bunched at its middle.
                let dist = spread * rng.gen_range(0.0f32..1.0).sqrt();
                pos = center + Vec2::new(angle.cos() * dist, angle.sin() * dist);
                match keep_clear {
                    // A pack centre this far out keeps the pack's whole
                    // radius outside the bubble, so no individual rock needs
                    // checking afterwards.
                    Some((point, radius)) if pos.distance(point) < radius + PACK_RADIUS => continue,
                    _ => break,
                }
            }
            pos
        })
        .collect()
}

/// One rock's position inside its pack, nudged clear of what is already
/// placed. Rocks are solid (see `ship::collision`), so two fused together is
/// a wall with a resource node inside it.
///
/// Gives up after a dozen tries and returns the last candidate: a pack has
/// finite room, and refusing to place the rock at all would silently thin the
/// field out, which is the fault this clumping exists to fix.
fn place_in_pack(pack: Vec2, radius: f32, placed: &[(Vec2, f32)], rng: &mut impl Rng) -> Vec2 {
    let mut pos = pack;
    for _attempt in 0..12 {
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let dist = PACK_RADIUS * rng.gen_range(0.0f32..1.0).sqrt();
        pos = pack + Vec2::new(angle.cos() * dist, angle.sin() * dist);
        if placed.iter().all(|(p, r)| pos.distance(*p) > (radius + r) * 1.1 + 40.0) {
            break;
        }
    }
    pos
}

/// How many packs a field breaks into.
const PACK_COUNT: usize = 5;

/// One rock's size, edge to edge.
const ROCK_SIZE: std::ops::Range<f32> = 200.0..800.0;

/// How far past its spread a field's rocks can reach: a pack centre lands
/// anywhere inside the spread, its rocks up to a pack radius from that, and
/// a rock's edge up to half the biggest size further.
pub(crate) const FIELD_OVERHANG: f32 = PACK_RADIUS + ROCK_SIZE.end * 0.5;

/// Radius of one pack. Sized against the clearance the overlap nudge needs:
/// two average rocks (size ~500, so radius ~250) must sit at least
/// `(250 + 250) * 1.1 + 40` = 590 units apart, and seven rocks need room for
/// that without the nudge giving up and letting them fuse. 2,200 leaves
/// roughly 1,300 units between neighbours -- about twenty cells, so a couple
/// are on screen at once and the rest are a short burn away.
const PACK_RADIUS: f32 = 2_200.0;

pub fn spawn_asteroid_field(
    commands: &mut Commands,
    asset_server: &AssetServer,
    center: Vec2,
    count: u32,
    spread: f32,
    // Somewhere no rock may be placed, as (point, radius). Rocks are solid, so
    // without this the field -- now aimed at the station the player arrives
    // at -- could put one on top of the ship at the moment it spawns.
    keep_clear: Option<(Vec2, f32)>,
    system_id: u32,
    rng: &mut impl Rng,
    // 1.0 = untouched, scales down toward 0.0 as the system's ambient
    // depletion (StarSystemDef.resource_fraction_remaining, see
    // celestial::galaxy::catch_up_system) progresses — applied post-roll so
    // it doesn't disturb the deterministic RNG sequence (same seed still
    // rolls the same asteroid count/size/type/position every time).
    depletion_mult: f32,
) {
    // Clumps, not a sprinkle. At the old twenty-over-30,000 density the
    // nearest rock averaged 104 grid cells away, while the viewport at
    // default zoom is about 35 cells across -- so you met one rock at a time
    // and a "field" never read as one. Rocks are drawn around a handful of
    // pack centres instead: 21 cells to the nearest neighbour, which puts
    // several on screen together and leaves real emptiness between packs.
    let packs = pack_centers(center, spread, keep_clear, rng);

    // Rocks are solid now (see ship::collision) — nudge overlapping rolls
    // apart so a field doesn't generate asteroids fused into each other.
    // Still deterministic: same seed, same draw sequence, same layout.
    let mut placed: Vec<(Vec2, f32)> = Vec::new();
    for i in 0..count {
        let size = rng.gen_range(ROCK_SIZE);
        let mass = size * 0.5;
        let radius = size * 0.5;

        // Round-robin rather than a random pick per rock, so every pack is
        // actually populated -- a random draw leaves some packs empty and
        // others doubled, which is the sparseness this is meant to fix.
        let pack = packs[(i as usize) % packs.len()];
        let pos = place_in_pack(pack, radius, &placed, rng);
        placed.push((pos, radius));

        let resource_type = match rng.gen_range(0..4) {
            0 => ResourceNodeType::MetalOre,
            1 => ResourceNodeType::RareCrystal,
            2 => ResourceNodeType::FuelDeposit,
            _ => ResourceNodeType::ExoticMatter,
        };
        // Gameplay rolls first, cosmetics last, so art changes can never
        // disturb what a system actually contains.
        let resource_amount = size * rng.gen_range(0.5..1.2);

        // ONE fixed-width draw for everything cosmetic. gen::<u32>() consumes
        // exactly one word; gen_range uses rejection sampling, so its
        // consumption depends on the range -- which would mean that changing
        // the variant count later silently reshuffled every existing system's
        // asteroid layout. Deriving all the jitter from the bits of a single
        // word makes the art independent of the gameplay stream for good.
        let art: u32 = rng.gen();
        let variant = (art % ASTEROID_VARIANTS as u32) as usize;
        let flip_x = (art >> 8) & 1 == 1;
        // The old flat per-resource wash is gone: the sprite carries its own
        // ore seams now, and a hue tint multiplied over pixel art just muds
        // the ore ladder. A neutral brightness wobble is all that is left, so
        // a field doesn't read as one rock stamped twenty times.
        let shade = 0.88 + ((art >> 16) & 0xff) as f32 / 255.0 * 0.12;
        let color = Color::srgb(shade, shade, shade);
        let sprite_path = asteroid_sprite(size, resource_type, variant);

        commands.spawn((
            (Sprite {
                    // NEAREST, not the project default. Sprites are sampled
                    // linear globally (main.rs never sets
                    // ImagePlugin::default_nearest), and these textures are
                    // blown up ~4x from texel to world size -- bilinear would
                    // smear the grain into mush and throw away the entire
                    // point of the pixel art. Set per-asset rather than
                    // globally so the ~100 smooth module/weapon renders keep
                    // their filtering.
                    image: asset_server
                        .load_builder()
                        .with_settings(|s: &mut bevy::image::ImageLoaderSettings| {
                            s.sampler = bevy::image::ImageSampler::nearest();
                        })
                        .load(sprite_path),
                    color,
                    custom_size: Some(Vec2::splat(size)),
                    // Free second silhouette per variant, and safe here: the
                    // baked light is straight-down, so a mirror leaves the
                    // lighting consistent across the field.
                    flip_x,
                    ..default()
                }, Transform::from_xyz(pos.x, pos.y, -0.5)),
            CelestialBody {
                body_type: CelestialBodyType::Asteroid,
                mass,
                radius: size * 0.5,
                name: "Asteroid".into(),
            },
            SpacePoi {
                poi_type: SpacePoiType::AsteroidNode,
                looted: false,
                discovered: false,
                name: "Asteroid".into(),
                loot_value: 0,
            },
            MineableResource {
                // Bigger rock = more to mine (size ranges 200-800).
                resource_remaining: resource_amount * depletion_mult,
                resource_type,
                extraction_rate: 5.0,
            },
            StarSystemMember { system_id },
        ));
    }
}

#[cfg(test)]
mod asteroid_art_tests {
    use super::*;

    /// Every (size class x ore type x variant) combination must resolve to a
    /// file that actually exists.
    ///
    /// `asteroid_sprite` builds paths with `format!` rather than indexing a
    /// const table, so a typo or a missing render fails silently in game as an
    /// invisible asteroid -- and 36 files is far too many to check by eye.
    #[test]
    fn every_asteroid_sprite_exists() {
        let sizes = [250.0_f32, 450.0, 700.0];
        let ores = [
            ResourceNodeType::MetalOre,
            ResourceNodeType::RareCrystal,
            ResourceNodeType::FuelDeposit,
            ResourceNodeType::ExoticMatter,
        ];
        let mut checked = 0;
        for size in sizes {
            for ore in ores {
                for variant in 0..ASTEROID_VARIANTS {
                    let rel = asteroid_sprite(size, ore, variant);
                    let path = std::path::Path::new("assets").join(&rel);
                    assert!(path.exists(), "missing asteroid sprite: {}", rel);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, sizes.len() * ores.len() * ASTEROID_VARIANTS);
    }
}

#[cfg(test)]
mod field_density_tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    /// Reproduces the spawner's layout loop exactly, minus the Bevy entities.
    /// Returns every rock as `(position, radius)`.
    fn layout_clear(
        count: u32,
        spread: f32,
        keep_clear: Option<(Vec2, f32)>,
        seed: u64,
    ) -> Vec<(Vec2, f32)> {
        let mut rng = StdRng::seed_from_u64(seed);
        let packs = pack_centers(Vec2::ZERO, spread, keep_clear, &mut rng);
        let mut placed: Vec<(Vec2, f32)> = Vec::new();
        for i in 0..count {
            let size = rng.gen_range(ROCK_SIZE);
            let radius = size * 0.5;
            let pack = packs[(i as usize) % packs.len()];
            let pos = place_in_pack(pack, radius, &placed, &mut rng);
            placed.push((pos, radius));
        }
        placed
    }

    fn layout(count: u32, spread: f32, seed: u64) -> Vec<(Vec2, f32)> {
        layout_clear(count, spread, None, seed)
    }

    /// The field is aimed at the station the player arrives at, so without a
    /// keep-clear bubble a solid rock can spawn on top of the ship at the
    /// moment it appears. Checked at the pack level because a pack centre
    /// outside the bubble by its own radius keeps every rock in it outside.
    #[test]
    fn no_rock_lands_on_the_station() {
        let station = Vec2::new(8_000.0, -3_000.0);
        let clear = 12_000.0;
        for seed in [1u64, 42, 5_000, 123_456, 777] {
            let rocks = layout_clear(30, 30_000.0, Some((station, clear)), seed);
            for (pos, radius) in &rocks {
                let gap = pos.distance(station) - radius;
                assert!(
                    gap >= clear - PACK_RADIUS,
                    "seed {seed}: a rock sits {gap:.0} from the station, inside the \
                     {clear:.0} bubble the ship spawns in"
                );
            }
        }
    }

    /// Rocks are solid, so two overlapping ones are a wall with a resource
    /// node buried in it. Clumping makes this much easier to get wrong than
    /// scattering did, which is the reason this test exists.
    #[test]
    fn rocks_in_a_pack_do_not_fuse_together() {
        for seed in [1u64, 42, 5_000, 123_456] {
            let rocks = layout(30, 30_000.0, seed);
            for (i, (pos_a, r_a)) in rocks.iter().enumerate() {
                for (pos_b, r_b) in rocks.iter().skip(i + 1) {
                    let gap = pos_a.distance(*pos_b) - (r_a + r_b);
                    assert!(
                        gap > 0.0,
                        "seed {seed}: two rocks overlap by {} units", -gap
                    );
                }
            }
        }
    }

    /// The actual complaint: one rock every few hundred cells. Measured as
    /// mean distance to nearest neighbour, in grid cells (66 units each).
    ///
    /// Both numbers measured by running this same metric over both layouts,
    /// not estimated: the old flat scatter of 20 rocks over a 30,000 disc
    /// averages 104 cells to the nearest rock, and packs bring that to 21.
    /// The viewport at default zoom is about 35 cells across, so the old
    /// field could only ever be one rock at a time and this one cannot.
    #[test]
    fn neighbours_are_close_enough_to_share_a_screen() {
        const CELL: f32 = 66.0;
        for seed in [1u64, 42, 5_000, 123_456] {
            let rocks = layout(30, 30_000.0, seed);
            let mut sum = 0.0;
            for (i, (pos_a, _)) in rocks.iter().enumerate() {
                let mut nearest = f32::MAX;
                for (j, (pos_b, _)) in rocks.iter().enumerate() {
                    if i != j {
                        nearest = nearest.min(pos_a.distance(*pos_b));
                    }
                }
                sum += nearest;
            }
            let mean_cells = (sum / rocks.len() as f32) / CELL;
            assert!(
                mean_cells < 40.0,
                "seed {seed}: nearest rock averages {mean_cells:.0} cells away -- \
                 still a sprinkle rather than packs"
            );
        }
    }

    /// Packs must stay packs: rocks close to their own neighbours, with real
    /// emptiness between groups. If this ratio climbs, the clumping has
    /// decayed back into an even spread.
    #[test]
    fn the_field_is_clumped_and_not_merely_denser() {
        for seed in [1u64, 42, 5_000, 123_456] {
            let rocks = layout(30, 30_000.0, seed);
            let mut nearest_sum = 0.0;
            let mut all_sum = 0.0;
            let mut all_count = 0.0;
            for (i, (pos_a, _)) in rocks.iter().enumerate() {
                let mut nearest = f32::MAX;
                for (j, (pos_b, _)) in rocks.iter().enumerate() {
                    if i == j { continue; }
                    let d = pos_a.distance(*pos_b);
                    nearest = nearest.min(d);
                    all_sum += d;
                    all_count += 1.0;
                }
                nearest_sum += nearest;
            }
            let ratio = (nearest_sum / rocks.len() as f32) / (all_sum / all_count);
            assert!(
                ratio < 0.12,
                "seed {seed}: nearest/mean ratio {ratio:.3} -- rocks are spread evenly \
                 rather than gathered into packs"
            );
        }
    }
}


#[cfg(test)]
mod planet_variety_tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    const ALL: [PlanetType; 12] = [
        PlanetType::Lava, PlanetType::Volcanic, PlanetType::Desert,
        PlanetType::Rocky, PlanetType::Barren, PlanetType::Ocean,
        PlanetType::Terran, PlanetType::Toxic, PlanetType::Ice,
        PlanetType::Gas, PlanetType::IceGiant, PlanetType::Shattered,
    ];

    const BANDS: [OrbitBand; 3] = [OrbitBand::Hot, OrbitBand::Temperate, OrbitBand::Cold];

    /// Every sprite a type can ask for has to exist. An index with no file
    /// behind it is an invisible planet at runtime and nothing else: the load
    /// fails quietly and the sprite never appears.
    #[test]
    fn every_planet_sprite_exists_on_disk() {
        for kind in ALL {
            let variants = kind.sprites();
            assert!(!variants.is_empty(), "{kind:?} has no sprite to draw");
            for &idx in variants {
                let path = format!("assets/sprites/celestial/planets/planet{idx:02}.png");
                assert!(
                    std::path::Path::new(&path).exists(),
                    "{kind:?} wants {path}, which is not in the asset folder"
                );
            }
        }
    }

    /// Every type must be reachable from some band. `Shattered` spent the
    /// project so far defined but never constructed -- the old spawner picked
    /// from a hardcoded four-type array -- so it existed in the data model and
    /// could not occur in a game.
    #[test]
    fn every_planet_type_can_actually_occur() {
        for kind in ALL {
            let found = BANDS.iter().any(|b| PlanetType::for_band(*b).contains(&kind));
            assert!(found, "{kind:?} is in no band, so it can never spawn");
        }
    }

    /// A system should read as a system: scorched close in, cold at the back.
    #[test]
    fn the_innermost_is_hot_and_the_outermost_is_cold() {
        for count in 2..=6 {
            assert_eq!(band_for_orbit(0, count), OrbitBand::Hot, "{count} planets: innermost");
            assert_eq!(
                band_for_orbit(count - 1, count),
                OrbitBand::Cold,
                "{count} planets: outermost"
            );
        }
    }

    /// Hot bands must not offer ice, cold bands must not offer lava. This is
    /// the whole point of banding, and it is easy to break by adding a type to
    /// the wrong list.
    #[test]
    fn bands_do_not_offer_contradictory_worlds() {
        let hot = PlanetType::for_band(OrbitBand::Hot);
        for banned in [PlanetType::Ice, PlanetType::IceGiant, PlanetType::Ocean, PlanetType::Gas] {
            assert!(!hot.contains(&banned), "{banned:?} offered next to the star");
        }
        let cold = PlanetType::for_band(OrbitBand::Cold);
        for banned in [PlanetType::Lava, PlanetType::Volcanic, PlanetType::Desert] {
            assert!(!cold.contains(&banned), "{banned:?} offered out in the cold");
        }
    }

    /// Giants have to be giants, or the size ladder carries no information.
    #[test]
    fn giants_outsize_every_rocky_world() {
        let smallest_giant = PlanetType::IceGiant.radius_range().0;
        for kind in ALL {
            if matches!(kind, PlanetType::Gas | PlanetType::IceGiant) {
                continue;
            }
            assert!(
                kind.radius_range().1 <= smallest_giant,
                "{kind:?} can reach {}, overlapping the giants at {smallest_giant}",
                kind.radius_range().1
            );
        }
    }

    #[test]
    fn every_type_has_a_sane_size_and_mass_range() {
        for kind in ALL {
            let (r_lo, r_hi) = kind.radius_range();
            let (m_lo, m_hi) = kind.mass_range();
            assert!(r_lo > 0.0 && r_lo < r_hi, "{kind:?}: radius range {r_lo}..{r_hi}");
            assert!(m_lo > 0.0 && m_lo < m_hi, "{kind:?}: mass range {m_lo}..{m_hi}");
            assert!(
                (0.0..=1.0).contains(&kind.atmosphere_chance()),
                "{kind:?}: atmosphere chance out of range"
            );
            assert!(
                (0.0..=1.0).contains(&kind.ring_chance()),
                "{kind:?}: ring chance out of range"
            );
        }
    }

    /// The tint is for telling two worlds of one type apart, not recolouring
    /// them. If the wobble ever drives a channel far from the art's own
    /// palette, the pixel work turns into a flat wash.
    #[test]
    fn the_tint_wobble_stays_subtle() {
        let mut rng = StdRng::seed_from_u64(9);
        for kind in ALL {
            for _ in 0..200 {
                let c = vary(kind.tint(), &mut rng).to_srgba();
                for (name, v) in [("r", c.red), ("g", c.green), ("b", c.blue)] {
                    assert!(
                        (0.65..=1.0).contains(&v),
                        "{kind:?}: {name} channel at {v} -- that is a recolour, not a wobble"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod planet_placement_tests {
    use super::*;
    use crate::celestial::components::StarSizeClass;

    const ALL: [PlanetType; 12] = [
        PlanetType::Lava, PlanetType::Volcanic, PlanetType::Desert,
        PlanetType::Rocky, PlanetType::Barren, PlanetType::Ocean,
        PlanetType::Terran, PlanetType::Toxic, PlanetType::Ice,
        PlanetType::Gas, PlanetType::IceGiant, PlanetType::Shattered,
    ];

    /// Runs the spawner's orbit walk over a worst case: the largest body of
    /// every type, back to back, at the tightest legal gap.
    fn walk(star_radius: f32, radii: &[f32], gap: f32) -> Vec<f32> {
        let mut frontier = star_radius;
        let mut prev = 0.0;
        radii
            .iter()
            .map(|&r| {
                frontier = next_orbit(frontier, prev, r, gap);
                prev = r;
                frontier
            })
            .collect()
    }

    /// Planets are solid colliders, so two of them sharing space is a wall
    /// you can be crushed inside. The old placement was
    /// `star_radius * 2 + (i + 1) * step` with step 25,000-45,000 and knew
    /// nothing of planet size -- with radii doubled, a 60,000-radius gas
    /// giant and its neighbour would overlap by tens of thousands of units.
    #[test]
    fn no_two_planets_ever_intersect() {
        let radii: Vec<f32> = ALL.iter().map(|t| t.radius_range().1).collect();
        for class in [StarSizeClass::Dwarf, StarSizeClass::Main, StarSizeClass::Giant, StarSizeClass::Supergiant] {
            for gap in [ORBIT_GAP_MIN, ORBIT_GAP_MAX] {
                let orbits = walk(class.radius(), &radii, gap);
                for i in 1..orbits.len() {
                    let inner_edge = orbits[i] - radii[i];
                    let outer_edge = orbits[i - 1] + radii[i - 1];
                    assert!(
                        inner_edge > outer_edge,
                        "{class:?}: planet {i} starts at {inner_edge}, inside planet {} \
                         which ends at {outer_edge}",
                        i - 1
                    );
                }
            }
        }
    }

    /// And none of them is inside the star.
    #[test]
    fn no_planet_is_inside_its_star() {
        let radii: Vec<f32> = ALL.iter().map(|t| t.radius_range().1).collect();
        for class in [StarSizeClass::Dwarf, StarSizeClass::Main, StarSizeClass::Giant, StarSizeClass::Supergiant] {
            let star_radius = class.radius();
            for (i, orbit) in walk(star_radius, &radii, ORBIT_GAP_MIN).iter().enumerate() {
                assert!(
                    orbit - radii[i] > star_radius,
                    "{class:?}: planet {i} reaches {} from the centre, inside a star of {star_radius}",
                    orbit - radii[i]
                );
            }
        }
    }

    /// Gravity you can feel. Engines manage about 180 u/s² (see
    /// `ship::movement::THRUST_SCALE`), so a surface pull in the tens is a
    /// real tug you can still climb out of. Under the old
    /// `strength = mass * 100` the same figure was about 0.0009 -- present in
    /// the data model and completely imperceptible.
    #[test]
    fn planets_pull_hard_enough_to_notice() {
        for kind in ALL {
            let radius = kind.radius_range().0;
            let strength = kind.surface_gravity() * radius * radius;
            // What gravity::accumulate_gravity computes at the surface.
            let surface_accel = strength / (radius * radius);
            assert!(
                surface_accel >= 8.0,
                "{kind:?} pulls at {surface_accel} u/s² at its own surface -- not felt"
            );
            assert!(
                surface_accel <= 120.0,
                "{kind:?} pulls at {surface_accel} u/s², near the ~180 the engines make -- \
                 that is a trap, not a planet"
            );
        }
    }

    /// Bigger worlds pull harder, or size carries no information.
    #[test]
    fn the_giants_pull_hardest() {
        let giant = PlanetType::Gas.surface_gravity();
        for kind in ALL {
            if matches!(kind, PlanetType::Gas) { continue; }
            assert!(
                kind.surface_gravity() < giant,
                "{kind:?} pulls as hard as a gas giant"
            );
        }
        assert!(PlanetType::Shattered.surface_gravity() < PlanetType::Rocky.surface_gravity());
    }
}

#[cfg(test)]
mod motion_and_pull_tests {
    use super::*;
    use crate::celestial::components::StarSizeClass;

    const CLASSES: [StarSizeClass; 4] = [
        StarSizeClass::Dwarf,
        StarSizeClass::Main,
        StarSizeClass::Giant,
        StarSizeClass::Supergiant,
    ];
    const ALL_PLANETS: [PlanetType; 12] = [
        PlanetType::Lava, PlanetType::Volcanic, PlanetType::Desert,
        PlanetType::Rocky, PlanetType::Barren, PlanetType::Ocean,
        PlanetType::Terran, PlanetType::Toxic, PlanetType::Ice,
        PlanetType::Gas, PlanetType::IceGiant, PlanetType::Shattered,
    ];
    /// The starter ship's acceleration, per `ship::movement::THRUST_SCALE`.
    const STARTER_THRUST: f32 = 180.0;

    /// Pull at `distance` from a body whose surface pull is `surface` at
    /// `radius` -- what `gravity::accumulate_gravity` computes for an
    /// inverse-square well built as `surface * radius²`.
    fn pull(surface: f32, radius: f32, distance: f32) -> f32 {
        surface * radius * radius / (distance * distance)
    }

    /// The report: flying right past the star, barely any pull. The old well
    /// was `mass * 500`, about 0.0016 u/s² at a dwarf's surface.
    #[test]
    fn a_star_is_the_deepest_well_in_its_system() {
        let strongest_planet = ALL_PLANETS
            .iter()
            .map(|p| p.surface_gravity())
            .fold(0.0f32, f32::max);
        for class in CLASSES {
            assert!(
                class.surface_gravity() > strongest_planet,
                "{class:?} pulls at {}, no harder than a planet at {strongest_planet}",
                class.surface_gravity()
            );
        }
    }

    /// A fight to climb out of, never a trap.
    #[test]
    fn every_star_can_be_escaped() {
        for class in CLASSES {
            assert!(
                class.surface_gravity() < STARTER_THRUST,
                "{class:?} pulls at {} at its surface, at or beyond the starter's \
                 {STARTER_THRUST} -- a ship skimming it could never climb out",
                class.surface_gravity()
            );
        }
    }

    /// Felt from the moment you launch. Haven's star is a dwarf of radius
    /// 40,000 and the spawn is 75,000 from it (see galaxy::HAVEN_LOCAL_CENTER).
    #[test]
    fn havens_star_is_felt_at_the_spawn() {
        let class = StarSizeClass::Dwarf;
        let r = class.radius();
        let at_spawn = pull(class.surface_gravity(), r, 75_000.0);
        assert!(
            at_spawn >= 20.0,
            "Haven's star pulls at {at_spawn} u/s² at the spawn -- not something you notice"
        );
        assert!(
            75_000.0 < r * 6.0,
            "the spawn is outside the star's influence radius, so it pulls at nothing"
        );
    }

    /// Kepler: inner worlds go round faster. A flat period range ignored
    /// distance entirely, so the outermost planet circled as fast as the
    /// innermost and covered several times the ground doing it.
    #[test]
    fn inner_planets_orbit_faster() {
        assert!(orbital_period(150_000.0) < orbital_period(300_000.0));
        assert!(orbital_period(300_000.0) < orbital_period(900_000.0));
    }

    /// The report: planets go way too fast. The old 60-300s period at today's
    /// orbits put a planet 300,000 out on a 60s orbit at about 31,000 u/s --
    /// twenty-six times faster than a missile. Orbital speed has to sit well
    /// below the ship's, so a planet can be caught and circled.
    #[test]
    fn planets_drift_rather_than_whip_past() {
        const MISSILE_TOP_SPEED: f32 = 1_200.0;
        for distance in [150_000.0f32, 300_000.0, 600_000.0, 1_200_000.0] {
            let speed = std::f32::consts::TAU * distance / orbital_period(distance);
            assert!(
                speed < MISSILE_TOP_SPEED * 0.25,
                "a planet {distance} out orbits at {speed} u/s -- that is whipping past"
            );
            assert!(speed > 20.0, "a planet {distance} out orbits at only {speed} u/s -- frozen");
        }
        // And the figure the old code produced, so the comparison is honest.
        let old = std::f32::consts::TAU * 300_000.0 / 60.0;
        assert!(old > 30_000.0, "the old fastest orbit was {old}, not ~31,000");
    }
}
