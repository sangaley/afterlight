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
    if let Some(sys) = galaxy.systems.iter_mut().find(|s| s.id == id) {
        darken(sys, TAKEN);
        return true;
    }
    false
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
        if let Some(sys) = galaxy.systems.iter_mut().find(|s| s.id == id) {
            darken(sys, TAKEN);
        }
    }
    info!("FRONT: DEPTHS_DARK took {n} systems");
}

pub struct FrontPlugin;

impl Plugin for FrontPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dark_from_env, seed_front).chain());
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
            assert_ne!(gone.discovery, crate::celestial::resources::SystemDiscovery::Unknown, "the front can't be seen");
            // Seeding again does nothing: the front is already there.
            assert!(!seed_origin(&mut galaxy));
        }
    }
}
