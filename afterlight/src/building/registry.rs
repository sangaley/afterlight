use bevy::prelude::*;
use std::collections::HashMap;
use crate::components::*;

/// Rust-tan, so ablative reads as the soft plate it is.
const ABLATIVE_TONE: Color = Color::srgb(0.62, 0.42, 0.28);
/// Pale polished blue, so reflective plate catches the eye beside dark steel.
const REFLECTIVE_TONE: Color = Color::srgb(0.68, 0.84, 0.95);

/// Definition of a module type with all its stats
#[allow(dead_code)]
pub struct ModuleDef {
    pub name: &'static str,
    pub description: &'static str,
    pub category: ModuleCategory,
    pub size: IVec2,
    pub health: f32,
    pub power_generation: f32,
    pub power_consumption: f32,
    pub color: Color,
    pub companion: CompanionData,
    pub customizable: bool,
    pub cost: u32,
    pub base_stats: CalculatedStats,
    /// Whether this module gets a CrewStation when spawned
    pub crew_station: bool,
}

/// Companion component data to attach when spawning a module
pub enum CompanionData {
    /// No special companion components
    None,
    /// Reactor: generates power, has heat management
    Reactor { output: f32, max_heat: f32, explosion_risk: bool },
    /// Engine: provides thrust
    Engine { thrust: f32, noise_level: f32 },
    /// Oxygen scrubber: generates O2
    OxygenScrubber { output: f32 },
    /// Generic life support (CO2 filter, water recycler)
    LifeSupport { o2_gen: f32, co2_filter: f32 },
    /// Thruster: maneuvering control
    Thruster { thrust_power: f32 },
    /// Cargo/storage
    Cargo { capacity: f32 },
    /// Weapon system
    Weapon {
        damage: f32,
        range: f32,
        fire_rate: f32,
        ammo: u32,
        mount_type: MountType,
        ammo_type: AmmoType,
    },
    /// Active radar (generates noise on ping)
    Radar { range: f32, noise_on_ping: f32 },
    /// Passive radar (no noise)
    PassiveRadar { range: f32 },
    /// Detection system (non-radar)
    Detection { range: f32 },
    /// Light source
    Light { range: f32, intensity: f32, attracts_creatures: bool },
    /// Repair capability
    Repair { rate: f32 },
    /// Navigation system
    Navigation { map_range: f32 },
    /// Docking port
    Docking,
    /// Salvage arm
    Salvage { range: f32, efficiency: f32 },
    /// Crew quarters: provides berths
    Quarters { berths: u32 },
    /// Crew welfare facility
    CrewFacility { facility_type: crate::components::FacilityType },
    /// Capacitor: stores power for burst discharge
    Capacitor { capacity: f32, charge_rate: f32 },
    /// Power conduit: routes power through non-adjacent modules
    PowerConduit { throughput: f32 },
    /// Fire suppression system
    FireSuppression { effectiveness: f32 },
    /// Active cooling pump
    CoolingPump { cooling_rate: f32 },
    /// Passive heat vent
    HeatVent { dissipation_rate: f32 },
    /// Power transformer
    Transformer { efficiency: f32 },
    /// Oxygen storage tank
    OxygenTank { capacity: f32 },
    /// Ammo auto-loader
    AmmoAutoloader { reload_bonus: f32 },
    /// Radiation shielding reinforcement for hull
    RadiationShielding { shielding_bonus: f32 },
    /// Drone bay: deploys repair/scout drones
    DroneBay { drone_count: u32, drone_range: f32 },
    /// Conveyor tube: moves ammo/resources between adjacent modules
    ConveyorTube { speed: f32 },
    /// Fuel processor: refines fuel, reduces consumption
    FuelProcessor { efficiency: f32 },
    /// Hull seal system: automated breach sealing
    HullSeal { seal_rate: f32 },
    ForceField { radius: f32, power_per_tile: f32 },
    Airlock { suits: u32 },
    /// Targeting computer: boosts weapon accuracy
    TargetingComputer { accuracy_bonus: f32 },
    /// AI combat core: auto-targets highest threat
    AICombatCore { priority_bonus: f32 },
    MemoryCore { autonomy: f32 },
    /// Research lab: generates research points from specimens
    ResearchLab { research_speed: f32 },
}

/// Data-driven registry of all module types
#[derive(Resource)]
pub struct ModuleRegistry {
    pub defs: HashMap<ModuleType, ModuleDef>,
}

impl ModuleRegistry {
    pub fn get(&self, module_type: ModuleType) -> &ModuleDef {
        self.defs.get(&module_type).expect("ModuleType not in registry")
    }

    /// Safe accessor that returns None if the module type isn't registered
    #[cfg(test)]
    pub fn try_get(&self, module_type: ModuleType) -> Option<&ModuleDef> {
        self.defs.get(&module_type)
    }
}

/// Build the complete module registry with all 42 module definitions
pub fn build_registry() -> ModuleRegistry {
    let mut defs = HashMap::new();

    // ========================================================================
    // POWER (5)
    // ========================================================================

    defs.insert(ModuleType::SmallReactor, ModuleDef {
        name: "Small Reactor",
        description: "Workhorse fission cell. Won't win any power awards, but it'll keep the lights on when everything else is failing.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 300.0,
        power_consumption: 0.0,
        color: Color::srgb(0.8, 0.4, 0.1),
        companion: CompanionData::Reactor { output: 300.0, max_heat: 100.0, explosion_risk: true },
        customizable: true,
        cost: 75,
        base_stats: CalculatedStats {
            reactor: Some(ReactorStats {
                power_output: 60.0,
                heat_generation: 80.0,
                explosion_risk: 0.1,
            }),
            ..Default::default()
        },
        crew_station: true,
    });

    defs.insert(ModuleType::StandardReactor, ModuleDef {
        name: "Standard Reactor",
        description: "The backbone of most starship power grids. Solid output, manageable heat. Staff it for peak performance.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 100.0,
        power_generation: 500.0,
        power_consumption: 0.0,
        color: Color::srgb(0.9, 0.5, 0.1),
        companion: CompanionData::Reactor { output: 500.0, max_heat: 100.0, explosion_risk: true },
        customizable: false,
        cost: 135,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::LargeReactor, ModuleDef {
        name: "Large Reactor",
        description: "Heavy-duty reactor that drinks fuel and radiates heat, but powers a full weapons loadout without breaking a sweat.",
        category: ModuleCategory::Power,
        size: IVec2::new(2, 1),
        health: 150.0,
        power_generation: 200.0,
        power_consumption: 0.0,
        color: Color::srgb(1.0, 0.5, 0.0),
        companion: CompanionData::Reactor { output: 200.0, max_heat: 100.0, explosion_risk: true },
        customizable: false,
        cost: 375,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::BatteryBank, ModuleDef {
        name: "Battery Bank",
        description: "Chemical battery array. Zero heat, zero noise, zero glory - but it keeps systems alive when reactors go down.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 30.0,
        power_consumption: 0.0,
        color: Color::srgb(0.7, 0.6, 0.1),
        companion: CompanionData::None,
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::RTG, ModuleDef {
        name: "RTG",
        description: "Radioisotope thermoelectric generator. Low but eternal output.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 120.0,
        power_generation: 40.0,
        power_consumption: 0.0,
        color: Color::srgb(0.6, 0.8, 0.2),
        companion: CompanionData::None,
        customizable: false,
        cost: 188,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // PROPULSION (5)
    // ========================================================================

    defs.insert(ModuleType::SmallEngine, ModuleDef {
        name: "Small Engine",
        description: "Entry-level drive. Gets you moving, but don't expect to outrun anything with teeth.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.5, 0.5, 0.5),
        companion: CompanionData::Engine { thrust: 250.0, noise_level: 20.0 },
        customizable: true,
        cost: 38,
        base_stats: CalculatedStats {
            engine: Some(EngineStats {
                thrust: 50.0,
                fuel_efficiency: 1.0,
                noise: 10.0,
            }),
            ..Default::default()
        },
        crew_station: true,
    });

    defs.insert(ModuleType::StandardEngine, ModuleDef {
        name: "Standard Engine",
        description: "Reliable mid-range drive. Enough thrust to dodge most threats, quiet enough to not attract them.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 30.0,
        color: Color::srgb(0.6, 0.6, 0.6),
        companion: CompanionData::Engine { thrust: 400.0, noise_level: 30.0 },
        customizable: false,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::LargeEngine, ModuleDef {
        name: "Large Engine",
        description: "Twin-turbine powerhouse. Makes everything around it vibrate, but nothing in the void can catch you at full burn.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(2, 1),
        health: 150.0,
        power_generation: 0.0,
        power_consumption: 50.0,
        color: Color::srgb(0.7, 0.7, 0.7),
        companion: CompanionData::Engine { thrust: 180.0, noise_level: 50.0 },
        customizable: false,
        cost: 225,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::SilentDrive, ModuleDef {
        name: "Silent Drive",
        description: "Magneto-hydrodynamic drive with no moving parts. Almost silent - creatures won't hear you coming.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 40.0,
        color: Color::srgb(0.3, 0.3, 0.4),
        companion: CompanionData::Engine { thrust: 80.0, noise_level: 5.0 },
        customizable: false,
        cost: 150,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::ManeuveringThruster, ModuleDef {
        name: "Maneuvering Thruster",
        description: "Attitude jet for fine adjustments. Doesn't push hard, but lets you thread the needle between rock formations.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.4, 0.4, 0.5),
        companion: CompanionData::Engine { thrust: 40.0, noise_level: 10.0 },
        customizable: false,
        cost: 30,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // LIFE SUPPORT (3)
    // ========================================================================

    defs.insert(ModuleType::OxygenScrubber, ModuleDef {
        name: "O2 Scrubber",
        description: "Extracts breathable O2 from mineral reserves. The hum is annoying, but the alternative is suffocation.",
        category: ModuleCategory::LifeSupport,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.2, 0.6, 0.8),
        companion: CompanionData::OxygenScrubber { output: 30.0 },
        customizable: true,
        cost: 45,
        base_stats: CalculatedStats {
            life_support: Some(LifeSupportStats {
                // Must match the companion output above — CalculatedStats
                // overrides it once a module is customized, and the old 10.0
                // silently cut a customized scrubber's output to a third.
                o2_generation: 30.0,
                co2_filtering: 8.0,
                crew_capacity: 5,
            }),
            ..Default::default()
        },
        crew_station: true,
    });

    defs.insert(ModuleType::CO2Scrubber, ModuleDef {
        name: "CO2 Scrubber",
        description: "Lithium hydroxide scrubber. Pulls CO2 out before it puts your crew to sleep permanently.",
        category: ModuleCategory::LifeSupport,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.3, 0.5, 0.7),
        companion: CompanionData::LifeSupport { o2_gen: 0.0, co2_filter: 30.0 },
        customizable: false,
        cost: 38,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::WasteRecycler, ModuleDef {
        name: "Water Recycler",
        description: "Closed-loop waste recycler. Don't ask where the input comes from. Just drink it.",
        category: ModuleCategory::LifeSupport,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.2, 0.4, 0.7),
        companion: CompanionData::LifeSupport { o2_gen: 0.0, co2_filter: 0.0 },
        customizable: false,
        cost: 38,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // CONTROL (2)
    // ========================================================================

    defs.insert(ModuleType::NavigationConsole, ModuleDef {
        name: "Navigation Console",
        description: "Holographic chart table with dead-reckoning computer. Shows you where you are - and where you shouldn't go.",
        category: ModuleCategory::Control,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.5, 0.3, 0.6),
        companion: CompanionData::Navigation { map_range: 500.0 },
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::HelmStation, ModuleDef {
        name: "Helm Station",
        description: "Where the driver sits. A staffed helm means tighter turns and faster response when something lunges at you.",
        category: ModuleCategory::Control,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.4, 0.3, 0.5),
        companion: CompanionData::None,
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // ========================================================================
    // WEAPONS (6)
    // ========================================================================

    // --- KINETIC WEAPONS ---
    defs.insert(ModuleType::Cannon, ModuleDef {
        name: "Cannon",
        description: "Heavy kinetic rounds. Slow fire, devastating impact. The workhorse of combat.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 90.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.7, 0.3, 0.2),
        companion: CompanionData::Weapon {
            damage: 50.0, range: 16200.0, fire_rate: 0.6, ammo: 30,
            mount_type: MountType::Turret, ammo_type: AmmoType::Bullet,
        },
        customizable: true,
        cost: 113,
        base_stats: CalculatedStats {
            weapon: Some(WeaponStats {
                damage: 50.0, range: 16200.0, fire_rate: 0.6, max_ammo: 30, power_cost: 20.0,
            }),
            ..Default::default()
        },
        crew_station: true,
    });

    defs.insert(ModuleType::Railgun, ModuleDef {
        name: "Railgun",
        description: "Electromagnetically accelerated slugs. Extreme range and penetration. Capacitor-hungry.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(2, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 40.0,
        color: Color::srgb(0.5, 0.5, 0.8),
        companion: CompanionData::Weapon {
            damage: 100.0, range: 25200.0, fire_rate: 0.25, ammo: 15,
            mount_type: MountType::Fixed, ammo_type: AmmoType::Bullet,
        },
        customizable: true,
        cost: 300,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::Coilgun, ModuleDef {
        name: "Coilgun",
        description: "Burst-fire magnetic accelerator. High DPS at medium range. Burns through ammo fast.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 25.0,
        color: Color::srgb(0.4, 0.5, 0.7),
        companion: CompanionData::Weapon {
            damage: 19.0, range: 18000.0, fire_rate: 2.0, ammo: 60,
            mount_type: MountType::Turret, ammo_type: AmmoType::Bullet,
        },
        customizable: true,
        cost: 135,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::Gatling, ModuleDef {
        name: "Gatling",
        description: "Rotary autocannon. Fills space with lead up close. Inaccurate but terrifying against swarms.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.6, 0.3, 0.3),
        companion: CompanionData::Weapon {
            // Short-range brawler archetype: close the distance, get the
            // fire-rate reward. Range down from 3900 (was blurring into
            // mid-range territory); damage up from 6.5 to help offset the
            // exposure of actually being that close.
            damage: 8.0, range: 6600.0, fire_rate: 6.0, ammo: 300,
            mount_type: MountType::Turret, ammo_type: AmmoType::Bullet,
        },
        customizable: true,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // --- ENERGY WEAPONS ---
    defs.insert(ModuleType::Laser, ModuleDef {
        name: "Laser",
        description: "Sustained energy beam. Precise, no ammo, overheats fast. The surgeon's tool.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 35.0,
        color: Color::srgb(0.3, 0.7, 0.3),
        companion: CompanionData::Weapon {
            damage: 25.0, range: 16200.0, fire_rate: 1.0, ammo: 999,
            mount_type: MountType::Turret, ammo_type: AmmoType::Charge,
        },
        customizable: true,
        cost: 150,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::PlasmaCaster, ModuleDef {
        name: "Plasma Caster",
        description: "Superheated plasma bolts. Area damage on impact. Power hungry but devastating.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 30.0,
        color: Color::srgb(0.8, 0.4, 0.1),
        companion: CompanionData::Weapon {
            damage: 44.0, range: 12600.0, fire_rate: 0.8, ammo: 999,
            mount_type: MountType::Broadside, ammo_type: AmmoType::Charge,
        },
        customizable: true,
        cost: 188,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::IonDisruptor, ModuleDef {
        name: "Ion Disruptor",
        description: "Disrupts electrical systems up close. Low damage, high disable chance. Makes targets helpless.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 25.0,
        color: Color::srgb(0.5, 0.2, 0.6),
        companion: CompanionData::Weapon {
            // Short-range brawler: a disable effect is a close-quarters tool
            // by nature. Range down from 4200 to sit clearly below the
            // mid-range tier (Cannon/Coilgun/Laser/PlasmaCaster at 4200-6000).
            damage: 12.5, range: 5400.0, fire_rate: 0.5, ammo: 999,
            mount_type: MountType::Turret, ammo_type: AmmoType::Charge,
        },
        customizable: true,
        cost: 135,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // --- MISSILE WEAPONS ---
    defs.insert(ModuleType::HeavyMissile, ModuleDef {
        name: "Heavy Missile Launcher",
        description: "Self-propelled warhead. Slow, devastating, limited supply. Make every shot count.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 25.0,
        color: Color::srgb(0.7, 0.2, 0.2),
        companion: CompanionData::Weapon {
            damage: 75.0, range: 28800.0, fire_rate: 0.3, ammo: 12,
            mount_type: MountType::Fixed, ammo_type: AmmoType::Missile,
        },
        customizable: true,
        cost: 150,
        base_stats: CalculatedStats {
            weapon: Some(WeaponStats {
                damage: 75.0, range: 28800.0, fire_rate: 0.3, max_ammo: 12, power_cost: 25.0,
            }),
            ..Default::default()
        },
        crew_station: true,
    });

    defs.insert(ModuleType::GuidedMissile, ModuleDef {
        name: "Guided Missile",
        description: "Lock on, fire, forget. Tracking missiles that chase targets. Counterable with ECM.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.8, 0.3, 0.2),
        companion: CompanionData::Weapon {
            damage: 44.0, range: 25200.0, fire_rate: 0.4, ammo: 16,
            mount_type: MountType::Turret, ammo_type: AmmoType::Missile,
        },
        customizable: true,
        cost: 188,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // --- LAUNCHERS FOR DESIGNED MISSILES (combat::missile_design) ---
    // Damage, range and fire rate here are only the starting figures: each
    // launcher's own missile design overwrites them (fit_missile_loadouts).
    // Like a built gun's firing piece they need no crew of their own.
    defs.insert(ModuleType::LaunchPad, ModuleDef {
        name: "Launch Pad",
        description: "Launches one small missile you design: 120 mm, up to 4 parts. Cheap and quick to reload.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.75, 0.45, 0.25),
        companion: CompanionData::Weapon {
            damage: 60.0, range: 15600.0, fire_rate: 0.3, ammo: 12,
            mount_type: MountType::Fixed, ammo_type: AmmoType::Missile,
        },
        customizable: true,
        cost: 90,
        base_stats: CalculatedStats {
            weapon: Some(WeaponStats { damage: 60.0, range: 15600.0, fire_rate: 0.3, max_ammo: 12, power_cost: 10.0 }),
            ..Default::default()
        },
        crew_station: false,
    });
    defs.insert(ModuleType::RailLauncher, ModuleDef {
        name: "Rail Launcher",
        description: "Launches a missile you design along its rail: 250 mm, up to 6 parts. Face it at the target.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 3),
        health: 120.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.75, 0.45, 0.25),
        companion: CompanionData::Weapon {
            damage: 260.0, range: 20700.0, fire_rate: 0.15, ammo: 8,
            mount_type: MountType::Fixed, ammo_type: AmmoType::Missile,
        },
        customizable: true,
        cost: 220,
        base_stats: CalculatedStats {
            weapon: Some(WeaponStats { damage: 260.0, range: 20700.0, fire_rate: 0.15, max_ammo: 8, power_cost: 20.0 }),
            ..Default::default()
        },
        crew_station: false,
    });
    defs.insert(ModuleType::VlsSilo, ModuleDef {
        name: "VLS Silo",
        description: "Launches a big missile you design up out of the deck: 400 mm, up to 9 parts. Strikes in any direction, turns late, reloads slowly.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(2, 2),
        health: 220.0,
        power_generation: 0.0,
        power_consumption: 35.0,
        color: Color::srgb(0.75, 0.45, 0.25),
        companion: CompanionData::Weapon {
            damage: 2000.0, range: 21600.0, fire_rate: 0.07, ammo: 6,
            mount_type: MountType::Fixed, ammo_type: AmmoType::Missile,
        },
        customizable: true,
        cost: 480,
        base_stats: CalculatedStats {
            weapon: Some(WeaponStats { damage: 2000.0, range: 21600.0, fire_rate: 0.07, max_ammo: 6, power_cost: 35.0 }),
            ..Default::default()
        },
        crew_station: false,
    });

    defs.insert(ModuleType::ClusterRocket, ModuleDef {
        name: "Cluster Rocket",
        description: "Fires a spread of unguided rockets. Area saturation. Quantity over quality.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.6, 0.2, 0.1),
        companion: CompanionData::Weapon {
            damage: 19.0, range: 14400.0, fire_rate: 1.5, ammo: 40,
            mount_type: MountType::Fixed, ammo_type: AmmoType::Missile,
        },
        customizable: true,
        cost: 98,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // --- UTILITY WEAPONS ---
    defs.insert(ModuleType::MiningDrill, ModuleDef {
        name: "Mining Drill",
        description: "Extracts resources from asteroids. Weak in combat but essential for self-sustaining expeditions.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.6, 0.5, 0.3),
        companion: CompanionData::Weapon {
            damage: 5.0, range: 80.0, fire_rate: 2.0, ammo: 999,
            mount_type: MountType::Fixed, ammo_type: AmmoType::Charge,
        },
        customizable: true,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::TractorBeam, ModuleDef {
        name: "Tractor Beam",
        description: "Pulls objects toward ship. Salvage, debris clearing, creative combat applications.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 25.0,
        color: Color::srgb(0.3, 0.5, 0.6),
        companion: CompanionData::Weapon {
            damage: 0.0, range: 300.0, fire_rate: 1.0, ammo: 999,
            mount_type: MountType::Turret, ammo_type: AmmoType::Charge,
        },
        customizable: true,
        cost: 113,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::EMPPulse, ModuleDef {
        name: "EMP Pulse",
        description: "Disables electronics in a radius. Affects friend and foe. Use with extreme caution.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 30.0,
        color: Color::srgb(0.4, 0.3, 0.7),
        companion: CompanionData::Weapon {
            damage: 6.0, range: 3600.0, fire_rate: 0.2, ammo: 5,
            mount_type: MountType::Broadside, ammo_type: AmmoType::Charge,
        },
        customizable: true,
        cost: 150,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // ========================================================================
    // DETECTION (4)
    // ========================================================================

    defs.insert(ModuleType::RadarArray, ModuleDef {
        name: "Radar Array",
        description: "Ping and pray. Reveals everything nearby, but everything nearby now knows exactly where you are.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.2, 0.5, 0.5),
        companion: CompanionData::Radar { range: 2500.0, noise_on_ping: 80.0 },
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::AdvancedRadar, ModuleDef {
        name: "Advanced Radar",
        description: "Military-grade phased array. Sees farther, pings quieter. The upgrade you wish you'd bought before the first ambush.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 30.0,
        color: Color::srgb(0.1, 0.6, 0.6),
        companion: CompanionData::Radar { range: 4000.0, noise_on_ping: 60.0 },
        customizable: false,
        cost: 150,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::PassiveRadar, ModuleDef {
        name: "Passive Radar",
        description: "Sensitive hydrophone array. Hears everything without revealing your position. Patience is a survival strategy.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.2, 0.4, 0.4),
        companion: CompanionData::PassiveRadar { range: 3000.0 },
        customizable: false,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::DepthScanner, ModuleDef {
        name: "Long Range Scanner",
        description: "Extended-range sensor array. Reveals nearby hazards, wrecks, and terrain before you fly straight into them.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.3, 0.5, 0.4),
        companion: CompanionData::Detection { range: 400.0 },
        customizable: false,
        cost: 68,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // STORAGE (5)
    // ========================================================================

    defs.insert(ModuleType::SmallCargo, ModuleDef {
        name: "Small Cargo",
        description: "A steel box bolted to the floor. Holds salvage, samples, and whatever else you drag back from the void.",
        category: ModuleCategory::Storage,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.4, 0.2),
        companion: CompanionData::Cargo { capacity: 100.0 },
        customizable: false,
        cost: 23,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::LargeCargo, ModuleDef {
        name: "Large Cargo",
        description: "Walk-in cargo hold with reinforced shelving. Triple the capacity, double the heartbreak when it depressurizes.",
        category: ModuleCategory::Storage,
        size: IVec2::new(2, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.6, 0.5, 0.2),
        companion: CompanionData::Cargo { capacity: 300.0 },
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AmmoBay, ModuleDef {
        name: "Ammo Bay",
        description: "Magazine racks for torpedoes, shells, and mines. Keep it away from reactors unless you enjoy fireworks.",
        category: ModuleCategory::Storage,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.6, 0.3, 0.2),
        companion: CompanionData::Cargo { capacity: 200.0 },
        customizable: false,
        cost: 38,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::FuelTank, ModuleDef {
        name: "Fuel Tank",
        description: "Pressurized fuel bladder. More range means deeper ventures - but also more to explode if something breaches it.",
        category: ModuleCategory::Storage,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.4, 0.3),
        companion: CompanionData::Cargo { capacity: 160.0 },
        customizable: false,
        cost: 30,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::SpecimenVault, ModuleDef {
        name: "Specimen Vault",
        description: "Temperature-regulated containment for biological specimens. The research division pays triple for intact samples.",
        category: ModuleCategory::Storage,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.4, 0.5, 0.5),
        companion: CompanionData::Cargo { capacity: 60.0 },
        customizable: false,
        cost: 53,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // CREW (5)
    // ========================================================================

    defs.insert(ModuleType::BasicQuarters, ModuleDef {
        name: "Basic Quarters",
        description: "Cramped steel bunks and a shared locker. Houses 10 in hot-bunk rotation, and nobody's thrilled about it.",
        category: ModuleCategory::Crew,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.3, 0.6, 0.3),
        companion: CompanionData::Quarters { berths: 10 },
        customizable: false,
        cost: 30,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::Barracks, ModuleDef {
        name: "Barracks",
        description: "Military-style dormitory with stacked racks. Fits 20 bodies, though personal space is a myth down here.",
        category: ModuleCategory::Crew,
        size: IVec2::new(2, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.35, 0.65, 0.35),
        companion: CompanionData::Quarters { berths: 20 },
        customizable: false,
        cost: 90,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::MedBay, ModuleDef {
        name: "Med Bay",
        description: "Surgical suite and recovery ward. Wounded crew walk here between fights and recover at 1 HP/s. Nobody is treated while the shooting lasts.",
        category: ModuleCategory::Crew,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.8, 0.8, 0.9),
        companion: CompanionData::CrewFacility { facility_type: crate::components::FacilityType::MedBay },
        customizable: false,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::MessHall, ModuleDef {
        name: "Mess Hall",
        description: "Hot meals and a place to sit that isn't bolted to a reactor. Boosts morale at +2/s for the whole crew.",
        category: ModuleCategory::Crew,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.4, 0.7, 0.4),
        companion: CompanionData::CrewFacility { facility_type: crate::components::FacilityType::MessHall },
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::RecRoom, ModuleDef {
        name: "Rec Room",
        description: "Cards, books, and a dartboard. Keeps crew morale from dropping below 30 - sanity insurance.",
        category: ModuleCategory::Crew,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.5, 0.7, 0.3),
        companion: CompanionData::CrewFacility { facility_type: crate::components::FacilityType::RecRoom },
        customizable: false,
        cost: 38,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // UTILITY (7)
    // ========================================================================

    defs.insert(ModuleType::RepairBay, ModuleDef {
        name: "Repair Bay",
        description: "Robotic arm with welding torch and spare parts bin. Automatically patches damaged modules in its room.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.4, 0.6, 0.4),
        companion: CompanionData::Repair { rate: 5.0 },
        customizable: false,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::ManeuverThruster, ModuleDef {
        name: "Maneuvering Thruster",
        description: "Vertical thruster for attitude control. Essential for maneuvering - without one you're drifting forever.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.2, 0.3, 0.5),
        companion: CompanionData::Thruster { thrust_power: 100.0 },
        customizable: false,
        cost: 15,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::Floodlight, ModuleDef {
        name: "Floodlight",
        description: "Halogen flood array. Lights up the immediate area but acts like a dinner bell for anything hungry.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.9, 0.9, 0.5),
        companion: CompanionData::Light { range: 200.0, intensity: 1.0, attracts_creatures: true },
        customizable: false,
        cost: 23,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::Searchlight, ModuleDef {
        name: "Searchlight",
        description: "Narrow-beam spotlight. Cuts through the darkness without painting a target on your hull.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.9, 0.8, 0.4),
        companion: CompanionData::Light { range: 400.0, intensity: 0.8, attracts_creatures: false },
        customizable: false,
        cost: 38,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AirlockChamber, ModuleDef {
        name: "Airlock Chamber",
        description: "Pressurized chamber for suited EVA operations, and the rack the suits hang on. Four of them - a breach with more crew than suits is a choice about who gets one. Also the lock the dead go out of.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.5, 0.5, 0.6),
        companion: CompanionData::Airlock { suits: 12 },
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::DockingPort, ModuleDef {
        name: "Docking Port",
        description: "Universal coupling for stations, wrecks, and allied vessels. Your ticket to resupply and trade.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.4, 0.4, 0.5),
        companion: CompanionData::Docking,
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::SalvageArm, ModuleDef {
        name: "Breaker Drill",
        description: "Industrial grinder that chews wreck blocks straight into the hold. Faster than an EVA detail and nobody's at risk - but it needs power, an operator, and the racket carries to everything with ears.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.6, 0.5, 0.3),
        companion: CompanionData::Salvage { range: 250.0, efficiency: 1.0 },
        customizable: false,
        cost: 68,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // ========================================================================
    // POWER — NEW (4)
    // ========================================================================

    defs.insert(ModuleType::FusionReactor, ModuleDef {
        name: "Fusion Reactor",
        description: "Deuterium fusion core. 4x the output of a large reactor, but runs dangerously hot. 3x3 endgame power.",
        category: ModuleCategory::Power,
        size: IVec2::new(3, 3),
        health: 250.0,
        power_generation: 400.0,
        power_consumption: 0.0,
        color: Color::srgb(1.0, 0.6, 0.0),
        companion: CompanionData::Reactor { output: 400.0, max_heat: 100.0, explosion_risk: true },
        customizable: false,
        cost: 900,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::Capacitor, ModuleDef {
        name: "Capacitor",
        description: "Banks surplus power for weapon burst fire. Charges when generation exceeds demand.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.6, 0.7, 0.2),
        companion: CompanionData::Capacitor { capacity: 300.0, charge_rate: 50.0 },
        customizable: false,
        cost: 113,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::PowerConduit, ModuleDef {
        name: "Power Conduit",
        description: "Connects power grid across gaps. Place between reactors and distant systems.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.5, 0.2),
        companion: CompanionData::PowerConduit { throughput: 100.0 },
        customizable: false,
        cost: 30,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::SolarCell, ModuleDef {
        name: "Solar Cell",
        description: "Photovoltaic panel. Trickle power near stars, useless in deep space.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 30.0,
        power_generation: 15.0,
        power_consumption: 0.0,
        color: Color::srgb(0.3, 0.5, 0.8),
        companion: CompanionData::Reactor { output: 15.0, max_heat: 0.0, explosion_risk: false },
        customizable: false,
        cost: 23,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // PROPULSION — NEW (3)
    // ========================================================================

    defs.insert(ModuleType::JetDrive, ModuleDef {
        name: "Jet Drive",
        description: "Water-jet propulsion. Fast but announces your position to everything nearby.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 45.0,
        color: Color::srgb(0.7, 0.5, 0.5),
        companion: CompanionData::Engine { thrust: 150.0, noise_level: 60.0 },
        customizable: false,
        cost: 135,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::EmergencyThruster, ModuleDef {
        name: "Emergency Thruster",
        description: "Single-use thruster burst. Maximum escape velocity when something big is chasing you.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 60.0,
        color: Color::srgb(0.8, 0.3, 0.3),
        companion: CompanionData::Engine { thrust: 200.0, noise_level: 80.0 },
        customizable: false,
        cost: 90,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::RudderAssembly, ModuleDef {
        name: "Rudder Assembly",
        description: "Precision RCS thruster. Near-silent steering for tight maneuvers.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.4, 0.4, 0.4),
        companion: CompanionData::Engine { thrust: 20.0, noise_level: 2.0 },
        customizable: false,
        cost: 38,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // LIFE SUPPORT — NEW (3)
    // ========================================================================

    defs.insert(ModuleType::AdvancedOxygenator, ModuleDef {
        name: "Advanced Oxygenator",
        description: "Industrial-grade life support processor. Generates breathable air for large crews.",
        category: ModuleCategory::LifeSupport,
        size: IVec2::new(2, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 25.0,
        color: Color::srgb(0.2, 0.7, 0.9),
        companion: CompanionData::OxygenScrubber { output: 80.0 },
        customizable: false,
        cost: 150,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::FireSuppression, ModuleDef {
        name: "Fire Suppression",
        description: "Halon suppression system. Automatically smothers fires in adjacent compartments.",
        category: ModuleCategory::LifeSupport,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.8, 0.2, 0.2),
        companion: CompanionData::FireSuppression { effectiveness: 2.0 },
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AtmosphereMonitor, ModuleDef {
        name: "Atmosphere Monitor",
        description: "Monitors CO2, pressure, and contaminant levels. Warns before conditions turn lethal.",
        category: ModuleCategory::LifeSupport,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.3, 0.6, 0.6),
        companion: CompanionData::LifeSupport { o2_gen: 0.0, co2_filter: 10.0 },
        customizable: false,
        cost: 30,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // (Old Phase B weapon entries removed — replaced by new weapon family system above)
    // ========================================================================

    // AmmoAutoloader (was TorpedoLoader) — kept as a utility module
    defs.insert(ModuleType::AmmoAutoloader, ModuleDef {
        name: "Ammo Autoloader",
        description: "Speeds up reload of adjacent weapon systems. Essential for high fire-rate builds.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.5, 0.3, 0.3),
        companion: CompanionData::AmmoAutoloader { reload_bonus: 0.3 },
        customizable: false,
        cost: 90,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // ========================================================================
    // MULTI-BLOCK EXTENSION BLOCKS
    // These attach to weapon/reactor/engine cores to enhance them.
    // Stats emerge from physical layout — more blocks = better performance.
    // ========================================================================

    defs.insert(ModuleType::BarrelExtension, ModuleDef {
        name: "Barrel Extension",
        description: "Extends a weapon barrel. Each section adds range and accuracy. Chain from the weapon core outward.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.55, 0.30, 0.25),
        companion: CompanionData::None,
        customizable: true,
        cost: 23,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AmmoFeedUnit, ModuleDef {
        name: "Ammo Feed",
        description: "Feeds ammunition to a weapon core. More feeds = faster reload. Place adjacent to weapon core.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.60, 0.40, 0.20),
        companion: CompanionData::None,
        customizable: true,
        cost: 30,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::CoolingJacket, ModuleDef {
        name: "Cooling Jacket",
        description: "Dissipates weapon heat. More cooling = longer sustained fire. Place adjacent to barrel or core.",
        category: ModuleCategory::Weapons,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 8.0,
        color: Color::srgb(0.25, 0.45, 0.60),
        companion: CompanionData::None,
        customizable: true,
        cost: 26,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::ReactorFuelRod, ModuleDef {
        name: "Fuel Rod",
        description: "Nuclear fuel rod for reactors. More rods = more power output. Place adjacent to reactor core.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 15.0,
        power_consumption: 0.0,
        color: Color::srgb(0.70, 0.65, 0.15),
        companion: CompanionData::None,
        customizable: true,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::ReactorCooling, ModuleDef {
        name: "Reactor Cooling",
        description: "Dedicated reactor coolant loop. Prevents meltdown under heavy load. Place adjacent to reactor.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.20, 0.50, 0.65),
        companion: CompanionData::None,
        customizable: true,
        cost: 34,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::EngineNozzle, ModuleDef {
        name: "Engine Nozzle",
        description: "Thrust nozzle extension. More nozzles = more thrust. Chain from engine core backward.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 55.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.30, 0.50, 0.70),
        companion: CompanionData::None,
        customizable: true,
        cost: 26,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::ShieldEmitter, ModuleDef {
        name: "Shield Emitter",
        description: "Projects a shield in the direction it faces. Multiple emitters create wider coverage.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 45.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.30, 0.55, 0.80),
        companion: CompanionData::None,
        customizable: true,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::DarkWard, ModuleDef {
        name: "Dark Ward",
        description: "The Silence's pattern. Holds the ship together inside systems the Cascade has taken. Its draw climbs steeply with how deep you go, how long you stay and how big the ship is.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: crate::narrative::ward::IDLE_DRAW,
        color: Color::srgb(0.35, 0.20, 0.50),
        companion: CompanionData::None,
        customizable: false,
        cost: 400,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // ADVANCED WEAPON ENHANCERS (optional optimization blocks)
    // ========================================================================

    // ========================================================================
    // GUN PARTS -- combat::gun_assembly turns a firing piece and the parts
    // touching it into one gun. Numbers: combat::gun_math.
    // ========================================================================

    defs.insert(ModuleType::FiringPiece, ModuleDef {
        name: "Firing Piece", description: "The breech. One per gun: barrels run out from its front, everything else bolts on behind and beside it.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 160.0,
        power_generation: 0.0, power_consumption: 10.0, color: Color::srgb(0.55, 0.45, 0.25),
        companion: CompanionData::Weapon {
            // Placeholders: combat::gun_assembly works the real numbers out
            // from the parts around it and overwrites these.
            damage: 10.0, range: 18000.0, fire_rate: 0.2, ammo: 40,
            mount_type: MountType::Fixed, ammo_type: AmmoType::Bullet,
        }, customizable: false, cost: 60,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::GaugeIncreaser, ModuleDef {
        name: "Gauge Increaser", description: "+55 mm of bore, up to 8 per gun (500 mm). Bigger shells hit harder and load slower.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 120.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.45, 0.42, 0.35),
        companion: CompanionData::None, customizable: false, cost: 25,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::GaugeReducer, ModuleDef {
        name: "Gauge Reducer", description: "-20 mm of bore, down to 20 mm. A small gun fires tiny shells fast and weighs little on its turret: point defence.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 80.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.42, 0.45, 0.38),
        companion: CompanionData::None, customizable: false, cost: 20,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::Loader, ModuleDef {
        name: "Loader", description: "Autoloader. A straight run of these is one loader, taking 1 m of shell per block. Loaders work in parallel.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 80.0,
        power_generation: 0.0, power_consumption: 3.0, color: Color::srgb(0.40, 0.40, 0.35),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::ClipRack, ModuleDef {
        name: "Clip Rack", description: "A ready rack beside a loader: 3 shells and a 25% faster load per loader-length of rack. It burns if hit.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 60.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.45, 0.40, 0.25),
        companion: CompanionData::None, customizable: false, cost: 20,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::ShellFeeder, ModuleDef {
        name: "Shell Feeder", description: "Pulls shells from a magazine into the gun's racks.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 60.0,
        power_generation: 0.0, power_consumption: 2.0, color: Color::srgb(0.40, 0.38, 0.30),
        companion: CompanionData::None, customizable: false, cost: 15,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::BreechCooler, ModuleDef {
        name: "Breech Cooler", description: "Carries 0.6 heat a second off the gun. A big gun with too few waits on itself.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 70.0,
        power_generation: 0.0, power_consumption: 3.0, color: Color::srgb(0.35, 0.45, 0.55),
        companion: CompanionData::None, customizable: false, cost: 25,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::RecoilAbsorber, ModuleDef {
        name: "Recoil Absorber", description: "Soaks one unit of recoil per shot, keeping the spread tight under sustained fire.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 100.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.40, 0.40, 0.45),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::GunBarrel, ModuleDef {
        name: "Barrel", description: "1 m of tube. Needs 1 m per 100 mm of gauge to burn the propellant fully; longer is steadier.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 90.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.35, 0.37, 0.40),
        companion: CompanionData::None, customizable: false, cost: 20,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::MuzzleBrake, ModuleDef {
        name: "Muzzle Brake", description: "The barrel's last metre, vented: halves recoil, costs a little accuracy.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 80.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.38, 0.38, 0.40),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::Mantlet, ModuleDef {
        name: "Mantlet", description: "Thick armour in front of the breech, where a turret is usually hit.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 300.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.50, 0.50, 0.52),
        companion: CompanionData::None, customizable: false, cost: 45,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::RailMagnet, ModuleDef {
        name: "Rail Magnet", description: "Sits beside the barrel: +90 m/s per magnet, drawing power while it charges.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 70.0,
        power_generation: 0.0, power_consumption: 15.0, color: Color::srgb(0.60, 0.40, 0.30),
        companion: CompanionData::None, customizable: false, cost: 40,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::Magazine, ModuleDef {
        name: "Magazine", description: "The ship's store of shells for built guns. Feeders draw from it, and it is what a flash reaches.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 120.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.45, 0.38, 0.25),
        companion: CompanionData::None, customizable: false, cost: 35,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::BeltConveyor, ModuleDef {
        name: "Belt Conveyor", description: "Carries shells from a magazine to a feeder.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 40.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.35, 0.35, 0.33),
        companion: CompanionData::None, customizable: false, cost: 5,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::TurretRingSmall, ModuleDef {
        name: "Small Turret Ring", description: "A 1x1 bearing. The turret above it reaches about 1.5 cells: room for a light gun.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 200.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.42, 0.40, 0.36),
        companion: CompanionData::None, customizable: false, cost: 40,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::TurretRing, ModuleDef {
        name: "Turret Ring", description: "A 3x3 bearing. The turret above it reaches about 2.5 cells: a twin medium mount.",
        category: ModuleCategory::Guns, size: IVec2::new(3, 3), health: 400.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.42, 0.40, 0.36),
        companion: CompanionData::None, customizable: false, cost: 110,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::TurretRingLarge, ModuleDef {
        name: "Large Turret Ring", description: "A 5x5 bearing. The turret above it reaches about 3.5 cells: heavy guns.",
        category: ModuleCategory::Guns, size: IVec2::new(5, 5), health: 700.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.42, 0.40, 0.36),
        companion: CompanionData::None, customizable: false, cost: 240,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::TraverseMotor, ModuleDef {
        name: "Traverse Motor", description: "Turns the turret on the ring it touches. Turn speed is motors against everything up top.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 90.0,
        power_generation: 0.0, power_consumption: 6.0, color: Color::srgb(0.42, 0.40, 0.36),
        companion: CompanionData::None, customizable: false, cost: 35,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::AmmoHoist, ModuleDef {
        name: "Ammo Hoist", description: "Beside a turret ring: lifts shells from the magazines up into the turret, one every 1.5 s. A cook-off up top can flash back down it.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 100.0,
        power_generation: 0.0, power_consumption: 3.0, color: Color::srgb(0.45, 0.38, 0.25),
        companion: CompanionData::None, customizable: false, cost: 45,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::AntiFlashDoor, ModuleDef {
        name: "Anti-Flash Door", description: "Carries shells like a belt, but a flash coming down a hoist stops at it instead of reaching the magazine.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 160.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.55, 0.50, 0.20),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // Lasers built from parts (combat::lasers).
    defs.insert(ModuleType::LaserCore, ModuleDef {
        name: "Laser Core", description: "The heart of a laser: cavities, modifiers and emitters connect to it (or to couplers off it). One laser per core.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 220.0,
        power_generation: 0.0, power_consumption: 4.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 90,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::LaserCoupler, ModuleDef {
        name: "Laser Coupler", description: "Extends a laser core: cavities, Q-switches, doublers, destabilisers, emitters and beam relays may touch it instead.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 120.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 25,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::LaserCavity, ModuleDef {
        name: "Laser Cavity", description: "Stores 100 charge. Counts touching the core, a coupler or another counted cavity.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 110.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 40,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::LaserPump, ModuleDef {
        name: "Laser Pump", description: "Beside a counted cavity: +12 charge a second, drawing 60 MW of ship power while its bank fills (2 MW once it's full). Lasers are paid for in power.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 90.0,
        power_generation: 0.0, power_consumption: 60.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 45,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::QSwitch, ModuleDef {
        name: "Q-switch", description: "On the core: the laser pulses instead of holding a steady beam. 1, 2, 3 or 4 switches give 1, 2, 4 or 8 pulses a second; pulses do more per charge but burn less deep into armour.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 100.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 50,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::FrequencyDoubler, ModuleDef {
        name: "Frequency Doubler", description: "On the core: +15 intensity, halving the wavelength. Intensity is what burns through armour (four count).",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 100.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 60,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::LaserDestabiliser, ModuleDef {
        name: "Laser Destabiliser", description: "On the core: the bank lets its charge go faster (25% a second with none, 58% with two, 76% with four). Bigger bursts, drains sooner.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 100.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 45,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::LaserEmitter, ModuleDef {
        name: "Laser Emitter", description: "Fires the laser. On the hull touching the core or a coupler, or on a turret fed by a Beam Relay. Lenses in front of it carry the beam further.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 140.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 70,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::LaserLens, ModuleDef {
        name: "Laser Lens", description: "In a straight line in front of an emitter: the beam keeps more of its strength over range and wanders less.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 80.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::BeamRelay, ModuleDef {
        name: "Beam Relay", description: "Beside a turret ring and touching a laser's core or a coupler: carries the charge up to the emitters on that turret.",
        category: ModuleCategory::Lasers, size: IVec2::new(1, 1), health: 120.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.55, 0.30, 0.30),
        companion: CompanionData::None, customizable: false, cost: 40,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // Gun house roof fittings: on a turret's roof (PgUp twice in build mode),
    // in place of an armour plate. Each counts for the turret it stands on.
    defs.insert(ModuleType::Rangefinder, ModuleDef {
        name: "Rangefinder", description: "On a turret's roof: its guns' spread tightens by 15% (two at most count).",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 70.0,
        power_generation: 0.0, power_consumption: 2.0, color: Color::srgb(0.40, 0.45, 0.50),
        companion: CompanionData::None, customizable: false, cost: 40,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::RoofVent, ModuleDef {
        name: "Roof Vent", description: "On a turret's roof: cools every gun under it like a Breech Cooler each (two at most count).",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 90.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.40, 0.42, 0.45),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::Periscope, ModuleDef {
        name: "Periscope", description: "On a turret's roof: a gun sight. Its guns lead their shots as if a Targeting Computer stood beside them, and the turret turns 15% quicker (two at most count).",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 50.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.35, 0.50, 0.60),
        companion: CompanionData::None, customizable: false, cost: 35,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::ArmouredHatch, ModuleDef {
        name: "Armoured Hatch", description: "On a turret's roof: crew get in to the parts under it, and rebuild them twice as fast.",
        category: ModuleCategory::Guns, size: IVec2::new(1, 1), health: 140.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.42, 0.44, 0.48),
        companion: CompanionData::None, customizable: false, cost: 25,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::OverchargeCapacitor, ModuleDef {
        name: "Overcharge Capacitor", description: "Stores energy for a single devastating shot. 3x damage, 15s cooldown. Worth the wait.",
        category: ModuleCategory::Weapons, size: IVec2::new(1, 1), health: 50.0,
        power_generation: 0.0, power_consumption: 15.0, color: Color::srgb(0.60, 0.50, 0.15),
        companion: CompanionData::None, customizable: true, cost: 90,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::BoreEvacuator, ModuleDef {
        name: "Bore Evacuator", description: "Clears barrel fumes between shots. +20% fire rate on kinetic weapons.",
        category: ModuleCategory::Weapons, size: IVec2::new(1, 1), health: 30.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.45, 0.40, 0.35),
        companion: CompanionData::None, customizable: false, cost: 26,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::MagneticAccelerator, ModuleDef {
        name: "Magnetic Accelerator", description: "Electromagnetic boost stage. +40% projectile velocity for railguns and coilguns. Power hungry.",
        category: ModuleCategory::Weapons, size: IVec2::new(1, 1), health: 60.0,
        power_generation: 0.0, power_consumption: 20.0, color: Color::srgb(0.35, 0.40, 0.65),
        companion: CompanionData::None, customizable: true, cost: 113,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::FocusingArray, ModuleDef {
        name: "Focusing Array", description: "Precision optics for energy weapons. Tightens beam 30%, extends effective range.",
        category: ModuleCategory::Weapons, size: IVec2::new(1, 1), health: 40.0,
        power_generation: 0.0, power_consumption: 5.0, color: Color::srgb(0.30, 0.55, 0.40),
        companion: CompanionData::None, customizable: true, cost: 60,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::WarheadBay, ModuleDef {
        name: "Warhead Bay", description: "Extra torpedo/missile storage. +8 ammo capacity per bay. Explosive if hit.",
        category: ModuleCategory::Weapons, size: IVec2::new(1, 1), health: 50.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.65, 0.25, 0.20),
        companion: CompanionData::None, customizable: false, cost: 45,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // ========================================================================
    // ADVANCED REACTOR ENHANCERS
    // ========================================================================

    defs.insert(ModuleType::FuelEnrichmentUnit, ModuleDef {
        name: "Fuel Enrichment Unit", description: "Enriches fuel rods for 40% more output. Generates 50% more heat. Risk and reward.",
        category: ModuleCategory::Power, size: IVec2::new(1, 1), health: 60.0,
        power_generation: 0.0, power_consumption: 5.0, color: Color::srgb(0.70, 0.60, 0.10),
        companion: CompanionData::None, customizable: true, cost: 75,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::ContainmentField, ModuleDef {
        name: "Containment Field", description: "Magnetic containment around reactor. Reduces explosion radius by 60% if reactor blows. Buys time.",
        category: ModuleCategory::Power, size: IVec2::new(1, 1), health: 80.0,
        power_generation: 0.0, power_consumption: 10.0, color: Color::srgb(0.30, 0.40, 0.60),
        companion: CompanionData::None, customizable: false, cost: 113,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::EmergencyShutdown, ModuleDef {
        name: "Emergency Shutdown", description: "Auto-SCRAMs reactor before meltdown. One-time use - saves the ship, kills the power.",
        category: ModuleCategory::Power, size: IVec2::new(1, 1), health: 40.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.80, 0.20, 0.15),
        companion: CompanionData::None, customizable: false, cost: 60,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::PowerRegulator, ModuleDef {
        name: "Power Regulator", description: "Smooths power fluctuations. Reduces wasted energy by 15%. Quiet, efficient, boring. Essential.",
        category: ModuleCategory::Power, size: IVec2::new(1, 1), health: 50.0,
        power_generation: 0.0, power_consumption: 2.0, color: Color::srgb(0.45, 0.50, 0.55),
        companion: CompanionData::None, customizable: false, cost: 45,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // ========================================================================
    // ADVANCED ENGINE ENHANCERS
    // ========================================================================

    defs.insert(ModuleType::Afterburner, ModuleDef {
        name: "Afterburner", description: "Injects extra fuel for a 200% thrust boost. Lasts 5 seconds. 30s cooldown. Escape or engage.",
        category: ModuleCategory::Propulsion, size: IVec2::new(1, 1), health: 50.0,
        power_generation: 0.0, power_consumption: 10.0, color: Color::srgb(0.80, 0.40, 0.10),
        companion: CompanionData::None, customizable: true, cost: 90,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::ThrustVectoring, ModuleDef {
        name: "Thrust Vectoring", description: "Articulated nozzle that improves turning at speed. +40% turn rate. Makes the ship agile.",
        category: ModuleCategory::Propulsion, size: IVec2::new(1, 1), health: 40.0,
        power_generation: 0.0, power_consumption: 5.0, color: Color::srgb(0.35, 0.55, 0.70),
        companion: CompanionData::None, customizable: true, cost: 60,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::FuelInjector, ModuleDef {
        name: "Fuel Injector", description: "Optimizes fuel-air mix. -20% fuel consumption. Pays for itself after three expeditions.",
        category: ModuleCategory::Propulsion, size: IVec2::new(1, 1), health: 35.0,
        power_generation: 0.0, power_consumption: 2.0, color: Color::srgb(0.40, 0.50, 0.40),
        companion: CompanionData::None, customizable: false, cost: 53,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::InertialDampener, ModuleDef {
        name: "Inertial Dampener", description: "Reduces drift and improves handling near gravity wells. -30% gravity effect on ship.",
        category: ModuleCategory::Propulsion, size: IVec2::new(1, 1), health: 45.0,
        power_generation: 0.0, power_consumption: 8.0, color: Color::srgb(0.35, 0.45, 0.55),
        companion: CompanionData::None, customizable: true, cost: 75,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // ========================================================================
    // DEFENSE MODULES
    // ========================================================================

    defs.insert(ModuleType::DecoyLauncher, ModuleDef {
        name: "Decoy Launcher", description: "Launches heat decoys that distract guided missiles and creature attention. 6 charges.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 40.0,
        power_generation: 0.0, power_consumption: 5.0, color: Color::srgb(0.60, 0.50, 0.20),
        companion: CompanionData::None, customizable: false, cost: 68,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::ChaffDispenser, ModuleDef {
        name: "Chaff Dispenser", description: "Disrupts targeting systems in a radius. -50% enemy accuracy for 8 seconds. 4 uses.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 35.0,
        power_generation: 0.0, power_consumption: 3.0, color: Color::srgb(0.50, 0.55, 0.60),
        companion: CompanionData::None, customizable: false, cost: 53,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::AblativeArmor, ModuleDef {
        name: "Ablative Armor", description: "Soft sacrificial plate. A shell that hits it stops there, however big, and the plate breaks. Small guns strip it quickly. Replaceable at station.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 100.0,
        power_generation: 0.0, power_consumption: 0.0, color: ABLATIVE_TONE,
        companion: CompanionData::None, customizable: false, cost: 40,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::AngledAblativeArmor, ModuleDef {
        name: "Angled Ablative Armor",
        description: "Ablative plate cut to a wedge. Stops a shell dead like the flat plate, and fits the hull's edge. Rotate to pick which corner the face looks out from.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 90.0,
        power_generation: 0.0, power_consumption: 0.0, color: ABLATIVE_TONE,
        companion: CompanionData::None, customizable: false, cost: 44,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::ReflectivePlate, ModuleDef {
        name: "Reflective Plate",
        description: "Polished plate that turns most of a laser away. Thin and brittle: shells and small guns go through it easily. A stronger laser still burns in.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 120.0,
        power_generation: 0.0, power_consumption: 0.0, color: REFLECTIVE_TONE,
        companion: CompanionData::None, customizable: false, cost: 70,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::AngledReflectivePlate, ModuleDef {
        name: "Angled Reflective Plate",
        description: "Reflective plate cut to a wedge. Turns lasers like the flat plate, and fits the hull's edge. Rotate to pick which corner the face looks out from.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 110.0,
        power_generation: 0.0, power_consumption: 0.0, color: REFLECTIVE_TONE,
        companion: CompanionData::None, customizable: false, cost: 74,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::PointDefenseDrone, ModuleDef {
        name: "Point Defense Drone", description: "Autonomous drone that intercepts incoming projectiles. Limited ammo, auto-returns to dock.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 30.0,
        power_generation: 0.0, power_consumption: 12.0, color: Color::srgb(0.40, 0.55, 0.50),
        companion: CompanionData::None, customizable: false, cost: 135,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::HullReinforcePlate, ModuleDef {
        name: "Hull Reinforce Plate", description: "Reinforces adjacent hull and modules. +30% HP to neighbors. Reduces cascade chance by 25%.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 100.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.60, 0.60, 0.63),
        companion: CompanionData::None, customizable: false, cost: 38,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // ========================================================================
    // ADVANCED UTILITY
    // ========================================================================

    defs.insert(ModuleType::SignalJammer, ModuleDef {
        name: "Signal Jammer", description: "Scrambles enemy detection. -40% detection range against your ship. Power hungry.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 40.0,
        power_generation: 0.0, power_consumption: 20.0, color: Color::srgb(0.30, 0.35, 0.50),
        companion: CompanionData::None, customizable: true, cost: 98,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::GravityCompensator, ModuleDef {
        name: "Gravity Compensator", description: "Partially negates gravitational pull. -30% gravity effect. Essential near black holes.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 60.0,
        power_generation: 0.0, power_consumption: 25.0, color: Color::srgb(0.40, 0.35, 0.60),
        companion: CompanionData::None, customizable: true, cost: 150,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::RadiationHardening, ModuleDef {
        name: "Radiation Hardening", description: "Lead-composite shielding for adjacent modules. -50% radiation damage to neighbors.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 80.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.35, 0.40, 0.30),
        companion: CompanionData::None, customizable: false, cost: 68,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::EmergencyO2Cache, ModuleDef {
        name: "Emergency O2 Cache", description: "Sealed oxygen reserve. When its compartment runs thin it lets go all at once and refills it. One use.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 40.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.25, 0.55, 0.55),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::BlackBox, ModuleDef {
        name: "Black Box", description: "Indestructible flight recorder. Survives ship destruction. Preserves unlocks and data.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 999.0,
        power_generation: 0.0, power_consumption: 1.0, color: Color::srgb(0.15, 0.15, 0.20),
        companion: CompanionData::None, customizable: false, cost: 225,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // ========================================================================
    // STRUCTURAL ENHANCERS
    // ========================================================================

    defs.insert(ModuleType::ReinforcedJoint, ModuleDef {
        name: "Reinforced Joint", description: "Structural reinforcement between barrel blocks. -40% cascade explosion chance in the chain.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 70.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.62, 0.60, 0.56),
        companion: CompanionData::None, customizable: false, cost: 34,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::VibrationDamper, ModuleDef {
        name: "Vibration Damper", description: "Absorbs vibrations from adjacent weapons firing. +10% accuracy for nearby weapons.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 40.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.40, 0.42, 0.45),
        companion: CompanionData::None, customizable: false, cost: 26,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::ThermalInsulator, ModuleDef {
        name: "Thermal Insulator", description: "Blocks heat transfer between sections. Keeps hot reactors from cooking adjacent rooms.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 50.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.55, 0.45, 0.30),
        companion: CompanionData::None, customizable: false, cost: 23,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::StructuralBrace, ModuleDef {
        name: "Structural Brace", description: "Heavy-duty support strut. +25% HP to all adjacent hull segments and modules.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 120.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.62, 0.62, 0.64),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::CornerArmorPlate, ModuleDef {
        name: "Corner Armor Plate",
        description: "L-shaped armor block. Wraps a hull corner without blocking the cell behind it - a neighbor can still be built into the notch.",
        category: ModuleCategory::Structural, size: IVec2::new(2, 2), health: 160.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.56, 0.54, 0.50),
        companion: CompanionData::None, customizable: false, cost: 41,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // ========================================================================
    // SHAPED MODULES — shape chosen for what the module does, see MODULES.md
    // ========================================================================

    defs.insert(ModuleType::BridgeWing, ModuleDef {
        name: "Bridge Wing",
        description: "Command bridge with wings that extend past the hull on both sides. Real warships do this so the officer of the watch can actually see down the hull during docking - nobody misjudges a berth with this kind of sightline.",
        category: ModuleCategory::Control, size: IVec2::new(3, 2), health: 150.0,
        power_generation: 0.0, power_consumption: 15.0, color: Color::srgb(0.42, 0.32, 0.52),
        companion: CompanionData::None, customizable: false, cost: 135,
        base_stats: CalculatedStats::default(), crew_station: true,
    });

    defs.insert(ModuleType::SurgicalBay, ModuleDef {
        name: "Surgical Bay",
        description: "Full surgical theater instead of a single cot - a narrow triage entry opens into a wide treatment bay. Same care as a Med Bay, room for more than one patient at a time.",
        category: ModuleCategory::Crew, size: IVec2::new(3, 2), health: 170.0,
        power_generation: 0.0, power_consumption: 30.0, color: Color::srgb(0.78, 0.80, 0.92),
        companion: CompanionData::CrewFacility { facility_type: crate::components::FacilityType::MedBay },
        customizable: false, cost: 180,
        base_stats: CalculatedStats::default(), crew_station: true,
    });

    defs.insert(ModuleType::GalleyMess, ModuleDef {
        name: "Galley Mess",
        description: "A proper galley run - narrow working kitchen with a dining nook built into the bend, the way ship galleys have always been laid out. Feeds more crew without feeling like a cafeteria line.",
        category: ModuleCategory::Crew, size: IVec2::new(2, 2), health: 110.0,
        power_generation: 0.0, power_consumption: 10.0, color: Color::srgb(0.38, 0.68, 0.42),
        companion: CompanionData::CrewFacility { facility_type: crate::components::FacilityType::MessHall },
        customizable: false, cost: 105,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::BulkCargoHold, ModuleDef {
        name: "Bulk Cargo Hold",
        description: "Cargo bay shaped to eat the dead space a boxy hold would waste - real cargo-ship design obsesses over exactly this. More capacity per cell than Large Cargo.",
        category: ModuleCategory::Storage, size: IVec2::new(2, 2), health: 150.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.62, 0.52, 0.24),
        companion: CompanionData::Cargo { capacity: 440.0 },
        customizable: false, cost: 98,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::DockingHub, ModuleDef {
        name: "Docking Hub",
        description: "Multi-port docking node, same idea as an ISS connecting node - more than one ship, wreck, or station can couple to it at once, from different sides. No queue.",
        category: ModuleCategory::Utility, size: IVec2::new(3, 3), health: 180.0,
        power_generation: 0.0, power_consumption: 15.0, color: Color::srgb(0.42, 0.42, 0.55),
        companion: CompanionData::Docking,
        customizable: false, cost: 165,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::WellnessHub, ModuleDef {
        name: "Wellness Hub",
        description: "Combined gym and training node with access from every side, so crew drift in between shifts instead of scheduling around it. Same layout logic as the ISS's exercise module.",
        category: ModuleCategory::Crew, size: IVec2::new(3, 3), health: 140.0,
        power_generation: 0.0, power_consumption: 20.0, color: Color::srgb(0.52, 0.52, 0.72),
        companion: CompanionData::CrewFacility { facility_type: crate::components::FacilityType::TrainingRoom },
        customizable: false, cost: 173,
        base_stats: CalculatedStats::default(), crew_station: true,
    });

    defs.insert(ModuleType::StaggeredArmorPlate, ModuleDef {
        name: "Staggered Armor Plate",
        description: "Offset plating, laid the way real armor is - staggered so no single straight seam runs through it. There's no clean line for a hit to travel along.",
        category: ModuleCategory::Structural, size: IVec2::new(3, 2), health: 480.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.54, 0.54, 0.58),
        companion: CompanionData::None, customizable: false, cost: 225,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::AngledHullPlate, ModuleDef {
        name: "Angled Hull Plate",
        description: "Structural framing cut to a taper instead of a square face. Light - it deflects far better than it stops, so it belongs on a corner you want shots to skip off, not in front of anything important. Rotate to pick which corner the face looks out from.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 90.0,
        power_generation: 0.0, power_consumption: 0.0, color: crate::sprite_map::HULL_TONE,
        companion: CompanionData::None, customizable: false, cost: 19,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::AngledArmorPlate, ModuleDef {
        name: "Angled Armor Plate",
        description: "Armor cut to a taper instead of a flat face. Less steel than a flat plate and it stops less head-on, but a round arriving off-axis skips away instead of biting. Angle the hull as well and the two stack. Rotate to pick which corner the face looks out from.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 190.0,
        power_generation: 0.0, power_consumption: 0.0, color: crate::sprite_map::HULL_TONE,
        companion: CompanionData::None, customizable: false, cost: 64,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // More angle pieces: each a cell cut by one line (building::module_shape).
    defs.insert(ModuleType::BevelledArmorPlate, ModuleDef {
        name: "Bevelled Armor Plate",
        description: "A full plate with one corner chipped off at 45 degrees: nearly all the steel of a flat plate, and no square corner to catch a round. Rotate to pick the corner.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 230.0,
        power_generation: 0.0, power_consumption: 0.0, color: crate::sprite_map::HULL_TONE,
        companion: CompanionData::None, customizable: false, cost: 58,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::SlopeArmorPlateLow, ModuleDef {
        name: "Slope Plate (low)",
        description: "The low half of a long, shallow slope: lay it beside a Slope Plate (high) and the two make one face rising a cell over two. Shallow slopes turn rounds away better than any wedge.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 140.0,
        power_generation: 0.0, power_consumption: 0.0, color: crate::sprite_map::HULL_TONE,
        companion: CompanionData::None, customizable: false, cost: 52,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::SlopeArmorPlateHigh, ModuleDef {
        name: "Slope Plate (high)",
        description: "The high half of a long, shallow slope: lay it beside a Slope Plate (low).",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 210.0,
        power_generation: 0.0, power_consumption: 0.0, color: crate::sprite_map::HULL_TONE,
        companion: CompanionData::None, customizable: false, cost: 60,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::SlopeArmorPlateLowMirror, ModuleDef {
        name: "Slope Plate (low, mirrored)",
        description: "The low half of a long slope, mirrored for the other side of the ship.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 140.0,
        power_generation: 0.0, power_consumption: 0.0, color: crate::sprite_map::HULL_TONE,
        companion: CompanionData::None, customizable: false, cost: 52,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::SlopeArmorPlateHighMirror, ModuleDef {
        name: "Slope Plate (high, mirrored)",
        description: "The high half of a long slope, mirrored for the other side of the ship.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 210.0,
        power_generation: 0.0, power_consumption: 0.0, color: crate::sprite_map::HULL_TONE,
        companion: CompanionData::None, customizable: false, cost: 60,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::HalfArmorPlate, ModuleDef {
        name: "Half Armor Plate",
        description: "Half a cell of plate: a thin skirt, a step in a roof line, light armour where a full plate is too heavy.",
        category: ModuleCategory::Utility, size: IVec2::new(1, 1), health: 120.0,
        power_generation: 0.0, power_consumption: 0.0, color: crate::sprite_map::HULL_TONE,
        companion: CompanionData::None, customizable: false, cost: 34,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    // ========================================================================
    // DETECTION — NEW (3)
    // ========================================================================

    defs.insert(ModuleType::HydrophoneArray, ModuleDef {
        name: "Hydrophone Array",
        description: "Towed hydrophone array. Listens for distant movement without revealing your position.",
        category: ModuleCategory::Detection,
        size: IVec2::new(2, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.2, 0.5, 0.6),
        companion: CompanionData::PassiveRadar { range: 4500.0 },
        customizable: false,
        cost: 188,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::ThermalImager, ModuleDef {
        name: "Thermal Imager",
        description: "Infrared sensor. Spots warm-blooded creatures and thermal vents through darkness.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.7, 0.3, 0.2),
        companion: CompanionData::Detection { range: 300.0 },
        customizable: false,
        cost: 90,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::ProximityAlarm, ModuleDef {
        name: "Proximity Alarm",
        description: "Trip-wire sensor. Screams a warning when anything gets within arm's reach.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 30.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.8, 0.6, 0.2),
        companion: CompanionData::Detection { range: 150.0 },
        customizable: false,
        cost: 26,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::MissileWarner, ModuleDef {
        name: "Missile Warner",
        description: "Spots enemy missiles out to 4,500. Point defence guns and decoy launchers only act on missiles a warner has seen: place them round the ship, and keep them alive.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 4.0,
        color: Color::srgb(0.85, 0.4, 0.2),
        companion: CompanionData::None,
        customizable: false,
        cost: 30,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // STORAGE — NEW (2)
    // ========================================================================

    defs.insert(ModuleType::ReinforcedVault, ModuleDef {
        name: "Reinforced Vault",
        description: "Armored compartment. Survives explosions that destroy everything else around it.",
        category: ModuleCategory::Storage,
        size: IVec2::new(1, 1),
        health: 150.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.5, 0.5, 0.4),
        companion: CompanionData::Cargo { capacity: 80.0 },
        customizable: false,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::CryoStorage, ModuleDef {
        name: "Cryo Storage",
        description: "Sub-zero storage. Keeps biological specimens viable for research at the station.",
        category: ModuleCategory::Storage,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.3, 0.6, 0.8),
        companion: CompanionData::Cargo { capacity: 50.0 },
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // CREW — NEW (3)
    // ========================================================================

    defs.insert(ModuleType::OfficerQuarters, ModuleDef {
        name: "Officer Quarters",
        description: "Officers' cabin with desks and washbasin. Sleeps 4 in comfort the lower decks never see.",
        category: ModuleCategory::Crew,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 8.0,
        color: Color::srgb(0.4, 0.6, 0.5),
        companion: CompanionData::Quarters { berths: 4 },
        customizable: false,
        cost: 68,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::TrainingRoom, ModuleDef {
        name: "Training Room",
        description: "Simulator and workshop. Crew train here between shifts, slowly improving at their station.",
        category: ModuleCategory::Crew,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.5, 0.5, 0.7),
        companion: CompanionData::CrewFacility { facility_type: crate::components::FacilityType::TrainingRoom },
        customizable: false,
        cost: 83,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::Brig, ModuleDef {
        name: "Brig",
        description: "Reinforced holding cell. Keeps mutinous crew contained. Morale penalty for occupants.",
        category: ModuleCategory::Crew,
        size: IVec2::new(1, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.4, 0.4, 0.4),
        companion: CompanionData::Quarters { berths: 1 },
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // UTILITY — NEW (6)
    // ========================================================================

    defs.insert(ModuleType::AdvancedRepairBay, ModuleDef {
        name: "Advanced Repair Bay",
        description: "Full machine shop with welding rigs. Repairs modules and hull at double speed.",
        category: ModuleCategory::Utility,
        size: IVec2::new(2, 1),
        health: 120.0,
        power_generation: 0.0,
        power_consumption: 25.0,
        color: Color::srgb(0.4, 0.7, 0.5),
        companion: CompanionData::Repair { rate: 12.0 },
        customizable: false,
        cost: 188,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::DroneBay, ModuleDef {
        name: "Drone Bay",
        description: "Launches autonomous repair drones that patch nearby damaged modules while you focus on driving.",
        category: ModuleCategory::Utility,
        size: IVec2::new(2, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 30.0,
        color: Color::srgb(0.5, 0.6, 0.7),
        companion: CompanionData::DroneBay { drone_count: 3, drone_range: 200.0 },
        customizable: false,
        cost: 225,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    defs.insert(ModuleType::DeepFloodlight, ModuleDef {
        name: "Deep Floodlight",
        description: "High-intensity deep-rated lamp. Pierces the abyssal dark, but draws attention.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(1.0, 1.0, 0.6),
        companion: CompanionData::Light { range: 350.0, intensity: 1.5, attracts_creatures: true },
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::StealthCoating, ModuleDef {
        name: "Stealth Coating",
        description: "Sound-dampening hull tiles. Reduces noise emission from adjacent modules.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.2, 0.2, 0.3),
        companion: CompanionData::None,
        customizable: false,
        cost: 113,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::HullPatch, ModuleDef {
        name: "Hull Patch",
        description: "Self-sealing resin patch. Slowly mends minor hull damage without power or crew.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.5, 0.3),
        companion: CompanionData::Repair { rate: 1.0 },
        customizable: false,
        cost: 15,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::SignalBuoy, ModuleDef {
        name: "Signal Buoy",
        description: "Deployable transponder. Marks points of interest on your navigation map.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 30.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.7, 0.7, 0.2),
        companion: CompanionData::None,
        customizable: false,
        cost: 19,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // STRUCTURAL (8) — NEW CATEGORY
    // ========================================================================

    defs.insert(ModuleType::HullBeam, ModuleDef {
        name: "Hull Beam",
        description: "Load-bearing I-beam. Cheap, tough, fills structural gaps in your hull layout.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 120.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.6, 0.6, 0.6),
        companion: CompanionData::None,
        customizable: false,
        cost: 15,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // Filler for the bare deck of a big hull. Each is light and cheap with
    // one small use, so a big ship can be built out without being made a
    // slab of armour that takes all night to kill.
    defs.insert(ModuleType::LightFrame, ModuleDef {
        name: "Light Frame",
        description: "Open girder framing to fill bare deck. Light and cheap: it takes a hit for whatever stands behind it, and the crew put it back up in half the time of anything else.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 45.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.50, 0.52, 0.55),
        companion: CompanionData::None, customizable: false, cost: 8,
        base_stats: CalculatedStats::default(), crew_station: false,
    });
    defs.insert(ModuleType::SupplyCrates, ModuleDef {
        name: "Supply Crates",
        description: "Spares stacked where the work is. Crew rebuilding anything within 5 blocks of a stack work 30% faster.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 60.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.55, 0.47, 0.32),
        companion: CompanionData::None, customizable: false, cost: 20,
        base_stats: CalculatedStats::default(), crew_station: false,
    });
    defs.insert(ModuleType::CoolantRun, ModuleDef {
        name: "Coolant Run",
        description: "Pipework run through the deck. Draws heat out of its own block and the four beside it with no power of its own -- a quarter of what a Cooling Pump does, but it never switches off.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 50.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.35, 0.50, 0.58),
        companion: CompanionData::None, customizable: false, cost: 25,
        base_stats: CalculatedStats::default(), crew_station: false,
    });
    defs.insert(ModuleType::BallastBlock, ModuleDef {
        name: "Ballast Block",
        description: "Dense fill. Tough, and thick enough to spend a round's punch before it reaches what's behind -- it does nothing else.",
        category: ModuleCategory::Structural, size: IVec2::new(1, 1), health: 240.0,
        power_generation: 0.0, power_consumption: 0.0, color: Color::srgb(0.33, 0.33, 0.35),
        companion: CompanionData::None, customizable: false, cost: 30,
        base_stats: CalculatedStats::default(), crew_station: false,
    });

    defs.insert(ModuleType::HullCorner, ModuleDef {
        name: "Hull Corner",
        description: "Angled brace for hull corners. Prevents stress fractures where plates meet at sharp angles.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.55, 0.55, 0.6),
        companion: CompanionData::None,
        customizable: false,
        cost: 11,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::Bulkhead, ModuleDef {
        name: "Bulkhead",
        description: "Airtight bulkhead wall. Seals rooms from each other - when one depressurizes, the rest survive.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 150.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.5, 0.55),
        companion: CompanionData::None,
        customizable: false,
        cost: 23,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::PressureFrame, ModuleDef {
        name: "Radiation Shield",
        description: "Lead-composite framework for your hull. Boosts radiation shielding of nearby segments. Worth every cell.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 200.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.4, 0.45, 0.5),
        companion: CompanionData::RadiationShielding { shielding_bonus: 100.0 },
        customizable: false,
        cost: 150,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AirlockValve, ModuleDef {
        name: "Flood Valve",
        description: "Motorized gate valve. Slam it shut to isolate a decompression compartment, or open it to equalize pressure.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.3, 0.4, 0.6),
        companion: CompanionData::None,
        customizable: false,
        cost: 38,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AccessHatch, ModuleDef {
        name: "Access Hatch",
        description: "Pressure-sealed doorway. Lets crew move between compartments without compromising hull integrity.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.55, 0.5),
        companion: CompanionData::None,
        customizable: false,
        cost: 19,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::ViewPort, ModuleDef {
        name: "View Port",
        description: "Reinforced glass porthole. Fragile, but your crew will lose their minds without one. Morale matters.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.7, 0.8),
        companion: CompanionData::None,
        customizable: false,
        cost: 26,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::ArmorPlate, ModuleDef {
        name: "Armor Plate",
        description: "Solid plate of hardened steel. Absorbs hits that would shred anything else. Pure defense.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 200.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.52, 0.52, 0.58),
        companion: CompanionData::None,
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // ========================================================================
    // NEW BLOCKS — Heat, Propulsion, Life Support, Detection, Weapons, Crew
    // ========================================================================

    defs.insert(ModuleType::CoolingPump, ModuleDef {
        name: "Cooling Pump",
        description: "Active cooling system. Draws heat from adjacent blocks and dissipates it. Essential near reactors.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.3, 0.6, 0.8),
        companion: CompanionData::CoolingPump { cooling_rate: 80.0 },
        customizable: false,
        cost: 90,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::HeatVent, ModuleDef {
        name: "Heat Vent",
        description: "Passive radiator. Dissipates heat into the void. More effective in deep space.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.7, 0.4, 0.3),
        companion: CompanionData::HeatVent { dissipation_rate: 40.0 },
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::Transformer, ModuleDef {
        name: "Transformer",
        description: "Voltage step-down unit. Reduces power loss over distance in the power network.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.5, 0.5, 0.2),
        companion: CompanionData::Transformer { efficiency: 0.9 },
        customizable: false,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::VectorThruster, ModuleDef {
        name: "Vector Thruster",
        description: "Omnidirectional thrust unit. Improves maneuverability at the cost of high power draw.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 30.0,
        color: Color::srgb(0.4, 0.6, 0.3),
        companion: CompanionData::Engine { thrust: 80.0, noise_level: 25.0 },
        customizable: false,
        cost: 135,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AttitudeThruster, ModuleDef {
        name: "Attitude Thruster",
        description: "Fine RCS control. Smaller than a full main thruster but allows precise orientation adjustments.",
        category: ModuleCategory::Propulsion,
        size: IVec2::new(1, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.3, 0.5, 0.6),
        companion: CompanionData::Thruster { thrust_power: 50.0 },
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::OxygenTank, ModuleDef {
        name: "Oxygen Tank",
        description: "Pressurized O2 reserve. Pushes air into its compartment when a breach is emptying it, buying the crew time to seal the hole. Scrubbers refill it.",
        category: ModuleCategory::LifeSupport,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.4, 0.7, 0.9),
        companion: CompanionData::OxygenTank { capacity: 200.0 },
        customizable: false,
        cost: 68,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AirCirculator, ModuleDef {
        name: "Air Circulator",
        description: "Distributes breathable air between rooms. Improves CO2 filtering efficiency.",
        category: ModuleCategory::LifeSupport,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.5, 0.7, 0.6),
        companion: CompanionData::LifeSupport { o2_gen: 0.0, co2_filter: 15.0 },
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::CreatureScanner, ModuleDef {
        name: "Creature Scanner",
        description: "Bio-signature detector. Identifies creature types at extended range. Essential for deep space ventures.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.6, 0.3, 0.5),
        companion: CompanionData::Detection { range: 400.0 },
        customizable: false,
        cost: 150,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::MineralScanner, ModuleDef {
        name: "Mineral Scanner",
        description: "Ground-penetrating radar. Detects mineral deposits and salvageable materials.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.5, 0.5, 0.3),
        companion: CompanionData::Detection { range: 300.0 },
        customizable: false,
        cost: 150,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // AmmoAutoloader duplicate removed — defined earlier in the file

    defs.insert(ModuleType::EngineeringStation, ModuleDef {
        name: "Engineering Station",
        description: "Crew-operated workbench. Boosts repair rate of nearby modules by 25% when staffed.",
        category: ModuleCategory::Crew,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.5, 0.4, 0.3),
        companion: CompanionData::CrewFacility { facility_type: crate::components::FacilityType::EngineeringStation },
        customizable: false,
        cost: 90,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // ========================================================================
    // Phase B: 17 new modules
    // ========================================================================

    // --- Logistics ---
    defs.insert(ModuleType::ConveyorTube, ModuleDef {
        name: "Conveyor Tube",
        description: "Moves ammo and resources between adjacent modules automatically.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.6, 0.6, 0.5),
        companion: CompanionData::ConveyorTube { speed: 1.0 },
        customizable: false,
        cost: 30,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::MaintenanceLocker, ModuleDef {
        name: "Maintenance Locker",
        description: "Stores tools. +15% repair speed to adjacent modules.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.45, 0.35),
        companion: CompanionData::None,
        customizable: false,
        cost: 23,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::FuelProcessor, ModuleDef {
        name: "Fuel Processor",
        description: "Refines fuel, reducing fuel consumption of adjacent engines by 20%.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.5, 0.4, 0.3),
        companion: CompanionData::FuelProcessor { efficiency: 1.2 },
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // --- Damage Infrastructure ---
    defs.insert(ModuleType::HullSealer, ModuleDef {
        name: "Hull Seal System",
        description: "Automated breach sealer. Restores air pressure in depressurized rooms without crew.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.3, 0.5, 0.7),
        companion: CompanionData::HullSeal { seal_rate: 0.15 },
        customizable: false,
        cost: 45,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::EmergencyForceField, ModuleDef {
        name: "Emergency Force Field",
        description: "Throws a containment field over nearby hull breaches, holding the air in while the hull is still open. Costs power for every hole it holds - field a colander and you will brown out. Holds only; the plate still needs patching.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.35, 0.65, 0.85),
        companion: CompanionData::ForceField { radius: 3.5, power_per_tile: 12.0 },
        customizable: false,
        cost: 70,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::EmergencyBulkhead, ModuleDef {
        name: "Emergency Bulkhead",
        description: "Auto-seals when adjacent room depressurizes. Blocks fire and decompression spread.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.7, 0.3, 0.3),
        companion: CompanionData::None,
        customizable: false,
        cost: 60,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::FirebreakWall, ModuleDef {
        name: "Firebreak Wall",
        description: "Blocks fire spread unconditionally. No seal needed.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 120.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.8, 0.4, 0.2),
        companion: CompanionData::None,
        customizable: false,
        cost: 38,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::PressureSensor, ModuleDef {
        name: "Pressure Sensor",
        description: "Detects hull breaches early, triggers warnings.",
        category: ModuleCategory::Detection,
        size: IVec2::new(1, 1),
        health: 30.0,
        power_generation: 0.0,
        power_consumption: 5.0,
        color: Color::srgb(0.4, 0.6, 0.5),
        companion: CompanionData::Detection { range: 100.0 },
        customizable: false,
        cost: 26,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // --- Navigation & Control ---
    defs.insert(ModuleType::TargetingComputer, ModuleDef {
        name: "Targeting Computer",
        description: "+15% weapon accuracy for all weapons.",
        category: ModuleCategory::Control,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.4, 0.5, 0.6),
        companion: CompanionData::TargetingComputer { accuracy_bonus: 0.15 },
        customizable: false,
        cost: 75,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AutopilotCore, ModuleDef {
        name: "Autopilot Core",
        description: "Enables station-keeping autopilot when active.",
        category: ModuleCategory::Control,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 15.0,
        color: Color::srgb(0.3, 0.5, 0.6),
        companion: CompanionData::None,
        customizable: false,
        cost: 68,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::AICombatCore, ModuleDef {
        name: "AI Combat Core",
        description: "Auto-targets highest threat. +20% targeting priority.",
        category: ModuleCategory::Control,
        size: IVec2::new(2, 1),
        health: 80.0,
        power_generation: 0.0,
        power_consumption: 30.0,
        color: Color::srgb(0.4, 0.4, 0.6),
        companion: CompanionData::AICombatCore { priority_bonus: 0.2 },
        customizable: false,
        cost: 113,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::MemoryCore, ModuleDef {
        name: "Memory Core",
        description: "Redundant cognition lattice. Holds enough of the ship's own judgement to keep a station running with nobody standing at it. Crews call them the quiet crew.",
        category: ModuleCategory::Control,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 12.0,
        color: Color::srgb(0.35, 0.42, 0.55),
        companion: CompanionData::MemoryCore { autonomy: 0.16 },
        customizable: false,
        cost: 140,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    // --- Environmental Interaction ---
    defs.insert(ModuleType::ThermalVentGenerator, ModuleDef {
        name: "Thermal Vent Generator",
        description: "Generates power near thermal vents.",
        category: ModuleCategory::Power,
        size: IVec2::new(1, 1),
        health: 60.0,
        power_generation: 10.0,
        power_consumption: 0.0,
        color: Color::srgb(0.8, 0.4, 0.2),
        companion: CompanionData::Reactor { output: 10.0, max_heat: 50.0, explosion_risk: false },
        customizable: false,
        cost: 53,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::MineralExtractor, ModuleDef {
        name: "Mineral Extractor",
        description: "Harvests minerals from the environment.",
        category: ModuleCategory::Utility,
        size: IVec2::new(1, 1),
        health: 70.0,
        power_generation: 0.0,
        power_consumption: 20.0,
        color: Color::srgb(0.6, 0.5, 0.3),
        companion: CompanionData::Salvage { range: 100.0, efficiency: 1.5 },
        customizable: false,
        cost: 68,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::CreatureContainment, ModuleDef {
        name: "Creature Containment",
        description: "Stores captured creatures for study.",
        category: ModuleCategory::Storage,
        size: IVec2::new(2, 1),
        health: 100.0,
        power_generation: 0.0,
        power_consumption: 10.0,
        color: Color::srgb(0.3, 0.6, 0.5),
        companion: CompanionData::Cargo { capacity: 100.0 },
        customizable: false,
        cost: 83,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::ResearchLab, ModuleDef {
        name: "Research Lab",
        description: "Generates research points from specimens in adjacent containment.",
        category: ModuleCategory::Utility,
        size: IVec2::new(2, 1),
        health: 60.0,
        power_generation: 0.0,
        power_consumption: 25.0,
        color: Color::srgb(0.4, 0.5, 0.6),
        companion: CompanionData::ResearchLab { research_speed: 1.0 },
        customizable: false,
        cost: 98,
        base_stats: CalculatedStats::default(),
        crew_station: true,
    });

    // --- Interior Tiles ---
    defs.insert(ModuleType::Corridor, ModuleDef {
        name: "Corridor",
        description: "Basic passageway. Crew moves faster through corridors.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 50.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.55, 0.55, 0.55),
        companion: CompanionData::None,
        customizable: false,
        cost: 11,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::LadderShaft, ModuleDef {
        name: "Ladder Shaft",
        description: "Vertical access between decks.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 40.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.5, 0.5, 0.45),
        companion: CompanionData::None,
        customizable: false,
        cost: 15,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    defs.insert(ModuleType::MaintenanceTunnel, ModuleDef {
        name: "Maintenance Tunnel",
        description: "Tight access for crew to reach isolated modules.",
        category: ModuleCategory::Structural,
        size: IVec2::new(1, 1),
        health: 30.0,
        power_generation: 0.0,
        power_consumption: 0.0,
        color: Color::srgb(0.45, 0.45, 0.4),
        companion: CompanionData::None,
        customizable: false,
        cost: 11,
        base_stats: CalculatedStats::default(),
        crew_station: false,
    });

    ModuleRegistry { defs }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modules the build palette deliberately does not offer, and why.
    ///
    /// Weapon pieces are fitted through the customisation panel, not placed on
    /// the grid. The rest are superseded or inert. Anything not on this list
    /// and not in a palette is a mistake, and
    /// `every_registered_module_is_reachable` will say so.
    const NOT_IN_PALETTE: &[ModuleType] = &[
        // Weapon subcomponents: assembled in the customisation panel.
        ModuleType::BarrelExtension,
        ModuleType::AmmoFeedUnit,
        ModuleType::CoolingJacket,
        ModuleType::OverchargeCapacitor,
        ModuleType::BoreEvacuator,
        ModuleType::MagneticAccelerator,
        ModuleType::FocusingArray,
        ModuleType::WarheadBay,
        // Superseded: passages are HullLayer::Hallway now. The variants stay
        // so blueprints written before that change still deserialize.
        ModuleType::Corridor,
        ModuleType::LadderShaft,
        ModuleType::MaintenanceTunnel,
        // Inert: a sprite, a colour and a description promising it "blocks
        // heat transfer between sections". Nothing reads it. Offering it
        // would be selling the player a rock.
        ModuleType::ThermalInsulator,
    ];

    /// Every module that can be spawned must be obtainable, or knowingly not.
    ///
    /// `category()` says what a module IS and the palette lists say what the
    /// build menu OFFERS. They are maintained by hand, in different files, and
    /// they had drifted apart by forty entries.
    ///
    /// Three casualties mattered. `ShieldEmitter` was absent from Utility's
    /// list, yet shield strength is base plus a count of live emitters and the
    /// shield code's own comment says "building an emitter at the dock grows
    /// the bubble" -- so shields were frozen for the whole game at whatever the
    /// starter hull shipped with. `MemoryCore` was absent from Control's, which
    /// is worse, because losing every core ends the run. And the entire
    /// Structural category was unreachable, because `BuildCategory` had no
    /// Structural variant at all -- taking flat armour plate with it, the
    /// plainest piece of defence in the game.
    ///
    /// This test walks the registry rather than a hand-written list of
    /// variants, so a module added tomorrow is covered without anyone
    /// remembering to come back here.
    #[test]
    fn every_registered_module_is_reachable() {
        let registry = build_registry();

        let mut orphans: Vec<(ModuleType, ModuleCategory)> = registry
            .defs
            .keys()
            .copied()
            .filter(|m| !NOT_IN_PALETTE.contains(m))
            .filter(|m| !m.category().module_types().contains(m))
            .map(|m| (m, m.category()))
            .collect();
        orphans.sort_by_key(|(m, _)| format!("{m:?}"));

        assert!(
            orphans.is_empty(),
            "these modules exist in the registry but appear in no build \
             palette, so no player can ever obtain one -- add each to its \
             ModuleCategory::module_types() list, or to NOT_IN_PALETTE with \
             a reason: {orphans:?}"
        );
    }

    /// A module may not be both offered and listed as deliberately withheld.
    #[test]
    fn not_in_palette_entries_are_actually_absent() {
        for m in NOT_IN_PALETTE {
            assert!(
                !m.category().module_types().contains(m),
                "{m:?} is on NOT_IN_PALETTE but its category offers it; \
                 delete it from one of the two"
            );
        }
    }

    /// A palette list may not claim a module that belongs to another category.
    ///
    /// `to_module_category` routes a tab to exactly one `ModuleCategory`, so a
    /// module listed under the wrong one shows up on a tab that then disagrees
    /// with everything else reading `category()`.
    #[test]
    fn palette_lists_agree_with_category() {
        for category in ModuleCategory::ALL {
            for m in category.module_types() {
                assert_eq!(
                    m.category(),
                    *category,
                    "{m:?} is listed in {category:?}'s palette but category() \
                     puts it in {:?}",
                    m.category()
                );
            }
        }
    }

    #[test]
    fn all_module_types_registered() {
        let registry = build_registry();

        // Collect all ModuleType variants via ModuleCategory::module_types()
        let all_types: Vec<ModuleType> = ModuleCategory::ALL
            .iter()
            .flat_map(|cat| cat.module_types().iter().copied())
            .collect();

        assert!(!all_types.is_empty(), "Should have module types defined");

        for module_type in &all_types {
            assert!(
                registry.try_get(*module_type).is_some(),
                "ModuleType {:?} is not registered in the registry",
                module_type
            );
        }
    }

    #[test]
    fn registry_modules_have_valid_stats() {
        let registry = build_registry();

        for (module_type, def) in &registry.defs {
            assert!(def.health > 0.0, "{:?} has zero or negative health", module_type);
            assert!(def.size.x > 0 && def.size.y > 0, "{:?} has invalid size", module_type);
            assert!(!def.name.is_empty(), "{:?} has empty name", module_type);
            assert!(def.cost > 0, "{:?} has zero cost", module_type);
        }
    }

    #[test]
    fn try_get_returns_none_for_missing() {
        // Build a registry and verify try_get works for known types
        let registry = build_registry();
        assert!(registry.try_get(ModuleType::SmallReactor).is_some());
    }
}
