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

const fn class(key: &'static str, name: &'static str, role: ClassRole, weight: u32) -> ShipClass {
    ShipClass { key, name, role, weight }
}

use ClassRole::*;

// Weights: scouts and line ships are the bulk of any system, civilians are
// common enough to notice, experimentals and capitals are things you
// remember meeting.
const TERRAN: &[ShipClass] = &[
    class("lancer", "Lancer", Scout, 30),
    class("warden", "Warden", Line, 25),
    class("bulwark", "Bulwark", Line, 20),
    class("sovereign", "Sovereign", Capital, 4),
    class("oracle", "Oracle", Experimental, 6),
    class("homeward", "Homeward", Civilian, 6),
    class("hauler", "Convoy Hauler", Civilian, 10),
];

// The Throne of Ages is being redrawn. Until it is back, the original
// dreadnought design stands in as the boss (see `original_weight`).
const ETERNAL: &[ShipClass] = &[
    class("echo_escort", "Echo Escort", Line, 50),
    class("herald", "Herald", Line, 30),
    class("reliquary_of_echoes", "Reliquary of Echoes", Experimental, 15),
    // The big-ships capital: turret guns, a spinal, missiles and flak
    // (tools/faction-arming/build_eternal_vigil.py).
    class("vigil", "Vigil", Capital, 6),
];

const GILDED: &[ShipClass] = &[
    class("courier", "Courier", Scout, 25),
    class("dividend", "Dividend", Line, 25),
    class("debt_collector", "Debt Collector", Line, 15),
    class("throne_barge", "Throne Barge", Capital, 4),
    class("bullion_freighter", "Bullion Freighter", Civilian, 10),
];

const CHOIR: &[ShipClass] = &[
    class("shard", "Shard", Scout, 30),
    class("chorus", "Chorus", Line, 25),
    class("requiem", "Requiem", Capital, 5),
    class("echo_chamber", "Echo Chamber", Experimental, 8),
];

const SYNTHESIS: &[ShipClass] = &[
    class("graft", "Graft", Scout, 30),
    class("chimera", "Chimera", Line, 25),
    class("assimilator", "Assimilator", Capital, 5),
    class("bloom", "Bloom", Experimental, 8),
];

const CORPSE: &[ShipClass] = &[
    class("mourner", "Mourner", Line, 35),
    class("pyre", "Pyre", Line, 15),
    class("dead_sun", "Dead Sun", Capital, 4),
    class("ossuary", "Ossuary", Experimental, 8),
];

// Warden Tug and Herd-keeper are being redrawn; the seed ark and net-weaver
// fly alongside the original Preserve hull until they are back.
const PRESERVE: &[ShipClass] = &[
    class("net_weaver", "Net-weaver", Experimental, 25),
    class("seed_ark", "Seed Ark", Capital, 8),
];

const SILENCE: &[ShipClass] = &[
    class("listener", "Listener", Scout, 40),
    class("hush", "Hush", Line, 30),
    class("vigil", "Vigil", Line, 12),
];

// Traders: civilian hulls, and the arms bazaar that parks between two
// fleets mid-battle and sells to both. Their fighting is done by their ally.
const MERCHANTS: &[ShipClass] = &[
    class("pedlar", "Pedlar", Civilian, 30),
    class("hauler", "Container Hauler", Civilian, 25),
    class("bulk_freighter", "Bulk Freighter", Civilian, 12),
    class("tanker", "Tanker", Civilian, 12),
    class("mining_barge", "Mining Barge", Civilian, 10),
    class("arms_bazaar", "Arms Bazaar", Experimental, 5),
];

// Drawn 2026-10-21 for the trade war; awaiting review on the concepts page.
const COMPACT: &[ShipClass] = &[
    class("tug", "Tug", Civilian, 35),
    class("ore_hauler", "Ore Hauler", Civilian, 30),
    class("fuel_runner", "Fuel Runner", Civilian, 20),
];

const SHEPHERD: &[ShipClass] = &[
    class("lamb", "Lamb", Scout, 40),
    class("ram", "Ram", Line, 30),
    class("the_shepherd", "The Shepherd", Boss, 4),
];

/// Every class a faction flies. Empty means "the one original design".
pub fn roster(faction: AiShipType) -> &'static [ShipClass] {
    use AiShipType::*;
    match faction {
        TerranHegemony => TERRAN,
        EternalHegemony => ETERNAL,
        GildedThrone => GILDED,
        BrokenChoir => CHOIR,
        SynthesisCollective => SYNTHESIS,
        CorpseStars => CORPSE,
        StellarPreserve => PRESERVE,
        TheSilence => SILENCE,
        Shepherd => SHEPHERD,
        // All seven redrawn after "too basic"; flies the original until approved.
        RecursiveKingdom => &[],
        MerchantPrinces => MERCHANTS,
        FreeHaulersCompact => COMPACT,
    }
}

/// How often the faction's original design still turns up next to its new
/// roster: only while some of its classes are out for rework, so that slot
/// isn't simply empty.
pub fn original_weight(faction: AiShipType) -> u32 {
    match faction {
        AiShipType::StellarPreserve => 60,
        AiShipType::EternalHegemony => 5,
        _ => 0,
    }
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
