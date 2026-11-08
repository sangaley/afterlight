//! The gun house: a roof of real blocks over a turret's gun parts.
//!
//! It used to be one picture stretched over the deck, and read as a sticker
//! on the ship. Now it is armour: Armor Plates and Angled Armor Plates laid
//! on the turret's roof, over the breeches and loaders, with 45-degree
//! angle pieces on its corners (two-cell chamfers on a big turret), and
//! fittings among the plates that each do something for that turret:
//!
//! - Rangefinder: its guns' spread tightens 15% each (two count).
//! - Roof Vent: cools every gun under it like a Breech Cooler (two count).
//! - Periscope: a gun sight -- its guns lead like a Targeting Computer's --
//!   and the turret turns 15% quicker each (two count).
//! - Armoured Hatch: crew get at the parts under the roof and rebuild them
//!   twice as fast.
//!
//! A round meets the roof before the parts under it (a roof is a layer of
//! its own in the hit walk, combat::new_projectiles). A turret with guns and
//! no roof gets the default one laid (`lay_default_roofs`); in build mode
//! PgUp from the turret deck goes up onto the roof to change it. A roof block
//! is an ordinary module on the turret: what makes it roof is its type
//! (`is_roof_block`), so saves, designs and crew rebuilds carry it with no
//! extra state.

use bevy::prelude::*;

use super::turret_deck::{self, OnTurret, PendingTurretMount, TurretBody, OUT_FRONT};
use crate::components::{DestroyedModule, Module, ModuleType, Rotation};

/// What stands on a turret's roof rather than its deck.
pub fn is_roof_block(t: ModuleType) -> bool {
    matches!(
        t,
        ModuleType::ArmorPlate
            | ModuleType::AngledArmorPlate
            | ModuleType::BevelledArmorPlate
            | ModuleType::SlopeArmorPlateLow
            | ModuleType::SlopeArmorPlateHigh
            | ModuleType::SlopeArmorPlateLowMirror
            | ModuleType::SlopeArmorPlateHighMirror
            | ModuleType::HalfArmorPlate
            | ModuleType::Rangefinder
            | ModuleType::RoofVent
            | ModuleType::Periscope
            | ModuleType::ArmouredHatch
    )
}

/// Above the gun parts (0.2-0.4 in the turret's frame) and the barrels'
/// mantlets come out over it (turret_deck::GUN_HOUSE_Z + 0.1).
pub const ROOF_Z: f32 = 0.5;

/// The art for a roof plate or angle piece: the deck's armour, a shade
/// lighter -- the roof is the highest thing on the ship.
pub fn roof_sprite(t: ModuleType) -> Option<&'static str> {
    match t {
        ModuleType::ArmorPlate => Some("sprites/gun_parts/roof_plate.png"),
        ModuleType::AngledArmorPlate => Some("sprites/gun_parts/roof_wedge.png"),
        ModuleType::BevelledArmorPlate => Some("sprites/gun_parts/roof_bevelled_plate.png"),
        ModuleType::SlopeArmorPlateLow => Some("sprites/gun_parts/roof_slope_low.png"),
        ModuleType::SlopeArmorPlateHigh => Some("sprites/gun_parts/roof_slope_high.png"),
        ModuleType::SlopeArmorPlateLowMirror => Some("sprites/gun_parts/roof_slope_low_mirror.png"),
        ModuleType::SlopeArmorPlateHighMirror => Some("sprites/gun_parts/roof_slope_high_mirror.png"),
        ModuleType::HalfArmorPlate => Some("sprites/gun_parts/roof_half_plate.png"),
        _ => None,
    }
}

/// The angle piece whose sloped face looks out toward `out` (a diagonal):
/// an Angled Armor Plate at North faces north-east, and each turn carries
/// it round a corner (building::module_facing).
fn wedge_facing(out: IVec2) -> Rotation {
    match (out.x.signum(), out.y.signum()) {
        (1, 1) => Rotation::North,
        (1, -1) => Rotation::East,
        (-1, 1) => Rotation::West,
        _ => Rotation::South,
    }
}

/// The roof a turret gets unless someone has built it another: plates over
/// the cells its gun parts (all but what comes out the front) span, angle
/// pieces cutting the corners, and the fittings.
pub fn default_roof(parts: &[(ModuleType, IVec2)], deck_size: i32) -> Vec<(ModuleType, IVec2, Rotation)> {
    use std::collections::HashMap;
    let housed: Vec<IVec2> = parts
        .iter()
        .filter(|(t, _)| !OUT_FRONT.contains(t) && !is_roof_block(*t))
        .map(|(_, c)| *c)
        .collect();
    if housed.is_empty() {
        return Vec::new();
    }
    let (x0, x1) = (housed.iter().map(|c| c.x).min().unwrap(), housed.iter().map(|c| c.x).max().unwrap());
    let (y0, y1) = (housed.iter().map(|c| c.y).min().unwrap(), housed.iter().map(|c| c.y).max().unwrap());
    let lanes: Vec<i32> = parts.iter().filter(|(t, _)| *t == ModuleType::FiringPiece).map(|(_, c)| c.y).collect();
    // The front cells the guns come out through stay plain plate.
    let port = |c: IVec2| c.x == x1 && lanes.contains(&c.y);

    let mut roof: HashMap<IVec2, (ModuleType, Rotation)> = HashMap::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            let c = IVec2::new(x, y);
            if turret_deck::deck_cells(deck_size).contains(&c) {
                roof.insert(c, (ModuleType::ArmorPlate, Rotation::North));
            }
        }
    }
    // Shape the edges. The back corners are cut at 45 degrees: one angle
    // piece on a small roof; on a big one the corner goes and the two cells
    // beside it take a longer edge between them. At the front, beside the
    // guns, a big roof slopes down along each side over two cells, and a
    // smaller one has its corners bevelled -- most of the steel stays where
    // the guns are.
    let big = x1 - x0 >= 4 && y1 - y0 >= 4;
    let free = |roof: &HashMap<IVec2, (ModuleType, Rotation)>, c: IVec2| roof.contains_key(&c) && !port(c);
    for (corner, out) in [(IVec2::new(x0, y1), IVec2::new(-1, 1)), (IVec2::new(x0, y0), IVec2::new(-1, -1))] {
        let sides = [corner - IVec2::new(out.x, 0), corner - IVec2::new(0, out.y)];
        let wedge = (ModuleType::AngledArmorPlate, wedge_facing(out));
        if big && sides.iter().all(|s| free(&roof, *s)) {
            roof.remove(&corner);
            for s in sides {
                roof.insert(s, wedge);
            }
        } else if free(&roof, corner) {
            roof.insert(corner, wedge);
        }
    }
    for (y, out) in [(y1, IVec2::new(1, 1)), (y0, IVec2::new(1, -1))] {
        let (back, front) = (IVec2::new(x1 - 1, y), IVec2::new(x1, y));
        if big && free(&roof, back) && free(&roof, front) {
            // Falling toward the bow: the mirrored pair along the top edge,
            // the plain pair turned half round along the bottom.
            let (high, low, turn) = if out.y > 0 {
                (ModuleType::SlopeArmorPlateHighMirror, ModuleType::SlopeArmorPlateLowMirror, Rotation::North)
            } else {
                (ModuleType::SlopeArmorPlateHigh, ModuleType::SlopeArmorPlateLow, Rotation::South)
            };
            roof.insert(back, (high, turn));
            roof.insert(front, (low, turn));
        } else if free(&roof, front) {
            roof.insert(front, (ModuleType::BevelledArmorPlate, wedge_facing(out)));
        }
    }
    // Fittings, each on the first free plate of its candidates.
    let mid = (y0 + y1).div_euclid(2);
    let mut fit = |t: ModuleType, candidates: &[IVec2]| {
        if let Some(c) = candidates
            .iter()
            .find(|c| !port(**c) && roof.get(*c).is_some_and(|(k, _)| *k == ModuleType::ArmorPlate))
        {
            // Fitting art is drawn facing the bow already.
            roof.insert(*c, (t, Rotation::North));
        }
    };
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    fit(ModuleType::ArmouredHatch, &[IVec2::new(x0, mid), IVec2::new(x0, mid + 1), IVec2::new(x0 + 1, mid)]);
    if w >= 3 {
        fit(ModuleType::RoofVent, &[IVec2::new(x0 + 1, mid), IVec2::new(x0 + 1, mid + 1)]);
    }
    if h >= 3 {
        for (edge, inward) in [(y1, -1), (y0, 1)] {
            fit(ModuleType::Periscope, &[IVec2::new(x1 - 1, edge), IVec2::new(x1 - 1, edge + inward), IVec2::new(x1 - 2, edge)]);
        }
    }
    if w >= 4 && h >= 3 {
        for (edge, inward) in [(y1, -1), (y0, 1)] {
            fit(ModuleType::Rangefinder, &[IVec2::new(x0 + 1, edge), IVec2::new(x0 + 2, edge), IVec2::new(x0 + 1, edge + inward)]);
        }
    }
    let mut out: Vec<(ModuleType, IVec2, Rotation)> = roof.into_iter().map(|(c, (t, r))| (t, c, r)).collect();
    out.sort_by_key(|(_, c, _)| (c.x, c.y));
    out
}

/// A turret whose roof has been seen to: laid by default, or found already
/// built (from a design or a save). Stripping it later is the builder's
/// business and doesn't bring it back.
#[derive(Component)]
pub struct RoofLaid;

/// Give each turret with guns and no roof the default one. Waits while any
/// of the turret's blocks are still being mounted (a design or a save may be
/// bringing its own roof).
#[allow(clippy::type_complexity)]
pub fn lay_default_roofs(
    mut commands: Commands,
    bodies: Query<(Entity, &TurretBody, &ChildOf, Option<&Children>), Without<RoofLaid>>,
    parts: Query<&Module, Without<DestroyedModule>>,
    pending: Query<(&PendingTurretMount, &ChildOf)>,
    ai: Query<(), With<crate::ai_ship::components::AiShip>>,
    asset_server: Res<AssetServer>,
    registry: Res<crate::building::registry::ModuleRegistry>,
) {
    for (body, turret, parent, children) in bodies.iter() {
        let ship = parent.parent();
        let on: Vec<(ModuleType, IVec2)> = children
            .into_iter()
            .flatten()
            .filter_map(|c| parts.get(*c).ok())
            .map(|m| (m.module_type, m.grid_position))
            .collect();
        if !on.iter().any(|(t, _)| *t == ModuleType::FiringPiece) {
            continue;
        }
        if pending.iter().any(|(p, c)| c.parent() == ship && p.0 == turret.ring_cell) {
            continue;
        }
        commands.entity(body).try_insert(RoofLaid);
        if on.iter().any(|(t, _)| is_roof_block(*t)) {
            continue;
        }
        let roof = default_roof(&on, turret.size);
        if super::gun_assembly::trace() {
            info!("ROOF laid on turret {:?} (ring {:?}, {}): {} blocks", body, turret.ring_cell, if ai.contains(ship) { "AI" } else { "player" }, roof.len());
        }
        for (t, cell, rotation) in roof {
            let block = crate::ship::spawn_module(&mut commands, &asset_server, ship, t, cell, rotation, &registry);
            commands.entity(block).insert(PendingTurretMount(turret.ring_cell));
            if ai.contains(ship) {
                commands.entity(block).insert(crate::ai_ship::components::OwnedByAiShip { root: ship });
            }
        }
    }
}

/// What a turret's roof gives its guns: the live fittings on it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoofFittings {
    pub rangefinders: u32,
    pub vents: u32,
    pub periscopes: u32,
    pub hatches: u32,
}

/// At most this many of each fitting counts.
pub const FITTINGS_COUNTED: u32 = 2;

impl RoofFittings {
    /// Spread multiplier for the turret's guns.
    pub fn spread_mult(&self) -> f32 {
        0.85f32.powi(self.rangefinders.min(FITTINGS_COUNTED) as i32)
    }
    /// Extra Breech Coolers each gun counts.
    pub fn coolers(&self) -> u32 {
        self.vents.min(FITTINGS_COUNTED)
    }
    /// Traverse multiplier.
    pub fn traverse_mult(&self) -> f32 {
        1.0 + 0.15 * self.periscopes.min(FITTINGS_COUNTED) as f32
    }
    /// A sight: the guns lead like a Targeting Computer's.
    pub fn sighted(&self) -> bool {
        self.periscopes > 0
    }
    /// How much faster crew rebuild the parts under the roof.
    pub fn repair_mult(&self) -> f32 {
        if self.hatches > 0 { 2.0 } else { 1.0 }
    }
}

/// Count each turret's live fittings, writing only when they change (so
/// guns reassemble only then).
pub fn count_roof_fittings(
    mut commands: Commands,
    mut bodies: Query<(Entity, Option<&Children>, Option<&mut RoofFittings>), With<TurretBody>>,
    parts: Query<&Module, (Without<DestroyedModule>, With<OnTurret>)>,
) {
    for (body, children, fittings) in bodies.iter_mut() {
        let mut now = RoofFittings::default();
        for m in children.into_iter().flatten().filter_map(|c| parts.get(*c).ok()) {
            if m.health <= 0.0 {
                continue;
            }
            match m.module_type {
                ModuleType::Rangefinder => now.rangefinders += 1,
                ModuleType::RoofVent => now.vents += 1,
                ModuleType::Periscope => now.periscopes += 1,
                ModuleType::ArmouredHatch => now.hatches += 1,
                _ => {}
            }
        }
        match fittings {
            Some(mut f) => {
                if *f != now {
                    *f = now;
                }
            }
            None => {
                commands.entity(body).try_insert(now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn twin_225() -> Vec<(ModuleType, IVec2)> {
        use ModuleType::*;
        let mut p = Vec::new();
        for lane in [1, -1] {
            let s = if lane > 0 { 1 } else { -1 };
            p.push((FiringPiece, IVec2::new(0, lane)));
            p.push((Mantlet, IVec2::new(1, lane)));
            for x in 2..6 {
                p.push((GunBarrel, IVec2::new(x, lane)));
            }
            for x in -1..=1 {
                p.push((GaugeIncreaser, IVec2::new(x, lane + s)));
            }
            p.push((Loader, IVec2::new(-1, lane)));
            p.push((ClipRack, IVec2::new(-2, lane)));
            p.push((BreechCooler, IVec2::new(-3, lane)));
        }
        p
    }

    fn count(roof: &[(ModuleType, IVec2, Rotation)], t: ModuleType) -> usize {
        roof.iter().filter(|(k, _, _)| *k == t).count()
    }

    #[test]
    fn a_big_turret_gets_long_chamfers_and_every_fitting() {
        let roof = default_roof(&twin_225(), 5);
        // Rear corners cut two cells long (two pieces each); the front sides
        // slope down to the bow over two cells.
        assert_eq!(count(&roof, ModuleType::AngledArmorPlate), 4, "{roof:?}");
        assert_eq!(count(&roof, ModuleType::SlopeArmorPlateHighMirror) + count(&roof, ModuleType::SlopeArmorPlateLowMirror), 2);
        assert_eq!(count(&roof, ModuleType::SlopeArmorPlateHigh) + count(&roof, ModuleType::SlopeArmorPlateLow), 2);
        assert_eq!(count(&roof, ModuleType::ArmouredHatch), 1);
        assert_eq!(count(&roof, ModuleType::RoofVent), 1);
        assert_eq!(count(&roof, ModuleType::Periscope), 2);
        assert_eq!(count(&roof, ModuleType::Rangefinder), 2);
        // Where a gun comes out is plain plate, and every block is on the deck.
        assert!(roof.iter().any(|(t, c, _)| *t == ModuleType::ArmorPlate && *c == IVec2::new(1, 1)));
        let deck = turret_deck::deck_cells(5);
        assert!(roof.iter().all(|(_, c, _)| deck.contains(c)));
        // One block a cell.
        let mut cells: Vec<IVec2> = roof.iter().map(|(_, c, _)| *c).collect();
        cells.sort_by_key(|c| (c.x, c.y));
        cells.dedup();
        assert_eq!(cells.len(), roof.len());
    }

    #[test]
    fn corner_pieces_face_out_of_their_corner() {
        let roof = default_roof(&twin_225(), 5);
        for (t, c, r) in &roof {
            if *t != ModuleType::AngledArmorPlate {
                continue;
            }
            let want = match (c.x >= 0, c.y > 0) {
                (true, true) => Rotation::North,
                (true, false) => Rotation::East,
                (false, true) => Rotation::West,
                (false, false) => Rotation::South,
            };
            assert_eq!(*r, want, "{c:?}");
        }
    }

    #[test]
    fn a_small_ciws_still_gets_a_roof_and_a_hatch() {
        use ModuleType::*;
        let ciws = [
            (FiringPiece, IVec2::ZERO),
            (GaugeReducer, IVec2::new(0, 1)),
            (GaugeReducer, IVec2::new(0, -1)),
            (GunBarrel, IVec2::new(1, 0)),
            (Loader, IVec2::new(-1, 0)),
            (ClipRack, IVec2::new(-1, 1)),
            (BreechCooler, IVec2::new(-1, -1)),
            (BreechCooler, IVec2::new(1, 1)),
        ];
        let roof = default_roof(&ciws, 1);
        assert!(!roof.is_empty());
        assert_eq!(count(&roof, ArmouredHatch), 1);
        assert_eq!(count(&roof, AngledArmorPlate), 2, "back corners");
        assert_eq!(count(&roof, BevelledArmorPlate), 2, "front corners");
    }

    #[test]
    fn fittings_count_up_to_two() {
        let f = RoofFittings { rangefinders: 5, vents: 3, periscopes: 1, hatches: 0 };
        assert!((f.spread_mult() - 0.85 * 0.85).abs() < 1e-6);
        assert_eq!(f.coolers(), 2);
        assert!((f.traverse_mult() - 1.15).abs() < 1e-6);
        assert!(f.sighted());
        assert_eq!(f.repair_mult(), 1.0);
    }
}
