//! The title screen's nebula: one per playthrough, drained by the Cascade.
//!
//! A port of `tools/art/nebula.py`, which has the reasoning at length. In
//! short, it is coloured the way NASA's pictures are, not the way the eye sees:
//! three emission gases (sulfur, hydrogen, oxygen) are separate noise layers,
//! each mapped to a colour by the palette, as the Hubble palette maps SII,
//! H-alpha and OIII to red, green and blue. Dark dust is ridged noise that eats
//! the gas and catches light along one edge. Stars go on top, the brightest
//! with diffraction spikes (six for Webb, four for Hubble).
//!
//! The Cascade drains it: past a front that comes in from the right,
//! everything goes grey and dark, with a faint violet seam at the front. Late
//! on, a pocket of colour stays round the player.
//!
//! Rendered on its own thread at 960x540 (a fraction of a second) and faded
//! in, so the menu is never held up by it.

use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

pub const W: usize = 960;
pub const H: usize = 540;
/// The Python tool's sizes are in pixels of a 1280-wide picture.
const K: f32 = W as f32 / 1280.0;

/// Where the player sits on the picture, as fractions: the pocket of colour
/// the Cascade leaves, and the "you" mark on the menu.
pub const YOU: (f32, f32) = (0.6, 0.56);

struct Palette {
    gases: [[f32; 3]; 3],
    dust: [f32; 3],
    spikes: u32,
}

const PALETTES: [Palette; 4] = [
    // Hubble: gold and teal.
    Palette { gases: [[1.00, 0.62, 0.22], [0.95, 0.80, 0.40], [0.20, 0.62, 0.95]], dust: [0.30, 0.16, 0.08], spikes: 4 },
    // Webb's Cosmic Cliffs: rust and blue.
    Palette { gases: [[0.95, 0.48, 0.20], [0.85, 0.58, 0.32], [0.30, 0.55, 1.00]], dust: [0.36, 0.20, 0.10], spikes: 6 },
    // HOO: red and cyan.
    Palette { gases: [[0.95, 0.22, 0.30], [0.90, 0.30, 0.38], [0.25, 0.85, 0.95]], dust: [0.20, 0.08, 0.10], spikes: 4 },
    // Veil: violet and rose.
    Palette { gases: [[0.70, 0.30, 0.95], [0.95, 0.35, 0.65], [0.30, 0.75, 0.90]], dust: [0.16, 0.08, 0.20], spikes: 6 },
];

/// Per-axis sample positions for upscaling a grid of `n` cells to `len`
/// pixels: the lower index and a smoothed weight toward the next.
fn axis(n: usize, len: usize) -> Vec<(usize, usize, f32)> {
    (0..len)
        .map(|i| {
            let f = ((i as f32 + 0.5) * n as f32 / len as f32 - 0.5).clamp(0.0, (n - 1) as f32);
            let a = f.floor() as usize;
            let b = (a + 1).min(n - 1);
            let t = f - a as f32;
            (a, b, t * t * (3.0 - 2.0 * t))
        })
        .collect()
}

/// Fractal value noise: random grids upsampled smooth and summed.
fn noise(rng: &mut StdRng, scale: f32, octaves: u32, persistence: f32) -> Vec<f32> {
    let scale = scale * K;
    let mut out = vec![0.0f32; W * H];
    let (mut amp, mut total) = (1.0f32, 0.0f32);
    for o in 0..octaves {
        let gw = ((W as f32 / scale * 2f32.powi(o as i32)) as usize).max(2);
        let gh = ((H as f32 / scale * 2f32.powi(o as i32)) as usize).max(2);
        let grid: Vec<f32> = (0..gw * gh).map(|_| rng.r#gen::<f32>()).collect();
        let xs = axis(gw, W);
        let ys = axis(gh, H);
        for (y, &(y0, y1, ty)) in ys.iter().enumerate() {
            let (r0, r1) = (&grid[y0 * gw..y0 * gw + gw], &grid[y1 * gw..y1 * gw + gw]);
            let row = &mut out[y * W..y * W + W];
            for (x, &(x0, x1, tx)) in xs.iter().enumerate() {
                let top = r0[x0] + (r0[x1] - r0[x0]) * tx;
                let bot = r1[x0] + (r1[x1] - r1[x0]) * tx;
                row[x] += amp * (top + (bot - top) * ty);
            }
        }
        total += amp;
        amp *= persistence;
    }
    for v in &mut out {
        *v /= total;
    }
    out
}

/// Sample `field` at positions pushed by two other fields (a domain warp).
fn warp(field: &[f32], dx: &[f32], dy: &[f32], strength: f32) -> Vec<f32> {
    let s = strength * K;
    let mut out = vec![0.0f32; W * H];
    for y in 0..H {
        for x in 0..W {
            let i = y * W + x;
            let sx = (x as f32 + (dx[i] - 0.5) * s).clamp(0.0, (W - 1) as f32) as usize;
            let sy = (y as f32 + (dy[i] - 0.5) * s).clamp(0.0, (H - 1) as f32) as usize;
            out[i] = field[sy * W + sx];
        }
    }
    out
}

/// One box-blur pass of radius `r` along rows (`horizontal`) or columns,
/// edges held.
fn box_pass(a: &[f32], r: usize, horizontal: bool) -> Vec<f32> {
    let (outer, inner) = if horizontal { (H, W) } else { (W, H) };
    let at = |o: usize, i: usize| if horizontal { o * W + i } else { i * W + o };
    let mut out = vec![0.0f32; W * H];
    let n = (2 * r + 1) as f32;
    for o in 0..outer {
        let get = |i: isize| a[at(o, i.clamp(0, inner as isize - 1) as usize)];
        let mut sum: f32 = (-(r as isize)..=r as isize).map(get).sum();
        for i in 0..inner {
            out[at(o, i)] = sum / n;
            sum += get(i as isize + r as isize + 1) - get(i as isize - r as isize);
        }
    }
    out
}

/// Close to a Gaussian of radius `r` (in 1280-wide pixels): three box passes
/// each way.
fn blur(a: &[f32], r: f32) -> Vec<f32> {
    let k = ((r * K * 0.6).round() as usize).max(1);
    let mut v = a.to_vec();
    for _ in 0..3 {
        v = box_pass(&box_pass(&v, k, true), k, false);
    }
    v
}

/// The picture, as sRGB bytes (RGBA, W x H). `drain` is the Cascade level,
/// 0..1.
pub fn render(seed: u64, drain: f32) -> Vec<u8> {
    let mut rng = StdRng::seed_from_u64(seed);
    let pal = &PALETTES[(seed % PALETTES.len() as u64) as usize];
    let (wf, hf) = (W as f32, H as f32);

    // Where the cloud sits: right of centre, a tilted blob, so the left side
    // stays dark for the title.
    let cx = wf * rng.gen_range(0.66..0.76);
    let cy = hf * rng.gen_range(0.42..0.58);
    let ang: f32 = rng.gen_range(-0.6..0.6);
    let (su, sv) = (wf * rng.gen_range(0.30..0.40), hf * rng.gen_range(0.38..0.52));
    let (ca, sa) = (ang.cos(), ang.sin());
    let mut shape = vec![0.0f32; W * H];
    for y in 0..H {
        for x in 0..W {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            let u = (dx * ca + dy * sa) / su;
            let v = (-dx * sa + dy * ca) / sv;
            shape[y * W + x] = (-(u * u + v * v) * 1.6).exp();
        }
    }

    let wx = noise(&mut rng, 300.0, 4, 0.55);
    let wy = noise(&mut rng, 300.0, 4, 0.55);
    let mut emission: Vec<Vec<f32>> = Vec::with_capacity(3);
    for _ in 0..3 {
        let scale = rng.gen_range(160.0..260.0);
        let strength = rng.gen_range(160.0..260.0);
        let power: f32 = rng.gen_range(1.5..2.1);
        let gain: f32 = rng.gen_range(0.8..1.25);
        let mut n = warp(&noise(&mut rng, scale, 7, 0.55), &wx, &wy, strength);
        for (v, s) in n.iter_mut().zip(&shape) {
            *v = ((*v - 0.32) * 2.1).max(0.0).powf(power) * s * gain;
        }
        emission.push(n);
    }

    // Dust: ridged noise, thickest where it folds; lit along its upper edge.
    let d = warp(&noise(&mut rng, 220.0, 7, 0.55), &wy, &wx, 220.0);
    let raw: Vec<f32> = d
        .iter()
        .zip(&shape)
        .map(|(v, s)| {
            let ridge = 1.0 - (v * 2.0 - 1.0).abs();
            ((ridge - 0.66) * 3.2).clamp(0.0, 1.0).powf(1.4) * (s * 1.6).clamp(0.0, 1.0)
        })
        .collect();
    let dust = blur(&raw, 2.0);
    let soft = blur(&dust, 3.0);
    let shift = ((4.0 * K).round() as usize).max(1);
    let rim: Vec<f32> = (0..W * H)
        .map(|i| {
            let above = if i >= shift * W { soft[i - shift * W] } else { soft[i + (H - shift) * W] };
            (dust[i] - above).clamp(0.0, 1.0) * 2.2
        })
        .collect();

    let total: Vec<f32> = (0..W * H).map(|i| emission[0][i] + emission[1][i] + emission[2][i]).collect();
    let glow = blur(&total, 40.0);
    let haze = blur(&total, 14.0);

    let mut rgb = vec![[0.0f32; 3]; W * H];
    for i in 0..W * H {
        let mut c = [0.0f32; 3];
        for (g, gas) in pal.gases.iter().enumerate() {
            for ch in 0..3 {
                c[ch] += emission[g][i] * gas[ch];
            }
        }
        let x = (i % W) as f32;
        for ch in 0..3 {
            c[ch] *= 1.0 - dust[i] * 0.9;
            c[ch] += dust[i] * 0.08 * pal.dust[ch];
            c[ch] += rim[i] * shape[i] * pal.gases[0][ch] * 0.6;
            c[ch] += glow[i] * pal.gases[2][ch] * 0.3;
            c[ch] += haze[i] * pal.gases[1][ch] * 0.08;
            // Tone map.
            c[ch] = 1.0 - (-c[ch] * 1.6).exp();
        }
        // More colour, then darken the left so the title reads.
        let grey = (c[0] + c[1] + c[2]) / 3.0;
        let left = ((x / wf - 0.18) * 1.9).clamp(0.08, 1.0);
        for ch in 0..3 {
            c[ch] = (grey + (c[ch] - grey) * 1.55).clamp(0.0, 1.0) * left;
        }
        rgb[i] = c;
    }

    // Stars: most faint, a few bright, the brightest with spikes.
    let mut stars = vec![0.0f32; W * H];
    let count = (4200.0 * (W * H) as f32 / (1280.0 * 720.0)) as usize;
    let mut placed: Vec<(usize, usize, f32)> = Vec::with_capacity(count);
    for _ in 0..count {
        let (sx, sy) = (rng.gen_range(0..W), rng.gen_range(0..H));
        // Pareto (Lomax) with shape 2.2, as numpy draws it.
        let u: f32 = rng.r#gen();
        let mag = ((1.0 - u).powf(-1.0 / 2.2) - 1.0) * 0.18 + 0.05;
        stars[sy * W + sx] = mag.min(3.0);
        placed.push((sx, sy, mag));
    }
    let halo = blur(&stars, 1.6);
    let mut light: Vec<f32> = (0..W * H).map(|i| stars[i] * 0.9 + halo[i] * 1.4).collect();
    placed.sort_by(|a, b| b.2.total_cmp(&a.2));
    for &(x0, y0, m) in placed.iter().filter(|s| s.0 as f32 > wf * 0.45).take(5) {
        let len = ((10.0 + 22.0 * m.min(2.0)) * K) as i32;
        for k in 0..pal.spikes {
            let base = if pal.spikes == 4 { std::f32::consts::FRAC_PI_2 } else { 0.0 };
            let a = std::f32::consts::PI * k as f32 / (pal.spikes as f32 / 2.0) + base;
            for t in 1..len {
                let px = (x0 as f32 + a.cos() * t as f32) as i32;
                let py = (y0 as f32 + a.sin() * t as f32) as i32;
                if (0..W as i32).contains(&px) && (0..H as i32).contains(&py) {
                    let f = 1.0 - t as f32 / len as f32;
                    light[py as usize * W + px as usize] += m * 0.35 * f * f;
                }
            }
        }
    }
    let tint = [0.92f32, 0.95, 1.0];
    for i in 0..W * H {
        let s = light[i].clamp(0.0, 1.0);
        for ch in 0..3 {
            rgb[i][ch] = 1.0 - (1.0 - rgb[i][ch]) * (1.0 - s * tint[ch]);
        }
    }

    if drain > 0.0 {
        // The dark comes in from the right: grey, then black, with a faint
        // violet seam at its front. A pocket of colour stays round the player.
        let edge = noise(&mut rng, 120.0, 4, 0.55);
        let front = wf * (1.0 - drain * 0.95);
        let (px, py) = (wf * YOU.0, hf * YOU.1);
        for y in 0..H {
            for x in 0..W {
                let i = y * W + x;
                let at = x as f32 - front + edge[i] * 140.0 * K;
                let mut t = (at / (160.0 * K)).clamp(0.0, 1.0);
                if drain > 0.6 {
                    let (ex, ey) = ((x as f32 - px) / (150.0 * K), (y as f32 - py) / (120.0 * K));
                    t = t.min(1.0 - (-(ex * ex + ey * ey)).exp());
                }
                let c = rgb[i];
                let grey = (c[0] + c[1] + c[2]) / 3.0;
                let seam = (-(at / (24.0 * K)).powi(2)).exp() * 0.10;
                let violet = [0.45f32, 0.32, 0.75];
                for ch in 0..3 {
                    rgb[i][ch] = c[ch] * (1.0 - t) + grey * 0.16 * t + seam * violet[ch];
                }
            }
        }
    }

    // Dithered to bytes: the drained side is a slow ramp through the darkest
    // greys, and plain rounding drew it as visible bands.
    let mut bytes = Vec::with_capacity(W * H * 4);
    for (i, c) in rgb.into_iter().enumerate() {
        let mut h = (i as u32).wrapping_mul(0x9E37_79B9) ^ (seed as u32);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        let d = (h & 0xFFFF) as f32 / 65535.0;
        for ch in c {
            bytes.push((ch.clamp(0.0, 1.0) * 255.0 + d).min(255.0) as u8);
        }
        bytes.push(255);
    }
    bytes
}

/// Marks the title screen's picture. Holds the picture while it is being
/// painted on its own thread.
#[derive(Component)]
pub struct MenuNebula {
    pending: Arc<Mutex<Option<Vec<u8>>>>,
    shown_for: f32,
}

impl MenuNebula {
    /// Start painting one. The menu shows black until it is ready.
    pub fn paint(seed: u64, drain: f32) -> Self {
        let pending = Arc::new(Mutex::new(None));
        let slot = pending.clone();
        std::thread::spawn(move || {
            let bytes = render(seed, drain);
            if let Ok(mut s) = slot.lock() {
                *s = Some(bytes);
            }
        });
        Self { pending, shown_for: -1.0 }
    }
}

/// Put the picture up when it's painted, and fade it in.
pub fn show_menu_nebula(
    time: Res<Time<Real>>,
    mut images: ResMut<Assets<Image>>,
    mut q: Query<(&mut MenuNebula, &mut ImageNode)>,
) {
    for (mut neb, mut node) in q.iter_mut() {
        if neb.shown_for < 0.0 {
            let ready = neb.pending.lock().ok().and_then(|mut s| s.take());
            if let Some(bytes) = ready {
                let image = Image::new(
                    Extent3d { width: W as u32, height: H as u32, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    bytes,
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::RENDER_WORLD,
                );
                node.image = images.add(image);
                neb.shown_for = 0.0;
            }
            continue;
        }
        if neb.shown_for < 1.5 {
            neb.shown_for += time.delta_secs();
            let t = (neb.shown_for / 1.5).clamp(0.0, 1.0);
            node.color = Color::srgba(1.0, 1.0, 1.0, t * t * (3.0 - 2.0 * t));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_paints_the_same_picture() {
        assert_eq!(render(7, 0.3), render(7, 0.3));
    }

    #[test]
    fn the_cascade_takes_colour_from_the_right() {
        let colour = |bytes: &[u8], x0: usize, x1: usize| -> f32 {
            let mut sum = 0.0;
            for y in 0..H {
                for x in x0..x1 {
                    let i = (y * W + x) * 4;
                    let (r, g, b) = (bytes[i] as f32, bytes[i + 1] as f32, bytes[i + 2] as f32);
                    sum += r.max(g).max(b) - r.min(g).min(b);
                }
            }
            sum
        };
        let clean = render(3, 0.0);
        let drained = render(3, 0.5);
        let right = W * 9 / 10;
        assert!(colour(&drained, right, W) < colour(&clean, right, W) * 0.3);
    }
}

#[cfg(test)]
mod dump {
    /// `AFTERLIGHT_NEBULA_DUMP=<dir> cargo test nebula_dump -- --ignored`
    /// writes a few pictures as PPM, to look at outside the game.
    #[test]
    #[ignore]
    fn nebula_dump() {
        let dir = std::env::var("AFTERLIGHT_NEBULA_DUMP").unwrap_or_else(|_| "/tmp".into());
        for (seed, drain) in [(4u64, 0.0f32), (4, 0.5), (4, 0.85), (1, 0.0), (2, 0.0), (3, 0.0)] {
            let bytes = super::render(seed, drain);
            let mut out = format!("P6\n{} {}\n255\n", super::W, super::H).into_bytes();
            for px in bytes.chunks(4) {
                out.extend_from_slice(&px[..3]);
            }
            std::fs::write(format!("{dir}/nebula_{seed}_{drain}.ppm"), out).unwrap();
        }
    }
}
