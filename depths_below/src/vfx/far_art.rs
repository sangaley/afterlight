//! Ships seen from very far out: one small picture each, not a thousand blocks.
//!
//! Past the Cosmoteer roof (vfx::ship_roof) the camera keeps going -- out to
//! zoom 20, where a block is three pixels across. Drawn block by block a ship
//! there is a shimmer of sprites far smaller than their art, and a big fleet
//! is tens of thousands of them. So from `ART_FROM` each ship and each turret
//! is baked into one picture of itself: the very sprites on screen, painted
//! in their own order into a few pixels a block, with a dark rim round the
//! hull so it reads against space. Parts keep their colours -- a reactor's
//! glow, an engine, a painted chevron -- because the bake is made from them,
//! not from a flat silhouette (the slab look ship_roof moved away from).
//!
//! Fully out, the blocks the picture stands in for stop drawing (moved to a
//! render layer no camera looks at); zoom back in and they're back. Close in
//! nothing is baked at all.
//!
//! A picture keeps up with the fight by being patched, not baked again. Baking
//! a capital takes the best part of a frame, so it used to wait seconds between
//! bakes while the ship was taken apart. Now a ship with anything changed is
//! looked over about ten times a second, what's being drawn is compared with
//! what was painted, and only the squares of the picture around a difference
//! are painted over. A square comes out exactly as a whole new bake would.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::image::ImageSampler;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::ai_ship::components::AiShip;
use crate::camera::CameraState;
use crate::combat::turret_deck::TurretBody;
use crate::components::{HullSegment, Module, Ship};

/// Zoom where the picture starts to show over the blocks...
pub const ART_FROM: f32 = 8.0;
/// ...and where it stands in for them.
pub const ART_SOLID: f32 = 10.5;

/// Pixels per block. At zoom 11 a block is six screen pixels: one to one.
const PX_PER_CELL: f32 = 6.0;
/// Ship-local units per baked pixel.
const UNITS_PER_PX: f32 = crate::building::GRID_SIZE / PX_PER_CELL;
/// Samples per baked pixel, each way.
const SUBSAMPLES: usize = 1;
/// Source art is shrunk to this many pixels on its long side before it's
/// sampled, so a sample stands for an area of the art, not one stray texel.
const MIP_SIDE: u32 = 32;
/// Over a ship's blocks (0-1) and its weapons raised over the roof (0.75),
/// under its turrets (1.2), which carry pictures of their own.
const SHIP_ART_Z: f32 = 1.15;
/// Over everything on a turret: plates, parts, gun house roof (0.5).
const TURRET_ART_Z: f32 = 0.9;
/// The rim round the hull: near-black, mostly solid.
const RIM: LinearRgba = LinearRgba { red: 0.004, green: 0.005, blue: 0.009, alpha: 0.8 };
/// How often a ship is looked over with no change seen, for what nothing
/// announces: art that has finished loading.
const REFRESH_S: f32 = 3.0;
/// Soonest a changed ship is looked over again, so a hit shows this soon.
const PATCH_S: f32 = 0.1;
/// Sprites looked over a frame: a big fleet takes a few frames to get round.
const SPRITES_PER_FRAME: usize = 8000;
/// The side of a square of picture painted over at once, in blocks. Wide
/// enough that a blast's worth of neighbouring blocks is a few squares.
const SQUARE_CELLS: f32 = 8.0;
/// No camera draws this layer.
const HIDDEN_LAYER: usize = 29;
/// Biggest picture side, in pixels (a 680-block-long ship).
const MAX_SIDE: u32 = 4096;

/// How much of the picture shows at a zoom: 0 close in, 1 far out.
pub fn art_alpha(zoom: f32) -> f32 {
    let t = ((zoom - ART_FROM) / (ART_SOLID - ART_FROM)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// On a ship or turret: its picture, and what it was baked from.
#[derive(Component)]
pub struct FarArt {
    sprite: Entity,
    image: Handle<Image>,
    /// When it was last looked over.
    seen_at: f32,
    /// Something it's made from may have changed.
    dirty: bool,
    /// Kept to paint over: the texture can't be read back.
    picture: Option<Picture>,
    /// What's in the picture. By what it looks like, not by entity: a roof
    /// relaid the same is all new entities and nothing to paint.
    painted: HashMap<PaintKey, Rect>,
    /// Times the picture has been painted on, whole or in part.
    repaints: u32,
}

/// What one sprite in a picture looks like, near enough: too small a change
/// to see doesn't repaint anything.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PaintKey {
    at: [i32; 2],
    z: i32,
    angle: i32,
    size: [i32; 2],
    rect: [i32; 4],
    flip: (bool, bool),
    tint: [u8; 4],
    burn: u8,
    source: AssetId<Image>,
}

/// The picture itself, a child of what it pictures.
#[derive(Component)]
pub struct FarArtSprite;

/// A block not drawn while its ship's picture stands in for it.
#[derive(Component)]
pub struct FarHidden;

/// Source art shrunk for sampling: premultiplied linear colour, rows top down.
pub struct Mip {
    w: u32,
    h: u32,
    /// Source pixels per mip pixel.
    step: f32,
    px: Vec<LinearRgba>,
}

impl Mip {
    /// Box-filter `width x height` straight-alpha linear pixels down to
    /// `MIP_SIDE` on the long side.
    pub fn build(width: u32, height: u32, at: impl Fn(u32, u32) -> LinearRgba) -> Self {
        let step = width.max(height).div_ceil(MIP_SIDE).max(1);
        let (w, h) = (width.div_ceil(step), height.div_ceil(step));
        let mut px = Vec::with_capacity((w * h) as usize);
        for my in 0..h {
            for mx in 0..w {
                let mut sum = LinearRgba::NONE;
                let mut n = 0.0;
                for y in my * step..((my + 1) * step).min(height) {
                    for x in mx * step..((mx + 1) * step).min(width) {
                        let c = at(x, y);
                        sum.red += c.red * c.alpha;
                        sum.green += c.green * c.alpha;
                        sum.blue += c.blue * c.alpha;
                        sum.alpha += c.alpha;
                        n += 1.0;
                    }
                }
                px.push(if n > 0.0 { sum * (1.0 / n) } else { LinearRgba::NONE });
            }
        }
        Self { w, h, step: step as f32, px }
    }

    /// A plain white square: a sprite with no art is its colour.
    pub fn white() -> Self {
        Self { w: 1, h: 1, step: 1.0, px: vec![LinearRgba::WHITE] }
    }

    /// The pixel under source-art coordinate `(x, y)` (pixels, top down).
    fn at(&self, x: f32, y: f32) -> LinearRgba {
        let mx = ((x / self.step) as i64).clamp(0, self.w as i64 - 1) as usize;
        let my = ((y / self.step) as i64).clamp(0, self.h as i64 - 1) as usize;
        self.px[my * self.w as usize + mx]
    }
}

/// One sprite, as the bake sees it: where it sits in its parent and what of
/// its art shows.
pub struct Stamp<'a> {
    pub at: Vec2,
    pub angle: f32,
    pub size: Vec2,
    /// The part of the art drawn, in source pixels (x, y, w, h), top down.
    pub rect: Rect,
    pub flip_x: bool,
    pub flip_y: bool,
    pub tint: LinearRgba,
    /// How much it glows with damage, 0..1.
    pub burn: f32,
    pub art: &'a Mip,
    /// The image `art` was made from.
    pub source: AssetId<Image>,
    pub z: f32,
}

impl Stamp<'_> {
    fn key(&self) -> PaintKey {
        let eighths = |v: f32| (v * 8.0).round() as i32;
        let level = |v: f32| (v * 32.0).round() as u8;
        PaintKey {
            at: [eighths(self.at.x), eighths(self.at.y)],
            z: (self.z * 1000.0).round() as i32,
            angle: (self.angle * 1000.0).round() as i32,
            size: [eighths(self.size.x), eighths(self.size.y)],
            rect: [self.rect.min.x as i32, self.rect.min.y as i32, self.rect.max.x as i32, self.rect.max.y as i32],
            flip: (self.flip_x, self.flip_y),
            tint: [level(self.tint.red), level(self.tint.green), level(self.tint.blue), level(self.tint.alpha)],
            burn: level(self.burn),
            source: self.source,
        }
    }

    /// Ship-local corners' bounding box.
    fn bounds(&self) -> Rect {
        let (s, c) = self.angle.sin_cos();
        let h = self.size * 0.5;
        let ext = Vec2::new((h.x * c).abs() + (h.y * s).abs(), (h.x * s).abs() + (h.y * c).abs());
        Rect::from_center_half_size(self.at, ext)
    }

    /// Premultiplied colour at a ship-local point, or nothing off the sprite.
    fn sample(&self, p: Vec2) -> Option<LinearRgba> {
        let (s, c) = self.angle.sin_cos();
        let d = p - self.at;
        let q = Vec2::new(d.x * c + d.y * s, -d.x * s + d.y * c);
        let mut u = q.x / self.size.x + 0.5;
        let mut v = q.y / self.size.y + 0.5;
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return None;
        }
        if self.flip_x {
            u = 1.0 - u;
        }
        if self.flip_y {
            v = 1.0 - v;
        }
        // Art rows run top down; v runs up.
        let texel = self.art.at(self.rect.min.x + u * self.rect.width(), self.rect.min.y + (1.0 - v) * self.rect.height());
        let t = self.tint;
        let glow = self.burn * texel.alpha;
        Some(LinearRgba::new(
            (texel.red * t.red + EMBER.0 * glow) * t.alpha,
            (texel.green * t.green + EMBER.1 * glow) * t.alpha,
            (texel.blue * t.blue + EMBER.2 * glow) * t.alpha,
            texel.alpha * t.alpha,
        ))
    }
}

/// A picture being painted: premultiplied linear pixels, rows bottom up.
pub struct Canvas {
    pub origin: Vec2,
    pub w: u32,
    pub h: u32,
    pub px: Vec<LinearRgba>,
}

impl Canvas {
    /// A clear canvas over `bounds` (ship-local), with room for the rim.
    pub fn over(bounds: Rect) -> Option<Self> {
        let pad = 2.0 * UNITS_PER_PX;
        let lo = ((bounds.min - pad) / UNITS_PER_PX).floor() * UNITS_PER_PX;
        let hi = ((bounds.max + pad) / UNITS_PER_PX).ceil() * UNITS_PER_PX;
        let w = ((hi.x - lo.x) / UNITS_PER_PX).round() as u32;
        let h = ((hi.y - lo.y) / UNITS_PER_PX).round() as u32;
        if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
            return None;
        }
        Some(Self { origin: lo, w, h, px: vec![LinearRgba::NONE; (w * h) as usize] })
    }

    /// Paint one sprite over what's there.
    pub fn paint(&mut self, stamp: &Stamp) {
        let b = stamp.bounds();
        let x0 = (((b.min.x - self.origin.x) / UNITS_PER_PX).floor().max(0.0)) as u32;
        let y0 = (((b.min.y - self.origin.y) / UNITS_PER_PX).floor().max(0.0)) as u32;
        let x1 = (((b.max.x - self.origin.x) / UNITS_PER_PX).ceil() as u32).min(self.w);
        let y1 = (((b.max.y - self.origin.y) / UNITS_PER_PX).ceil() as u32).min(self.h);
        let share = 1.0 / (SUBSAMPLES * SUBSAMPLES) as f32;
        for y in y0..y1 {
            for x in x0..x1 {
                let mut src = LinearRgba::NONE;
                for sy in 0..SUBSAMPLES {
                    for sx in 0..SUBSAMPLES {
                        let p = self.origin
                            + Vec2::new(
                                x as f32 + (sx as f32 + 0.5) / SUBSAMPLES as f32,
                                y as f32 + (sy as f32 + 0.5) / SUBSAMPLES as f32,
                            ) * UNITS_PER_PX;
                        if let Some(c) = stamp.sample(p) {
                            src = src + c * share;
                        }
                    }
                }
                if src.alpha <= 0.0 {
                    continue;
                }
                let dst = &mut self.px[(y * self.w + x) as usize];
                *dst = src + *dst * (1.0 - src.alpha);
            }
        }
    }

    /// A dark rim on the clear pixels touching the picture.
    pub fn rim(&mut self) {
        let solid = |c: &LinearRgba| c.alpha > 0.5;
        let mask: Vec<bool> = self.px.iter().map(solid).collect();
        let (w, h) = (self.w as i32, self.h as i32);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if self.px[i].alpha > 0.05 {
                    continue;
                }
                let touches = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        let (nx, ny) = (x + dx, y + dy);
                        nx >= 0 && ny >= 0 && nx < w && ny < h && mask[(ny * w + nx) as usize]
                    })
                });
                if touches {
                    self.px[i] = LinearRgba::new(RIM.red * RIM.alpha, RIM.green * RIM.alpha, RIM.blue * RIM.alpha, RIM.alpha);
                }
            }
        }
    }

    /// One pixel as sRGB bytes.
    fn bytes_at(&self, x: u32, y: u32) -> [u8; 4] {
        let c = self.px[(y * self.w + x) as usize];
        let straight = if c.alpha > 0.0 {
            LinearRgba::new(c.red / c.alpha, c.green / c.alpha, c.blue / c.alpha, c.alpha.min(1.0))
        } else {
            LinearRgba::NONE
        };
        Color::LinearRgba(straight).to_srgba().to_u8_array()
    }

    /// The finished picture, rows top down.
    pub fn finish(self) -> Picture {
        let mut bytes = Vec::with_capacity((self.w * self.h * 4) as usize);
        for y in (0..self.h).rev() {
            for x in 0..self.w {
                bytes.extend_from_slice(&self.bytes_at(x, y));
            }
        }
        Picture { origin: self.origin, w: self.w, h: self.h, bytes }
    }

    /// Ship-local centre and size of the picture.
    pub fn placement(&self) -> (Vec2, Vec2) {
        let size = Vec2::new(self.w as f32, self.h as f32) * UNITS_PER_PX;
        (self.origin + size * 0.5, size)
    }
}

/// A painted picture: sRGB bytes, rows top down.
pub struct Picture {
    origin: Vec2,
    w: u32,
    h: u32,
    bytes: Vec<u8>,
}

impl Picture {
    /// As an image to hand the renderer.
    fn image(&self) -> Image {
        let mut image = Image::new(
            Extent3d { width: self.w, height: self.h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            self.bytes.clone(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::linear();
        image
    }

    /// Whether something at `bounds`, rim included, fits in the picture.
    fn has_room_for(&self, bounds: Rect) -> bool {
        let size = Vec2::new(self.w as f32, self.h as f32) * UNITS_PER_PX;
        let room = Rect::from_corners(self.origin, self.origin + size).inflate(-UNITS_PER_PX);
        room.contains(bounds.min) && room.contains(bounds.max)
    }

    /// Paint one square over again from everything that touches it. `stamps`
    /// are in painting order, each with its bounds.
    fn repaint(&mut self, square: IVec2, stamps: &[(Rect, &Stamp)]) {
        let side = SQUARE_CELLS * crate::building::GRID_SIZE;
        let lo = square.as_vec2() * side;
        let Some(mut canvas) = Canvas::over(Rect::from_corners(lo, lo + side)) else { return };
        let size = Vec2::new(canvas.w as f32, canvas.h as f32) * UNITS_PER_PX;
        let area = Rect::from_corners(canvas.origin, canvas.origin + size);
        for (bounds, stamp) in stamps {
            if !bounds.intersect(area).is_empty() {
                canvas.paint(stamp);
            }
        }
        canvas.rim();
        // The outermost pixels only show the rim what's beside them.
        let offset = ((canvas.origin - self.origin) / UNITS_PER_PX).round().as_ivec2();
        for y in 1..canvas.h.saturating_sub(1) {
            for x in 1..canvas.w.saturating_sub(1) {
                let (px, py) = (x as i32 + offset.x, y as i32 + offset.y);
                if px < 0 || py < 0 || px >= self.w as i32 || py >= self.h as i32 {
                    continue;
                }
                let row = self.h as usize - 1 - py as usize;
                let i = (row * self.w as usize + px as usize) * 4;
                self.bytes[i..i + 4].copy_from_slice(&canvas.bytes_at(x, y));
            }
        }
    }
}

/// The squares of picture `bounds` touches.
fn mark_squares(bounds: Rect, squares: &mut HashSet<IVec2>) {
    let side = SQUARE_CELLS * crate::building::GRID_SIZE;
    let lo = (bounds.min / side).floor().as_ivec2();
    let hi = (bounds.max / side).floor().as_ivec2();
    for y in lo.y..=hi.y {
        for x in lo.x..=hi.x {
            squares.insert(IVec2::new(x, y));
        }
    }
}

/// Shrunk copies of the art that's been baked from, by image.
#[derive(Default)]
pub struct MipCache(HashMap<AssetId<Image>, Mip>);

impl MipCache {
    /// The shrunk art for a sprite, made the first time it's asked for. None
    /// while the art is still loading or can't be read back.
    fn get(&mut self, images: &Assets<Image>, handle: &Handle<Image>) -> Option<&Mip> {
        let id = handle.id();
        if id == Handle::<Image>::default().id() {
            return None;
        }
        if !self.0.contains_key(&id) {
            let image = images.get(id)?;
            image.data.as_ref()?;
            let (w, h) = (image.width(), image.height());
            let mip = Mip::build(w, h, |x, y| image.get_color_at(x, y).map(|c| c.to_linear()).unwrap_or(LinearRgba::NONE));
            self.0.insert(id, mip);
        }
        self.0.get(&id)
    }
}

/// Give every ship and turret a picture to bake into.
#[allow(clippy::type_complexity)]
pub fn attach_far_art(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    owners: Query<(Entity, Has<TurretBody>), (Or<(With<Ship>, With<AiShip>, With<TurretBody>, With<crate::combat::severance::DetachedSection>)>, Without<FarArt>)>,
) {
    // DEPTHS_NO_FAR_ART=1: blocks all the way out, to measure against.
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *OFF.get_or_init(|| std::env::var_os("DEPTHS_NO_FAR_ART").is_some()) {
        return;
    }
    for (owner, turret) in owners.iter() {
        let image = images.add(Image::new_fill(
            Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ));
        let z = if turret { TURRET_ART_Z } else { SHIP_ART_Z };
        let sprite = commands
            .spawn((
                Sprite { image: image.clone(), color: Color::WHITE.with_alpha(0.0), ..default() },
                Transform::from_xyz(0.0, 0.0, z),
                Visibility::Hidden,
                FarArtSprite,
                ChildOf(owner),
            ))
            .id();
        commands.entity(owner).try_insert(FarArt { sprite, image, seen_at: f32::NEG_INFINITY, dirty: true, picture: None, painted: HashMap::default(), repaints: 0 });
    }
}

/// What a picture is painted from: a ship's hull, parts and roof, or a
/// turret's plates, parts and gun house roof -- whatever of those is drawn.
#[allow(clippy::type_complexity)]
type Pictured<'w, 's> = Query<
    'w,
    's,
    (&'static Sprite, &'static Transform, &'static Visibility, Option<&'static HullSegment>, Option<&'static Module>, Has<FarHidden>),
    (
        Or<(With<HullSegment>, With<Module>, With<super::ship_roof::RoofTile>, With<super::ship_roof::AiRoofTile>)>,
        Without<FarArtSprite>,
    ),
>;

/// Blocks with something changed that could show in their ship's picture.
#[allow(clippy::type_complexity)]
type ChangedBlocks<'w, 's> = Query<
    'w,
    's,
    &'static ChildOf,
    (
        Or<(With<HullSegment>, With<Module>, With<super::ship_roof::RoofTile>, With<super::ship_roof::AiRoofTile>)>,
        Or<(Changed<HullSegment>, Changed<Module>, Changed<Sprite>, Changed<Visibility>, Changed<Transform>)>,
        Without<FarArtSprite>,
    ),
>;

/// The glow of a wound in the far picture.
const EMBER: (f32, f32, f32) = (1.0, 0.22, 0.03);
/// How hot a block all but gone glows.
const BURN: f32 = 0.32;

/// How much of a block is left, 0..1, for how dark it paints.
fn wear_of(hull: Option<&HullSegment>, module: Option<&Module>) -> f32 {
    let (health, max) = match (hull, module) {
        (Some(h), _) => (h.health, h.max_health),
        (_, Some(m)) => (m.health, m.max_health),
        _ => return 1.0,
    };
    if max > 0.0 { (health / max).clamp(0.0, 1.0) } else { 1.0 }
}

/// A block's colour for the picture: darker as it's shot up, down to a third.
fn worn(tint: LinearRgba, left: f32) -> LinearRgba {
    let k = 0.33 + 0.67 * left;
    LinearRgba::new(tint.red * k, tint.green * k, tint.blue * k, tint.alpha)
}

/// One sprite going into a picture.
struct Layer<'q> {
    z: f32,
    block: Entity,
    sprite: &'q Sprite,
    transform: &'q Transform,
    left: f32,
    /// The hull cell it is, for what's standing.
    cell: Option<IVec2>,
    hidden: bool,
}

/// How hurt each cell with something hurt on it is, 0..1 left.
fn hurt_cells<'a>(blocks: impl Iterator<Item = (Option<&'a HullSegment>, Option<&'a Module>)>) -> HashMap<IVec2, f32> {
    let mut hurt: HashMap<IVec2, f32> = HashMap::default();
    for (hull, module) in blocks {
        let left = wear_of(hull, module);
        if left >= 1.0 {
            continue;
        }
        let mut mark = |cell: IVec2| {
            let at = hurt.entry(cell).or_insert(1.0);
            *at = at.min(left);
        };
        if let Some(h) = hull {
            mark(h.grid_position);
        } else if let Some(m) = module {
            let footprint = crate::building::footprints::footprint_override(m.module_type);
            for cell in crate::building::ShipGrid::cells_for(m.grid_position, m.size, m.rotation, footprint) {
                mark(cell);
            }
        }
    }
    hurt
}

/// Bake the pictures nobody has yet and patch the ones that have fallen behind,
/// only when they can be seen.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn bake_far_art(
    mut commands: Commands,
    camera: Res<CameraState>,
    time: Res<Time>,
    mut images: ResMut<Assets<Image>>,
    mut owners: Query<(Entity, &mut FarArt, Option<Ref<Children>>, Option<Ref<super::ship_roof::Scars>>)>,
    pictured: Pictured,
    changed: ChangedBlocks,
    mut sprites: Query<(&mut Sprite, &mut Transform), (With<FarArtSprite>, Without<HullSegment>, Without<Module>)>,
    mut cache: Local<MipCache>,
    mut was_far: Local<bool>,
) {
    let alpha = art_alpha(camera.zoom);
    if alpha <= 0.0 {
        *was_far = false;
        return;
    }
    // Nothing was watching while the camera was in close.
    let back_out = !std::mem::replace(&mut *was_far, true);
    let now = time.elapsed_secs();
    let full = alpha >= 1.0;
    for parent in changed.iter() {
        if let Ok((_, mut art, ..)) = owners.get_mut(parent.parent()) {
            art.dirty = true;
        }
    }
    // Never looked at first, then changed, then the stalest.
    let mut due: Vec<(u8, f32, Entity)> = Vec::new();
    for (owner, mut art, children, scars) in owners.iter_mut() {
        if back_out || children.is_some_and(|c| c.is_changed()) || scars.is_some_and(|s| s.is_changed()) {
            art.dirty = true;
        }
        let since = now - art.seen_at;
        let rank = if !art.seen_at.is_finite() {
            0
        } else if art.dirty && since >= PATCH_S {
            1
        } else if since >= REFRESH_S {
            2
        } else {
            continue;
        };
        due.push((rank, art.seen_at, owner));
    }
    due.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));

    let white = Mip::white();
    let side = SQUARE_CELLS * crate::building::GRID_SIZE;
    let (mut looked, mut baked_whole) = (0, false);
    for (_, _, owner) in due {
        if looked >= SPRITES_PER_FRAME {
            break;
        }
        let Ok((_, mut art, children, scars)) = owners.get_mut(owner) else { continue };
        let art = &mut *art;
        // One whole bake a frame: a fleet coming into view spreads out.
        if art.picture.is_none() && baked_whole {
            continue;
        }
        let drawn: Vec<(Entity, _)> = children
            .iter()
            .flat_map(|c| c.iter())
            .filter_map(|c| pictured.get(c).ok().filter(|(_, _, v, ..)| **v != Visibility::Hidden).map(|p| (c, p)))
            .collect();
        looked += drawn.len().max(1);
        // Damage shows through the roof: the plating over a hurt block is
        // what you actually see of it from out here.
        let hurt = hurt_cells(drawn.iter().map(|(_, (_, _, _, hull, module, _))| (*hull, *module)));
        let default_art = Handle::<Image>::default().id();
        let mut layers: Vec<Layer> = drawn
            .iter()
            .map(|(block, (sprite, transform, _, hull, module, hidden))| {
                let plating = hull.is_none() && module.is_none() && sprite.image.id() != default_art;
                let left = if plating {
                    hurt.get(&crate::building::local_to_grid(transform.translation.truncate())).copied().unwrap_or(1.0)
                } else {
                    wear_of(*hull, *module)
                };
                Layer { z: transform.translation.z, block: *block, sprite, transform, left, cell: hull.map(|h| h.grid_position), hidden: *hidden }
            })
            .collect();
        layers.sort_by(|a, b| a.z.total_cmp(&b.z));

        // Shrink any art not seen before, then paint bottom up.
        for layer in &layers {
            cache.get(&images, &layer.sprite.image);
        }
        let mut stamps: Vec<Stamp> = layers
            .iter()
            .filter_map(|layer| {
                let sprite = layer.sprite;
                let t = layer.transform;
                let source = sprite.image.id();
                let mip = if source == default_art { &white } else { cache.0.get(&source)? };
                let (src_w, src_h) = (mip.w as f32 * mip.step, mip.h as f32 * mip.step);
                let rect = sprite.rect.unwrap_or(Rect::new(0.0, 0.0, src_w, src_h));
                let size = sprite.custom_size.unwrap_or(rect.size()) * t.scale.truncate().abs();
                Some(Stamp {
                    at: t.translation.truncate(),
                    angle: t.rotation.to_euler(EulerRot::ZYX).0,
                    size,
                    rect,
                    flip_x: sprite.flip_x,
                    flip_y: sprite.flip_y,
                    tint: worn(sprite.color.to_linear(), layer.left),
                    burn: (1.0 - layer.left) * BURN,
                    art: mip,
                    source,
                    z: layer.z,
                })
            })
            .collect();
        // Wounds glow, on top of everything.
        if let Some(scars) = &scars {
            let standing = layers.iter().filter_map(|layer| layer.cell).collect();
            for (cell, bright) in super::ship_roof::embers(scars, &standing) {
                stamps.push(Stamp {
                    at: crate::building::grid_to_local(cell),
                    angle: 0.0,
                    size: Vec2::splat(crate::building::GRID_SIZE * 1.3),
                    rect: Rect::new(0.0, 0.0, white.w as f32 * white.step, white.h as f32 * white.step),
                    flip_x: false,
                    flip_y: false,
                    tint: LinearRgba::new(EMBER.0 * bright, EMBER.1 * bright, EMBER.2 * bright, 0.95),
                    burn: 0.0,
                    art: &white,
                    source: default_art,
                    z: SHIP_ART_Z,
                });
            }
        }

        // What's different from what was painted?
        let painted: HashMap<PaintKey, Rect> = stamps.iter().map(|s| (s.key(), s.bounds())).collect();
        let mut squares: HashSet<IVec2> = HashSet::default();
        let mut room = true;
        for (key, bounds) in &art.painted {
            if !painted.contains_key(key) {
                mark_squares(*bounds, &mut squares);
            }
        }
        for (key, bounds) in &painted {
            if !art.painted.contains_key(key) {
                mark_squares(*bounds, &mut squares);
                room &= art.picture.as_ref().is_some_and(|p| p.has_room_for(*bounds));
            }
        }
        art.seen_at = now;
        art.dirty = false;

        // Painting nearly all of it a square at a time is slower than once.
        let whole = match &art.picture {
            None => true,
            Some(p) => {
                let squares_in_it = (p.w * p.h) as f32 * UNITS_PER_PX * UNITS_PER_PX / (side * side);
                !stamps.is_empty() && (!room || squares.len() as f32 * 2.0 > squares_in_it)
            }
        };
        if whole && baked_whole {
            art.dirty = true;
            continue;
        }
        // Fully out, what's painted stops drawing (blocks built or roofs laid
        // since it was last looked at included).
        if full {
            for layer in layers.iter().filter(|l| !l.hidden) {
                commands.entity(layer.block).try_insert((RenderLayers::layer(HIDDEN_LAYER), FarHidden));
            }
        }
        if whole {
            let Some(bounds) = stamps.iter().map(Stamp::bounds).reduce(|a, b| a.union(b)) else { continue };
            let Some(mut canvas) = Canvas::over(bounds) else { continue };
            for stamp in &stamps {
                canvas.paint(stamp);
            }
            canvas.rim();
            let (centre, size) = canvas.placement();
            art.picture = Some(canvas.finish());
            baked_whole = true;
            if let Ok((mut sprite, mut t)) = sprites.get_mut(art.sprite) {
                sprite.custom_size = Some(size);
                t.translation = centre.extend(t.translation.z);
            }
        } else if squares.is_empty() {
            continue;
        } else if let Some(picture) = art.picture.as_mut() {
            let bounded: Vec<(Rect, &Stamp)> = stamps.iter().map(|s| (s.bounds(), s)).collect();
            for square in &squares {
                picture.repaint(*square, &bounded);
            }
        }
        art.painted = painted;
        art.repaints += 1;
        let Some(picture) = art.picture.as_ref() else { continue };
        // DEPTHS_FAR_TRACE=1 logs each bake; DEPTHS_FAR_DUMP=<dir> writes
        // each picture there as raw RGBA.
        if std::env::var_os("DEPTHS_FAR_TRACE").is_some() {
            let what = if whole { "baked".to_string() } else { format!("patched {} squares of", squares.len()) };
            info!("FAR ART {what} {owner}: {}x{} px from {} sprites, zoom {:.1}", picture.w, picture.h, stamps.len(), camera.zoom);
        }
        if let Some(dir) = std::env::var_os("DEPTHS_FAR_DUMP") {
            let path = std::path::Path::new(&dir).join(format!("{owner}_{}x{}.rgba", picture.w, picture.h));
            let _ = std::fs::write(path, &picture.bytes);
        }
        let _ = images.insert(&art.image, picture.image());
    }
}

/// Fade the pictures with the zoom; fully out, hide the blocks they stand in
/// for, and bring them back as the camera comes in.
#[allow(clippy::type_complexity)]
pub fn show_far_art(
    mut commands: Commands,
    camera: Res<CameraState>,
    owners: Query<(&FarArt, Option<&Children>)>,
    pictured: Pictured,
    mut sprites: Query<(&mut Sprite, &mut Visibility), (With<FarArtSprite>, Without<HullSegment>, Without<Module>)>,
    hidden: Query<Entity, With<FarHidden>>,
    mut was_full: Local<bool>,
) {
    let alpha = art_alpha(camera.zoom);
    for (art, _) in owners.iter() {
        let Ok((mut sprite, mut vis)) = sprites.get_mut(art.sprite) else { continue };
        let want = alpha;
        // Not shown until it's been baked once.
        let want_vis = if want > 0.0 && art.picture.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want_vis {
            *vis = want_vis;
        }
        if (sprite.color.alpha() - want).abs() > 0.001 {
            sprite.color.set_alpha(want);
        }
    }
    let full = alpha >= 1.0;
    if full == *was_full {
        return;
    }
    *was_full = full;
    if std::env::var_os("DEPTHS_FAR_TRACE").is_some() {
        info!("FAR ART {} at zoom {:.1}", if full { "stands in" } else { "steps back" }, camera.zoom);
    }
    if full {
        for (art, children) in owners.iter() {
            if art.picture.is_none() {
                continue;
            }
            for child in children.into_iter().flatten() {
                if pictured.contains(*child) {
                    commands.entity(*child).try_insert((RenderLayers::layer(HIDDEN_LAYER), FarHidden));
                }
            }
        }
    } else {
        for block in hidden.iter() {
            commands.entity(block).try_remove::<(RenderLayers, FarHidden)>();
        }
    }
}

/// Off the flight screen every block draws again.
pub fn clear_far_art(mut commands: Commands, hidden: Query<Entity, With<FarHidden>>) {
    for block in hidden.iter() {
        commands.entity(block).try_remove::<(RenderLayers, FarHidden)>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(c: LinearRgba) -> Mip {
        Mip::build(4, 4, |_, _| c)
    }

    fn stamp(art: &Mip, at: Vec2, angle: f32) -> Stamp<'_> {
        Stamp {
            at,
            angle,
            size: Vec2::splat(66.0),
            rect: Rect::new(0.0, 0.0, 4.0, 4.0),
            flip_x: false,
            flip_y: false,
            tint: LinearRgba::WHITE,
            burn: 0.0,
            art,
            source: AssetId::default(),
            z: 0.0,
        }
    }

    #[test]
    fn it_shows_only_far_out() {
        assert_eq!(art_alpha(1.8), 0.0);
        assert_eq!(art_alpha(5.5), 0.0, "the Cosmoteer roof has the mid zoom to itself");
        assert_eq!(art_alpha(ART_SOLID), 1.0);
        assert_eq!(art_alpha(20.0), 1.0);
    }

    #[test]
    fn a_block_paints_its_own_cell_and_the_rim_goes_round_it() {
        let red = solid(LinearRgba::RED);
        let s = stamp(&red, Vec2::ZERO, 0.0);
        let mut canvas = Canvas::over(s.bounds()).unwrap();
        canvas.paint(&s);
        let filled = canvas.px.iter().filter(|c| c.alpha > 0.99).count();
        assert_eq!(filled, (PX_PER_CELL * PX_PER_CELL) as usize, "one block is 6x6 pixels");
        canvas.rim();
        let rim = canvas.px.iter().filter(|c| (c.alpha - RIM.alpha).abs() < 1e-4).count();
        assert_eq!(rim, 4 * PX_PER_CELL as usize + 4, "a one-pixel ring round it");
    }

    #[test]
    fn upper_sprites_cover_lower_ones() {
        let red = solid(LinearRgba::RED);
        let blue = solid(LinearRgba::BLUE);
        let lower = stamp(&red, Vec2::ZERO, 0.0);
        let upper = stamp(&blue, Vec2::ZERO, 0.0);
        let mut canvas = Canvas::over(lower.bounds()).unwrap();
        canvas.paint(&lower);
        canvas.paint(&upper);
        let c = canvas.px[(canvas.h / 2 * canvas.w + canvas.w / 2) as usize];
        assert!(c.blue > 0.99 && c.red < 0.01);
    }

    #[test]
    fn half_see_through_tints_blend() {
        let white = Mip::white();
        let base = stamp(&white, Vec2::ZERO, 0.0);
        let mut shade = stamp(&white, Vec2::ZERO, 0.0);
        shade.tint = LinearRgba::new(0.0, 0.0, 0.0, 0.5);
        let mut canvas = Canvas::over(base.bounds()).unwrap();
        canvas.paint(&base);
        canvas.paint(&shade);
        let c = canvas.px[(canvas.h / 2 * canvas.w + canvas.w / 2) as usize];
        assert!((c.red - 0.5).abs() < 0.01 && (c.alpha - 1.0).abs() < 0.01, "{c:?}");
    }

    #[test]
    fn art_turns_with_its_sprite() {
        // Left half red, right half blue; turned a half turn the blue is on
        // the left.
        let art = Mip::build(4, 4, |x, _| if x < 2 { LinearRgba::RED } else { LinearRgba::BLUE });
        let s = stamp(&art, Vec2::ZERO, std::f32::consts::PI);
        let mut canvas = Canvas::over(s.bounds()).unwrap();
        canvas.paint(&s);
        let row = canvas.h / 2;
        let left = canvas.px[(row * canvas.w + 3) as usize];
        let right = canvas.px[(row * canvas.w + canvas.w - 4) as usize];
        assert!(left.blue > 0.9 && right.red > 0.9, "{left:?} {right:?}");
    }

    #[test]
    fn the_picture_sits_over_the_blocks() {
        let red = solid(LinearRgba::RED);
        let a = stamp(&red, Vec2::new(0.0, -33.0), 0.0);
        let b = stamp(&red, Vec2::new(660.0, -33.0), 0.0);
        let canvas = Canvas::over(a.bounds().union(b.bounds())).unwrap();
        let (centre, size) = canvas.placement();
        assert!((centre - Vec2::new(330.0, -33.0)).length() < UNITS_PER_PX, "{centre}");
        assert!(size.x >= 726.0 && size.y >= 66.0);
    }

    #[test]
    fn mips_average_the_art() {
        let m = Mip::build(64, 64, |x, _| if x % 2 == 0 { LinearRgba::WHITE } else { LinearRgba::NONE });
        assert_eq!((m.w, m.h), (32, 32));
        let c = m.at(10.0, 10.0);
        assert!((c.alpha - 0.5).abs() < 1e-4 && (c.red - 0.5).abs() < 1e-4, "premultiplied half cover: {c:?}");
    }

    /// The promise everything rests on: a square painted over comes out exactly
    /// as a whole new bake would, rim and neighbours included.
    #[test]
    fn a_patch_is_the_same_as_a_new_bake() {
        let red = solid(LinearRgba::RED);
        let blue = solid(LinearRgba::BLUE);
        let grid: Vec<Stamp> = (0..20)
            .flat_map(|x| (0..12).map(move |y| (x, y)))
            .map(|(x, y)| stamp(if (x + y) % 3 == 0 { &blue } else { &red }, Vec2::new(x as f32, y as f32) * 66.0, 0.0))
            .collect();
        let bounds = grid.iter().map(Stamp::bounds).reduce(|a, b| a.union(b)).unwrap();
        let bake = |stamps: &[&Stamp]| {
            let mut canvas = Canvas::over(bounds).unwrap();
            for s in stamps {
                canvas.paint(s);
            }
            canvas.rim();
            canvas.finish()
        };
        let all: Vec<&Stamp> = grid.iter().collect();
        let mut patched = bake(&all);

        // Shot out of the middle, and right on a square's corner.
        let shot = |s: &Stamp| s.at == Vec2::new(528.0, 528.0) || s.at == Vec2::new(198.0, 264.0);
        let after: Vec<&Stamp> = grid.iter().filter(|s| !shot(s)).collect();
        let mut squares = HashSet::default();
        for s in grid.iter().filter(|s| shot(s)) {
            mark_squares(s.bounds(), &mut squares);
        }
        let bounded: Vec<(Rect, &Stamp)> = after.iter().map(|s| (s.bounds(), *s)).collect();
        for square in &squares {
            patched.repaint(*square, &bounded);
        }

        let fresh = bake(&after);
        assert_eq!((patched.w, patched.h), (fresh.w, fresh.h));
        assert!(patched.bytes == fresh.bytes, "the patch differs from a new bake");
    }

    fn app(zoom: f32) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Image>>();
        app.insert_resource(CameraState { zoom, ..default() });
        app.add_systems(Update, (attach_far_art, bake_far_art, show_far_art).chain());
        app
    }

    fn ship(app: &mut App) -> Entity {
        app.world_mut().spawn((Ship, Transform::default(), Visibility::Inherited, crate::vfx::ship_roof::Scars::default())).id()
    }

    fn hull(app: &mut App, ship: Entity, cell: IVec2) -> Entity {
        app.world_mut()
            .spawn((
                HullSegment { health: 100.0, max_health: 100.0, grid_position: cell, ..default() },
                Sprite { color: Color::srgb(0.5, 0.6, 0.7), custom_size: Some(Vec2::splat(66.0)), ..default() },
                Transform::from_translation(crate::building::grid_to_local(cell).extend(0.1)),
                Visibility::Inherited,
                ChildOf(ship),
            ))
            .id()
    }

    /// Run a frame as if long enough has gone by for a look.
    fn look(app: &mut App, ship: Entity) {
        app.world_mut().get_mut::<FarArt>(ship).unwrap().seen_at = -100.0;
        app.update();
    }

    /// The colour in the picture at the middle of a cell, in sRGB bytes.
    fn pixel(app: &App, ship: Entity, cell: IVec2) -> [u8; 4] {
        let art = app.world().get::<FarArt>(ship).unwrap();
        let picture = art.picture.as_ref().unwrap();
        let at = ((crate::building::grid_to_local(cell) - picture.origin) / UNITS_PER_PX).as_ivec2();
        let i = ((picture.h as i32 - 1 - at.y) * picture.w as i32 + at.x) as usize * 4;
        // What the renderer was given, not just what's kept to paint on.
        let image = app.world().resource::<Assets<Image>>().get(&art.image).unwrap();
        let data = image.data.as_ref().unwrap();
        [data[i], data[i + 1], data[i + 2], data[i + 3]]
    }

    fn repaints(app: &App, ship: Entity) -> u32 {
        app.world().get::<FarArt>(ship).unwrap().repaints
    }

    /// The complaint: zoomed out, a hit has to show.
    #[test]
    fn a_hit_shows_in_the_picture() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let (hit, other) = (hull(&mut app, s, IVec2::ZERO), IVec2::new(1, 0));
        hull(&mut app, s, other);
        app.update();
        app.update();
        let (before, other_before) = (pixel(&app, s, IVec2::ZERO), pixel(&app, s, other));

        app.world_mut().get_mut::<HullSegment>(hit).unwrap().health = 20.0;
        look(&mut app, s);

        let after = pixel(&app, s, IVec2::ZERO);
        assert!(after[2] < before[2], "not darker: {before:?} to {after:?}");
        assert!(after[0] as f32 / after[2] as f32 > before[0] as f32 / before[2] as f32, "not hotter: {before:?} to {after:?}");
        assert_eq!(pixel(&app, s, other), other_before, "a block that wasn't hit changed");
    }

    /// A hole opens, and doesn't wait for a bake.
    #[test]
    fn a_dead_block_leaves_a_hole() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let dead = hull(&mut app, s, IVec2::ZERO);
        hull(&mut app, s, IVec2::new(4, 0));
        // So a smaller picture doesn't move the place being looked at.
        hull(&mut app, s, IVec2::new(-4, 0));
        app.update();
        app.update();
        assert_eq!(pixel(&app, s, IVec2::ZERO)[3], 255);

        app.world_mut().entity_mut(dead).despawn();
        look(&mut app, s);
        assert_eq!(pixel(&app, s, IVec2::ZERO)[3], 0, "the block is still in the picture");
    }

    /// Plating lies over most of a ship, so what's hit under it has to show on
    /// it or a fight only shows when whole blocks go.
    #[test]
    fn damage_shows_through_the_roof() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let hit = hull(&mut app, s, IVec2::ZERO);
        hull(&mut app, s, IVec2::new(1, 0));
        // Plating has art of its own; any image that isn't the blank will do.
        let art = app.world_mut().resource_mut::<Assets<Image>>().add(Image::new_fill(
            Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            TextureDimension::D2,
            &[200, 200, 200, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        ));
        app.world_mut().spawn((
            Sprite { image: art, custom_size: Some(Vec2::splat(66.0)), ..default() },
            Transform::from_translation(crate::building::grid_to_local(IVec2::ZERO).extend(0.5)),
            Visibility::Inherited,
            crate::vfx::ship_roof::RoofTile(1.0),
            ChildOf(s),
        ));
        app.update();
        app.update();
        let before = pixel(&app, s, IVec2::ZERO);
        app.world_mut().get_mut::<HullSegment>(hit).unwrap().health = 20.0;
        look(&mut app, s);
        let after = pixel(&app, s, IVec2::ZERO);
        assert!(after[2] < before[2], "the roof hid it: {before:?} to {after:?}");
    }

    /// A roof relaid the same is all new entities and nothing to paint; a ship
    /// that's fine costs no repaints.
    #[test]
    fn nothing_changed_paints_nothing() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        hull(&mut app, s, IVec2::ZERO);
        let same = hull(&mut app, s, IVec2::X);
        app.update();
        app.update();
        let painted = repaints(&app, s);
        look(&mut app, s);
        assert_eq!(repaints(&app, s), painted, "looked over, unchanged, and painted anyway");

        app.world_mut().entity_mut(same).despawn();
        hull(&mut app, s, IVec2::X);
        look(&mut app, s);
        assert_eq!(repaints(&app, s), painted, "the same block again isn't a change");
    }

    /// A block built past the edge of the picture makes the picture bigger.
    #[test]
    fn building_past_the_edge_grows_the_picture() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        hull(&mut app, s, IVec2::ZERO);
        app.update();
        app.update();
        let far = IVec2::new(30, 0);
        hull(&mut app, s, far);
        look(&mut app, s);
        assert_eq!(pixel(&app, s, far)[3], 255, "the new block isn't in the picture");
    }

    /// Fully out the art stops drawing; a block built later stops too, and they
    /// all come back on zooming in.
    #[test]
    fn the_picture_stands_in_far_out_and_steps_back_close_in() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let b = hull(&mut app, s, IVec2::ZERO);
        app.update();
        app.update();
        assert!(app.world().get::<FarHidden>(b).is_some());
        let late = hull(&mut app, s, IVec2::X);
        look(&mut app, s);
        app.update();
        assert!(app.world().get::<FarHidden>(late).is_some(), "a new block is still drawn under the picture");

        app.world_mut().resource_mut::<CameraState>().zoom = 3.0;
        app.update();
        app.update();
        assert!(app.world().get::<FarHidden>(b).is_none());
        assert!(app.world().get::<RenderLayers>(late).is_none());
    }
}
