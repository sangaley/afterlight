//! What you know of the dark, against what's true ("Fog of war plan" in the
//! plan doc).
//!
//! The map used to tell the truth: a system the dark took went dark on your
//! map that moment, across space you'd never flown. Now each system keeps
//! what you last heard of it, and the map draws that. News comes in a few
//! minutes of flight for every jump between you and it, at once next door,
//! all of it within a few jumps at a station's bar, and at the last moment
//! from a warp drive that won't go where you thought you could. Until then a
//! taken system still shows who held it.
//!
//! Then the map forgets. Some while after you heard a system was taken, you
//! no longer remember what was in it; later still, not even its name. Only
//! the map forgets: the systems you visited still count as visited, or the
//! story (which rides on them) would run backwards.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ai_ship::components::AiShipType;
use crate::celestial::resources::{GalaxyMap, StarSystemDef, SystemDiscovery, SystemStreamingManager};

use super::front::is_taken;

/// Seconds of flight per jump between you and a system for its news to
/// reach you. Next door is at once.
pub const NEWS_PER_JUMP: f64 = 180.0;

/// How many jumps out a station's bar has heard of.
pub const BAR_JUMPS: u32 = 3;

/// After hearing a system was taken: how long until you no longer remember
/// what was in it, and how long until its name goes too.
pub const FORGET_INSIDES: f64 = 20.0 * 60.0;
pub const FORGET_NAME: f64 = 80.0 * 60.0;

/// What you last heard of a system. Saved with it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Heard {
    /// How dark it was, as you last heard.
    pub dark: f32,
    /// When that news came (the galaxy clock, `GalaxyState::galaxy_time`).
    pub at: f64,
    /// When you heard it had been taken: the map forgets from here.
    pub taken_at: Option<f64>,
    /// Who held it before the dark did, which the map shows until you hear.
    pub owner_before: Option<AiShipType>,
    pub capital_before: bool,
}

/// Bring what you know of `sys` up to the truth. Returns true when this is
/// the news that it was taken.
pub fn hear(sys: &mut StarSystemDef, now: f64) -> bool {
    sys.heard.dark = sys.dark;
    sys.heard.at = now;
    if is_taken(sys) && sys.heard.taken_at.is_none() {
        sys.heard.taken_at = Some(now);
        if sys.discovery == SystemDiscovery::Unknown {
            sys.discovery = SystemDiscovery::Located;
        }
        return true;
    }
    false
}

/// A system as you know it: how dark you last heard it was, and, if you
/// haven't heard it fell, still whoever held it.
pub fn as_known(sys: &StarSystemDef) -> StarSystemDef {
    let mut known = sys.clone();
    known.dark = sys.heard.dark;
    if is_taken(sys) && sys.heard.taken_at.is_none() {
        known.faction = sys.heard.owner_before;
        known.capital = sys.heard.capital_before;
    }
    known
}

/// The whole galaxy as you know it, for the map.
pub fn known_map(galaxy: &GalaxyMap) -> GalaxyMap {
    GalaxyMap { systems: galaxy.systems.iter().map(as_known).collect(), galaxy_seed: galaxy.galaxy_seed }
}

/// How much of a taken system the map has let go of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Forgot {
    Nothing,
    /// What was in it: stations, planets, who was there.
    Insides,
    /// And its name.
    Name,
}

pub fn forgotten(sys: &StarSystemDef, now: f64) -> Forgot {
    match sys.heard.taken_at {
        Some(t) if now - t >= FORGET_NAME => Forgot::Name,
        Some(t) if now - t >= FORGET_INSIDES => Forgot::Insides,
        _ => Forgot::Nothing,
    }
}

/// Jumps from `from` to every system, along the same lanes the front spreads
/// by (each system's three nearest, both ways).
pub fn jumps_from(galaxy: &GalaxyMap, from: u32) -> std::collections::HashMap<u32, u32> {
    use crate::celestial::galaxy::nearest_neighbors;
    let mut lanes: std::collections::HashMap<u32, Vec<u32>> = Default::default();
    for s in &galaxy.systems {
        for n in nearest_neighbors(galaxy, s.id, 3) {
            lanes.entry(s.id).or_default().push(n);
            lanes.entry(n).or_default().push(s.id);
        }
    }
    let mut out = std::collections::HashMap::from([(from, 0u32)]);
    let mut ring = vec![from];
    while !ring.is_empty() {
        let mut next = Vec::new();
        for at in ring {
            let d = out[&at];
            for n in lanes.get(&at).into_iter().flatten() {
                if !out.contains_key(n) {
                    out.insert(*n, d + 1);
                    next.push(*n);
                }
            }
        }
        ring = next;
    }
    out
}

/// How long news takes to come `jumps` jumps.
pub fn news_delay(jumps: u32) -> f64 {
    jumps.saturating_sub(1) as f64 * NEWS_PER_JUMP
}

/// How far your sensors reach near the dark, as a share of their full reach:
/// half next to it, a quarter inside.
pub fn sensor_reach(hops_from_dark: Option<u32>) -> f32 {
    match hops_from_dark {
        Some(0) => 0.25,
        Some(1) => 0.5,
        _ => 1.0,
    }
}

/// Systems to hear about at once, whatever the distance: one the warp drive
/// just refused to jump into, say.
#[derive(Resource, Default)]
pub struct Overheard(pub Vec<u32>);

/// Where you are, for news: the system you're in, or the nearest to you in
/// open space.
fn where_you_are(galaxy: &GalaxyMap, streaming: &SystemStreamingManager) -> Option<u32> {
    streaming.loaded_system.or_else(|| {
        galaxy
            .systems
            .iter()
            .min_by(|a, b| a.galaxy_pos.distance(streaming.current_galaxy_pos).total_cmp(&b.galaxy_pos.distance(streaming.current_galaxy_pos)))
            .map(|s| s.id)
    })
}

fn tell(notes: &mut MessageWriter<crate::events::ShowNotification>, name: &str, jumps: u32) {
    let message = if jumps <= 1 {
        format!("The dark has taken {name}. Nothing there answers.")
    } else {
        format!("Word from {name}, {jumps} jumps off: the dark has taken it.")
    };
    notes.write(crate::events::ShowNotification { message, notification_type: crate::events::NotificationType::Danger, duration: 8.0 });
}

/// News comes in, a few minutes per jump.
fn hear_news(
    mut galaxy: ResMut<GalaxyMap>,
    state: Res<crate::celestial::resources::GalaxyState>,
    streaming: Res<SystemStreamingManager>,
    mut overheard: ResMut<Overheard>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut every: Local<f32>,
    time: Res<Time>,
) {
    *every += time.delta_secs();
    if *every < 1.0 && overheard.0.is_empty() {
        return;
    }
    *every = 0.0;
    let now = state.galaxy_time;
    let Some(here) = where_you_are(&galaxy, &streaming) else { return };
    let jumps = jumps_from(&galaxy, here);
    let forced = std::mem::take(&mut overheard.0);
    let mut told = Vec::new();
    for sys in galaxy.systems.iter_mut() {
        let j = jumps.get(&sys.id).copied().unwrap_or(u32::MAX);
        let due = forced.contains(&sys.id) || now - sys.heard.at >= news_delay(j);
        // Read before writing, so a quiet tick doesn't mark the map changed.
        if due && (sys.heard.dark != sys.dark || (is_taken(sys) && sys.heard.taken_at.is_none())) && hear(sys, now) {
            told.push((sys.name.clone(), j));
        } else if due {
            sys.heard.at = now;
        }
    }
    for (name, j) in told {
        info!("NEWS: {name} taken, heard from {j} jumps");
        tell(&mut notes, &name, j);
    }
}

/// A station's bar has heard everything within a few jumps.
fn hear_at_the_bar(
    mut galaxy: ResMut<GalaxyMap>,
    state: Res<crate::celestial::resources::GalaxyState>,
    streaming: Res<SystemStreamingManager>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    let Some(here) = where_you_are(&galaxy, &streaming) else { return };
    let jumps = jumps_from(&galaxy, here);
    let now = state.galaxy_time;
    let mut told = Vec::new();
    for sys in galaxy.systems.iter_mut().filter(|s| jumps.get(&s.id).is_some_and(|j| *j <= BAR_JUMPS)) {
        if hear(sys, now) {
            told.push(sys.name.clone());
        }
    }
    for name in told {
        notes.write(crate::events::ShowNotification {
            message: format!("At the bar they're saying {name} is gone. The dark has it."),
            notification_type: crate::events::NotificationType::Danger,
            duration: 8.0,
        });
    }
}

pub struct NewsPlugin;

impl Plugin for NewsPlugin {
    fn build(&self, app: &mut App) {
        use crate::states::GameState;
        app.init_resource::<Overheard>()
            .add_systems(Update, hear_news.run_if(in_state(GameState::Exploring)))
            .add_systems(OnEnter(GameState::StationDocked), hear_at_the_bar)
            .add_systems(OnEnter(GameState::Docked), hear_at_the_bar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn galaxy() -> GalaxyMap {
        crate::celestial::galaxy::generate_galaxy_map(42)
    }

    /// Next door is news at once; each jump further costs a few minutes.
    #[test]
    fn news_takes_longer_from_further_off() {
        assert_eq!(news_delay(0), 0.0);
        assert_eq!(news_delay(1), 0.0);
        assert_eq!(news_delay(2), NEWS_PER_JUMP);
        assert!(news_delay(4) > news_delay(3));
    }

    /// Every system can be reached, and you're no jumps from yourself.
    #[test]
    fn every_system_is_some_jumps_away() {
        let g = galaxy();
        let j = jumps_from(&g, 0);
        assert_eq!(j[&0], 0);
        assert_eq!(j.len(), g.systems.len());
        assert!(j.values().any(|d| *d >= 3));
    }

    /// Before you hear, a taken system shows who held it and no dark; after,
    /// the truth.
    #[test]
    fn the_map_shows_what_you_heard() {
        let mut g = galaxy();
        let id = g.systems.iter().find(|s| s.faction.is_some() && s.id != 0).unwrap().id;
        let owner = g.systems.iter().find(|s| s.id == id).unwrap().faction;
        super::super::front::take(&mut g, id);
        let sys = g.systems.iter().find(|s| s.id == id).unwrap().clone();
        assert!(is_taken(&sys));
        let before = as_known(&sys);
        assert!(!is_taken(&before), "unheard, the map still shows it standing");
        assert_eq!(before.faction, owner);
        let mut heard = sys.clone();
        assert!(hear(&mut heard, 100.0));
        let after = as_known(&heard);
        assert!(is_taken(&after));
        assert_eq!(after.faction, None);
        assert!(!hear(&mut heard, 200.0), "it's news once");
    }

    /// The insides go first, then the name; nothing before you've heard.
    #[test]
    fn the_map_forgets_in_order() {
        let mut sys = galaxy().systems[5].clone();
        assert_eq!(forgotten(&sys, 1.0e9), Forgot::Nothing);
        sys.heard.taken_at = Some(100.0);
        assert_eq!(forgotten(&sys, 100.0 + FORGET_INSIDES - 1.0), Forgot::Nothing);
        assert_eq!(forgotten(&sys, 100.0 + FORGET_INSIDES), Forgot::Insides);
        assert_eq!(forgotten(&sys, 100.0 + FORGET_NAME), Forgot::Name);
    }

    /// Forgetting never touches what you've visited: the story counts it.
    #[test]
    fn forgetting_leaves_visited_alone() {
        let mut sys = galaxy().systems[5].clone();
        sys.discovery = SystemDiscovery::Visited;
        sys.dark = 1.0;
        hear(&mut sys, 0.0);
        let _ = forgotten(&sys, 1.0e6);
        assert_eq!(sys.discovery, SystemDiscovery::Visited);
    }

    /// Sensors see less nearer the dark.
    #[test]
    fn sensors_shrink_near_the_dark() {
        assert_eq!(sensor_reach(None), 1.0);
        assert_eq!(sensor_reach(Some(3)), 1.0);
        assert!(sensor_reach(Some(1)) < 1.0);
        assert!(sensor_reach(Some(0)) < sensor_reach(Some(1)));
    }
}
