//! A faction's starport as the hub of a city: the approved starport design
//! stays exactly as drawn, and the owner builds its arms out of it -- its
//! own style of arm, its own sections, its own and its friends' ships moored
//! along them. The city rides the planet with the starport (it carries the
//! same `StationAnchor`), and the starport's docking reach, berth, guard post
//! and convoy stop all grow to the city's size.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::prelude::*;

use super::raft::{plan_round, GivenHub, RaftPlan, CELL};
use crate::ai_ship::components::AiShipType;

/// The city round a starport. Sits in `LiveStation::parts`, so it goes when
/// the starport does.
#[derive(Component, Clone, Copy, Debug)]
pub struct StarportRaft;

/// Ships moored round a starport when it's laid out.
const SHIPS: usize = 22;
/// How far the berth arm runs past the starport's hull, in blocks.
const BERTH_ARM: f32 = 110.0;

/// The starport's hull as built, as cells about its middle.
fn hull_cells(owner: AiShipType, key: &'static str) -> Vec<IVec2> {
    crate::ai_ship::spawner::built_design(owner, Some(key)).hull_cells.iter().map(|c| c.grid_pos).collect()
}

fn seed_for(system_id: u32, owner: AiShipType) -> u64 {
    (system_id as u64 ^ 0x5157_A270).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ owner as u64
}

/// The city laid out round `owner`'s starport (design `key`) in a system.
/// The same every time for the same system, so it's worked out once.
pub fn plan(system_id: u32, owner: AiShipType, key: &'static str) -> Arc<RaftPlan> {
    static PLANS: OnceLock<Mutex<HashMap<(u32, AiShipType), Arc<RaftPlan>>>> = OnceLock::new();
    let plans = PLANS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(p) = plans.lock().unwrap().get(&(system_id, owner)) {
        return p.clone();
    }
    let p = Arc::new(lay_out(seed_for(system_id, owner), owner, &hull_cells(owner, key)));
    plans.lock().unwrap().insert((system_id, owner), p.clone());
    p
}

fn lay_out(seed: u64, owner: AiShipType, cells: &[IVec2]) -> RaftPlan {
    let look = super::station_style::look(Some(owner));
    let pool = super::raft::pool_for(Some(owner));
    let hull = GivenHub::from_cells(cells);
    let reach = cells.iter().map(|c| c.as_vec2().length()).fold(0.0, f32::max);
    let mut plan = plan_round(seed, SHIPS, &look, hull, hull.reach[0] + BERTH_ARM, &pool);
    plan.extent = plan.extent.max(reach * CELL + CELL);
    plan
}

/// Build the city round a starport at `at`. `anchor` is the starport's
/// own, so the city moves with it.
pub fn spawn(
    commands: &mut Commands,
    system_id: u32,
    owner: AiShipType,
    plan: &RaftPlan,
    at: Vec2,
    anchor: crate::diplomacy::stations::StationAnchor,
    art: &super::raft_art::RaftArt,
    bakes: &mut super::raft_art::RaftBakes,
) -> Entity {
    let look = super::station_style::look(Some(owner));
    let pool = super::raft::pool_for(Some(owner));
    let seed = seed_for(system_id, owner);
    let live = super::raft_life::RaftLive::for_owner(plan.clone(), seed, None, look.theme, pool, false);
    let plan = live.plan.clone();
    let root = commands
        .spawn((
            // Under the starport's own hull (its blocks sit at 0.15).
            Transform::from_xyz(at.x, at.y, 0.0),
            Visibility::default(),
            StarportRaft,
            anchor,
            crate::ship::collision::Collider::circles(plan.circles.clone(), f32::INFINITY),
            super::home_base::StationCircles(plan.circles.clone()),
            live,
        ))
        .id();
    super::raft_art::spawn_raft(commands, root, &plan, art, bakes, seed, look.theme);
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diplomacy::stations::station_plan;

    /// Every starport's city roots on its hull, holds together, and is a
    /// city: arms, sections and ships, reaching well past the hull.
    #[test]
    fn every_starport_grows_a_city_that_holds_together() {
        for owner in crate::world::station_style::OWNERS {
            let Some(sp) = station_plan(owner) else { continue };
            let cells = hull_cells(owner, sp.key);
            assert!(!cells.is_empty(), "{owner:?}: no hull");
            let plan = lay_out(7, owner, &cells);
            let hull = GivenHub::from_cells(&cells);
            let reach = cells.iter().map(|c| c.as_vec2().length()).fold(0.0, f32::max) * CELL;
            assert!(plan.extent > reach + 4_000.0, "{owner:?}: {} round a {reach} hull", plan.extent);
            let ships = plan.moorings.iter().filter(|m| m.slot.is_some()).count();
            assert!(ships >= 12, "{owner:?}: {ships} ships");
            // Nothing of the city's own where the hull is, but the arms.
            let hull_rect = Rect::from_center_half_size(Vec2::ZERO, (hull.half - Vec2::splat(1.0)) * CELL);
            for m in plan.moorings.iter().chain(&plan.spares).filter(|m| !m.design.starts_with("arm/")) {
                let r = super::super::raft::outline(m, 0.0).unwrap();
                assert!(r.intersect(hull_rect).is_empty(), "{owner:?}: {} under the hull", m.design);
            }
            // Joined: every piece reaches an arm that runs in under the hull.
            let pieces: Vec<Rect> = plan.moorings.iter().chain(&plan.spares).filter_map(|m| super::super::raft::outline(m, CELL * 0.6)).collect();
            let mut reached: Vec<bool> = pieces.iter().map(|p| !p.intersect(hull_rect).is_empty()).collect();
            assert!(reached.iter().filter(|r| **r).count() >= 4, "{owner:?}: arms don't reach the hull");
            let mut frontier: Vec<usize> = (0..pieces.len()).filter(|i| reached[*i]).collect();
            while let Some(i) = frontier.pop() {
                for j in 0..pieces.len() {
                    if !reached[j] && !pieces[i].intersect(pieces[j]).is_empty() {
                        reached[j] = true;
                        frontier.push(j);
                    }
                }
            }
            let loose: Vec<&str> = plan.moorings.iter().chain(&plan.spares).zip(&reached).filter(|(_, r)| !**r).map(|(m, _)| m.design).collect();
            assert!(loose.is_empty(), "{owner:?}: hanging: {loose:?}");
        }
    }

    #[test]
    fn a_starport_city_is_the_same_each_visit() {
        let a = plan(42, AiShipType::GildedThrone, "exchange");
        let b = lay_out(seed_for(42, AiShipType::GildedThrone), AiShipType::GildedThrone, &hull_cells(AiShipType::GildedThrone, "exchange"));
        assert_eq!(a.moorings, b.moorings);
    }
}
