//! The Cascade on the map: a dark front that takes the galaxy a system at a
//! time ("Live Cascade plan" in the plan doc).
//!
//! Each system carries how far the dark has it (`StarSystemDef.dark`): 0
//! untouched, between 0 and 1 reaching it, 1 taken. That number is saved with
//! the system. The front starts where the prologue left it, at the far edge:
//! a run begins with the deepest system, the Shepherd's, already gone.
//! Haven is never taken.

use bevy::prelude::*;

use crate::ai_ship::components::AiShipType;
use crate::celestial::resources::{GalaxyMap, StarSystemDef};

/// Darkness at which a system counts as taken.
pub const TAKEN: f32 = 1.0;

pub fn is_taken(sys: &StarSystemDef) -> bool {
    sys.dark >= TAKEN
}

/// Haven: the one system the dark never takes.
pub fn is_spared(sys: &StarSystemDef) -> bool {
    sys.id == 0
}

/// Where the front starts: the deepest system, the Shepherd's haunt, or the
/// furthest from Haven if nothing haunts the edge.
pub fn origin(galaxy: &GalaxyMap) -> Option<u32> {
    galaxy
        .systems
        .iter()
        .find(|s| s.haunt == Some(AiShipType::Shepherd))
        .or_else(|| galaxy.systems.iter().filter(|s| !is_spared(s)).max_by(|a, b| a.galaxy_pos.length().total_cmp(&b.galaxy_pos.length())))
        .map(|s| s.id)
}

/// A galaxy with no dark anywhere (a fresh run, or a save from before the
/// front existed) gets its origin taken. Returns whether it did anything.
pub fn seed_origin(galaxy: &mut GalaxyMap) -> bool {
    if galaxy.systems.iter().any(|s| s.dark > 0.0) {
        return false;
    }
    let Some(id) = origin(galaxy) else { return false };
    take(galaxy, id)
}

/// Take a system outright: dark, and nobody's.
pub fn take(galaxy: &mut GalaxyMap, id: u32) -> bool {
    let Some(sys) = galaxy.systems.iter_mut().find(|s| s.id == id) else { return false };
    if is_spared(sys) {
        return false;
    }
    darken(sys, TAKEN);
    go_dark(galaxy, id);
    true
}

/// Take a system (or darken it this far), and put it on the map: the front
/// is seen coming, even across space you've never been.
pub fn darken(sys: &mut StarSystemDef, to: f32) {
    if is_spared(sys) {
        return;
    }
    sys.dark = sys.dark.max(to.min(TAKEN));
    if sys.discovery == crate::celestial::resources::SystemDiscovery::Unknown {
        sys.discovery = crate::celestial::resources::SystemDiscovery::Located;
    }
}

/// What a system loses when the dark takes it: its owner, and the owner's
/// capital moves to their nearest system still standing. Returns who held
/// it, and whether it was their capital.
pub fn go_dark(galaxy: &mut GalaxyMap, id: u32) -> Option<(AiShipType, bool)> {
    let i = galaxy.systems.iter().position(|s| s.id == id)?;
    let (from, was_capital, pos) = (galaxy.systems[i].faction, galaxy.systems[i].capital, galaxy.systems[i].galaxy_pos);
    galaxy.systems[i].faction = None;
    galaxy.systems[i].capital = false;
    let loser = from?;
    if was_capital {
        let next = galaxy
            .systems
            .iter()
            .enumerate()
            .filter(|(_, s)| s.faction == Some(loser) && !is_taken(s))
            .min_by(|(_, a), (_, b)| a.galaxy_pos.distance(pos).total_cmp(&b.galaxy_pos.distance(pos)))
            .map(|(j, _)| j);
        if let Some(j) = next {
            galaxy.systems[j].capital = true;
        }
    }
    Some((loser, was_capital))
}

/// How many jumps a system is from the nearest one the dark has taken: 0 if
/// it's taken, None if nothing is.
pub fn hops_from_dark(galaxy: &GalaxyMap, id: u32) -> Option<u32> {
    use crate::celestial::galaxy::nearest_neighbors;
    let taken = |id: u32| galaxy.systems.iter().find(|s| s.id == id).is_some_and(is_taken);
    if taken(id) {
        return Some(0);
    }
    let neighbours = |id: u32| {
        let mut n = nearest_neighbors(galaxy, id, 3);
        n.extend(galaxy.systems.iter().filter(|s| nearest_neighbors(galaxy, s.id, 3).contains(&id)).map(|s| s.id));
        n
    };
    let (mut seen, mut ring) = (vec![id], vec![id]);
    for d in 1..=galaxy.systems.len() as u32 {
        let mut next = Vec::new();
        for at in &ring {
            for n in neighbours(*at) {
                if taken(n) {
                    return Some(d);
                }
                if !seen.contains(&n) {
                    seen.push(n);
                    next.push(n);
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        ring = next;
    }
    None
}

/// What the dark does to prices: a station next to it charges 40% more and
/// pays 20% less; one jump further, 20% and 10%.
pub fn front_prices(galaxy: &GalaxyMap, id: Option<u32>) -> (f32, f32) {
    match id.and_then(|id| hops_from_dark(galaxy, id)) {
        Some(1) => (1.4, 0.8),
        Some(2) => (1.2, 0.9),
        _ => (1.0, 1.0),
    }
}

/// How many systems the dark has taken.
pub fn taken_count(galaxy: &GalaxyMap) -> usize {
    galaxy.systems.iter().filter(|s| is_taken(s)).count()
}

fn seed_front(mut galaxy: ResMut<GalaxyMap>) {
    // Read through Deref first, so a quiet frame doesn't mark the map changed.
    if galaxy.systems.iter().any(|s| s.dark > 0.0) || galaxy.systems.is_empty() {
        return;
    }
    if seed_origin(&mut galaxy) {
        info!("FRONT: the dark begins at system {:?}", origin(&galaxy));
    }
}

/// Debug override, in the house style (`DEPTHS_SUMMIT`): `DEPTHS_DARK=5`
/// takes the five systems furthest from Haven at once, so the dark can be
/// looked at without waiting for it. Applied once per launch.
fn dark_from_env(mut galaxy: ResMut<GalaxyMap>, mut done: Local<bool>) {
    if *done || galaxy.systems.is_empty() {
        return;
    }
    *done = true;
    let Some(n) = std::env::var("DEPTHS_DARK").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let mut far: Vec<(u32, f32)> = galaxy.systems.iter().filter(|s| !is_spared(s)).map(|s| (s.id, s.galaxy_pos.length())).collect();
    far.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (id, _) in far.into_iter().take(n) {
        take(&mut galaxy, id);
    }
    info!("FRONT: DEPTHS_DARK took {n} systems");
}

/// How fast the dark moves: one system's worth per this many seconds of
/// flight, shared across the whole front (so a wider front doesn't move
/// faster, it moves less surely). Tuned so the first system after the edge
/// goes dark about half an hour into a demo.
pub const SECONDS_PER_SYSTEM: f32 = 1_800.0;
/// What one finished Summit Record step pushes it on by, in systems.
pub const STORY_LURCH: f32 = 0.5;

/// The systems the dark is next to: not taken, not Haven, and a neighbour
/// of something taken.
pub fn frontier(galaxy: &GalaxyMap) -> Vec<u32> {
    use crate::celestial::galaxy::nearest_neighbors;
    let taken: Vec<u32> = galaxy.systems.iter().filter(|s| is_taken(s)).map(|s| s.id).collect();
    galaxy
        .systems
        .iter()
        .filter(|s| !is_taken(s) && !is_spared(s))
        .filter(|s| {
            nearest_neighbors(galaxy, s.id, 3).iter().any(|n| taken.contains(n))
                || taken.iter().any(|t| nearest_neighbors(galaxy, *t, 3).contains(&s.id))
        })
        .map(|s| s.id)
        .collect()
}

/// How strongly the dark pulls toward a system. It comes inward toward
/// Haven, and it goes fastest where there are many ships and where you've
/// been: it feeds on connection (the Summit Record's fourth piece). Never
/// explained anywhere; there to be noticed.
pub fn pull(sys: &StarSystemDef, ships_there: usize) -> f32 {
    let radius = crate::celestial::galaxy::GALAXY_RADIUS.max(1.0);
    let inward = 1.0 + 1.5 * (1.0 - (sys.galaxy_pos.length() / radius).min(1.0));
    let crowd = (ships_there as f32 * 0.15).min(3.0);
    let yours = if sys.discovery == crate::celestial::resources::SystemDiscovery::Visited { 2.0 } else { 0.0 };
    (1.0 + crowd + yours) * inward
}

/// Share of the dark that pours into the most-pulled system at the front;
/// the rest dims the others. Spread evenly, three neighbours would all fall
/// at once after an hour and a half; this way one goes, then the next.
const FOCUS: f32 = 0.85;

/// Spread `systems` worth of dark across the front: most into the system
/// that pulls hardest, the rest by pull. Returns the systems this took.
pub fn spread(galaxy: &mut GalaxyMap, systems: f32, ships_in: impl Fn(u32) -> usize) -> Vec<u32> {
    let front = frontier(galaxy);
    let weights: Vec<(u32, f32)> = front
        .iter()
        .filter_map(|id| galaxy.systems.iter().find(|s| s.id == *id).map(|s| (*id, pull(s, ships_in(*id)))))
        .collect();
    let total: f32 = weights.iter().map(|(_, w)| w).sum();
    if total <= 0.0 || systems <= 0.0 {
        return Vec::new();
    }
    let focus = weights.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| *id);
    let mut took = Vec::new();
    for (id, w) in weights {
        let share = if Some(id) == focus { FOCUS } else { 0.0 } + (1.0 - FOCUS) * w / total;
        let Some(sys) = galaxy.systems.iter_mut().find(|s| s.id == id) else { continue };
        let before = is_taken(sys);
        darken(sys, sys.dark + systems * share);
        if !before && is_taken(sys) {
            took.push(id);
        }
    }
    took
}

/// The dark moves while you fly: never while you're docked or building
/// (other states), never while a card is up to read, and never while the map
/// is open (it pauses game time).
#[allow(clippy::too_many_arguments)]
fn advance_front(
    time: Res<Time>,
    mut galaxy: ResMut<GalaxyMap>,
    mut sim: ResMut<crate::ai_ship::components::WorldSimulation>,
    mut contacts: ResMut<crate::diplomacy::contacts::Contacts>,
    mut standing: ResMut<crate::diplomacy::FactionStanding>,
    queue: Res<super::reader::LogQueue>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut steps_seen: Local<Option<u8>>,
) {
    if galaxy.systems.is_empty() {
        return;
    }
    // A finished story step pushes it on. The first frame only notes where
    // the chain stands, so loading a save doesn't count old steps again.
    let done = contacts.summit.done;
    let lurch = match *steps_seen {
        Some(seen) if done > seen => (done - seen) as f32 * STORY_LURCH,
        _ => 0.0,
    };
    *steps_seen = Some(done);
    let reading = !queue.pending.is_empty();
    let creep = if reading { 0.0 } else { time.delta_secs() / SECONDS_PER_SYSTEM * speed() };
    if creep + lurch <= 0.0 {
        return;
    }
    let ships_in = |id: u32| sim.ships.iter().filter(|s| s.system_id == id && s.behavior != crate::ai_ship::components::SimBehavior::Dead).count();
    let took = spread(&mut galaxy, creep + lurch, ships_in);
    for id in took {
        let name = galaxy.systems.iter().find(|s| s.id == id).map_or(String::new(), |s| s.name.clone());
        info!("FRONT: {name} taken ({} gone)", taken_count(&galaxy));
        notes.write(crate::events::ShowNotification {
            message: format!("The dark has taken {name}. Nothing there answers."),
            notification_type: crate::events::NotificationType::Danger,
            duration: 8.0,
        });
        // Everyone there stops where they are.
        for ship in sim.ships.iter_mut().filter(|s| s.system_id == id) {
            ship.behavior = crate::ai_ship::components::SimBehavior::Dead;
        }
        // Its holder loses it; if it was their capital, they move, and some
        // of their people don't get out.
        if let Some((loser, true)) = go_dark(&mut galaxy, id) {
            standing.break_alliance(loser, format!("With {name} gone, they break every pact"));
            for who in crate::diplomacy::contacts::lose_in_the_fall(&mut contacts, loser, &mut rand::thread_rng()) {
                notes.write(crate::events::ShowNotification {
                    message: format!("{who} did not make it out of {name}."),
                    notification_type: crate::events::NotificationType::Warning,
                    duration: 6.0,
                });
            }
        }
    }
}

/// How often the factions on the front push away from it, in seconds of
/// flight, and how hard.
const FLEE_EVERY: f32 = 300.0;
const FLEE_PUSH: f32 = 8.0;

/// Factions with ground next to the dark push into their hostile neighbours
/// further from it, to make room: borders move faster near the front.
fn war_away_from_the_dark(
    time: Res<Time>,
    mut galaxy: ResMut<GalaxyMap>,
    mut war: ResMut<crate::celestial::territory::BorderWar>,
    mut flips: MessageWriter<crate::celestial::territory::SystemChangedHands>,
    mut clock: Local<f32>,
) {
    *clock += time.delta_secs();
    if *clock < FLEE_EVERY {
        return;
    }
    *clock = 0.0;
    let mut moves: Vec<(u32, AiShipType)> = Vec::new();
    for sys in galaxy.systems.iter().filter(|s| hops_from_dark(&galaxy, s.id) == Some(1)) {
        let Some(owner) = sys.faction else { continue };
        let here = hops_from_dark(&galaxy, sys.id).unwrap_or(0);
        let target = crate::celestial::galaxy::nearest_neighbors(&galaxy, sys.id, 3)
            .into_iter()
            .filter_map(|n| galaxy.systems.iter().find(|s| s.id == n))
            .filter(|n| n.faction.is_some_and(|f| crate::ai_ship::components::factions_hostile(owner, f)))
            .filter(|n| hops_from_dark(&galaxy, n.id).is_none_or(|d| d > here))
            .map(|n| n.id)
            .next();
        if let Some(t) = target {
            moves.push((t, owner));
        }
    }
    for (target, by) in moves {
        info!("FRONT: {by:?} pushes into {target}, away from the dark");
        for flip in war.push(&mut galaxy, target, by, FLEE_PUSH) {
            flips.write(flip);
        }
    }
}

/// Nobody docks in the dark: a station there is shut, whoever built it.
fn refuse_dark_docking(
    galaxy: Res<GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut next: ResMut<NextState<crate::states::GameState>>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    let dark = streaming.loaded_system.and_then(|id| galaxy.systems.iter().find(|s| s.id == id)).is_some_and(is_taken);
    if dark {
        next.set(crate::states::GameState::Exploring);
        notes.write(crate::events::ShowNotification {
            message: "The station is dark. Nothing answers the docking call.".into(),
            notification_type: crate::events::NotificationType::Warning,
            duration: 5.0,
        });
    }
}

/// `DEPTHS_DARK_SPEED=20` runs the front twenty times faster, to watch it.
fn speed() -> f32 {
    static SPEED: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SPEED.get_or_init(|| std::env::var("DEPTHS_DARK_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0))
}

pub struct FrontPlugin;

impl Plugin for FrontPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dark_from_env, seed_front).chain()).add_systems(
            Update,
            (advance_front.after(seed_front), war_away_from_the_dark).run_if(in_state(crate::states::GameState::Exploring)),
        )
        .add_systems(OnEnter(crate::states::GameState::Docked), refuse_dark_docking);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh galaxy starts with the deepest system taken, and nothing else;
    /// Haven is never the origin.
    #[test]
    fn a_run_begins_with_the_edge_gone() {
        for seed in [1u64, 42, 777] {
            let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(seed);
            assert_eq!(taken_count(&galaxy), 0);
            assert!(seed_origin(&mut galaxy));
            assert_eq!(taken_count(&galaxy), 1);
            let gone = galaxy.systems.iter().find(|s| is_taken(s)).unwrap();
            assert!(!is_spared(gone), "seed {seed}: the dark started at Haven");
            assert_eq!(gone.haunt, Some(AiShipType::Shepherd), "seed {seed}: not the far edge");
            assert_eq!(gone.faction, None, "seed {seed}: the origin still has an owner");
            assert_ne!(gone.discovery, crate::celestial::resources::SystemDiscovery::Unknown, "the front can't be seen");
            // Seeding again does nothing: the front is already there.
            assert!(!seed_origin(&mut galaxy));
        }
    }

    /// Half an hour of flight takes about one system, never Haven, and
    /// the dark goes where it's pulled.
    #[test]
    fn it_spreads_by_the_clock_and_never_takes_haven() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        seed_origin(&mut galaxy);
        assert!(!frontier(&galaxy).is_empty());
        // Thirty minutes in one-second steps.
        let mut took = 0;
        for _ in 0..SECONDS_PER_SYSTEM as usize {
            took += spread(&mut galaxy, 1.0 / SECONDS_PER_SYSTEM, |_| 0).len();
        }
        assert!(took <= 1, "half an hour took {took} systems");
        // Another quarter hour and the first one has gone.
        for _ in 0..(SECONDS_PER_SYSTEM / 2.0) as usize {
            took += spread(&mut galaxy, 1.0 / SECONDS_PER_SYSTEM, |_| 0).len();
        }
        assert_eq!(took, 1, "forty-five minutes took {took} systems");
        // Run it far past the end of the galaxy: Haven stands.
        spread(&mut galaxy, 100.0, |_| 0);
        for _ in 0..200 {
            spread(&mut galaxy, 1.0, |_| 0);
        }
        let haven = galaxy.systems.iter().find(|s| s.id == 0).unwrap();
        assert_eq!(haven.dark, 0.0);
        assert_eq!(taken_count(&galaxy), galaxy.systems.len() - 1, "the dark stalled short of everything else");
    }

    /// A taken capital moves to its holder's nearest standing system.
    #[test]
    fn a_taken_capital_moves() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let capital = crate::celestial::territory::capital_of(&galaxy, AiShipType::BrokenChoir).unwrap();
        galaxy.systems.iter_mut().find(|s| s.id == capital).unwrap().dark = TAKEN;
        assert_eq!(go_dark(&mut galaxy, capital), Some((AiShipType::BrokenChoir, true)));
        let moved = crate::celestial::territory::capital_of(&galaxy, AiShipType::BrokenChoir);
        assert!(moved.is_some_and(|m| m != capital), "the Choir have no capital");
        let gone = galaxy.systems.iter().find(|s| s.id == capital).unwrap();
        assert_eq!(gone.faction, None);
    }

    /// Docking in a taken system bounces you straight back out.
    #[test]
    fn nobody_docks_in_the_dark() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        take(&mut galaxy, 5);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::events::ShowNotification>()
            .insert_resource(galaxy)
            .insert_resource(crate::celestial::resources::SystemStreamingManager { loaded_system: Some(5), ..default() })
            .init_resource::<NextState<crate::states::GameState>>()
            .add_systems(Update, refuse_dark_docking);
        app.update();
        let next = app.world().resource::<NextState<crate::states::GameState>>();
        assert!(matches!(next, NextState::Pending(crate::states::GameState::Exploring)), "{next:?}");
    }

    /// Prices climb next to the dark and ease further off.
    #[test]
    fn prices_climb_near_the_dark() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        assert_eq!(front_prices(&galaxy, Some(5)), (1.0, 1.0), "no dark, no markup");
        seed_origin(&mut galaxy);
        let origin = origin(&galaxy).unwrap();
        let next = frontier(&galaxy)[0];
        assert_eq!(hops_from_dark(&galaxy, origin), Some(0));
        assert_eq!(hops_from_dark(&galaxy, next), Some(1));
        assert!(front_prices(&galaxy, Some(next)).0 > 1.0);
        assert!(front_prices(&galaxy, Some(next)).1 < 1.0);
    }

    /// Crowds and your own route pull it; nearer Haven pulls it too.
    #[test]
    fn crowds_and_your_route_pull_it() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let mut sys = galaxy.systems[5].clone();
        let quiet = pull(&sys, 0);
        assert!(pull(&sys, 10) > quiet);
        sys.discovery = crate::celestial::resources::SystemDiscovery::Visited;
        assert!(pull(&sys, 0) > quiet);
    }
}
