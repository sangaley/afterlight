//! Where each of the expedition's records lies.
//!
//! The records used to be scattered by chance: any wreck, ruin or cave near
//! Haven had a 45% chance of carrying one from the opening band, and every
//! derelict elsewhere drew at random from its system's band. Haven's
//! neighbourhood is dense, so playtests found half the demo's trail in the
//! first three minutes -- six log cards back to back -- and then nothing at
//! all for the next forty: every early system was drawing from a band already
//! read, and the next band only exists further out.
//!
//! Now every record has one home, decided by the galaxy itself. Within each
//! tier, records go to that tier's systems nearest Haven first, in the
//! corpus's reading order. Haven holds the first. Each new system you reach
//! holds the next thing to read, so reading the trail in order *is* going
//! outward -- which is what the story's arc already rides on (distance from
//! Haven, see `CascadeState`).

use crate::celestial::resources::GalaxyMap;
use super::logs::{tier_for_danger, LogEntryDef, LOG_ENTRIES, MAX_TIER};

/// Every record and the system it lies in, in trail order: by tier, then by
/// the system's distance from Haven, then corpus order within a system.
///
/// A tier with more records than systems doubles up, nearest first. A tier no
/// system carries in this galaxy leaves its records unplaced; the galaxy's
/// own tests guarantee every tier is reachable.
pub fn plan(galaxy: &GalaxyMap) -> Vec<(u32, &'static LogEntryDef)> {
    let mut out = Vec::new();
    for tier in 0..=MAX_TIER {
        let mut systems: Vec<_> = galaxy
            .systems
            .iter()
            .filter(|s| tier_for_danger(s.danger_tier) == tier)
            .collect();
        if systems.is_empty() {
            continue;
        }
        systems.sort_by(|a, b| {
            a.galaxy_pos
                .length()
                .total_cmp(&b.galaxy_pos.length())
                .then(a.id.cmp(&b.id))
        });
        let band = LOG_ENTRIES.iter().filter(|e| e.tier == tier);
        let mut placed: Vec<(u32, &'static LogEntryDef)> = band
            .enumerate()
            .map(|(k, entry)| (systems[k % systems.len()].id, entry))
            .collect();
        // Doubled-up systems keep reading order, and the trail visits each
        // system's records together.
        placed.sort_by_key(|(id, _)| systems.iter().position(|s| s.id == *id));
        out.extend(placed);
    }
    out
}

/// The records `system_id` holds, in reading order.
pub fn records_in(galaxy: &GalaxyMap, system_id: u32) -> Vec<&'static LogEntryDef> {
    plan(galaxy)
        .into_iter()
        .filter(|(id, _)| *id == system_id)
        .map(|(_, entry)| entry)
        .collect()
}

/// The next record to read: the first in trail order whose title isn't in
/// `read`, with the system it lies in. Only records up to `max_tier` count, so
/// a demo build stops pointing once its own trail runs out.
pub fn next_record(
    galaxy: &GalaxyMap,
    read: &[String],
    max_tier: u8,
) -> Option<(u32, &'static LogEntryDef)> {
    plan(galaxy)
        .into_iter()
        .filter(|(_, e)| e.tier <= max_tier)
        .find(|(_, e)| !read.iter().any(|r| r == e.title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::celestial::galaxy::generate_galaxy_map;

    /// Haven holds the very first record, so the trail starts at home.
    #[test]
    fn haven_holds_the_first_record() {
        let galaxy = generate_galaxy_map(42);
        let first = plan(&galaxy)[0];
        assert_eq!(first.0, 0, "the first record isn't at Haven");
        assert_eq!(first.1.title, LOG_ENTRIES[0].title);
    }

    /// Every record has exactly one home, and none sits in a system whose
    /// tier doesn't match it.
    #[test]
    fn every_record_has_one_home_in_its_own_tier() {
        for seed in [1u64, 42, 777, 123_456] {
            let galaxy = generate_galaxy_map(seed);
            let plan = plan(&galaxy);
            assert_eq!(plan.len(), LOG_ENTRIES.len(), "seed {seed}: some records unplaced");
            for (id, entry) in &plan {
                let sys = galaxy.systems.iter().find(|s| s.id == *id).unwrap();
                assert_eq!(tier_for_danger(sys.danger_tier), entry.tier, "seed {seed}: {} misplaced", entry.title);
            }
        }
    }

    /// The pacing fix itself: Haven no longer hands over the whole opening
    /// band. One record at home, the rest out in the systems beyond it.
    #[test]
    fn the_opening_band_is_spread_across_systems() {
        let galaxy = generate_galaxy_map(42);
        let at_haven = records_in(&galaxy, 0).len();
        let band = LOG_ENTRIES.iter().filter(|e| e.tier == 0).count();
        assert!(at_haven <= 2, "Haven holds {at_haven} of {band} opening records");
        let homes: std::collections::HashSet<u32> =
            plan(&galaxy).into_iter().filter(|(_, e)| e.tier == 0).map(|(id, _)| id).collect();
        assert!(homes.len() >= 3, "the opening band sits in only {} systems", homes.len());
    }

    /// Reading order is trail order: the next record is the first unread one.
    #[test]
    fn next_record_follows_the_trail() {
        let galaxy = generate_galaxy_map(42);
        let plan = plan(&galaxy);
        let read = vec![plan[0].1.title.to_string()];
        let next = next_record(&galaxy, &read, MAX_TIER).unwrap();
        assert_eq!(next.1.title, plan[1].1.title);
    }
}
