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
//! render layer no camera looks at); zoom back in and they're back. A ship
//! is baked again when it changes -- a block shot off opens a hole in the
//! picture too -- and every few seconds while it's in view, for paint and
//! damage tint that change without a block going. Close in nothing is baked
//! at all.
//!

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::image::ImageSampler;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::ai_ship::components::AiShip;
use crate::building::layout::LayoutVersion;
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
pub(crate) const SHIP_ART_Z: f32 = 1.15;
/// Over everything on a turret: plates, parts, gun house roof (0.5).
const TURRET_ART_Z: f32 = 0.9;
/// The rim round the hull: near-black, mostly solid.
const RIM: LinearRgba = LinearRgba { red: 0.004, green: 0.005, blue: 0.009, alpha: 0.8 };
/// How often a ship in view is baked again with nothing changed.
const REFRESH_S: f32 = 3.0;
/// Soonest a changed ship is baked again. A ship under fire changes nearly
/// every frame, and a capital's bake takes the best part of a frame: baked
/// on every change, a fight seen from far out stuttered.
const REBAKE_S: f32 = 1.5;
/// Sprites a bake costs about a millisecond for; big pictures are refreshed
/// (for paint and damage tint alone) that much less often.
const SPRITES_PER_MS: f32 = 1500.0;
/// Bakes per frame: a fleet coming into view spreads over a few frames.
const BAKES_PER_FRAME: usize = 1;
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
    baked: Option<LayoutVersion>,
    baked_at: f32,
    /// Sprites the last bake painted, for how often it's worth repeating.
    sprites: usize,
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
    pub art: &'a Mip,
}

impl Stamp<'_> {
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
        Some(LinearRgba::new(texel.red * t.red * t.alpha, texel.green * t.green * t.alpha, texel.blue * t.blue * t.alpha, texel.alpha * t.alpha))
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

    /// The picture as an sRGB image, rows top down.
    pub fn into_image(self) -> Image {
        let mut data = Vec::with_capacity((self.w * self.h * 4) as usize);
        for y in (0..self.h).rev() {
            for x in 0..self.w {
                let c = self.px[(y * self.w + x) as usize];
                let straight = if c.alpha > 0.0 {
                    LinearRgba::new(c.red / c.alpha, c.green / c.alpha, c.blue / c.alpha, c.alpha.min(1.0))
                } else {
                    LinearRgba::NONE
                };
                data.extend_from_slice(&Color::LinearRgba(straight).to_srgba().to_u8_array());
            }
        }
        let mut image = Image::new(
            Extent3d { width: self.w, height: self.h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::linear();
        image
    }

    /// Ship-local centre and size of the picture.
    pub fn placement(&self) -> (Vec2, Vec2) {
        let size = Vec2::new(self.w as f32, self.h as f32) * UNITS_PER_PX;
        (self.origin + size * 0.5, size)
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
        commands.entity(owner).try_insert(FarArt { sprite, image, baked: None, baked_at: f32::NEG_INFINITY, sprites: 0 });
    }
}

/// What a picture is painted from: a ship's hull, parts and roof, or a
/// turret's plates, parts and gun house roof -- whatever of those is drawn.
#[allow(clippy::type_complexity)]
type Pictured<'w, 's> = Query<
    'w,
    's,
    (&'static Sprite, &'static Transform, &'static Visibility, Option<&'static HullSegment>, Option<&'static Module>),
    (
        Or<(With<HullSegment>, With<Module>, With<super::ship_roof::RoofTile>, With<super::ship_roof::AiRoofTile>)>,
        Without<FarArtSprite>,
    ),
>;

/// How bright a wreck's far picture stays.
const WRECK_SHADE: f32 = 0.42;

/// The glow of a wound in the far picture.
const EMBER: (f32, f32, f32) = (1.0, 0.22, 0.03);

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

/// Bake the pictures that are out of date, a few a frame, only when they can
/// be seen.
#[allow(clippy::too_many_arguments)]
pub fn bake_far_art(
    mut commands: Commands,
    camera: Res<CameraState>,
    time: Res<Time>,
    mut images: ResMut<Assets<Image>>,
    mut owners: Query<(Entity, &mut FarArt, Option<&LayoutVersion>, Option<&Children>, Option<&super::ship_roof::Scars>)>,
    pictured: Pictured,
    mut sprites: Query<(&mut Sprite, &mut Transform), (With<FarArtSprite>, Without<HullSegment>, Without<Module>)>,
    mut cache: Local<MipCache>,
) {
    let alpha = art_alpha(camera.zoom);
    if alpha <= 0.0 {
        return;
    }
    let now = time.elapsed_secs();
    let full = alpha >= 1.0;
    // Never baked first, then changed, then the stalest.
    let mut due: Vec<(u8, f32, Entity)> = owners
        .iter()
        .filter_map(|(e, art, version, ..)| {
            let since = now - art.baked_at;
            let cost_ms = art.sprites as f32 / SPRITES_PER_MS;
            let rank = match (art.baked, version) {
                (None, _) => 0,
                (Some(b), Some(v)) if b != *v && since >= REBAKE_S * cost_ms.max(1.0) => 1,
                _ if since >= REFRESH_S * cost_ms.max(1.0) => 2,
                _ => return None,
            };
            Some((rank, art.baked_at, e))
        })
        .collect();
    due.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));

    for (_, _, owner) in due.into_iter().take(BAKES_PER_FRAME) {
        let Ok((_, mut art, version, children, scars)) = owners.get_mut(owner) else { continue };
        let mut layers: Vec<(f32, Entity, &Sprite, &Transform, f32, Option<IVec2>)> = children
            .into_iter()
            .flatten()
            .filter_map(|c| pictured.get(*c).ok().filter(|(_, _, v, ..)| **v != Visibility::Hidden).map(|(s, t, _, hull, module)| (t.translation.z, *c, s, t, wear_of(hull, module), hull.map(|h| h.grid_position))))
            .collect();
        layers.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Shrink any art not seen before, then paint bottom up.
        for (_, _, sprite, ..) in &layers {
            cache.get(&images, &sprite.image);
        }
        let white = Mip::white();
        let mut stamps: Vec<Stamp> = layers
            .iter()
            .filter_map(|(_, _, sprite, t, wear, _)| {
                let art = if sprite.image.id() == Handle::<Image>::default().id() { &white } else { cache.0.get(&sprite.image.id())? };
                let (src_w, src_h) = (art.w as f32 * art.step, art.h as f32 * art.step);
                let rect = sprite.rect.unwrap_or(Rect::new(0.0, 0.0, src_w, src_h));
                let size = sprite.custom_size.unwrap_or(rect.size()) * t.scale.truncate().abs();
                let tint = sprite.color.to_linear();
                let tint = worn(tint, *wear);
                Some(Stamp {
                    at: t.translation.truncate(),
                    angle: t.rotation.to_euler(EulerRot::ZYX).0,
                    size,
                    rect,
                    flip_x: sprite.flip_x,
                    flip_y: sprite.flip_y,
                    tint,
                    art,
                })
            })
            .collect();
        // Wounds glow, on top of everything.
        if let Some(scars) = scars {
            let standing = layers.iter().filter_map(|(.., cell)| *cell).collect();
            for (cell, bright) in super::ship_roof::embers(scars, &standing) {
                let size = Vec2::splat(crate::building::GRID_SIZE * 1.3);
                stamps.push(Stamp {
                    at: crate::building::grid_to_local(cell),
                    angle: 0.0,
                    size,
                    rect: Rect::new(0.0, 0.0, white.w as f32 * white.step, white.h as f32 * white.step),
                    flip_x: false,
                    flip_y: false,
                    tint: LinearRgba::new(EMBER.0 * bright, EMBER.1 * bright, EMBER.2 * bright, 0.95),
                    art: &white,
                });
            }
        }
        art.baked = version.copied();
        art.baked_at = now;
        let Some(bounds) = stamps.iter().map(Stamp::bounds).reduce(|a, b| a.union(b)) else { continue };
        let Some(mut canvas) = Canvas::over(bounds) else { continue };
        for stamp in &stamps {
            canvas.paint(stamp);
        }
        canvas.rim();
        let (centre, size) = canvas.placement();
        art.sprites = stamps.len();
        // DEPTHS_FAR_TRACE=1 logs each bake; DEPTHS_FAR_DUMP=<dir> writes
        // each picture there as raw RGBA.
        if std::env::var_os("DEPTHS_FAR_TRACE").is_some() {
            info!("FAR ART baked {owner}: {}x{} px from {} sprites, centre {centre}, zoom {:.1}", canvas.w, canvas.h, stamps.len(), camera.zoom);
        }
        let image = canvas.into_image();
        if let Some(dir) = std::env::var_os("DEPTHS_FAR_DUMP") {
            if let Some(data) = image.data.as_ref() {
                let path = std::path::Path::new(&dir).join(format!("{owner}_{}x{}.rgba", image.width(), image.height()));
                let _ = std::fs::write(path, data);
            }
        }
        let _ = images.insert(&art.image, image);
        if let Ok((mut sprite, mut t)) = sprites.get_mut(art.sprite) {
            sprite.custom_size = Some(size);
            t.translation = centre.extend(t.translation.z);
        }
        // Fully out, what was just painted stops drawing (blocks built or
        // laid since the last bake included).
        if full {
            for (_, block, ..) in &layers {
                commands.entity(*block).try_insert((RenderLayers::layer(HIDDEN_LAYER), FarHidden));
            }
        }
    }
}

/// Fade the pictures with the zoom; fully out, hide the blocks they stand in
/// for, and bring them back as the camera comes in.
#[allow(clippy::type_complexity)]
pub fn show_far_art(
    mut commands: Commands,
    camera: Res<CameraState>,
    owners: Query<(&FarArt, Option<&Children>, Option<&crate::ai_ship::components::AiShipState>)>,
    pictured: Pictured,
    mut sprites: Query<(&mut Sprite, &mut Visibility), (With<FarArtSprite>, Without<HullSegment>, Without<Module>)>,
    hidden: Query<Entity, With<FarHidden>>,
    mut was_full: Local<bool>,
) {
    let alpha = art_alpha(camera.zoom);
    for (art, _, state) in owners.iter() {
        let Ok((mut sprite, mut vis)) = sprites.get_mut(art.sprite) else { continue };
        let want = alpha;
        // A wreck goes dark: from 20 km its name-plate was the only sign that
        // a ship had died.
        let shade = if state.is_some_and(|s| s.is_destroyed) { WRECK_SHADE } else { 1.0 };
        let shaded = Color::srgba(shade, shade, shade, sprite.color.alpha());
        if sprite.color != shaded {
            sprite.color = shaded;
        }
        // Not shown until it's been baked once.
        let want_vis = if want > 0.0 && art.baked_at.is_finite() { Visibility::Inherited } else { Visibility::Hidden };
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
        for (art, children, _) in owners.iter() {
            if !art.baked_at.is_finite() {
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
            art,
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
}
