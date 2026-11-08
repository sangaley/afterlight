//! What each recovered log does once it's read.
//!
//! The logs used to be mood and nothing else: other people's paperwork you
//! read and closed. Now every one points somewhere or changes something: it
//! marks a system on the map (a lead, a warning, or a Summit Record capital),
//! hands over what's left in the wreck it came from, patches the hull, or sets
//! the instruments lying. Each keeps its words and gains one closing line
//! saying what it points at ("Logs rework draft" in the plan doc).
//!
//! Where a log points is worked out from the galaxy and the trail alone, never
//! from anything that moves, so the map's notes can be rebuilt from the save's
//! list of logs read and nothing new has to be saved.

use bevy::prelude::*;

use crate::ai_ship::components::AiShipType;
use crate::celestial::resources::{GalaxyMap, SystemDiscovery};
use crate::resources::ItemType;

use super::logs::{tier_for_danger, FINALE_TITLE};

/// A system a log points at, worked out from where the log was found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// Where the next record on the trail lies.
    NextRecord,
    /// The nearest system held by a faction that shoots on sight.
    NearestHostile,
    /// The nearest capital of a faction that shoots on sight, and its fleet.
    NearestHostileCapital,
    /// The nearest system one ring further out.
    NextRing,
    /// A faction's capital.
    Capital(AiShipType),
    /// The nearest system a boss haunts.
    Haunt(AiShipType),
    /// Where the last record lies.
    Finale,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LogEffect {
    /// Marks a system to go to.
    Lead(Target),
    /// Marks a system to keep away from.
    Warning(Target),
    /// Marks a Summit Record capital.
    Story(AiShipType),
    /// What's left in the wreck goes into the hold.
    Cache { items: &'static [(ItemType, u32)], credits: u32 },
    /// The hull is patched by this share of each plate's full health.
    Patch(f32),
    /// The instruments lie for `secs`, at least this badly (interference).
    Instruments { secs: f32, phase: u8 },
    /// The ending (truth.rs), as before.
    Ending,
}

pub struct LogLead {
    pub title: &'static str,
    pub effect: LogEffect,
    /// The line added under the log. `{system}` is the system it points at.
    /// Empty: nothing added.
    pub coda: &'static str,
}

use AiShipType::*;
use LogEffect::*;
use Target::*;

/// Every log's effect, in corpus order.
pub const LOG_LEADS: &[LogLead] = &[
    // ---- ring 1, near Haven: practical, and the first pull toward the story ----
    LogLead { title: "Expedition Log #1", effect: Lead(NextRecord), coda: "Coordinates attached: {system}." },
    LogLead { title: "Recovered Note", effect: Warning(NearestHostile), coda: "Stay out of {system}. Whatever runs there runs without lights." },
    LogLead { title: "Ship's Log: CSS Meridian", effect: Cache { items: &[(ItemType::FuelCell, 3), (ItemType::AmmoCrate, 2)], credits: 0 }, coda: "Hold still sealed. Fuel and shells aboard, if anyone wants them." },
    LogLead { title: "Salvage Assay, Hull 7731", effect: Cache { items: &[(ItemType::RareAlloy, 2)], credits: 0 }, coda: "The plate is good. We took what we could carry." },
    LogLead { title: "Maintenance Order 44-C", effect: Patch(0.10), coda: "Rerouting instructions attached. Your nanites read them." },
    LogLead { title: "Crew Complaint (unresolved)", effect: Story(TheSilence), coda: "Kowal asked to be sent to the monks at {system}. He said they would know whose handwriting it was." },
    LogLead { title: "Research Note: Acoustics", effect: Story(BrokenChoir), coda: "Chen sent the recording to the Choir at {system}. It came back with a name on it." },
    // ---- ring 2, the end of the demo trail ----
    LogLead { title: "Expedition Log #2", effect: Cache { items: &[], credits: 400 }, coda: "The survey's strongbox is still bolted to the deck." },
    LogLead { title: "Distress Signal (Decoded)", effect: Warning(NearestHostileCapital), coda: "Last position before the signal ends: {system}." },
    LogLead { title: "Engineering Report", effect: Instruments { secs: 1.5, phase: 2 }, coda: "" },
    LogLead { title: "Personal Journal: Dr. Vasquez", effect: Story(MerchantPrinces), coda: "The Princes offered her a price for the symbols, at {system}. She asked where they had seen them before." },
    LogLead { title: "Warning Beacon", effect: Lead(NextRing), coda: "The marker stands on the road to {system}." },
    // ---- ring 3, past the demo trail ----
    LogLead { title: "Recovered Black Box", effect: Cache { items: &[], credits: 600 }, coda: "An insurance chit, made out to a ship with your serial. It still pays." },
    LogLead { title: "Audio Transcript #47", effect: Cache { items: &[(ItemType::AncientArtifact, 1)], credits: 0 }, coda: "The artifact is still aboard. It is still warm." },
    LogLead { title: "Assay 7731 \u{2014} resubmitted", effect: Instruments { secs: 10.0, phase: 2 }, coda: "" },
    LogLead { title: "Research Note: Evolution", effect: Story(StellarPreserve), coda: "The Preserve keeps one alive at {system}. They say it is quieter there." },
    LogLead { title: "Fragment: Ancient Text", effect: Warning(Haunt(EternalHegemony)), coda: "The rest of the inscription is at {system}. So is what wrote it." },
    LogLead { title: "Carved Metal (Translated)", effect: Instruments { secs: 6.0, phase: 2 }, coda: "" },
    // ---- ring 4, the far edge ----
    LogLead { title: "Personal Log (unsigned)", effect: Instruments { secs: 15.0, phase: 2 }, coda: "" },
    LogLead { title: "[corrupted]", effect: Instruments { secs: 15.0, phase: 3 }, coda: "" },
    LogLead { title: "[no header]", effect: Lead(Haunt(Shepherd)), coda: "It is waiting at {system}." },
    LogLead { title: "Final Entry", effect: Lead(Finale), coda: "" },
    LogLead { title: FINALE_TITLE, effect: Ending, coda: "" },
];

/// A log's effect, by title.
pub fn lead(title: &str) -> Option<&'static LogLead> {
    LOG_LEADS.iter().find(|l| l.title == title)
}

/// The system a log lies in on this galaxy's trail.
pub fn found_in(galaxy: &GalaxyMap, title: &str) -> Option<u32> {
    super::trail::plan(galaxy).into_iter().find(|(_, e)| e.title == title).map(|(s, _)| s)
}

/// Where a log's target is, counted from the system it was found in.
pub fn resolve(target: Target, galaxy: &GalaxyMap, title: &str) -> Option<u32> {
    let from = found_in(galaxy, title)?;
    let at = galaxy.systems.iter().find(|s| s.id == from)?.galaxy_pos;
    let nearest = |keep: &dyn Fn(&crate::celestial::resources::StarSystemDef) -> bool| {
        galaxy
            .systems
            .iter()
            .filter(|s| s.id != from && keep(s))
            .min_by(|a, b| a.galaxy_pos.distance(at).total_cmp(&b.galaxy_pos.distance(at)))
            .map(|s| s.id)
    };
    // Hostile as the run starts, so the answer never moves under a saved note.
    let hostile = |f: Option<AiShipType>| {
        f.is_some_and(|f| crate::diplomacy::starting_standing(f) < crate::diplomacy::StandingTier::HOSTILE_BELOW)
    };
    match target {
        NextRecord => {
            let plan = super::trail::plan(galaxy);
            let i = plan.iter().position(|(_, e)| e.title == title)?;
            plan.iter().skip(i + 1).map(|(s, _)| *s).find(|s| *s != from)
        }
        NearestHostile => nearest(&|s| hostile(s.faction)),
        NearestHostileCapital => nearest(&|s| s.capital && hostile(s.faction)),
        NextRing => {
            let ring = tier_for_danger(galaxy.systems.iter().find(|s| s.id == from)?.danger_tier);
            nearest(&|s| tier_for_danger(s.danger_tier) == ring + 1)
        }
        Capital(f) => crate::celestial::territory::capital_of(galaxy, f),
        // A boss haunts few systems, and the log may lie in the one it means.
        Haunt(f) => nearest(&|s| s.haunt == Some(f)).or_else(|| galaxy.systems.iter().find(|s| s.id == from && s.haunt == Some(f)).map(|s| s.id)),
        Finale => found_in(galaxy, FINALE_TITLE),
    }
}

/// The system a log marks on the map, if it marks one.
pub fn marks(galaxy: &GalaxyMap, title: &str) -> Option<u32> {
    match lead(title)?.effect {
        Lead(t) | Warning(t) => resolve(t, galaxy, title),
        Story(f) => resolve(Capital(f), galaxy, title),
        _ => None,
    }
}

/// The line added under a log, filled in.
pub fn coda(galaxy: &GalaxyMap, title: &str) -> Option<String> {
    let l = lead(title)?;
    if l.coda.is_empty() {
        return None;
    }
    let name = marks(galaxy, title)
        .and_then(|id| galaxy.systems.iter().find(|s| s.id == id))
        .map_or("somewhere further out".to_string(), |s| s.name.clone());
    Some(l.coda.replace("{system}", &name))
}

/// The map's notes from the logs read so far: (system, line, is a warning).
pub fn notes(galaxy: &GalaxyMap, read: &[String]) -> Vec<(u32, String, bool)> {
    read.iter()
        .filter_map(|title| {
            let system = marks(galaxy, title)?;
            let warning = matches!(lead(title)?.effect, Warning(_));
            Some((system, coda(galaxy, title)?, warning))
        })
        .collect()
}

/// A log was just read for the first time.
#[derive(Message, Clone, Debug)]
pub struct LogRead(pub String);

/// Do what a log does: add its closing line to the card, and mark, give,
/// patch or unsettle.
#[allow(clippy::too_many_arguments)]
fn apply_log_effects(
    mut read: MessageReader<LogRead>,
    mut galaxy: ResMut<GalaxyMap>,
    mut queue: ResMut<super::reader::LogQueue>,
    mut inventory: ResMut<crate::resources::Inventory>,
    mut currency: ResMut<crate::resources::Currency>,
    mut interference: ResMut<super::interference::Interference>,
    ship: Query<Entity, With<crate::components::Ship>>,
    mut hull: Query<(&mut crate::components::HullSegment, &ChildOf)>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    for LogRead(title) in read.read() {
        let Some(l) = lead(title) else { continue };
        info!("LOG read: {title} -> {:?}", l.effect);
        if let Some(line) = coda(&galaxy, title) {
            if let Some(card) = queue.pending.iter_mut().find(|(t, _)| t == title) {
                card.1 = format!("{}\n\n{line}", card.1);
            }
        }
        let note = |message: String| crate::events::ShowNotification {
            message,
            notification_type: crate::events::NotificationType::Info,
            duration: 6.0,
        };
        match l.effect {
            Lead(_) | Warning(_) | Story(_) => {
                let Some(id) = marks(&galaxy, title) else { continue };
                if let Some(sys) = galaxy.systems.iter_mut().find(|s| s.id == id) {
                    if sys.discovery == SystemDiscovery::Unknown {
                        sys.discovery = SystemDiscovery::Located;
                    }
                    let what = if matches!(l.effect, Warning(_)) { "Warning marked on your map" } else { "Marked on your map" };
                    notes.write(note(format!("{what}: {}", sys.name)));
                }
            }
            Cache { items, credits } => {
                let mut got = Vec::new();
                for &(item, n) in items {
                    if inventory.add_item(item, n) {
                        got.push(format!("{n} {}", item.name()));
                    }
                }
                if credits > 0 {
                    currency.credits += credits;
                    got.push(format!("{credits} credits"));
                }
                if !got.is_empty() {
                    notes.write(note(format!("From the wreck: {}", got.join(", "))));
                }
            }
            Patch(share) => {
                let Ok(ship) = ship.single() else { continue };
                for (mut seg, parent) in &mut hull {
                    if parent.parent() == ship && seg.health > 0.0 {
                        seg.health = (seg.health + seg.max_health * share).min(seg.max_health);
                    }
                }
                notes.write(note("Hull patched a little, the way the order says.".into()));
            }
            Instruments { secs, phase } => interference.jolt(secs, phase),
            Ending => {}
        }
    }
}

/// Debug override, in the house style (`AFTERLIGHT_SUMMIT`): `AFTERLIGHT_READ_LOGS=
/// Expedition Log #1|Recovered Note` reads those logs ten seconds into the
/// run, through the same queue and effects a derelict's pickup uses.
fn read_from_env(
    time: Res<Time>,
    mut stats: ResMut<crate::resources::Statistics>,
    mut queue: ResMut<super::reader::LogQueue>,
    mut read: MessageWriter<LogRead>,
    mut waited: Local<f32>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    *waited += time.delta_secs();
    if *waited < 10.0 {
        return;
    }
    *done = true;
    let Ok(want) = std::env::var("AFTERLIGHT_READ_LOGS") else { return };
    for title in want.split('|').map(str::trim) {
        let Some(entry) = super::logs::LOG_ENTRIES.iter().find(|e| e.title == title) else {
            warn!("AFTERLIGHT_READ_LOGS: no log called {title:?}");
            continue;
        };
        if !stats.logs_found.iter().any(|f| f == title) {
            stats.logs_found.push(title.to_string());
            queue.push(title.to_string(), entry.text.to_string());
            read.write(LogRead(title.to_string()));
        }
    }
}

pub struct LeadsPlugin;

impl Plugin for LeadsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<LogRead>().add_systems(Update, apply_log_effects).add_systems(
            Update,
            read_from_env.run_if(in_state(crate::states::GameState::Exploring)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::narrative::logs::LOG_ENTRIES;

    /// Every log does something, and only logs that exist have an effect.
    #[test]
    fn every_log_points_somewhere_or_changes_something() {
        for e in LOG_ENTRIES {
            assert!(lead(e.title).is_some(), "{} does nothing", e.title);
        }
        for l in LOG_LEADS {
            assert!(LOG_ENTRIES.iter().any(|e| e.title == l.title), "no log called {}", l.title);
            assert!(l.coda.is_ascii(), "{}: not plain ASCII", l.title);
        }
        assert_eq!(LOG_LEADS.len(), LOG_ENTRIES.len());
    }

    /// Every log that points somewhere finds a real system on real galaxies,
    /// and its closing line never shows a placeholder.
    #[test]
    fn every_pointer_lands_on_a_system() {
        for seed in [1u64, 42, 777] {
            let galaxy = crate::celestial::galaxy::generate_galaxy_map(seed);
            for l in LOG_LEADS {
                if matches!(l.effect, Lead(_) | Warning(_) | Story(_)) {
                    let id = marks(&galaxy, l.title).unwrap_or_else(|| panic!("seed {seed}: {} points nowhere", l.title));
                    if !matches!(l.effect, Lead(Haunt(_)) | Warning(Haunt(_)) | Lead(Finale)) {
                        assert_ne!(Some(id), found_in(&galaxy, l.title), "{} points at its own system", l.title);
                    }
                }
                if let Some(line) = coda(&galaxy, l.title) {
                    assert!(!line.contains('{'), "unfilled: {line}");
                }
            }
        }
    }

    /// The opening stays quiet: nothing in the first ring lies.
    #[test]
    fn the_first_ring_is_honest() {
        for e in LOG_ENTRIES.iter().filter(|e| e.tier == 0) {
            assert!(!matches!(lead(e.title).unwrap().effect, Instruments { .. }), "{} lies in ring 1", e.title);
        }
    }

    /// Notes come back from the list of logs read alone, the same every time.
    #[test]
    fn notes_rebuild_from_what_was_read() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let read = vec!["Expedition Log #1".to_string(), "Recovered Note".to_string(), "Ship's Log: CSS Meridian".to_string()];
        let a = notes(&galaxy, &read);
        assert_eq!(a.len(), 2, "the cache marks nothing");
        assert!(a[1].2, "the note is a warning");
        assert_eq!(a, notes(&galaxy, &read));
    }
}
