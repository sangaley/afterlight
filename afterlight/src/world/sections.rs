//! The parts of a station that aren't ships, built out of the same block
//! sprites the ships are: the hub at the middle, the arms (one design per
//! straight run), the gantries a moored ship ties up to, and the sections
//! bolted along the arms -- habitats, solar arrays, radiators, tank farms,
//! cargo yards, antennas, greenhouses, cranes -- big or small.
//!
//! Each is a `Design` registered under a made-up key, so it is baked, drawn
//! and collided with exactly as a ship design is. A section's base row is
//! y = 0 and it grows toward +y: away from the arm it's bolted to.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::Rng;

use super::raft::{Design, DesignCell};

const STEEL: &str = "sprites/hull/hull_steel.png";
const TITANIUM: &str = "sprites/hull/hull_titanium.png";
const HALL: &str = "sprites/hull/hull_hallway.png";
const COMPOSITE: &str = "sprites/hull/hull_composite.png";
const QUARTERS: &str = "sprites/modules/basic_quarters.png";
const REACTOR: &str = "sprites/modules/small_reactor.png";
const CARGO: &str = "sprites/modules/cargo_hold.png";
const BATTERY: &str = "sprites/modules/battery.png";
const LIFE: &str = "sprites/modules/life_support.png";
const LAB: &str = "sprites/modules/research_lab.png";
const FRAME: &str = "sprites/modules/light_frame.png";
const DOCK: &str = "sprites/modules/docking_port.png";

/// The registry: built designs by key. Leaked: there are a few dozen, made
/// once, kept for the session.
fn built() -> &'static Mutex<HashMap<String, &'static Design>> {
    static B: OnceLock<Mutex<HashMap<String, &'static Design>>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A built design by key, if it has been made.
pub fn get(key: &str) -> Option<&'static Design> {
    built().lock().ok()?.get(key).copied()
}

/// Keep `make`'s design under `key` (made once), and the key for good.
pub fn register(key: String, make: impl FnOnce() -> Design) -> &'static str {
    let mut b = built().lock().expect("sections registry");
    if let Some((k, _)) = b.get_key_value(&key) {
        return Box::leak(k.clone().into_boxed_str());
    }
    let d: &'static Design = Box::leak(Box::new(make()));
    let k: &'static str = Box::leak(key.clone().into_boxed_str());
    b.insert(key, d);
    k
}

/// A generator seeded from a key, so a built design is the same however
/// many were made before it.
fn seeded(key: &str) -> StdRng {
    use rand::SeedableRng;
    StdRng::seed_from_u64(key.bytes().fold(0xC0FFEE_u64, |a, b| a.wrapping_mul(131).wrapping_add(b as u64)))
}

/// Cells to a design.
struct Builder {
    cells: HashMap<IVec2, DesignCell>,
}

impl Builder {
    fn new() -> Self {
        Self { cells: HashMap::new() }
    }
    fn put(&mut self, x: i32, y: i32, sprite: &'static str, module: bool, lit: bool, outer: bool) {
        let at = IVec2::new(x, y);
        self.cells.insert(at, DesignCell { at, sprite, part: (0, 0, 1, 1), module, lit, outer, tint: None, lift: 0.0 });
    }
    fn tinted(&mut self, x: i32, y: i32, sprite: &'static str, tint: [f32; 3], lit: bool) {
        let at = IVec2::new(x, y);
        self.cells.insert(at, DesignCell { at, sprite, part: (0, 0, 1, 1), module: true, lit, outer: false, tint: Some(tint), lift: 0.0 });
    }
    fn lift(&mut self, x: i32, y: i32, by: f32) {
        if let Some(c) = self.cells.get_mut(&IVec2::new(x, y)) {
            c.lift = c.lift.max(by);
        }
    }
    fn done(self) -> Design {
        let min = self.cells.keys().fold(IVec2::MAX, |a, b| a.min(*b));
        let max = self.cells.keys().fold(IVec2::MIN, |a, b| a.max(*b));
        let mut cells: Vec<DesignCell> = self.cells.into_values().collect();
        cells.sort_by_key(|c| (c.at.y, c.at.x));
        Design { cells, min, max }
    }
}

/// Haven's hub: an octagonal deck, a lit habitat ring standing on it, eight
/// towers, a spire, and a docking collar on each flat where an arm leaves.
/// `r` is its half-width in blocks.
pub fn hub_octagon(r: i32, _layout: &mut StdRng) -> &'static str {
    let key = format!("hub/octagon/{r}");
    if get(&key).is_some() {
        return register(key, || unreachable!());
    }
    // Its own randomness, from its name: the same hub whatever was made first.
    let rng = &mut seeded(&key);
    let mut b = Builder::new();
    let ring = (r as f32 * 0.40, r as f32 * 0.55);
    for x in -r..=r {
        for y in -r..=r {
            if (x.abs() + y.abs()) as f32 > r as f32 * 1.32 {
                continue;
            }
            let d = ((x * x + y * y) as f32).sqrt();
            let edge = x.abs() == r || y.abs() == r || (x.abs() + y.abs()) as f32 >= r as f32 * 1.32 - 1.0;
            if x.abs() <= 1 || y.abs() <= 1 || (x.abs() - y.abs()).abs() <= 1 {
                b.put(x, y, HALL, false, false, false);
            } else if (ring.0..=ring.1).contains(&d) {
                b.put(x, y, QUARTERS, true, rng.gen_bool(0.55), false);
                b.lift(x, y, 0.6);
            } else if d < r as f32 * 0.2 {
                b.put(x, y, if d < r as f32 * 0.1 { REACTOR } else { TITANIUM }, d < r as f32 * 0.1, rng.gen_bool(0.3), false);
                b.lift(x, y, 1.0);
            } else {
                b.put(x, y, if edge { STEEL } else if (x + y).rem_euclid(9) == 0 { BATTERY } else { STEEL }, false, false, edge);
            }
        }
    }
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU + std::f32::consts::FRAC_PI_8;
        let (cx, cy) = ((a.cos() * r as f32 * 0.78) as i32, (a.sin() * r as f32 * 0.78) as i32);
        for x in cx - 6..=cx + 6 {
            for y in cy - 6..=cy + 6 {
                b.put(x, y, if (x + y).rem_euclid(4) == 0 { CARGO } else { QUARTERS }, true, rng.gen_bool(0.6), false);
                b.lift(x, y, 0.75);
            }
        }
    }
    for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
        for t in r - 4..r + 8 {
            for w in -12..=12 {
                let (x, y) = if dx != 0 { (dx * t, w) } else { (w, dy * t) };
                b.put(x, y, if w.abs() < 9 { DOCK } else { STEEL }, w.abs() < 9, rng.gen_bool(0.2), w.abs() == 12);
                b.lift(x, y, 0.15);
            }
        }
    }
    let d = b.done();
    register(key, || d)
}

/// The shapes a hub can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubKind {
    Octagon,
    Wheel,
    Hex,
    Spindle,
}

/// A hub of this shape, `r` blocks from its middle to its flats.
pub fn hub(kind: HubKind, r: i32, layout: &mut StdRng) -> &'static str {
    if kind == HubKind::Octagon {
        return hub_octagon(r, layout);
    }
    let key = format!("hub/{kind:?}/{r}");
    if get(&key).is_some() {
        return register(key, || unreachable!());
    }
    let rng = &mut seeded(&key);
    let mut b = Builder::new();
    let rf = r as f32;
    match kind {
        HubKind::Wheel => {
            // A rim of habitat, spokes, and a hub in the middle.
            for x in -r..=r {
                for y in -r..=r {
                    let d = ((x * x + y * y) as f32).sqrt();
                    let a = (y as f32).atan2(x as f32);
                    let spoke = ((a / (std::f32::consts::TAU / 6.0)).rem_euclid(1.0) - 0.5).abs() > 0.44;
                    if (rf * 0.72..=rf).contains(&d) {
                        let edge = d > rf - 1.2 || d < rf * 0.72 + 1.2;
                        if !edge && (rf * 0.78..=rf * 0.94).contains(&d) {
                            b.put(x, y, QUARTERS, true, rng.gen_bool(0.55), false);
                            b.lift(x, y, 0.6);
                        } else {
                            b.put(x, y, STEEL, false, false, edge);
                        }
                    } else if d < rf * 0.3 {
                        b.put(x, y, if d < rf * 0.16 { REACTOR } else { TITANIUM }, d < rf * 0.16, rng.gen_bool(0.35), d > rf * 0.3 - 1.2);
                        b.lift(x, y, 0.9);
                    } else if spoke {
                        b.put(x, y, HALL, false, rng.gen_bool(0.12), false);
                    }
                }
            }
        }
        HubKind::Hex => {
            // Three tiers of hexagon, each standing on the one below.
            for (tier, scale, sprite, lit) in [(0, 1.0f32, STEEL, 0.0), (1, 0.72, QUARTERS, 0.5), (2, 0.4, TITANIUM, 0.4)] {
                let rr = rf * scale;
                for x in -r..=r {
                    for y in -r..=r {
                        let q = x.abs() as f32 * 0.866 + y.abs() as f32 * 0.5;
                        if q <= rr * 0.866 && (y.abs() as f32) <= rr * 0.98 {
                            let edge = q > rr * 0.866 - 1.2;
                            b.put(x, y, if edge { STEEL } else { sprite }, tier > 0, rng.gen_bool(lit as f64), edge);
                            b.lift(x, y, tier as f32 * 0.4);
                        }
                    }
                }
            }
        }
        _ => {
            // A spine with decks along it, widest in the middle.
            let reach = (rf * 1.25) as i32;
            for y in -reach..=reach {
                for x in -10..=10 {
                    b.put(x, y, if x.abs() <= 1 { HALL } else { STEEL }, false, false, x.abs() == 10);
                }
            }
            for k in -3..=3 {
                let cy = k * reach / 4;
                let w = (rf * (0.85 - k.abs() as f32 * 0.14)) as i32;
                for x in -w..=w {
                    for y in cy - 8..=cy + 8 {
                        let edge = x.abs() == w || (y - cy).abs() == 8;
                        b.put(x, y, if edge { STEEL } else if x.abs() <= 10 { TITANIUM } else { QUARTERS }, !edge, !edge && rng.gen_bool(0.5), edge);
                        b.lift(x, y, if edge { 0.2 } else { 0.55 });
                    }
                }
            }
            for x in -13..=13 {
                for y in -13..=13 {
                    if x * x + y * y <= 169 {
                        b.put(x, y, REACTOR, true, rng.gen_bool(0.4), false);
                        b.lift(x, y, 1.0);
                    }
                }
            }
        }
    }
    // Docking collars where the arms go out (east and west at the flats;
    // the spindle's north and south ones at its ends).
    for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
        let at = if kind == HubKind::Spindle && dy != 0 { (rf * 1.25) as i32 } else { r };
        for t in at - 4..at + 8 {
            for w in -12..=12 {
                let (x, y) = if dx != 0 { (dx * t, w) } else { (w, dy * t) };
                b.put(x, y, if w.abs() < 9 { DOCK } else { STEEL }, w.abs() < 9, rng.gen_bool(0.2), w.abs() == 12);
                b.lift(x, y, 0.15);
            }
        }
        // Make sure the collar reaches the hub's body (a wheel's rim is round).
        for t in 0..at - 4 {
            for w in -3..=3 {
                let (x, y) = if dx != 0 { (dx * t, w) } else { (w, dy * t) };
                if !b.cells.contains_key(&IVec2::new(x, y)) {
                    b.put(x, y, HALL, false, false, false);
                }
            }
        }
    }
    let d = b.done();
    register(key, || d)
}

/// How far a hub of this shape reaches out to its collars along x and y.
pub fn hub_reach(kind: HubKind, r: i32) -> (i32, i32) {
    let along_y = if kind == HubKind::Spindle { (r as f32 * 1.25) as i32 } else { r };
    (r + 7, along_y + 7)
}

/// One straight run of an arm, `len` blocks along +x, `half` either side:
/// plating with a hallway down the middle and lit frames along it.
pub fn arm_segment(len: i32, half: i32) -> &'static str {
    register(format!("arm/{len}/{half}"), || {
        let mut b = Builder::new();
        for x in 0..len {
            for y in -half..=half {
                let edge = y.abs() == half;
                if y.abs() <= 1 {
                    b.put(x, y, if x % 6 == 0 { FRAME } else { HALL }, x % 6 == 0, x % 6 == 0, false);
                } else {
                    b.put(x, y, if edge { STEEL } else if x % 9 == 4 { BATTERY } else { STEEL }, false, false, edge);
                }
            }
        }
        b.done()
    })
}

/// A gantry a ship ties up to: `len` blocks out from the arm (+y), three wide.
pub fn gantry(len: i32) -> &'static str {
    register(format!("gantry/{len}"), || {
        let mut b = Builder::new();
        for y in 0..len {
            for x in -1..=1 {
                b.put(x, y, if x == 0 { HALL } else { TITANIUM }, x == 0 && y % 3 == 1, x == 0 && y % 3 == 1, x != 0);
            }
        }
        // A collar where the ship's airlock meets it.
        for x in -2..=2 {
            b.put(x, len - 1, DOCK, true, false, false);
        }
        b.done()
    })
}

/// What a station section is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Habitat,
    Solar,
    Radiator,
    Tanks,
    Cargo,
    Antenna,
    Greenhouse,
    Crane,
}

pub const ALL_KINDS: [Kind; 8] = [Kind::Habitat, Kind::Solar, Kind::Radiator, Kind::Tanks, Kind::Cargo, Kind::Antenna, Kind::Greenhouse, Kind::Crane];

/// A section, small or big (`size` 0 small .. 1 big), variant `v`. Its base
/// row is y = 0, flush against the arm; it grows toward +y.
pub fn section(kind: Kind, size: f32, v: u32, _layout: &mut StdRng) -> &'static str {
    let s = (size * 3.0).round() as i32; // 0..3
    let key = format!("section/{kind:?}/{s}/{v}");
    if get(&key).is_some() {
        return register(key, || unreachable!());
    }
    let rng = &mut seeded(&key);
    let k = 1.0 + s as f32 * 0.55;
    let mut b = Builder::new();
    let glass = [0.22, 0.55, 0.32];
    let panel = [0.12, 0.2, 0.42];
    match kind {
        Kind::Habitat => {
            let (w, h) = ((12.0 * k) as i32 + rng.gen_range(0..4), (16.0 * k) as i32 + rng.gen_range(0..6));
            for x in -w / 2..=w / 2 {
                for y in 0..h {
                    let edge = x.abs() == w / 2 || y == h - 1;
                    b.put(x, y, if edge { STEEL } else if x == 0 { HALL } else if (x + y) % 5 == 0 { LIFE } else { QUARTERS }, !edge, !edge && rng.gen_bool(0.5), edge);
                    if !edge && y > 2 {
                        b.lift(x, y, 0.4);
                    }
                }
            }
        }
        Kind::Solar => {
            let (cols, rows) = (1 + s.min(2) + rng.gen_range(0..2), 3 + s + rng.gen_range(0..2));
            for y in 0..rows * 8 + 3 {
                b.put(0, y, STEEL, false, false, true);
            }
            for c in 0..cols {
                for side in [-1, 1] {
                    for r in 0..rows {
                        let (x0, y0) = (side * (2 + c * 11), 3 + r * 8);
                        for dx in 0..10 {
                            for dy in 0..7 {
                                let x = x0 + side * dx;
                                let frame = dx == 0 || dx == 9 || dy == 0 || dy == 6;
                                if frame {
                                    b.put(x, y0 + dy, TITANIUM, false, false, true);
                                } else {
                                    b.tinted(x, y0 + dy, COMPOSITE, panel, false);
                                }
                            }
                        }
                    }
                }
            }
        }
        Kind::Radiator => {
            let n = 4 + s * 2 + rng.gen_range(0..2);
            let len = (18.0 * k) as i32;
            for x in -(n * 2)..=(n * 2) {
                b.put(x, 0, STEEL, false, false, true);
                b.put(x, 1, HALL, false, false, false);
            }
            for i in 0..n {
                let x = (i - n / 2) * 4;
                for y in 2..len {
                    b.tinted(x, y, TITANIUM, [0.58, 0.5, 0.46], y % 4 == 0);
                }
            }
        }
        Kind::Tanks => {
            let count = 3 + s + rng.gen_range(0..2);
            let reach = (14.0 * k) as i32;
            for y in 0..reach {
                b.put(0, y, STEEL, false, false, true);
                b.put(1, y, HALL, false, false, false);
            }
            for i in 0..count {
                let cx = if i % 2 == 0 { -6 - rng.gen_range(0..3) } else { 7 + rng.gen_range(0..3) };
                let r = 4 + s + rng.gen_range(0..2);
                // Never below the base row: that's the arm.
                let cy = (4 + (i as i32) * (reach / count as i32).max(5)).max(r + 1);
                for x in -r..=r {
                    for y in -r..=r {
                        let d = ((x * x + y * y) as f32).sqrt();
                        if d <= r as f32 {
                            b.put(cx + x, cy + y, REACTOR, true, d < 1.0, d > r as f32 - 1.0);
                            b.lift(cx + x, cy + y, (1.0 - d / r as f32) * 0.7);
                        }
                    }
                }
                // its pipe to the spine
                let (a, z) = if cx < 0 { (cx + r, 0) } else { (1, cx - r) };
                for x in a..=z {
                    b.put(x, cy, HALL, false, false, false);
                }
            }
        }
        Kind::Cargo => {
            let (w, h) = ((16.0 * k) as i32, (12.0 * k) as i32);
            let colours = [[0.55, 0.28, 0.2], [0.2, 0.42, 0.45], [0.6, 0.5, 0.2], [0.32, 0.34, 0.4], [0.45, 0.22, 0.35]];
            for x in -w / 2..=w / 2 {
                b.put(x, 0, STEEL, false, false, true);
            }
            let mut bx = -w / 2;
            while bx < w / 2 - 2 {
                let mut by = 1;
                while by < h {
                    let c = colours[rng.gen_range(0..colours.len())];
                    if rng.gen_bool(0.85) {
                        for x in bx..bx + 3 {
                            for y in by..by + 2 {
                                b.tinted(x, y, CARGO, c, false);
                            }
                        }
                    }
                    by += 3;
                }
                bx += 4;
            }
            // the yard's frame, lit at intervals
            for y in 0..h + 1 {
                b.put(-w / 2 - 1, y, TITANIUM, false, y % 6 == 0, true);
                b.put(w / 2 + 1, y, TITANIUM, false, y % 6 == 0, true);
            }
        }
        Kind::Antenna => {
            let len = (26.0 * k) as i32 + rng.gen_range(0..8);
            for y in 0..len {
                b.put(0, y, TITANIUM, false, false, true);
                if y % 6 == 0 {
                    for x in [-2, -1, 1, 2] {
                        b.put(x, y, STEEL, false, false, true);
                    }
                }
            }
            let r = 5 + s * 2;
            for x in -r..=r {
                for y in -r..=r {
                    let d = ((x * x + y * y) as f32).sqrt();
                    if d <= r as f32 {
                        b.tinted(x, len + r + y - 1, TITANIUM, [0.72, 0.74, 0.8], d < 1.0);
                        b.lift(x, len + r + y - 1, 0.5 * (1.0 - d / r as f32));
                    }
                }
            }
        }
        Kind::Greenhouse => {
            let r = (8.0 * k) as i32 + rng.gen_range(0..3);
            for y in 0..4 {
                b.put(0, y, HALL, false, false, false);
                b.put(-1, y, STEEL, false, false, true);
                b.put(1, y, STEEL, false, false, true);
            }
            for x in -r..=r {
                for y in -r..=r {
                    let d = ((x * x + y * y) as f32).sqrt();
                    if d <= r as f32 {
                        let rim = d > r as f32 - 1.5;
                        if rim {
                            b.put(x, r + 3 + y, TITANIUM, false, false, true);
                        } else {
                            b.tinted(x, r + 3 + y, LAB, glass, rng.gen_bool(0.3));
                        }
                        b.lift(x, r + 3 + y, 0.5 * (1.0 - d / r as f32));
                    }
                }
            }
        }
        Kind::Crane => {
            let len = (22.0 * k) as i32;
            for y in 0..len {
                for x in [-2, 2] {
                    b.tinted(x, y, STEEL, [0.72, 0.58, 0.24], false);
                }
                if y % 4 == 0 {
                    for x in -1..=1 {
                        b.tinted(x, y, STEEL, [0.68, 0.54, 0.22], false);
                    }
                }
            }
            for x in -(8 + s * 3)..=(8 + s * 3) {
                b.tinted(x, len, STEEL, [0.72, 0.58, 0.24], x.abs() == 8 + s * 3);
            }
        }
    }
    let d = b.done();
    register(key, || d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn sections_sit_on_their_base_row_and_grow_outward() {
        let mut rng = StdRng::seed_from_u64(1);
        for kind in ALL_KINDS {
            for size in [0.0, 0.5, 1.0] {
                let key = section(kind, size, 0, &mut rng);
                let d = get(key).expect("registered");
                assert_eq!(d.min.y, 0, "{kind:?} doesn't start at the arm");
                assert!(d.max.y > 2, "{kind:?} has no reach");
                // Something at the base row near the middle, to bolt on by.
                assert!(d.cells.iter().any(|c| c.at.y == 0 && c.at.x.abs() <= 2), "{kind:?} touches the arm nowhere near its middle");
            }
        }
    }

    #[test]
    fn every_hub_shape_is_one_piece() {
        let mut rng = StdRng::seed_from_u64(3);
        for kind in [HubKind::Octagon, HubKind::Wheel, HubKind::Hex, HubKind::Spindle] {
            let d = get(hub(kind, 70, &mut rng)).unwrap();
            let cells: std::collections::HashSet<IVec2> = d.cells.iter().map(|c| c.at).collect();
            let mut seen = std::collections::HashSet::new();
            let mut stack = vec![IVec2::ZERO];
            while let Some(p) = stack.pop() {
                if !cells.contains(&p) || !seen.insert(p) {
                    continue;
                }
                for d in [IVec2::X, IVec2::Y, IVec2::NEG_X, IVec2::NEG_Y] {
                    stack.push(p + d);
                }
            }
            assert_eq!(seen.len(), cells.len(), "{kind:?} hub comes apart");
        }
    }

    #[test]
    fn big_sections_are_bigger() {
        let mut rng = StdRng::seed_from_u64(2);
        let small = get(section(Kind::Habitat, 0.0, 1, &mut rng)).unwrap().span();
        let big = get(section(Kind::Habitat, 1.0, 1, &mut rng)).unwrap().span();
        assert!(big.x * big.y > small.x * small.y * 2);
    }
}
