//! A faction's starport as the hub of a city: the approved starport design
//! stays exactly as drawn, and the owner builds its arms out of it -- its
//! own style of arm, its own sections, its own and its friends' ships moored
//! along them. The city rides the planet with the starport (it carries the
//! same `StationAnchor`), and the starport's docking reach, berth, guard post
//! and convoy stop all grow to the city's size.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::prelude::*;

use super::raft::{plan_round, GivenHub, Mooring, RaftPlan, CELL};
use crate::ai_ship::components::AiShipType;

/// The city round a starport. Sits in `LiveStation::parts`, so it goes when
/// the starport does.
#[derive(Component, Clone, Copy, Debug)]
pub struct StarportRaft;

/// Ships moored round a starport when it's laid out.
const SHIPS: usize = 14;
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

/// How far the deck under the starport reaches past its hull, in blocks.
const DECK: i32 = 6;
/// A collar's length, where an arm meets the deck, in blocks.
const COLLAR: i32 = 16;

fn lay_out(seed: u64, owner: AiShipType, cells: &[IVec2]) -> RaftPlan {
    let look = super::station_style::look(Some(owner));
    let pool = super::raft::pool_for(Some(owner));
    let mut hull = GivenHub::from_cells(cells);
    let inside = Rect::from_center_half_size(Vec2::ZERO, hull.half.max(Vec2::ZERO));
    // Nothing moors where the deck and the collars on its rim are.
    hull.half += Vec2::splat((DECK + COLLAR / 2 + 2) as f32);
    let reach = cells.iter().map(|c| c.as_vec2().length()).fold(0.0, f32::max);
    let mut plan = plan_round(seed, SHIPS, &look, hull, hull.reach[0] + BERTH_ARM, &pool);
    plan.extent = plan.extent.max(reach * CELL + CELL);

    // The deck the starport stands on, over the arms' roots.
    let key = super::sections::deck(&format!("{owner:?}"), cells, DECK);
    let Some(deck) = super::sections::get(key) else { return plan };
    let on_deck: std::collections::HashSet<IVec2> = deck.cells.iter().map(|c| c.at).collect();
    let piece = |design: &'static str, at: Vec2, quarter: u8, level: f32| Mooring { design, at, quarter, level, street: None, slot: None, dock: Vec2::ZERO };
    let mut added = vec![piece(key, deck.centre() * CELL, 0, 0.55)];
    // A collar where each arm leaves it, over both.
    let worn = ((1.0 - look.order.clamp(0.0, 1.0)) * 3.0).round() as u8;
    for (i, (a, b)) in plan.streets.iter().enumerate() {
        let start = *a / CELL;
        if !inside.contains(start) {
            continue;
        }
        let dir = (*b - *a).normalize_or_zero().round().as_ivec2();
        let Some(arm) = plan.moorings.iter().find(|m| m.street == Some(i) && m.design.starts_with("arm/")) else { continue };
        let half: i32 = arm.design.split('/').nth(2).and_then(|h| h.parse().ok()).unwrap_or(6);
        let mut edge = start.round().as_ivec2();
        for _ in 0..600 {
            if !on_deck.contains(&(edge + dir)) {
                break;
            }
            edge += dir;
        }
        let collar = super::sections::arm_segment(COLLAR, half + 3, worn);
        added.push(piece(collar, edge.as_vec2() * CELL, arm.quarter, 0.6));
    }
    plan.moorings.splice(0..0, added);
    plan.circles = super::raft::collision_circles(&plan);
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
            assert!(ships >= 8, "{owner:?}: {ships} ships");
            // Nothing of the city's own where the hull is, but the arms.
            let hull_rect = Rect::from_center_half_size(Vec2::ZERO, ((hull.half - Vec2::splat(1.0)) * CELL).max(Vec2::ZERO));
            for m in plan.moorings.iter().chain(&plan.spares).filter(|m| !m.design.starts_with("arm/") && !m.design.starts_with("deck/")) {
                let r = super::super::raft::outline(m, 0.0).unwrap();
                assert!(r.intersect(hull_rect).is_empty(), "{owner:?}: {} under the hull", m.design);
            }
            // Joined: every piece reaches an arm that runs in under the hull.
            let pieces: Vec<Rect> = plan.moorings.iter().chain(&plan.spares).filter_map(|m| super::super::raft::outline(m, CELL * 0.6)).collect();
            let mut reached: Vec<bool> = pieces.iter().map(|p| !p.intersect(hull_rect).is_empty()).collect();
            // A sprawl may leave a side bare; the berth arm and one more at least.
            assert!(reached.iter().filter(|r| **r).count() >= 2, "{owner:?}: arms don't reach the hull");
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

    /// Maps of every starport city, as pictures, for looking at by eye:
    /// `CITY_MAPS=<dir> cargo test city_maps -- --ignored`.
    #[test]
    #[ignore]
    fn city_maps() {
        let Ok(dir) = std::env::var("CITY_MAPS") else { return };
        for owner in crate::world::station_style::OWNERS {
            let Some(sp) = station_plan(owner) else { continue };
            let cells = hull_cells(owner, sp.key);
            let plan = lay_out(seed_for(3, owner), owner, &cells);
            let size = 900usize;
            let scale = size as f32 / (plan.extent * 2.1);
            let mut px = vec![[8u8, 10, 16]; size * size];
            let mut fill = |r: Rect, c: [u8; 3]| {
                let to = |v: Vec2| ((v.x * scale + size as f32 / 2.0) as i32, (size as f32 / 2.0 - v.y * scale) as i32);
                let (x0, y1) = to(r.min);
                let (x1, y0) = to(r.max);
                for y in y0.max(0)..=y1.min(size as i32 - 1) {
                    for x in x0.max(0)..=x1.min(size as i32 - 1) {
                        px[y as usize * size + x as usize] = c;
                    }
                }
            };
            for m in plan.moorings.iter().filter(|m| m.design.starts_with("deck/")) {
                fill(super::super::raft::outline(m, 0.0).unwrap(), [70, 74, 86]);
            }
            for m in plan.moorings.iter().filter(|m| m.design.starts_with("arm/")) {
                fill(super::super::raft::outline(m, 0.0).unwrap(), [120, 125, 140]);
            }
            for c in &cells {
                let at = c.as_vec2() * CELL;
                fill(Rect::from_center_half_size(at, Vec2::splat(CELL * 0.5)), [40, 60, 130]);
            }
            for m in plan.moorings.iter().filter(|m| !m.design.starts_with("arm/") && !m.design.starts_with("deck/")) {
                let r = super::super::raft::outline(m, 0.0).unwrap();
                let c = if m.slot.is_some() { [90, 170, 230] } else if m.design.starts_with("gantry") { [150, 150, 150] } else { [200, 160, 90] };
                fill(r, c);
            }
            for m in &plan.spares {
                let r = super::super::raft::outline(m, 0.0).unwrap();
                fill(Rect::from_center_half_size(r.center(), r.half_size() * 0.3), [60, 80, 100]);
            }
            let mut out = format!("P6 {size} {size} 255\n").into_bytes();
            for p in px {
                out.extend_from_slice(&p);
            }
            std::fs::write(format!("{dir}/{owner:?}.ppm"), out).unwrap();
        }
    }
}
