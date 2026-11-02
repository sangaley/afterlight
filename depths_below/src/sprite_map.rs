use bevy::prelude::Color;

use crate::components::{ModuleType, CreatureType, HullMaterial, HullLayer, PoiType};

/// The body tone of `sprites/hull/hull_steel.png`, measured off the artwork
/// itself: the median of its opaque pixels is (59, 69, 81).
///
/// Wedge plating builds its shape from untextured child quads, so unlike every
/// other block it cannot get its colour from an image — something has to tell
/// it what the hull around it looks like. That something used to be a guess
/// per call site: `srgb(0.52, 0.52, 0.56)` in the registry and again in the
/// build palette, more than twice as bright as the plating it bolts to, so a
/// derived armour belt read as a pale fringe drawn around the ship rather than
/// part of it. Anything that wants to match the hull reads this.
pub const HULL_TONE: Color = Color::srgb(0.231, 0.271, 0.318);

/// Maps ModuleType to sprite asset path. Returns None for unmapped types (colored rect fallback).
pub fn module_sprite_path(module_type: ModuleType) -> Option<&'static str> {
    Some(match module_type {
        // Power
        ModuleType::SmallReactor | ModuleType::StandardReactor => "sprites/modules/small_reactor.png",
        ModuleType::LargeReactor => "sprites/modules/large_reactor_2x1.png",
        ModuleType::BatteryBank => "sprites/modules/battery.png",
        ModuleType::RTG => "sprites/modules/small_reactor.png",
        ModuleType::FusionReactor => "sprites/modules/large_reactor_3x3.png",
        ModuleType::Capacitor => "sprites/modules/battery.png",
        ModuleType::PowerConduit => "sprites/modules/battery.png",
        ModuleType::SolarCell => "sprites/modules/small_reactor.png",
        // Propulsion
        ModuleType::SmallEngine | ModuleType::StandardEngine => "sprites/modules/standard_engine.png",
        ModuleType::LargeEngine => "sprites/modules/standard_engine_2x1.png",
        ModuleType::SilentDrive => "sprites/modules/silent_drive.png",
        ModuleType::ManeuveringThruster => "sprites/modules/standard_engine.png",
        ModuleType::JetDrive => "sprites/modules/standard_engine.png",
        ModuleType::EmergencyThruster => "sprites/modules/standard_engine.png",
        ModuleType::RudderAssembly => "sprites/modules/standard_engine.png",
        // Life Support
        ModuleType::OxygenScrubber => "sprites/modules/oxygen_scrubber.png",
        ModuleType::CO2Scrubber => "sprites/modules/life_support.png",
        ModuleType::WasteRecycler => "sprites/modules/life_support.png",
        ModuleType::AdvancedOxygenator => "sprites/modules/oxygen_scrubber_2x1.png",
        ModuleType::FireSuppression => "sprites/modules/life_support.png",
        ModuleType::AtmosphereMonitor => "sprites/modules/life_support.png",
        // Control
        ModuleType::NavigationConsole => "sprites/modules/navigation.png",
        ModuleType::HelmStation => "sprites/modules/navigation.png",
        // Weapons
        // Kinetic weapons
        ModuleType::Cannon => "sprites/modules/cannon.png",
        ModuleType::Railgun => "sprites/modules/railgun_2x1.png",
        ModuleType::Coilgun => "sprites/modules/coilgun.png",
        ModuleType::Gatling => "sprites/modules/gatling.png",
        // Energy weapons
        ModuleType::Laser => "sprites/modules/laser.png",
        ModuleType::PlasmaCaster => "sprites/modules/plasma_caster.png",
        ModuleType::IonDisruptor => "sprites/modules/ion_disruptor.png",
        // Missile weapons
        ModuleType::HeavyMissile => "sprites/modules/heavy_missile.png",
        ModuleType::LaunchPad => "sprites/modules/launch_pad.png",
        ModuleType::RailLauncher => "sprites/modules/rail_launcher.png",
        ModuleType::VlsSilo => "sprites/modules/vls_silo.png",
        ModuleType::GuidedMissile => "sprites/modules/guided_missile.png",
        ModuleType::ClusterRocket => "sprites/modules/cluster_rocket.png",
        // Utility weapons
        ModuleType::MiningDrill => "sprites/modules/mining_drill.png",
        ModuleType::TractorBeam => "sprites/modules/tractor_beam.png",
        ModuleType::EMPPulse => "sprites/modules/emp_pulse.png",
        // Detection
        ModuleType::RadarArray | ModuleType::AdvancedRadar => "sprites/modules/sonar_array.png",
        ModuleType::PassiveRadar => "sprites/modules/passive_sonar.png",
        ModuleType::DepthScanner => "sprites/modules/depth_sensor.png",
        ModuleType::HydrophoneArray => "sprites/modules/passive_sonar_2x1.png",
        ModuleType::ThermalImager => "sprites/modules/depth_sensor.png",
        ModuleType::ProximityAlarm => "sprites/modules/depth_sensor.png",
        ModuleType::MissileWarner => "sprites/modules/missile_warner.png",
        // Storage
        ModuleType::SmallCargo => "sprites/modules/cargo_hold.png",
        ModuleType::LargeCargo => "sprites/modules/cargo_hold_2x1.png",
        ModuleType::AmmoBay => "sprites/modules/cargo_hold.png",
        ModuleType::FuelTank => "sprites/modules/ballast_tank.png",
        ModuleType::SpecimenVault => "sprites/modules/research_lab.png",
        ModuleType::ReinforcedVault => "sprites/modules/cargo_hold.png",
        ModuleType::CryoStorage => "sprites/modules/research_lab.png",
        // Crew
        ModuleType::BasicQuarters => "sprites/modules/basic_quarters.png",
        ModuleType::Barracks => "sprites/modules/basic_quarters_2x1.png",
        ModuleType::MedBay => "sprites/modules/medical_bay.png",
        ModuleType::MessHall => "sprites/modules/basic_quarters.png",
        ModuleType::RecRoom => "sprites/modules/basic_quarters.png",
        ModuleType::OfficerQuarters => "sprites/modules/basic_quarters.png",
        ModuleType::TrainingRoom => "sprites/modules/basic_quarters.png",
        ModuleType::Brig => "sprites/modules/basic_quarters.png",
        // Utility
        ModuleType::RepairBay => "sprites/modules/repair_station.png",
        ModuleType::ManeuverThruster => "sprites/modules/ballast_tank.png",
        ModuleType::Floodlight | ModuleType::Searchlight => "sprites/modules/floodlight.png",
        ModuleType::AirlockChamber => "sprites/modules/docking_port.png",
        ModuleType::DockingPort => "sprites/modules/docking_port.png",
        ModuleType::SalvageArm => "sprites/modules/salvage_arm.png",
        ModuleType::AdvancedRepairBay => "sprites/modules/repair_station_2x1.png",
        ModuleType::DroneBay => "sprites/modules/repair_station_2x1.png",
        ModuleType::DeepFloodlight => "sprites/modules/floodlight.png",
        ModuleType::StealthCoating => "sprites/modules/silent_drive.png",
        ModuleType::HullPatch => "sprites/modules/repair_station.png",
        ModuleType::SignalBuoy => "sprites/modules/floodlight.png",
        // Structural
        ModuleType::HullBeam => "sprites/modules/hull_beam.png",
        ModuleType::LightFrame => "sprites/modules/light_frame.png",
        ModuleType::SupplyCrates => "sprites/modules/supply_crates.png",
        ModuleType::CoolantRun => "sprites/modules/coolant_run.png",
        ModuleType::BallastBlock => "sprites/modules/ballast_block.png",
        ModuleType::HullCorner => "sprites/modules/hull_beam.png",
        ModuleType::Bulkhead => "sprites/modules/hull_beam.png",
        ModuleType::PressureFrame => "sprites/modules/hull_beam.png",
        ModuleType::AirlockValve => "sprites/modules/docking_port.png",
        ModuleType::AccessHatch => "sprites/modules/docking_port.png",
        ModuleType::ViewPort => "sprites/modules/floodlight.png",
        ModuleType::ArmorPlate => "sprites/modules/hull_beam.png",
        // New blocks — placeholder sprites
        ModuleType::CoolingPump => "sprites/modules/repair_station.png",
        ModuleType::HeatVent => "sprites/modules/hull_beam.png",
        ModuleType::Transformer => "sprites/modules/battery.png",
        ModuleType::VectorThruster => "sprites/modules/standard_engine.png",
        ModuleType::AttitudeThruster => "sprites/modules/ballast_tank.png",
        ModuleType::OxygenTank => "sprites/modules/oxygen_scrubber.png",
        ModuleType::AirCirculator => "sprites/modules/life_support.png",
        ModuleType::CreatureScanner => "sprites/modules/depth_sensor.png",
        ModuleType::MineralScanner => "sprites/modules/depth_sensor.png",
        ModuleType::AmmoAutoloader => "sprites/modules/ammo_autoloader.png",
        ModuleType::EngineeringStation => "sprites/modules/repair_station.png",
        // Phase B modules
        ModuleType::ConveyorTube => "sprites/modules/battery.png",
        ModuleType::MaintenanceLocker => "sprites/modules/repair_station.png",
        ModuleType::FuelProcessor => "sprites/modules/ballast_tank.png",
        ModuleType::HullSealer => "sprites/modules/ballast_tank.png",
        // Placeholder art borrowed from the EMP: a blue radial projector reads
        // as a field emitter and, more to the point, is findable on a hull of
        // grey boxes. Its own art comes with the pass.
        ModuleType::EmergencyForceField => "sprites/modules/emp_pulse.png",
        ModuleType::EmergencyBulkhead => "sprites/modules/hull_beam.png",
        ModuleType::FirebreakWall => "sprites/modules/hull_beam.png",
        ModuleType::PressureSensor => "sprites/modules/depth_sensor.png",
        ModuleType::MemoryCore => "sprites/modules/navigation.png",
        ModuleType::TargetingComputer => "sprites/modules/navigation.png",
        ModuleType::AutopilotCore => "sprites/modules/navigation.png",
        ModuleType::AICombatCore => "sprites/modules/navigation_2x1.png",
        ModuleType::ThermalVentGenerator => "sprites/modules/small_reactor.png",
        ModuleType::MineralExtractor => "sprites/modules/salvage_arm.png",
        ModuleType::CreatureContainment => "sprites/modules/research_lab_2x1.png",
        ModuleType::ResearchLab => "sprites/modules/research_lab_2x1.png",
        ModuleType::Corridor => "sprites/modules/hull_beam.png",
        ModuleType::LadderShaft => "sprites/modules/docking_port.png",
        ModuleType::MaintenanceTunnel => "sprites/modules/hull_beam.png",
        // Multi-block extension blocks (reuse existing sprites for now)
        ModuleType::BarrelExtension => "sprites/modules/hull_beam.png",
        ModuleType::AmmoFeedUnit => "sprites/modules/cargo_hold.png",
        ModuleType::CoolingJacket => "sprites/modules/repair_station.png",
        ModuleType::ReactorFuelRod => "sprites/modules/small_reactor.png",
        ModuleType::ReactorCooling => "sprites/modules/repair_station.png",
        ModuleType::EngineNozzle => "sprites/modules/standard_engine.png",
        ModuleType::ShieldEmitter => "sprites/modules/battery.png",
        // Advanced weapon enhancers
        // Gun parts. Placeholders drawn from the Gun Deck preview until the
        // Retro Diffusion art in GUN_PARTS_ART.md replaces them.
        ModuleType::MuzzleBrake => "sprites/gun_parts/muzzle_brake.png",
        ModuleType::RecoilAbsorber => "sprites/gun_parts/recoil_absorber.png",
        ModuleType::FiringPiece => "sprites/gun_parts/firing_piece.png",
        ModuleType::GaugeIncreaser => "sprites/gun_parts/gauge_increaser.png",
        ModuleType::GaugeReducer => "sprites/gun_parts/gauge_reducer.png",
        ModuleType::Loader => "sprites/gun_parts/autoloader.png",
        ModuleType::ClipRack => "sprites/gun_parts/ammo_clip.png",
        ModuleType::ShellFeeder => "sprites/gun_parts/ammo_feeder.png",
        ModuleType::BreechCooler => "sprites/gun_parts/cooler.png",
        ModuleType::GunBarrel => "sprites/gun_parts/barrel.png",
        ModuleType::Mantlet => "sprites/gun_parts/mantlet.png",
        ModuleType::RailMagnet => "sprites/gun_parts/rail_magnet.png",
        ModuleType::Magazine => "sprites/gun_parts/ammo_store.png",
        ModuleType::BeltConveyor => "sprites/gun_parts/conveyor.png",
        ModuleType::TurretRingSmall | ModuleType::TurretRing | ModuleType::TurretRingLarge =>
            "sprites/gun_parts/turret_ring.png",
        ModuleType::TraverseMotor => "sprites/gun_parts/traverse_motor.png",
        ModuleType::AmmoHoist => "sprites/gun_parts/ammo_hoist.png",
        ModuleType::LaserCore => "sprites/laser_parts/core.png",
        ModuleType::LaserCoupler => "sprites/laser_parts/coupler.png",
        ModuleType::LaserCavity => "sprites/laser_parts/cavity.png",
        ModuleType::LaserPump => "sprites/laser_parts/pump.png",
        ModuleType::QSwitch => "sprites/laser_parts/qswitch.png",
        ModuleType::FrequencyDoubler => "sprites/laser_parts/doubler.png",
        ModuleType::LaserDestabiliser => "sprites/laser_parts/destabiliser.png",
        ModuleType::LaserEmitter => "sprites/laser_parts/emitter.png",
        ModuleType::LaserLens => "sprites/laser_parts/lens.png",
        ModuleType::BeamRelay => "sprites/laser_parts/relay.png",
        ModuleType::Rangefinder => "sprites/gun_parts/rangefinder.png",
        ModuleType::RoofVent => "sprites/gun_parts/roof_vent.png",
        ModuleType::Periscope => "sprites/gun_parts/periscope.png",
        ModuleType::ArmouredHatch => "sprites/gun_parts/armoured_hatch.png",
        ModuleType::AntiFlashDoor => "sprites/gun_parts/anti_flash_door.png",
        ModuleType::OverchargeCapacitor => "sprites/modules/battery.png",
        ModuleType::BoreEvacuator => "sprites/modules/hull_beam.png",
        ModuleType::MagneticAccelerator => "sprites/modules/railgun.png",
        ModuleType::FocusingArray => "sprites/modules/depth_sensor.png",
        ModuleType::WarheadBay => "sprites/modules/cargo_hold.png",
        // Advanced reactor enhancers
        ModuleType::FuelEnrichmentUnit => "sprites/modules/small_reactor.png",
        ModuleType::ContainmentField => "sprites/modules/battery.png",
        ModuleType::EmergencyShutdown => "sprites/modules/battery.png",
        ModuleType::PowerRegulator => "sprites/modules/battery.png",
        // Advanced engine enhancers
        ModuleType::Afterburner => "sprites/modules/standard_engine.png",
        ModuleType::ThrustVectoring => "sprites/modules/standard_engine.png",
        ModuleType::FuelInjector => "sprites/modules/ballast_tank.png",
        ModuleType::InertialDampener => "sprites/modules/battery.png",
        // Defense modules
        ModuleType::DecoyLauncher => "sprites/modules/mine_layer.png",
        ModuleType::ChaffDispenser => "sprites/modules/mine_layer.png",
        ModuleType::AblativeArmor => "sprites/modules/hull_beam.png",
        ModuleType::PointDefenseDrone => "sprites/modules/point_defense.png",
        ModuleType::HullReinforcePlate => "sprites/modules/hull_beam.png",
        // Advanced utility
        ModuleType::SignalJammer => "sprites/modules/sonar_array.png",
        ModuleType::GravityCompensator => "sprites/modules/battery.png",
        ModuleType::RadiationHardening => "sprites/modules/hull_beam.png",
        ModuleType::EmergencyO2Cache => "sprites/modules/oxygen_scrubber.png",
        ModuleType::BlackBox => "sprites/modules/navigation.png",
        // Structural enhancers
        ModuleType::ReinforcedJoint => "sprites/modules/hull_beam.png",
        ModuleType::VibrationDamper => "sprites/modules/hull_beam.png",
        ModuleType::ThermalInsulator => "sprites/modules/hull_beam.png",
        ModuleType::StructuralBrace => "sprites/modules/hull_beam.png",
        ModuleType::CornerArmorPlate => "sprites/modules/hull_beam_2x2.png",
        ModuleType::BridgeWing => "sprites/modules/navigation_3x2.png",
        ModuleType::SurgicalBay => "sprites/modules/medical_bay_3x2.png",
        ModuleType::GalleyMess => "sprites/modules/basic_quarters_2x2.png",
        ModuleType::BulkCargoHold => "sprites/modules/cargo_hold_2x2.png",
        ModuleType::DockingHub => "sprites/modules/docking_port_3x3.png",
        ModuleType::WellnessHub => "sprites/modules/basic_quarters_3x3.png",
        ModuleType::StaggeredArmorPlate => "sprites/modules/hull_beam_3x2.png",
        ModuleType::AngledHullPlate => "sprites/modules/hull_beam.png",
        ModuleType::AngledArmorPlate => "sprites/modules/hull_beam.png",
        ModuleType::AngledAblativeArmor | ModuleType::ReflectivePlate | ModuleType::AngledReflectivePlate => "sprites/modules/hull_beam.png",
        ModuleType::BevelledArmorPlate => "sprites/modules/bevelled_plate.png",
        ModuleType::SlopeArmorPlateLow => "sprites/modules/slope_low.png",
        ModuleType::SlopeArmorPlateHigh => "sprites/modules/slope_high.png",
        ModuleType::SlopeArmorPlateLowMirror => "sprites/modules/slope_low_mirror.png",
        ModuleType::SlopeArmorPlateHighMirror => "sprites/modules/slope_high_mirror.png",
        ModuleType::HalfArmorPlate => "sprites/modules/half_plate.png",
    })
}

/// Sprite for a hull cell, given both its material and its layer.
///
/// Hallways get their own art rather than a tint of the armour plate. They
/// used to be that plate darkened to ~45%, which read as a burnt block and
/// sat within a few percent of the Void tint -- bad for the one layer crew
/// can actually walk on (crew/navigation.rs), where a player needs to trace a
/// route across the ship at a glance. Decking is material-independent: it is
/// the floor inside the hull, not the hull.
pub fn hull_layer_sprite_path(material: HullMaterial, layer: HullLayer) -> &'static str {
    match layer {
        HullLayer::Hallway => "sprites/hull/hull_hallway.png",
        _ => hull_sprite_path(material),
    }
}

/// The seam-aware variant of a hull tile, chosen by which neighbours match.
///
/// `mask` is the four-bit neighbour set: 1 north, 2 south, 4 west, 8 east,
/// where north is the cell at grid +y. Variant 00 is the untouched artwork, so
/// a tile that spawns with the plain texture and is corrected a frame later
/// does not flicker.
///
/// Derived from `hull_layer_sprite_path` rather than tabulated separately, so
/// the two cannot drift: change which texture a layer uses and its variants
/// follow. `tools/art/gen_hull_tiles.py` writes the files.
pub fn hull_layer_sprite_variant(
    material: HullMaterial,
    layer: HullLayer,
    mask: u8,
) -> String {
    let base = hull_layer_sprite_path(material, layer);
    let stem = base.strip_suffix(".png").unwrap_or(base);
    format!("{stem}_{:02}.png", mask & 0x0F)
}

pub fn hull_sprite_path(material: HullMaterial) -> &'static str {
    match material {
        HullMaterial::Steel => "sprites/hull/hull_steel.png",
        HullMaterial::Titanium => "sprites/hull/hull_titanium.png",
        HullMaterial::Composite => "sprites/hull/hull_composite.png",
        HullMaterial::AbyssalAlloy => "sprites/hull/hull_abyssal.png",
    }
}

pub fn creature_sprite_path(creature_type: CreatureType) -> &'static str {
    match creature_type {
        CreatureType::VoidDrifter => "sprites/creatures/ambient/void_drifter.png",
        CreatureType::Stalker => "sprites/creatures/hostile/stalker.png",
        CreatureType::Leviathan => "sprites/creatures/hostile/leviathan.png",
        CreatureType::ParasiteSwarm => "sprites/creatures/hostile/parasite.png",
    }
}

pub fn poi_sprite_path(poi_type: PoiType) -> &'static str {
    match poi_type {
        PoiType::Wreck => "sprites/environment/wreck.png",
        PoiType::Cave => "sprites/environment/cave.png",
        PoiType::Ruins => "sprites/environment/ruins.png",
        PoiType::ThermalVent => "sprites/environment/thermal_vent.png",
        PoiType::Settlement => "sprites/environment/settlement.png",
    }
}


/// Effect sprite paths for combat visuals
pub fn effect_sprite_path(effect: &str) -> &'static str {
    match effect {
        "torpedo" => "sprites/effects/torpedo_trail.png",
        "enemy_projectile" => "sprites/effects/enemy_projectile.png",
        "bubble" => "sprites/effects/bubble.png",
        "electric_shock" => "sprites/effects/electric_shock.png",
        "explosion" => "sprites/effects/explosion.png",
        "radar_ring" => "sprites/effects/sonar_ring.png",
        _ => "sprites/effects/torpedo_trail.png",
    }
}

/// Per-module sprite base rotation offset in radians.
/// Most sprites are drawn facing "up" (North = 0). Modules whose sprite
/// has a different natural orientation need an offset so that rotation
/// math works correctly.
/// Barrel-type weapons whose sprite is drawn long (1:3, turret centered,
/// barrel pointing "up"/North) so the barrel overhangs into the cells ahead.
/// spawner.rs sizes these tall and renders them on a higher z-layer so the
/// barrel draws over neighbouring blocks (Cosmoteer-style). Missile tubes,
/// mine layers and beam-only weapons are NOT barrel weapons.
pub fn is_barrel_weapon(module_type: ModuleType) -> bool {
    matches!(module_type,
        ModuleType::Cannon | ModuleType::Gatling | ModuleType::Coilgun | ModuleType::Railgun
        | ModuleType::MiningDrill | ModuleType::MagneticAccelerator | ModuleType::PointDefenseDrone)
}

pub fn sprite_base_rotation(module_type: ModuleType) -> f32 {
    use crate::components::ModuleCategory;
    // Base offset that aligns the sprite's native "facing" with the module's
    // placement Rotation. Derived from the two authored cases in the starter
    // ship (spawner.rs):
    //   • Engine art draws its exhaust pointing DOWN (South). Engines are
    //     placed Rotation::West (to_radians = +π/2) and must vent West (aft).
    //     South → West needs visual_angle = -π/2, so base = -π/2 - π/2 = -π.
    //   • Weapon art draws its muzzle pointing UP (North). Weapons are placed
    //     Rotation::East (to_radians = -π/2) and must fire East (forward).
    //     North → East needs visual_angle = -π/2, so base = 0.
    // Net: placement Rotation reads as the exhaust direction for engines and
    // the muzzle direction for weapons — the intuitive result at any angle.
    let is_engine = matches!(module_type.category(), ModuleCategory::Propulsion);
    let is_weapon = is_barrel_weapon(module_type)
        || matches!(module_type,
            ModuleType::Laser | ModuleType::PlasmaCaster | ModuleType::IonDisruptor
            | ModuleType::EMPPulse | ModuleType::HeavyMissile | ModuleType::GuidedMissile
            | ModuleType::ClusterRocket | ModuleType::LaunchPad | ModuleType::RailLauncher
            | ModuleType::VlsSilo | ModuleType::TractorBeam | ModuleType::SalvageArm);
    if is_engine {
        -std::f32::consts::PI
    } else if is_weapon {
        0.0
    } else {
        0.0
    }
}

/// How far a module's working end protrudes PAST its block, and which way.
/// Returns `(extension_world_units, protrude_sign)` where the sign is +1 when
/// the art's protruding end is the TOP (art +Y — gun barrels) and -1 when it's
/// the BOTTOM (art −Y — engine nozzles). The extension lengthens the sprite
/// along its local vertical axis and is offset so the housing stays centred on
/// the cell while the barrel/nozzle hangs off the end (rendered over the
/// neighbour via a raised Z, see spawner).
///
/// Tied to the specific sprite FILES whose art has actually been drawn with an
/// extended canvas — a module gets overhang only once its sprite includes the
/// protruding region, otherwise the square art would just stretch.
pub fn sprite_overhang(module_type: ModuleType) -> (f32, f32) {
    match module_sprite_path(module_type) {
        // Engines: nozzle vents aft (art bottom, −1). 1×1 sprites only — the
        // art aspect matches the cell exactly there, so the extension doesn't
        // distort (multi-cell overhang needs the base-size fix first).
        Some("sprites/modules/standard_engine.png") => (30.0, -1.0),
        Some("sprites/modules/silent_drive.png") => (24.0, -1.0),
        // Weapons: barrel/arm leads forward (art top, +1).
        // point_defense / railgun are turrets now — their barrels are separate
        // rotating sprites (see turret_barrel_sprite), so the base has no overhang.
        Some("sprites/modules/torpedo_tube.png") => (26.0, 1.0),
        // Missile/rocket launchers: tubes lie in the play plane pointing
        // forward and the warheads overhang the block, same 378x541 canvas
        // as the torpedo tube above.
        Some("sprites/modules/heavy_missile.png") => (26.0, 1.0),
        Some("sprites/modules/guided_missile.png") => (26.0, 1.0),
        Some("sprites/modules/cluster_rocket.png") => (26.0, 1.0),
        Some("sprites/modules/salvage_arm.png") => (34.0, 1.0),
        _ => (0.0, 0.0),
    }
}

/// The rotating barrel sprite for a gun turret, if this module is one. The base
/// mount stays static (the module's own sprite); this sprite is spawned as a
/// pivot-centred child the turret aim system rotates to point at the target.
pub fn turret_barrel_sprite(module_type: ModuleType) -> Option<&'static str> {
    match module_sprite_path(module_type) {
        Some("sprites/modules/point_defense.png") => Some("sprites/modules/turret_pd_barrel.png"),
        Some("sprites/modules/railgun.png") => Some("sprites/modules/turret_rg_barrel.png"),
        Some("sprites/modules/railgun_2x1.png") => Some("sprites/modules/turret_railgun_barrel.png"),
        Some("sprites/modules/cannon.png") => Some("sprites/modules/turret_cannon_barrel.png"),
        Some("sprites/modules/coilgun.png") => Some("sprites/modules/turret_coilgun_barrel.png"),
        Some("sprites/modules/gatling.png") => Some("sprites/modules/turret_gatling_barrel.png"),
        Some("sprites/modules/mining_drill.png") => Some("sprites/modules/turret_mining_drill_barrel.png"),
        _ => None,
    }
}

/// Base traverse speed (radians/sec) for a turret weapon — the "how fast it
/// tracks" knob, per weapon. Light autocannons whip around; the railgun is
/// ponderous. Per-weapon customization scales this (WeaponTuning::traverse).
pub fn turret_turn_speed(module_type: ModuleType) -> f32 {
    match module_type {
        ModuleType::Gatling | ModuleType::PointDefenseDrone => 6.5,
        ModuleType::Cannon => 3.2,
        ModuleType::Coilgun | ModuleType::Laser | ModuleType::PlasmaCaster
        | ModuleType::IonDisruptor | ModuleType::EMPPulse => 2.6,
        ModuleType::Railgun | ModuleType::MagneticAccelerator => 1.5,
        ModuleType::MiningDrill => 2.0,
        _ => 3.0,
    }
}

#[cfg(test)]
mod engine_facing_tests {
    /// Every saved design's main engines vent aft.
    ///
    /// A block's rotation names the way its exhaust points (see
    /// `sprite_base_rotation`), and every ship builds nose-right, so a main
    /// engine faces West. The hull-yard drew engines without the game's
    /// half-turn, so an engine that looked right there was saved East and
    /// flew with its exhaust pointing into the hull -- the starter pincer
    /// and five other designs did.
    #[test]
    fn every_designs_main_engines_vent_aft() {
        const MAIN: [&str; 8] = [
            "SmallEngine", "StandardEngine", "LargeEngine", "SilentDrive",
            "JetDrive", "EmergencyThruster", "VectorThruster", "Afterburner",
        ];
        let files = std::fs::read_dir("designs")
            .expect("designs/")
            .chain(std::fs::read_dir("designs/factions").expect("designs/factions/"))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"));
        let mut checked = 0;
        for path in files {
            let text = std::fs::read_to_string(&path).unwrap();
            let design: serde_json::Value = serde_json::from_str(&text).unwrap();
            for module in design["modules"].as_array().into_iter().flatten() {
                let kind = module["module_type"].as_str().unwrap_or("");
                if MAIN.contains(&kind) {
                    checked += 1;
                    assert_eq!(
                        module["rotation"].as_str(),
                        Some("West"),
                        "{} at {} in {} vents {} -- engines face aft (West)",
                        kind, module["grid_pos"], path.display(), module["rotation"],
                    );
                }
            }
        }
        assert!(checked > 0, "no engines found to check");
    }
}
