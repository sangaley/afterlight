use bevy::prelude::*;

// ============================================================================
// MULTI-BLOCK MACHINE COMPONENTS
// Weapons, reactors, engines, and shields are built from connected blocks.
// Stats emerge from physical layout. Damage propagates through the chain.
// ============================================================================

/// Role of a block in a multi-block machine
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum BlockRole {
    /// The brain of the machine. Determines type. Destruction = machine dead.
    Core,
    /// Barrel/tube extending from core. More = more range/accuracy.
    Barrel,
    /// Ammo feed connected to core. More = faster reload.
    AmmoFeed,
    /// Cooling block adjacent to core/barrel. More = longer sustained fire.
    Cooling,
    /// Fuel rod in a reactor. More = more power output.
    FuelRod,
    /// Engine nozzle. More = more thrust.
    Nozzle,
    /// Shield emitter. Placement determines coverage arc.
    ShieldEmitter,
}

impl BlockRole {
    /// Can this role connect to a core?
    pub fn connects_to_core(&self) -> bool {
        matches!(self, Self::Barrel | Self::AmmoFeed | Self::Cooling | Self::FuelRod | Self::Nozzle | Self::ShieldEmitter)
    }

    /// Can this role chain (extend from itself)?
    pub fn can_chain(&self) -> bool {
        matches!(self, Self::Barrel | Self::Nozzle)
    }
}

/// Marks a block as part of a multi-block machine
#[derive(Component, Clone, Debug)]
pub struct MachineBlock {
    pub role: BlockRole,
    /// Which core entity this block is connected to (None if disconnected)
    pub connected_core: Option<Entity>,
    /// Distance from core in the chain (core = 0, first barrel = 1, etc.)
    pub chain_distance: u32,
    /// The next block in the chain toward the tip (if any)
    pub next_in_chain: Option<Entity>,
    /// The previous block in the chain toward the core (if any)
    pub prev_in_chain: Option<Entity>,
}

impl Default for MachineBlock {
    fn default() -> Self {
        Self {
            role: BlockRole::Core,
            connected_core: None,
            chain_distance: 0,
            next_in_chain: None,
            prev_in_chain: None,
        }
    }
}

/// Component on the core block — aggregated stats from all connected blocks
#[derive(Component, Clone, Debug, Default)]
pub struct MachineStats {
    /// How many barrel blocks are connected
    pub barrel_count: u32,
    /// How many ammo feed blocks are connected
    pub feed_count: u32,
    /// How many cooling blocks are connected
    pub cooling_count: u32,
    /// How many fuel rod blocks are connected
    pub fuel_rod_count: u32,
    /// How many nozzle blocks are connected
    pub nozzle_count: u32,
    /// How many shield emitter blocks are connected
    pub emitter_count: u32,

    // === Calculated weapon stats (from physical layout) ===
    /// Base damage from core, multiplied by barrel quality
    pub effective_damage: f32,
    /// Range scales with barrel count
    pub effective_range: f32,
    /// Fire rate scales with feed count
    pub effective_fire_rate: f32,
    /// How long before overheating (scales with cooling)
    pub heat_capacity: f32,
    /// Accuracy (more barrels = better, but diminishing returns)
    pub effective_accuracy: f32,
}

/// Stable snapshot of a weapon core's registry-defined stats, taken once at
/// spawn. calculate_machine_stats must read its "base" numbers from here —
/// never from the live Weapon component, and never from MachineStats itself.
/// MachineStats gets fully overwritten every frame by rebuild_machine_connections
/// (`*core_stats = stats.clone()` from a freshly Default::default()'d
/// accumulator), and apply_machine_stats_to_weapons writes the *computed*
/// numbers straight back into Weapon. Using either of those live values as
/// the calculation's input made every frame's output the next frame's input:
/// for a bare core with no barrels (effective_range *= 0.6), range decayed
/// by 0.6x every single frame — to a denormalized float within a couple of
/// seconds. That's why kinetic weapons "aimed at their own muzzle" (range
/// clamped the aim point back to weapon_pos) and appeared frozen or wild.
#[derive(Component, Clone, Copy, Debug)]
pub struct BaseWeaponStats {
    pub damage: f32,
    pub range: f32,
    pub fire_rate: f32,
    pub max_ammo: u32,
}

/// Marks a block as disconnected from its core (barrel chain severed)
#[derive(Component)]
pub struct Disconnected;

/// Cascade explosion data — when a block is destroyed, chance to chain
#[derive(Component)]
pub struct CascadeRisk {
    /// Base chance (0.0-1.0) to cascade to adjacent blocks
    pub cascade_chance: f32,
    /// Damage dealt to adjacent block if cascade triggers
    pub cascade_damage: f32,
}

impl Default for CascadeRisk {
    fn default() -> Self {
        Self {
            cascade_chance: 0.20,   // 20% base — real risk. Build reinforced joints.
            cascade_damage: 35.0,   // Enough to cripple the next block. Chain reactions are devastating.
        }
    }
}

/// Tracks the stress on a barrel block (higher stress = higher cascade chance)
/// Stress increases with chain distance from core (tip has least stress, base has most)
#[derive(Component)]
pub struct BarrelStress {
    /// Blocks supported beyond this point (including this one)
    pub load: u32,
    /// Modified cascade chance based on stress
    pub effective_cascade_chance: f32,
}

/// Machine type — what kind of machine the core creates
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MachineType {
    Weapon,
    Reactor,
    Engine,
    Shield,
}
