//! How each owner builds its stations: the metal's tint and the colour of
//! its windows (from the faction's livery), the shape of its hub, how
//! orderly it builds (a rich, proud faction straight and even, a poor or
//! broken one crooked), what sections it bolts on, and who may moor there --
//! its own ships and those of any faction not hostile to it. A station with
//! no owner (Haven, a free port) takes everyone.

use bevy::prelude::*;

use super::sections::{HubKind, Kind};
use crate::ai_ship::components::{factions_hostile, AiShipType};

/// A station's paint: the metal's tint and its window lights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub tint: [f32; 3],
    pub windows: [[f32; 3]; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// Picks the theme the pictures are painted in (0: no owner).
    pub theme: u8,
    pub order: f32,
    pub hub: HubKind,
    pub arms: usize,
    pub parts: &'static [Kind],
}

const WARM: [f32; 3] = [1.0, 0.76, 0.42];
const WHITE: [f32; 3] = [0.9, 0.94, 1.0];
const CYAN: [f32; 3] = [0.45, 0.88, 1.0];

/// Every owner in a fixed order: its index + 1 is its theme number.
pub const OWNERS: [AiShipType; 12] = [
    AiShipType::TerranHegemony,
    AiShipType::EternalHegemony,
    AiShipType::GildedThrone,
    AiShipType::BrokenChoir,
    AiShipType::SynthesisCollective,
    AiShipType::CorpseStars,
    AiShipType::StellarPreserve,
    AiShipType::TheSilence,
    AiShipType::Shepherd,
    AiShipType::MerchantPrinces,
    AiShipType::FreeHaulersCompact,
    AiShipType::RecursiveKingdom,
];

/// The paint for a theme number.
pub fn theme(id: u8) -> Theme {
    use AiShipType::*;
    let Some(owner) = id.checked_sub(1).and_then(|i| OWNERS.get(i as usize)) else {
        return Theme { tint: [1.0, 1.0, 1.0], windows: [WARM, WHITE, CYAN] };
    };
    let base = crate::ai_ship::components::faction_map_color(*owner).to_srgba();
    // The metal leans a little toward the faction's colour; no more.
    let tint = [0.78 + 0.3 * base.red, 0.78 + 0.3 * base.green, 0.78 + 0.3 * base.blue];
    let windows = match owner {
        TerranHegemony => [WHITE, [0.6, 0.8, 1.0], [1.0, 0.85, 0.6]],
        EternalHegemony => [[1.0, 0.8, 0.4], [1.0, 0.5, 0.35], [1.0, 0.9, 0.7]],
        GildedThrone => [[1.0, 0.82, 0.4], [1.0, 0.7, 0.3], [1.0, 0.95, 0.8]],
        BrokenChoir => [[0.75, 0.95, 0.8], [0.6, 0.7, 1.0], [0.9, 0.9, 0.9]],
        SynthesisCollective => [[0.35, 0.95, 0.85], [0.8, 0.5, 1.0], [0.5, 1.0, 0.6]],
        CorpseStars => [[0.95, 0.85, 0.65], [1.0, 0.35, 0.3], [0.75, 0.6, 1.0]],
        StellarPreserve => [[0.6, 1.0, 0.7], [0.95, 0.97, 0.9], [0.4, 0.9, 1.0]],
        TheSilence => [[0.7, 0.8, 1.0], [0.9, 0.92, 0.95], [0.7, 0.8, 1.0]],
        Shepherd => [[1.0, 0.95, 0.8], [0.95, 0.85, 0.5], [0.9, 0.95, 1.0]],
        MerchantPrinces => [WARM, [1.0, 0.45, 0.8], [0.6, 1.0, 0.6]],
        FreeHaulersCompact => [[1.0, 0.7, 0.3], [1.0, 0.9, 0.6], WHITE],
        RecursiveKingdom => [[1.0, 0.55, 0.25], [1.0, 0.3, 0.2], WARM],
    };
    Theme { tint, windows }
}

/// How `owner` (None: nobody's) builds.
pub fn look(owner: Option<AiShipType>) -> Look {
    use AiShipType::*;
    use Kind::*;
    let Some(owner) = owner else {
        return Look { theme: 0, order: 0.55, hub: HubKind::Octagon, arms: 9, parts: &[Habitat, Solar, Radiator, Tanks, Cargo, Antenna, Greenhouse, Crane] };
    };
    let theme = OWNERS.iter().position(|o| *o == owner).map_or(0, |i| i as u8 + 1);
    let (order, hub, arms, parts): (f32, HubKind, usize, &'static [Kind]) = match owner {
        TerranHegemony => (0.95, HubKind::Octagon, 6, &[Habitat, Antenna, Radiator, Solar]),
        EternalHegemony => (1.0, HubKind::Hex, 7, &[Habitat, Antenna, Tanks]),
        GildedThrone => (1.0, HubKind::Wheel, 8, &[Habitat, Greenhouse, Cargo, Habitat]),
        Shepherd => (0.95, HubKind::Wheel, 8, &[Habitat, Habitat, Solar, Antenna]),
        StellarPreserve => (0.85, HubKind::Wheel, 7, &[Greenhouse, Greenhouse, Solar, Habitat]),
        TheSilence => (0.9, HubKind::Spindle, 4, &[Antenna, Antenna, Radiator]),
        SynthesisCollective => (0.5, HubKind::Hex, 7, &[Greenhouse, Tanks, Habitat]),
        MerchantPrinces => (0.4, HubKind::Octagon, 8, &[Cargo, Cargo, Crane, Habitat, Tanks]),
        FreeHaulersCompact => (0.2, HubKind::Spindle, 7, &[Tanks, Cargo, Crane, Radiator]),
        CorpseStars => (0.25, HubKind::Spindle, 5, &[Tanks, Habitat, Antenna, Radiator]),
        BrokenChoir => (0.1, HubKind::Hex, 6, &[Antenna, Antenna, Radiator, Habitat]),
        RecursiveKingdom => (0.15, HubKind::Octagon, 6, &[Cargo, Tanks, Crane, Radiator]),
    };
    Look { theme, order, hub, arms, parts }
}

/// Whether ships of `ship` may moor at a station of `owner`: always at an
/// ownerless one; otherwise only the owner's own and those not hostile to it.
pub fn may_moor(owner: Option<AiShipType>, ship: AiShipType) -> bool {
    owner.is_none_or(|o| o == ship || !factions_hostile(o, ship))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enemies_never_moor_at_each_others_stations() {
        for a in OWNERS {
            for b in OWNERS {
                if factions_hostile(a, b) {
                    assert!(!may_moor(Some(a), b), "{b:?} at {a:?}'s station");
                    assert!(!may_moor(Some(b), a), "{a:?} at {b:?}'s station");
                }
            }
            assert!(may_moor(Some(a), a), "{a:?} not welcome at home");
            assert!(may_moor(None, a), "Haven turns {a:?} away");
        }
    }

    #[test]
    fn the_rich_build_straighter_than_the_poor() {
        assert!(look(Some(AiShipType::GildedThrone)).order > look(Some(AiShipType::FreeHaulersCompact)).order + 0.5);
        assert!(look(Some(AiShipType::TerranHegemony)).order > look(Some(AiShipType::BrokenChoir)).order + 0.5);
    }

    #[test]
    fn every_owner_has_its_own_paint() {
        let themes: Vec<Theme> = OWNERS.iter().map(|o| theme(look(Some(*o)).theme)).collect();
        for i in 0..themes.len() {
            for j in i + 1..themes.len() {
                assert_ne!(themes[i], themes[j], "{:?} and {:?} look the same", OWNERS[i], OWNERS[j]);
            }
        }
    }
}
