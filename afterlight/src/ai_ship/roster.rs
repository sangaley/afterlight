//! Which hulls each faction flies.
//!
//! A faction used to be one design: every Terran ship was the same Terran
//! ship. Now a faction has a roster of classes, from scouts up to capitals,
//! plus the civilians and oddities its lore calls for. Each class is a
//! hand-drawn design at `designs/factions/<slug>/<key>.json`, approved on the
//! fleet concepts page (tools/fleet-concepts).
//!
//! A faction with an empty roster flies its one original design,
//! `designs/factions/<slug>.json`, exactly as before. So does any class whose
//! file is missing.

use rand::Rng;

use super::components::AiShipType;

/// What a hull is for. Decides how often it turns up and how many a system
/// may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassRole {
    Scout,
    Line,
    Capital,
    Boss,
    Civilian,
    Experimental,
}

impl ClassRole {
    /// The most of this role one system's population may hold. Capitals and
    /// bosses are meant to be events, and they are also the biggest block
    /// counts in the game.
    pub fn per_system_cap(self) -> usize {
        match self {
            ClassRole::Boss => 1,
            ClassRole::Capital => 3,
            ClassRole::Experimental => 4,
            _ => usize::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShipClass {
    /// File stem under designs/factions/<slug>/.
    pub key: &'static str,
    pub name: &'static str,
    pub role: ClassRole,
    /// Relative odds of a new ship being this class.
    pub weight: u32,
}

pub(crate) const fn class(key: &'static str, name: &'static str, role: ClassRole, weight: u32) -> ShipClass {
    ShipClass { key, name, role, weight }
}

// The rosters themselves: the reworked fleet, approved on the review site and
// put in the game 2026-11-23 (tools/fleet-rework/integrate.py writes them).
use super::roster_classes::*;

/// Every class a faction flies. Empty means "the one original design".
pub fn roster(faction: AiShipType) -> &'static [ShipClass] {
    use AiShipType::*;
    match faction {
        TerranHegemony => TERRAN_HEGEMONY,
        EternalHegemony => ETERNAL_HEGEMONY,
        GildedThrone => GILDED_THRONE,
        BrokenChoir => BROKEN_CHOIR,
        SynthesisCollective => SYNTHESIS_COLLECTIVE,
        CorpseStars => CORPSE_STARS,
        StellarPreserve => STELLAR_PRESERVE,
        TheSilence => THE_SILENCE,
        Shepherd => SHEPHERD,
        RecursiveKingdom => RECURSIVE_KINGDOM,
        MerchantPrinces => MERCHANT_PRINCES,
        FreeHaulersCompact => FREE_HAULERS_COMPACT,
    }
}

/// How often the faction's original design still turns up next to its new
/// roster: only while some of its classes are out for rework, so that slot
/// isn't simply empty.
pub fn original_weight(_faction: AiShipType) -> u32 {
    // Every faction's roster is whole again: the original stand-ins retire.
    0
}

/// The roster's own `&'static str` for a key read back from a save: every
/// class and every station design. None for a key no longer on any roster
/// (the ship then flies its faction's original design).
pub fn intern(key: &str) -> Option<&'static str> {
    const ALL: [AiShipType; 12] = [
        AiShipType::StellarPreserve, AiShipType::SynthesisCollective, AiShipType::BrokenChoir,
        AiShipType::CorpseStars, AiShipType::TheSilence, AiShipType::TerranHegemony,
        AiShipType::GildedThrone, AiShipType::RecursiveKingdom, AiShipType::EternalHegemony,
        AiShipType::Shepherd, AiShipType::MerchantPrinces, AiShipType::FreeHaulersCompact,
    ];
    ALL.iter()
        .flat_map(|&f| roster(f).iter().map(|c| c.key))
        .find(|k| *k == key)
}

pub fn find(faction: AiShipType, key: &str) -> Option<&'static ShipClass> {
    roster(faction).iter().find(|c| c.key == key)
}

/// Pick a class for a new ship, or None for the faction's original design.
/// `allowed` lets the caller refuse a class (e.g. a role already at its cap).
pub fn roll<R: Rng>(
    faction: AiShipType,
    rng: &mut R,
    allowed: impl Fn(&ShipClass) -> bool,
) -> Option<&'static ShipClass> {
    let classes: Vec<&ShipClass> = roster(faction).iter().filter(|c| allowed(c)).collect();
    let original = if roster(faction).is_empty() { 1 } else { original_weight(faction) };
    let total: u32 = classes.iter().map(|c| c.weight).sum::<u32>() + original;
    if total == 0 {
        return None;
    }
    let mut pick = rng.gen_range(0..total);
    for c in classes {
        if pick < c.weight {
            return Some(c);
        }
        pick -= c.weight;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::blueprint;

    const ALL: [AiShipType; 10] = [
        AiShipType::StellarPreserve,
        AiShipType::SynthesisCollective,
        AiShipType::BrokenChoir,
        AiShipType::CorpseStars,
        AiShipType::TheSilence,
        AiShipType::TerranHegemony,
        AiShipType::GildedThrone,
        AiShipType::RecursiveKingdom,
        AiShipType::EternalHegemony,
        AiShipType::Shepherd,
    ];

    /// Every class on a roster has its design on disk, and it loads.
    #[test]
    fn every_class_has_a_design() {
        for faction in ALL {
            let slug = super::super::layouts::design_slug(faction);
            for c in roster(faction) {
                let path = format!("designs/factions/{}/{}.json", slug, c.key);
                let bp = blueprint::load_design_file(&path)
                    .unwrap_or_else(|| panic!("{path} is missing or doesn't parse"));
                assert!(!bp.hull_cells.is_empty() && !bp.modules.is_empty(), "{path} is empty");
            }
        }
    }

    /// Keys are unique within a faction, and every roster can actually roll
    /// each of its classes.
    #[test]
    fn rosters_are_well_formed() {
        for faction in ALL {
            let r = roster(faction);
            for (i, a) in r.iter().enumerate() {
                assert!(a.weight > 0, "{faction:?}/{} can never be rolled", a.key);
                assert!(r[i + 1..].iter().all(|b| b.key != a.key), "{faction:?} lists {} twice", a.key);
            }
        }
    }

    #[test]
    fn a_capped_role_is_never_rolled() {
        let mut rng = rand::thread_rng();
        for _ in 0..2000 {
            if let Some(c) = roll(AiShipType::Shepherd, &mut rng, |c| c.role != ClassRole::Boss) {
                assert_ne!(c.role, ClassRole::Boss);
            }
        }
    }
}
