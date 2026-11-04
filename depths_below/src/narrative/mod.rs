//! Narrative layer: what the game is about, told through what it already
//! simulates rather than through cutscenes.

pub mod logs;
pub mod doppelganger;
pub mod expedition;
pub mod interference;
pub mod leads;
pub mod reader;
pub mod summit;
pub mod trail;
pub mod truth;

use bevy::prelude::*;

use crate::celestial::resources::{GalaxyMap, SystemDiscovery, SystemStreamingManager};
use crate::resources::Statistics;
use crate::states::GameState;

/// How far along the story is, and how bad it is where the player is standing.
///
/// Deliberately **derived, never stored**. Everything it reads is already in
/// the save — which systems have been visited, which logs have been read, and
/// where the player is — so there is no new field to persist, no migration,
/// and no way for it to drift out of step with the run. It also means loading
/// an old save produces the right value immediately rather than a zeroed one.
///
/// This matters for a second reason. `handle_save_request` is at Bevy's
/// 16-parameter limit and says so in a comment, so a new saved resource is not
/// a free change. Deriving sidesteps that entirely.
/// Raised the moment the finale entry is *read*, and only then.
///
/// The ending used to trigger on `Statistics.logs_found` containing the
/// finale, which is a state rather than an event: loading any save made after
/// the ending would replay the whole sequence, and a new expedition that
/// inherited the old run's statistics would fire it in the first second.
/// Finding it is the moment that means something.
#[derive(Resource, Debug, Default)]
pub struct FinaleFound(pub bool);

#[derive(Resource, Debug, Default)]
pub struct CascadeState {
    /// 0.0 at the first launch, 1.0 at the far edge with everything read.
    /// Progression: how far the player has come, and it never goes down.
    pub level: f32,
    /// Local intensity: the log tier of the system the player is in right
    /// now. Flying home lowers this; it is "how bad is it *here*", not "how
    /// far along am I".
    pub ring: u8,
    /// Fraction of the corpus read, 0.0..1.0.
    pub read: f32,
    /// Furthest galaxy distance any visited system sits at, over the galaxy
    /// radius. The high-water mark, so retreating does not rewind the story.
    pub reach: f32,
}

impl CascadeState {
    /// Debug override, in the house style (`DEPTHS_MOVETEST`,
    /// `DEPTHS_SKIP_MENU`, `DEPTHS_AI_VS_AI_TEST`). Without it, seeing a late
    /// beat costs an hour of flying, which is how late beats end up untested.
    ///
    /// `DEPTHS_CASCADE=0.8` pins the level; ring follows unless the player is
    /// somewhere that says otherwise.
    fn override_level() -> Option<f32> {
        std::env::var("DEPTHS_CASCADE").ok()?.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
    }
}

/// Weighting between "how far out have you been" and "how much have you read".
/// Distance leads because it is the axis the player actually steers.
const REACH_WEIGHT: f32 = 0.6;
const READ_WEIGHT: f32 = 0.4;

/// How far out the player has actually been, as a fraction of the galaxy's
/// radius. A high-water mark, so retreating never rewinds the story.
///
/// Only Visited counts. Systems that are merely Located show up on the map
/// from passive sensor range, and seeing a light is not the same as going
/// there — if Located counted, the arc would advance while the player sat
/// still.
///
/// This is the term the whole story rides on, so it is a free function with
/// tests rather than four lines buried in a system nothing can call.
pub fn reach_from_visited(galaxy: &GalaxyMap, radius: f32) -> f32 {
    let radius = radius.max(1.0);
    galaxy
        .systems
        .iter()
        .filter(|s| matches!(s.discovery, SystemDiscovery::Visited))
        .map(|s| s.galaxy_pos.length() / radius)
        .fold(0.0f32, f32::max)
        .clamp(0.0, 1.0)
}

fn update_cascade(
    time: Res<Time>,
    galaxy: Res<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
    stats: Res<Statistics>,
    mut cascade: ResMut<CascadeState>,
    mut last_trace: Local<f32>,
) {
    let radius = crate::celestial::galaxy::GALAXY_RADIUS.max(1.0);

    let reach = reach_from_visited(&galaxy, radius);

    let total = logs::LOG_ENTRIES.len().max(1) as f32;
    let read = (stats.logs_found.len() as f32 / total).clamp(0.0, 1.0);

    cascade.reach = reach;
    cascade.read = read;
    cascade.level = CascadeState::override_level()
        .unwrap_or_else(|| (REACH_WEIGHT * reach + READ_WEIGHT * read).clamp(0.0, 1.0));

    // DEPTHS_CASCADE_TRACE=1 prints the arc's state as it moves. Kept rather
    // than deleted because the open question about this system is pacing --
    // how long a real player takes to climb it -- and that can only be
    // answered by watching it during an actual session. Throttled to once a
    // second so a long run stays readable.
    if std::env::var("DEPTHS_CASCADE_TRACE").is_ok() {
        let now = time.elapsed_secs();
        if now - *last_trace >= 1.0 {
            *last_trace = now;
            info!(
                "[CASCADE] level={:.3} reach={:.3} read={:.3} ring={} sys={:?}",
                cascade.level, cascade.reach, cascade.read, cascade.ring, streaming.loaded_system
            );
        }
    }

    // Where the player is standing right now. A blind warp into empty space
    // has no system, so fall back to raw distance from Haven.
    cascade.ring = match streaming.loaded_system.and_then(|id| galaxy.systems.iter().find(|s| s.id == id)) {
        Some(sys) => logs::tier_for_danger(sys.danger_tier),
        None => {
            let d = streaming.current_galaxy_pos.length() / radius;
            ((d * (logs::MAX_TIER as f32 + 1.0)) as u8).min(logs::MAX_TIER)
        }
    };
}

pub struct NarrativePlugin;

impl Plugin for NarrativePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FinaleFound>();
        app.add_plugins(reader::LogReaderPlugin);
        app.add_plugins(truth::TruthPlugin);
        app.add_plugins(doppelganger::DoppelgangerPlugin);
        app.add_plugins(interference::InterferencePlugin);
        app.add_plugins(expedition::ExpeditionPlugin);
        app.add_plugins(summit::SummitPlugin);
        app.add_plugins(leads::LeadsPlugin);
        app.init_resource::<CascadeState>().add_systems(
            Update,
            (update_cascade, grant_hull_materials)
                .chain()
                .run_if(in_state(GameState::Exploring).or_else(in_state(GameState::StationDocked))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reading everything without leaving home must not finish the story, and
    /// flying to the edge without reading anything must not either. Both
    /// halves have to contribute or one of them is decoration.
    #[test]
    fn neither_axis_alone_completes_the_arc() {
        assert!((REACH_WEIGHT + READ_WEIGHT - 1.0).abs() < f32::EPSILON);
        assert!(REACH_WEIGHT < 1.0 && READ_WEIGHT < 1.0);
        assert!(REACH_WEIGHT > READ_WEIGHT, "distance is the axis the player steers");
    }

    use crate::celestial::resources::StarSystemDef;

    fn sys(id: u32, pos: Vec2, discovery: SystemDiscovery) -> StarSystemDef {
        StarSystemDef {
            id,
            name: format!("S{id}"),
            galaxy_pos: pos,
            local_center: Vec2::ZERO,
            seed: 0,
            faction: None,
            capital: false,
            haunt: None,
            danger_tier: 0.0,
            discovery,
            last_updated: 0.0,
            resource_fraction_remaining: 1.0,
        }
    }

    /// Sitting at Haven must read as zero however long you sit there. Haven is
    /// at the galaxy origin, and if this were ever non-zero the story would
    /// start advancing before the player had gone anywhere.
    #[test]
    fn haven_alone_is_no_reach() {
        let g = GalaxyMap {
            systems: vec![sys(0, Vec2::ZERO, SystemDiscovery::Visited)],
            galaxy_seed: 1,
        };
        assert_eq!(reach_from_visited(&g, 5_000_000.0), 0.0);
    }

    /// Seeing a system is not going to one. Passive sensors mark neighbours as
    /// Located from a long way off; if that counted, the arc would advance
    /// while the player sat still at the station.
    #[test]
    fn located_is_not_visited() {
        let g = GalaxyMap {
            systems: vec![
                sys(0, Vec2::ZERO, SystemDiscovery::Visited),
                sys(1, Vec2::new(5_000_000.0, 0.0), SystemDiscovery::Located),
                sys(2, Vec2::new(4_000_000.0, 0.0), SystemDiscovery::Unknown),
            ],
            galaxy_seed: 1,
        };
        assert_eq!(reach_from_visited(&g, 5_000_000.0), 0.0);
    }

    /// Going somewhere moves it, and the far edge reads as the far edge.
    #[test]
    fn visiting_the_edge_reads_as_the_edge() {
        let g = GalaxyMap {
            systems: vec![
                sys(0, Vec2::ZERO, SystemDiscovery::Visited),
                sys(1, Vec2::new(5_000_000.0, 0.0), SystemDiscovery::Visited),
            ],
            galaxy_seed: 1,
        };
        assert!((reach_from_visited(&g, 5_000_000.0) - 1.0).abs() < 1e-6);
    }

    /// It is a high-water mark. Flying home must not rewind the story — the
    /// player has still been out there, and the point of the whole arc is that
    /// you cannot take it back.
    #[test]
    fn coming_home_does_not_rewind_it() {
        let far = vec![
            sys(0, Vec2::ZERO, SystemDiscovery::Visited),
            sys(1, Vec2::new(2_500_000.0, 0.0), SystemDiscovery::Visited),
        ];
        let g = GalaxyMap { systems: far, galaxy_seed: 1 };
        let out = reach_from_visited(&g, 5_000_000.0);
        assert!((out - 0.5).abs() < 1e-6, "got {out}");
        // The player is now back at Haven; the set of visited systems is
        // unchanged, so the figure must be too.
        assert_eq!(reach_from_visited(&g, 5_000_000.0), out);
    }

    /// The opening must read as zero. If a fresh run starts part-way up the
    /// scale, the quiet first hour the story depends on never happens.
    #[test]
    fn a_fresh_run_is_silent() {
        let c = CascadeState::default();
        assert_eq!(c.level, 0.0);
        assert_eq!(c.ring, 0);
    }
}

/// Hull materials are earned by going out, not by spending.
///
/// `is_hull_material_unlocked` (building/mod.rs) has always gated Titanium,
/// Composite and Abyssal Alloy behind strings in `Unlocks.hull_types` — and
/// nothing anywhere ever pushed those strings. The gate could not open. The
/// player was on Steel for the entire game while the factions at the edge fly
/// Abyssal Alloy, which is three times the hull health and five times the
/// absorption, on top of a difficulty multiplier.
///
/// Titanium and Composite now come with the Summit Record (steps 2 and 4,
/// narrative::summit), so going out *for* something is what earns them.
/// Abyssal Alloy stays on distance until the step that would carry it (6)
/// exists. `Unlocks` is deliberately not cleared by `reset_for_new_game` —
/// this is the one thing that carries between runs.
fn grant_hull_materials(
    cascade: Res<CascadeState>,
    mut unlocks: ResMut<crate::resources::Unlocks>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    const TIERS: [(f32, &str, &str); 1] = [(0.74, "abyssal_alloy", "Abyssal Alloy")];
    for (at, key, label) in TIERS {
        if cascade.reach < at {
            continue;
        }
        if unlocks.hull_types.iter().any(|h| h == key) {
            continue;
        }
        unlocks.hull_types.push(key.to_string());
        notifications.write(crate::events::ShowNotification {
            message: format!("{label} plating unlocked — salvaged from what is out here."),
            notification_type: crate::events::NotificationType::Success,
            duration: 6.0,
        });
    }
}

#[cfg(test)]
mod progression_tests {
    /// Mirrors the tiers in grant_hull_materials.
    const TIERS: [(f32, &str); 1] = [(0.74, "abyssal_alloy")];

    fn unlocked_at(reach: f32) -> Vec<&'static str> {
        TIERS.iter().filter(|(at, _)| reach >= *at).map(|(_, k)| *k).collect()
    }

    /// A fresh run starts on Steel and nothing else, or the ramp has no floor.
    #[test]
    fn nothing_is_granted_at_the_start() {
        assert!(unlocked_at(0.0).is_empty());
    }

    /// Every tier must actually become reachable, since the whole defect being
    /// fixed here is a gate that could never open.
    #[test]
    fn every_tier_is_reachable() {
        assert_eq!(unlocked_at(1.0).len(), TIERS.len());
    }

    /// Distance alone no longer hands out the early plating: that is the
    /// story's to give (narrative::summit), so the far edge is the only gate.
    #[test]
    fn distance_only_grants_the_last_tier() {
        assert!(unlocked_at(0.7).is_empty());
        assert_eq!(unlocked_at(0.9), vec!["abyssal_alloy"]);
    }

    /// The strings must match what building::is_hull_material_unlocked looks
    /// for. They are compared by literal, so a typo silently re-breaks the
    /// gate in exactly the way it was broken before.
    #[test]
    fn the_keys_match_the_gate() {
        for (_, key) in TIERS {
            assert!(matches!(key, "titanium" | "composite" | "abyssal_alloy"), "unknown key {key}");
        }
    }
}
