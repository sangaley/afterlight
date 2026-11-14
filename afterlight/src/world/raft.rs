//! Haven as the Raft: a city of ships moored round an old hulk.
//!
//! Every hull in it is one of the game's own ship designs (designs/factions),
//! at the ships' own scale, moored side-on along gantry streets that run out
//! from the anchor, and glued to their neighbours with walkways -- the way the
//! ISS is modules joined by trusses, or Snow Crash's Raft is boats lashed round
//! a carrier. `plan_raft` lays one out from a seed as plain data, so it can be
//! tested; world::raft_art bakes each design into a painted picture and draws
//! the plan.
//!
//! The street due east is fixed: its end is the berth tip the player docks
//! at, `HAVEN_RADIUS` east of the centre, and the lane past it is kept clear.

use std::collections::HashMap;
use std::sync::OnceLock;

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::components::{HullLayer, ModuleType};

/// One block, in world units.
pub const CELL: f32 = 66.0;

/// The hulk at the middle: an old throne barge.
pub const ANCHOR: &str = "gilded_throne/throne_barge";

/// Big hulls lashed straight onto the anchor: (design, cell offset, quarter turns).
const LASHED: [(&str, IVec2, u8); 3] = [
    ("merchant_princes/bulk_freighter", IVec2::new(-5, 82), 0),
    ("merchant_princes/bulk_freighter", IVec2::new(10, -82), 2),
    ("stellar_preserve/seed_ark", IVec2::new(-150, 0), 1),
];

/// What moors along the streets: working ships of every faction.
pub const MOORED: &[&str] = &[
    "terran_hegemony/hauler",
    "merchant_princes/tanker",
    "merchant_princes/hauler",
    "free_haulers_compact/fuel_runner",
    "free_haulers_compact/ore_hauler",
    "free_haulers_compact/tug",
    "merchant_princes/pedlar",
    "gilded_throne/courier",
    "shepherd/lamb",
    "merchant_princes/mining_barge",
    "terran_hegemony/homeward",
    "corpse_stars/mourner",
    "synthesis_collective/bloom",
    "the_silence/hush",
    "broken_choir/shard",
    "gilded_throne/dividend",
];

/// How a block of a design is drawn: the sprite it's painted from, and
/// whether it's a lit room.
#[derive(Clone, Debug)]
pub struct DesignCell {
    pub at: IVec2,
    pub sprite: &'static str,
    /// Which part of a multi-block sprite this cell shows: (column, row, of w, of h).
    pub part: (u8, u8, u8, u8),
    pub module: bool,
    pub lit: bool,
    pub outer: bool,
}

#[derive(Clone, Debug)]
pub struct Design {
    pub cells: Vec<DesignCell>,
    pub min: IVec2,
    pub max: IVec2,
}

impl Design {
    /// Its middle, in cells from its root.
    pub fn centre(&self) -> Vec2 {
        (self.min + self.max).as_vec2() * 0.5
    }
    /// Its size in cells, along x and y.
    pub fn span(&self) -> IVec2 {
        self.max - self.min + IVec2::ONE
    }
}

/// Rooms that show lights at night.
fn lit_room(t: ModuleType) -> bool {
    let n = format!("{t:?}").to_lowercase();
    ["quarters", "bridge", "navigation", "research", "medical", "lifesupport", "cargo", "repair", "oxygen", "hab", "storage"]
        .iter()
        .any(|k| n.contains(k))
}

/// Sprite blocks wide and high, from the file's suffix ("_2x1").
fn sprite_blocks(path: &str) -> (u8, u8) {
    let stem = path.trim_end_matches(".png");
    stem.rsplit('_')
        .next()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1, 1))
}

/// A design read from designs/factions, as cells to paint.
pub fn load_design(key: &str) -> Option<Design> {
    let bp = crate::building::blueprint::load_design_file(format!("designs/factions/{key}.json"))?;
    let mut cells: HashMap<IVec2, DesignCell> = HashMap::new();
    for hc in &bp.hull_cells {
        let sprite = crate::sprite_map::hull_layer_sprite_path(hc.material, hc.layer);
        cells.insert(
            hc.grid_pos,
            DesignCell { at: hc.grid_pos, sprite, part: (0, 0, 1, 1), module: false, lit: false, outer: hc.layer == HullLayer::Outer },
        );
    }
    for m in &bp.modules {
        if m.extras.as_ref().is_some_and(|e| e.turret.is_some()) {
            continue; // stands on a turret's own grid
        }
        let Some(sprite) = crate::sprite_map::module_sprite_path(m.module_type) else { continue };
        let (w, h) = sprite_blocks(sprite);
        for x in 0..w as i32 {
            for y in 0..h as i32 {
                let at = m.grid_pos + m.rotation.rotate_offset(IVec2::new(x, y));
                cells.insert(
                    at,
                    DesignCell { at, sprite, part: (x as u8, (h as i32 - 1 - y) as u8, w, h), module: true, lit: lit_room(m.module_type), outer: false },
                );
            }
        }
    }
    if cells.is_empty() {
        return None;
    }
    let min = cells.keys().fold(IVec2::MAX, |a, b| a.min(*b));
    let max = cells.keys().fold(IVec2::MIN, |a, b| a.max(*b));
    let mut cells: Vec<DesignCell> = cells.into_values().collect();
    cells.sort_by_key(|c| (c.at.y, c.at.x));
    Some(Design { cells, min, max })
}

/// Every design the Raft uses, read once.
pub fn library() -> &'static HashMap<&'static str, Design> {
    static LIB: OnceLock<HashMap<&'static str, Design>> = OnceLock::new();
    LIB.get_or_init(|| {
        std::iter::once(ANCHOR)
            .chain(LASHED.iter().map(|l| l.0))
            .chain(MOORED.iter().copied())
            .filter_map(|k| load_design(k).map(|d| (k, d)))
            .collect()
    })
}

/// A cell offset turned `quarter` quarter-turns anticlockwise.
pub fn turn(v: Vec2, quarter: u8) -> Vec2 {
    match quarter % 4 {
        0 => v,
        1 => Vec2::new(-v.y, v.x),
        2 => -v,
        _ => Vec2::new(v.y, -v.x),
    }
}

/// One ship in the Raft.
#[derive(Clone, Debug, PartialEq)]
pub struct Mooring {
    pub design: &'static str,
    /// Where its middle is, world units from the Raft's centre.
    pub at: Vec2,
    pub quarter: u8,
    /// How high it stands (0 low .. 1 high): what's drawn over what.
    pub level: f32,
    /// Which street it's on (None: the anchor and the hulls lashed to it).
    pub street: Option<usize>,
    /// Its berth number (None for the hulk and the hulls lashed to it):
    /// ships come and go, berths stay put.
    pub slot: Option<usize>,
    /// Where its walkway meets the street, world units from the centre.
    pub dock: Vec2,
}

/// A walkway: its ends, its width, and the berths it joins (None: the hulk
/// or a lashed hull, which never leave).
#[derive(Clone, Debug)]
pub struct Tube {
    pub a: Vec2,
    pub b: Vec2,
    pub width: f32,
    pub ends: [Option<usize>; 2],
}

#[derive(Clone, Debug)]
pub struct RaftPlan {
    pub moorings: Vec<Mooring>,
    /// Empty berths: where arriving ships tie up. They touch nothing moored.
    pub spares: Vec<Mooring>,
    /// Gantry streets, end to end.
    pub streets: Vec<(Vec2, Vec2)>,
    /// Walkways.
    pub tubes: Vec<Tube>,
    /// The solid parts as circles.
    pub circles: Vec<(Vec2, f32)>,
    /// Where the player's walkway meets the Raft: the east street's end.
    pub berth_tip: Vec2,
    /// The furthest anything reaches from the centre.
    pub extent: f32,
}

/// Half the width of the lane in to the berth, kept clear of everything.
pub const LANE_HALF_WIDTH: f32 = 2_800.0;

/// The east street: fixed, so the berth is too.
const EAST_FROM: f32 = 40.0;
/// Its end, in cells: `HAVEN_RADIUS` east of the centre.
pub fn east_tip_cells() -> f32 {
    super::home_base::HAVEN_RADIUS / CELL
}

/// Lay the Raft out. `ships` is how many moor along the streets (the anchor
/// and the hulls lashed to it come on top): the nearest slots fill first, so
/// a bigger Raft is the same Raft with its streets filled further out.
pub fn plan_raft(seed: u64, ships: usize) -> RaftPlan {
    let lib = library();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut moorings = Vec::new();
    let place = |key: &'static str, centre_cells: Vec2, quarter: u8, level: f32, street: Option<usize>| -> Option<Mooring> {
        lib.get(key)?;
        Some(Mooring { design: key, at: centre_cells * CELL, quarter, level, street, slot: None, dock: Vec2::ZERO })
    };
    if let Some(m) = place(ANCHOR, Vec2::ZERO, 0, 0.8, None) {
        moorings.push(m);
    }
    for (key, at, q) in LASHED {
        if let Some(m) = place(key, at.as_vec2(), q, 0.6, None) {
            moorings.push(m);
        }
    }

    // Streets: the fixed one east, the rest off the anchor's other sides.
    // Each starts under the anchor, so it is fixed to it, not floating off it.
    let tip = east_tip_cells();
    let mut streets_cells: Vec<(Vec2, Vec2)> = vec![(Vec2::new(EAST_FROM, 0.0), Vec2::new(tip, 0.0))];
    // A city grid: avenues north and south off both ends of the hulk and
    // off the lashed hulls, and one west past the seed ark. Square to the
    // ships, so they pack into blocks.
    let others = [
        (Vec2::Y, Vec2::new(30.0, 0.0)),
        (Vec2::NEG_Y, Vec2::new(-30.0, 0.0)),
        (Vec2::Y, Vec2::new(160.0, 40.0)),
        (Vec2::NEG_Y, Vec2::new(160.0, -40.0)),
        (Vec2::Y, Vec2::new(-130.0, 50.0)),
        (Vec2::NEG_Y, Vec2::new(-130.0, -50.0)),
        (Vec2::NEG_X, Vec2::new(-60.0, 0.0)),
    ];
    for (dir, from) in others {
        let len: f32 = rng.gen_range(200.0..260.0);
        streets_cells.push((from, from + dir * len));
    }

    // Every slot along every street. Each side of a street has its own run
    // of ships, end to end with a gap, so none sits on another.
    let mut slots: Vec<(f32, Mooring, Vec2)> = Vec::new();
    for (si, (a, b)) in streets_cells.iter().enumerate() {
        let along = (*b - *a).normalize();
        let across = Vec2::new(-along.y, along.x);
        let len = a.distance(*b);
        // Ships lie along the street, square to the grid.
        let lie = if along.x.abs() >= along.y.abs() { [0u8, 2] } else { [1u8, 3] };
        // Keep the last stretch of the east street open for the berth.
        let end = if si == 0 { len - 25.0 } else { len - 8.0 };
        let core: Vec<Rect> = moorings.iter().filter_map(|m| outline(m, CELL * 2.0)).collect();
        // Four ranks a side, each moored right behind the one in front of it
        // and glued to it: packed tight, they read as districts, not a queue.
        for side in [-1.0f32, 1.0] {
            // The outer edge of the rank in front, along the street: (from, to, edge).
            let mut front: Vec<(f32, f32, f32)> = Vec::new();
            for rank in 0..4usize {
                let mut this_rank: Vec<(f32, f32, f32)> = Vec::new();
                let mut cursor = 6.0 + rng.gen_range(0.0..8.0);
                loop {
                    let key = MOORED[rng.gen_range(0..MOORED.len())];
                    let Some(d) = lib.get(key) else { break };
                    let quarter = lie[rng.gen_range(0..2)];
                    let span = d.span().as_vec2();
                    let (length, breadth) = if quarter % 2 == 0 { (span.x, span.y) } else { (span.y, span.x) };
                    if cursor + length > end {
                        break;
                    }
                    let (t0, t1) = (cursor, cursor + length);
                    // Moored right behind what's in front of it; where the rank in
                    // front has a gap, behind its nearest ship either side.
                    let behind = if rank == 0 {
                        Some(4.0)
                    } else {
                        let over = front.iter().filter(|(f0, f1, _)| *f0 < t1 && *f1 > t0).map(|(_, _, e)| *e).fold(None, |a: Option<f32>, e| Some(a.map_or(e, |a: f32| a.max(e))));
                        over.or_else(|| front.iter().filter(|(f0, f1, _)| *f0 < t1 + 30.0 && *f1 > t0 - 30.0).map(|(_, _, e)| *e).fold(None, |a: Option<f32>, e| Some(a.map_or(e, |a: f32| a.max(e)))))
                    };
                    // Nothing in front to moor to: no ship here in this rank.
                    let Some(behind) = behind else {
                        cursor += 6.0;
                        continue;
                    };
                    let off = behind + 1.0 + breadth * 0.5;
                    let t = cursor + length * 0.5;
                    let centre = *a + along * t + across * side * off;
                    // Still under the hulk: move along rather than lose the slot.
                    let probe = Mooring { design: key, at: centre * CELL, quarter, level: 0.0, street: None, slot: None, dock: Vec2::ZERO };
                    if outline(&probe, 0.0).is_some_and(|r| core.iter().any(|c| !c.intersect(r).is_empty())) {
                        cursor += 4.0;
                        continue;
                    }
                    let level = rng.gen_range(0.3..0.5) - rank as f32 * 0.06;
                    let key_dist = centre.length() + rank as f32 * 25.0 + rng.gen_range(0.0..30.0);
                    let dock = (*a + along * t) * CELL;
                    slots.push((key_dist, Mooring { design: key, at: centre * CELL, quarter, level, street: Some(si), slot: None, dock }, *a + along * t));
                    this_rank.push((t0, t1, off + breadth * 0.5));
                    cursor += length + rng.gen_range(0.5..1.5);
                }
                front = this_rank;
            }
        }
    }
    // Nearest first; a ship whose outline would touch one already moored
    // (or the hulk, or the hulls lashed to it) is passed over.
    slots.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut taken: Vec<Rect> = moorings.iter().filter_map(|m| outline(m, CELL * 0.3)).collect();
    // The lane in to the berth is never moored in.
    taken.push(Rect::new(tip * CELL + CELL, -LANE_HALF_WIDTH, tip * CELL + 80_000.0, LANE_HALF_WIDTH));
    let mut tubes = Vec::new();
    // The lashed hulls, walked to from the hulk.
    for m in moorings.iter().skip(1) {
        tubes.push(Tube { a: Vec2::ZERO, b: m.at, width: CELL * 2.6, ends: [None, None] });
    }
    // Every berth that fits, numbered in order; the first `ships` are taken,
    // the rest wait empty for arrivals.
    let mut spares = Vec::new();
    let mut slot = 0;
    for (_, mut m, _) in slots {
        let Some(r) = outline(&m, CELL * 0.3) else { continue };
        if taken.iter().any(|o| !o.intersect(r).is_empty()) {
            continue;
        }
        taken.push(r);
        m.slot = Some(slot);
        slot += 1;
        if slot <= ships {
            // Every ship has its walkway to the street.
            tubes.push(Tube { a: m.at, b: m.dock, width: CELL * 1.8, ends: [m.slot, None] });
            moorings.push(m);
        } else {
            spares.push(m);
        }
    }
    // And to its neighbours on the same side of the street, end to end.
    let moored: Vec<&Mooring> = moorings.iter().filter(|m| m.street.is_some()).collect();
    for (i, m) in moored.iter().enumerate() {
        let Some(mine) = outline(m, 0.0) else { continue };
        for o in moored.iter().skip(i + 1) {
            if o.street != m.street {
                continue;
            }
            let Some(theirs) = outline(o, 0.0) else { continue };
            // Same side: their outlines overlap across the street's line.
            let gap = mine.union(theirs).size() - mine.size() - theirs.size();
            if gap.x.max(gap.y) < CELL * 8.0 && gap.x.min(gap.y) < 0.0 {
                tubes.push(Tube { a: m.at, b: o.at, width: CELL * 1.4, ends: [m.slot, o.slot] });
            }
        }
    }

    let streets: Vec<(Vec2, Vec2)> = streets_cells.iter().map(|(a, b)| (*a * CELL, *b * CELL)).collect();
    let mut plan = RaftPlan { moorings, spares, streets, tubes, circles: Vec::new(), berth_tip: Vec2::new(tip * CELL, 0.0), extent: 0.0 };
    plan.circles = collision_circles(&plan);
    // The extent counts the empty berths too: ships will moor there.
    let mut all = plan.clone();
    all.moorings.extend(plan.spares.iter().cloned());
    plan.extent = collision_circles(&all).iter().map(|(c, r)| c.length() + r).fold(0.0, f32::max);
    plan
}

/// A moored ship's outline in world units, grown by `margin` all round.
pub fn outline(m: &Mooring, margin: f32) -> Option<Rect> {
    let d = library().get(m.design)?;
    let span = d.span().as_vec2() * CELL;
    let half = if m.quarter % 2 == 0 { span * 0.5 } else { Vec2::new(span.y, span.x) * 0.5 };
    Some(Rect::from_center_half_size(m.at, half + Vec2::splat(margin)))
}

/// Each ship as a row of circles down its length, and each street as a
/// thin row of its own.
pub fn collision_circles(plan: &RaftPlan) -> Vec<(Vec2, f32)> {
    let mut out = Vec::new();
    for m in &plan.moorings {
        let Some(r) = outline(m, 0.0) else { continue };
        // A row of circles down the long side, each as wide as the short one.
        let size = r.size();
        let (long, short, along) = if size.x >= size.y { (size.x, size.y, Vec2::X) } else { (size.y, size.x, Vec2::Y) };
        let n = ((long / short.max(1.0)).ceil() as usize).max(1);
        for i in 0..n {
            let t = if n == 1 { 0.0 } else { i as f32 / (n - 1) as f32 - 0.5 };
            out.push((m.at + along * t * (long - short), short * 0.5));
        }
    }
    for (a, b) in &plan.streets {
        let len = a.distance(*b);
        let n = (len / (CELL * 4.0)) as usize;
        for i in 0..=n {
            out.push((a.lerp(*b, i as f32 / n.max(1) as f32), CELL * 1.5));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_design_it_uses_is_on_disk() {
        let lib = library();
        assert!(lib.contains_key(ANCHOR));
        for k in MOORED {
            assert!(lib.contains_key(k), "{k}");
        }
    }

    #[test]
    fn the_same_seed_lays_out_the_same_raft() {
        assert_eq!(plan_raft(5, 60).moorings, plan_raft(5, 60).moorings);
    }

    #[test]
    fn a_bigger_raft_is_the_same_raft_filled_further_out() {
        let small = plan_raft(9, 40);
        let big = plan_raft(9, 80);
        for m in &small.moorings {
            assert!(big.moorings.contains(m), "{m:?} gone from the bigger raft");
        }
        assert!(big.moorings.len() > small.moorings.len());
    }

    #[test]
    fn the_berth_is_due_east_at_havens_radius() {
        let plan = plan_raft(3, 80);
        assert!((plan.berth_tip - Vec2::new(super::super::home_base::HAVEN_RADIUS, 0.0)).length() < 1.0);
    }

    /// Nothing moored or built in the lane the player comes in by: east of
    /// the tip, as wide as a big ship.
    #[test]
    fn the_lane_to_the_berth_is_clear() {
        for seed in 0..12 {
            let plan = plan_raft(seed, 150);
            let tip = plan.berth_tip;
            for (c, r) in &plan.circles {
                let in_lane_x = c.x + r > tip.x + 2.0 * CELL && c.x - r < tip.x + 60_000.0;
                let in_lane_y = c.y.abs() - r < 2_500.0;
                assert!(!(in_lane_x && in_lane_y), "seed {seed}: something at {c} r {r} in the lane");
            }
        }
    }

    #[test]
    fn no_ship_sits_on_another() {
        for seed in 0..8 {
            let plan = plan_raft(seed, 150);
            let boxes: Vec<Rect> = plan.moorings.iter().filter_map(|m| outline(m, 0.0)).collect();
            for i in 0..boxes.len() {
                for j in i + 1..boxes.len() {
                    assert!(boxes[i].intersect(boxes[j]).is_empty(), "seed {seed}: {:?} on {:?}", plan.moorings[i], plan.moorings[j]);
                }
            }
        }
    }

    #[test]
    fn empty_berths_touch_nothing_moored() {
        let plan = plan_raft(4, 40);
        assert!(!plan.spares.is_empty(), "no room for arrivals");
        let all: Vec<Rect> = plan.moorings.iter().chain(&plan.spares).filter_map(|m| outline(m, 0.0)).collect();
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                assert!(all[i].intersect(all[j]).is_empty());
            }
        }
    }

    #[test]
    fn every_ship_has_a_walkway() {
        let plan = plan_raft(2, 100);
        for m in plan.moorings.iter().skip(1) {
            assert!(plan.tubes.iter().any(|t| t.a == m.at || t.b == m.at), "{} has no walkway", m.design);
        }
    }

    #[test]
    fn it_stays_within_its_extent_and_is_city_sized() {
        let plan = plan_raft(1, 150);
        assert!(plan.extent < super::super::home_base::HAVEN_EXTENT, "{}", plan.extent);
        assert!(plan.extent > 15_000.0, "{} is not a city", plan.extent);
    }
}

