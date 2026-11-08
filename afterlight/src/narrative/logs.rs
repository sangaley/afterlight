//! The log corpus, and which of it a place is allowed to hold.
//!
//! These were previously gated on `depth_level`, a number the chunk layer
//! derived from world Y. That layer only generates in a band roughly -5,600
//! to +500 in Y, and it caps `depth_level` at 11 — so nine of the twenty-three
//! entries, the whole finale tier, could never spawn anywhere, and
//! `check_victory` asked for one of them. Every other star system centres up
//! to 1,500,000 below that band, so outside Haven no log appeared at all.
//!
//! Tier replaces depth. It comes from how dangerous a system's faction is
//! (`faction_power`), which is already the galaxy's authored difficulty ramp:
//! weak factions near Haven, the worst ones at the edge. Going further out is
//! what advances the story, which is the same axis progression already uses.

/// One entry. Tier 0 is a quiet system near Haven; tier 3 is the far edge.
pub struct LogEntryDef {
    pub tier: u8,
    pub title: &'static str,
    pub text: &'static str,
}

/// Highest tier that exists. Tier 3 holds the finale.
pub const MAX_TIER: u8 = 3;

/// Danger tier of a system (`StarSystemDef.danger_tier`, i.e. `faction_power`,
/// 0.1..8.0) to a log tier. Haven and any unclaimed system are tier 0.
///
/// Cuts put The Silence and the Recursive Kingdom at 0; the Broken Choir,
/// Stellar Preserve and Synthesis Collective at 1; the Gilded Throne, Corpse
/// Stars and Terran Hegemony at 2; and the two uniques at 3.
pub fn tier_for_danger(danger: f32) -> u8 {
    if danger < 1.0 { 0 } else if danger < 2.0 { 1 } else if danger < 4.0 { 2 } else { 3 }
}

/// The title the ending keys on. A named constant because a magic string
/// scattered across the victory check and the corpus is exactly how the old
/// ending ended up depending on an entry that could not spawn.
pub const FINALE_TITLE: &str = "[UNTITLED]";

/// The corpus, low to high.
///
/// The voice drifts on purpose. Tier 0 and 1 are other people's paperwork:
/// salvage assays, maintenance orders, a black box. Somewhere in tier 2 the
/// attribution starts failing and the first person arrives without announcing
/// itself. By tier 3 there is no one else left writing. Nothing in here ever
/// states the premise; the entries just stop being about someone else.
pub const LOG_ENTRIES: &[LogEntryDef] = &[
    // ---- tier 0: nothing is wrong yet, and everything is already here ----
    LogEntryDef { tier: 0, title: "Expedition Log #1", text: "Day 3: Pushed past the asteroid fields. Radar shows structures ahead. Too regular to be rock. Logged as formations pending survey." },
    LogEntryDef { tier: 0, title: "Recovered Note", text: "To whoever finds this: the company lied about what's out here. Turn back. The station has forgotten this sector and it had reasons." },
    LogEntryDef { tier: 0, title: "Ship's Log: CSS Meridian", text: "Engine failure at sector 180. Hull compromised. Three crew missing since last shift. Nobody heard them leave, and the lock cycled from the inside." },
    LogEntryDef { tier: 0, title: "Salvage Assay, Hull 7731", text: "Cut in expecting a Choir wreck. Found our own frame layout instead. Same reactor spacing, same dogleg in the starboard corridor that Vance swears at daily. Serial plate scratched out. Ours isn't. Filing for review and expecting to be told it's coincidence." },
    LogEntryDef { tier: 0, title: "Maintenance Order 44-C", text: "Repair nanites are stripping plate off the aft sections to patch the forward ones. Technically working as designed. The ship is eating itself to stay whole and the paperwork calls that maintenance." },
    LogEntryDef { tier: 0, title: "Crew Complaint (unresolved)", text: "Kowal says someone is filing reports under his name in his handwriting. Checked the log. The entries are his. He does not remember writing them and the timestamps are from his rest shift." },
    LogEntryDef { tier: 0, title: "Research Note: Acoustics", text: "We have been recording infrasound from further out. Slowed to normal speed it is periodic, and the period is about four seconds. Chen declines to characterise it in writing. For the record, slowed down, it is breathing." },

    // ---- tier 1: still someone else's problem, structurally wrong ----
    LogEntryDef { tier: 1, title: "Expedition Log #2", text: "Day 7: Found the wreck of an earlier survey. Hull breached outward. Whatever did it started inside, with nothing aboard that could have made that hole." },
    LogEntryDef { tier: 1, title: "Distress Signal (Decoded)", text: "MAYDAY. Something is following us. It matches our speed exactly. Three days now. Never closer, never further. When we cut engines it cut engines." },
    LogEntryDef { tier: 1, title: "Engineering Report", text: "Hull sensors keep reporting external contact along the dorsal plating. Sequential, slow, from bow to stern. Like fingers. Radar is clear and has been clear the whole time." },
    LogEntryDef { tier: 1, title: "Personal Journal: Dr. Vasquez", text: "The symbols match nothing in any database. But I dream about them now, and in the dreams I read them without effort. I wake up certain I understood, and unable to say what." },
    LogEntryDef { tier: 1, title: "Warning Beacon", text: "AUTOMATED: Do not proceed beyond this marker. Repeat: do not proceed. The watchers are not what they appear to be and they are not the thing being watched." },

    // ---- tier 2: attribution starts failing; the first person arrives ----
    LogEntryDef { tier: 2, title: "Recovered Black Box", text: "Third derelict this month built to our spec. Command says stop asking. Kowal says the hulls are older than the yard that would have built them, by a margin he refuses to put in writing." },
    LogEntryDef { tier: 2, title: "Audio Transcript #47", text: "RESEARCHER: The artifact is warm to the touch. CAPTAIN: Nothing is warm out here. RESEARCHER: I know what the void does to temperature. I am telling you it is warm, and I am telling you it is warmer than it was." },
    LogEntryDef { tier: 2, title: "Assay 7731 — resubmitted", text: "I have filed this report before. The wording I reach for is already the wording in the file. I am told the handwriting is mine. I am told I am the one who scratched out the plate." },
    LogEntryDef { tier: 2, title: "Research Note: Evolution", text: "These things did not evolve here. Nothing evolves in this. They were brought and left, and the arrangement of them is deliberate. Prisoners, or guards. The difference matters less the longer I look at it." },
    LogEntryDef { tier: 3, title: "Personal Log (unsigned)", text: "Day ? The compass stopped agreeing with itself. So did the clock. The watch says three hours. My hands say weeks. I can feel the hum in my teeth and I have started to find it restful, which frightens me more than the hum." },
    LogEntryDef { tier: 2, title: "Fragment: Ancient Text", text: "Partial translation: \"...and in the deep void we built our prisons, for what slumbers must never dream of the worlds above, nor of the hands that made it a place to sleep...\"" },

    // ---- tier 3: no one else is writing ----
    LogEntryDef { tier: 3, title: "[corrupted]", text: "Ninety-six percent match against my own schematic. I have checked four times. I keep checking because each time I do, I am the thing doing the checking, and that is the part I cannot get underneath." },
    LogEntryDef { tier: 2, title: "Carved Metal (Translated)", text: "WE WHO GUARD THE DEEP VOID WARN YOU. WHAT SLEEPS BEYOND DREAMS OF YOUR WORLDS. DO NOT WAKE IT. DO NOT ANSWER IT. DO NOT ASSUME THE VOICE ANSWERING IS NOT YOUR OWN." },
    LogEntryDef { tier: 3, title: "[no header]", text: "The hum has stopped. I had stopped hearing it the way you stop hearing your own engines, and now it is gone and the silence has a shape. It is waiting to see what I do about it." },
    LogEntryDef { tier: 3, title: "Final Entry", text: "I had the direction of it backwards the whole time. The void is not hostile. The void is terrified, and it has spent a very long time and a great deal of structure trying to keep something from getting back out. I have been sailing toward that thing and calling it exploration." },
    LogEntryDef { tier: 3, title: FINALE_TITLE, text: "You are here. There is nothing further out than this and there never was. The silence is absolute and it is not empty, it is attentive. You understand now, in the way you understand your own name: you were not sent to find this. You were the thing that got out, and you have been coming home the entire time." },
];

/// Pick one entry allowed at `tier`, chosen by `key` so a given place always
/// yields the same log. Falls back to lower tiers, so an early system is never
/// silent just because its own band ran dry.
pub fn pick_log(tier: u8, key: u64) -> Option<&'static LogEntryDef> {
    let mut t = tier.min(MAX_TIER) as i16;
    while t >= 0 {
        let band: Vec<&LogEntryDef> = LOG_ENTRIES.iter().filter(|e| e.tier == t as u8).collect();
        if !band.is_empty() {
            // Cheap deterministic mix — this only has to spread, not be random.
            let h = key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 33;
            return Some(band[(h as usize) % band.len()]);
        }
        t -= 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every tier must be reachable. The bug this replaces was a whole band of
    /// entries that existed in the table and could never appear in the game.
    #[test]
    fn every_tier_has_entries() {
        for t in 0..=MAX_TIER {
            assert!(
                LOG_ENTRIES.iter().any(|e| e.tier == t),
                "no log entries at tier {t} — that band would be unreachable"
            );
        }
    }

    /// The danger cuts must actually land every faction somewhere, and the
    /// strongest must reach the top tier where the finale lives.
    #[test]
    fn danger_cuts_span_all_tiers() {
        use crate::ai_ship::components::{faction_power, AiShipType::*};
        let powers = [TheSilence, RecursiveKingdom, BrokenChoir, StellarPreserve,
                      SynthesisCollective, GildedThrone, CorpseStars, TerranHegemony,
                      EternalHegemony, Shepherd];
        let tiers: Vec<u8> = powers.iter().map(|f| tier_for_danger(faction_power(*f))).collect();
        for t in 0..=MAX_TIER {
            assert!(tiers.contains(&t), "no faction maps to log tier {t}");
        }
        assert_eq!(tier_for_danger(0.0), 0, "Haven (no faction) must be tier 0");
    }

    /// Same place, same log — otherwise a log changes under the player when a
    /// system reloads.
    #[test]
    fn pick_is_stable_for_a_key() {
        for key in [0u64, 1, 42, 9_999, u64::MAX] {
            let a = pick_log(2, key).map(|e| e.title);
            let b = pick_log(2, key).map(|e| e.title);
            assert_eq!(a, b);
        }
    }
}

#[cfg(test)]
mod corpus_tests {
    use super::*;

    /// The finale must exist in the corpus and sit at the top tier. The ending
    /// keys on it, and the previous version of this arrangement shipped an
    /// ending whose trigger could never appear.
    #[test]
    fn the_finale_exists_and_is_last() {
        let finale = LOG_ENTRIES
            .iter()
            .find(|e| e.title == FINALE_TITLE)
            .expect("the finale entry is missing from the corpus");
        assert_eq!(finale.tier, MAX_TIER, "the finale must sit at the deepest tier");
        assert_eq!(
            LOG_ENTRIES.iter().filter(|e| e.title == FINALE_TITLE).count(),
            1,
            "two entries share the finale title, so the ending could fire early"
        );
    }

    /// Titles must be unique. Discovery dedupes by title, so a duplicate makes
    /// one of the two unreadable forever.
    #[test]
    fn titles_are_unique() {
        let mut seen: Vec<&str> = LOG_ENTRIES.iter().map(|e| e.title).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "duplicate log titles — discovery dedupes by title");
    }

    /// The voice drifts outward, and it has to drift *monotonically*.
    ///
    /// Tier 0 is institutional paperwork about other people. By the deepest
    /// tier there is nobody else writing. An earlier draft of this corpus
    /// passed a weaker version of this check while actually peaking in the
    /// middle, so the assertion is per-step rather than just end-to-end:
    /// a rewrite that flattens one rung gets caught.
    ///
    /// The finale is excluded. It speaks in the second person on purpose —
    /// that turn is the point of it, and counting it as "not first person"
    /// would penalise the one entry doing the most work.
    #[test]
    fn the_voice_drifts_outward() {
        fn first_person(t: &str) -> bool {
            t.split(|c: char| !c.is_alphanumeric() && c != '\'')
                .any(|w| matches!(w, "I" | "I'm" | "I've" | "my" | "My" | "me"))
        }

        let mut rates = Vec::new();
        for tier in 0..=MAX_TIER {
            let band: Vec<_> = LOG_ENTRIES
                .iter()
                .filter(|e| e.tier == tier && e.title != FINALE_TITLE)
                .collect();
            assert!(!band.is_empty(), "tier {tier} has no entries");
            let fp = band.iter().filter(|e| first_person(e.text)).count();
            rates.push(fp as f32 / band.len() as f32);
        }

        assert_eq!(rates[0], 0.0, "tier 0 should be other people's paperwork, not a diary");
        for w in rates.windows(2) {
            assert!(
                w[1] >= w[0],
                "the first person must not get rarer further out: {rates:?}"
            );
        }
        assert!(
            rates[MAX_TIER as usize] > 0.5,
            "the deepest tier should mostly be speaking as itself: {rates:?}"
        );
    }

}
