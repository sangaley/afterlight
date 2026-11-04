// Layout fields are part of the data model — not all are consumed yet.
#![allow(dead_code)]

use bevy::prelude::*;
use crate::components::{ModuleType, HullMaterial, HullLayer, Rotation};
use crate::combat::ammo_types::KineticAmmoType;
use super::components::AiShipType;

pub struct HullCellDef {
    pub grid_pos: IVec2,
    pub layer: HullLayer,
    pub material: HullMaterial,
}

pub struct ModulePlacement {
    pub module_type: ModuleType,
    pub grid_pos: IVec2,
    pub rotation: Rotation,
}

/// Combat loadout for one weapon placement, matched back to its
/// ModulePlacement by grid_pos (positions are unique within a ship) rather
/// than touching every ModulePlacement literal in this file. A weapon with
/// no matching entry gets pure registry defaults (fire group 0, 1.0x
/// tuning, AP ammo where applicable) — this is additive, not required.
pub struct WeaponLoadout {
    pub grid_pos: IVec2,
    pub fire_group: u8,
    pub tuning: crate::building::customization::tuning::WeaponTuning,
    pub ammo: Option<crate::combat::ammo_types::KineticAmmoType>,
}

/// Shorthand for a WeaponLoadout entry.
pub fn wl(
    grid_pos: IVec2,
    fire_group: u8,
    velocity: f32,
    fire_rate: f32,
    damage: f32,
    ammo: Option<crate::combat::ammo_types::KineticAmmoType>,
) -> WeaponLoadout {
    WeaponLoadout {
        grid_pos,
        fire_group,
        tuning: crate::building::customization::tuning::WeaponTuning { velocity, fire_rate, damage, traverse: 1.0 },
        ammo,
    }
}

pub struct AiShipLayout {
    pub hull_cells: Vec<HullCellDef>,
    pub modules: Vec<ModulePlacement>,
    pub body_size: Vec2,
    pub hull_material: HullMaterial,
    pub loadouts: Vec<WeaponLoadout>,
}

impl AiShipLayout {
    /// Converts a built-in layout to the canonical design format
    /// (building::blueprint). The layouts in this file are the FALLBACK —
    /// designs/factions/<slug>.json wins when present, and each layout
    /// self-exports there on first spawn. Edit the JSON, not this file.
    pub fn to_design(&self, name: &str) -> crate::building::blueprint::Blueprint {
        use crate::building::blueprint::{Blueprint, BlueprintHullCell, BlueprintModule, ModuleExtras, BLUEPRINT_VERSION};
        use crate::building::customization::tuning::SelectedAmmo;
        Blueprint {
            name: name.into(),
            hull_cells: self.hull_cells.iter().map(|c| BlueprintHullCell {
                grid_pos: c.grid_pos,
                layer: c.layer,
                material: c.material,
            }).collect(),
            modules: self.modules.iter().map(|m| {
                let loadout = self.loadouts.iter().find(|l| l.grid_pos == m.grid_pos);
                let extras = loadout.map(|l| ModuleExtras {
                    tuning: Some(l.tuning),
                    fire_group: Some(l.fire_group),
                    ammo: l.ammo.map(SelectedAmmo),
                    turret: None,
                    shell: None,
                    missile: None,
                    point_defence: false,
                    signature: None,
                });
                BlueprintModule {
                    module_type: m.module_type,
                    grid_pos: m.grid_pos,
                    rotation: m.rotation,
                    custom_name: None,
                    subcomponents: None,
                    extras,
                }
            }).collect(),
            created_at: "builtin".into(),
            version: BLUEPRINT_VERSION,
            guns: Vec::new(),
        }
    }
}

/// Stable file name per faction (designs/factions/<slug>.json).
/// Filename stem for this faction's design under `designs/factions/`.
///
/// These MUST match the files on disk. `faction_design()` in spawner.rs
/// silently falls back to the built-in layout and re-exports it when a file is
/// missing, so a slug that no longer matches quietly replaces a hand-armoured
/// hull with the old one and reports nothing.
pub fn design_slug(ship_type: AiShipType) -> &'static str {
    match ship_type {
        AiShipType::StellarPreserve => "stellar_preserve",
        AiShipType::SynthesisCollective => "synthesis_collective",
        AiShipType::BrokenChoir => "broken_choir",
        AiShipType::CorpseStars => "corpse_stars",
        AiShipType::TheSilence => "the_silence",
        AiShipType::TerranHegemony => "terran_hegemony",
        AiShipType::GildedThrone => "gilded_throne",
        AiShipType::RecursiveKingdom => "recursive_kingdom",
        AiShipType::EternalHegemony => "eternal_hegemony",
        AiShipType::Shepherd => "shepherd",
        AiShipType::MerchantPrinces => "merchant_princes",
        AiShipType::FreeHaulersCompact => "free_haulers_compact",
    }
}


pub fn get_layout(ship_type: AiShipType) -> AiShipLayout {
    match ship_type {
        AiShipType::StellarPreserve => stellar_preserve_layout(),
        AiShipType::SynthesisCollective => synthesis_collective_layout(),
        AiShipType::BrokenChoir => broken_choir_layout(),
        AiShipType::CorpseStars => corpse_stars_layout(),
        AiShipType::TheSilence => the_silence_layout(),
        AiShipType::TerranHegemony => terran_hegemony_layout(),
        AiShipType::GildedThrone => gilded_throne_layout(),
        AiShipType::RecursiveKingdom => recursive_kingdom_layout(),
        AiShipType::EternalHegemony => eternal_hegemony_layout(),
        AiShipType::Shepherd => shepherd_layout(),
        // Never flown: both have hand-drawn rosters and a design file of
        // their own (designs/factions/<slug>.json). A stand-in so a missing
        // file still builds a ship.
        AiShipType::MerchantPrinces | AiShipType::FreeHaulersCompact => the_silence_layout(),
    }
}

/// Adapt shared armour placements into this file's ModulePlacement.
fn plates(v: Vec<(IVec2, ModuleType, Rotation)>) -> Vec<ModulePlacement> {
    v.into_iter()
        .map(|(grid_pos, module_type, rotation)| ModulePlacement { module_type, grid_pos, rotation })
        .collect()
}

/// Helper: build ship-shaped hull from row definitions (y, x_min, x_max)
fn build_shaped_hull(rows: &[(i32, i32, i32)], material: HullMaterial) -> Vec<HullCellDef> {
    let mut hull_cells = Vec::new();
    for &(y, x_min, x_max) in rows {
        for x in x_min..=x_max {
            let is_top = !rows.iter().any(|&(ry, rxmin, rxmax)| ry == y + 1 && x >= rxmin && x <= rxmax);
            let is_bot = !rows.iter().any(|&(ry, rxmin, rxmax)| ry == y - 1 && x >= rxmin && x <= rxmax);
            let is_left = x == x_min;
            let is_right = x == x_max;
            let layer = if is_top || is_bot || is_left || is_right { HullLayer::Outer } else { HullLayer::Inner };
            hull_cells.push(HullCellDef { grid_pos: IVec2::new(x, y), layer, material });
        }
    }
    hull_cells
}

fn hull_size(rows: &[(i32, i32, i32)]) -> Vec2 {
    let x_min = rows.iter().map(|r| r.1).min().unwrap_or(0);
    let x_max = rows.iter().map(|r| r.2).max().unwrap_or(0);
    let y_min = rows.iter().map(|r| r.0).min().unwrap_or(0);
    let y_max = rows.iter().map(|r| r.0).max().unwrap_or(0);
    Vec2::new((x_max - x_min + 1) as f32 * 66.0, (y_max - y_min + 1) as f32 * 66.0)
}

// ============================================================================
// STELLAR PRESERVE - Creature-towed ship with harness/capture gear
// Organic-looking, wide for creature containment, net launchers on sides
// ============================================================================
fn stellar_preserve_layout() -> AiShipLayout {
    let material = HullMaterial::Steel;
    let rows: &[(i32, i32, i32)] = &[
        ( 3,   4,  9),
        ( 2,   0, 11),
        ( 1,  -3, 10),
        ( 0,  -4, 11),
        (-1,  -4, 10),
        (-2,  -3,  9),
        (-3,   1,  6),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // Small backup engines (main movement is creature-towed)
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-3, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-3, -1), rotation: Rotation::West },
        // Power
        ModulePlacement { module_type: ModuleType::SmallReactor, grid_pos: IVec2::new(-1, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(0, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::ManeuverThruster, grid_pos: IVec2::new(-2, 0), rotation: Rotation::North },
        // Creature containment — now a proper menagerie
        ModulePlacement { module_type: ModuleType::CreatureContainment, grid_pos: IVec2::new(2, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::CreatureContainment, grid_pos: IVec2::new(2, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::SpecimenVault, grid_pos: IVec2::new(1, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::SpecimenVault, grid_pos: IVec2::new(3, 0), rotation: Rotation::North },
        // Net launchers and light weapons on hull edges
        ModulePlacement { module_type: ModuleType::TractorBeam, grid_pos: IVec2::new(9, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::TractorBeam, grid_pos: IVec2::new(9, -1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(8, 2), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(8, -2), rotation: Rotation::South },
        // Scanners for finding creatures
        ModulePlacement { module_type: ModuleType::CreatureScanner, grid_pos: IVec2::new(6, 2), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Floodlight, grid_pos: IVec2::new(10, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::BasicQuarters, grid_pos: IVec2::new(5, 0), rotation: Rotation::North },
    ];
    // Avoids combat entirely (flee-only per ai_brain.rs) — no combat
    // identity to tune, registry defaults on its pair of defensive Gatlings.
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledHullPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledHullPlate, 2)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts: vec![] }
}

// ============================================================================
// SYNTHESIS COLLECTIVE - Bio-organic hybrid, eerie bioluminescent, self-healing hull
// Composite material (organic), acid/electric bio-weapons
// ============================================================================
fn synthesis_collective_layout() -> AiShipLayout {
    let material = HullMaterial::Composite;
    // Organic, bulbous shape
    let rows: &[(i32, i32, i32)] = &[
        ( 4,   5,  8),
        ( 3,   2, 10),
        ( 2,  -1, 11),
        ( 1,  -3, 10),
        ( 0,  -4,  9),
        (-1,  -4,  8),
        (-2,  -3,  9),
        (-3,  -1,  9),
        (-4,   3, 10),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // "Creature heart" reactor cluster (standard reactors reflavored)
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(1, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(1, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(2, 0), rotation: Rotation::North },
        // Engines
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-3, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-3, -1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-3, 0), rotation: Rotation::West },
        // Thruster
        ModulePlacement { module_type: ModuleType::ManeuverThruster, grid_pos: IVec2::new(-1, 0), rotation: Rotation::North },
        // Bio-weapons on exterior — more coverage
        ModulePlacement { module_type: ModuleType::PlasmaCaster, grid_pos: IVec2::new(9, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::PlasmaCaster, grid_pos: IVec2::new(9, -2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::PlasmaCaster, grid_pos: IVec2::new(8, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::PlasmaCaster, grid_pos: IVec2::new(7, 2), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(6, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(7, -3), rotation: Rotation::South },
        // Healing/support — this hull regenerates
        ModulePlacement { module_type: ModuleType::RepairBay, grid_pos: IVec2::new(3, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::RepairBay, grid_pos: IVec2::new(4, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::BasicQuarters, grid_pos: IVec2::new(5, 0), rotation: Rotation::North },
    ];
    // Zealots: reckless power draw (quadratic tuning cost be damned),
    // fire fast and hot on everything they've got.
    let loadouts = vec![
        wl(IVec2::new(9, 1), 0, 1.1, 1.2, 1.25, None),
        wl(IVec2::new(9, -2), 0, 1.1, 1.2, 1.25, None),
        wl(IVec2::new(8, 0), 0, 1.1, 1.2, 1.25, None),
        wl(IVec2::new(7, 2), 0, 1.1, 1.2, 1.25, None),
        wl(IVec2::new(6, 0), 1, 1.0, 1.1, 1.15, None),
        wl(IVec2::new(7, -3), 1, 1.0, 1.1, 1.15, None),
    ];
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledArmorPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledArmorPlate, 2)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts }
}

// ============================================================================
// THE BROKEN CHOIR - Ghost ships, partially destroyed, holes in hull
// Steel (rusted), modules randomly missing, eerie design
// ============================================================================
fn broken_choir_layout() -> AiShipLayout {
    let material = HullMaterial::Steel;
    // Damaged, asymmetric shape (holes represented by missing cells)
    let rows: &[(i32, i32, i32)] = &[
        ( 3,   6, 10),
        ( 2,   2, 12),
        ( 1,  -2, 12),
        ( 0,  -3, 12),
        (-1,  -3, 11),
        (-2,  -2,  9),
        (-3,   2,  9),
        (-4,   6, 10),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // Barely functional engines
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-2, 0), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-2, -1), rotation: Rotation::West },
        // Flickering reactors
        ModulePlacement { module_type: ModuleType::SmallReactor, grid_pos: IVec2::new(1, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::SmallReactor, grid_pos: IVec2::new(0, -1), rotation: Rotation::North },
        // Random weapons still active
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(12, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(10, -4), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(6, 2), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(7, -4), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::ClusterRocket, grid_pos: IVec2::new(12, 1), rotation: Rotation::East },
        // Empty quarters (no crew)
        ModulePlacement { module_type: ModuleType::BasicQuarters, grid_pos: IVec2::new(4, 0), rotation: Rotation::North },
        // Old cargo
        ModulePlacement { module_type: ModuleType::SmallCargo, grid_pos: IVec2::new(6, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::SmallCargo, grid_pos: IVec2::new(3, -2), rotation: Rotation::North },
    ];
    // Battle-damaged and mindless: worn warheads (damage down), gatlings
    // spraying fire indiscriminately (fire rate up, accuracy/damage down),
    // Incendiary fits the derelict-ghost-ship theme.
    let loadouts = vec![
        wl(IVec2::new(6, 2), 0, 1.0, 1.3, 0.85, Some(KineticAmmoType::Incendiary)),
        wl(IVec2::new(7, -4), 0, 1.0, 1.3, 0.85, Some(KineticAmmoType::Incendiary)),
        wl(IVec2::new(12, 0), 1, 0.9, 1.0, 0.9, None),
        wl(IVec2::new(10, -4), 1, 0.9, 1.0, 0.9, None),
        wl(IVec2::new(12, 1), 1, 0.9, 1.0, 0.9, None),
    ];
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledHullPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledHullPlate, 2)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts }
}

// ============================================================================
// CORPSE STARS - Deep-zone heavy tanks, abyssal alloy, pressure weapons
// Compact, dense, extremely armored
// ============================================================================
fn corpse_stars_layout() -> AiShipLayout {
    let material = HullMaterial::AbyssalAlloy;
    // Dense, compact diamond shape
    let rows: &[(i32, i32, i32)] = &[
        ( 4,  -2,  4),
        ( 3,  -3,  7),
        ( 2,  -4, 10),
        ( 1,  -4, 12),
        ( 0,  -5, 13),
        (-1,  -5, 13),
        (-2,  -4, 12),
        (-3,  -4, 10),
        (-4,  -3,  7),
        (-5,  -2,  4),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // Powerful engines for ramming
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-3, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-3, -1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-3, 0), rotation: Rotation::West },
        // Heavy power
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(1, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(1, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::LargeReactor, grid_pos: IVec2::new(0, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::ManeuverThruster, grid_pos: IVec2::new(-1, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(-2, 0), rotation: Rotation::North },
        // Pressure weapons on edges — a full battery now
        ModulePlacement { module_type: ModuleType::EMPPulse, grid_pos: IVec2::new(11, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(11, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(11, -2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(12, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(10, 2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(9, -3), rotation: Rotation::South },
        // Reinforced interior
        ModulePlacement { module_type: ModuleType::RepairBay, grid_pos: IVec2::new(4, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::AdvancedRepairBay, grid_pos: IVec2::new(5, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::BasicQuarters, grid_pos: IVec2::new(6, 0), rotation: Rotation::North },
        // Deep sensors
        ModulePlacement { module_type: ModuleType::DepthScanner, grid_pos: IVec2::new(7, 0), rotation: Rotation::East },
    ];
    // Crushing pressure made physical: slow, heavy hits. HESH (shockwave
    // through armor, no penetration needed) over AP — this is about
    // crushing force, not piercing.
    let loadouts = vec![
        wl(IVec2::new(11, 1), 1, 1.0, 0.9, 1.2, None),
        wl(IVec2::new(11, -2), 1, 1.0, 0.9, 1.2, None),
        wl(IVec2::new(12, 0), 0, 1.1, 0.7, 1.4, Some(KineticAmmoType::HESH)),
        wl(IVec2::new(10, 2), 0, 1.1, 0.7, 1.4, Some(KineticAmmoType::HESH)),
        wl(IVec2::new(9, -3), 2, 1.0, 1.0, 1.1, None),
        wl(IVec2::new(11, 0), 3, 1.0, 1.0, 1.0, None),
    ];
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledArmorPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledArmorPlate, 3)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts }
}

// ============================================================================
// THE SILENCE - Stealth surveillance, narrow, sensor-heavy, no weapons
// Composite, silent drive, fastest flee speed
// ============================================================================
fn the_silence_layout() -> AiShipLayout {
    let material = HullMaterial::Composite;
    // Long, thin needle shape
    let rows: &[(i32, i32, i32)] = &[
        ( 3,   3,  6),
        ( 2,   2,  8),
        ( 1,  -5, 14),
        ( 0,  -6, 15),
        (-1,  -6, 15),
        (-2,   2,  8),
        (-3,   3,  6),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // Silent engines
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-4, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-4, -1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-4, 0), rotation: Rotation::West },
        // Quiet reactors
        ModulePlacement { module_type: ModuleType::SmallReactor, grid_pos: IVec2::new(-1, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::SmallReactor, grid_pos: IVec2::new(0, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::ManeuverThruster, grid_pos: IVec2::new(-2, 0), rotation: Rotation::North },
        // Stealth coating
        ModulePlacement { module_type: ModuleType::StealthCoating, grid_pos: IVec2::new(1, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StealthCoating, grid_pos: IVec2::new(2, 0), rotation: Rotation::North },
        // Massive sensor array (the "glass eye") — the whole point of the ship
        ModulePlacement { module_type: ModuleType::AdvancedRadar, grid_pos: IVec2::new(11, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::PassiveRadar, grid_pos: IVec2::new(10, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::PassiveRadar, grid_pos: IVec2::new(10, -1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HydrophoneArray, grid_pos: IVec2::new(12, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::CreatureScanner, grid_pos: IVec2::new(8, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::MineralScanner, grid_pos: IVec2::new(6, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::DepthScanner, grid_pos: IVec2::new(4, 0), rotation: Rotation::East },
        // Signal buoy (broadcasts intel)
        ModulePlacement { module_type: ModuleType::SignalBuoy, grid_pos: IVec2::new(13, 0), rotation: Rotation::East },
    ];
    // Carries zero weapons — nothing to tune.
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledHullPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledHullPlate, 2)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts: vec![] }
}

// ============================================================================
// TERRAN HEGEMONY - Heavy battleship, massive hull, multiple weapon systems.
// Titanium, slow but devastating firepower — the strongest "normal" faction,
// though the true bosses (Eternal Hegemony, The Shepherd) now dwarf even this.
// ============================================================================
fn terran_hegemony_layout() -> AiShipLayout {
    let material = HullMaterial::Titanium;
    // Massive wide battleship
    let rows: &[(i32, i32, i32)] = &[
        ( 5,   5,  9),
        ( 4,   2, 11),
        ( 3,   0, 13),
        ( 2,  -2, 14),
        ( 1,  -4, 15),
        ( 0,  -5, 15),
        (-1,  -5, 15),
        (-2,  -4, 15),
        (-3,  -2, 14),
        (-4,   0, 13),
        (-5,   2, 11),
        (-6,   5,  9),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // 4 large engines
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-4, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-4, -1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-2, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-2, -1), rotation: Rotation::West },
        // Heavy power plant
        ModulePlacement { module_type: ModuleType::LargeReactor, grid_pos: IVec2::new(2, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(2, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(4, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(0, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(0, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::ManeuverThruster, grid_pos: IVec2::new(1, 0), rotation: Rotation::North },
        // Crew
        ModulePlacement { module_type: ModuleType::Barracks, grid_pos: IVec2::new(6, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::BasicQuarters, grid_pos: IVec2::new(7, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::MessHall, grid_pos: IVec2::new(7, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::RepairBay, grid_pos: IVec2::new(3, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::AdvancedRepairBay, grid_pos: IVec2::new(3, -1), rotation: Rotation::North },
        // Weapons array (devastating)
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(13, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(13, 2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(13, -3), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(15, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(15, -2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(11, 4), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Cannon, grid_pos: IVec2::new(9, 3), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Cannon, grid_pos: IVec2::new(9, -4), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(8, 2), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(8, -3), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::PointDefenseDrone, grid_pos: IVec2::new(5, 3), rotation: Rotation::North },
        // Bridge
        ModulePlacement { module_type: ModuleType::HelmStation, grid_pos: IVec2::new(9, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::RadarArray, grid_pos: IVec2::new(11, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::TargetingComputer, grid_pos: IVec2::new(10, -1), rotation: Rotation::East },
    ];
    // Tanky battleship: hits hard, doesn't rush. APFSDS on the railguns
    // (electromagnetic accelerator + fastest penetrator is a natural pair).
    let loadouts = vec![
        wl(IVec2::new(9, 3), 0, 1.0, 0.9, 1.2, Some(KineticAmmoType::APHE)),
        wl(IVec2::new(9, -4), 0, 1.0, 0.9, 1.2, Some(KineticAmmoType::APHE)),
        wl(IVec2::new(8, 2), 1, 1.0, 1.1, 1.0, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(8, -3), 1, 1.0, 1.1, 1.0, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(13, 0), 2, 1.1, 0.8, 1.3, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(13, 2), 2, 1.1, 0.8, 1.3, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(13, -3), 2, 1.1, 0.8, 1.3, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(15, 1), 3, 1.0, 1.0, 1.1, None),
        wl(IVec2::new(15, -2), 3, 1.0, 1.0, 1.1, None),
        wl(IVec2::new(11, 4), 3, 1.0, 1.0, 1.1, None),
    ];
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledArmorPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledArmorPlate, 3)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts }
}

// ============================================================================
// GILDED THRONE - Elite tactical ship, balanced, flanking design
// Titanium, well-armed but not overkill, designed for coordination
// ============================================================================
fn gilded_throne_layout() -> AiShipLayout {
    let material = HullMaterial::Titanium;
    let rows: &[(i32, i32, i32)] = &[
        ( 3,   1, 11),
        ( 2,  -2, 13),
        ( 1,  -3, 14),
        ( 0,  -3, 14),
        (-1,  -3, 14),
        (-2,  -2, 13),
        (-3,   1, 11),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // Fast engines
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-2, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-2, -1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-2, 0), rotation: Rotation::West },
        // Power
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(1, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(0, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::ManeuverThruster, grid_pos: IVec2::new(0, -1), rotation: Rotation::North },
        // Tactical systems
        ModulePlacement { module_type: ModuleType::TargetingComputer, grid_pos: IVec2::new(4, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::RadarArray, grid_pos: IVec2::new(10, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::RepairBay, grid_pos: IVec2::new(3, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StealthCoating, grid_pos: IVec2::new(2, 0), rotation: Rotation::North },
        // Crew
        ModulePlacement { module_type: ModuleType::BasicQuarters, grid_pos: IVec2::new(5, 0), rotation: Rotation::North },
        // Weapons (precise, not overwhelming, but more of it)
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(14, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(14, -1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(11, 2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(8, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(8, -2), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(6, 2), rotation: Rotation::North },
    ];
    // Tactical mercs: precise over spray-and-pray. APFSDS on the railgun —
    // fastest, sharpest penetrator for a single clean kill shot.
    let loadouts = vec![
        wl(IVec2::new(8, 1), 0, 1.0, 1.2, 1.0, Some(KineticAmmoType::AP)),
        wl(IVec2::new(8, -2), 0, 1.0, 1.2, 1.0, Some(KineticAmmoType::AP)),
        wl(IVec2::new(11, 2), 1, 1.3, 0.9, 1.2, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(14, 0), 2, 1.0, 1.0, 1.05, None),
        wl(IVec2::new(14, -1), 2, 1.0, 1.0, 1.05, None),
        wl(IVec2::new(6, 2), 3, 1.0, 1.0, 1.1, None),
    ];
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledArmorPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledArmorPlate, 2)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts }
}

// ============================================================================
// RECURSIVE KINGDOM - Tiny junk ships, minimal systems, expendable
// Steel (rusted), asymmetric, few modules, kamikaze tendencies
// ============================================================================
fn recursive_kingdom_layout() -> AiShipLayout {
    let material = HullMaterial::Steel;
    // Tiny asymmetric junk ship — a bit bigger than before, but still the
    // smallest thing flying. "Tiny and expendable" is the whole point.
    let rows: &[(i32, i32, i32)] = &[
        ( 2,   1,  4),
        ( 1,  -1,  6),
        ( 0,  -2,  7),
        (-1,  -2,  5),
        (-2,   0,  3),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // One sputtering engine
        ModulePlacement { module_type: ModuleType::SmallEngine, grid_pos: IVec2::new(-1, 0), rotation: Rotation::West },
        // Tiny reactor barely keeping things running
        ModulePlacement { module_type: ModuleType::SmallReactor, grid_pos: IVec2::new(0, 0), rotation: Rotation::North },
        // Two weapons now (mine layer + a scavenged gun - cheap and dirty)
        ModulePlacement { module_type: ModuleType::ClusterRocket, grid_pos: IVec2::new(7, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(4, 1), rotation: Rotation::North },
        // Scrap cargo
        ModulePlacement { module_type: ModuleType::SmallCargo, grid_pos: IVec2::new(2, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::SmallCargo, grid_pos: IVec2::new(3, -1), rotation: Rotation::North },
    ];
    // Junk weapons spraying everything they've got — fire rate way up,
    // damage/accuracy down. Flak fits scrappy proximity-fused junk shells
    // and gives paper-shield swarmers rare anti-missile utility.
    let loadouts = vec![
        wl(IVec2::new(4, 1), 0, 1.0, 1.5, 0.7, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(7, 0), 1, 1.0, 1.3, 0.8, None),
    ];
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledHullPlate)));
    // Scrap hull: a couple of salvaged plates bolted on, not a belt. At 34
    // cells anything more buries the ship in armour it has no business carrying.
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledHullPlate, 1)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts }
}

// ============================================================================
// ETERNAL HEGEMONY - Terran Hegemony's design taken to its limit: a true mega-battleship.
// Titanium, roughly 1.5x Terran Hegemony's footprint in every dimension, with
// weapon coverage to match. Spawns only far past the star system — finding
// one at all is most of the fight.
// ============================================================================
fn eternal_hegemony_layout() -> AiShipLayout {
    let material = HullMaterial::Titanium;
    let rows: &[(i32, i32, i32)] = &[
        ( 7,   8, 12),
        ( 6,   5, 15),
        ( 5,   2, 17),
        ( 4,   0, 18),
        ( 3,  -3, 18),
        ( 2,  -5, 18),
        ( 1,  -7, 18),
        ( 0,  -8, 18),
        (-1,  -8, 18),
        (-2,  -7, 18),
        (-3,  -5, 18),
        (-4,  -3, 18),
        (-5,   0, 18),
        (-6,   2, 17),
        (-7,   5, 15),
        (-8,   8, 12),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // Six large engines — this thing is heavy
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-6, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-6, 0), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-6, -1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-4, 1), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-4, -1), rotation: Rotation::West },
        // Power plant — a small city's worth
        ModulePlacement { module_type: ModuleType::LargeReactor, grid_pos: IVec2::new(3, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::LargeReactor, grid_pos: IVec2::new(3, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(5, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(1, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(1, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(1, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::ManeuverThruster, grid_pos: IVec2::new(2, 0), rotation: Rotation::North },
        // Crew — a real complement
        ModulePlacement { module_type: ModuleType::Barracks, grid_pos: IVec2::new(7, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Barracks, grid_pos: IVec2::new(7, -1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::BasicQuarters, grid_pos: IVec2::new(8, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::MessHall, grid_pos: IVec2::new(9, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::AdvancedRepairBay, grid_pos: IVec2::new(4, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::AdvancedRepairBay, grid_pos: IVec2::new(4, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::RepairBay, grid_pos: IVec2::new(4, 0), rotation: Rotation::North },
        // Weapons array — nearly double Terran Hegemony's coverage
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(16, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(16, 3), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(16, -4), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(15, 5), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(18, 2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(18, -3), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(18, 4), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(18, -5), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Cannon, grid_pos: IVec2::new(11, 5), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Cannon, grid_pos: IVec2::new(11, -6), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::Cannon, grid_pos: IVec2::new(9, 6), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Cannon, grid_pos: IVec2::new(9, -7), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(10, 4), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(10, -5), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(7, 4), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(7, -5), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(12, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(12, -2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::PointDefenseDrone, grid_pos: IVec2::new(6, 3), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::PointDefenseDrone, grid_pos: IVec2::new(6, -4), rotation: Rotation::South },
        // Bridge
        ModulePlacement { module_type: ModuleType::HelmStation, grid_pos: IVec2::new(12, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::RadarArray, grid_pos: IVec2::new(14, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::TargetingComputer, grid_pos: IVec2::new(13, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::TargetingComputer, grid_pos: IVec2::new(15, 0), rotation: Rotation::East },
    ];
    // Overwhelming firepower across the board — grinds anything down with
    // sheer coverage rather than any single specialty.
    let loadouts = vec![
        wl(IVec2::new(11, 5), 0, 1.0, 0.9, 1.2, Some(KineticAmmoType::APHE)),
        wl(IVec2::new(11, -6), 0, 1.0, 0.9, 1.2, Some(KineticAmmoType::APHE)),
        wl(IVec2::new(9, 6), 0, 1.0, 0.9, 1.2, Some(KineticAmmoType::APHE)),
        wl(IVec2::new(9, -7), 0, 1.0, 0.9, 1.2, Some(KineticAmmoType::APHE)),
        wl(IVec2::new(10, 4), 1, 1.0, 1.1, 1.05, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(10, -5), 1, 1.0, 1.1, 1.05, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(7, 4), 1, 1.0, 1.1, 1.05, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(7, -5), 1, 1.0, 1.1, 1.05, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(12, 1), 1, 1.0, 1.0, 1.15, None),
        wl(IVec2::new(12, -2), 1, 1.0, 1.0, 1.15, None),
        wl(IVec2::new(16, 0), 2, 1.1, 0.85, 1.25, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(16, 3), 2, 1.1, 0.85, 1.25, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(16, -4), 2, 1.1, 0.85, 1.25, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(15, 5), 2, 1.1, 0.85, 1.25, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(18, 2), 3, 1.0, 1.0, 1.15, None),
        wl(IVec2::new(18, -3), 3, 1.0, 1.0, 1.15, None),
        wl(IVec2::new(18, 4), 3, 1.0, 1.0, 1.15, None),
        wl(IVec2::new(18, -5), 3, 1.0, 1.0, 1.15, None),
    ];
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledArmorPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledArmorPlate, 3)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts }
}

// ============================================================================
// THE SHEPHERD - The largest, hardest kill in the game. Synthesis Collective's organic
// hull language taken to a monstrous scale, armed like a Eternal Hegemony and
// self-healing like the Cult it's descended from. Spawns beyond everything
// else in explored space.
// ============================================================================
fn shepherd_layout() -> AiShipLayout {
    let material = HullMaterial::AbyssalAlloy;
    let rows: &[(i32, i32, i32)] = &[
        ( 8,  14, 20),
        ( 7,  10, 21),
        ( 6,   5, 22),
        ( 5,   1, 23),
        ( 4,  -3, 23),
        ( 3,  -6, 23),
        ( 2,  -9, 23),
        ( 1, -11, 23),
        ( 0, -12, 23),
        (-1, -12, 23),
        (-2, -11, 23),
        (-3,  -9, 23),
        (-4,  -6, 23),
        (-5,  -3, 23),
        (-6,   1, 23),
        (-7,   5, 22),
        (-8,  10, 21),
        (-9,  14, 20),
    ];
    let hull_cells = build_shaped_hull(rows, material);
    let mut modules = vec![
        // Massive engine cluster
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-9, 2), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-9, 0), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::LargeEngine, grid_pos: IVec2::new(-9, -2), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-6, 2), rotation: Rotation::West },
        ModulePlacement { module_type: ModuleType::StandardEngine, grid_pos: IVec2::new(-6, -2), rotation: Rotation::West },
        // Reactor core — the "heart" (Cult lineage)
        ModulePlacement { module_type: ModuleType::LargeReactor, grid_pos: IVec2::new(2, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::LargeReactor, grid_pos: IVec2::new(2, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(4, 0), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::StandardReactor, grid_pos: IVec2::new(0, 2), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(-2, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::FuelTank, grid_pos: IVec2::new(-2, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::ManeuverThruster, grid_pos: IVec2::new(-1, 0), rotation: Rotation::North },
        // Self-healing organic tissue — extensive, like the Cult
        ModulePlacement { module_type: ModuleType::AdvancedRepairBay, grid_pos: IVec2::new(6, 1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::AdvancedRepairBay, grid_pos: IVec2::new(6, -1), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::RepairBay, grid_pos: IVec2::new(5, 2), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::RepairBay, grid_pos: IVec2::new(5, -2), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::BasicQuarters, grid_pos: IVec2::new(7, 0), rotation: Rotation::North },
        // Overwhelming firepower — bio-weapons and conventional side by side
        ModulePlacement { module_type: ModuleType::PlasmaCaster, grid_pos: IVec2::new(20, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::PlasmaCaster, grid_pos: IVec2::new(20, -1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::PlasmaCaster, grid_pos: IVec2::new(21, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(19, 4), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(19, -5), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(18, 6), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::Railgun, grid_pos: IVec2::new(18, -7), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(23, 3), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(23, -4), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(23, 5), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::HeavyMissile, grid_pos: IVec2::new(23, -6), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(15, 2), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(15, -3), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(14, 7), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::IonDisruptor, grid_pos: IVec2::new(14, -8), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::Cannon, grid_pos: IVec2::new(12, 6), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Cannon, grid_pos: IVec2::new(12, -7), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(10, 7), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::Gatling, grid_pos: IVec2::new(10, -8), rotation: Rotation::South },
        ModulePlacement { module_type: ModuleType::PointDefenseDrone, grid_pos: IVec2::new(8, 6), rotation: Rotation::North },
        ModulePlacement { module_type: ModuleType::PointDefenseDrone, grid_pos: IVec2::new(8, -7), rotation: Rotation::South },
        // Sensors — it sees everything coming
        ModulePlacement { module_type: ModuleType::AdvancedRadar, grid_pos: IVec2::new(13, 0), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::TargetingComputer, grid_pos: IVec2::new(13, 1), rotation: Rotation::East },
        ModulePlacement { module_type: ModuleType::TargetingComputer, grid_pos: IVec2::new(13, -1), rotation: Rotation::East },
    ];
    // The apex: strongest tuning in the game across every weapon it
    // carries. The hardest kill in the game should feel like it.
    let loadouts = vec![
        wl(IVec2::new(12, 6), 0, 1.0, 1.0, 1.3, Some(KineticAmmoType::APHE)),
        wl(IVec2::new(12, -7), 0, 1.0, 1.0, 1.3, Some(KineticAmmoType::APHE)),
        wl(IVec2::new(10, 7), 0, 1.0, 1.2, 1.15, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(10, -8), 0, 1.0, 1.2, 1.15, Some(KineticAmmoType::Flak)),
        wl(IVec2::new(15, 2), 1, 1.0, 1.0, 1.3, None),
        wl(IVec2::new(15, -3), 1, 1.0, 1.0, 1.3, None),
        wl(IVec2::new(14, 7), 1, 1.0, 1.0, 1.3, None),
        wl(IVec2::new(14, -8), 1, 1.0, 1.0, 1.3, None),
        wl(IVec2::new(19, 4), 2, 1.2, 0.9, 1.4, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(19, -5), 2, 1.2, 0.9, 1.4, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(18, 6), 2, 1.2, 0.9, 1.4, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(18, -7), 2, 1.2, 0.9, 1.4, Some(KineticAmmoType::APFSDS)),
        wl(IVec2::new(20, 1), 2, 1.15, 1.0, 1.35, None),
        wl(IVec2::new(20, -1), 2, 1.15, 1.0, 1.35, None),
        wl(IVec2::new(21, 0), 2, 1.15, 1.0, 1.35, None),
        wl(IVec2::new(23, 3), 3, 1.0, 1.05, 1.25, None),
        wl(IVec2::new(23, -4), 3, 1.0, 1.05, 1.25, None),
        wl(IVec2::new(23, 5), 3, 1.0, 1.05, 1.25, None),
        wl(IVec2::new(23, -6), 3, 1.0, 1.05, 1.25, None),
    ];
    modules.extend(plates(crate::building::armour::belt(rows, ModuleType::AngledArmorPlate)));
    modules.extend(plates(crate::building::armour::caps(rows, ModuleType::AngledArmorPlate, 3)));
    AiShipLayout { hull_cells, modules, body_size: hull_size(rows), hull_material: material, loadouts }
}

#[cfg(test)]
mod layout_tests {
    /// Every launcher on every faction ship must have a clear lane out.
    ///
    /// This is the invariant behind the cook-off: a buried tube detonates in
    /// its own hull, so a ship that ships with one destroys itself the first
    /// time it fires. Guarding it here means an outline change that walls in
    /// a silo fails the build instead of the fleet.
    #[test]
    fn no_launcher_is_entombed() {
        let mut failures: Vec<String> = Vec::new();

        for &ship_type in ALL.iter() {
            let layout = get_layout(ship_type);
            let mut blocks: Vec<(IVec2, Option<ModuleType>, Rotation)> = layout
                .hull_cells
                .iter()
                .map(|h| (h.grid_pos, None, Rotation::North))
                .collect();
            blocks.extend(
                layout.modules.iter().map(|m| (m.grid_pos, Some(m.module_type), m.rotation)),
            );

            for (cell, mt, blocker) in crate::building::entombed_launchers(&blocks) {
                failures.push(format!(
                    "{ship_type:?}: {mt:?} at ({}, {}) is blocked at ({}, {})",
                    cell.x, cell.y, blocker.x, blocker.y
                ));
            }
        }

        assert!(failures.is_empty(), "buried launchers:\n{}", failures.join("\n"));
    }

    use super::*;
    use std::collections::HashSet;

    const ALL: [AiShipType; 12] = [
        AiShipType::StellarPreserve, AiShipType::SynthesisCollective, AiShipType::BrokenChoir,
        AiShipType::CorpseStars, AiShipType::TheSilence, AiShipType::TerranHegemony,
        AiShipType::GildedThrone, AiShipType::RecursiveKingdom, AiShipType::EternalHegemony,
        AiShipType::Shepherd, AiShipType::MerchantPrinces, AiShipType::FreeHaulersCompact,
    ];

    /// A slug must name a file that is actually there, and no design file may
    /// be left unclaimed.
    ///
    /// `faction_design()` in spawner.rs loads `designs/factions/<slug>.json`
    /// and, when the file is missing, silently rebuilds from the fallback in
    /// this file and writes that out instead. Nothing is logged. So renaming a
    /// faction without moving its JSON swaps a hand-armoured hull for the old
    /// one and the only symptom is that the ship looks subtly wrong in a fight
    /// nobody is watching closely.
    ///
    /// The orphan half of the check is the half that catches a half-done
    /// rename: the stale file stays on disk claimed by nobody.
    #[test]
    fn design_slugs_and_files_agree() {
        use std::collections::HashSet;
        use std::path::Path;

        let dir = Path::new("designs/factions");
        assert!(dir.is_dir(), "designs/factions missing — run from the crate root");

        let claimed: HashSet<String> = ALL.iter().map(|&f| design_slug(f).to_string()).collect();
        assert_eq!(claimed.len(), ALL.len(), "two factions share a design slug");

        let on_disk: HashSet<String> = std::fs::read_dir(dir)
            .expect("read designs/factions")
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str()?.strip_suffix(".json").map(str::to_string))
            .collect();

        let orphans: Vec<_> = on_disk.difference(&claimed).cloned().collect();
        assert!(
            orphans.is_empty(),
            "design files claimed by no faction slug (half-finished rename?): {:?}",
            orphans
        );

        let absent: Vec<_> = claimed.difference(&on_disk).cloned().collect();
        assert!(
            absent.is_empty(),
            "faction slugs with no design file — these would be rebuilt from the \
             fallback and written out at runtime, untracked: {:?}",
            absent
        );

        // Each file must also agree with its own filename internally, since
        // the name field is what a design round-trips under.
        for slug in &claimed {
            let path = dir.join(format!("{}.json", slug));
            let bp = crate::building::blueprint::load_design_file(&path)
                .unwrap_or_else(|| panic!("{} exists but will not parse", path.display()));
            assert_eq!(
                &bp.name, slug,
                "{} carries name {:?} — the loader would re-export it under the wrong stem",
                path.display(), bp.name
            );
        }
    }

    fn hull_cells(layout: &AiShipLayout) -> HashSet<IVec2> {
        layout.hull_cells.iter().map(|c| c.grid_pos).collect()
    }

    fn is_plate(mt: ModuleType) -> bool {
        matches!(mt, ModuleType::AngledArmorPlate | ModuleType::AngledHullPlate)
    }

    /// Angled plating must sit OUTBOARD. Hull wins its own cell in ShipGrid, so
    /// a plate on a hull cell is armour that can never be hit.
    #[test]
    fn plates_never_share_a_cell_with_hull() {
        for ship in ALL {
            let layout = get_layout(ship);
            let hull = hull_cells(&layout);
            for m in layout.modules.iter().filter(|m| is_plate(m.module_type)) {
                assert!(!hull.contains(&m.grid_pos),
                    "{ship:?}: plate at {:?} is buried in hull and would never be hit", m.grid_pos);
            }
        }
    }

    /// ...and must still touch the ship, or it's floating in space.
    #[test]
    fn plates_are_attached_to_the_hull() {
        for ship in ALL {
            let layout = get_layout(ship);
            let hull = hull_cells(&layout);
            for m in layout.modules.iter().filter(|m| is_plate(m.module_type)) {
                let touching = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y]
                    .iter().any(|d| hull.contains(&(m.grid_pos + *d)));
                assert!(touching, "{ship:?}: plate at {:?} is not attached to anything", m.grid_pos);
            }
        }
    }

    /// Every non-plate module sits on hull. Redesigning a silhouette is the
    /// easy way to strand a reactor in open space.
    #[test]
    fn every_module_sits_on_hull() {
        for ship in ALL {
            let layout = get_layout(ship);
            let hull = hull_cells(&layout);
            for m in layout.modules.iter().filter(|m| !is_plate(m.module_type)) {
                assert!(hull.contains(&m.grid_pos),
                    "{ship:?}: {:?} at {:?} is off the hull", m.module_type, m.grid_pos);
            }
        }
    }

    /// Two things must not occupy the same cell.
    #[test]
    fn no_two_modules_share_a_cell() {
        for ship in ALL {
            let layout = get_layout(ship);
            let mut seen = HashSet::new();
            for m in &layout.modules {
                assert!(seen.insert(m.grid_pos),
                    "{ship:?}: two modules both at {:?}", m.grid_pos);
            }
        }
    }

    /// Every faction must exist as an exported design on disk.
    ///
    /// `designs/factions/<slug>.json` WINS over the layout in this file, and
    /// a layout self-exports there the first time one of its ships spawns.
    /// That means a faction nobody has met yet has no file, and the only copy
    /// of its ship is the Rust fallback -- so what a fresh clone fights is
    /// whatever this file happens to say, not what was tuned in the hull yard.
    ///
    /// Void Titan was missing exactly this way: it spawns only at the extreme
    /// edge of explored space, so no run had ever produced one.
    ///
    /// Deliberately NOT compared against `get_layout`. The JSON is allowed to
    /// diverge -- editing it is the documented workflow and the whole reason
    /// it wins -- so this asserts only what stays true however it is edited.
    /// If this fails, run:
    ///
    ///     cargo test export_missing_faction_designs -- --ignored
    #[test]
    fn every_faction_has_an_exported_design() {
        let mut missing: Vec<String> = Vec::new();

        for ship in ALL {
            let slug = design_slug(ship);
            let path = format!("designs/factions/{slug}.json");
            match crate::building::blueprint::load_design_file(&path) {
                None => missing.push(format!("{slug}: absent, or failed to parse")),
                Some(design) => {
                    if design.name != slug {
                        missing.push(format!(
                            "{slug}: file names itself {:?}, which is the name the game shows",
                            design.name
                        ));
                    }
                    if design.hull_cells.is_empty() {
                        missing.push(format!("{slug}: no hull cells -- ship has no body"));
                    }
                    if design.modules.is_empty() {
                        missing.push(format!("{slug}: no modules -- ship cannot act"));
                    }
                }
            }
        }

        assert!(
            missing.is_empty(),
            "faction designs not on disk:\n{}\n\nregenerate with: \
             cargo test export_missing_faction_designs -- --ignored",
            missing.join("\n")
        );
    }

    /// Writes an exported design for any faction that has none, through the
    /// same `to_design` path the game self-exports with.
    ///
    /// Ignored by default because it writes into the repository. It exists so
    /// that filling the gap is one command, rather than hunting for a boss at
    /// the edge of the galaxy to make the game export it for you.
    #[test]
    #[ignore = "writes into designs/factions; run deliberately"]
    fn export_missing_faction_designs() {
        let mut written: Vec<String> = Vec::new();

        for ship in ALL {
            let slug = design_slug(ship);
            let path = format!("designs/factions/{slug}.json");
            if crate::building::blueprint::load_design_file(&path).is_some() {
                continue;
            }
            let design = get_layout(ship).to_design(slug);
            crate::building::blueprint::write_design_file(&path, &design)
                .unwrap_or_else(|e| panic!("could not write {path}: {e}"));
            written.push(format!(
                "{slug} ({} hull cells, {} modules)",
                design.hull_cells.len(),
                design.modules.len()
            ));
        }

        if written.is_empty() {
            println!("every faction already has an exported design");
        } else {
            println!("exported:\n  {}", written.join("\n  "));
        }
    }
}

