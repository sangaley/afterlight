use bevy::prelude::*;
use crate::ai_ship::components::AiShipType;

/// How much is known about a star system on the galaxy map. Unknown systems
/// are invisible; Located ones show up as a bare pip (position known, not
/// much else) once revealed by scanning or a purchased star chart; Visited
/// is everything once you've actually warped there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemDiscovery {
    Unknown,
    Located,
    Visited,
}

impl SystemDiscovery {
    /// Stable numeric encoding for save files (see resources::SystemSaveData).
    pub fn as_u8(self) -> u8 {
        match self {
            SystemDiscovery::Unknown => 0,
            SystemDiscovery::Located => 1,
            SystemDiscovery::Visited => 2,
        }
    }
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => SystemDiscovery::Located,
            2 => SystemDiscovery::Visited,
            _ => SystemDiscovery::Unknown,
        }
    }
}

/// One star system's entry in the persistent galaxy layout — generated once
/// per session, not regenerated per-warp like today's disposable systems.
#[derive(Clone, Debug)]
pub struct StarSystemDef {
    pub id: u32,
    pub name: String,
    /// Abstract position used only for the galaxy map layout and the
    /// distance-based warp cost/charge-time formula — NOT a real Bevy
    /// Transform coordinate (see the plan's coordinate-model writeup for why:
    /// only one system is ever physically spawned at a time, so there's no
    /// shared physics frame for this to be a "real" position in).
    pub galaxy_pos: Vec2,
    /// Where this system's star/planets/asteroids actually spawn when it's
    /// loaded, at today's existing spawn-scale (hundreds of thousands of
    /// units) — unrelated to `galaxy_pos`.
    pub local_center: Vec2,
    pub seed: u64,
    /// Who holds it (celestial::territory). None = nobody's: Haven.
    pub faction: Option<AiShipType>,
    /// Its owner rules from here.
    pub capital: bool,
    /// A boss that haunts it (the Eternal Hegemony or the Shepherd hold no
    /// ground; they haunt the deepest systems). Its danger is the boss's.
    pub haunt: Option<AiShipType>,
    /// == faction_power(faction), or 0.0 if safe. Cached here rather than
    /// recomputed from `faction` every time so danger_tier is one field to
    /// read wherever it's needed (spawner.rs, map UI color-coding).
    pub danger_tier: f32,
    pub discovery: SystemDiscovery,
    /// Game time (Time::elapsed_secs_f64) this system's mutable state was
    /// last brought up to date — the anchor for the Cold-tier catch-up
    /// math (Phase 4): `elapsed = now - last_updated`.
    pub last_updated: f64,
    /// 1.0 = untouched, 0.0 = fully depleted. No floor — real depletion,
    /// driven by real Warm-tier simulation or the Cold-tier lazy formula.
    pub resource_fraction_remaining: f32,
    /// How far the Cascade has it (narrative::front): 0.0 untouched,
    /// between 0 and 1 the dark is reaching it, 1.0 taken.
    pub dark: f32,
}

/// The persistent galaxy layout — generated once per session. Replaces the
/// old model of despawning/regenerating a single disposable system on every
/// warp; see celestial::galaxy for generation and celestial::warp for how a
/// warp now targets one of these by id instead of rolling a random one.
#[derive(Resource, Default)]
pub struct GalaxyMap {
    pub systems: Vec<StarSystemDef>,
    pub galaxy_seed: u64,
}

/// Tracks the Hot/Warm tiers of the galaxy's tiered simulation (see the
/// plan's coordinate-model writeup): `loaded_system` is the one system with
/// real physical entities right now (Haven at game start); `warm_systems`
/// are its nearest neighbors on the galaxy map, which keep ticking through
/// `ai_ship::simulation::tick_world_simulation`'s abstract off-screen
/// combat/movement even with nothing physically spawned. Everything else is
/// Cold — never ticked, brought up to date via one-shot catch-up math
/// (celestial::galaxy::catch_up_system) the moment it matters again.
#[derive(Resource, Default)]
pub struct SystemStreamingManager {
    pub loaded_system: Option<u32>,
    pub warm_systems: Vec<u32>,
    /// Galaxy-space position of wherever the player currently is — mirrors
    /// `loaded_system`'s galaxy_pos when Hot is a real system, but also
    /// valid when loaded_system is None (a blind warp landed in empty
    /// space). Used for passive proximity discovery, which doesn't care
    /// whether you're standing in a system or open void.
    pub current_galaxy_pos: Vec2,
}

/// Where an interstellar warp is headed — either a known system (clicked
/// directly, or close enough on the map to snap to it) or a raw point in
/// galaxy space with nothing confirmed there yet (a blind jump: the galaxy
/// map is clickable anywhere, not just on discovered pips, matching "one
/// continuous space" rather than a discovered-systems-only picker).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GalaxyWarpTarget {
    System(u32),
    BlindPoint(Vec2),
}

/// What the player has chosen to warp to via the galaxy map — what
/// celestial::warp::warp_input_system now charges toward, replacing the old
/// GalaxyState.next_system_id auto-increment. Distinct from
/// ui::PendingWarpTarget (a Vec2, the LOCAL same-system G-key dash target)
/// — different scales, must not be conflated.
#[derive(Resource, Default)]
pub struct PendingGalaxyWarpTarget(pub Option<GalaxyWarpTarget>);

/// Derelicts and anomalies already stripped, per system, by their slot in
/// that system's spawn order (see `poi::PoiSlot`).
///
/// A system's entities are despawned on every jump and respawned from its
/// seed, and the seed rolls every hulk full again -- so without this the same
/// derelict paid out on every visit. Written the moment something is looted
/// rather than on unload, and saved with the galaxy
/// (`resources::SystemSaveData::looted_pois`).
#[derive(Resource, Default)]
pub struct LootedPois(pub std::collections::HashMap<u32, Vec<u32>>);

impl LootedPois {
    pub fn mark(&mut self, system_id: u32, slot: u32) {
        let slots = self.0.entry(system_id).or_default();
        if !slots.contains(&slot) {
            slots.push(slot);
        }
    }

    pub fn for_system(&self, system_id: u32) -> &[u32] {
        self.0.get(&system_id).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Lightweight metadata per star system (bookkeeping, not ECS)
#[derive(Clone, Debug)]
pub struct StarSystemInfo {
    pub id: u32,
    pub star_entity: Option<Entity>,
    pub planet_entities: Vec<Entity>,
    pub center: Vec2,
    pub is_alive: bool,
    /// The star's own radius. Published because the star is SOLID and
    /// enormous (40k-150k by class), so anything placed in the system has to
    /// be placed outside it -- see `galaxy::belt_radii`.
    pub star_radius: f32,
    /// Each planet's whole path, innermost first, eccentricity included.
    ///
    /// Not `(orbit, radius)` circles, which is what this used to publish: at
    /// eccentricity up to 0.3 a planet strays up to 30% either side of that
    /// circle, and the asteroid field placed against the circles had 89 rocks
    /// on a planet's real path in the seed-42 galaxy. Anything placed to stay
    /// out of a planet's way measures against this (see `poi::KeepClear`).
    pub planet_paths: Vec<super::orbits::PlanetPath>,
}

/// Global galaxy state
#[derive(Resource)]
pub struct GalaxyState {
    pub systems: Vec<StarSystemInfo>,
    pub current_system: u32,
    pub total_bodies: u32,
    pub galaxy_time: f64,
    pub next_system_id: u32,
}

impl Default for GalaxyState {
    fn default() -> Self {
        Self {
            systems: Vec::new(),
            current_system: 0,
            total_bodies: 0,
            galaxy_time: 0.0,
            next_system_id: 0,
        }
    }
}

/// All tuning constants for celestial mechanics
#[derive(Resource)]
pub struct CelestialConfig {
    /// Gameplay-tuned gravity constant
    pub gravity_constant: f32,
    /// Max force applied to ship (prevents instant death)
    pub max_gravity_force: f32,
    /// How fast black holes consume bodies
    pub black_hole_consume_speed: f32,
    /// Random flare buildup rate range (per second)
    pub flare_buildup_rate_min: f32,
    pub flare_buildup_rate_max: f32,
    /// How much worse flare radiation is vs base stellar radiation
    pub flare_radiation_multiplier: f32,
    /// How long a flare lasts (seconds)
    pub flare_duration: f32,
    /// Supernova blast radius
    pub star_death_supernova_radius: f32,
    /// Supernova damage
    pub supernova_damage: f32,
    /// Speed multiplier for freed planets
    pub freed_planet_speed_multiplier: f32,
    /// Warp charge time (seconds)
    pub warp_charge_time: f32,
    /// Distance between star systems
    pub system_spacing: f32,
    /// Gradual crush damage per second near black hole event horizon
    pub black_hole_crush_damage_rate: f32,
}

impl Default for CelestialConfig {
    fn default() -> Self {
        Self {
            // Gravity — realistic. If your thrust < gravity pull, you're dead.
            gravity_constant: 600_000.0,        // Strong enough that underpowered ships get pulled in
            max_gravity_force: 50_000.0,        // Effectively uncapped — physics decides, not a clamp
            // Black holes — terrifying but survivable if you react
            black_hole_consume_speed: 0.2,      // Slower consumption = more escape time
            black_hole_crush_damage_rate: 30.0,  // You went near a black hole without a gravity compensator. That's on you.
            // Flares — unpredictable but not constant
            flare_buildup_rate_min: 0.005,      // Slower buildup = less frequent flares
            flare_buildup_rate_max: 0.03,       // Reduced max rate
            flare_radiation_multiplier: 8.0,     // Deadly without shielding. Get behind a planet or die.
            flare_duration: 6.0,                // Slightly shorter
            // Supernova — the big event
            star_death_supernova_radius: 60_000.0,  // Bigger radius — harder to avoid
            supernova_damage: 800.0,             // Near-instant kill if you're close. Run or die.
            freed_planet_speed_multiplier: 60.0, // Slower freed planets — more time to dodge
            // Warp — should feel intentional, not instant
            warp_charge_time: 8.0,              // Increased from 5 — commitment required
            system_spacing: 200_000.0,
        }
    }
}
