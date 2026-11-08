// Event fields are part of the data contract — readers may not consume all fields yet.
#![allow(dead_code)]

use bevy::prelude::*;
use crate::components::{ModuleType, Rotation, HullLayer, HullMaterial, SubComponentType, ExplosiveType};
use crate::ai_ship::components::AiShipType;
use crate::resources::ItemType;

// ============================================================================
// SUBMARINE EVENTS
// ============================================================================

/// Fired when ship takes damage
#[derive(Message)]
pub struct ShipDamaged {
    pub source: DamageSource,
    pub amount: f32,
    pub position: Option<Vec2>,
    pub direction: Option<Vec2>,  // Normalized attack direction
    /// The blocks have already taken this. Gunfire is resolved block by block
    /// where it lands (new_projectiles::check_projectile_hits), the same way
    /// for every ship; the event is then only for what reacts to being hit --
    /// camera shake, audio, damage arrows, the death screen -- and
    /// process_ship_damage must not apply it a second time.
    pub applied: bool,
}

#[derive(Clone, Debug)]
pub enum DamageSource {
    Radiation,
    Creature(Entity),
    Collision,
    Explosion,
    Fire,
    /// Another ship's guns.
    Gunfire,
}

/// Fired whenever a weapon actually discharges (player or AI) — audio/vfx hook.
/// Continuous weapons (laser) write this every frame they're beaming; the
/// audio system rate-limits per weapon type.
#[derive(Message)]
pub struct WeaponFired {
    pub weapon_type: ModuleType,
    pub position: Vec2,
    pub from_player: bool,
}

/// Fired when a hull segment is breached
#[derive(Message)]
pub struct HullBreached {
    pub segment: Entity,
    pub severity: f32,  // 0.0 to 1.0
}

/// Fired when a module is damaged
#[derive(Message)]
pub struct ModuleDamaged {
    pub module: Entity,
    pub amount: f32,
}

/// Fired when a module is destroyed
#[derive(Message)]
pub struct ModuleDestroyed {
    pub module: Entity,
}

/// Fired when a room depressurizes (air escaping through hull breach)
#[derive(Message)]
pub struct RoomDepressurized {
    pub room_id: usize,
    pub severity: f32,
}

/// Fired when power state changes significantly
#[derive(Message)]
pub struct PowerStateChanged {
    pub new_balance: f32,
    pub is_critical: bool,
}

/// Fired when oxygen state changes significantly
#[derive(Message)]
pub struct OxygenStateChanged {
    pub new_level: f32,
    pub is_critical: bool,
}

/// Fired when depth changes zones
#[derive(Message)]
pub struct DepthZoneChanged {
    pub new_depth: f32,
    pub new_zone: crate::components::ZoneType,
}

// ============================================================================
// BUILDING EVENTS
// ============================================================================

/// Request to place a module (uses ModuleType enum instead of String)
#[derive(Message)]
pub struct PlaceModuleRequest {
    pub module_type: ModuleType,
    pub grid_position: IVec2,
    pub rotation: Rotation,
    pub custom_name: Option<String>,
    pub subcomponents: Option<Vec<SubComponentType>>,
    /// Per-module design state (tuning, fire group, ammo) restored on spawn.
    /// None = registry defaults. See building::blueprint::ModuleExtras.
    pub extras: Option<crate::building::blueprint::ModuleExtras>,
    /// If true, skip cost deduction (used by blueprint loading)
    pub free: bool,
}

/// Request to place a hull segment
#[derive(Message)]
pub struct PlaceHullRequest {
    pub layer: HullLayer,
    pub material: HullMaterial,
    pub grid_position: IVec2,
    /// If true, skip cost deduction (used by blueprint loading)
    pub free: bool,
}

/// Module was successfully placed
#[derive(Message)]
pub struct ModulePlaced {
    pub module: Entity,
    pub module_type: ModuleType,
    pub grid_position: IVec2,
}

/// Request to remove a module
#[derive(Message)]
pub struct RemoveModuleRequest {
    pub module: Entity,
}

/// Request to remove a hull segment (build mode deletion)
#[derive(Message)]
pub struct RemoveHullRequest {
    pub hull: Entity,
}

/// Module was removed
#[derive(Message)]
pub struct ModuleRemoved {
    pub module_type: ModuleType,
    pub grid_position: IVec2,
}

// ============================================================================
// CREW EVENTS
// ============================================================================

/// Crew member took damage
#[derive(Message)]
pub struct CrewDamaged {
    pub crew: Entity,
    pub amount: f32,
    pub source: CrewDamageSource,
}

#[derive(Clone, Debug)]
pub enum CrewDamageSource {
    Suffocation,
    Decompression,
    Fire,
    Creature,
    Explosion,
    Boarders,
    /// Nobody claimed it. A death still gets announced — losing a hand and
    /// not being told is worse than being told imprecisely.
    Unknown,
}

impl CrewDamageSource {
    /// How the cause reads in a notification. The derived `Debug` was going
    /// straight into player-facing text as "Cause: Explosion".
    pub fn describe(&self) -> &'static str {
        match self {
            CrewDamageSource::Suffocation => "asphyxiation",
            CrewDamageSource::Decompression => "decompression",
            CrewDamageSource::Fire => "fire",
            CrewDamageSource::Creature => "hostile boarders",
            CrewDamageSource::Explosion => "blast trauma",
            CrewDamageSource::Boarders => "parasites",
            CrewDamageSource::Unknown => "unknown causes",
        }
    }
}

/// Crew member died
#[derive(Message)]
pub struct CrewDied {
    pub crew: Entity,
    pub name: String,
    pub cause: CrewDamageSource,
}

/// Staffing priority changed on a module
#[derive(Message)]
pub struct StaffingPriorityChanged {
    pub module: Entity,
    pub new_priority: u8,
}

// ============================================================================
// CREATURE EVENTS
// ============================================================================

/// Creature spotted the ship
#[derive(Message)]
pub struct CreatureSpotted {
    pub creature: Entity,
    pub creature_type: crate::components::CreatureType,
    pub distance: f32,
}

/// Creature is attacking
#[derive(Message)]
pub struct CreatureAttacking {
    pub creature: Entity,
    pub target: Entity,
}

/// Creature was killed
#[derive(Message)]
pub struct CreatureKilled {
    pub creature: Entity,
    pub creature_type: crate::components::CreatureType,
    pub loot: Vec<ItemType>,
}

// ============================================================================
// ECOSYSTEM EVENTS
// ============================================================================

/// A creature ate another creature
#[derive(Message)]
pub struct CreatureAteCreature {
    pub predator: Entity,
    pub predator_type: crate::components::CreatureType,
    pub prey_type: crate::components::CreatureType,
    pub position: Vec2,
}

/// A creature ate from a corpse
#[derive(Message)]
pub struct CreatureAteCorpse {
    pub creature: Entity,
    pub creature_type: crate::components::CreatureType,
    pub corpse_type: crate::components::CreatureType,
}

/// Type of ecosystem cascade
#[derive(Clone, Debug)]
pub enum CascadeType {
    PredatorVoid,
    ScavengerSwarm,
    PreyBoom,
    TerritoryShift,
}

/// Large-scale ecosystem event triggered by player actions
#[derive(Message)]
pub struct EcosystemCascade {
    pub cascade_type: CascadeType,
    pub position: Vec2,
}

// ============================================================================
// WORLD EVENTS
// ============================================================================

/// Entered a new chunk
#[derive(Message)]
pub struct ChunkEntered {
    pub chunk_pos: IVec2,
}

/// Discovered a point of interest
#[derive(Message)]
pub struct PoiDiscovered {
    pub poi_type: crate::components::PoiType,
    pub position: Vec2,
}

/// Started docking with something
#[derive(Message)]
pub struct DockingStarted {
    pub target: Entity,
}

/// Finished docking
#[derive(Message)]
pub struct DockingCompleted {
    pub target: Entity,
}

// ============================================================================
// UI EVENTS
// ============================================================================

/// Notification to display
/// Start a brand-new expedition, wiping the previous run.
///
/// Raised by both paths out of the main menu. Before this existed a fresh
/// expedition silently inherited the last one's credits, cargo, contracts,
/// standing, statistics, drifting dead and galaxy — and once the story landed,
/// its read logs too, which meant a new run could fire the ending in its first
/// second because `logs_found` still held the finale.
#[derive(Message)]
pub struct NewExpeditionRequest;

#[derive(Message)]
pub struct ShowNotification {
    pub message: String,
    pub notification_type: NotificationType,
    pub duration: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum NotificationType {
    Info,
    Warning,
    Danger,
    Success,
}

/// Request to open a menu
#[derive(Message)]
pub struct OpenMenu {
    pub menu_type: MenuType,
}

#[derive(Clone, Copy, Debug)]
pub enum MenuType {
    Pause,
    Inventory,
    Map,
    CrewManagement,
    BuildMenu,
    Settings,
}

/// Request to close current menu
#[derive(Message)]
pub struct CloseMenu;

// ============================================================================
// CHAIN REACTION / FIRE / CASCADE EVENTS
// ============================================================================

/// Fired when an explosive module detonates (after PendingDetonation timer)
#[derive(Message)]
pub struct ModuleExploded {
    pub grid_position: IVec2,
    pub blast_damage: f32,
    pub explosive_type: ExplosiveType,
}

/// Fired to ignite a module
#[derive(Message)]
pub struct FireStarted {
    pub module: Entity,
    pub grid_position: IVec2,
    pub intensity: f32,
}

/// Fired when a fire goes out
#[derive(Message)]
pub struct FireExtinguished {
    pub module: Entity,
    pub cause: FireExtinguishCause,
}

#[derive(Clone, Copy, Debug)]
pub enum FireExtinguishCause {
    Decompression,
    BurnedOut,
    CrewSuppressed,
}

/// Fired when a hull segment reaches 0 HP
#[derive(Message)]
pub struct HullSegmentDestroyed {
    pub segment: Entity,
    pub grid_position: IVec2,
}

// ============================================================================
// CRISIS MANAGEMENT EVENTS
// ============================================================================

/// Request to seal/unseal a bulkhead door
#[derive(Message)]
pub struct ToggleBulkhead {
    pub segment: Entity,
    pub seal: bool,
}

/// Fired when a crew member is dispatched to handle an emergency
#[derive(Message)]
pub struct CrewDispatched {
    pub crew: Entity,
    pub room_id: usize,
    pub reason: DispatchReason,
}

#[derive(Clone, Copy, Debug)]
pub enum DispatchReason {
    Decompression,
    Fire,
}

// ============================================================================
// AI SUBMARINE EVENTS
// ============================================================================

/// Fired when an AI ship takes damage
#[derive(Message)]
pub struct AiShipDamaged {
    pub target: Entity,
    pub source: DamageSource,
    pub amount: f32,
    pub position: Option<Vec2>,
    pub direction: Option<Vec2>,
    /// The ship (player Ship entity or AI ship ROOT entity) that fired the
    /// shot responsible for this damage, if attributable. Lets a hit ship's
    /// "under fire" retaliation target whoever ACTUALLY shot it instead of
    /// defaulting to a guess. None for non-attributable sources (fire DoT
    /// ticks, a ship's own destruction cascade, creature attacks).
    pub attacker: Option<Entity>,
}

/// Fired when a module on an AI ship cooks off (ammo/fuel/reactor blast) —
/// world position so audio/vfx can attenuate by distance to the player.
#[derive(Message)]
pub struct AiModuleExploded {
    pub position: Vec2,
    pub blast_damage: f32,
}

/// Fired when an AI ship is destroyed
#[derive(Message)]
pub struct AiShipDestroyed {
    pub entity: Entity,
    pub ship_type: AiShipType,
    pub position: Vec2,
    /// Set if this ship was a tagged bounty target (see ai_ship::components::BountyTarget)
    /// — lets contract tracking complete the specific bounty instead of any
    /// ship of the same faction.
    pub bounty_id: Option<u32>,
    /// How the ship went out of the fight. A ship is "defeated" when it can
    /// no longer fight, not when every hull tile is ground to zero — see
    /// ai_ship::combat::check_ai_cripple.
    pub cause: ShipDeathCause,
}

/// The three ways a ship leaves a fight, which shape the wreck it leaves.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ShipDeathCause {
    /// Hull ground to nothing — the old, slow way. Little left to salvage.
    #[default]
    Gutted,
    /// Guns and engines both shot out: the crew strikes colors and the ship
    /// drifts as an intact derelict. The cleanest kill, and the best salvage.
    Struck,
    /// Last reactor breached and the containment let go. Big detonation,
    /// shattered hulk.
    Meltdown,
}

// ============================================================================
// CONTRACT EVENTS
// ============================================================================

/// A contract was accepted from the mission board
#[derive(Message)]
pub struct ContractAccepted {
    pub contract_id: u32,
}

/// A contract objective was completed (during exploration)
#[derive(Message)]
pub struct ContractCompleted {
    pub contract_id: u32,
}

/// A contract was failed (e.g. game over with deposit)
#[derive(Message)]
pub struct ContractFailed {
    pub contract_id: u32,
}

/// A completed contract was turned in at surface for rewards
#[derive(Message)]
pub struct ContractTurnedIn {
    pub contract_id: u32,
    pub reward: u32,
    pub faction: crate::contracts::Faction,
}

// ============================================================================
// SAVE/LOAD EVENTS
// ============================================================================

#[derive(Message)]
pub struct SaveGameRequest {
    pub slot: u32,
}

#[derive(Message)]
pub struct LoadGameRequest {
    pub slot: u32,
}

#[derive(Message)]
pub struct GameSaved {
    pub slot: u32,
    pub success: bool,
}

#[derive(Message)]
pub struct GameLoaded {
    pub slot: u32,
    pub success: bool,
}

// ============================================================================
// PLUGIN TO REGISTER ALL EVENTS
// ============================================================================

pub struct EventsPlugin;

impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
        app
            // Ship events
            .add_message::<ShipDamaged>()
            .add_message::<WeaponFired>()
            .add_message::<HullBreached>()
            .add_message::<ModuleDamaged>()
            .add_message::<ModuleDestroyed>()
            .add_message::<RoomDepressurized>()
            .add_message::<PowerStateChanged>()
            .add_message::<OxygenStateChanged>()
            .add_message::<DepthZoneChanged>()
            // Building events
            .add_message::<PlaceModuleRequest>()
            .add_message::<PlaceHullRequest>()
            .add_message::<ModulePlaced>()
            .add_message::<RemoveModuleRequest>()
            .add_message::<RemoveHullRequest>()
            .add_message::<ModuleRemoved>()
            // Crew events
            .add_message::<CrewDamaged>()
            .add_message::<CrewDied>()
            .add_message::<StaffingPriorityChanged>()
            // Creature events
            .add_message::<CreatureSpotted>()
            .add_message::<CreatureAttacking>()
            .add_message::<CreatureKilled>()
            // Ecosystem events
            .add_message::<CreatureAteCreature>()
            .add_message::<CreatureAteCorpse>()
            .add_message::<EcosystemCascade>()
            // World events
            .add_message::<ChunkEntered>()
            .add_message::<PoiDiscovered>()
            .add_message::<DockingStarted>()
            .add_message::<DockingCompleted>()
            // UI events
            .add_message::<ShowNotification>()
            .add_message::<NewExpeditionRequest>()
            .add_message::<OpenMenu>()
            .add_message::<CloseMenu>()
            // Chain reaction / fire / cascade events
            .add_message::<ModuleExploded>()
            .add_message::<FireStarted>()
            .add_message::<FireExtinguished>()
            .add_message::<HullSegmentDestroyed>()
            // Crisis management events
            .add_message::<ToggleBulkhead>()
            .add_message::<CrewDispatched>()
            // AI ship events
            .add_message::<AiShipDamaged>()
            .add_message::<AiModuleExploded>()
            .add_message::<AiShipDestroyed>()
            // Contract events
            .add_message::<ContractAccepted>()
            .add_message::<ContractCompleted>()
            .add_message::<ContractFailed>()
            .add_message::<ContractTurnedIn>()
            // Save/load events
            .add_message::<SaveGameRequest>()
            .add_message::<LoadGameRequest>()
            .add_message::<GameSaved>()
            .add_message::<GameLoaded>();
    }
}
