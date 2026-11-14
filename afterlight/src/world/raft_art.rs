//! Drawing the Raft: every design it uses baked once into a painted picture,
//! on its own thread, then shown as one sprite per moored ship (and one more
//! for its lights) -- a hundred ships is a few hundred sprites, not a hundred
//! thousand blocks.
//!
//! The painting follows the previews the playtester picked: the design's own
//! block sprites as the detail, pulled toward one gunmetal so every ship in
//! the Raft looks like it belongs to the others; each hull domed and lit from
//! the upper left; its own shadow cast to the lower right; and a separate
//! layer of lit windows, portholes and red/green running lights with their
//! glow, which also spills back onto the hull round them.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::raft::{library, turn, Design, RaftPlan, CELL};

/// Picture pixels a block: about 11 m a pixel, sharp at docking zoom.
pub const PX: usize = 6;
/// Room round a hull for its shadow and glow.
const PAD: usize = 10;

/// One design, painted: the hull (with its shadow) and its lights.
pub struct Baked {
    pub body: Image,
    pub lights: Image,
    /// World size of either picture.
    pub size: Vec2,
}

// ---------------------------------------------------------------------------
// small image maths

struct Plane {
    w: usize,
    h: usize,
    v: Vec<f32>,
}

impl Plane {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, v: vec![0.0; w * h] }
    }
    fn at(&self, x: isize, y: isize) -> f32 {
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        self.v[y * self.w + x]
    }
    /// Close to a Gaussian of radius r: three box passes each way.
    fn blur(&self, r: f32) -> Plane {
        let k = ((r * 0.6).round() as isize).max(1);
        let mut cur = Plane { w: self.w, h: self.h, v: self.v.clone() };
        for _ in 0..3 {
            for horizontal in [true, false] {
                let mut out = Plane::new(self.w, self.h);
                let n = (2 * k + 1) as f32;
                let (outer, inner) = if horizontal { (self.h, self.w) } else { (self.w, self.h) };
                for o in 0..outer {
                    let get = |i: isize| if horizontal { cur.at(i, o as isize) } else { cur.at(o as isize, i) };
                    let mut sum: f32 = (-k..=k).map(get).sum();
                    for i in 0..inner {
                        let idx = if horizontal { o * self.w + i } else { i * self.w + o };
                        out.v[idx] = sum / n;
                        sum += get(i as isize + k + 1) - get(i as isize - k);
                    }
                }
                cur = out;
            }
        }
        cur
    }
    fn shifted(&self, dx: isize, dy: isize) -> Plane {
        let mut out = Plane::new(self.w, self.h);
        for y in 0..self.h as isize {
            for x in 0..self.w as isize {
                let (sx, sy) = (x - dx, y - dy);
                if sx >= 0 && sy >= 0 && sx < self.w as isize && sy < self.h as isize {
                    out.v[y as usize * self.w + x as usize] = self.v[sy as usize * self.w + sx as usize];
                }
            }
        }
        out
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A block sprite shrunk to `PX` pixels a block (RGBA, 0..1).
struct Shrunk {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

fn shrunk(path: &'static str) -> Option<&'static Shrunk> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, Option<&'static Shrunk>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().ok()?.get(path) {
        return *hit;
    }
    let made = (|| {
        let bytes = std::fs::read(format!("assets/{path}")).ok()?;
        let img = Image::from_buffer(
            &bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::Default,
            RenderAssetUsages::MAIN_WORLD,
        )
        .ok()?;
        let (sw, sh) = (img.width() as usize, img.height() as usize);
        let data = img.data.as_ref()?;
        if data.len() < sw * sh * 4 {
            return None;
        }
        let (bw, bh) = (((sw as f32 / 378.0).round() as usize).max(1), ((sh as f32 / 378.0).round() as usize).max(1));
        let (w, h) = (bw * PX, bh * PX);
        let mut px = vec![[0.0f32; 4]; w * h];
        for y in 0..h {
            for x in 0..w {
                let (x0, x1) = (x * sw / w, ((x + 1) * sw / w).max(x * sw / w + 1));
                let (y0, y1) = (y * sh / h, ((y + 1) * sh / h).max(y * sh / h + 1));
                let mut acc = [0.0f32; 4];
                let mut n = 0.0;
                for yy in (y0..y1).step_by(2) {
                    for xx in (x0..x1).step_by(2) {
                        let i = (yy * sw + xx) * 4;
                        let a = data[i + 3] as f32 / 255.0;
                        for c in 0..3 {
                            acc[c] += data[i + c] as f32 / 255.0 * a;
                        }
                        acc[3] += a;
                        n += 1.0;
                    }
                }
                if acc[3] > 0.0 {
                    px[y * w + x] = [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], acc[3] / n];
                }
            }
        }
        Some(&*Box::leak(Box::new(Shrunk { w, h, px })))
    })();
    cache.lock().ok()?.insert(path, made);
    made
}

fn image(w: usize, h: usize, rgba: Vec<u8>) -> Image {
    Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Paint one design turned `quarter` quarter-turns. The lighting is fixed to
/// the screen, so each turn is painted on its own.
pub fn bake(design: &Design, quarter: u8, seed: u64) -> Baked {
    let mut rng = StdRng::seed_from_u64(seed ^ (quarter as u64) << 40);
    // Cells turned, and their box.
    let turned: Vec<(IVec2, &super::raft::DesignCell)> = design
        .cells
        .iter()
        .map(|c| (turn(c.at.as_vec2(), quarter).round().as_ivec2(), c))
        .collect();
    let min = turned.iter().fold(IVec2::MAX, |a, (p, _)| a.min(*p));
    let max = turned.iter().fold(IVec2::MIN, |a, (p, _)| a.max(*p));
    let (gw, gh) = ((max.x - min.x + 1) as usize, (max.y - min.y + 1) as usize);
    let (w, h) = (gw * PX + 2 * PAD, gh * PX + 2 * PAD);
    let idx = |x: usize, y: usize| y * w + x;

    let mut rgb = vec![[0.0f32; 3]; w * h];
    let mut mask = Plane::new(w, h);
    let mut module = Plane::new(w, h);
    let mut lit_cells: Vec<(usize, usize)> = Vec::new();
    let steel = [0.42f32, 0.45, 0.52];
    for (p, c) in &turned {
        let x0 = PAD + (p.x - min.x) as usize * PX;
        let y0 = PAD + (max.y - p.y) as usize * PX;
        let src = shrunk(c.sprite);
        for dy in 0..PX {
            for dx in 0..PX {
                let mut col = steel;
                if let Some(s) = src {
                    let (pc, pr, pw, ph) = c.part;
                    let sx = (pc as usize * PX + dx) * s.w / (pw as usize * PX).max(1);
                    let sy = (pr as usize * PX + dy) * s.h / (ph as usize * PX).max(1);
                    let v = s.px[sy.min(s.h - 1) * s.w + sx.min(s.w - 1)];
                    for k in 0..3 {
                        col[k] = steel[k] * (1.0 - v[3]) + v[k] * v[3];
                    }
                }
                if c.outer {
                    col = col.map(|v| v * 0.82);
                }
                let i = idx(x0 + dx, y0 + dy);
                rgb[i] = col;
                mask.v[i] = 1.0;
                if c.module {
                    module.v[i] = 1.0;
                }
            }
        }
        if c.lit {
            lit_cells.push((x0, y0));
        }
    }

    // One material: the detail kept, the colour pulled to gunmetal.
    let gunmetal = [0.21f32, 0.23, 0.29];
    for i in 0..w * h {
        let c = rgb[i];
        let lum = c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11;
        for k in 0..3 {
            rgb[i][k] = c[k] * 0.45 + gunmetal[k] * (0.6 + lum * 1.2) * 0.55;
        }
    }

    // Shape: anti-aliased edge, domed body, lit from the upper left.
    let soft = mask.blur(1.0);
    let m: Vec<f32> = soft.v.iter().map(|v| smoothstep(0.42, 0.58, *v)).collect();
    let d1 = mask.blur(5.0);
    let d2 = mask.blur(12.0);
    let mb = module.blur(2.0);
    let mut height = Plane::new(w, h);
    for i in 0..w * h {
        height.v[i] = (d1.v[i] * 0.3 + d2.v[i] * 0.3 + mb.v[i] * 0.05) * m[i];
    }
    let hb = height.blur(1.2);
    let ao = height.blur(7.0);
    let cast = height.blur(2.0).shifted(5, 5);
    let key = Vec3::new(-0.55, -0.5, 0.67).normalize();
    let fill = Vec3::new(0.6, 0.55, 0.58).normalize();
    let warm = Vec3::new(1.0, 0.86, 0.70);
    let cool = Vec3::new(0.42, 0.52, 0.85);
    let k = 45.0;
    let mut lit = vec![Vec3::ZERO; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = idx(x, y);
            let gx = (hb.at(x as isize + 1, y as isize) - hb.at(x as isize - 1, y as isize)) * 0.5;
            let gy = (hb.at(x as isize, y as isize + 1) - hb.at(x as isize, y as isize - 1)) * 0.5;
            let n = Vec3::new(-gx * k, -gy * k, 1.0).normalize();
            let diff = n.dot(key).max(0.0);
            let fil = n.dot(fill).max(0.0);
            let spec = diff.powi(24);
            let albedo = Vec3::from(rgb[i]);
            let mut c = albedo * (warm * diff * 1.25 + cool * fil * 0.35 + cool * 0.12) + Vec3::new(1.0, 0.95, 0.85) * spec * 0.55;
            c *= 1.0 - ((ao.v[i] - height.v[i]).max(0.0) * 2.2).min(0.9);
            c *= 1.0 - ((cast.v[i] - height.v[i] - 0.02).max(0.0) * 1.8).min(0.6);
            lit[i] = c;
        }
    }

    // Lights: windows in the lit rooms, portholes round the edge, running
    // lights at the ends.
    let mut light = vec![Vec3::ZERO; w * h];
    let warm_win = Vec3::new(1.0, 0.76, 0.42);
    let white = Vec3::new(0.9, 0.94, 1.0);
    let cyan = Vec3::new(0.45, 0.88, 1.0);
    for (x0, y0) in lit_cells {
        for _ in 0..rng.gen_range(0..3) {
            let (x, y) = (x0 + rng.gen_range(1..PX - 2), y0 + rng.gen_range(1..PX - 1));
            let col = if rng.gen_bool(0.8) { warm_win } else if rng.gen_bool(0.6) { white } else { cyan };
            let s = rng.gen_range(0.9..1.6);
            light[idx(x, y)] += col * s;
            light[idx(x + 1, y)] += col * s;
        }
    }
    let inner = mask.blur(2.5);
    for y in (0..h).step_by(3) {
        for x in (0..w).step_by(3) {
            let i = idx(x, y);
            let rim = m[i] - smoothstep(0.75, 0.95, inner.v[i]);
            if rim > 0.5 && rng.gen_bool(0.3) {
                light[i] += warm_win * 0.8;
            }
        }
    }
    let long_x = gw >= gh;
    let ends = if long_x {
        [(PAD + 1, h / 2), (w - PAD - 2, h / 2)]
    } else {
        [(w / 2, PAD + 1), (w / 2, h - PAD - 2)]
    };
    for ((x, y), col) in ends.into_iter().zip([Vec3::new(1.0, 0.2, 0.15), Vec3::new(0.3, 1.0, 0.45)]) {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            light[idx((x + dx).min(w - 1), (y + dy).min(h - 1))] += col * 2.4;
        }
    }

    // Glow: the lights blurred out, and spilling onto the hull.
    let chan = |c: usize| {
        let mut p = Plane::new(w, h);
        for i in 0..w * h {
            p.v[i] = light[i][c];
        }
        p
    };
    let mut glow = vec![Vec3::ZERO; w * h];
    for c in 0..3 {
        let base = chan(c);
        let (g1, g2, g3, spill) = (base.blur(1.5), base.blur(5.0), base.blur(12.0), base.blur(9.0));
        for i in 0..w * h {
            glow[i][c] = base.v[i] * 0.8 + g1.v[i] * 1.0 + g2.v[i] * 0.8 + g3.v[i] * 0.5;
            lit[i][c] += rgb[i][c] * spill.v[i] * 4.0 * m[i];
        }
    }

    // The hull's shadow on whatever is below it.
    let shadow = mask.shifted(5, 5).blur(4.0);
    let mut body = Vec::with_capacity(w * h * 4);
    let mut lights = Vec::with_capacity(w * h * 4);
    for i in 0..w * h {
        let a_hull = m[i];
        let a_shadow = shadow.v[i] * 0.55 * (1.0 - a_hull);
        let a = a_hull + a_shadow;
        // Tone map, then straight alpha: the shadow is black.
        let c = lit[i].map(|v| 1.0 - (-v * 1.15).exp());
        let c = if a > 0.0 { c * (a_hull / a) } else { Vec3::ZERO };
        body.extend_from_slice(&[(c.x * 255.0) as u8, (c.y * 255.0) as u8, (c.z * 255.0) as u8, (a.min(1.0) * 255.0) as u8]);
        let g = glow[i];
        let peak = g.max_element();
        if peak > 0.002 {
            let col = (g / peak).map(|v| v.min(1.0));
            lights.extend_from_slice(&[(col.x * 255.0) as u8, (col.y * 255.0) as u8, (col.z * 255.0) as u8, (peak.min(1.0) * 255.0) as u8]);
        } else {
            lights.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    let size = Vec2::new(w as f32, h as f32) * (CELL / PX as f32);
    Baked { body: image(w, h, body), lights: image(w, h, lights), size }
}

// ---------------------------------------------------------------------------
// in the world

pub type BakeKey = (&'static str, u8);

/// Pictures painted so far, and the ones being painted.
#[derive(Resource, Default)]
pub struct RaftBakes {
    done: HashMap<BakeKey, (Handle<Image>, Handle<Image>, Vec2)>,
    asked: HashSet<BakeKey>,
    pending: Arc<Mutex<Vec<(BakeKey, Baked)>>>,
}

impl RaftBakes {
    /// A finished picture: (hull, lights, world size).
    pub fn done_get(&self, key: &BakeKey) -> Option<&(Handle<Image>, Handle<Image>, Vec2)> {
        self.done.get(key)
    }

    /// Paint these on a thread of their own, unless they're done or asked.
    pub fn ask(&mut self, keys: impl IntoIterator<Item = BakeKey>) {
        let new: Vec<BakeKey> = keys.into_iter().filter(|k| !self.done.contains_key(k) && self.asked.insert(*k)).collect();
        if new.is_empty() {
            return;
        }
        let out = self.pending.clone();
        std::thread::spawn(move || {
            let lib = library();
            for key in new {
                let Some(d) = lib.get(key.0) else { continue };
                let seed = key.0.bytes().fold(7u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
                let baked = bake(d, key.1, seed);
                if let Ok(mut p) = out.lock() {
                    p.push((key, baked));
                }
            }
        });
    }
}

/// The art beyond the baked hulls: a soft glow, a walkway tube, a girder.
#[derive(Resource, Clone)]
pub struct RaftArt {
    pub glow: Handle<Image>,
    pub tube: Handle<Image>,
    pub girder: Handle<Image>,
    pub shuttle: Handle<Image>,
}

pub fn make_raft_art(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let n = 96usize;
    let mut glow = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let d = Vec2::new(x as f32 - n as f32 / 2.0, y as f32 - n as f32 / 2.0).length() / (n as f32 / 2.0);
            let f = (1.0 - d).max(0.0);
            glow.extend_from_slice(&[255, 255, 255, (f * f * f * 255.0) as u8]);
        }
    }
    // A tube seen from above: round in section, lit along its back, with a
    // row of lit ports.
    let (tw, th) = (64usize, 12usize);
    let mut tube = Vec::with_capacity(tw * th * 4);
    for y in 0..th {
        let s = (y as f32 + 0.5) / th as f32 * 2.0 - 1.0;
        let shade = (1.0 - s * s).sqrt() * (0.75 - s * 0.35);
        for x in 0..tw {
            let port = y == th / 2 && x % 8 == 0;
            let c = if port { [255, 200, 130] } else { [(70.0 * shade + 20.0) as u8, (76.0 * shade + 22.0) as u8, (92.0 * shade + 28.0) as u8] };
            tube.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    // A girder: two rails and diagonals.
    let (gw, gh) = (64usize, 16usize);
    let mut girder = vec![0u8; gw * gh * 4];
    let mut put = |x: usize, y: usize, v: u8| {
        if x < gw && y < gh {
            let i = (y * gw + x) * 4;
            girder[i..i + 4].copy_from_slice(&[v, v.saturating_add(6), v.saturating_add(18), 255]);
        }
    };
    for x in 0..gw {
        put(x, 1, 120);
        put(x, 2, 90);
        put(x, gh - 2, 70);
        put(x, gh - 3, 90);
        let t = x % 16;
        let y = if (x / 16) % 2 == 0 { 2 + t * (gh - 5) / 16 } else { gh - 3 - t * (gh - 5) / 16 };
        put(x, y, 100);
    }
    // A shuttle: a small grey hull, its nose lit, its drive glowing.
    let (sw, sh) = (20usize, 10usize);
    let mut shuttle = vec![0u8; sw * sh * 4];
    for y in 0..sh {
        for x in 0..sw {
            let dy = (y as f32 + 0.5 - sh as f32 / 2.0).abs() / (sh as f32 / 2.0);
            let taper = 1.0 - (x as f32 / sw as f32).powi(3) * 0.8;
            if dy < taper {
                let i = (y * sw + x) * 4;
                let shade = (1.0 - dy * 0.5) * 110.0;
                let c = if x >= sw - 3 && dy < 0.4 { [255, 220, 150] } else if x < 2 { [120, 200, 255] } else { [shade as u8, (shade * 1.05) as u8, (shade * 1.2) as u8] };
                shuttle[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }
    commands.insert_resource(RaftArt {
        glow: images.add(image(n, n, glow)),
        tube: images.add(image(tw, th, tube)),
        girder: images.add(image(gw, gh, girder)),
        shuttle: images.add(image(sw, sh, shuttle)),
    });
}

/// A moored ship's picture (or its lights), waiting for its bake.
#[derive(Component)]
pub struct RaftHull {
    pub key: BakeKey,
    pub lights: bool,
}

/// Build the Raft's sprites under `root`.
pub fn spawn_raft(commands: &mut Commands, root: Entity, plan: &RaftPlan, art: &RaftArt, bakes: &mut RaftBakes, seed: u64) {
    // The empty berths' ships too, and the tug: arrivals need their pictures.
    bakes.ask(plan.moorings.iter().chain(&plan.spares).map(|m| (m.design, m.quarter)).chain((0..2).map(|q| ("free_haulers_compact/tug", q))));
    let mut kids = Vec::new();
    // Haze: the city's light in the space round it.
    kids.push(
        commands
            .spawn((Sprite { image: art.glow.clone(), color: Color::srgba(1.0, 0.82, 0.6, 0.10), custom_size: Some(Vec2::splat(plan.extent * 2.4)), ..default() }, Transform::from_xyz(0.0, 0.0, -0.04)))
            .id(),
    );
    // Streets: girders, with lights down them and a beacon at the end.
    for (a, b) in &plan.streets {
        let along = *b - *a;
        let len = along.length();
        let angle = along.to_angle();
        let seg = CELL * 6.0;
        let n = (len / seg).ceil() as usize;
        for i in 0..n {
            let at = *a + along * ((i as f32 + 0.5) / n as f32);
            kids.push(
                commands
                    .spawn((Sprite { image: art.girder.clone(), custom_size: Some(Vec2::new(len / n as f32 + 2.0, CELL * 2.6)), ..default() }, Transform::from_translation(at.extend(0.002)).with_rotation(Quat::from_rotation_z(angle))))
                    .id(),
            );
            if i % 2 == 0 {
                kids.push(
                    commands
                        .spawn((Sprite { image: art.glow.clone(), color: Color::srgba(1.0, 0.8, 0.5, 0.8), custom_size: Some(Vec2::splat(CELL * 3.0)), ..default() }, Transform::from_translation(at.extend(0.003))))
                        .id(),
                );
            }
        }
        kids.push(
            commands
                .spawn((
                    Sprite { image: art.glow.clone(), color: Color::srgb(0.35, 0.9, 0.5), custom_size: Some(Vec2::splat(CELL * 9.0)), ..default() },
                    Transform::from_translation(b.extend(0.06)),
                    super::home_base::StationBeacon::new(len * 0.01, Color::srgb(0.35, 0.9, 0.5)),
                ))
                .id(),
        );
    }
    // Walkways.
    for tube in &plan.tubes {
        kids.push(spawn_tube(commands, art, tube));
    }
    // The ships, higher ones over lower ones, lights over each.
    for m in &plan.moorings {
        super::raft_life::spawn_ship(commands, root, m, bakes, seed);
    }
    super::raft_life::spawn_traffic(commands, root, plan, art, seed);
    commands.entity(root).add_children(&kids);
}

/// A walkway between two points, owned by the berths it joins.
pub fn spawn_tube(commands: &mut Commands, art: &RaftArt, tube: &super::raft::Tube) -> Entity {
    let along = tube.b - tube.a;
    commands
        .spawn((
            Sprite { image: art.tube.clone(), custom_size: Some(Vec2::new(along.length().max(1.0), tube.width)), ..default() },
            Transform::from_translation(((tube.a + tube.b) * 0.5).extend(0.004)).with_rotation(Quat::from_rotation_z(along.to_angle())),
            super::raft_life::TubeOf(tube.ends),
        ))
        .id()
}

/// Put finished pictures on the ships waiting for them.
pub fn apply_raft_bakes(mut images: ResMut<Assets<Image>>, bakes: Option<ResMut<RaftBakes>>, mut hulls: Query<(&RaftHull, &mut Sprite)>) {
    let Some(mut bakes) = bakes else { return };
    let ready: Vec<(BakeKey, Baked)> = match bakes.pending.lock() {
        Ok(mut p) if !p.is_empty() => p.drain(..).collect(),
        _ => return,
    };
    for (key, baked) in ready {
        let entry = (images.add(baked.body), images.add(baked.lights), baked.size);
        bakes.done.insert(key, entry);
    }
    for (hull, mut sprite) in &mut hulls {
        if sprite.color != Color::NONE {
            continue;
        }
        if let Some((body, glow, size)) = bakes.done.get(&hull.key) {
            sprite.image = if hull.lights { glow.clone() } else { body.clone() };
            sprite.custom_size = Some(*size);
            sprite.color = Color::WHITE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_bakes_to_a_picture_its_own_size() {
        let lib = library();
        let d = lib.get("free_haulers_compact/tug").expect("tug design");
        let b = bake(d, 0, 1);
        let span = d.span();
        let (w, h) = (b.body.width() as usize, b.body.height() as usize);
        assert_eq!(w, span.x as usize * PX + 2 * PAD);
        assert_eq!(h, span.y as usize * PX + 2 * PAD);
        // Turned a quarter, it's the other way round.
        let t = bake(d, 1, 1);
        assert_eq!(t.body.width() as usize, h);
        // It has a hull and it has lights.
        let data = b.body.data.as_ref().unwrap();
        assert!(data.chunks(4).filter(|p| p[3] == 255).count() > span.x as usize * PX);
        let lights = b.lights.data.as_ref().unwrap();
        assert!(lights.chunks(4).any(|p| p[3] > 200), "no lights at all");
    }
}
