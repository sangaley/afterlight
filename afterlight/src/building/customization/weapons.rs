use std::collections::HashMap;
use super::parameters::*;

// ============================================================================
// WEAPON CUSTOMIZATION DEFINITIONS
// Each weapon family has completely different ship-component trees.
// This is where the FTD-level depth lives.
// ============================================================================

/// Weapon category → family hierarchy
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum WeaponCategory {
    Kinetic,
    Energy,
    Missile,
    Utility,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum WeaponFamily {
    // Kinetic
    Cannon,
    Railgun,
    Coilgun,
    Gatling,
    // Energy
    Laser,
    PlasmaCaster,
    IonDisruptor,
    // Missile
    HeavyMissile,
    GuidedMissile,
    ClusterRocket,
    // Utility
    MiningDrill,
    TractorBeam,
    EMPPulse,
}

impl WeaponCategory {
    pub fn families(&self) -> &[WeaponFamily] {
        match self {
            Self::Kinetic => &[WeaponFamily::Cannon, WeaponFamily::Railgun, WeaponFamily::Coilgun, WeaponFamily::Gatling],
            Self::Energy => &[WeaponFamily::Laser, WeaponFamily::PlasmaCaster, WeaponFamily::IonDisruptor],
            Self::Missile => &[WeaponFamily::HeavyMissile, WeaponFamily::GuidedMissile, WeaponFamily::ClusterRocket],
            Self::Utility => &[WeaponFamily::MiningDrill, WeaponFamily::TractorBeam, WeaponFamily::EMPPulse],
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Kinetic => "Kinetic",
            Self::Energy => "Energy",
            Self::Missile => "Missile",
            Self::Utility => "Utility",
        }
    }
}

impl WeaponFamily {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cannon => "Cannon",
            Self::Railgun => "Railgun",
            Self::Coilgun => "Coilgun",
            Self::Gatling => "Gatling",
            Self::Laser => "Laser",
            Self::PlasmaCaster => "Plasma Caster",
            Self::IonDisruptor => "Ion Disruptor",
            Self::HeavyMissile => "HeavyMissile",
            Self::GuidedMissile => "Guided Missile",
            Self::ClusterRocket => "Cluster Rocket",
            Self::MiningDrill => "Mining Drill",
            Self::TractorBeam => "Tractor Beam",
            Self::EMPPulse => "EMP Pulse",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Cannon => "Heavy kinetic rounds. Slow fire, devastating impact. The workhorse of combat.",
            Self::Railgun => "Electromagnetically accelerated slugs. Extreme range and penetration.",
            Self::Coilgun => "Burst-fire magnetic accelerator. High DPS at medium range.",
            Self::Gatling => "Rotary autocannon. Fills space with lead. Inaccurate but terrifying.",
            Self::Laser => "Sustained energy beam. Precise, no ammo, overheats fast.",
            Self::PlasmaCaster => "Superheated plasma bolts. Area damage on impact. Power hungry.",
            Self::IonDisruptor => "Disrupts electrical systems. Low damage, high disable chance.",
            Self::HeavyMissile => "Self-propelled warhead. Slow, devastating, limited supply.",
            Self::GuidedMissile => "Tracking missiles. Lock on, fire, forget. Counterable with ECM.",
            Self::ClusterRocket => "Fires a spread of unguided rockets. Area saturation.",
            Self::MiningDrill => "Extracts resources from asteroids. Weak combat use.",
            Self::TractorBeam => "Pulls objects toward ship. Salvage, debris clearing, creative combat.",
            Self::EMPPulse => "Disables electronics in a radius. Affects friend and foe.",
        }
    }

    pub fn category(&self) -> WeaponCategory {
        match self {
            Self::Cannon | Self::Railgun | Self::Coilgun | Self::Gatling => WeaponCategory::Kinetic,
            Self::Laser | Self::PlasmaCaster | Self::IonDisruptor => WeaponCategory::Energy,
            Self::HeavyMissile | Self::GuidedMissile | Self::ClusterRocket => WeaponCategory::Missile,
            Self::MiningDrill | Self::TractorBeam | Self::EMPPulse => WeaponCategory::Utility,
        }
    }

    /// Build the full customization definition for this weapon family
    pub fn customization_def(&self) -> ModuleCustomizationDef {
        match self {
            Self::Cannon => cannon_customization(),
            Self::Railgun => railgun_customization(),
            Self::Gatling => gatling_customization(),
            Self::Laser => laser_customization(),
            Self::HeavyMissile => torpedo_customization(),
            // Other families use a simplified version for now
            _ => default_weapon_customization(),
        }
    }
}

// ============================================================================
// CANNON — the deepest customization tree as the showcase
// ============================================================================

fn cannon_customization() -> ModuleCustomizationDef {
    ModuleCustomizationDef {
        slots: vec![
            // BARREL SLOT
            SubComponentSlotDef {
                slot_name: "Barrel".into(),
                description: "The gun tube. Length, bore, and construction affect range, damage, and weight.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Standard Barrel".into(),
                        description: "Balanced performance. No weaknesses, no standout strengths.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("barrel_length", "Longer barrel = more range + accuracy, slower traverse", "Range, Accuracy", 500.0, 3000.0, 1200.0, "mm"),
                            param("bore_diameter", "Wider bore = bigger rounds = more damage, heavier", "Damage, Weight", 20.0, 200.0, 80.0, "mm"),
                            param_stepped("wall_thickness", "Thicker walls handle more pressure but add weight", "Durability, Weight", 5.0, 40.0, 15.0, "mm", 1.0),
                            param("rifling_depth", "Deeper rifling = better accuracy, slightly slower velocity", "Accuracy", 0.0, 5.0, 1.5, "mm"),
                        ],
                    },
                    SubComponentOption {
                        name: "Long Range Barrel".into(),
                        description: "Extended barrel for maximum range. Heavy, slow to traverse.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("range".into(), 1.4); m.insert("traverse_speed".into(), 0.7); m },
                        parameters: vec![
                            param("barrel_length", "Extended tube for velocity", "Range", 1500.0, 5000.0, 3000.0, "mm"),
                            param("bore_diameter", "Matched to barrel length for optimal ballistics", "Damage", 30.0, 150.0, 60.0, "mm"),
                            param_stepped("wall_thickness", "Must be thick to handle barrel length stress", "Durability", 10.0, 50.0, 25.0, "mm", 1.0),
                            param("rifling_depth", "Precision rifling for long-range accuracy", "Accuracy", 1.0, 6.0, 3.0, "mm"),
                            param("muzzle_brake_effectiveness", "Reduces recoil for sustained accuracy", "Recoil", 0.0, 100.0, 60.0, "%"),
                        ],
                    },
                    SubComponentOption {
                        name: "Stubby Brawler Barrel".into(),
                        description: "Short barrel for close-range fights. Fast traverse, high fire rate.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("fire_rate".into(), 1.3); m.insert("range".into(), 0.6); m },
                        parameters: vec![
                            param("barrel_length", "Short and punchy", "Range", 200.0, 1000.0, 500.0, "mm"),
                            param("bore_diameter", "Wide bore for maximum close-range damage", "Damage", 40.0, 250.0, 120.0, "mm"),
                            param_stepped("wall_thickness", "Thick for sustained rapid fire", "Durability", 8.0, 35.0, 20.0, "mm", 1.0),
                        ],
                    },
                ],
                default_option: 0,
            },
            // AMMO SLOT
            SubComponentSlotDef {
                slot_name: "Ammunition".into(),
                description: "Projectile type and propellant. Determines damage type and ballistic performance.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Solid".into(),
                        description: "Dense metal, no filler. Hits the hull hard, splashes almost nothing.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("hull_damage".into(), 1.5); m.insert("area_damage".into(), 0.2); m },
                        parameters: vec![
                            param("caliber", "Projectile diameter - must be <= bore diameter", "Damage", 15.0, 200.0, 75.0, "mm"),
                            param("propellant_charge", "More propellant = faster muzzle velocity, more barrel wear", "Velocity, Barrel Wear", 10.0, 100.0, 50.0, "g"),
                            param_stepped("magazine_capacity", "More rounds = heavier magazine, slower reload", "Ammo, Reload Time", 5.0, 60.0, 20.0, "rounds", 5.0),
                            param("penetrator_length", "Longer penetrator = more armor piercing", "Penetration", 50.0, 500.0, 200.0, "mm"),
                        ],
                    },
                    SubComponentOption {
                        name: "Shredder".into(),
                        description: "Explosive filler. Covers an area, gives up penetration to do it.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("area_damage".into(), 1.8); m.insert("hull_damage".into(), 0.8); m },
                        parameters: vec![
                            param("caliber", "Shell diameter", "Damage", 20.0, 200.0, 80.0, "mm"),
                            param("propellant_charge", "Propellant amount", "Velocity", 10.0, 80.0, 40.0, "g"),
                            param_stepped("magazine_capacity", "Magazine size", "Ammo", 5.0, 40.0, 15.0, "rounds", 5.0),
                            param("explosive_filler", "More filler = bigger explosion, less penetration", "Blast Radius", 10.0, 200.0, 80.0, "g"),
                            param("fuse_delay", "Delay before detonation - 0 = impact, higher = penetrate then explode", "Detonation", 0.0, 50.0, 5.0, "ms"),
                        ],
                    },
                    SubComponentOption {
                        name: "Torch".into(),
                        description: "Sets the target burning. Damage that keeps arriving after the hit.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("fire_chance".into(), 0.8); m.insert("hull_damage".into(), 0.6); m },
                        parameters: vec![
                            param("caliber", "Shell diameter", "Damage", 20.0, 150.0, 70.0, "mm"),
                            param("propellant_charge", "Propellant amount", "Velocity", 10.0, 70.0, 35.0, "g"),
                            param_stepped("magazine_capacity", "Magazine size", "Ammo", 5.0, 50.0, 25.0, "rounds", 5.0),
                            param("incendiary_compound", "Burn intensity - more = hotter fire, shorter burn", "Fire Damage", 10.0, 100.0, 50.0, "g"),
                            param("burn_duration", "How long the fire lasts", "Fire Duration", 1.0, 15.0, 5.0, "s"),
                        ],
                    },
                    SubComponentOption {
                        name: "Blackout".into(),
                        description: "Pulse on impact. Everything wired in reach stops working for a while.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("disable_chance".into(), 0.6); m.insert("hull_damage".into(), 0.3); m },
                        parameters: vec![
                            param("caliber", "Shell diameter", "Damage", 30.0, 150.0, 80.0, "mm"),
                            param("propellant_charge", "Propellant amount", "Velocity", 10.0, 60.0, 30.0, "g"),
                            param_stepped("magazine_capacity", "Magazine size", "Ammo", 3.0, 20.0, 10.0, "rounds", 1.0),
                            param("emp_radius", "Disable radius on impact", "Disable Range", 50.0, 500.0, 200.0, "units"),
                            param("emp_duration", "How long systems stay disabled", "Disable Duration", 1.0, 10.0, 3.0, "s"),
                        ],
                    },
                ],
                default_option: 0,
            },
            // FEED MECHANISM SLOT
            SubComponentSlotDef {
                slot_name: "Feed Mechanism".into(),
                description: "How ammunition is loaded into the chamber. Affects fire rate and reliability.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Manual Load".into(),
                        description: "Crew manually loads each round. Slow but reliable. No power draw.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("fire_rate".into(), 0.5); m.insert("reliability".into(), 1.0); m },
                        parameters: vec![
                            param("crew_skill_bonus", "Trained crew load faster", "Fire Rate", 0.0, 50.0, 0.0, "%"),
                        ],
                    },
                    SubComponentOption {
                        name: "Belt Feed".into(),
                        description: "Continuous belt feed. Good sustained fire, can jam.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("fire_rate".into(), 1.2); m.insert("reliability".into(), 0.85); m },
                        parameters: vec![
                            param("belt_speed", "Feed speed - faster = higher fire rate, more jams", "Fire Rate, Jam Chance", 50.0, 500.0, 200.0, "rpm"),
                            param("belt_tension", "Tighter belt = fewer jams, more wear", "Reliability, Wear", 10.0, 100.0, 50.0, "N"),
                        ],
                    },
                    SubComponentOption {
                        name: "Autoloader".into(),
                        description: "Mechanized loading. Fast, power-hungry, heavy.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("fire_rate".into(), 1.8); m.insert("power_draw".into(), 1.5); m },
                        parameters: vec![
                            param("loader_speed", "Mechanical cycle rate", "Fire Rate", 100.0, 800.0, 400.0, "rpm"),
                            param("power_consumption", "Power draw of the autoloader motor", "Power Draw", 5.0, 50.0, 20.0, "MW"),
                            param("buffer_size", "Ready rounds in the loader - more = sustained bursts", "Burst Length", 1.0, 10.0, 3.0, "rounds"),
                        ],
                    },
                ],
                default_option: 1,
            },
            // COOLING SLOT
            SubComponentSlotDef {
                slot_name: "Cooling".into(),
                description: "Heat management. Sustained fire generates heat - too much and the weapon shuts down.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Passive Radiator".into(),
                        description: "No power, no weight, slow cooling. Fine for occasional fire.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("surface_area", "More area = faster passive cooling", "Cooling Rate", 100.0, 1000.0, 400.0, "cm2"),
                            param("emissivity", "Material thermal emissivity", "Cooling Rate", 0.3, 1.0, 0.7, ""),
                        ],
                    },
                    SubComponentOption {
                        name: "Active Coolant Loop".into(),
                        description: "Pumped coolant. Fast heat dissipation, uses power, adds weight.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("cooling_rate".into(), 2.5); m },
                        parameters: vec![
                            param("coolant_flow_rate", "Faster flow = more cooling, more power", "Cooling Rate, Power", 1.0, 20.0, 8.0, "L/s"),
                            param("coolant_temperature", "Lower base temp = more cooling headroom", "Heat Capacity", -50.0, 20.0, -10.0, "degC"),
                            param("pump_power", "Power draw of the coolant pump", "Power Draw", 2.0, 30.0, 10.0, "MW"),
                        ],
                    },
                    SubComponentOption {
                        name: "Heat Sink Magazine".into(),
                        description: "Absorbs heat into replaceable sinks. Burst-friendly, limited capacity.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("burst_cooling".into(), 3.0); m },
                        parameters: vec![
                            param("sink_capacity", "Total heat absorbable before replacement", "Heat Budget", 500.0, 5000.0, 2000.0, "kJ"),
                            param("sink_count", "Number of sinks - more = longer sustained fire", "Sustained Fire", 1.0, 8.0, 3.0, "units"),
                            param("eject_speed", "How fast spent sinks are swapped", "Recovery Time", 0.5, 5.0, 2.0, "s"),
                        ],
                    },
                ],
                default_option: 0,
            },
            // MOUNT SLOT
            SubComponentSlotDef {
                slot_name: "Mount".into(),
                description: "How the weapon is attached to the hull. Affects traverse speed and stability.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Fixed Mount".into(),
                        description: "Bolted to the hull. No traverse - aim by turning the ship. Lightest, cheapest.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("traverse_speed".into(), 0.0); m.insert("accuracy".into(), 1.2); m },
                        parameters: vec![
                            param("vibration_damping", "Reduces fire-induced vibration", "Accuracy", 0.0, 100.0, 30.0, "%"),
                        ],
                    },
                    SubComponentOption {
                        name: "Turret Mount".into(),
                        description: "Motorized rotation. Tracks targets independently. Standard choice.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("traverse_speed".into(), 1.0); m },
                        parameters: vec![
                            param("traverse_speed", "Rotation speed of the turret", "Target Tracking", 10.0, 180.0, 60.0, "deg/s"),
                            param("elevation_range", "Vertical aim range", "Coverage", 10.0, 90.0, 45.0, "deg"),
                            param("stabilization", "Gyro stabilization - counters ship movement", "Moving Accuracy", 0.0, 100.0, 50.0, "%"),
                            param("motor_power", "Turret motor power draw", "Power Draw", 2.0, 20.0, 8.0, "MW"),
                        ],
                    },
                ],
                default_option: 1,
            },
            // TARGETING SLOT
            SubComponentSlotDef {
                slot_name: "Targeting".into(),
                description: "Fire control system. Determines how well the weapon predicts target movement.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Manual Aim".into(),
                        description: "Player aims directly. No auto-lead. Skill-based.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![],
                    },
                    SubComponentOption {
                        name: "Basic Lead Computer".into(),
                        description: "Calculates linear lead. Works on straight-moving targets.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("accuracy".into(), 1.3); m },
                        parameters: vec![
                            param("prediction_range", "Max range for lead calculation", "Effective Range", 200.0, 2000.0, 800.0, "units"),
                            param("update_rate", "How often targeting recalculates", "Tracking Speed", 1.0, 30.0, 10.0, "Hz"),
                        ],
                    },
                    SubComponentOption {
                        name: "Advanced Tracking System".into(),
                        description: "Predictive targeting with maneuver anticipation. Radar-linked.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("accuracy".into(), 1.6); m.insert("power_draw".into(), 1.3); m },
                        parameters: vec![
                            param("prediction_range", "Max targeting range", "Effective Range", 500.0, 3000.0, 1500.0, "units"),
                            param("update_rate", "Recalculation frequency", "Tracking Speed", 5.0, 60.0, 30.0, "Hz"),
                            param("maneuver_prediction", "Anticipates target direction changes", "vs Evasive Targets", 0.0, 100.0, 50.0, "%"),
                            param("radar_link_bonus", "Bonus when linked to ship radar", "Synergy", 0.0, 50.0, 20.0, "%"),
                        ],
                    },
                ],
                default_option: 1,
            },
        ],
    }
}

// ============================================================================
// RAILGUN — different feel than cannon
// ============================================================================

fn railgun_customization() -> ModuleCustomizationDef {
    ModuleCustomizationDef {
        slots: vec![
            SubComponentSlotDef {
                slot_name: "Rail Assembly".into(),
                description: "The electromagnetic rails that accelerate the projectile.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Standard Rails".into(),
                        description: "Balanced performance.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("rail_length", "Longer rails = higher muzzle velocity", "Range, Damage", 500.0, 4000.0, 2000.0, "mm"),
                            param("rail_gap", "Gap between rails - affects field strength", "Velocity", 10.0, 80.0, 30.0, "mm"),
                            param("rail_material_conductivity", "Better conductivity = less energy loss", "Efficiency", 50.0, 100.0, 75.0, "%"),
                        ],
                    },
                ],
                default_option: 0,
            },
            SubComponentSlotDef {
                slot_name: "Capacitor Bank".into(),
                description: "Stores energy for each shot. Bigger bank = more powerful shots.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Standard Capacitor".into(),
                        description: "Reliable energy storage.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("capacitor_size", "Total energy per shot", "Damage", 100.0, 2000.0, 500.0, "kJ"),
                            param("charge_rate", "How fast capacitors refill", "Fire Rate", 10.0, 200.0, 80.0, "kJ/s"),
                            param("discharge_efficiency", "Energy that reaches the projectile vs lost as heat", "Efficiency", 50.0, 95.0, 75.0, "%"),
                        ],
                    },
                ],
                default_option: 0,
            },
            SubComponentSlotDef {
                slot_name: "Projectile".into(),
                description: "The slug being fired. Material and shape affect penetration.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Tungsten Slug".into(),
                        description: "Dense, hard. Maximum penetration.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("penetration".into(), 1.5); m },
                        parameters: vec![
                            param("slug_mass", "Heavier = more damage, slower velocity", "Damage, Velocity", 10.0, 500.0, 100.0, "g"),
                            param("slug_length", "Longer penetrator = deeper penetration", "Penetration", 20.0, 300.0, 100.0, "mm"),
                        ],
                    },
                    SubComponentOption {
                        name: "Explosive-Tipped Slug".into(),
                        description: "Penetrates then detonates inside the target.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("area_damage".into(), 1.3); m },
                        parameters: vec![
                            param("slug_mass", "Total slug mass", "Damage", 20.0, 400.0, 120.0, "g"),
                            param("explosive_filler", "Explosive payload", "Internal Damage", 5.0, 100.0, 30.0, "g"),
                            param("fuse_depth", "How deep it penetrates before detonating", "Optimal vs Hull Thickness", 10.0, 200.0, 50.0, "mm"),
                        ],
                    },
                ],
                default_option: 0,
            },
        ],
    }
}

fn gatling_customization() -> ModuleCustomizationDef {
    ModuleCustomizationDef {
        slots: vec![
            SubComponentSlotDef {
                slot_name: "Barrel Cluster".into(),
                description: "Multiple rotating barrels. More barrels = higher sustained fire.".into(),
                options: vec![
                    SubComponentOption {
                        name: "4-Barrel Assembly".into(),
                        description: "Light, fast spin-up.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("fire_rate".into(), 1.0); m },
                        parameters: vec![
                            param("spin_up_time", "Time to reach full RPM", "Response Time", 0.5, 3.0, 1.0, "s"),
                            param("barrel_rpm", "Rotation speed", "Fire Rate", 1000.0, 6000.0, 3000.0, "rpm"),
                            param("barrel_caliber", "Per-barrel bore diameter", "Per-Round Damage", 10.0, 30.0, 15.0, "mm"),
                        ],
                    },
                    SubComponentOption {
                        name: "8-Barrel Assembly".into(),
                        description: "Heavy, insane fire rate. Eats ammo.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("fire_rate".into(), 2.0); m.insert("ammo_consumption".into(), 2.0); m },
                        parameters: vec![
                            param("spin_up_time", "Slow spin-up due to mass", "Response Time", 1.0, 5.0, 2.5, "s"),
                            param("barrel_rpm", "Rotation speed", "Fire Rate", 2000.0, 10000.0, 5000.0, "rpm"),
                            param("barrel_caliber", "Per-barrel bore diameter", "Per-Round Damage", 10.0, 25.0, 12.0, "mm"),
                        ],
                    },
                ],
                default_option: 0,
            },
        ],
    }
}

fn laser_customization() -> ModuleCustomizationDef {
    ModuleCustomizationDef {
        slots: vec![
            SubComponentSlotDef {
                slot_name: "Emitter".into(),
                description: "The laser generation system.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Solid-State Emitter".into(),
                        description: "Reliable, moderate power. Standard choice.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("beam_power", "Output wattage - more = more damage, more heat", "DPS, Heat", 50.0, 1000.0, 300.0, "kW"),
                            param("wavelength", "Shorter wavelength = better focus at range", "Range", 200.0, 1200.0, 500.0, "nm"),
                            param("pulse_frequency", "Continuous vs pulsed - pulsed does burst damage", "Damage Profile", 0.0, 1000.0, 0.0, "Hz"),
                        ],
                    },
                ],
                default_option: 0,
            },
            SubComponentSlotDef {
                slot_name: "Focusing Optics".into(),
                description: "Lens assembly that shapes the beam.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Standard Lens".into(),
                        description: "Good all-around focus.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("lens_diameter", "Bigger lens = tighter focus at range, heavier", "Range, Weight", 50.0, 500.0, 150.0, "mm"),
                            param("focal_length", "Distance of optimal focus - damage drops outside", "Optimal Range", 100.0, 2000.0, 600.0, "units"),
                            param("coating_reflectivity", "Anti-reflective coating quality", "Efficiency", 80.0, 99.9, 95.0, "%"),
                        ],
                    },
                ],
                default_option: 0,
            },
            SubComponentSlotDef {
                slot_name: "Cooling".into(),
                description: "Lasers generate immense waste heat.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Radiative Cooling".into(),
                        description: "Passive heat radiation. Limits sustained fire time.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("radiator_area", "Surface area for heat dumping", "Sustained Fire", 200.0, 2000.0, 800.0, "cm2"),
                            param("max_operating_temp", "Shut-down temperature threshold", "Overheat Limit", 200.0, 800.0, 400.0, "degC"),
                        ],
                    },
                ],
                default_option: 0,
            },
        ],
    }
}

fn torpedo_customization() -> ModuleCustomizationDef {
    ModuleCustomizationDef {
        slots: vec![
            SubComponentSlotDef {
                slot_name: "Warhead".into(),
                description: "The business end of the torpedo.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Standard Explosive".into(),
                        description: "Reliable blast damage.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("explosive_mass", "More explosive = bigger boom, heavier missile", "Damage, Weight", 50.0, 1000.0, 300.0, "kg"),
                            param("blast_radius", "Damage falloff radius", "Area", 100.0, 1000.0, 400.0, "units"),
                            param("fuse_type_proximity", "Proximity fuse distance - 0 = contact only", "Detonation", 0.0, 100.0, 30.0, "units"),
                        ],
                    },
                    SubComponentOption {
                        name: "Shaped Charge".into(),
                        description: "Focused penetration. Devastating vs single targets.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("penetration".into(), 2.0); m.insert("area_damage".into(), 0.3); m },
                        parameters: vec![
                            param("charge_mass", "Explosive liner mass", "Penetration", 30.0, 500.0, 150.0, "kg"),
                            param("cone_angle", "Narrower = deeper penetration, smaller hit area", "Penetration vs Area", 20.0, 90.0, 45.0, "deg"),
                        ],
                    },
                ],
                default_option: 0,
            },
            SubComponentSlotDef {
                slot_name: "Propulsion".into(),
                description: "How the torpedo moves after launch.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Chemical Motor".into(),
                        description: "Fast burn, limited range. Gets there quick.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("fuel_mass", "More fuel = more range, heavier", "Range, Weight", 20.0, 300.0, 100.0, "kg"),
                            param("thrust", "Motor thrust - more = faster, louder", "Speed, Noise", 500.0, 5000.0, 2000.0, "N"),
                            param("burn_time", "How long the motor runs", "Range", 2.0, 30.0, 10.0, "s"),
                        ],
                    },
                ],
                default_option: 0,
            },
            SubComponentSlotDef {
                slot_name: "Guidance".into(),
                description: "Navigation system for the torpedo.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Unguided (Dumb Fire)".into(),
                        description: "Fires straight. Cheap, reliable, aim well.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("accuracy".into(), 0.5); m },
                        parameters: vec![],
                    },
                    SubComponentOption {
                        name: "Wire-Guided".into(),
                        description: "Player steers via wire link. Accurate, limited range.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("accuracy".into(), 1.5); m },
                        parameters: vec![
                            param("wire_length", "Max control range before wire breaks", "Control Range", 500.0, 3000.0, 1500.0, "units"),
                        ],
                    },
                    SubComponentOption {
                        name: "Heat-Seeking".into(),
                        description: "Locks onto heat signatures. Fire and forget.".into(),
                        stat_modifiers: { let mut m = HashMap::new(); m.insert("accuracy".into(), 1.8); m },
                        parameters: vec![
                            param("seeker_sensitivity", "Minimum heat signature to track", "Lock Range", 10.0, 100.0, 40.0, "units"),
                            param("seeker_fov", "Field of view of seeker head", "Tracking Cone", 10.0, 90.0, 30.0, "deg"),
                            param("countermeasure_resistance", "Resistance to flares/decoys", "vs ECM", 0.0, 100.0, 30.0, "%"),
                        ],
                    },
                ],
                default_option: 0,
            },
        ],
    }
}

fn default_weapon_customization() -> ModuleCustomizationDef {
    ModuleCustomizationDef {
        slots: vec![
            SubComponentSlotDef {
                slot_name: "Power".into(),
                description: "Power regulation for the weapon system.".into(),
                options: vec![
                    SubComponentOption {
                        name: "Standard".into(),
                        description: "Default power configuration.".into(),
                        stat_modifiers: HashMap::new(),
                        parameters: vec![
                            param("power_draw", "Base power consumption", "Power", 5.0, 50.0, 15.0, "MW"),
                            param("efficiency", "Power-to-output ratio", "Efficiency", 50.0, 100.0, 75.0, "%"),
                        ],
                    },
                ],
                default_option: 0,
            },
        ],
    }
}

/// Register all weapon customization definitions
pub fn register_weapon_customizations(registry: &mut CustomizationRegistry) {
    for category in [WeaponCategory::Kinetic, WeaponCategory::Energy, WeaponCategory::Missile, WeaponCategory::Utility] {
        for family in category.families() {
            let key = format!("weapon_{:?}", family).to_lowercase();
            registry.register(&key, family.customization_def());
        }
    }
}
