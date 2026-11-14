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
    /// Its own colour, where it isn't the sprite's (glass, panels, cargo).
    pub tint: Option<[f32; 3]>,
    /// How much higher it stands than the deck (0..1): tiers.
    pub lift: f32,
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
            DesignCell { at: hc.grid_pos, sprite, part: (0, 0, 1, 1), module: false, lit: false, outer: hc.layer == HullLayer::Outer, tint: None, lift: 0.0 },
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
                    DesignCell { at, sprite, part: (x as u8, (h as i32 - 1 - y) as u8, w, h), module: true, lit: lit_room(m.module_type), outer: false, tint: None, lift: 0.0 },
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

/// Every ship design the Raft moors, read once.
pub fn library() -> &'static HashMap<&'static str, Design> {
    static LIB: OnceLock<HashMap<&'static str, Design>> = OnceLock::new();
    LIB.get_or_init(|| MOORED.iter().copied().filter_map(|k| load_design(k).map(|d| (k, d))).collect())
}

/// A design by key: a ship's, or a built part's (world::sections).
pub fn design(key: &str) -> Option<&'static Design> {
    library().get(key).or_else(|| super::sections::get(key))
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

/// The berth arm's end, in cells: `HAVEN_RADIUS` east of the centre.
pub fn east_tip_cells() -> f32 {
    super::home_base::HAVEN_RADIUS / CELL
}

/// How a station is laid out. `order` 1 is a rich, proud owner's: every arm
/// one straight run, everything evenly spaced. Toward 0 it's a poor one's:
/// arms that jog and kink and change width, things spaced as they came.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub order: f32,
    /// The hub's half-width, in blocks.
    pub hub: i32,
    /// Arms in all, the four off the hub's flats and branches off them.
    pub arms: usize,
}

/// Haven: a refuge, kept up but not grand.
pub const HAVEN_STYLE: Style = Style { order: 0.55, hub: 100, arms: 11 };

/// Blocks of gantry between an arm and a moored ship.
const GANTRY: i32 = 3;

fn quarter_facing(v: IVec2) -> u8 {
    match (v.x, v.y) {
        (0, 1) => 0,
        (-1, 0) => 1,
        (0, -1) => 2,
        _ => 3,
    }
}

/// What a placed piece may not touch: arms are only off limits to things
/// bolted to other arms.
#[derive(Clone, Copy, PartialEq)]
enum Owner {
    Arm(usize),
    Thing,
}

fn clear(taken: &[(Rect, Owner)], r: Rect, own_arm: usize) -> bool {
    taken.iter().all(|(o, who)| *who == Owner::Arm(own_arm) || o.intersect(r).is_empty())
}

/// Haven as it is laid out at the start of a run.
pub fn plan_raft(seed: u64, ships: usize) -> RaftPlan {
    plan_station(seed, ships, &HAVEN_STYLE)
}

/// Lay a station out: a hub, arms out of it (straight or crooked by its
/// owner's `order`), and along both sides of every arm, runs of berths
/// (each a gantry with a ship tied up to it) and station sections bolted
/// flush, big and small. Nothing is placed where it would touch something it
/// isn't joined to, so nothing hangs. `ships` berths nearest the hub are
/// taken; the rest wait empty for arrivals.
pub fn plan_station(seed: u64, ships: usize, style: &Style) -> RaftPlan {
    use super::sections;
    let mut rng = StdRng::seed_from_u64(seed);
    let order = style.order.clamp(0.0, 1.0);
    let mess = 1.0 - order;
    let mut moorings: Vec<Mooring> = Vec::new();
    let mut taken: Vec<(Rect, Owner)> = Vec::new();
    let tip = east_tip_cells();
    // The lane in to the berth is never built in.
    taken.push((Rect::new(tip * CELL + CELL, -LANE_HALF_WIDTH, tip * CELL + 80_000.0, LANE_HALF_WIDTH), Owner::Thing));

    let static_piece = |key: &'static str, centre: Vec2, quarter: u8, level: f32, street: Option<usize>| Mooring {
        design: key,
        at: centre * CELL,
        quarter,
        level,
        street,
        slot: None,
        dock: Vec2::ZERO,
    };

    // The hub.
    let hub = sections::hub_octagon(style.hub, &mut rng);
    let hub_m = static_piece(hub, Vec2::ZERO, 0, 0.85, None);
    if let Some(r) = outline(&hub_m, 0.0) {
        taken.push((r, Owner::Thing));
    }
    moorings.push(hub_m);
    let collar = (style.hub + 7) as f32;

    // The arms: (start, direction, length, must be straight).
    let mut arms: Vec<(Vec2, IVec2, f32, bool)> = vec![(Vec2::new(collar - 2.0, 0.0), IVec2::X, tip - (collar - 2.0), true)];
    for dir in [IVec2::Y, IVec2::NEG_X, IVec2::NEG_Y] {
        arms.push((dir.as_vec2() * (collar - 2.0), dir, rng.gen_range(320.0..420.0), false));
    }

    // Each arm in straight runs; crooked ones jog between runs. Branches
    // come off the main arms' actual runs (so off a crooked arm's kinks too).
    struct Seg {
        a: Vec2,
        dir: IVec2,
        len: f32,
        half: i32,
    }
    let mut segs: Vec<Seg> = Vec::new();
    let mut elbows: Vec<(Mooring, usize)> = Vec::new();
    let mut lay = |segs: &mut Vec<Seg>, elbows: &mut Vec<(Mooring, usize)>, rng: &mut StdRng, start: Vec2, dir: IVec2, len: f32, straight: bool| {
        let perp = IVec2::new(-dir.y, dir.x);
        let mut pos = start;
        let mut t = 0.0;
        while t < len - 1.0 {
            let mut piece = if straight || order >= 0.95 { len - t } else { rng.gen_range(95.0..170.0f32).min(len - t) };
            // No stub at the end: fold a short remainder into this run.
            if len - t - piece < 40.0 {
                piece = len - t;
            }
            let half = 5 + if straight { 1 } else { rng.gen_range(0..=((mess * 3.0) as i32)) };
            segs.push(Seg { a: pos, dir, len: piece, half });
            pos += dir.as_vec2() * piece;
            t += piece;
            if t < len - 1.0 && !straight {
                let jog = (rng.gen_range(-1.0..1.0f32) * mess * 16.0).round() as i32;
                if jog.abs() >= 2 {
                    // An elbow joins the runs: a short arm across, overlapping both.
                    let across = perp * jog.signum();
                    let elbow_len = jog.abs() + half * 2 + 1;
                    let key = sections::arm_segment(elbow_len, half);
                    let centre = pos + across.as_vec2() * (jog.abs() as f32 * 0.5) - dir.as_vec2() * (half as f32 * 0.5);
                    // The design lies along +x: turn it to lie along `across`.
                    elbows.push((static_piece(key, centre, (quarter_facing(across) + 1) % 4, 0.5, None), segs.len() - 1));
                    pos += across.as_vec2() * jog.abs() as f32;
                }
            }
        }
    };
    for (start, dir, len, straight) in arms {
        lay(&mut segs, &mut elbows, &mut rng, start, dir, len, straight);
    }
    let mains = segs.len();
    for _ in 0..style.arms.saturating_sub(4) {
        // Off a main arm's run (not the berth arm), square to it.
        let candidates: Vec<usize> = (0..mains).filter(|i| segs[*i].dir != IVec2::X || segs[*i].a.y.abs() > 1.0).filter(|i| segs[*i].len > 90.0).collect();
        if candidates.is_empty() {
            break;
        }
        let i = candidates[rng.gen_range(0..candidates.len())];
        let (a, dir, len) = (segs[i].a, segs[i].dir, segs[i].len);
        let at = rng.gen_range(40.0..(len - 30.0));
        let side = if rng.gen_bool(0.5) { IVec2::new(-dir.y, dir.x) } else { IVec2::new(dir.y, -dir.x) };
        let blen = rng.gen_range(140.0..230.0);
        lay(&mut segs, &mut elbows, &mut rng, a + dir.as_vec2() * at, side, blen, false);
    }

    // The runs themselves, as designs.
    let mut streets = Vec::new();
    for (i, seg) in segs.iter().enumerate() {
        let key = sections::arm_segment(seg.len.round().max(2.0) as i32, seg.half);
        let centre = seg.a + seg.dir.as_vec2() * (seg.len * 0.5);
        let q = (quarter_facing(seg.dir) + 1) % 4; // the run's design lies along +x
        let m = static_piece(key, centre, q, 0.5, Some(i));
        if let Some(r) = outline(&m, 0.0) {
            taken.push((r, Owner::Arm(i)));
        }
        moorings.push(m);
        streets.push((seg.a * CELL, (seg.a + seg.dir.as_vec2() * seg.len) * CELL));
    }
    // Elbows are in everyone's way, their own arm's included.
    for (m, _seg) in elbows {
        if let Some(r) = outline(&m, 0.0) {
            taken.push((r, Owner::Thing));
        }
        moorings.push(m);
    }

    // Along both sides of every run: berths and sections.
    let big: Vec<&'static str> = MOORED.iter().copied().filter(|k| library().get(k).is_some_and(|d| d.span().max_element() >= 70)).collect();
    let small: Vec<&'static str> = MOORED.iter().copied().filter(|k| library().get(k).is_some_and(|d| d.span().max_element() < 70)).collect();
    let mut berths: Vec<(f32, Mooring)> = Vec::new();
    let mut variant = 0u32;
    for (i, seg) in segs.iter().enumerate() {
        for side in [-1i32, 1] {
            let out = IVec2::new(-seg.dir.y, seg.dir.x) * side;
            let q_out = quarter_facing(out);
            let mut t = 6.0 + rng.gen_range(0.0..6.0 * mess.max(0.2));
            let mut ships_run = rng.gen_bool(0.6);
            let mut run_left = 0;
            let mut misses = 0;
            while t < seg.len - 6.0 && misses < 40 {
                if run_left == 0 {
                    ships_run = rng.gen_bool(0.7);
                    run_left = if ships_run { rng.gen_range(3..6) } else { rng.gen_range(1..3) };
                }
                let gap = if order > 0.8 { 2.0 } else { rng.gen_range(1.0..2.0 + mess * 5.0) };
                if ships_run {
                    let pool = if t / seg.len < 0.55 && !big.is_empty() { &big } else { &small };
                    let mut key = pool[rng.gen_range(0..pool.len())];
                    // Too thick for what's left of the run: a smaller one.
                    if library().get(key).is_some_and(|d| t + d.span().min_element() as f32 > seg.len - 4.0) {
                        key = small[rng.gen_range(0..small.len())];
                    }
                    let Some(d) = library().get(key) else { break };
                    let span = d.span();
                    // Nose-in: its long side out from the arm.
                    let q = if span.x >= span.y { (q_out + 1) % 4 } else { q_out };
                    let (long, along) = (span.max_element() as f32, span.min_element() as f32);
                    if t + along > seg.len - 4.0 {
                        break;
                    }
                    let mid = t + along * 0.5;
                    let base = seg.a + seg.dir.as_vec2() * mid;
                    let centre = base + out.as_vec2() * (seg.half as f32 + 0.5 + GANTRY as f32 + long * 0.5);
                    let ship = Mooring { design: key, at: centre * CELL, quarter: q, level: rng.gen_range(0.3..0.42), street: Some(i), slot: None, dock: (base + out.as_vec2() * (seg.half as f32 + 0.5)) * CELL };
                    let g_key = sections::gantry(GANTRY);
                    let g_centre = base + out.as_vec2() * (seg.half as f32 + 1.0) + turn(super::sections::get(g_key).map_or(Vec2::ZERO, |g| g.centre()), q_out);
                    let gantry = static_piece(g_key, g_centre, q_out, 0.45, Some(i));
                    match (outline(&ship, CELL * 0.3), outline(&gantry, 0.0)) {
                        (Some(rs), Some(rg)) if clear(&taken, rs, i) && clear(&taken, rg, i) => {
                            taken.push((rs, Owner::Thing));
                            taken.push((rg, Owner::Thing));
                            moorings.push(gantry);
                            berths.push((ship.at.length() + rng.gen_range(0.0..2_000.0), ship));
                            t += along + gap;
                            run_left -= 1;
                        }
                        _ => {
                            t += 4.0;
                            misses += 1;
                        }
                    }
                } else {
                    let kind = super::sections::ALL_KINDS[rng.gen_range(0..super::sections::ALL_KINDS.len())];
                    let size = if rng.gen_bool(0.3) { rng.gen_range(0.6..1.0) } else { rng.gen_range(0.0..0.4) };
                    variant += 1;
                    let key = sections::section(kind, size, variant % 6, &mut rng);
                    let Some(d) = super::sections::get(key) else { break };
                    let w = d.span().x as f32;
                    if t + w > seg.len - 4.0 {
                        // Too wide for what's left: try the run as berths.
                        ships_run = true;
                        run_left = 1;
                        misses += 1;
                        continue;
                    }
                    let mid = t + w * 0.5;
                    let origin = seg.a + seg.dir.as_vec2() * mid + out.as_vec2() * (seg.half as f32 + 1.0);
                    let centre = origin + turn(d.centre(), q_out);
                    let m = static_piece(key, centre, q_out, rng.gen_range(0.38..0.5), Some(i));
                    match outline(&m, 0.0) {
                        Some(r) if clear(&taken, r, i) => {
                            taken.push((r, Owner::Thing));
                            moorings.push(m);
                            t += w + gap;
                            run_left -= 1;
                        }
                        _ => {
                            t += 4.0;
                            misses += 1;
                        }
                    }
                }
            }
        }
    }

    // The nearest berths are taken, the rest wait empty.
    berths.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut spares = Vec::new();
    let mut tubes = Vec::new();
    for (k, (_, mut m)) in berths.into_iter().enumerate() {
        m.slot = Some(k);
        if k < ships {
            tubes.push(Tube { a: m.at, b: m.dock, width: CELL * 1.6, ends: [m.slot, None] });
            moorings.push(m);
        } else {
            spares.push(m);
        }
    }

    let mut plan = RaftPlan { moorings, spares, streets, tubes, circles: Vec::new(), berth_tip: Vec2::new(tip * CELL, 0.0), extent: 0.0 };
    plan.circles = collision_circles(&plan);
    let mut all = plan.clone();
    all.moorings.extend(plan.spares.iter().cloned());
    plan.extent = collision_circles(&all).iter().map(|(c, r)| c.length() + r).fold(0.0, f32::max);
    plan
}

fn design_of(key: &str) -> Option<&'static Design> {
    design(key)
}

/// The faction a design belongs to: its folder under designs/factions.
pub fn faction_of(design: &str) -> &str {
    design.split('/').next().unwrap_or(design)
}

/// `berth` with `design` in it instead, if it fits inside the berth's own
/// outline (so it can't touch a neighbour). Same middle, same turn.
pub fn refit(berth: &Mooring, design: &'static str) -> Option<Mooring> {
    let lib = library();
    let (old, new) = (design_of(berth.design)?, lib.get(design)?);
    let turned = |d: &Design| {
        let s = d.span();
        if berth.quarter % 2 == 0 { s } else { IVec2::new(s.y, s.x) }
    };
    let (o, n) = (turned(old), turned(new));
    (n.x <= o.x && n.y <= o.y).then(|| Mooring { design, ..berth.clone() })
}

/// A moored ship's outline in world units, grown by `margin` all round.
pub fn outline(m: &Mooring, margin: f32) -> Option<Rect> {
    let d = design_of(m.design)?;
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_design_it_uses_is_on_disk() {
        let lib = library();
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
        let small = plan_raft(9, 20);
        let big = plan_raft(9, 40);
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
        for seed in 0..6 {
            let plan = plan_raft(seed, 200);
            let ships: Vec<Rect> = plan.moorings.iter().chain(&plan.spares).filter(|m| m.slot.is_some()).filter_map(|m| outline(m, -CELL * 0.25)).collect();
            let all: Vec<Rect> = plan.moorings.iter().chain(&plan.spares).filter_map(|m| outline(m, -CELL * 0.25)).collect();
            for s in &ships {
                let hits = all.iter().filter(|o| !o.intersect(*s).is_empty()).count();
                assert_eq!(hits, 1, "seed {seed}: a ship at {s:?} overlaps something");
            }
        }
    }

    /// Nothing hangs: from the hub, every arm, section, gantry and berth is
    /// reached through things that touch.
    #[test]
    fn everything_is_joined_to_the_hub() {
        for seed in 0..6 {
            for order in [0.1, 0.55, 1.0] {
                let style = Style { order, ..HAVEN_STYLE };
                let plan = plan_station(seed, 200, &style);
                let pieces: Vec<Rect> = plan.moorings.iter().chain(&plan.spares).filter_map(|m| outline(m, CELL * 0.6)).collect();
                let mut reached = vec![false; pieces.len()];
                reached[0] = true;
                let mut frontier = vec![0];
                while let Some(i) = frontier.pop() {
                    for j in 0..pieces.len() {
                        if !reached[j] && !pieces[i].intersect(pieces[j]).is_empty() {
                            reached[j] = true;
                            frontier.push(j);
                        }
                    }
                }
                let loose: Vec<&str> = plan.moorings.iter().chain(&plan.spares).zip(&reached).filter(|(_, r)| !**r).map(|(m, _)| m.design).collect();
                assert!(loose.is_empty(), "seed {seed} order {order}: hanging: {loose:?}");
            }
        }
    }

    #[test]
    fn a_proud_owner_builds_straight_and_a_poor_one_crooked() {
        let straight = plan_station(3, 64, &Style { order: 1.0, ..HAVEN_STYLE });
        let crooked = plan_station(3, 64, &Style { order: 0.1, ..HAVEN_STYLE });
        // A straight station's arms are one run each; a crooked one's break up.
        assert_eq!(straight.streets.len(), HAVEN_STYLE.arms);
        assert!(crooked.streets.len() > straight.streets.len() + 3, "{} runs", crooked.streets.len());
    }

    #[test]
    fn there_are_sections_as_well_as_ships() {
        let plan = plan_raft(5, 64);
        let sections = plan.moorings.iter().filter(|m| m.design.starts_with("section/")).count();
        assert!(sections >= 10, "{sections} sections");
        let sizes: std::collections::HashSet<&str> = plan.moorings.iter().filter(|m| m.design.starts_with("section/")).map(|m| m.design.split('/').nth(2).unwrap_or("")).collect();
        assert!(sizes.len() >= 2, "only one size of section: {sizes:?}");
    }

    #[test]
    fn empty_berths_touch_nothing_moored() {
        // (That they touch nothing is no_ship_sits_on_another's job.)
        let plan = plan_raft(4, 30);
        assert!(!plan.spares.is_empty(), "no room for arrivals");
    }

    #[test]
    fn there_is_room_to_grow() {
        let plan = plan_raft(6, 64);
        let berths = plan.moorings.iter().chain(&plan.spares).filter(|m| m.slot.is_some()).count();
        assert!(berths >= 55, "{berths} berths");
    }

    #[test]
    fn a_refit_never_grows_past_its_berth() {
        let plan = plan_raft(3, 30);
        let berth = plan.spares.iter().find(|m| m.design == "merchant_princes/tanker").or(plan.spares.first()).unwrap();
        for d in MOORED {
            if let Some(m) = refit(berth, d) {
                let (a, b) = (outline(&m, 0.0).unwrap(), outline(berth, 0.0).unwrap());
                assert!(b.contains(a.min - Vec2::splat(0.01)) || (a.width() <= b.width() + 0.1 && a.height() <= b.height() + 0.1));
            }
        }
    }

    #[test]
    fn every_ship_has_a_walkway() {
        let plan = plan_raft(2, 100);
        for m in plan.moorings.iter().filter(|m| m.slot.is_some()) {
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
