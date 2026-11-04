//! The Dark Ward: the one block that lets a ship into a system the Cascade
//! has taken ("Live Cascade plan", the user's addition).
//!
//! Without one aboard the warp drive refuses to jump into the dark. With one,
//! you can go in, but its draw climbs steeply with how deep you are (systems
//! in from the edge of the dark), how long you stay, and how big the ship is:
//! a small ship can linger at the edge, a big one deep in can only touch and
//! leave. When the power runs short the ward fails in stages: first every
//! readout lies, then the crew start to break, then the hull goes.
//!
//! The pattern comes from the Abbess with the second piece of the summit
//! record; before that the yard won't lay one.

use bevy::prelude::*;

use crate::celestial::resources::{GalaxyMap, SystemStreamingManager};
use crate::components::{CrewMember, HullSegment, Module, ModuleType, Ship};

use super::front::is_taken;

/// What a ward draws outside the dark.
pub const IDLE_DRAW: f32 = 10.0;
/// The summit record step that gives you the pattern.
pub const PATTERN_STEP: u8 = 2;
/// Seconds of a failing ward before the crew start to break, and before the
/// hull goes.
const CREW_BREAK_AFTER: f32 = 15.0;
const HULL_GOES_AFTER: f32 = 45.0;
/// What a failing ward costs per second: crew health, and a share of each
/// hull plate's full health.
const CREW_HURT_PER_SEC: f32 = 1.5;
const HULL_SHARE_PER_SEC: f32 = 0.01;

/// How many systems in from the edge of the dark a system is: 1 next to a
/// system still standing, more the further in. 0 if it isn't taken.
pub fn depth(galaxy: &GalaxyMap, id: u32) -> u32 {
    use crate::celestial::galaxy::nearest_neighbors;
    let taken = |id: u32| galaxy.systems.iter().find(|s| s.id == id).is_some_and(is_taken);
    if !taken(id) {
        return 0;
    }
    let neighbours = |id: u32| {
        let mut n = nearest_neighbors(galaxy, id, 3);
        n.extend(galaxy.systems.iter().filter(|s| nearest_neighbors(galaxy, s.id, 3).contains(&id)).map(|s| s.id));
        n
    };
    let mut seen = vec![id];
    let mut ring = vec![id];
    for d in 1..=galaxy.systems.len() as u32 {
        let mut next = Vec::new();
        for at in &ring {
            for n in neighbours(*at) {
                if !taken(n) {
                    return d;
                }
                if !seen.contains(&n) {
                    seen.push(n);
                    next.push(n);
                }
            }
        }
        if next.is_empty() {
            return d;
        }
        ring = next;
    }
    galaxy.systems.len() as u32
}

/// The ward's draw: 40 for a hundred-block ship one system in, doubling with
/// each system deeper and climbing 40% a minute inside.
pub fn draw(blocks: usize, depth: u32, minutes_inside: f32) -> f32 {
    40.0 * (blocks as f32 / 100.0) * 2f32.powi(depth as i32) * 1.4f32.powf(minutes_inside)
}

/// How the ward is doing, for the HUD, the warp drive and the failure.
#[derive(Resource, Default, Debug)]
pub struct WardState {
    /// A working ward is aboard.
    pub aboard: bool,
    /// Seconds spent in the dark this time.
    pub inside: f32,
    /// Seconds the ward has been failing.
    pub failing: f32,
    /// The draw it needs right now.
    pub need: f32,
    stage: u8,
    jolt_timer: f32,
}

/// Whether the warp drive will take you to `target`: anywhere standing, or
/// into the dark with a working ward aboard.
pub fn may_enter(galaxy: &GalaxyMap, target: u32, ward_aboard: bool) -> bool {
    ward_aboard || !galaxy.systems.iter().find(|s| s.id == target).is_some_and(is_taken)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_ward(
    time: Res<Time>,
    galaxy: Res<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
    power: Res<crate::resources::PowerState>,
    mut state: ResMut<WardState>,
    mut interference: ResMut<super::interference::Interference>,
    ship: Query<Entity, With<Ship>>,
    mut modules: Query<(&mut Module, &ChildOf)>,
    mut hull: Query<(&mut HullSegment, &ChildOf)>,
    mut crew: Query<&mut CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    let Ok(ship) = ship.single() else { return };
    let dt = time.delta_secs();
    let here = streaming.loaded_system.and_then(|id| galaxy.systems.iter().find(|s| s.id == id));
    let in_dark = here.is_some_and(is_taken);

    let blocks = modules.iter().filter(|(_, p)| p.parent() == ship).count() + hull.iter().filter(|(_, p)| p.parent() == ship).count();
    let wards = modules
        .iter()
        .filter(|(m, p)| p.parent() == ship && m.module_type == ModuleType::DarkWard && m.is_active && m.health > 0.0)
        .count();
    state.aboard = wards > 0;

    if !in_dark {
        state.inside = 0.0;
        state.failing = 0.0;
        state.stage = 0;
        state.need = 0.0;
        for (mut m, p) in &mut modules {
            if p.parent() == ship && m.module_type == ModuleType::DarkWard && m.power_consumption != IDLE_DRAW {
                m.power_consumption = IDLE_DRAW;
            }
        }
        return;
    }

    state.inside += dt;
    let deep = here.map_or(1, |s| depth(&galaxy, s.id));
    state.need = draw(blocks, deep, state.inside / 60.0);
    if wards > 0 {
        let each = state.need / wards as f32;
        for (mut m, p) in &mut modules {
            if p.parent() == ship && m.module_type == ModuleType::DarkWard {
                m.power_consumption = each;
            }
        }
    }

    let holding = wards > 0 && power.power_balance >= 0.0;
    if holding {
        state.failing = 0.0;
        state.stage = 0;
        // Even held, nothing in here reads quite true.
        state.jolt_timer += dt;
        if state.jolt_timer >= 7.0 {
            state.jolt_timer = 0.0;
            interference.jolt(0.8, 2);
        }
        return;
    }

    state.failing += dt;
    let stage = if state.failing >= HULL_GOES_AFTER { 3 } else if state.failing >= CREW_BREAK_AFTER { 2 } else { 1 };
    if stage > state.stage {
        state.stage = stage;
        let message = match stage {
            1 if wards == 0 => "No ward. Nothing reads true in here.",
            1 => "The ward is starving. Nothing reads true.",
            2 => "The crew are breaking. Get out of the dark.",
            _ => "The hull is coming apart. Get out now.",
        };
        notes.write(crate::events::ShowNotification {
            message: message.into(),
            notification_type: crate::events::NotificationType::Danger,
            duration: 6.0,
        });
    }
    interference.jolt(0.3, 3);
    if stage >= 2 {
        for mut c in &mut crew {
            c.health = (c.health - CREW_HURT_PER_SEC * dt).max(0.0);
        }
    }
    if stage >= 3 {
        for (mut seg, p) in &mut hull {
            if p.parent() == ship && seg.health > 0.0 {
                seg.health = (seg.health - seg.max_health * HULL_SHARE_PER_SEC * dt).max(0.0);
            }
        }
    }
}

/// The yard won't lay a ward until the Abbess has given you the pattern.
pub fn pattern_known(contacts: &crate::diplomacy::contacts::Contacts) -> bool {
    contacts.summit.done >= PATTERN_STEP
}

pub struct WardPlugin;

impl Plugin for WardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WardState>()
            .add_systems(Update, run_ward.run_if(in_state(crate::states::GameState::Exploring)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small and shallow is cheap; deeper, longer or bigger costs far more.
    #[test]
    fn the_draw_climbs_steeply() {
        let base = draw(100, 1, 0.0);
        assert!((base - 80.0).abs() < 0.01);
        assert!(draw(100, 2, 0.0) > base * 1.9);
        assert!(draw(100, 1, 5.0) > base * 5.0);
        assert!(draw(400, 1, 0.0) > base * 3.9);
    }

    /// The edge of the dark is depth 1; further in is deeper; a standing
    /// system is depth 0.
    #[test]
    fn depth_counts_in_from_the_edge() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        assert_eq!(depth(&galaxy, 5), 0);
        super::super::front::seed_origin(&mut galaxy);
        let origin = super::super::front::origin(&galaxy).unwrap();
        assert_eq!(depth(&galaxy, origin), 1);
        // Take everything but Haven: the far edge is now deep.
        let ids: Vec<u32> = galaxy.systems.iter().map(|s| s.id).collect();
        for id in ids {
            super::super::front::take(&mut galaxy, id);
        }
        assert!(depth(&galaxy, origin) > 1);
    }

    /// The warp drive takes you into the dark only with a ward.
    #[test]
    fn no_ward_no_entry() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        super::super::front::take(&mut galaxy, 5);
        assert!(!may_enter(&galaxy, 5, false));
        assert!(may_enter(&galaxy, 5, true));
        assert!(may_enter(&galaxy, 6, false));
    }
}
