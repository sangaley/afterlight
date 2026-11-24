//! Generated liveries: a paint job of its own for every ship.
//!
//! A seed picks a palette (a hull colour, a panel colour and three accents)
//! and a mix of the paint editor's decals laid over the ship's own shape:
//! one big motif -- a swept arrow, bands, a painted nose, coloured flanks, a
//! checkered stern -- and a few details over it -- pinstripes, hazard
//! hatching, spine chevrons, a roundel, running lights. Laid mirror-wise
//! port and starboard, as a painter would. Two ships of the same design come
//! out different; the same seed always comes out the same.

use bevy::platform::collections::HashSet;

use bevy::prelude::*;

use super::ship_roof::{Decal, DecalShape, Scheme};

/// A livery's colours, sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// The hull plating.
    pub main: [u8; 3],
    /// Panels and the darker plating.
    pub second: [u8; 3],
    /// What the decals are painted in; the first is the trim too.
    pub accents: [[u8; 3]; 3],
}

impl Palette {
    pub fn scheme(&self) -> Scheme {
        let v = |c: [u8; 3]| Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) / 255.0;
        Scheme { main: v(self.main), second: v(self.second), trim: v(self.accents[0]), warm: false }
    }
}

const fn pal(main: [u8; 3], second: [u8; 3], a: [u8; 3], b: [u8; 3], c: [u8; 3]) -> Palette {
    Palette { main, second, accents: [a, b, c] }
}

/// The ready-made palettes a livery is drawn from.
pub const PALETTES: [Palette; 14] = [
    // Hauler: blue-grey with hazard yellow and black.
    pal([96, 108, 124], [52, 58, 68], [236, 186, 36], [24, 24, 28], [226, 222, 210]),
    // Racer: white with navy, red and gold.
    pal([210, 212, 214], [40, 54, 92], [196, 38, 40], [32, 72, 168], [232, 176, 48]),
    // Teal and orange.
    pal([44, 104, 108], [34, 44, 50], [236, 120, 36], [236, 226, 200], [96, 214, 206]),
    // Olive drab with tan and a red stripe.
    pal([92, 98, 64], [52, 56, 38], [196, 168, 116], [176, 44, 34], [230, 226, 212]),
    // Royal: deep purple with gold.
    pal([76, 52, 110], [36, 30, 50], [226, 178, 64], [234, 226, 214], [168, 132, 220]),
    // Ice: pale blue-grey with cyan and navy.
    pal([168, 186, 200], [70, 86, 104], [56, 196, 232], [24, 44, 86], [244, 248, 250]),
    // Ember: charcoal with rust, orange and yellow.
    pal([66, 64, 66], [120, 52, 32], [230, 104, 30], [246, 196, 52], [196, 44, 28]),
    // Mint with coral.
    pal([112, 176, 150], [38, 74, 66], [244, 112, 92], [244, 240, 226], [250, 206, 70]),
    // Desert: sand with maroon and turquoise.
    pal([186, 160, 118], [104, 76, 52], [126, 32, 40], [48, 170, 168], [240, 230, 204]),
    // Crimson with black and white.
    pal([140, 30, 36], [32, 28, 30], [236, 232, 224], [222, 172, 60], [40, 40, 44]),
    // Corporate: light grey with blue and orange.
    pal([184, 188, 192], [90, 98, 110], [36, 108, 200], [236, 128, 40], [60, 170, 90]),
    // Toxic: gunmetal with lime and magenta.
    pal([58, 62, 66], [26, 28, 30], [162, 230, 40], [214, 50, 160], [236, 236, 236]),
    // Sky: blue with white and red.
    pal([62, 112, 176], [30, 48, 80], [240, 240, 236], [212, 50, 44], [250, 204, 60]),
    // Forest: green with cream and copper.
    pal([54, 94, 64], [28, 44, 32], [232, 220, 186], [196, 110, 56], [218, 196, 64]),
];

/// splitmix64: small, fast and good enough to roll paint with.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// 0..n.
    fn below(&mut self, n: i32) -> i32 {
        if n <= 0 {
            0
        } else {
            (self.next() % n as u64) as i32
        }
    }
    /// lo..=hi.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below(hi - lo + 1)
    }
    fn chance(&mut self, p: f32) -> bool {
        (self.next() % 10_000) as f32 / 10_000.0 < p
    }
}

/// A seed from an entity, for a ship with no seed of its own.
pub fn seed_of(entity: Entity) -> u64 {
    Rng::new(entity.to_bits()).next()
}

/// A palette from the seed.
pub fn palette(seed: u64) -> Palette {
    let mut rng = Rng::new(seed.wrapping_mul(31));
    let mut p = PALETTES[rng.below(PALETTES.len() as i32) as usize];
    // The accents in another order now and then, so the same palette paints
    // its ships differently.
    let k = rng.below(3) as usize;
    p.accents.swap(0, k);
    p
}

/// A faction ship's palette: the faction's own colour on the hull and its
/// accent first, so a ship still says whose it is, with two accents more.
///
/// The two more are tones of the faction's own two colours, never a hue of
/// their own: the playtester found the old picks (lime, coral, magenta from
/// the ready-made palettes) fought the faction's colours. Ships still differ
/// by which tones the seed picks and by the decals' shapes.
pub fn faction_palette(seed: u64, base: [u8; 3], accent: [u8; 3]) -> Palette {
    let darker = [(base[0] as f32 * 0.5) as u8, (base[1] as f32 * 0.5) as u8, (base[2] as f32 * 0.52) as u8];
    let tones = faction_tones(base, accent);
    let mut rng = Rng::new(seed.wrapping_mul(31));
    let n = tones.len() as i32;
    let first = rng.below(n) as usize;
    let second = (first + 1 + rng.below(n - 1) as usize) % tones.len();
    Palette { main: base, second: darker, accents: [accent, tones[first], tones[second]] }
}

/// The colours a faction's decals may use besides its accent: its accent
/// deepened and paled, its hull colour lifted, and the two mixed.
pub fn faction_tones(base: [u8; 3], accent: [u8; 3]) -> [[u8; 3]; 4] {
    let mix = |a: [u8; 3], b: [u8; 3], t: f32| {
        [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round().clamp(0.0, 255.0) as u8)
    };
    const WHITE: [u8; 3] = [236, 234, 228];
    const BLACK: [u8; 3] = [18, 18, 20];
    [mix(accent, BLACK, 0.35), mix(accent, WHITE, 0.4), mix(base, WHITE, 0.45), mix(base, accent, 0.5)]
}

/// The decals of a seed's livery in `palette`, laid over the ship's `cells`
/// (bow toward +x).
pub fn livery_decals(seed: u64, palette: &Palette, cells: &HashSet<IVec2>) -> Vec<Decal> {
    let Some(first) = cells.iter().next() else { return Vec::new() };
    let (lo, hi) = cells.iter().fold((*first, *first), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
    let mut l = Lay { cells, s: lo.y + hi.y, out: Vec::new() };
    let mut rng = Rng::new(seed);
    let len = hi.x - lo.x + 1;
    let wide = (hi.y - lo.y + 2) / 2;
    // Colours by job: the motif, the lines, the symbols.
    let mut order = [0usize, 1, 2];
    for i in (1..3).rev() {
        order.swap(i, rng.below(i as i32 + 1) as usize);
    }
    let [big, line, mark] = order.map(|i| palette.accents[i]);
    // Port half, nose to stern; the starboard half is its mirror.
    let mut upper: Vec<IVec2> = cells.iter().copied().filter(|c| 2 * c.y >= l.s).collect();
    upper.sort_by_key(|c| (-c.x, c.y));
    let mid2 = l.s;
    let dy = move |c: IVec2| (2 * c.y - mid2).abs() / 2;

    // --- The motif ---------------------------------------------------------
    let motif = rng.below(6);
    match motif {
        // An arrow (or two) swept back from the spine, pointing at the bow.
        0 | 1 => {
            let n = if motif == 1 && len >= 14 { 2 } else { 1 };
            let mut a = rng.range(len / 6, len / 3);
            for k in 0..n {
                let w = rng.range(2, 4);
                let colour = if k == 0 { big } else { line };
                for &c in &upper {
                    let v = hi.x - c.x - dy(c);
                    if (a..a + w).contains(&v) {
                        l.put(c, DecalShape::Full, 0, colour, 1);
                    } else if v == a - 1 && dy(c) > 0 {
                        l.put(c, DecalShape::Diagonal, 0, colour, 1);
                    } else if v == a + w && dy(c) > 0 {
                        l.put(c, DecalShape::Diagonal, 2, colour, 1);
                    }
                }
                a += w + rng.range(2, 4);
            }
        }
        // Bands across the hull, each with its edges in the line colour.
        2 => {
            let n = rng.range(1, 3);
            let w = rng.range(1, 3);
            let gap = (len / (n + 1)).max(w + 1);
            for k in 1..=n {
                let x0 = hi.x - k * gap;
                for &c in upper.iter().filter(|c| (x0 - w + 1..=x0).contains(&c.x)) {
                    l.put(c, DecalShape::Full, 0, big, 1);
                    if c.x == x0 {
                        l.put(c, DecalShape::Edge, 1, line, 2);
                    }
                    if c.x == x0 - w + 1 {
                        l.put(c, DecalShape::Edge, 3, line, 2);
                    }
                }
            }
        }
        // The nose painted, its back edge swept.
        3 => {
            let n = rng.range(len / 6, len / 3).max(2);
            for &c in &upper {
                let u = hi.x - c.x + dy(c);
                if u < n {
                    l.put(c, DecalShape::Full, 0, big, 1);
                } else if u == n {
                    l.put(c, DecalShape::Diagonal, 1, big, 1);
                }
            }
        }
        // The flanks painted along their outer edge.
        4 => {
            let t = rng.range(1, 2.max(wide / 3));
            let from = hi.x - rng.range(0, len / 4);
            for &c in upper.iter().filter(|c| c.x <= from) {
                let edge = !cells.contains(&(c + IVec2::Y)) || wide - dy(c) <= t;
                if edge && dy(c) > 0 {
                    l.put(c, DecalShape::Full, 0, big, 1);
                }
            }
        }
        // A checkered stern.
        _ => {
            let n = rng.range(2, (len / 4).max(2));
            for &c in upper.iter().filter(|c| c.x < lo.x + n) {
                if (c.x + c.y).rem_euclid(2) == 0 {
                    l.put(c, DecalShape::Full, 0, big, 1);
                }
            }
        }
    }

    // --- Details over it ---------------------------------------------------
    let mut details = [0, 1, 2, 3, 4];
    for i in (1..5).rev() {
        details.swap(i, rng.below(i as i32 + 1) as usize);
    }
    let count = rng.range(2, 3);
    for &d in details.iter().take(count as usize) {
        match d {
            // A pinstripe (or two) along each flank.
            0 if wide >= 3 => {
                let y = rng.range(1, wide - 2);
                let (x0, x1) = (lo.x + rng.range(1, len / 5), hi.x - rng.range(1, len / 4));
                let double = rng.chance(0.5);
                for &c in upper.iter().filter(|c| dy(**c) == y && (x0..=x1).contains(&c.x)) {
                    l.put(c, DecalShape::Stripe, 0, line, 2);
                    if double && cells.contains(&(c + IVec2::Y)) {
                        l.put(c + IVec2::Y, DecalShape::Stripe, 0, mark, 2);
                    }
                }
            }
            // A run of hazard hatching between two edge lines.
            1 if wide >= 3 => {
                let y = rng.range(1, wide - 1);
                let run = rng.range(3, (len / 3).max(3));
                let x1 = hi.x - rng.range(len / 4, len / 2);
                for &c in upper.iter().filter(|c| dy(**c) == y && (x1 - run..=x1).contains(&c.x)) {
                    l.put(c, DecalShape::Slash, 0, line, 2);
                    l.put(c, DecalShape::Edge, 0, line, 2);
                    l.put(c, DecalShape::Edge, 2, line, 2);
                }
            }
            // Chevrons down the spine, pointing at the bow.
            2 => {
                let step = rng.range(2, 4);
                let (x0, x1) = (lo.x + len / 5, hi.x - rng.range(1, 3));
                for &c in upper.iter().filter(|c| dy(**c) == 0 && (x0..=x1).contains(&c.x) && (hi.x - c.x) % step == 0) {
                    l.put(c, DecalShape::Chevron, 1, mark, 3);
                }
            }
            // A roundel or a diamond: on the spine where the ship has two
            // middle rows, else one on each flank.
            3 => {
                let shape = if rng.chance(0.6) { DecalShape::Arc } else { DecalShape::Diagonal };
                let x = hi.x - rng.range(len / 4, len * 2 / 3);
                let y = if l.s.rem_euclid(2) == 1 { (l.s + 1) / 2 } else { (l.s / 2) + rng.range(2, (wide - 2).max(2)) };
                // The four cells round the corner at (x - 1/2, y - 1/2).
                for (dx, dyy, turn) in [(0, 0, 0u8), (-1, 0, 1), (-1, -1, 2), (0, -1, 3)] {
                    let c = IVec2::new(x + dx, y + dyy);
                    // Below the middle it's the mirror of the half above.
                    if 2 * c.y >= l.s {
                        l.put(c, shape, turn, mark, 3);
                    }
                }
            }
            // Running lights down each flank.
            _ => {
                let y = if wide >= 2 { rng.range(1, wide - 1) } else { 0 };
                let step = rng.range(2, 3);
                for &c in upper.iter().filter(|c| dy(**c) == y && (hi.x - c.x) % step == 1) {
                    l.put(c, DecalShape::Dot, 0, mark, 3);
                }
            }
        }
    }
    l.out
}

/// Decals going down, each with its mirror across the ship's middle.
struct Lay<'a> {
    cells: &'a HashSet<IVec2>,
    /// lo.y + hi.y: the mirror of row y is row s - y.
    s: i32,
    out: Vec<Decal>,
}

impl Lay<'_> {
    fn put(&mut self, cell: IVec2, shape: DecalShape, turn: u8, colour: [u8; 3], layer: u8) {
        let d = Decal { cell, shape, turn, flip: false, colour, layer };
        if self.cells.contains(&cell) && !self.out.contains(&d) {
            self.out.push(d);
        }
        let m = IVec2::new(cell.x, self.s - cell.y);
        if m != cell && self.cells.contains(&m) {
            let d = d.mirrored(m);
            if !self.out.contains(&d) {
                self.out.push(d);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hull() -> HashSet<IVec2> {
        let mut c = HashSet::new();
        for x in -12..=14 {
            let w = if x > 8 { 14 - x } else { 7 };
            for y in -w..=w {
                c.insert(IVec2::new(x, y));
            }
        }
        c
    }

    #[test]
    fn a_seed_always_paints_the_same_and_seeds_differ() {
        let cells = hull();
        let a = livery_decals(7, &palette(7), &cells);
        assert_eq!(a, livery_decals(7, &palette(7), &cells));
        let looks: HashSet<String> = (0..24).map(|s| format!("{:?}", livery_decals(s, &palette(s), &cells))).collect();
        assert!(looks.len() >= 20, "only {} different liveries in 24 seeds", looks.len());
    }

    #[test]
    fn a_livery_stays_on_the_ship_and_is_mirrored() {
        let cells = hull();
        for seed in 0..40 {
            let out = livery_decals(seed, &palette(seed), &cells);
            assert!(!out.is_empty(), "seed {seed} painted nothing");
            for d in &out {
                assert!(cells.contains(&d.cell));
                if d.cell.y != 0 {
                    let m = IVec2::new(d.cell.x, -d.cell.y);
                    assert!(out.iter().any(|e| e.cell == m && e.shape == d.shape && e.layer == d.layer), "seed {seed}: {d:?} has no mirror");
                }
            }
        }
    }
}
