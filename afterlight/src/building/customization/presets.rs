use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use super::parameters::ModuleCustomization;
use crate::components::ModuleType;

// ============================================================================
// PRESET SYSTEM
// One-click configurations for players who don't want to touch Tier 3.
// Each preset sets all parameters to a curated configuration.
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub description: String,
    pub slot_selections: HashMap<String, usize>,
    pub parameter_values: HashMap<String, f32>,
}

impl Preset {
    pub fn apply(&self, customization: &mut ModuleCustomization) {
        customization.slot_selections = self.slot_selections.clone();
        customization.parameter_values = self.parameter_values.clone();
    }
}

/// Built-in presets for cannon
pub fn cannon_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Balanced".into(),
            description: "Default configuration. Good at everything, great at nothing.".into(),
            slot_selections: HashMap::from([
                ("Barrel".into(), 0),      // Standard
                ("Ammunition".into(), 0),  // AP
                ("Feed Mechanism".into(), 1), // Belt Feed
                ("Cooling".into(), 0),     // Passive
                ("Mount".into(), 1),       // Turret
                ("Targeting".into(), 1),   // Basic Lead
            ]),
            parameter_values: HashMap::new(), // All defaults
        },
        Preset {
            name: "Sniper".into(),
            description: "Maximum range and accuracy. Slow fire rate. Picks off targets at distance.".into(),
            slot_selections: HashMap::from([
                ("Barrel".into(), 1),      // Long Range
                ("Ammunition".into(), 0),  // AP
                ("Feed Mechanism".into(), 0), // Manual (for precision)
                ("Cooling".into(), 0),     // Passive (low fire rate = low heat)
                ("Mount".into(), 1),       // Turret
                ("Targeting".into(), 2),   // Advanced Tracking
            ]),
            parameter_values: HashMap::from([
                ("Barrel.barrel_length".into(), 4000.0),
                ("Barrel.bore_diameter".into(), 50.0),
                ("Barrel.rifling_depth".into(), 4.0),
                ("Ammunition.propellant_charge".into(), 80.0),
                ("Ammunition.penetrator_length".into(), 400.0),
                ("Targeting.prediction_range".into(), 2500.0),
            ]),
        },
        Preset {
            name: "Brawler".into(),
            description: "Maximum fire rate at close range. Gets in close and hammers.".into(),
            slot_selections: HashMap::from([
                ("Barrel".into(), 2),      // Stubby
                ("Ammunition".into(), 1),  // HE (area damage up close)
                ("Feed Mechanism".into(), 2), // Autoloader
                ("Cooling".into(), 1),     // Active Coolant
                ("Mount".into(), 1),       // Turret
                ("Targeting".into(), 0),   // Manual (close range doesn't need lead)
            ]),
            parameter_values: HashMap::from([
                ("Barrel.bore_diameter".into(), 200.0),
                ("Feed Mechanism.loader_speed".into(), 600.0),
                ("Cooling.coolant_flow_rate".into(), 15.0),
            ]),
        },
        Preset {
            name: "Fire Starter".into(),
            description: "Incendiary rounds. Sets everything on fire. Chaos.".into(),
            slot_selections: HashMap::from([
                ("Barrel".into(), 0),      // Standard
                ("Ammunition".into(), 2),  // Incendiary
                ("Feed Mechanism".into(), 1), // Belt Feed
                ("Cooling".into(), 0),     // Passive
                ("Mount".into(), 1),       // Turret
                ("Targeting".into(), 1),   // Basic Lead
            ]),
            parameter_values: HashMap::from([
                ("Ammunition.incendiary_compound".into(), 80.0),
                ("Ammunition.burn_duration".into(), 10.0),
            ]),
        },
    ]
}

/// Built-in presets for railgun
pub fn railgun_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Balanced".into(),
            description: "Default configuration. Solid all-round railgun.".into(),
            slot_selections: HashMap::from([
                ("Rail Assembly".into(), 0),
                ("Capacitor Bank".into(), 0),
                ("Projectile".into(), 0), // Tungsten Slug
            ]),
            parameter_values: HashMap::new(),
        },
        Preset {
            name: "Long Lance".into(),
            description: "Maxed-out rails and a heavy tungsten slug. Extreme range and penetration, slow recharge.".into(),
            slot_selections: HashMap::from([
                ("Rail Assembly".into(), 0),
                ("Capacitor Bank".into(), 0),
                ("Projectile".into(), 0), // Tungsten Slug
            ]),
            parameter_values: HashMap::from([
                ("Rail Assembly.rail_length".into(), 4000.0),
                ("Rail Assembly.rail_material_conductivity".into(), 95.0),
                ("Capacitor Bank.capacitor_size".into(), 1900.0),
                ("Projectile.slug_mass".into(), 400.0),
                ("Projectile.slug_length".into(), 280.0),
            ]),
        },
        Preset {
            name: "Rapid Rail".into(),
            description: "Short rails, small capacitor, light slug. Trades punch for a faster shot cycle.".into(),
            slot_selections: HashMap::from([
                ("Rail Assembly".into(), 0),
                ("Capacitor Bank".into(), 0),
                ("Projectile".into(), 0), // Tungsten Slug
            ]),
            parameter_values: HashMap::from([
                ("Rail Assembly.rail_length".into(), 900.0),
                ("Capacitor Bank.capacitor_size".into(), 180.0),
                ("Capacitor Bank.charge_rate".into(), 180.0),
                ("Projectile.slug_mass".into(), 30.0),
            ]),
        },
        Preset {
            name: "Armor Breaker".into(),
            description: "Explosive-tipped slug tuned to punch through hull, then detonate inside.".into(),
            slot_selections: HashMap::from([
                ("Rail Assembly".into(), 0),
                ("Capacitor Bank".into(), 0),
                ("Projectile".into(), 1), // Explosive-Tipped Slug
            ]),
            parameter_values: HashMap::from([
                ("Capacitor Bank.capacitor_size".into(), 1200.0),
                ("Projectile.slug_mass".into(), 250.0),
                ("Projectile.explosive_filler".into(), 80.0),
                ("Projectile.fuse_depth".into(), 120.0),
            ]),
        },
    ]
}

/// Built-in presets for gatling
pub fn gatling_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Balanced".into(),
            description: "Default 4-barrel assembly. Fast spin-up, manageable ammo consumption.".into(),
            slot_selections: HashMap::from([
                ("Barrel Cluster".into(), 0), // 4-Barrel
            ]),
            parameter_values: HashMap::new(),
        },
        Preset {
            name: "Wall of Lead".into(),
            description: "8-barrel assembly at max RPM. Obscene fire rate, eats ammo, slow to spin up.".into(),
            slot_selections: HashMap::from([
                ("Barrel Cluster".into(), 1), // 8-Barrel
            ]),
            parameter_values: HashMap::from([
                ("Barrel Cluster.barrel_rpm".into(), 10000.0),
                ("Barrel Cluster.spin_up_time".into(), 3.0),
            ]),
        },
        Preset {
            name: "Precision Burst".into(),
            description: "4-barrel assembly with wider-caliber barrels. Controlled bursts, more damage per hit.".into(),
            slot_selections: HashMap::from([
                ("Barrel Cluster".into(), 0), // 4-Barrel
            ]),
            parameter_values: HashMap::from([
                ("Barrel Cluster.barrel_rpm".into(), 1800.0),
                ("Barrel Cluster.barrel_caliber".into(), 28.0),
                ("Barrel Cluster.spin_up_time".into(), 0.6),
            ]),
        },
    ]
}

/// Built-in presets for laser
pub fn laser_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Balanced".into(),
            description: "Default emitter and optics. Steady damage, manageable heat.".into(),
            slot_selections: HashMap::from([
                ("Emitter".into(), 0),
                ("Focusing Optics".into(), 0),
                ("Cooling".into(), 0),
            ]),
            parameter_values: HashMap::new(),
        },
        Preset {
            name: "Beam Lance".into(),
            description: "Long focal length and a big radiator. Precise sustained fire at extreme range.".into(),
            slot_selections: HashMap::from([
                ("Emitter".into(), 0),
                ("Focusing Optics".into(), 0),
                ("Cooling".into(), 0),
            ]),
            parameter_values: HashMap::from([
                ("Emitter.beam_power".into(), 550.0),
                ("Emitter.wavelength".into(), 250.0),
                ("Focusing Optics.lens_diameter".into(), 450.0),
                ("Focusing Optics.focal_length".into(), 1600.0),
                ("Cooling.radiator_area".into(), 1800.0),
            ]),
        },
        Preset {
            name: "Overcharged".into(),
            description: "Beam power pushed to the redline with a small radiator. Devastating until it overheats.".into(),
            slot_selections: HashMap::from([
                ("Emitter".into(), 0),
                ("Focusing Optics".into(), 0),
                ("Cooling".into(), 0),
            ]),
            parameter_values: HashMap::from([
                ("Emitter.beam_power".into(), 1000.0),
                ("Cooling.radiator_area".into(), 300.0),
                ("Cooling.max_operating_temp".into(), 700.0),
            ]),
        },
    ]
}

/// Built-in presets for heavy missiles (torpedoes)
pub fn torpedo_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Balanced".into(),
            description: "Standard explosive warhead, unguided. Cheap and reliable.".into(),
            slot_selections: HashMap::from([
                ("Warhead".into(), 0),     // Standard Explosive
                ("Propulsion".into(), 0),
                ("Guidance".into(), 0),    // Unguided
            ]),
            parameter_values: HashMap::new(),
        },
        Preset {
            name: "Bunker Buster".into(),
            description: "Shaped charge with heat-seeking guidance. Punches deep into a single target.".into(),
            slot_selections: HashMap::from([
                ("Warhead".into(), 1),     // Shaped Charge
                ("Propulsion".into(), 0),
                ("Guidance".into(), 2),    // Heat-Seeking
            ]),
            parameter_values: HashMap::from([
                ("Warhead.charge_mass".into(), 450.0),
                ("Warhead.cone_angle".into(), 25.0),
                ("Guidance.seeker_sensitivity".into(), 60.0),
            ]),
        },
        Preset {
            name: "Fire and Forget".into(),
            description: "Heat-seeking warhead that hunts targets down. Set it loose and move on.".into(),
            slot_selections: HashMap::from([
                ("Warhead".into(), 0),     // Standard Explosive
                ("Propulsion".into(), 0),
                ("Guidance".into(), 2),    // Heat-Seeking
            ]),
            parameter_values: HashMap::from([
                ("Guidance.seeker_sensitivity".into(), 80.0),
                ("Guidance.seeker_fov".into(), 70.0),
                ("Guidance.countermeasure_resistance".into(), 60.0),
            ]),
        },
        Preset {
            name: "Saturation".into(),
            description: "Unguided but massive blast radius. Cheap, dumb, and devastating in a crowd.".into(),
            slot_selections: HashMap::from([
                ("Warhead".into(), 0),     // Standard Explosive
                ("Propulsion".into(), 0),
                ("Guidance".into(), 0),    // Unguided
            ]),
            parameter_values: HashMap::from([
                ("Warhead.explosive_mass".into(), 900.0),
                ("Warhead.blast_radius".into(), 950.0),
                ("Warhead.fuse_type_proximity".into(), 80.0),
            ]),
        },
    ]
}

/// Built-in presets for utility weapons that share the default power-only tree
/// (Mining Drill, Tractor Beam, EMP Pulse)
pub fn utility_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Balanced".into(),
            description: "Default power configuration.".into(),
            slot_selections: HashMap::from([("Power".into(), 0)]),
            parameter_values: HashMap::new(),
        },
        Preset {
            name: "Efficient".into(),
            description: "Low power draw, high efficiency. Easier on the reactor.".into(),
            slot_selections: HashMap::from([("Power".into(), 0)]),
            parameter_values: HashMap::from([
                ("Power.power_draw".into(), 8.0),
                ("Power.efficiency".into(), 95.0),
            ]),
        },
        Preset {
            name: "Overclocked".into(),
            description: "Pushed well past rated power. Draws far more than it should.".into(),
            slot_selections: HashMap::from([("Power".into(), 0)]),
            parameter_values: HashMap::from([
                ("Power.power_draw".into(), 45.0),
                ("Power.efficiency".into(), 60.0),
            ]),
        },
    ]
}

/// Dispatch to the preset list for a given module type.
/// Returns an empty vec for module types with no curated presets.
pub fn presets_for(module_type: ModuleType) -> Vec<Preset> {
    match module_type {
        ModuleType::Cannon => cannon_presets(),
        ModuleType::Railgun => railgun_presets(),
        ModuleType::Gatling => gatling_presets(),
        ModuleType::Laser => laser_presets(),
        ModuleType::HeavyMissile => torpedo_presets(),
        ModuleType::MiningDrill | ModuleType::TractorBeam | ModuleType::EMPPulse => utility_presets(),
        _ => Vec::new(),
    }
}
