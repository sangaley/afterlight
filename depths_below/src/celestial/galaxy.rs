use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[cfg(test)]
use crate::ai_ship::components::faction_power;
use super::components::StarSystemMember;
use super::resources::{GalaxyMap, StarSystemDef, StarSystemInfo, SystemDiscovery, SystemStreamingManager};

/// How many nearest-neighbor systems stay Warm (real ticking background
/// simulation) around the currently-loaded (Hot) system. Small and fixed
/// regardless of galaxy size — see the plan's coordinate-model writeup.
pub const WARM_NEIGHBOR_COUNT: usize = 3;

/// Passive discovery range: any Unknown system within this distance of
/// wherever you're currently Hot quietly reveals as Located (a dim pip, no
/// details) — no scanning action needed, just proximity, Cosmoteer-style.
pub const SENSOR_RANGE: f32 = 2_000_000.0;

/// Active scan range: pressing the radar ping (Z) with an active detection
/// module sweeps interstellar space and reveals any Unknown system within this
/// distance as Located — much longer reach than passive proximity, so a scan
/// actually surfaces systems you couldn't see just by sitting still. Half the
/// galaxy radius and a bit: a scan lights up a big neighbourhood, with the
/// systems this far apart, but never the whole map.
pub const GALAXY_SCAN_RANGE: f32 = 3_200_000.0;

/// How close a blind-warp target has to land to an actual system's
/// galaxy_pos to snap to arriving AT that system (revealing it fully)
/// instead of landing in genuinely empty space nearby. Forgiving on
/// purpose — the map's pips are small and clicking exactly on one of the
/// UNDISCOVERED ones (which aren't even rendered) would otherwise be
/// next to impossible.
pub const SNAP_TOLERANCE: f32 = 150_000.0;

/// Ambient passive depletion rate applied by catch_up_system, expressed as
/// fraction-per-second. ~33 minutes of real elapsed time (Cold or Warm, it
/// doesn't matter) fully exhausts an untouched system's resources — real,
/// permanent, no floor, per the plan's danger-model writeup. Tune by feel.
const AMBIENT_DEPLETION_PER_SECOND: f32 = 0.0005;

/// Non-Haven system count for this pass. 20-40 was the target range;
/// nothing below scales with this as a hardcoded assumption (it's just the
/// loop bound for generation), so raising it later to scale toward
/// "hundreds" doesn't require touching this module's logic.
pub const SYSTEM_COUNT: usize = 30;

/// Haven's local-space center. A constant because world::home_base needs it
/// before the galaxy has been generated: the game opens docked at Haven, a
/// couple of frames before OnEnter(Exploring) rolls the galaxy, and Haven's
/// station has to exist for that whole time.
/// Where Haven's star sits, in the shared local frame.
///
/// 75,000 units from `world::home_base::STATION_POS`, the fixed spot the ship
/// spawns beside. The old value was (200,000, -450,000) -- 492,000 from the
/// station -- so the player began half a million units OUTSIDE their own
/// solar system.
///
/// The window is narrow and both walls are solid. Haven's star is a dwarf of
/// radius 40,000 (seed 42), and the nearest a planet's inner EDGE can come
/// is `star_radius + ORBIT_GAP_MIN` = 120,000, whatever its size. So the
/// spawn has to sit inside (40,000, 120,000); 75,000 is near the middle of
/// it. An earlier pass had this at 95,000, which was fine until planets
/// doubled in size -- then the nearest possible edge moved inward to 85,000
/// and the station was sitting ON an orbit a solid body sweeps along.
///
/// This is the one hand-placed system in the galaxy, which is appropriate --
/// it is home, and the tutorial runs here.
pub const HAVEN_LOCAL_CENTER: Vec2 = Vec2::new(31_000.0, -68_400.0);

/// Haven's seed, fixed like its centre, so the home system is the same in
/// every galaxy -- and its stations, which need its planets, can be placed
/// before the galaxy is rolled.
pub const HAVEN_SEED: u64 = 42;

/// Haven's entry in the galaxy, the one system not rolled.
pub fn haven_def() -> StarSystemDef {
    StarSystemDef {
        id: 0,
        name: "Haven".to_string(),
        galaxy_pos: Vec2::ZERO,
        // Matches today's spawn_initial_system center exactly.
        local_center: HAVEN_LOCAL_CENTER,
        seed: HAVEN_SEED,
        faction: None,
        capital: false,
        haunt: None,
        danger_tier: 0.0,
        discovery: SystemDiscovery::Visited,
        last_updated: 0.0,
        resource_fraction_remaining: 1.0,
        dark: 0.0,
    }
}

/// Names for every system but Haven, dealt in id order. Generated systems
/// used to be called "System-07", and the HUD, the warp notice and the solar
/// flare warning all printed that (or "Star-0") to the player.
const SYSTEM_NAMES: [&str; 40] = [
    "Vesper", "Calder", "Morrow", "Ashfall", "Tamsin", "Kestrel", "Halcyon", "Brine",
    "Sable", "Corvane", "Ilex", "Thule", "Merrow", "Cinder", "Lacuna", "Severn",
    "Nadir", "Ostra", "Gallow", "Riven", "Tarn", "Ember", "Hollow", "Quillon",
    "Ardent", "Mire", "Solace", "Vigil", "Anselm", "Durance", "Fenwick", "Marrow",
    "Pyre", "Rook", "Umber", "Caddis", "Lorn", "Wake", "Tessaly", "Orison",
];

/// The player-facing name of a star system, derived from its id alone so
/// anything holding an id can name it without the galaxy. Its star carries
/// the same name, and its planets are numbered after it ("Calder II").
pub fn system_name(id: u32) -> String {
    if id == 0 {
        return "Haven".to_string();
    }
    let i = (id - 1) as usize;
    let base = SYSTEM_NAMES[i % SYSTEM_NAMES.len()];
    match i / SYSTEM_NAMES.len() {
        0 => base.to_string(),
        lap => format!("{base} {}", roman(lap as u32 + 1)),
    }
}

/// Roman numeral for a small positive number (planet order, repeated names).
pub fn roman(mut n: u32) -> String {
    const TABLE: [(u32, &str); 9] = [
        (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let mut out = String::new();
    for (value, numeral) in TABLE {
        while n >= value {
            out.push_str(numeral);
            n -= value;
        }
    }
    out
}

/// Abstract galaxy-map radius (NOT a real Transform coordinate — see
/// StarSystemDef::galaxy_pos doc comment).
pub const GALAXY_RADIUS: f32 = 5_000_000.0;

/// Closest two systems may sit on the galaxy map. Must stay above twice
/// `SNAP_TOLERANCE` (150,000), or one blind-warp click falls inside two
/// systems at once and which one you arrive at stops being predictable.
/// Was 320,000, which packed a constellation so tightly it looked like a bunch
/// of grapes, not stars with empty space between them, and made every jump
/// to a neighbour cost next to nothing.
const MIN_SYSTEM_SEPARATION: f32 = 1_000_000.0;

/// How many constellations the systems are grouped into. Thirty systems over
/// seven groups averages four or five each — enough to read as a cluster,
/// few enough that the gaps between them are the dominant feature.
const CONSTELLATION_COUNT: usize = 7;

/// Smallest distance between two constellation centres. Not much under twice
/// `CONSTELLATION_SPREAD`: groups still read as groups, but seven of them have
/// to fit in the galaxy.
const CONSTELLATION_SEPARATION: f32 = 2_400_000.0;

/// How far from its constellation's centre a system may sit.
const CONSTELLATION_SPREAD: f32 = 1_400_000.0;

/// Fraction of the galaxy radius the constellation centres are allowed to
/// reach, leaving the rim as genuine emptiness rather than a ring of systems
/// pressed against the edge of the map.
const CONSTELLATION_REACH: f32 = 0.72;
const MAX_PLACEMENT_ATTEMPTS: u32 = 200;

/// Generates the persistent galaxy layout. System 0 is always Haven's home
/// system (fixed at galaxy-space origin, safe, pre-visited). The rest are
/// scattered via rejection sampling with a minimum separation so the map
/// doesn't clump, then sorted by distance from Haven so territory
/// (celestial::territory) can deal capitals weak-near/strong-far for pacing.
pub fn generate_galaxy_map(galaxy_seed: u64) -> GalaxyMap {
    let mut rng = StdRng::seed_from_u64(galaxy_seed);

    // Constellations, not an even scatter. The old layout sampled uniform
    // area density across the whole disc with a flat 400,000 separation,
    // which produces the one thing a starmap must not be: evenly spaced. No
    // region read as anywhere in particular, so there was nothing to
    // recognise and nothing to aim at.
    //
    // Systems are grouped instead. Haven anchors the core, the other
    // constellation centres scatter core-dense, and each system is placed
    // near one of them -- so the map has crowded neighbourhoods with real
    // emptiness between, and warping "to the next constellation over" is a
    // thing you can see before you do it.
    let mut cluster_centers: Vec<Vec2> = vec![Vec2::ZERO]; // Haven anchors the core
    for _ in 1..CONSTELLATION_COUNT {
        let mut candidate = None;
        for _ in 0..MAX_PLACEMENT_ATTEMPTS {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            // `powf(0.9)` rather than the old `sqrt` (0.5, uniform area):
            // weights the draw toward small radii, so the middle of the
            // galaxy is denser than the rim the way a populated core is.
            let r = GALAXY_RADIUS * CONSTELLATION_REACH * rng.gen_range(0.2f32..1.0).powf(0.9);
            let p = Vec2::new(angle.cos() * r, angle.sin() * r);
            if cluster_centers.iter().all(|c| c.distance(p) >= CONSTELLATION_SEPARATION) {
                candidate = Some(p);
                break;
            }
        }
        // A centre that cannot be placed is dropped rather than defaulted.
        // The previous loop defaulted an unplaceable system to Vec2::ZERO,
        // which silently stacked it on top of Haven at the origin.
        if let Some(p) = candidate {
            cluster_centers.push(p);
        }
    }

    // O(n^2) worst case (each candidate checked against every already-placed
    // system) — trivial at n=30, still fine at a few hundred.
    let mut positions: Vec<Vec2> = vec![Vec2::ZERO]; // Haven reserves the origin
    for i in 0..SYSTEM_COUNT {
        let center = cluster_centers[i % cluster_centers.len()];
        let mut candidate = None;
        let mut last = center;
        for _ in 0..MAX_PLACEMENT_ATTEMPTS {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let r = CONSTELLATION_SPREAD * rng.gen_range(0.15f32..1.0).sqrt();
            let p = center + Vec2::new(angle.cos() * r, angle.sin() * r);
            last = p;
            if positions.iter().all(|existing| existing.distance(p) >= MIN_SYSTEM_SEPARATION) {
                candidate = Some(p);
                break;
            }
        }
        // A full constellation: anywhere in the galaxy with room will do,
        // rather than crowding a neighbour.
        if candidate.is_none() {
            for _ in 0..MAX_PLACEMENT_ATTEMPTS {
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let r = GALAXY_RADIUS * 0.97 * rng.gen_range(0.0f32..1.0).sqrt();
                let p = Vec2::new(angle.cos() * r, angle.sin() * r);
                last = p;
                if positions.iter().all(|existing| existing.distance(p) >= MIN_SYSTEM_SEPARATION) {
                    candidate = Some(p);
                    break;
                }
            }
        }
        // Fall back to the last candidate tried, never to the origin.
        positions.push(candidate.unwrap_or(last));
    }

    let mut non_haven: Vec<Vec2> = positions[1..].to_vec();
    non_haven.sort_by(|a, b| a.length().partial_cmp(&b.length()).unwrap());

    let mut systems = Vec::with_capacity(SYSTEM_COUNT + 1);
    systems.push(haven_def());

    // Every system's local_center lives in the SAME shared local coordinate
    // space (only ever one system is physically "Hot" at a time, so this
    // never causes a visual clash) — but the render-distance spawn check
    // that promotes a Warm system's abstract SimulatedShip into a real
    // entity compares raw positions in that same space. Two systems'
    // local_centers landing within render distance of each other by pure
    // chance would let a Warm neighbor's ships erroneously materialize
    // before the player ever actually travels there. Rejection-sample with
    // a wide separation margin (well beyond RENDER_DISTANCE/DESPAWN_DISTANCE,
    // ai_ship/simulation.rs) to rule that out.
    let mut local_centers: Vec<Vec2> = vec![HAVEN_LOCAL_CENTER];
    const LOCAL_MIN_SEPARATION: f32 = 200_000.0;
    const LOCAL_RANGE: f32 = 1_500_000.0;

    for (rank, pos) in non_haven.into_iter().enumerate() {
        let id = (rank + 1) as u32;

        let mut local_center = Vec2::ZERO;
        for _ in 0..MAX_PLACEMENT_ATTEMPTS {
            let candidate = Vec2::new(
                rng.gen_range(-LOCAL_RANGE..LOCAL_RANGE),
                rng.gen_range(-LOCAL_RANGE..0.0),
            );
            if local_centers.iter().all(|existing| existing.distance(candidate) >= LOCAL_MIN_SEPARATION) {
                local_center = candidate;
                break;
            }
        }
        local_centers.push(local_center);

        systems.push(StarSystemDef {
            id,
            name: system_name(id),
            galaxy_pos: pos,
            local_center,
            seed: rng.gen::<u64>(),
            // Dealt by celestial::territory::assign_territory below.
            faction: None,
            capital: false,
            haunt: None,
            danger_tier: 0.0,
            discovery: SystemDiscovery::Unknown,
            last_updated: 0.0,
            resource_fraction_remaining: 1.0,
            dark: 0.0,
        });
    }

    super::territory::assign_territory(&mut systems);
    GalaxyMap { systems, galaxy_seed }
}

/// Generates the galaxy once per session, same guard pattern as today's
/// spawn_initial_system (celestial/mod.rs). Also seeds the streaming
/// manager: Haven (system 0) starts Hot, its nearest neighbors start Warm.
pub fn generate_galaxy_on_enter(
    mut galaxy_map: ResMut<GalaxyMap>,
    mut streaming: ResMut<SystemStreamingManager>,
) {
    if !galaxy_map.systems.is_empty() {
        return;
    }
    // DEPTHS_GALAXY_SEED: the same galaxy every launch -- for filming one
    // place across several runs.
    let seed = std::env::var("DEPTHS_GALAXY_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or_else(rand::random::<u64>);
    *galaxy_map = generate_galaxy_map(seed);
    streaming.loaded_system = Some(0);
    streaming.current_galaxy_pos = Vec2::ZERO;
    streaming.warm_systems = nearest_neighbors(&galaxy_map, 0, WARM_NEIGHBOR_COUNT);
    info!("Galaxy generated: {} systems, seed={}", galaxy_map.systems.len(), galaxy_map.galaxy_seed);
}

/// Bevy system wrapper for passive_proximity_discovery — runs continuously
/// while Exploring, not gated behind any player action.
pub fn passive_proximity_discovery_system(
    mut galaxy_map: ResMut<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
) {
    passive_proximity_discovery(&mut galaxy_map, streaming.current_galaxy_pos);
}

/// Distance a system's faction population cluster sits away from the star
/// itself. Stars run 40k-150k radius (StarSizeClass::radius) but several
/// faction territories are smaller than that (Recursive Kingdom's is only 15k) — if
/// the cluster were centered ON the star like the star's own position, the
/// player (and the ships) would routinely be inside or right on top of the
/// star. Offsetting the cluster clear of any star size decouples the two
/// entirely.
pub const FACTION_CLUSTER_OFFSET: f32 = 220_000.0;

/// Where a system's faction population is actually centered — offset from
/// the star (system.local_center) so it never overlaps it (see
/// FACTION_CLUSTER_OFFSET doc comment). Deterministic per system (derived
/// from its id, not randomized per call) so it's stable every time the
/// system is loaded, and spreads the offset DIRECTION across systems so
/// they don't all put their cluster in the exact same relative spot.
pub fn faction_cluster_center(system: &StarSystemDef) -> Vec2 {
    let angle = (system.id as f32 * 2.399963).rem_euclid(std::f32::consts::TAU);
    system.local_center + Vec2::new(angle.cos(), angle.sin()) * FACTION_CLUSTER_OFFSET
}

/// Nearest `count` other systems to a raw galaxy-space position — the
/// general form behind both `nearest_neighbors` (Warm-tier selection around
/// a loaded system) and blind-warp arrival (nothing to exclude by id).
/// O(n log n) in galaxy size, trivial at n=30, still fine at a few hundred.
pub fn nearest_neighbors_to_pos(galaxy_map: &GalaxyMap, pos: Vec2, exclude_id: Option<u32>, count: usize) -> Vec<u32> {
    let mut others: Vec<(u32, f32)> = galaxy_map.systems.iter()
        .filter(|s| Some(s.id) != exclude_id)
        .map(|s| (s.id, s.galaxy_pos.distance(pos)))
        .collect();
    others.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    others.into_iter().take(count).map(|(id, _)| id).collect()
}

/// Nearest `count` other systems to `system_id` by galaxy_pos distance — the
/// Warm tier (see SystemStreamingManager doc comment).
pub fn nearest_neighbors(galaxy_map: &GalaxyMap, system_id: u32, count: usize) -> Vec<u32> {
    let Some(origin) = galaxy_map.systems.iter().find(|s| s.id == system_id) else {
        return Vec::new();
    };
    nearest_neighbors_to_pos(galaxy_map, origin.galaxy_pos, Some(system_id), count)
}

/// If a blind-warp target lands within SNAP_TOLERANCE of an actual system
/// (Unknown or otherwise), returns its id — the jump snaps to arriving AT
/// that system instead of at the raw empty-space point.
pub fn system_within_snap_tolerance(galaxy_map: &GalaxyMap, pos: Vec2) -> Option<u32> {
    galaxy_map.systems.iter()
        .filter(|s| s.galaxy_pos.distance(pos) <= SNAP_TOLERANCE)
        .min_by(|a, b| a.galaxy_pos.distance(pos).partial_cmp(&b.galaxy_pos.distance(pos)).unwrap())
        .map(|s| s.id)
}

/// Passive proximity discovery: any Unknown system within SENSOR_RANGE of
/// wherever the player currently is (Hot system, or a blind-space position —
/// see SystemStreamingManager.loaded_system) reveals as Located. Runs
/// continuously while Exploring, not gated behind any scan action —
/// Cosmoteer-style ambient fog-of-war peel-back.
pub fn passive_proximity_discovery(galaxy_map: &mut GalaxyMap, current_pos: Vec2) {
    for system in galaxy_map.systems.iter_mut() {
        if system.discovery == SystemDiscovery::Unknown && system.galaxy_pos.distance(current_pos) <= SENSOR_RANGE {
            system.discovery = SystemDiscovery::Located;
        }
    }
}

/// Deterministic local-space arrival point for a blind warp that lands in
/// genuinely empty space (no system within snap tolerance) — doesn't need
/// the same separation guarantees real systems' local_centers do, since
/// nothing else ever spawns there; just needs to vary with where in the
/// galaxy the blind point actually was instead of always landing in one
/// fixed void.
pub fn blind_point_local_center(galaxy_pos: Vec2) -> Vec2 {
    Vec2::new(
        (galaxy_pos.x * 0.15).rem_euclid(1_000_000.0) - 500_000.0,
        -((galaxy_pos.y.abs() * 0.15).rem_euclid(700_000.0)) - 200_000.0,
    )
}

/// Brings a system's ambient resource depletion up to date for however long
/// it's been since it was last checked — the Cold-tier "life happens in the
/// background" math (see the plan's coordinate-model writeup). Real and
/// permanent: no floor keeping it above zero. Called whenever a system
/// needs a genuine answer (loading it, or unloading it to stamp the
/// timestamp for next time), never on a per-frame timer.
pub fn catch_up_system(def: &mut StarSystemDef, now: f64) {
    let elapsed = (now - def.last_updated).max(0.0) as f32;
    def.resource_fraction_remaining = (def.resource_fraction_remaining - elapsed * AMBIENT_DEPLETION_PER_SECOND).max(0.0);
    def.last_updated = now;
}

/// Radius of the asteroid field, unchanged from the original generator.
const FIELD_SPREAD: f32 = 30_000.0;

/// Rocks per field. Up from the original twenty so that the five packs the
/// field breaks into hold six rocks each rather than four -- a pack of four
/// at pack spacing still reads as scattered singles.
const FIELD_ROCKS: u32 = 30;

/// How far from the field's centre a rock's edge can reach.
const FIELD_REACH: f32 = FIELD_SPREAD + super::spawning::FIELD_OVERHANG;

/// Room between the field's outermost rock and a planet's surface at its
/// closest pass: a hull's length, so a ship working the edge of the field is
/// not run down by a world.
const FIELD_PLANET_MARGIN: f32 = 2_000.0;

/// Spacing of the points tried along a bearing when looking for room.
const FIELD_STEP: f32 = 2_000.0;

/// When the station's bearing has no room for the field, it swings round
/// the star five degrees at a time, alternating sides, up to half a turn.
const FIELD_SWING_STEP: f32 = 5.0 * std::f32::consts::PI / 180.0;
const FIELD_SWINGS: usize = 72;

/// How much room to leave around the station a player arrives at. Rocks are
/// solid, and the field is aimed at that station now, so without a bubble one
/// could spawn on top of the ship. Wide enough that the nearest rock is a
/// short burn (~180 cells) rather than a collision.
const STATION_KEEP_CLEAR: f32 = 12_000.0;

/// The star and planets `spawn_system_contents` will spawn for this system,
/// rolled from its seed without spawning anything -- the same stream, so the
/// same worlds (see `spawning::roll_star_system`).
pub fn system_roll(def: &StarSystemDef) -> super::spawning::SystemRoll {
    super::spawning::roll_star_system(&mut StdRng::seed_from_u64(def.seed))
}

/// This system's primary station, which is where the player turns up.
fn station_pos(def: &StarSystemDef) -> Vec2 {
    crate::world::home_base::station_sites(def)
        .first()
        .map(|s| s.pos)
        .unwrap_or(def.local_center)
}

/// Bearing from a system's star to its primary station, so the asteroid
/// field can be put on the side of the star the player arrives on.
///
/// Falls back to due east only if the station resolves exactly onto the star,
/// which `station_sites` never produces.
fn station_field_bearing(star: Vec2, station: Vec2) -> f32 {
    let to_station = station - star;
    if to_station.length_squared() < 1.0 {
        0.0
    } else {
        to_station.to_angle()
    }
}

/// Whether a field centred at `offset` from the star keeps every rock out
/// of the star and clear of every planet's path.
fn field_fits(star_radius: f32, planets: &[super::orbits::PlanetPath], offset: Vec2) -> bool {
    offset.length() - FIELD_REACH >= star_radius
        && planets.iter().all(|p| p.clears(offset, FIELD_REACH + FIELD_PLANET_MARGIN))
}

/// Distance from the star to the middle of the widest stretch along `dir`
/// where the whole field fits between the star and the planets' paths.
///
/// A stretch still open at the outermost planet's furthest reach runs out
/// past every world, and doesn't count: offered alongside the real gaps it
/// tends to win on width -- it is unbounded -- and drags the field out to
/// where nobody flies.
fn widest_gap_along(star_radius: f32, planets: &[super::orbits::PlanetPath], dir: Vec2) -> Option<f32> {
    let outermost = planets.iter().map(|p| p.outer_reach()).fold(star_radius, f32::max);
    let first = star_radius + FIELD_REACH;
    let at = |i: usize| first + i as f32 * FIELD_STEP;
    let mut widest: Option<(usize, usize)> = None;
    let mut open: Option<usize> = None;
    let mut i = 0;
    while at(i) <= outermost {
        if field_fits(star_radius, planets, dir * at(i)) {
            open.get_or_insert(i);
        } else if let Some(from) = open.take() {
            let to = i - 1;
            if widest.is_none_or(|(a, b)| to - from > b - a) {
                widest = Some((from, to));
            }
        }
        i += 1;
    }
    // The middle POINT tried, not the midpoint: every point tried in the
    // stretch is known to fit, and the field goes on one of them.
    widest.map(|(from, to)| at((from + to) / 2))
}

/// Where to put this system's asteroid field, as an offset from the star.
///
/// The original generator hardcoded `(50_000, 0)`, which was simply wrong:
/// stars are solid -- `Collider::circle(radius, INFINITY)` -- and run 40,000
/// in radius for a dwarf up to 150,000 for a supergiant. A 30,000-radius
/// field centred 50,000 out therefore spanned 20,000-80,000 from the star,
/// so for anything Main-class or larger -- about 60% of systems, by the class
/// roll in `spawn_star_system` -- every rock in it spawned inside the star
/// and could never be reached. That is why asteroids appeared not to exist.
///
/// Still one field, not a belt: this keeps the original shape and rock count
/// and only moves it somewhere it can exist. It goes in the middle of the
/// widest gap between the star's surface and the planets' paths, at `angle`
/// around the star, so it clears the star and the worlds either side
/// without being pinned to one side of every system in the galaxy.
///
/// The gaps are measured against each planet's real ellipse. They used to be
/// circular bands of `orbit ± radius`, but at eccentricity up to 0.3 a planet
/// strays up to 30% either side of its orbit's circle -- a gap that is wide
/// on the circles can be closed on the real paths -- and 89 rocks in the
/// seed-42 galaxy sat where a planet passes. All planets' ellipses point the
/// same way, so a bearing pinched shut usually has room a little way round;
/// if `angle` has none the field swings round the star until one does.
fn asteroid_field_offset(star_radius: f32, planets: &[super::orbits::PlanetPath], angle: f32) -> Vec2 {
    for k in 0..=FIELD_SWINGS {
        // 0, +1, -1, +2, -2... steps either side of the station's bearing.
        let side = if k % 2 == 1 { 1.0 } else { -1.0 };
        let dir = Vec2::from_angle(angle + ((k + 1) / 2) as f32 * FIELD_SWING_STEP * side);
        if let Some(distance) = widest_gap_along(star_radius, planets, dir) {
            return dir * distance;
        }
    }
    // No gap anywhere round the star: just outside the outermost path, on
    // the station's side. Always clear, if out of the way.
    let outermost = planets.iter().map(|p| p.outer_reach()).fold(star_radius, f32::max);
    Vec2::from_angle(angle) * (outermost + FIELD_REACH + FIELD_PLANET_MARGIN)
}

/// Spawns a system's full contents (star, planets, asteroids, POIs)
/// deterministically from its seed — one continuous RNG stream shared
/// across all three spawn calls (see spawning.rs's seeding-fix doc
/// comments), scaling asteroid resource amounts by the system's current
/// depletion (catch_up_system should be called first by the caller so this
/// reflects up-to-date depletion, not a stale snapshot).
pub fn spawn_system_contents(
    commands: &mut Commands,
    asset_server: &AssetServer,
    textures: &crate::vfx::procedural_textures::CelestialTextures,
    def: &StarSystemDef,
    // The expedition records this system holds (narrative::trail::records_in).
    records: &[&'static crate::narrative::logs::LogEntryDef],
    // Derelict/anomaly slots already stripped here (LootedPois::for_system).
    looted: &[u32],
) -> StarSystemInfo {
    let mut rng = StdRng::seed_from_u64(def.seed);

    let system_info = super::spawning::spawn_star_system(
        commands, asset_server, def.local_center, def.id, &mut rng, textures,
    );

    // Toward this system's primary station, not a free roll. A field at a
    // random bearing is a field you have to go hunting for: you arrive at a
    // station, and the rocks are as likely to be on the far side of the star
    // as near you. Aiming it at the station you dock at means every system
    // has visible rocks from where you actually turn up, without the field
    // being any denser or larger.
    let station = station_pos(def);
    let field_angle = station_field_bearing(def.local_center, station);
    super::spawning::spawn_asteroid_field(
        commands, asset_server,
        def.local_center + asteroid_field_offset(
            system_info.star_radius, &system_info.planet_paths, field_angle,
        ),
        FIELD_ROCKS, FIELD_SPREAD,
        Some((station, STATION_KEEP_CLEAR)),
        def.id, &mut rng,
        def.resource_fraction_remaining,
    );

    let planet_positions: Vec<Vec2> = system_info.planet_entities.iter()
        .map(|_| def.local_center + Vec2::new(rng.gen_range(-30_000.0..30_000.0), rng.gen_range(-30_000.0..30_000.0)))
        .collect();
    let keep_clear = super::poi::KeepClear {
        center: def.local_center,
        star_radius: system_info.star_radius,
        planets: &system_info.planet_paths,
    };
    super::poi::spawn_system_pois(commands, &keep_clear, def.id, Some(station), &planet_positions, &mut rng, records, looted, crate::narrative::front::is_taken(def));

    system_info
}

/// Despawns every entity tagged for `system_id` and brings its depletion
/// math up to date (stamping last_updated) before it goes Cold.
pub fn unload_system(
    commands: &mut Commands,
    member_query: &Query<(Entity, &StarSystemMember)>,
    galaxy_map: &mut GalaxyMap,
    system_id: u32,
    now: f64,
) {
    for (entity, member) in member_query.iter() {
        if member.system_id == system_id {
            commands.entity(entity).despawn();
        }
    }
    if let Some(def) = galaxy_map.systems.iter_mut().find(|s| s.id == system_id) {
        catch_up_system(def, now);
    }
}

/// Brings a system's depletion up to date, then spawns its contents. The
/// combination `unload_system` (old) + `load_system` (new) is what
/// celestial::warp::execute_warp_jump now does on a completed jump, instead
/// of despawning everything and rolling a brand-new random system.
pub fn load_system(
    commands: &mut Commands,
    asset_server: &AssetServer,
    textures: &crate::vfx::procedural_textures::CelestialTextures,
    galaxy_map: &mut GalaxyMap,
    system_id: u32,
    now: f64,
    looted: &super::resources::LootedPois,
) -> Option<StarSystemInfo> {
    let def = galaxy_map.systems.iter_mut().find(|s| s.id == system_id)?;
    catch_up_system(def, now);
    def.discovery = SystemDiscovery::Visited;
    let def = galaxy_map.systems.iter().find(|s| s.id == system_id)?;
    let records = crate::narrative::trail::records_in(galaxy_map, system_id);
    Some(spawn_system_contents(commands, asset_server, textures, def, &records, looted.for_system(system_id)))
}

#[cfg(test)]
mod galaxy_tests {
    use super::*;
    use crate::narrative::logs::{tier_for_danger, MAX_TIER};

    /// A generated galaxy must reach every band of the log corpus.
    ///
    /// This is the regression guard for the bug the tier system replaced: the
    /// finale entries sat in the table behind a gate nothing could satisfy, so
    /// the game shipped an ending that could not be reached and a victory
    /// check that asked for it. Assert across several seeds, since faction
    /// placement is random within the weak-near/strong-far bias.
    #[test]
    fn every_log_tier_is_reachable_in_a_real_galaxy() {
        for seed in [1u64, 7, 99, 12_345, 987_654_321] {
            let map = generate_galaxy_map(seed);
            let mut seen = [0usize; (MAX_TIER as usize) + 1];
            for sys in &map.systems {
                // The system's own danger: its owner's, or its haunting boss's.
                seen[tier_for_danger(sys.danger_tier) as usize] += 1;
            }
            for tier in 0..=MAX_TIER as usize {
                assert!(
                    seen[tier] > 0,
                    "seed {seed}: no system reaches log tier {tier} — those entries \
                     would be unreachable, which is the bug this replaced. \
                     counts: {seen:?}"
                );
            }
        }
    }

    /// Haven must stay the quiet one. It has no faction, and the opening of
    /// the story depends on nothing being wrong there yet.
    #[test]
    fn haven_is_the_lowest_tier() {
        let map = generate_galaxy_map(42);
        let haven = map.systems.iter().find(|s| s.id == 0).expect("Haven exists");
        assert!(haven.faction.is_none(), "Haven should be unclaimed");
        assert_eq!(tier_for_danger(haven.faction.map(faction_power).unwrap_or(0.0)), 0);
    }
}

#[cfg(test)]
mod asteroid_placement_tests {
    use super::*;
    use crate::celestial::components::StarSizeClass;
    use crate::celestial::orbits::{orbit_radius, PlanetPath};
    use crate::celestial::spawning::ORBIT_GAP_MIN;

    const CLASSES: [StarSizeClass; 4] = [
        StarSizeClass::Dwarf,
        StarSizeClass::Main,
        StarSizeClass::Giant,
        StarSizeClass::Supergiant,
    ];

    /// Planets the way `spawn_star_system` walks them outward: each orbit
    /// clears the previous body's radius, its own, and a gap. Every one at
    /// eccentricity `e`, which the walk knows nothing about.
    fn paths(star_radius: f32, radii: &[f32], gap: f32, e: f32) -> Vec<PlanetPath> {
        let mut frontier = star_radius;
        let mut prev = 0.0;
        radii
            .iter()
            .map(|&r| {
                frontier += prev + r + gap;
                prev = r;
                PlanetPath { semi_major_axis: frontier, eccentricity: e, radius: r }
            })
            .collect()
    }

    /// Closest a planet's surface comes to `offset`, walking its ellipse
    /// densely rather than trusting `PlanetPath::clearance`.
    fn surface_gap(path: &PlanetPath, offset: Vec2) -> f32 {
        (0..4096)
            .map(|k| {
                let theta = k as f32 / 4096.0 * std::f32::consts::TAU;
                Vec2::from_angle(theta) * orbit_radius(path.semi_major_axis, path.eccentricity, theta)
            })
            .map(|p| p.distance(offset))
            .fold(f32::INFINITY, f32::min)
            - path.radius
    }

    /// The original fault: the field's offset was hardcoded to (50,000, 0)
    /// with a 30,000 spread, so every rock sat 20,000-80,000 from the star
    /// while a Main-class star is 80,000 in radius and solid. In about 60% of
    /// systems the whole field spawned inside the sun.
    #[test]
    fn the_field_never_spawns_inside_the_star() {
        for class in CLASSES {
            let star = class.radius();
            for radii in [vec![], vec![20_000.0], vec![60_000.0, 20_000.0, 40_000.0]] {
                for gap in [ORBIT_GAP_MIN, 140_000.0] {
                    for e in [0.0, 0.3] {
                        let p = paths(star, &radii, gap, e);
                        let inner = asteroid_field_offset(star, &p, 0.7).length() - FIELD_REACH;
                        assert!(
                            inner >= star,
                            "{class:?} with {} planets at e={e}: rocks reach {inner}, inside the star",
                            radii.len()
                        );
                    }
                }
            }
        }
    }

    /// Planets are solid too, large, and on ellipses: at e=0.3 a world
    /// strays 30% either side of its orbit's circle, which is what closed
    /// the gaps the old circular bands said were open.
    #[test]
    fn the_field_clears_every_planets_real_path() {
        for class in CLASSES {
            let star = class.radius();
            for radii in [vec![20_000.0], vec![60_000.0, 20_000.0], vec![10_000.0, 60_000.0, 30_000.0]] {
                for gap in [ORBIT_GAP_MIN, 140_000.0] {
                    for e in [0.0, 0.15, 0.3] {
                        for angle in [0.0, 2.1, std::f32::consts::PI, 4.4] {
                            let p = paths(star, &radii, gap, e);
                            let field = asteroid_field_offset(star, &p, angle);
                            for path in &p {
                                let gap_to_rocks = surface_gap(path, field) - FIELD_REACH;
                                assert!(
                                    gap_to_rocks > 0.0,
                                    "{class:?}, e={e}, bearing {angle}: a planet of radius {} \
                                     passes {:.0} inside the field's outermost rock",
                                    path.radius, -gap_to_rocks
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// The field is no longer pinned to the same side of every system in the
    /// galaxy, which is what `(50_000, 0)` did for all 31 of them.
    #[test]
    fn the_field_is_not_always_in_the_same_direction() {
        let star = StarSizeClass::Main.radius();
        let p = paths(star, &[30_000.0, 20_000.0], ORBIT_GAP_MIN, 0.0);
        let a = asteroid_field_offset(star, &p, 0.0);
        let c = asteroid_field_offset(star, &p, 2.4);
        assert!(a.distance(c) > FIELD_SPREAD, "two angles put the field in the same place");
        assert!(
            (a.length() - c.length()).abs() <= FIELD_STEP,
            "on circular orbits the angle changed the distance from the star, not just the direction"
        );
    }

    /// A bearing an eccentric world pinches shut is not given up on: the
    /// field swings round to where there is room, rather than out past every
    /// planet. All the ellipses point the same way (perihelion at angle 0),
    /// so the gaps are narrowest there and widest opposite.
    #[test]
    fn a_pinched_bearing_swings_the_field_rather_than_exiling_it() {
        // Supergiant, one inner world on the tightest gap at e=0.3: at
        // perihelion its surface comes within 5k of the star's, far too
        // little for a field ~65k across.
        let star = StarSizeClass::Supergiant.radius();
        let p = paths(star, &[20_000.0, 20_000.0], ORBIT_GAP_MIN, 0.3);
        let field = asteroid_field_offset(star, &p, 0.0);
        assert!(
            field.length() < p[1].outer_reach(),
            "field sent out to {:.0}, past every planet, when there is room further round",
            field.length()
        );
    }
}

#[cfg(test)]
mod galaxy_layout_tests {
    use super::*;

    /// The galaxy is rolled fresh each game, so room for everyone has to hold
    /// for any seed, not just the handful the other tests look at. And nobody
    /// is stranded: every system has a neighbour within sensible reach, and
    /// Haven's sensors find somewhere to go on the first day.
    #[test]
    fn every_seed_spreads_the_systems_out() {
        let mut nearest_sum = 0.0;
        let mut pairs = 0;
        for seed in 0..300u64 {
            let map = generate_galaxy_map(seed * 7_919 + 3);
            assert_eq!(map.systems.len(), SYSTEM_COUNT + 1);
            for a in &map.systems {
                let nearest = map.systems.iter()
                    .filter(|b| b.id != a.id)
                    .map(|b| a.galaxy_pos.distance(b.galaxy_pos))
                    .fold(f32::MAX, f32::min);
                assert!(nearest >= MIN_SYSTEM_SEPARATION, "seed {seed}: {} is only {nearest} from its neighbour", a.name);
                assert!(nearest <= 2_600_000.0, "seed {seed}: {} is stranded, {nearest} from anyone", a.name);
                assert!(a.galaxy_pos.length() <= GALAXY_RADIUS, "seed {seed}: {} is off the map", a.name);
                nearest_sum += nearest;
                pairs += 1;
            }
            let from_home = map.systems.iter()
                .filter(|s| s.id != 0 && s.galaxy_pos.length() <= SENSOR_RANGE)
                .count();
            assert!(from_home >= 2, "seed {seed}: Haven's sensors find only {from_home} systems");
        }
        println!("average nearest neighbour: {}", nearest_sum / pairs as f32);
    }

    const SEEDS: [u64; 6] = [1, 42, 7_777, 123_456, u64::MAX / 3, 9_999_999];

    /// Two systems closer than twice the snap tolerance means one click on the
    /// map lands inside both, and which one you warp to stops being
    /// predictable. This is a static relationship between two constants, so it
    /// holds for every seed and is the first thing to check after retuning
    /// either.
    #[test]
    fn systems_stay_far_enough_apart_to_click() {
        assert!(
            MIN_SYSTEM_SEPARATION > SNAP_TOLERANCE * 2.0,
            "separation {MIN_SYSTEM_SEPARATION} is within two snap tolerances ({}), \
             so one click can resolve to two systems",
            SNAP_TOLERANCE * 2.0
        );

        for seed in SEEDS {
            let map = generate_galaxy_map(seed);
            for (i, a) in map.systems.iter().enumerate() {
                for b in map.systems.iter().skip(i + 1) {
                    let d = a.galaxy_pos.distance(b.galaxy_pos);
                    assert!(
                        d >= MIN_SYSTEM_SEPARATION,
                        "seed {seed}: systems {} and {} are {d} apart",
                        a.id, b.id
                    );
                }
            }
        }
    }

    /// Only Haven sits at the origin. The previous placer defaulted an
    /// unplaceable system to `Vec2::ZERO`, which stacked it silently on top of
    /// Haven -- invisible on the map, and two systems one click could not tell
    /// apart.
    #[test]
    fn nothing_is_stacked_on_haven() {
        for seed in SEEDS {
            let map = generate_galaxy_map(seed);
            assert_eq!(map.systems[0].galaxy_pos, Vec2::ZERO, "Haven left the origin");
            for sys in map.systems.iter().skip(1) {
                assert_ne!(
                    sys.galaxy_pos, Vec2::ZERO,
                    "seed {seed}: system {} defaulted onto the origin", sys.id
                );
            }
        }
    }

    #[test]
    fn every_system_fits_on_the_map() {
        for seed in SEEDS {
            let map = generate_galaxy_map(seed);
            assert_eq!(map.systems.len(), SYSTEM_COUNT + 1, "seed {seed}: wrong system count");
            for sys in &map.systems {
                assert!(
                    sys.galaxy_pos.length() <= GALAXY_RADIUS,
                    "seed {seed}: system {} is at {}, off the edge of the map",
                    sys.id, sys.galaxy_pos.length()
                );
            }
        }
    }

    /// The point of the change: systems are grouped, not evenly spread.
    ///
    /// Measured rather than asserted by construction -- each system's nearest
    /// neighbour should be much closer than the average system, which is true
    /// of clusters and false of an even scatter.
    ///
    /// Spreading the systems far apart (a request: they looked crammed) made
    /// the groups much looser than they were, 0.114 up to about 0.28. An even
    /// scatter at this separation is what this still guards against: past 0.36
    /// there's no neighbourhood left to recognise.
    #[test]
    fn systems_are_grouped_into_constellations() {
        for seed in SEEDS {
            let map = generate_galaxy_map(seed);
            let pts: Vec<Vec2> = map.systems.iter().map(|s| s.galaxy_pos).collect();

            let mut nearest_sum = 0.0;
            let mut all_sum = 0.0;
            let mut all_count = 0.0;
            for (i, a) in pts.iter().enumerate() {
                let mut nearest = f32::MAX;
                for (j, b) in pts.iter().enumerate() {
                    if i == j { continue; }
                    let d = a.distance(*b);
                    nearest = nearest.min(d);
                    all_sum += d;
                    all_count += 1.0;
                }
                nearest_sum += nearest;
            }
            let ratio = (nearest_sum / pts.len() as f32) / (all_sum / all_count);
            assert!(
                ratio < 0.36,
                "seed {seed}: nearest-neighbour/mean ratio {ratio:.3} -- systems are \
                 spread evenly rather than gathered into constellations"
            );
        }
    }
}

#[cfg(test)]
mod haven_spawn_tests {
    use super::*;
    use crate::celestial::orbits::PlanetPath;
    use crate::world::home_base::STATION_POS;

    /// Haven's star, as its fixed seed rolls it. Derived rather than
    /// hardcoded so this keeps telling the truth if that roll is retuned.
    fn havens_star_radius() -> f32 {
        system_roll(&haven_def()).star_radius()
    }

    fn spawn_to_star() -> f32 {
        STATION_POS.distance(HAVEN_LOCAL_CENTER)
    }

    /// The closest a planet's inner edge can come to the star, whatever its
    /// size. The orbit walk clears the previous body, this body's own radius
    /// and a gap, so on the first planet the radius cancels and the floor is
    /// exactly `star_radius + ORBIT_GAP_MIN`.
    fn nearest_possible_planet_edge(star_radius: f32) -> f32 {
        star_radius + crate::celestial::spawning::ORBIT_GAP_MIN
    }

    /// The ship must not spawn inside its own sun. Stars are solid and
    /// infinitely massive, so this is not a cosmetic concern.
    #[test]
    fn the_spawn_is_outside_havens_star() {
        let radius = havens_star_radius();
        let distance = spawn_to_star();
        assert!(
            distance > radius * 1.2,
            "spawn is {distance} from a star of radius {radius} -- inside it, or too close to it"
        );
    }

    /// ...and inside the planet orbits, which is the point of moving it --
    /// without ever sitting ON one, because planets are solid and sweep
    /// their whole orbit every few minutes.
    #[test]
    fn the_spawn_is_inside_the_planet_orbits() {
        let radius = havens_star_radius();
        let nearest_edge = nearest_possible_planet_edge(radius);
        let distance = spawn_to_star();
        assert!(
            distance < nearest_edge,
            "spawn is {distance} out; a planet's inner edge can reach {nearest_edge}, \
             so a solid world can sweep through the station"
        );
    }

    /// The fault this fixes, stated as a number. The old centre put the spawn
    /// 492,000 units from Haven's star, so every planet and every rock in the
    /// home system was unreachable and the only planet anyone ever saw was the
    /// parallax one glued to the camera.
    #[test]
    fn the_spawn_is_not_half_a_million_units_from_home() {
        let distance = spawn_to_star();
        assert!(
            distance < 150_000.0,
            "spawn is {distance} from Haven's star; the old value was 492,000 and that \
             is what made the home system unvisitable"
        );
    }

    /// Asteroids land on the side of the star the player arrives on, within
    /// a short flight rather than somewhere around the far limb.
    #[test]
    fn havens_asteroids_are_near_the_spawn() {
        let radius = havens_star_radius();
        let bearing = station_field_bearing(HAVEN_LOCAL_CENTER, STATION_POS);

        // Across the range of first-planet sizes and gaps the walk can roll.
        for first_radius in [10_000.0f32, 30_000.0, 60_000.0] {
            for gap in [crate::celestial::spawning::ORBIT_GAP_MIN, 140_000.0] {
                let orbit = radius + first_radius + gap;
                let planets = [PlanetPath { semi_major_axis: orbit, eccentricity: 0.0, radius: first_radius }];
                let field = HAVEN_LOCAL_CENTER + asteroid_field_offset(radius, &planets, bearing);
                let from_spawn = field.distance(STATION_POS);
                assert!(
                    from_spawn < FIELD_SPREAD * 2.0,
                    "first planet r={first_radius} gap={gap}: field centre is {from_spawn} \
                     from the spawn, too far to see rocks from where the player starts"
                );
            }
        }
    }

    /// The field must clear the bodies on both sides of it. This is why
    /// ORBIT_GAP_MIN is tied to FIELD_SPREAD rather than chosen by feel.
    #[test]
    fn the_field_never_touches_a_planet_or_the_star() {
        let star = havens_star_radius();
        for first_radius in [10_000.0f32, 30_000.0, 60_000.0] {
            for gap in [crate::celestial::spawning::ORBIT_GAP_MIN, 140_000.0] {
                let orbit = star + first_radius + gap;
                let planet = PlanetPath { semi_major_axis: orbit, eccentricity: 0.0, radius: first_radius };
                let field = asteroid_field_offset(star, &[planet], 0.0);
                assert!(
                    field.length() - FIELD_REACH > star,
                    "rocks reach {} from the centre, inside a star of {star}",
                    field.length() - FIELD_REACH
                );
                assert!(
                    planet.clearance(field) > FIELD_REACH,
                    "the planet passes {:.0} from the field's centre, inside its rocks",
                    planet.clearance(field)
                );
            }
        }
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::celestial::components::{CelestialBody, CelestialBodyType, OrbitalPath};
    use crate::celestial::poi::{SpacePoi, SpacePoiType};
    use crate::components::LogEntry;

    /// One system spawned headlessly, the way `load_system` spawns it.
    fn spawn(galaxy: &GalaxyMap, def: &StarSystemDef) -> (App, StarSystemInfo) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Image>();
        let records = crate::narrative::trail::records_in(galaxy, def.id);
        let def = def.clone();
        let textures = crate::vfx::procedural_textures::CelestialTextures {
            solid: default(),
            glow: default(),
            ring: default(),
        };
        let info = app
            .world_mut()
            .run_system_once(move |mut commands: Commands, assets: Res<AssetServer>| {
                spawn_system_contents(&mut commands, &assets, &textures, &def, &records, &[])
            })
            .unwrap();
        (app, info)
    }

    /// The places a pilot is sent to: everything but the asteroid rocks.
    fn destinations(app: &mut App) -> Vec<(SpacePoiType, Vec2, bool)> {
        let world = app.world_mut();
        let mut q = world.query::<(&SpacePoi, &Transform, Option<&LogEntry>)>();
        q.iter(world)
            .filter(|(poi, _, _)| poi.poi_type != SpacePoiType::AsteroidNode)
            .map(|(poi, tf, log)| (poi.poi_type, tf.translation.truncate(), log.is_some()))
            .collect()
    }

    /// Every derelict, anomaly and waystation -- and so every expedition
    /// record -- is somewhere a ship can stop: well outside its star, and
    /// never on a planet's path. Measured against the planets as spawned,
    /// walking each actual ellipse, not through `KeepClear` itself.
    #[test]
    fn points_of_interest_are_clear_of_stars_and_planet_paths() {
        let galaxy = generate_galaxy_map(42);
        for def in &galaxy.systems {
            let (mut app, info) = spawn(&galaxy, def);
            let world = app.world_mut();
            let mut q = world.query::<(&CelestialBody, &OrbitalPath)>();
            let paths: Vec<(Vec<Vec2>, f32)> = q
                .iter(world)
                .filter(|(body, _)| body.body_type == CelestialBodyType::Planet)
                .map(|(body, orbit)| {
                    let points = (0..2048)
                        .map(|k| {
                            let theta = k as f32 / 2048.0 * std::f32::consts::TAU;
                            let r = crate::celestial::orbits::orbit_radius(
                                orbit.semi_major_axis, orbit.eccentricity, theta,
                            );
                            def.local_center + Vec2::from_angle(theta) * r
                        })
                        .collect();
                    (points, body.radius)
                })
                .collect();
            for (kind, pos, _) in destinations(&mut app) {
                let from_star = pos.distance(def.local_center);
                assert!(
                    from_star > info.star_radius * 1.4,
                    "{}: {kind:?} {from_star:.0} from a star of radius {:.0}",
                    def.name, info.star_radius,
                );
                for (points, radius) in &paths {
                    let nearest = points.iter().map(|p| p.distance(pos)).fold(f32::INFINITY, f32::min);
                    assert!(
                        nearest > radius + 1_000.0,
                        "{}: {kind:?} {nearest:.0} from the path of a planet of radius {radius:.0}",
                        def.name,
                    );
                }
            }
        }
    }

    /// A jump drops you at a system's station, and that system's records are
    /// a short hop from it. Haven's first is the exception: it lies off the
    /// spawn berth, where training ends.
    #[test]
    fn records_wait_a_short_hop_from_the_station() {
        let galaxy = generate_galaxy_map(42);
        let mut checked = 0;
        for def in galaxy.systems.iter().filter(|s| s.id != 0) {
            let (mut app, _) = spawn(&galaxy, def);
            let station = crate::world::home_base::station_sites(def)[0].pos;
            for (_, pos, _) in destinations(&mut app).into_iter().filter(|(_, _, record)| *record) {
                // 15-35 km by design; up to ~57 km in seed 42 where a planet's
                // path or the star crowds the station and the record has to
                // look further round. Hundreds of km before.
                let hop = pos.distance(station);
                assert!(hop < 75_000.0, "{}: a record {hop:.0} from the station", def.name);
                checked += 1;
            }
        }
        assert!(checked > 10, "only {checked} records checked");
    }

    /// Each spawned planet as (points round its ellipse, radius), walked
    /// from the entities themselves.
    fn spawned_paths(app: &mut App, center: Vec2) -> Vec<(Vec<Vec2>, f32)> {
        let world = app.world_mut();
        let mut q = world.query::<(&CelestialBody, &OrbitalPath)>();
        q.iter(world)
            .filter(|(body, _)| body.body_type == CelestialBodyType::Planet)
            .map(|(body, orbit)| {
                let points = (0..2048)
                    .map(|k| {
                        let theta = k as f32 / 2048.0 * std::f32::consts::TAU;
                        let r = crate::celestial::orbits::orbit_radius(
                            orbit.semi_major_axis, orbit.eccentricity, theta,
                        );
                        center + Vec2::from_angle(theta) * r
                    })
                    .collect();
                (points, body.radius)
            })
            .collect()
    }

    fn nearest(points: &[Vec2], pos: Vec2) -> f32 {
        points.iter().map(|p| p.distance(pos)).fold(f32::INFINITY, f32::min)
    }

    const SEEDS: [u64; 4] = [42, 1, 7, 99_999];

    /// Stations are placed from planets rolled off the system's seed, never
    /// from spawned ones -- docking, the map and warp arrival need them with
    /// nothing loaded. That only works while the roll and the spawn agree, so
    /// compare the roll with what actually spawns.
    #[test]
    fn the_seed_roll_is_the_system_that_spawns() {
        for seed in SEEDS {
            let galaxy = generate_galaxy_map(seed);
            for def in &galaxy.systems {
                let roll = system_roll(def);
                let (mut app, info) = spawn(&galaxy, def);
                assert_eq!(roll.star_radius(), info.star_radius, "seed {seed}: {} star", def.name);
                let world = app.world_mut();
                let mut q = world.query::<(&CelestialBody, &OrbitalPath)>();
                let mut spawned: Vec<(f32, f32, f32)> = q
                    .iter(world)
                    .filter(|(body, _)| body.body_type == CelestialBodyType::Planet)
                    .map(|(body, orbit)| (orbit.semi_major_axis, orbit.eccentricity, body.radius))
                    .collect();
                spawned.sort_by(|a, b| a.0.total_cmp(&b.0));
                let rolled: Vec<(f32, f32, f32)> = roll
                    .planet_paths()
                    .iter()
                    .map(|p| (p.semi_major_axis, p.eccentricity, p.radius))
                    .collect();
                assert_eq!(rolled, spawned, "seed {seed}: {} planets", def.name);
            }
        }
    }

    /// Planets sweep their whole ellipse every few minutes and are solid.
    /// No station -- nor the docking range round it, where a jump drops you
    /// -- and no asteroid may sit anywhere one passes. Before this, 16 of the
    /// 31 systems in seed 42 had a station on a planet's path and 89 rocks
    /// sat on one. Measured against the planets as spawned, walking each
    /// actual ellipse.
    #[test]
    fn no_station_or_rock_is_on_a_planets_path() {
        for seed in SEEDS {
            let galaxy = generate_galaxy_map(seed);
            for def in &galaxy.systems {
                let (mut app, info) = spawn(&galaxy, def);
                let paths = spawned_paths(&mut app, def.local_center);

                for site in crate::world::home_base::station_sites(def) {
                    let from_star = site.pos.distance(def.local_center);
                    assert!(
                        from_star > info.star_radius * 1.4,
                        "seed {seed}: {} {from_star:.0} from a star of radius {:.0}",
                        site.name, info.star_radius,
                    );
                    for (points, radius) in &paths {
                        let surface = nearest(points, site.pos) - radius;
                        assert!(
                            surface > site.dock_range(),
                            "seed {seed}: a planet of radius {radius:.0} passes {surface:.0} from \
                             {}, inside its {:.0} docking range",
                            site.name, site.dock_range(),
                        );
                    }
                }

                let world = app.world_mut();
                let mut q = world.query::<(&CelestialBody, &Transform)>();
                let rocks: Vec<(Vec2, f32)> = q
                    .iter(world)
                    .filter(|(body, _)| body.body_type == CelestialBodyType::Asteroid)
                    .map(|(body, tf)| (tf.translation.truncate(), body.radius))
                    .collect();
                assert!(!rocks.is_empty(), "seed {seed}: {} has no asteroids", def.name);
                for (pos, rock) in rocks {
                    assert!(
                        pos.distance(def.local_center) - rock > info.star_radius,
                        "seed {seed}: {} has a rock inside its star",
                        def.name,
                    );
                    for (points, radius) in &paths {
                        let surface = nearest(points, pos) - radius - rock;
                        assert!(
                            surface > 1_000.0,
                            "seed {seed}: {} has a rock {surface:.0} from the surface of a \
                             planet of radius {radius:.0} at its closest pass",
                            def.name,
                        );
                    }
                }
            }
        }
    }
}
