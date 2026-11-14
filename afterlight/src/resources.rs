// Resource fields are part of the data model — not all are consumed by systems yet.
#![allow(dead_code)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::components::{
    ModuleType, ModuleCategory, Rotation, HullLayer, HullMaterial, SubComponentType,
    CalculatedStats, WeaponStats, EngineStats, ReactorStats, LifeSupportStats,
    ComponentPieceType, ComponentPiece,
};
use crate::states::GameState;

/// Remembers which state we paused from so ESC resumes correctly
#[derive(Resource, Default)]
pub struct PrePauseState(pub Option<GameState>);

/// Tracks how long the current Exploring session has been running.
/// Resets on each OnEnter(Exploring) to provide proper grace periods.
#[derive(Resource, Default)]
pub struct ExploringSessionTimer {
    pub elapsed: f32,
}

// ============================================================================
// SUBMARINE RESOURCES
// ============================================================================

#[derive(Resource, Default)]
pub struct DepthState {
    pub current_depth: f32,
    pub target_depth: f32,
}

#[derive(Resource, Default)]
pub struct PowerState {
    pub total_power_generation: f32,
    pub total_power_consumption: f32,
    pub power_balance: f32,
}

#[derive(Resource, Default)]
pub struct PowerGraph {
    pub powered_tiles: HashSet<IVec2>,
}

// ============================================================================
// POWER ROUTING — player-controlled reactor allocation (Weapons/Shields/Engines)
// ============================================================================

/// The three combat systems the player can route reactor power between.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PowerChannel {
    Weapons,
    Shields,
    Engines,
}

impl PowerChannel {
    pub const ALL: [PowerChannel; 3] =
        [PowerChannel::Weapons, PowerChannel::Shields, PowerChannel::Engines];

    pub fn name(&self) -> &'static str {
        match self {
            PowerChannel::Weapons => "WEAPONS",
            PowerChannel::Shields => "SHIELDS",
            PowerChannel::Engines => "ENGINES",
        }
    }
}

/// Player-set routing of reactor output across the three combat channels.
/// Each value is a 0..=100 "power level" and the three do NOT have to sum to
/// 100 — asking for more than the reactor can supply browns the channels out
/// (see [`PowerChannels`] / `update_power_allocation`). A balanced 50/50/50
/// spends the whole reactor at nominal (×1.0) performance everywhere.
#[derive(Resource, Clone, Copy)]
pub struct PowerAllocation {
    pub weapons: f32,
    pub shields: f32,
    pub engines: f32,
}

impl Default for PowerAllocation {
    fn default() -> Self {
        Self { weapons: 50.0, shields: 50.0, engines: 50.0 }
    }
}

impl PowerAllocation {
    pub const MIN: f32 = 0.0;
    pub const MAX: f32 = 100.0;
    pub const STEP: f32 = 10.0;

    pub fn get(&self, ch: PowerChannel) -> f32 {
        match ch {
            PowerChannel::Weapons => self.weapons,
            PowerChannel::Shields => self.shields,
            PowerChannel::Engines => self.engines,
        }
    }

    /// Set one channel to an exact level, clamped to [MIN, MAX]. Used by the
    /// draggable sliders for arbitrary (non-stepped) allocations.
    pub fn set(&mut self, ch: PowerChannel, value: f32) {
        let v = value.clamp(Self::MIN, Self::MAX);
        match ch {
            PowerChannel::Weapons => self.weapons = v,
            PowerChannel::Shields => self.shields = v,
            PowerChannel::Engines => self.engines = v,
        }
    }

    /// Nudge one channel by `delta`, clamped to [MIN, MAX]. Channels are
    /// independent — raising one does not lower the others (over-commit is
    /// allowed and surfaces as a brownout).
    pub fn adjust(&mut self, ch: PowerChannel, delta: f32) {
        let v = (self.get(ch) + delta).clamp(Self::MIN, Self::MAX);
        match ch {
            PowerChannel::Weapons => self.weapons = v,
            PowerChannel::Shields => self.shields = v,
            PowerChannel::Engines => self.engines = v,
        }
    }
}

/// Live per-channel result of routing, recomputed every frame from
/// [`PowerAllocation`] + the reactor's current generation. `*_mult` is the
/// performance multiplier each gameplay system reads (1.0 = nominal, capped
/// at 2.0, ~0.0 = browned out / offline). `brownout` is true when the trio's
/// total demand exceeds what the reactor can supply.
#[derive(Resource, Clone, Copy)]
pub struct PowerChannels {
    pub weapons_mult: f32,
    pub shields_mult: f32,
    pub engines_mult: f32,
    /// Total power the three channels requested this frame.
    pub demand: f32,
    /// Reactor power available to the trio this frame (= total generation).
    pub supply: f32,
    pub brownout: bool,
}

impl Default for PowerChannels {
    fn default() -> Self {
        // Nominal until the first compute — systems behave as before when the
        // routing layer hasn't run yet (e.g. while docked).
        Self {
            weapons_mult: 1.0,
            shields_mult: 1.0,
            engines_mult: 1.0,
            demand: 0.0,
            supply: 0.0,
            brownout: false,
        }
    }
}

impl PowerChannels {
    pub fn mult(&self, ch: PowerChannel) -> f32 {
        match ch {
            PowerChannel::Weapons => self.weapons_mult,
            PowerChannel::Shields => self.shields_mult,
            PowerChannel::Engines => self.engines_mult,
        }
    }
}

/// Double-buffered heat map for heat diffusion calculations
#[derive(Resource, Default)]
pub struct HeatNetworkState {
    /// Keyed by (owning ship, ship-local cell).
    ///
    /// Keyed by bare IVec2 this was a single map shared by every ship in the
    /// world — and grid coordinates are ship-local, so an AI reactor at ITS
    /// (0,0) wrote into the player's (0,0). The workaround was to scope the
    /// whole heat simulation to the player, which meant enemy guns never
    /// overheated: yours stuttered under sustained fire and theirs didn't.
    /// Same shape of bug GridOccupancy had, same fix.
    pub temperatures: HashMap<(Entity, IVec2), f32>,
    pub prev_temperatures: HashMap<(Entity, IVec2), f32>,
}

#[derive(Resource, Default)]
pub struct ResearchState {
    pub research_points: f32,
    pub research_rate: f32,
}

#[derive(Resource, Default)]
pub struct AutopilotState {
    pub enabled: bool,
    pub target_depth: f32,
}

/// Accumulated targeting computer accuracy bonus for the current frame.
#[derive(Resource, Default)]
pub struct TargetingBonus {
    pub accuracy_bonus: f32,
}

#[derive(Resource, Default)]
pub struct OxygenState {
    pub total_oxygen_generation: f32,
    pub total_oxygen_consumption: f32,
    pub oxygen_balance: f32,
    pub current_oxygen: f32,
    pub max_oxygen: f32,
}

#[derive(Resource, Default)]
pub struct HullState {
    pub hull_integrity: f32,
    pub max_radiation_shielding: f32,
}

#[derive(Resource, Default)]
pub struct NoiseState {
    pub noise_level: f32,
}

/// Fuel state for combustion engines (Phase 3.3)
#[derive(Resource)]
pub struct FuelState {
    pub current_fuel: f32,
    pub max_fuel: f32,
    pub fuel_consumption_rate: f32,
}

impl Default for FuelState {
    fn default() -> Self {
        Self {
            current_fuel: 1500.0,    // Enough for a real expedition; outposts resupply
            max_fuel: 1500.0,
            // Per engine. 0.8 was set for a five-engine ship; the starter now
            // has twenty, and at 0.8 it burned its tank in 94 s at full throttle.
            fuel_consumption_rate: 0.2,
        }
    }
}

/// Ship blueprint for saving/loading
#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct ShipBlueprint {
    pub hull_segments: Vec<HullSegmentData>,
    pub modules: Vec<ModuleData>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HullSegmentData {
    pub grid_position: IVec2,
    pub health: f32,
    pub radiation_shielding: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModuleData {
    pub module_type: ModuleType,
    pub grid_position: IVec2,
    pub health: f32,
    #[serde(default = "default_rotation")]
    pub rotation: Rotation,
    #[serde(default = "default_true")]
    pub is_active: bool,
    pub custom_data: Option<CustomModuleData>,
    /// Multi-block customization parameter values (Tier 3 slider data)
    #[serde(default)]
    pub customization_params: Option<std::collections::HashMap<String, f32>>,
    /// Stat tuning multipliers (velocity, fire_rate, damage)
    #[serde(default)]
    pub tuning: Option<crate::building::customization::tuning::WeaponTuning>,
    /// Loaded kinetic ammo type
    #[serde(default)]
    pub selected_ammo: Option<crate::combat::ammo_types::KineticAmmoType>,
    /// On a turret deck: the hull cell of its Turret Ring; grid_position is
    /// then in the turret's own grid (combat::turret_deck).
    #[serde(default)]
    pub turret: Option<IVec2>,
    /// A built gun's shell design (combat::shell), on its firing piece.
    #[serde(default)]
    pub shell: Option<crate::combat::shell::ShellDesign>,
    /// A launcher's missile design (combat::missile_design).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missile: Option<crate::combat::missile_design::MissileDesign>,
    /// On point defence (combat::missile_defence).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub point_defence: bool,
    /// Built to a faction's signature (diplomacy::signatures), by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

fn default_rotation() -> Rotation { Rotation::North }
fn default_true() -> bool { true }

/// Serialized data for custom modules
#[derive(Serialize, Deserialize, Clone)]
pub struct CustomModuleData {
    pub custom_name: String,
    pub subcomponents: Vec<SubComponentType>,
}

impl Default for ShipBlueprint {
    fn default() -> Self {
        Self {
            hull_segments: Vec::new(),
            modules: Vec::new(),
        }
    }
}

// ============================================================================
// WORLD RESOURCES
// ============================================================================

#[derive(Resource)]
pub struct WorldState {
    pub seed: u64,
    pub time_of_day: f32,
    pub current_biome: BiomeType,
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            seed: 0,
            time_of_day: 0.0,
            current_biome: BiomeType::OpenVoid,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BiomeType {
    OpenVoid,
    AsteroidField,
    CrystalFormation,
    VoidRift,
    ThermalVents,
    IceShells,
    DeadZone,
    AncientRuins,
}

/// Tracks which chunks are loaded
#[derive(Resource)]
pub struct ChunkManager {
    pub loaded_chunks: HashMap<IVec2, Entity>,
    pub chunk_size: f32,
    pub render_distance: i32,
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self {
            loaded_chunks: HashMap::new(),
            chunk_size: 512.0,
            render_distance: 3,
        }
    }
}

/// Tracks discovered locations for the map
#[derive(Resource, Default, Serialize, Deserialize, Clone)]
pub struct DiscoveredLocations {
    pub wrecks: Vec<Vec2>,
    pub caves: Vec<Vec2>,
    pub settlements: Vec<Vec2>,
    pub special: Vec<(Vec2, String)>,
}

// ============================================================================
// ECOSYSTEM RESOURCES
// ============================================================================

use crate::components::CreatureType;

/// Record of a creature kill for cascade detection
pub struct EcoKillRecord {
    pub killer_type: Option<CreatureType>,
    pub victim_type: CreatureType,
    pub position: Vec2,
    pub time: f32,
    pub by_player: bool,
}

/// Tracks the living ecosystem state
#[derive(Resource, Default)]
pub struct EcosystemState {
    pub population_counts: HashMap<CreatureType, u32>,
    pub recent_kills: Vec<EcoKillRecord>,
    pub total_elapsed: f32,
}

/// All ecosystem tuning constants
#[derive(Resource)]
pub struct EcosystemConfig {
    pub max_total_creatures: u32,
    pub per_type_caps: HashMap<CreatureType, u32>,
    pub hunt_hunger_threshold: f32,
    pub starve_hunger_threshold: f32,
    pub starvation_damage: f32,
    pub reproduction_hunger_max: f32,
    pub reproduction_cooldown: f32,
    pub territory_default_radius: f32,
    pub noise_emit_interval: f32,
    pub noise_decay_rate: f32,
    pub max_trail_points: usize,
    pub cascade_kill_count: u32,
    pub cascade_time_window: f32,
    pub max_corpses: usize,
    pub corpse_decay_time: f32,
}

impl Default for EcosystemConfig {
    fn default() -> Self {
        let mut per_type_caps = HashMap::new();
        per_type_caps.insert(CreatureType::VoidDrifter, 12);
        per_type_caps.insert(CreatureType::Stalker, 4);
        per_type_caps.insert(CreatureType::Leviathan, 1);
        per_type_caps.insert(CreatureType::ParasiteSwarm, 15);

        Self {
            max_total_creatures: 30,
            per_type_caps,
            hunt_hunger_threshold: 40.0,
            starve_hunger_threshold: 90.0,
            starvation_damage: 2.0,
            reproduction_hunger_max: 20.0,
            reproduction_cooldown: 60.0,
            territory_default_radius: 300.0,
            noise_emit_interval: 0.5,
            noise_decay_rate: 10.0,
            max_trail_points: 100,
            cascade_kill_count: 3,
            cascade_time_window: 30.0,
            max_corpses: 8,
            corpse_decay_time: 120.0,
        }
    }
}

// ============================================================================
// CREW RESOURCES
// ============================================================================

#[derive(Resource, Default)]
pub struct CrewRoster {
    pub members: Vec<Entity>,
}

#[derive(Resource, Default)]
pub struct StaffingState {
    pub total_berths: u32,
    pub total_crew: u32,
    pub staffed_stations: u32,
    pub total_stations: u32,
}

// ============================================================================
// INVENTORY / ECONOMY
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ItemType {
    ScrapMetal,
    Crystal,
    BioSample,
    FuelCell,
    RareAlloy,
    AncientArtifact,
    AmmoCrate,
}

impl ItemType {
    pub fn name(&self) -> &'static str {
        match self {
            ItemType::ScrapMetal => "Scrap Metal",
            ItemType::Crystal => "Crystal",
            ItemType::BioSample => "Bio Sample",
            ItemType::FuelCell => "Fuel Cell",
            ItemType::RareAlloy => "Rare Alloy",
            ItemType::AncientArtifact => "Ancient Artifact",
            ItemType::AmmoCrate => "Ammo Crate",
        }
    }

    pub fn weight(&self) -> f32 {
        match self {
            ItemType::ScrapMetal => 5.0,
            ItemType::Crystal => 2.0,
            ItemType::BioSample => 1.0,
            ItemType::FuelCell => 3.0,
            ItemType::RareAlloy => 8.0,
            ItemType::AncientArtifact => 10.0,
            ItemType::AmmoCrate => 4.0,
        }
    }

    /// Baseline credit value per unit, before any station price variation
    /// (see `station_item_price`). This used to be the actual sell price,
    /// identical at every station.
    fn base_value(&self) -> u32 {
        match self {
            ItemType::ScrapMetal => 12,
            ItemType::Crystal => 30,
            ItemType::BioSample => 18,
            ItemType::FuelCell => 24,
            ItemType::RareAlloy => 60,
            ItemType::AncientArtifact => 120,
            ItemType::AmmoCrate => 36,
        }
    }
}

/// Per-station price multiplier for a given item. Was a pure hash of
/// (station_idx, item) with no meaning behind it — every station had a
/// "personality" but there was no way to learn or predict it beyond trial
/// and error. Now driven by the station's type (see world::station_types —
/// a Mining Colony pays badly for ore because it's mining its own, a
/// Research Outpost pays well for crystal/artifacts because it wants to
/// study them, etc.) with a small +/-15% per-station jitter on top so two
/// stations of the same type aren't identical twins.
fn station_price_multiplier(station_idx: usize, item: ItemType) -> f32 {
    let base = crate::world::station_types::type_price_multiplier(
        crate::world::station_types::station_type(station_idx),
        item,
    );
    let hash = (station_idx as u32).wrapping_mul(2654435761)
        .wrapping_add((item as u32).wrapping_mul(40503))
        .wrapping_add(0x9E3779B9);
    let normalized = (hash % 1000) as f32 / 1000.0; // 0.0..1.0
    let jitter = 0.85 + normalized * 0.30; // 0.85..1.15
    base * jitter
}

/// Final per-unit sell price for `item` at station `station_idx`
/// (0 = Haven, 1..=12 = outposts — see world::home_base).
pub fn station_item_price(station_idx: usize, item: ItemType) -> u32 {
    ((item.base_value() as f32) * station_price_multiplier(station_idx, item))
        .round()
        .max(1.0) as u32
}

/// A temporary shortage: one station pays over the odds for one item
/// for a few minutes. Keeps trade routes from being solved once and
/// forever — the best route this run isn't the best route right now.
pub struct MarketEvent {
    pub station_idx: usize,
    pub item: ItemType,
    pub sell_mult: f32,
    pub remaining: f32,
}

#[derive(Resource)]
pub struct MarketEvents {
    pub active: Vec<MarketEvent>,
    /// Seconds until the next shortage roll.
    pub next_roll: f32,
    /// Docked at a faction station: its name and the terms your standing
    /// gets you (diplomacy::stations). None at an ordinary station.
    pub dock_terms: Option<DockTerms>,
}

/// What a faction station charges you, from your standing with it.
#[derive(Clone, Debug)]
pub struct DockTerms {
    /// Whose station; None for a free port (an ownerless station).
    pub faction: Option<crate::ai_ship::components::AiShipType>,
    pub name: String,
    /// Your standing with them, as the diplomacy window names it.
    pub standing: &'static str,
    /// Multiplies what you pay: goods and services.
    pub buy: f32,
    /// Multiplies what they pay you for cargo.
    pub sell: f32,
}

impl Default for MarketEvents {
    fn default() -> Self {
        Self { active: Vec::new(), next_roll: 120.0, dock_terms: None }
    }
}

impl MarketEvents {
    /// What services cost at the station you're docked at, on top of its
    /// own discounts: your standing's terms at a faction station, else 1.
    pub fn service_multiplier(&self) -> f32 {
        self.dock_terms.as_ref().map_or(1.0, |t| t.buy)
    }

    pub fn multiplier(&self, station_idx: usize, item: ItemType) -> f32 {
        self.active
            .iter()
            .filter(|e| e.station_idx == station_idx && e.item == item)
            .map(|e| e.sell_mult)
            .fold(1.0, f32::max)
    }
}

/// Sell price including any active shortage at this station.
pub fn live_item_price(events: &MarketEvents, station_idx: usize, item: ItemType) -> u32 {
    let terms = events.dock_terms.as_ref().map_or(1.0, |t| t.sell);
    (shelf_price(events, station_idx, item) * terms).round().max(1.0) as u32
}

/// The station's own sell price with any shortage, before your terms.
fn shelf_price(events: &MarketEvents, station_idx: usize, item: ItemType) -> f32 {
    (station_item_price(station_idx, item) as f32) * events.multiplier(station_idx, item)
}

/// Per-unit BUY price at a station: 1.5x its (live) sell price. The
/// spread kills same-station arbitrage, while station identity keeps
/// cross-station routes alive — a Mining Colony's cheap scrap (0.55x
/// sell → ~0.83x buy) resells at a Trade Hub (1.2x) at a real margin.
/// Shortages raise the local buy price too: scarce goods cost more
/// everywhere they're scarce.
pub fn live_item_buy_price(events: &MarketEvents, station_idx: usize, item: ItemType) -> u32 {
    let sell = shelf_price(events, station_idx, item).round().max(1.0) as u32;
    let base = (sell * 3 / 2).max(sell + 2);
    let terms = events.dock_terms.as_ref().map_or(1.0, |t| t.buy);
    // Never below what they'd pay you for it, or buy-and-resell at one
    // counter would print money on good terms.
    ((base as f32 * terms).round() as u32).max(live_item_price(events, station_idx, item) + 1)
}

#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct Inventory {
    pub items: HashMap<ItemType, u32>,
    pub max_capacity: f32,
    pub current_weight: f32,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            items: HashMap::new(),
            max_capacity: 100.0,
            current_weight: 0.0,
        }
    }
}

impl Inventory {
    pub fn add_item(&mut self, item: ItemType, count: u32) -> bool {
        let added_weight = item.weight() * count as f32;
        if self.max_capacity > 0.0 && self.current_weight + added_weight > self.max_capacity {
            return false;
        }
        *self.items.entry(item).or_insert(0) += count;
        self.current_weight += added_weight;
        true
    }

    pub fn remove_item(&mut self, item: ItemType, count: u32) -> bool {
        if let Some(current) = self.items.get_mut(&item) {
            if *current >= count {
                *current -= count;
                self.current_weight -= item.weight() * count as f32;
                if *current == 0 {
                    self.items.remove(&item);
                }
                return true;
            }
        }
        false
    }

}

/// Oxygen a single crew member burns per second.
///
/// Sized against the starter hull, which is the only ship whose numbers are
/// fixed: two scrubbers at 30/s each, and a full complement of 20 (one per
/// berth, see crew::spawn_starter_crew). At 1.5 that is 30/s consumed against
/// 60/s generated, so the ship breathes comfortably intact AND exactly holds
/// its own on a single surviving scrubber — losing one is a warning, losing
/// both is the emergency.
///
/// It was 3.0, sized for a fixed roster of 8. Once the crew gate meant a ship
/// wanted one hand per post, a full complement consumed exactly what it
/// generated, and any scrubber damage at all started killing people.
pub const OXYGEN_PER_CREW: f32 = 1.5;

#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct Currency {
    pub credits: u32,
}

impl Default for Currency {
    fn default() -> Self {
        Self { credits: 750 } // Enough for a solid starter build with one weapon system
    }
}

// ============================================================================
// PROGRESSION / UNLOCKS
// ============================================================================

#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct Unlocks {
    pub modules: Vec<String>,
    pub hull_types: Vec<String>,
    pub blueprints_found: Vec<String>,
}

impl Default for Unlocks {
    fn default() -> Self {
        Self {
            modules: vec![
                "reactor".into(), "engine".into(), "quarters".into(),
                "oxygen".into(), "thruster".into(), "light".into(),
            ],
            hull_types: vec!["standard".into()],
            blueprints_found: Vec::new(),
        }
    }
}

#[derive(Resource, Default, Serialize, Deserialize, Clone)]
pub struct Statistics {
    pub max_depth_reached: f32,
    pub creatures_encountered: HashMap<String, u32>,
    pub creatures_killed: u32,
    pub wrecks_salvaged: u32,
    pub crew_lost: u32,
    pub ships_lost: u32,
    pub play_time_seconds: f32,
    pub logs_found: Vec<String>,
}

// ============================================================================
// VICTORY
// ============================================================================

#[derive(Resource, Default)]
pub struct VictoryState {
    pub achieved: bool,
}

/// Why the last run ended — shown on the death screen so players learn what
/// actually killed them instead of guessing.
#[derive(Resource, Default)]
pub struct DeathCause {
    /// Final verdict, composed by check_game_over when the run ends.
    pub cause: Option<String>,
    /// Most recent damage taken: (description, elapsed seconds when it hit).
    /// Used to attribute hull/crew deaths to their actual source.
    pub last_damage: Option<(String, f64)>,
}

// ============================================================================
// GAME CONFIG
// ============================================================================

#[derive(Resource)]
pub struct GameConfig {
    // Radiation
    pub radiation_damage_multiplier: f32,
    pub radiation_per_unit: f32,

    // Oxygen
    pub base_oxygen_consumption_per_crew: f32,
    pub suffocation_damage_rate: f32,

    // Movement
    pub base_ship_speed: f32,
    pub depth_change_speed: f32,

    // Creatures
    pub creature_spawn_rate: f32,
    pub creature_detection_noise_threshold: f32,
    pub creature_detection_light_threshold: f32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            // Radiation — brutal near stars without shielding. Player MUST build for it.
            radiation_damage_multiplier: 1.5,    // Higher than original — punishment for no shielding
            radiation_per_unit: 0.12,            // Slightly higher base — void isn't safe either
            // Oxygen — you NEED scrubbers. No scrubbers = crew dies fast.
            // See OXYGEN_PER_CREW for why this number is what it is.
            base_oxygen_consumption_per_crew: OXYGEN_PER_CREW,
            suffocation_damage_rate: 8.0,           // Fast death without air — motivates building life support
            // Movement — should feel weighty in space
            base_ship_speed: 120.0,          // Slightly faster — space is big
            depth_change_speed: 25.0,
            // Creatures — steady trickle, not overwhelming
            creature_spawn_rate: 0.08,            // Slightly slower spawning
            creature_detection_noise_threshold: 40.0, // Easier to detect creatures
            creature_detection_light_threshold: 25.0,
        }
    }
}

// ============================================================================
// INPUT STATE
// ============================================================================

/// F is bound to four separate actions - recall a salvage detail, send one
/// out, dock, and turn in contracts at a station - and each one used to read
/// the raw keypress on its own. Nothing arbitrated between them, so wherever
/// their conditions overlapped they ALL fired on the same press: parking on a
/// wreck inside a station's dock radius docked the ship *and* pushed a detail
/// out onto the hull, stranding them the moment the ship teleported to its
/// berth.
///
/// The press is now a single token. Handlers run in a fixed order (see the
/// `.after()` chains where they're registered) and the first one whose own
/// conditions are actually met claims it; everyone downstream sees nothing.
/// Claim LAST, once you know you're going to act - a handler that claims and
/// then bails swallows the press.
#[derive(Resource, Default)]
pub struct InteractPress {
    available: bool,
}

impl InteractPress {
    /// Take the press if it's still going. True means it's yours to act on.
    pub fn claim(&mut self) -> bool {
        let got = self.available;
        self.available = false;
        got
    }

    /// Peek without taking - for handlers that need to check other conditions
    /// before committing.
    pub fn pending(&self) -> bool {
        self.available
    }
}

/// Re-arms the interact token once per frame, ahead of every handler.
pub fn refresh_interact_press(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut press: ResMut<InteractPress>,
) {
    press.available = keyboard.just_pressed(KeyCode::KeyF);
}

#[derive(Resource, Default)]
pub struct InputState {
    pub movement: Vec2,
    pub depth_input: f32,       // -1 rise, +1 sink
    pub mouse_world_pos: Vec2,
    pub mouse_grid_pos: IVec2,
    pub brake: bool,            // Shift — retro-thrust against current velocity
    /// Right-stick aim direction (ship-facing + dumb-fire weapons). Persists
    /// after the stick releases so the nose holds heading; cleared when the
    /// mouse moves and takes aim back. See gamepad::gamepad_flight.
    pub gamepad_aim: Option<Vec2>,
    /// World-space thrust, -1..1, from holding range on a locked ship
    /// (ship::keep_range). Zero whenever the player is steering.
    pub autopilot: Vec2,
}

// ============================================================================
// BUILDING MODE (category-based selection)
// ============================================================================

/// Build categories include hull as a special category + all module categories
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildCategory {
    Hull,
    Power,
    Propulsion,
    LifeSupport,
    Control,
    Weapons,
    Guns,
    Lasers,
    Detection,
    Storage,
    Crew,
    Utility,
    Structural,
}

impl BuildCategory {
    pub const ALL: &'static [BuildCategory] = &[
        BuildCategory::Hull,
        BuildCategory::Power,
        BuildCategory::Propulsion,
        BuildCategory::LifeSupport,
        BuildCategory::Control,
        BuildCategory::Weapons,
        BuildCategory::Guns,
        BuildCategory::Lasers,
        BuildCategory::Detection,
        BuildCategory::Storage,
        BuildCategory::Crew,
        BuildCategory::Utility,
        BuildCategory::Structural,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            BuildCategory::Hull => "Hull",
            BuildCategory::Power => "Power",
            BuildCategory::Propulsion => "Propulsion",
            BuildCategory::LifeSupport => "Life Support",
            BuildCategory::Control => "Control",
            BuildCategory::Weapons => "Weapons",
            BuildCategory::Guns => "Guns",
            BuildCategory::Lasers => "Lasers",
            BuildCategory::Detection => "Detection",
            BuildCategory::Storage => "Storage",
            BuildCategory::Crew => "Crew",
            BuildCategory::Utility => "Utility",
            BuildCategory::Structural => "Structural",
        }
    }

    /// Convert to ModuleCategory. Returns None for Hull.
    pub fn to_module_category(&self) -> Option<ModuleCategory> {
        match self {
            BuildCategory::Hull => None,
            BuildCategory::Power => Some(ModuleCategory::Power),
            BuildCategory::Propulsion => Some(ModuleCategory::Propulsion),
            BuildCategory::LifeSupport => Some(ModuleCategory::LifeSupport),
            BuildCategory::Control => Some(ModuleCategory::Control),
            BuildCategory::Weapons => Some(ModuleCategory::Weapons),
            BuildCategory::Guns => Some(ModuleCategory::Guns),
            BuildCategory::Lasers => Some(ModuleCategory::Lasers),
            BuildCategory::Detection => Some(ModuleCategory::Detection),
            BuildCategory::Storage => Some(ModuleCategory::Storage),
            BuildCategory::Crew => Some(ModuleCategory::Crew),
            BuildCategory::Utility => Some(ModuleCategory::Utility),
            BuildCategory::Structural => Some(ModuleCategory::Structural),
        }
    }

    /// What this tab actually offers, in slot order.
    ///
    /// Not simply `to_module_category().module_types()`: the Structural tab
    /// hands its two angled plates to the Hull tab, which cycles them right
    /// after the hull layers so you can wall a ship in without changing tabs.
    /// Listing them in both places would put the same item under two labels.
    ///
    /// Hull owns no modules, so it answers with nothing -- its own items are
    /// HULL_LAYERS plus HULL_PLATING, resolved in `current_selection`.
    ///
    /// The pre-made weapons (combat::gun_design::WeaponKits) aren't here
    /// either: they're read from disk and come first in their tab, ahead of
    /// these (`BuildingState::kits_here`).
    pub fn items(&self) -> &'static [ModuleType] {
        match self {
            BuildCategory::Hull => &[],
            BuildCategory::Structural => STRUCTURAL_PALETTE,
            BuildCategory::Weapons => WEAPONS_PALETTE,
            other => other
                .to_module_category()
                .map(|c| c.module_types())
                .unwrap_or(&[]),
        }
    }

    pub fn item_count(&self) -> usize {
        match self {
            // Outer, Inner, Hallway, Void, BulkheadDoor, then the angled plating.
            BuildCategory::Hull => HULL_LAYERS.len() + HULL_PLATING.len(),
            other => other.items().len(),
        }
    }
}

/// What's currently selected for placement
#[derive(Clone, Copy, Debug)]
pub enum BuildSelection {
    Hull(HullLayer),
    Module(ModuleType),
}

/// Building mode state with category-based selection
#[derive(Resource)]
pub struct BuildingState {
    pub category_index: usize,
    pub selected_index: usize,
    pub rotation: Rotation,
    pub hull_material: HullMaterial,
    pub is_valid_placement: bool,
    pub placement_reason: Option<String>,
    pub ghost_position: IVec2,
    /// Is the cursor actually over a cell of the ship right now?
    ///
    /// `ghost_position` keeps its last value when the cursor leaves the hull,
    /// and its initial value is (0, 0) -- the ship's own origin, which the
    /// camera centres on. So the placement ghost sat pulsing in the exact
    /// middle of the screen from the moment build mode opened, before the
    /// player had pointed at anything.
    pub cursor_on_grid: bool,
    /// When true, rotation was set by auto-rotate (will be overridden on ghost move).
    /// When false, user manually set rotation with R key.
    pub auto_rotated: bool,
    /// Shift held at the press of a hull drag: the corner a rectangle fill
    /// started from. The rectangle runs to `ghost_position` until release.
    pub hull_rect_anchor: Option<IVec2>,
    /// Which deck build mode is working on: the hull, or the turret decks
    /// above it (building::turret_build).
    pub deck: BuildDeck,
    /// The tab each pre-made weapon (combat::gun_design::WeaponKits, read
    /// from disk) sits in, by its index: guns in Guns, laser units in
    /// Lasers, launchers and the missile-defence pack in Weapons. Each tab
    /// offers its kits first, then its parts, the way From the Depths puts a
    /// working weapon in front of the blocks it's built from.
    pub kit_tabs: Vec<BuildCategory>,
    /// The signature version picked with V (diplomacy::signatures), by id.
    /// It only applies while the selected block is the one it's built on.
    pub signature: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildDeck {
    #[default]
    Hull,
    Turret,
    /// The gun house roof over the turrets' guns (combat::gun_house).
    Roof,
}

const HULL_LAYERS: [HullLayer; 5] = [
    HullLayer::Outer,
    HullLayer::Inner,
    HullLayer::Hallway,
    HullLayer::Void,
    HullLayer::BulkheadDoor,
];

/// Plating that cycles alongside the hull layers. These are Modules rather
/// than HullSegments, but they're hull work: you place them while walling a
/// ship in, not while fitting it out. Keeping them in the Structural module
/// list meant leaving the hull category mid-wall to find them.
const HULL_PLATING: [ModuleType; 2] = [
    ModuleType::AngledArmorPlate,
    ModuleType::AngledHullPlate,
];

/// The Structural tab, in slot order: `ModuleCategory::Structural` minus the
/// two angled plates that ride with the hull layers on the Hull tab.
///
/// This whole category had no tab. `BuildCategory` simply had no Structural
/// variant, so the fifteen modules below -- flat armour plate among them, the
/// plainest piece of defence in the game, worth 40 armour in
/// `building::placement_armor_value` -- could not be built by any means.
/// `structural_palette_matches_module_category` keeps this list and that one
/// from drifting apart again.
/// The Weapons tab: missile launchers and the special weapons, nothing else.
///
/// `ModuleCategory::Weapons` still holds the old single-block guns (Cannon,
/// Railgun, Coilgun, Gatling, the one-block Laser and missile racks) and the
/// bolt-on upgrades that went with them. Guns are built from parts now (the
/// Guns and Lasers tabs) and those blocks only live on as data for older
/// ships, so the shipyard stopped offering them.
const WEAPONS_PALETTE: &[ModuleType] = &[
    ModuleType::LaunchPad,
    ModuleType::RailLauncher,
    ModuleType::VlsSilo,
    ModuleType::PlasmaCaster,
    ModuleType::IonDisruptor,
    ModuleType::EMPPulse,
    ModuleType::TractorBeam,
    ModuleType::MiningDrill,
];

const STRUCTURAL_PALETTE: &[ModuleType] = &[
    ModuleType::ArmorPlate,
    // The two armours that aren't plain plate, each beside its wedge.
    ModuleType::AblativeArmor,
    ModuleType::AngledAblativeArmor,
    ModuleType::ReflectivePlate,
    ModuleType::AngledReflectivePlate,
    // The other angle pieces, beside the plate they're cut from (the 45-degree
    // wedges ride in HULL_PLATING).
    ModuleType::BevelledArmorPlate,
    ModuleType::SlopeArmorPlateLow,
    ModuleType::SlopeArmorPlateHigh,
    ModuleType::SlopeArmorPlateLowMirror,
    ModuleType::SlopeArmorPlateHighMirror,
    ModuleType::HalfArmorPlate,
    ModuleType::CornerArmorPlate,
    ModuleType::StaggeredArmorPlate,
    ModuleType::HullBeam,
    // Filler for bare deck, each with one small use.
    ModuleType::LightFrame,
    ModuleType::SupplyCrates,
    ModuleType::CoolantRun,
    ModuleType::BallastBlock,
    ModuleType::HullCorner,
    ModuleType::Bulkhead,
    ModuleType::PressureFrame,
    ModuleType::AirlockValve,
    ModuleType::AccessHatch,
    ModuleType::ViewPort,
    ModuleType::EmergencyBulkhead,
    ModuleType::FirebreakWall,
    ModuleType::StructuralBrace,
    ModuleType::VibrationDamper,
    ModuleType::ReinforcedJoint,
];

impl Default for BuildingState {
    fn default() -> Self {
        Self {
            category_index: 0,
            selected_index: 0,
            rotation: Rotation::North,
            hull_material: HullMaterial::Steel,
            is_valid_placement: false,
            placement_reason: None,
            ghost_position: IVec2::ZERO,
            cursor_on_grid: false,
            auto_rotated: true,
            hull_rect_anchor: None,
            deck: BuildDeck::Hull,
            kit_tabs: Vec::new(),
            signature: None,
        }
    }
}

impl BuildingState {
    /// The signature picked for `module_type`, if the pick is for that block.
    pub fn signature_for(&self, module_type: ModuleType) -> Option<&'static crate::diplomacy::signatures::SignatureDef> {
        self.signature.and_then(crate::diplomacy::signatures::get).filter(|s| s.base == module_type)
    }

    pub fn current_category(&self) -> BuildCategory {
        BuildCategory::ALL[self.category_index % BuildCategory::ALL.len()]
    }

    pub fn current_selection(&self) -> BuildSelection {
        let cat = self.current_category();
        match cat {
            BuildCategory::Hull => {
                let idx = self.selected_index % (HULL_LAYERS.len() + HULL_PLATING.len());
                match HULL_LAYERS.get(idx) {
                    Some(layer) => BuildSelection::Hull(*layer),
                    None => BuildSelection::Module(HULL_PLATING[idx - HULL_LAYERS.len()]),
                }
            }
            _ => {
                let types = cat.items();
                let kits = self.kits_here();
                if types.is_empty() && kits == 0 {
                    BuildSelection::Module(ModuleType::StandardReactor)
                } else {
                    let idx = self.selected_index % (kits + types.len());
                    match idx.checked_sub(kits) {
                        Some(i) => BuildSelection::Module(types[i]),
                        // A whole pre-made weapon is picked (`premade_index`),
                        // placed by building::gun_stamp; this stands in for
                        // code that wants a block.
                        None => BuildSelection::Module(ModuleType::TurretRingSmall),
                    }
                }
            }
        }
    }

    pub fn selection_name(&self) -> &'static str {
        match self.current_selection() {
            BuildSelection::Hull(layer) => match layer {
                HullLayer::Outer => "Outer Hull",
                HullLayer::Inner => "Inner Hull",
                HullLayer::Hallway => "Hallway",
                HullLayer::Void => "Void Space",
                HullLayer::BulkheadDoor => "Bulkhead Door",
            },
            BuildSelection::Module(mt) => mt.name(),
        }
    }

    pub fn next_category(&mut self) {
        self.category_index = (self.category_index + 1) % BuildCategory::ALL.len();
        self.selected_index = 0;
    }

    /// How many pre-made weapons the current tab opens with.
    pub fn kits_here(&self) -> usize {
        let cat = self.current_category();
        self.kit_tabs.iter().filter(|t| **t == cat).count()
    }

    /// Items the current tab offers: its kits, then its blocks.
    fn items_here(&self) -> usize {
        self.kits_here() + self.current_category().item_count()
    }

    /// The pre-made weapon picked (its index in WeaponKits), when the
    /// selection is on one of the current tab's kits.
    pub fn premade_index(&self) -> Option<usize> {
        let total = self.items_here();
        if total == 0 {
            return None;
        }
        let cat = self.current_category();
        let nth = self.selected_index % total;
        self.kit_tabs.iter().enumerate().filter(|(_, t)| **t == cat).nth(nth).map(|(i, _)| i)
    }

    pub fn next_item(&mut self) {
        let count = self.items_here();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    pub fn prev_item(&mut self) {
        let count = self.items_here();
        if count > 0 {
            if self.selected_index == 0 {
                self.selected_index = count - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

// ============================================================================
// CUSTOMIZATION MODE
// ============================================================================

/// State for customizing modules with sliders and ship-component placement
#[derive(Resource)]
pub struct CustomizationState {
    pub active: bool,
    pub module_type: ModuleType,
    pub properties: HashMap<String, f32>,
    pub preview_stats: CalculatedStats,
}

impl Default for CustomizationState {
    fn default() -> Self {
        Self {
            active: false,
            module_type: ModuleType::HeavyMissile,
            properties: HashMap::new(),
            preview_stats: CalculatedStats::default(),
        }
    }
}

impl CustomizationState {
    /// Start customizing a module type
    pub fn start_customizing(&mut self, module_type: ModuleType) {
        self.active = true;
        self.module_type = module_type;
        self.properties.clear();

        // Set default property values based on module type
        match module_type.category() {
            ModuleCategory::Weapons => {
                self.properties.insert("barrel_length".to_string(), 5.0);
                self.properties.insert("caliber".to_string(), 50.0);
                self.properties.insert("chamber_pressure".to_string(), 100.0);
            }
            ModuleCategory::Propulsion => {
                self.properties.insert("efficiency".to_string(), 1.0);
                self.properties.insert("propeller_count".to_string(), 4.0);
                self.properties.insert("propeller_pitch".to_string(), 1.0);
            }
            ModuleCategory::Power => {
                self.properties.insert("enrichment".to_string(), 1.0);
                self.properties.insert("fuel_rod_count".to_string(), 4.0);
                self.properties.insert("coolant_flow".to_string(), 100.0);
            }
            ModuleCategory::LifeSupport => {
                self.properties.insert("filter_size".to_string(), 1.0);
                self.properties.insert("absorber_efficiency".to_string(), 1.0);
            }
            _ => {}
        }

        self.recalculate_preview();
    }

    /// Update a property value
    pub fn update_property(&mut self, key: &str, value: f32) {
        self.properties.insert(key.to_string(), value);
        self.recalculate_preview();
    }

    /// Recalculate preview stats from current properties
    pub fn recalculate_preview(&mut self) {
        use crate::building::StatCalculator;

        // Build ship-components from properties
        let subcomponents = self.build_subcomponents();

        // Get base stats (would need registry, but for now use defaults)
        let base_stats = match self.module_type.category() {
            ModuleCategory::Weapons => CalculatedStats {
                weapon: Some(WeaponStats {
                    damage: 30.0,
                    range: 400.0,
                    fire_rate: 0.5,
                    max_ammo: 20,
                    power_cost: 25.0,
                }),
                ..Default::default()
            },
            ModuleCategory::Propulsion => CalculatedStats {
                engine: Some(EngineStats {
                    thrust: 50.0,
                    fuel_efficiency: 1.0,
                    noise: 10.0,
                }),
                ..Default::default()
            },
            ModuleCategory::Power => CalculatedStats {
                reactor: Some(ReactorStats {
                    power_output: 60.0,
                    heat_generation: 80.0,
                    explosion_risk: 0.1,
                }),
                ..Default::default()
            },
            ModuleCategory::LifeSupport => CalculatedStats {
                life_support: Some(LifeSupportStats {
                    o2_generation: 10.0,
                    co2_filtering: 8.0,
                    crew_capacity: 5,
                }),
                ..Default::default()
            },
            _ => CalculatedStats::default(),
        };

        self.preview_stats = StatCalculator::calculate_stats(
            self.module_type,
            &subcomponents,
            &base_stats,
        );
    }

    /// Build ship-components from current property values
    pub fn build_subcomponents(&self) -> Vec<SubComponentType> {
        use crate::components::*;

        let mut subcomponents = Vec::new();

        match self.module_type.category() {
            ModuleCategory::Weapons => {
                if let (Some(&length), Some(&caliber)) = (
                    self.properties.get("barrel_length"),
                    self.properties.get("caliber"),
                ) {
                    subcomponents.push(SubComponentType::BarrelComponent {
                        length,
                        caliber,
                        thickness: 5.0,
                    });
                }

                if let Some(&pressure) = self.properties.get("chamber_pressure") {
                    subcomponents.push(SubComponentType::ChamberComponent {
                        volume: 50.0,
                        pressure,
                    });
                }

                subcomponents.push(SubComponentType::LoaderComponent {
                    mechanism: LoaderMechanism::Automatic,
                    speed: 1.0,
                });
            }
            ModuleCategory::Propulsion => {
                if let Some(&efficiency) = self.properties.get("efficiency") {
                    subcomponents.push(SubComponentType::CombustionChamber { efficiency });
                }

                if let (Some(&pitch), Some(&count)) = (
                    self.properties.get("propeller_pitch"),
                    self.properties.get("propeller_count"),
                ) {
                    subcomponents.push(SubComponentType::PropellerBlade {
                        pitch,
                        count: count as u32,
                    });
                }
            }
            ModuleCategory::Power => {
                if let (Some(&enrichment), Some(&count)) = (
                    self.properties.get("enrichment"),
                    self.properties.get("fuel_rod_count"),
                ) {
                    subcomponents.push(SubComponentType::FuelRod {
                        enrichment,
                        count: count as u32,
                    });
                }

                if let Some(&flow) = self.properties.get("coolant_flow") {
                    subcomponents.push(SubComponentType::Coolant { flow_rate: flow });
                }
            }
            ModuleCategory::LifeSupport => {
                if let Some(&size) = self.properties.get("filter_size") {
                    subcomponents.push(SubComponentType::OxygenScrubber { filter_size: size });
                }

                if let Some(&efficiency) = self.properties.get("absorber_efficiency") {
                    subcomponents.push(SubComponentType::CO2Absorber { efficiency });
                }
            }
            _ => {}
        }

        subcomponents
    }

    /// Cancel customization
    pub fn cancel(&mut self) {
        self.active = false;
        self.properties.clear();
    }
}

// ============================================================================
// COMPONENT PLACEMENT STATE
// ============================================================================

/// Resource for component piece placement mode
#[derive(Resource)]
pub struct ComponentPlacementState {
    pub active: bool,
    pub module_type: ModuleType,
    pub selected_piece_type: Option<ComponentPieceType>,
    pub placed_pieces: Vec<ComponentPiece>,
    pub ghost_position: Option<IVec2>,
}

impl Default for ComponentPlacementState {
    fn default() -> Self {
        Self {
            active: false,
            module_type: ModuleType::HeavyMissile,
            selected_piece_type: None,
            placed_pieces: Vec::new(),
            ghost_position: None,
        }
    }
}

impl ComponentPlacementState {
    /// Start component placement mode
    pub fn start_placing(&mut self, module_type: ModuleType) {
        self.active = true;
        self.module_type = module_type;
        self.selected_piece_type = None;
        self.placed_pieces.clear();
        self.ghost_position = None;
    }

    /// Select a piece type to place
    pub fn select_piece(&mut self, piece_type: ComponentPieceType) {
        self.selected_piece_type = Some(piece_type);
    }

    /// Place a component piece at a position
    pub fn place_piece(&mut self, position: IVec2, piece_type: ComponentPieceType) -> bool {
        // Check if position is already occupied
        if self.placed_pieces.iter().any(|p| {
            let end_pos = p.internal_position + p.size;
            position.x >= p.internal_position.x && position.x < end_pos.x
                && position.y >= p.internal_position.y && position.y < end_pos.y
        }) {
            return false;
        }

        // Check if within grid bounds (4x4)
        if position.x < 0 || position.x >= 4 || position.y < 0 || position.y >= 4 {
            return false;
        }

        // Get piece size
        let size = match piece_type {
            ComponentPieceType::Barrel | ComponentPieceType::FuelTank => IVec2::new(2, 1),
            _ => IVec2::new(1, 1),
        };

        // Check if piece fits
        if position.x + size.x > 4 || position.y + size.y > 4 {
            return false;
        }

        // Validate placement rules
        if !self.validate_placement(&piece_type, position) {
            return false;
        }

        // Place the piece
        let piece = ComponentPiece {
            piece_type: piece_type.clone(),
            internal_position: position,
            size,
            properties: std::collections::HashMap::new(),
        };

        self.placed_pieces.push(piece);
        true
    }

    /// Validate placement rules (barrels at front, chambers behind, etc.)
    fn validate_placement(&self, piece_type: &ComponentPieceType, position: IVec2) -> bool {
        match piece_type {
            ComponentPieceType::Barrel => {
                // Barrels must be at front (leftmost columns 0-1)
                position.x <= 1
            }
            ComponentPieceType::Chamber | ComponentPieceType::CombustionChamber => {
                // Chambers must be in middle (columns 1-2)
                position.x >= 1 && position.x <= 2
            }
            ComponentPieceType::Loader => {
                // Loaders must be behind chambers (columns 2-3)
                position.x >= 2
            }
            ComponentPieceType::FuelRod => {
                // Fuel rods must be in center (columns 1-2, rows 1-2)
                position.x >= 1 && position.x <= 2 && position.y >= 1 && position.y <= 2
            }
            ComponentPieceType::CoolantPipe => {
                // Coolant must be adjacent to fuel rods
                self.is_adjacent_to_piece(position, ComponentPieceType::FuelRod)
            }
            _ => true, // No special rules for other pieces
        }
    }

    /// Check if a position is adjacent to a specific piece type
    fn is_adjacent_to_piece(&self, position: IVec2, piece_type: ComponentPieceType) -> bool {
        let adjacent_offsets = [
            IVec2::new(-1, 0),
            IVec2::new(1, 0),
            IVec2::new(0, -1),
            IVec2::new(0, 1),
        ];

        for offset in adjacent_offsets.iter() {
            let adj_pos = position + *offset;
            if self.placed_pieces.iter().any(|p| {
                p.piece_type == piece_type && p.internal_position == adj_pos
            }) {
                return true;
            }
        }

        false
    }

    /// Remove a piece at a position
    pub fn remove_piece(&mut self, position: IVec2) -> bool {
        if let Some(index) = self.placed_pieces.iter().position(|p| {
            let end_pos = p.internal_position + p.size;
            position.x >= p.internal_position.x && position.x < end_pos.x
                && position.y >= p.internal_position.y && position.y < end_pos.y
        }) {
            self.placed_pieces.remove(index);
            true
        } else {
            false
        }
    }

    /// Finalize and return placed pieces
    pub fn finalize(&mut self) -> Vec<ComponentPiece> {
        self.active = false;
        std::mem::take(&mut self.placed_pieces)
    }

    /// Cancel placement
    pub fn cancel(&mut self) {
        self.active = false;
        self.placed_pieces.clear();
        self.selected_piece_type = None;
    }

    /// Get all pieces connected to a piece at a position (flood-fill)
    pub fn get_connected_pieces(&self, start_position: IVec2) -> Vec<usize> {
        let mut connected = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        // Find the piece at start_position
        let start_piece_idx = match self.placed_pieces.iter().position(|p| {
            let end_pos = p.internal_position + p.size;
            start_position.x >= p.internal_position.x && start_position.x < end_pos.x
                && start_position.y >= p.internal_position.y && start_position.y < end_pos.y
        }) {
            Some(idx) => idx,
            None => return connected,
        };

        let start_piece = &self.placed_pieces[start_piece_idx];
        let target_type = start_piece.piece_type.clone();

        queue.push_back(start_piece_idx);
        visited.insert(start_piece_idx);

        while let Some(current_idx) = queue.pop_front() {
            connected.push(current_idx);
            let current_piece = &self.placed_pieces[current_idx];

            // Check all adjacent positions
            let adjacent_offsets = [
                IVec2::new(-1, 0),
                IVec2::new(1, 0),
                IVec2::new(0, -1),
                IVec2::new(0, 1),
            ];

            for offset in adjacent_offsets.iter() {
                let check_pos = current_piece.internal_position + *offset;

                // Find piece at this position
                if let Some(neighbor_idx) = self.placed_pieces.iter().enumerate().position(|(idx, p)| {
                    p.piece_type == target_type
                        && p.internal_position == check_pos
                        && !visited.contains(&idx)
                }) {
                    if !visited.contains(&neighbor_idx) {
                        visited.insert(neighbor_idx);
                        queue.push_back(neighbor_idx);
                    }
                }
            }
        }

        connected
    }
}

/// Resource for piece customization context menu
#[derive(Resource)]
pub struct PieceCustomizationState {
    pub active: bool,
    pub target_position: IVec2,
    pub connected_pieces: Vec<usize>,
    pub customize_group: bool,
    pub properties: std::collections::HashMap<String, f32>,
}

impl Default for PieceCustomizationState {
    fn default() -> Self {
        Self {
            active: false,
            target_position: IVec2::ZERO,
            connected_pieces: Vec::new(),
            customize_group: false,
            properties: std::collections::HashMap::new(),
        }
    }
}

impl PieceCustomizationState {
    /// Start customizing a piece or group
    pub fn start_customizing(&mut self, position: IVec2, connected_pieces: Vec<usize>, customize_group: bool) {
        self.active = true;
        self.target_position = position;
        self.connected_pieces = connected_pieces;
        self.customize_group = customize_group;
        self.properties.clear();
    }

    /// Apply customization and return the modified properties
    pub fn apply(&mut self) -> std::collections::HashMap<String, f32> {
        self.active = false;
        std::mem::take(&mut self.properties)
    }

    /// Cancel customization
    pub fn cancel(&mut self) {
        self.active = false;
        self.properties.clear();
    }
}

// ============================================================================
// SAVE DATA
// ============================================================================

/// Serialized crew data
#[derive(Serialize, Deserialize, Clone)]
pub struct CrewSaveData {
    pub name: String,
    pub health: f32,
    pub max_health: f32,
    pub oxygen: f32,
    pub morale: f32,
    #[serde(default)]
    pub assigned_module_grid: Option<IVec2>,
    /// The player's standing order for this hand. Defaulted so saves written
    /// before duties existed load as Auto — which is what they were.
    #[serde(default)]
    pub duty: crate::components::CrewDuty,
}

/// Hull segment with material info for save/load
#[derive(Serialize, Deserialize, Clone)]
pub struct HullSaveData {
    pub grid_position: IVec2,
    pub health: f32,
    pub max_health: f32,
    pub radiation_shielding: f32,
    pub material: HullMaterial,
    pub hull_layer: HullLayer,
}

/// Save file header for slot display
#[derive(Serialize, Deserialize, Clone)]
pub struct SaveSlotInfo {
    pub slot: u32,
    pub timestamp: String,
    pub depth: f32,
    pub play_time: f32,
    pub hull_integrity: f32,
    /// The galaxy's seed: the title screen paints this run's nebula from it.
    /// 0 on older saves.
    #[serde(default)]
    pub seed: u64,
    /// How far the Cascade had got (narrative::CascadeState::level), for the
    /// title screen's drained nebula. 0 on older saves.
    #[serde(default)]
    pub cascade: f32,
    /// Systems gone dark, of all of them: worked out when the save is read
    /// (meta::read_slot_info), not trusted from the file.
    #[serde(default)]
    pub dark_systems: u32,
    #[serde(default)]
    pub total_systems: u32,
}

/// Per-system galaxy state that a save must carry. Everything else about a
/// system (name, positions, seed, faction, danger) regenerates deterministically
/// from `SaveData::galaxy_seed` via celestial::galaxy::generate_galaxy_map, so
/// only the mutable, player-driven bits are persisted here.
#[derive(Clone, Serialize, Deserialize)]
pub struct SystemSaveData {
    pub id: u32,
    /// SystemDiscovery encoded (0=Unknown,1=Located,2=Visited); see as_u8/from_u8.
    pub discovery: u8,
    pub last_updated: f64,
    pub resource_fraction_remaining: f32,
    /// Derelicts/anomalies already stripped here, by spawn slot (see
    /// celestial::resources::LootedPois). Empty on older saves.
    #[serde(default)]
    pub looted_pois: Vec<u32>,
    /// Who holds it now, and whether it's their capital: borders move
    /// (celestial::territory). None on older saves: the generated owner stands.
    #[serde(default)]
    pub held: Option<HeldBy>,
    /// How far the Cascade has it (narrative::front). 0 on older saves.
    #[serde(default)]
    pub dark: f32,
    /// What you last heard of it (narrative::news). None on older saves: you
    /// know the truth.
    #[serde(default)]
    pub heard: Option<crate::narrative::news::Heard>,
}

/// A system's owner as saved.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HeldBy {
    pub owner: Option<crate::ai_ship::components::AiShipType>,
    pub capital: bool,
}

#[derive(Resource, Serialize, Deserialize)]
pub struct SaveData {
    pub version: u32,
    /// Haven's Raft: (berth, design) for every ship moored there
    /// (world::raft_life). Empty on older saves: Haven as a new run lays it.
    #[serde(default)]
    pub haven_berths: Vec<(usize, String)>,
    pub slot_info: SaveSlotInfo,
    pub ship: ShipBlueprint,
    pub hull_segments: Vec<HullSaveData>,
    pub crew: Vec<CrewSaveData>,
    pub inventory: Inventory,
    pub currency: Currency,
    pub unlocks: Unlocks,
    pub statistics: Statistics,
    pub discovered_locations: DiscoveredLocations,
    pub position: Vec2,
    pub current_depth: f32,
    pub world_seed: u64,
    pub was_exploring: bool,
    /// Current star system ID (for celestial state)
    #[serde(default)]
    pub current_system_id: u32,
    /// Galaxy seed for reproducible system generation
    #[serde(default)]
    pub galaxy_seed: u64,
    /// Per-system mutable state (discovery/depletion). Empty on legacy saves —
    /// the galaxy then regenerates fresh from `galaxy_seed`.
    #[serde(default)]
    pub galaxy_systems: Vec<SystemSaveData>,
    /// Everyone ever put out of an airlock, wherever they have drifted to.
    /// Never pruned — see crew::burial. Empty on legacy saves.
    #[serde(default)]
    pub drifting_dead: Vec<crate::crew::burial::DriftingBody>,
    /// Where in the galaxy the player was, when that was nowhere: set only for
    /// a save made in uncharted space after a blind jump. `current_system_id`
    /// can't say that -- it read None as 0, so a save in the void loaded at
    /// Haven. None on older saves, which then load into `current_system_id`.
    #[serde(default)]
    pub uncharted_galaxy_pos: Option<Vec2>,
    /// Fuel in the tank. None on saves made before it was kept; those load
    /// with a full tank, which is what loading one from the menu always gave.
    #[serde(default)]
    pub fuel: Option<f32>,
    /// Contract boards and accepted contracts. Empty on older saves.
    #[serde(default)]
    pub contracts: crate::contracts::ContractState,
    /// Standing with the contract factions. The starting standing on older saves.
    #[serde(default)]
    pub reputation: crate::contracts::FactionReputation,
    /// Your standing with each AI faction (diplomacy). None on older saves,
    /// which load with the lore's starting standing.
    #[serde(default)]
    pub standing: Option<crate::diplomacy::FactionStanding>,
    /// Every faction ship in the galaxy: class, position, health, damage,
    /// alive or dead. None on older saves, which roll their fleets afresh.
    #[serde(default)]
    pub world: Option<crate::ai_ship::components::SavedWorld>,
    /// Systems whose faction station you destroyed.
    #[serde(default)]
    pub destroyed_stations: Vec<u32>,
    /// The border war's standing: every contested system's shares.
    #[serde(default)]
    pub border_war: Option<crate::celestial::territory::BorderWar>,
    /// The people you've met at faction starports, and what they think of you.
    #[serde(default)]
    pub contacts: Option<crate::diplomacy::contacts::Contacts>,
    /// Work you've taken from contacts.
    #[serde(default)]
    pub jobs: Option<crate::diplomacy::jobs::Jobs>,
    /// The orbital clock (`GalaxyState::galaxy_time`): every planet's place
    /// on its orbit is read off it. 0 on older saves -- where every run's
    /// clock starts, and so where loading one always put the planets.
    #[serde(default)]
    pub galaxy_time: f64,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: 1,
            haven_berths: Vec::new(),
            slot_info: SaveSlotInfo {
                slot: 0,
                timestamp: String::new(),
                depth: 0.0,
                play_time: 0.0,
                hull_integrity: 1.0,
                seed: 0,
                cascade: 0.0,
                dark_systems: 0,
                total_systems: 0,
            },
            ship: ShipBlueprint::default(),
            hull_segments: Vec::new(),
            crew: Vec::new(),
            inventory: Inventory::default(),
            currency: Currency::default(),
            unlocks: Unlocks::default(),
            statistics: Statistics::default(),
            discovered_locations: DiscoveredLocations::default(),
            position: Vec2::ZERO,
            current_depth: 0.0,
            world_seed: 0,
            was_exploring: false,
            current_system_id: 0,
            galaxy_seed: 0,
            galaxy_systems: Vec::new(),
            drifting_dead: Vec::new(),
            uncharted_galaxy_pos: None,
            fuel: None,
            contracts: crate::contracts::ContractState::default(),
            reputation: crate::contracts::FactionReputation::default(),
            standing: None,
            world: None,
            destroyed_stations: Vec::new(),
            border_war: None,
            contacts: None,
            jobs: None,
            galaxy_time: 0.0,
        }
    }
}

/// Resource tracking auto-save timer
#[derive(Resource)]
pub struct AutoSaveTimer {
    pub timer: Timer,
    pub enabled: bool,
}

impl Default for AutoSaveTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(120.0, TimerMode::Repeating),
            enabled: true,
        }
    }
}

/// Resource for the save/load menu overlay
#[derive(Resource, Default)]
pub struct SaveLoadMenuState {
    pub is_saving: bool,
    pub selected_slot: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_state_defaults() {
        let fuel = FuelState::default();
        // The invariant that actually matters: a new tank starts full.
        assert_eq!(fuel.current_fuel, fuel.max_fuel);
        // The numbers are deliberate — see the Default impl. This asserted the
        // old 500 / 1.0 long after they were raised, so the suite has simply
        // been red ever since rather than catching anything.
        assert!((fuel.max_fuel - 1500.0).abs() < f32::EPSILON);
        assert!((fuel.fuel_consumption_rate - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn game_config_defaults() {
        let config = GameConfig::default();
        assert!(config.radiation_damage_multiplier > 0.0);
        assert!(config.radiation_per_unit > 0.0);
        assert!(config.base_oxygen_consumption_per_crew > 0.0);
        assert!(config.suffocation_damage_rate > 0.0);
        assert!(config.base_ship_speed > 0.0);
    }

    #[test]
    fn inventory_add_and_remove_items() {
        let mut inv = Inventory {
            items: HashMap::new(),
            max_capacity: 100.0,
            current_weight: 0.0,
        };

        // Add items
        assert!(inv.add_item(ItemType::ScrapMetal, 2));
        assert_eq!(inv.items.get(&ItemType::ScrapMetal), Some(&2));
        assert!((inv.current_weight - 10.0).abs() < f32::EPSILON); // 5.0 * 2

        // Remove one
        assert!(inv.remove_item(ItemType::ScrapMetal, 1));
        assert_eq!(inv.items.get(&ItemType::ScrapMetal), Some(&1));

        // Remove more than available should fail
        assert!(!inv.remove_item(ItemType::ScrapMetal, 5));
    }

    #[test]
    fn inventory_respects_capacity() {
        let mut inv = Inventory {
            items: HashMap::new(),
            max_capacity: 10.0,
            current_weight: 0.0,
        };

        // ScrapMetal weighs 5.0 each, so 2 fit but 3 don't
        assert!(inv.add_item(ItemType::ScrapMetal, 2));
        assert!(!inv.add_item(ItemType::ScrapMetal, 1));
    }

    #[test]
    fn building_state_category_cycling() {
        let mut state = BuildingState::default();
        let initial = state.current_category();
        assert_eq!(initial, BuildCategory::Hull);

        state.next_category();
        assert_eq!(state.current_category(), BuildCategory::Power);

        // Cycle through all categories
        for _ in 0..BuildCategory::ALL.len() - 1 {
            state.next_category();
        }
        assert_eq!(state.current_category(), BuildCategory::Hull);
    }

    #[test]
    fn building_state_item_cycling() {
        let mut state = BuildingState::default();
        let last = BuildCategory::Hull.item_count() - 1;
        state.next_item();
        assert_eq!(state.selected_index, 1);
        state.prev_item();
        assert_eq!(state.selected_index, 0);
        state.prev_item();
        assert_eq!(state.selected_index, last); // wraps around
    }

    /// The Hull category cycles four layers and then the angled plating, so
    /// walling a ship in and cornering it are the same trip through `[`/`]`.
    #[test]
    fn hull_category_cycles_layers_then_plating() {
        let mut state = BuildingState::default();
        assert_eq!(state.current_category(), BuildCategory::Hull);

        let mut seen = Vec::new();
        for _ in 0..BuildCategory::Hull.item_count() {
            seen.push(state.current_selection());
            state.next_item();
        }

        assert!(matches!(seen[0], BuildSelection::Hull(HullLayer::Outer)));
        assert!(matches!(seen[2], BuildSelection::Hull(HullLayer::Hallway)));
        assert!(matches!(seen[4], BuildSelection::Hull(HullLayer::BulkheadDoor)));
        assert!(matches!(seen[5], BuildSelection::Module(ModuleType::AngledArmorPlate)));
        assert!(matches!(seen[6], BuildSelection::Module(ModuleType::AngledHullPlate)));

        // Back to the start after a full lap.
        assert!(matches!(state.current_selection(), BuildSelection::Hull(HullLayer::Outer)));
    }

    /// The Structural tab and the Structural module category must stay in
    /// step, differing only by the two plates the Hull tab borrows.
    ///
    /// Two hand-written lists in two files is exactly the arrangement that
    /// hid an entire category from the build menu in the first place.
    #[test]
    fn structural_palette_matches_module_category() {
        let mut from_tab: Vec<&str> = STRUCTURAL_PALETTE
            .iter()
            .chain(HULL_PLATING.iter())
            .map(|m| m.name())
            .collect();
        let mut from_category: Vec<&str> = ModuleCategory::Structural
            .module_types()
            .iter()
            .map(|m| m.name())
            .collect();
        from_tab.sort_unstable();
        from_category.sort_unstable();
        assert_eq!(
            from_tab, from_category,
            "STRUCTURAL_PALETTE plus HULL_PLATING must cover the Structural \
             category exactly -- anything only on the right is buildable \
             nowhere, anything only on the left is offered twice"
        );
    }

    /// Guns are built from parts: the old one-block guns and their bolt-on
    /// upgrades stay as data for older ships but no tab offers them.
    #[test]
    fn no_tab_offers_the_old_single_block_guns() {
        let old = [
            ModuleType::Cannon, ModuleType::Railgun, ModuleType::Coilgun, ModuleType::Gatling,
            ModuleType::Laser, ModuleType::HeavyMissile, ModuleType::GuidedMissile, ModuleType::ClusterRocket,
            ModuleType::AmmoAutoloader, ModuleType::BarrelExtension, ModuleType::AmmoFeedUnit,
            ModuleType::CoolingJacket, ModuleType::OverchargeCapacitor, ModuleType::BoreEvacuator,
            ModuleType::MagneticAccelerator, ModuleType::FocusingArray, ModuleType::WarheadBay,
        ];
        for cat in BuildCategory::ALL {
            for mt in cat.items() {
                assert!(!old.contains(mt), "the {} tab still offers {:?}", cat.name(), mt);
            }
        }
        for mt in [ModuleType::LaunchPad, ModuleType::RailLauncher, ModuleType::VlsSilo, ModuleType::PlasmaCaster] {
            assert!(BuildCategory::Weapons.items().contains(&mt), "{:?} went missing", mt);
        }
    }

    /// A tab's pre-made weapons come first, then its blocks, and only its own
    /// kits count: Guns doesn't offer the laser unit sitting between them.
    #[test]
    fn a_tab_offers_its_kits_before_its_blocks() {
        let guns = BuildCategory::ALL.iter().position(|c| *c == BuildCategory::Guns).unwrap();
        let state = |i| BuildingState {
            category_index: guns,
            selected_index: i,
            kit_tabs: vec![BuildCategory::Guns, BuildCategory::Lasers, BuildCategory::Guns],
            ..Default::default()
        };
        assert_eq!(state(0).premade_index(), Some(0));
        assert_eq!(state(1).premade_index(), Some(2));
        assert_eq!(state(2).premade_index(), None);
        let first_part = BuildCategory::Guns.items()[0];
        assert!(matches!(state(2).current_selection(), BuildSelection::Module(mt) if mt == first_part));
        // Cycling wraps through kits and blocks together.
        let total = 2 + BuildCategory::Guns.items().len();
        assert_eq!(state(total).premade_index(), Some(0));
    }

    /// Every tab must offer something, or players hit a dead panel.
    #[test]
    fn every_build_tab_has_items() {
        for cat in BuildCategory::ALL {
            assert!(
                cat.item_count() > 0,
                "{} tab is empty",
                cat.name()
            );
        }
    }

    /// Slot widgets are spawned from `items()` and clicks resolve through
    /// `current_selection`, so the two must index the same list.
    #[test]
    fn selection_walks_every_slot_of_every_tab() {
        for (ci, cat) in BuildCategory::ALL.iter().enumerate() {
            let mut seen = Vec::new();
            for i in 0..cat.item_count() {
                let state = BuildingState {
                    category_index: ci,
                    selected_index: i,
                    ..Default::default()
                };
                seen.push(state.selection_name());
            }
            let count = seen.len();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(
                seen.len(),
                count,
                "{} tab has {} slots but they resolve to {} distinct items -- \
                 some slot places the same thing as another, or a label and \
                 its block have drifted apart",
                cat.name(),
                count,
                seen.len()
            );
        }
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;

    fn with_new_fields() -> SaveData {
        let mut save = SaveData::default();
        save.galaxy_seed = 9;
        save.galaxy_systems = vec![SystemSaveData {
            id: 4,
            discovery: 2,
            last_updated: 12.0,
            resource_fraction_remaining: 0.5,
            looted_pois: vec![0, 2],
            held: Some(HeldBy { owner: Some(crate::ai_ship::components::AiShipType::TerranHegemony), capital: true }),
            dark: 0.5,
            heard: Some(crate::narrative::news::Heard { dark: 0.25, at: 50.0, ..Default::default() }),
        }];
        save.uncharted_galaxy_pos = Some(Vec2::new(1_200_000.0, -40_000.0));
        save.fuel = Some(321.0);
        save.contracts.next_id = 17;
        save.reputation.navy = 42.0;
        save.galaxy_time = 5_432.25;
        save
    }

    /// Everything added to the save comes back out of it.
    #[test]
    fn the_new_fields_round_trip() {
        let json = serde_json::to_string(&with_new_fields()).unwrap();
        let back: SaveData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.galaxy_systems[0].looted_pois, vec![0, 2]);
        assert_eq!(back.uncharted_galaxy_pos, Some(Vec2::new(1_200_000.0, -40_000.0)));
        assert_eq!(back.fuel, Some(321.0));
        assert_eq!(back.contracts.next_id, 17);
        assert_eq!(back.reputation.navy, 42.0);
        assert_eq!(back.galaxy_time, 5_432.25);
        assert_eq!(back.galaxy_systems[0].heard.as_ref().map(|h| (h.dark, h.at)), Some((0.25, 50.0)));
    }

    /// A save written before those fields existed still loads, and reads as
    /// "nothing recorded" rather than as something it never said: in a
    /// system (not the void), no fuel figure, no contracts, starting standing,
    /// the orbital clock where every run starts it.
    #[test]
    fn a_save_from_before_them_still_loads() {
        let mut json = serde_json::to_value(with_new_fields()).unwrap();
        let obj = json.as_object_mut().unwrap();
        for key in ["uncharted_galaxy_pos", "fuel", "contracts", "reputation", "standing", "world", "destroyed_stations", "galaxy_time"] {
            assert!(obj.remove(key).is_some(), "{key} is not in the save under that name");
        }
        obj["galaxy_systems"][0].as_object_mut().unwrap().remove("looted_pois");
        obj["galaxy_systems"][0].as_object_mut().unwrap().remove("heard");

        let old: SaveData = serde_json::from_value(json).unwrap();
        assert!(old.galaxy_systems[0].looted_pois.is_empty());
        assert!(old.galaxy_systems[0].heard.is_none(), "an old save knows the truth (meta::load)");
        assert_eq!(old.uncharted_galaxy_pos, None);
        assert_eq!(old.fuel, None);
        assert!(old.contracts.active_contracts.is_empty());
        assert_eq!(old.contracts.next_id, 0);
        assert_eq!(old.reputation.navy, crate::contracts::FactionReputation::default().navy);
        assert_eq!(old.galaxy_time, 0.0);
    }
}
