//! Ships seen from very far out: a plain copy of every block, not its art.
//!
//! Past the Cosmoteer roof (vfx::ship_roof) the camera keeps going -- out to
//! zoom 90, where a block is a pixel across. Its real art there is a shimmer
//! of detail far too small to see. So from `ART_FROM` each block gets a flat
//! copy of itself in the average colour of its art, as a child of the block.
//!
//! Being a child is the point. There used to be one picture per ship, baked
//! from its blocks and rebaked every few seconds at best, so from far out your
//! own ship and the enemy's looked untouched while they were being taken
//! apart. A copy can't fall behind: a hit darkens that block's copy the same
//! frame, a block that dies or is hidden takes its copy with it, and a block
//! that's built, moved, or blown off with a severed section brings its own.
//!
//! Fully out, the real sprites stop drawing (moved to a render layer no camera
//! looks at) and every copy batches as one plain white square, so it's cheaper
//! than the art it stands in for. Zoom back in and they're back. Close in no
//! copies are made at all; a ship nobody ever zooms out on never pays for them.

use bevy::camera::visibility::RenderLayers;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use crate::camera::CameraState;
use crate::components::{HullSegment, Module};

/// Zoom where the copies start to show over the blocks...
pub const ART_FROM: f32 = 8.0;
/// ...and where they stand in for them.
pub const ART_SOLID: f32 = 10.5;

/// Over the block it copies, and nothing else: the blocks' own order stands.
const COPY_Z: f32 = 0.001;
/// Over a ship's blocks (0-1) and its weapons raised over the roof (0.75),
/// under its turrets (1.2).
const EMBER_Z: f32 = 1.15;
/// Copies made a frame, so a fleet coming into view spreads over a few frames.
/// A block keeps drawing its real art until it has one.
const COPIES_PER_FRAME: usize = 5000;
/// Copies overlap this much so a ship at an angle doesn't show hairlines of
/// space between its blocks.
const COPY_OVERLAP: f32 = 1.04;
/// How much brighter a copy is than the art it averages.
const COPY_GAIN: f32 = 1.5;
/// Pixels looked at each way when averaging a block's art.
const SAMPLES: u32 = 16;
/// No camera draws this layer.
const HIDDEN_LAYER: usize = 29;
/// The glow of a wound.
const EMBER: (f32, f32, f32) = (1.0, 0.22, 0.03);

pub fn art_alpha(zoom: f32) -> f32 {
    let t = ((zoom - ART_FROM) / (ART_SOLID - ART_FROM)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// On a block: its copy.
#[derive(Component)]
pub struct FarCopyOf(Entity);

/// The copy itself, a child of the block it copies. `solid` is its alpha at
/// full strength, which the zoom fades.
#[derive(Component)]
pub struct FarCopy {
    solid: f32,
}

/// A wound glowing on a far ship: a child of the ship.
#[derive(Component)]
pub struct FarEmber;

/// How solid an ember is at full strength.
const EMBER_SOLID: f32 = 0.95;

/// A block not drawn while its copy stands in for it.
#[derive(Component)]
pub struct FarHidden;

/// How far faded in the copies are now. Negative when they need setting
/// again whatever the zoom.
#[derive(Resource)]
pub struct FarFade(f32);

impl Default for FarFade {
    fn default() -> Self {
        Self(-1.0)
    }
}

/// The average colour of some art, by image and the part of it in use.
#[derive(Default)]
pub struct AverageCache(HashMap<(AssetId<Image>, [i32; 4]), LinearRgba>);

impl AverageCache {
    /// None while the art is still loading.
    fn get(&mut self, images: &Assets<Image>, sprite: &Sprite) -> Option<LinearRgba> {
        let id = sprite.image.id();
        if id == Handle::<Image>::default().id() {
            return Some(LinearRgba::WHITE);
        }
        let key = sprite.rect.map_or([0, 0, -1, -1], |r| [r.min.x as i32, r.min.y as i32, r.max.x as i32, r.max.y as i32]);
        if let Some(found) = self.0.get(&(id, key)) {
            return Some(*found);
        }
        let image = images.get(id)?;
        // Art that can't be read back (kept on the GPU only) is a mid grey,
        // not a block that never gets a copy and so never updates.
        let average = if image.data.is_none() {
            LinearRgba::gray(0.3)
        } else {
            let (w, h) = (image.width() as f32, image.height() as f32);
            let rect = sprite.rect.unwrap_or(Rect::new(0.0, 0.0, w, h));
            average_of(rect, |x, y| image.get_color_at(x, y).map(|c| c.to_linear()).unwrap_or(LinearRgba::NONE))
        };
        self.0.insert((id, key), average);
        Some(average)
    }
}

/// The colour of some art, weighted by how solid each pixel is, with alpha for
/// how much of the area it covers: a corner wedge is half see-through, which
/// at a few pixels is what a diagonal edge looks like.
fn average_of(rect: Rect, at: impl Fn(u32, u32) -> LinearRgba) -> LinearRgba {
    let (mut sum, mut coverage, mut n) = (Vec3::ZERO, 0.0f32, 0.0f32);
    for j in 0..SAMPLES {
        for i in 0..SAMPLES {
            let x = rect.min.x + rect.width() * (i as f32 + 0.5) / SAMPLES as f32;
            let y = rect.min.y + rect.height() * (j as f32 + 0.5) / SAMPLES as f32;
            let c = at(x as u32, y as u32);
            sum += Vec3::new(c.red, c.green, c.blue) * c.alpha;
            coverage += c.alpha;
            n += 1.0;
        }
    }
    if coverage <= 0.0 {
        return LinearRgba::NONE;
    }
    let c = sum / coverage;
    LinearRgba::new(c.x, c.y, c.z, coverage / n)
}

/// How much of a block is left, 0..1.
fn left_of(hull: Option<&HullSegment>, module: Option<&Module>) -> f32 {
    let (health, max) = match (hull, module) {
        (Some(h), _) => (h.health, h.max_health),
        (_, Some(m)) => (m.health, m.max_health),
        _ => return 1.0,
    };
    if max > 0.0 { (health / max).clamp(0.0, 1.0) } else { 1.0 }
}

/// A copy's colour at full strength: darker and hotter as the block is shot
/// up, so damage reads against the ship around it and not just as a slightly
/// duller patch.
fn copy_color(art: LinearRgba, tint: LinearRgba, left: f32) -> LinearRgba {
    // Brighter than the art: a dark hull a few pixels across against black
    // space otherwise reads as nothing but its shield.
    let dark = COPY_GAIN * (0.3 + 0.7 * left);
    let burn = (1.0 - left) * 0.32;
    LinearRgba::new(
        art.red * tint.red * dark + EMBER.0 * burn,
        art.green * tint.green * dark + EMBER.1 * burn,
        art.blue * tint.blue * dark + EMBER.2 * burn,
        art.alpha * tint.alpha,
    )
}

/// Give blocks their copies, a few thousand a frame, once they can be seen.
#[allow(clippy::type_complexity)]
pub fn attach_far_copies(
    mut commands: Commands,
    camera: Res<CameraState>,
    images: Res<Assets<Image>>,
    blocks: Query<(Entity, &Sprite, Option<&HullSegment>, Option<&Module>), (Or<(With<HullSegment>, With<Module>)>, Without<FarCopyOf>)>,
    mut cache: Local<AverageCache>,
) {
    // DEPTHS_NO_FAR_ART=1: blocks all the way out, to measure against.
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *OFF.get_or_init(|| std::env::var_os("DEPTHS_NO_FAR_ART").is_some()) {
        return;
    }
    let fade = art_alpha(camera.zoom);
    if fade <= 0.0 {
        return;
    }
    for (block, sprite, hull, module) in blocks.iter().take(COPIES_PER_FRAME) {
        let Some(art) = cache.get(&images, sprite) else { continue };
        let size = sprite.custom_size.or_else(|| sprite.rect.map(|r| r.size())).or_else(|| images.get(sprite.image.id()).map(|i| i.size_f32()));
        let Some(size) = size else { continue };
        let color = copy_color(art, sprite.color.to_linear(), left_of(hull, module));
        let copy = commands
            .spawn((
                Sprite {
                    color: Color::LinearRgba(color.with_alpha(color.alpha * fade)),
                    custom_size: Some(size * COPY_OVERLAP),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, COPY_Z),
                FarCopy { solid: color.alpha },
                ChildOf(block),
            ))
            .id();
        commands.entity(block).try_insert(FarCopyOf(copy));
    }
}

/// Damage, repairs and repaints, the frame they happen.
#[allow(clippy::type_complexity)]
pub fn refresh_far_copies(
    camera: Res<CameraState>,
    images: Res<Assets<Image>>,
    changed: Query<
        (&Sprite, Option<&HullSegment>, Option<&Module>, &FarCopyOf),
        (Without<FarCopy>, Or<(Changed<HullSegment>, Changed<Module>, Changed<Sprite>)>),
    >,
    mut copies: Query<(&mut Sprite, &mut FarCopy)>,
    mut cache: Local<AverageCache>,
) {
    let fade = art_alpha(camera.zoom);
    for (sprite, hull, module, link) in changed.iter() {
        let Some(art) = cache.get(&images, sprite) else { continue };
        let Ok((mut copy, mut far)) = copies.get_mut(link.0) else { continue };
        let color = copy_color(art, sprite.color.to_linear(), left_of(hull, module));
        let color = Color::LinearRgba(color.with_alpha(color.alpha * fade));
        if copy.color != color {
            copy.color = color;
        }
        far.solid = art.alpha * sprite.color.alpha();
    }
}

/// Wounds glow, over the blocks: a hole in a dark ship against dark space
/// doesn't show from far out. Laid again when a ship's scars change.
#[allow(clippy::type_complexity)]
pub fn far_embers(
    mut commands: Commands,
    camera: Res<CameraState>,
    ships: Query<(Entity, Ref<super::ship_roof::Scars>, Option<&Children>)>,
    embers: Query<(), With<FarEmber>>,
    hull: Query<(&HullSegment, &Visibility)>,
    mut laid: Local<HashSet<Entity>>,
) {
    let fade = art_alpha(camera.zoom);
    if fade <= 0.0 {
        laid.clear();
        return;
    }
    for (ship, scars, children) in ships.iter() {
        if laid.contains(&ship) && !scars.is_changed() {
            continue;
        }
        laid.insert(ship);
        let children = || children.into_iter().flatten().copied();
        for ember in children().filter(|c| embers.contains(*c)) {
            commands.entity(ember).despawn();
        }
        if scars.0.is_empty() && scars.1.is_empty() {
            continue;
        }
        let standing: HashSet<IVec2> = children()
            .filter_map(|c| hull.get(c).ok())
            .filter(|(_, v)| **v != Visibility::Hidden)
            .map(|(h, _)| h.grid_position)
            .collect();
        for (cell, bright) in super::ship_roof::embers(&scars, &standing) {
            let color = LinearRgba::new(EMBER.0 * bright, EMBER.1 * bright, EMBER.2 * bright, EMBER_SOLID * fade);
            commands.entity(ship).with_child((
                Sprite {
                    color: Color::LinearRgba(color),
                    custom_size: Some(Vec2::splat(crate::building::GRID_SIZE * 1.3)),
                    ..default()
                },
                Transform::from_translation(crate::building::grid_to_local(cell).extend(EMBER_Z)),
                FarEmber,
            ));
        }
    }
}

/// Fade the copies and embers with the zoom. Fully out, the real sprites stop
/// drawing; coming back in, they start again.
#[allow(clippy::type_complexity)]
pub fn show_far_copies(
    mut commands: Commands,
    camera: Res<CameraState>,
    mut last: ResMut<FarFade>,
    mut copies: Query<(&mut Sprite, &mut Visibility, Option<&FarCopy>), Or<(With<FarCopy>, With<FarEmber>)>>,
    copied: Query<Entity, (With<FarCopyOf>, Without<FarHidden>)>,
    roofs: Query<Entity, (Or<(With<super::ship_roof::RoofTile>, With<super::ship_roof::AiRoofTile>)>, Without<FarHidden>)>,
    hidden: Query<Entity, With<FarHidden>>,
) {
    let fade = art_alpha(camera.zoom);
    if fade != last.0 {
        last.0 = fade;
        let vis = if fade > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        for (mut sprite, mut visibility, copy) in &mut copies {
            let solid = copy.map_or(EMBER_SOLID, |c| c.solid);
            sprite.color.set_alpha(solid * fade);
            if *visibility != vis {
                *visibility = vis;
            }
        }
    }
    // Every frame, not on crossing the line: blocks built, roofs relaid and
    // severed sections join in whenever they appear. Mostly nothing to do.
    if fade >= 1.0 {
        for block in copied.iter().chain(roofs.iter()) {
            commands.entity(block).try_insert((RenderLayers::layer(HIDDEN_LAYER), FarHidden));
        }
    } else {
        for block in hidden.iter() {
            commands.entity(block).try_remove::<(RenderLayers, FarHidden)>();
        }
    }
}

/// Off the flight screen every block draws its art again, and no copies.
pub fn clear_far_copies(
    mut commands: Commands,
    mut last: ResMut<FarFade>,
    hidden: Query<Entity, With<FarHidden>>,
    mut copies: Query<&mut Visibility, Or<(With<FarCopy>, With<FarEmber>)>>,
) {
    for block in hidden.iter() {
        commands.entity(block).try_remove::<(RenderLayers, FarHidden)>();
    }
    for mut visibility in &mut copies {
        *visibility = Visibility::Hidden;
    }
    last.0 = -1.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(zoom: f32) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Image>>();
        app.init_resource::<FarFade>();
        app.insert_resource(CameraState { zoom, ..default() });
        app.add_systems(Update, (attach_far_copies, refresh_far_copies, far_embers, show_far_copies).chain());
        app
    }

    fn block(app: &mut App, ship: Entity, cell: IVec2) -> Entity {
        app.world_mut()
            .spawn((
                HullSegment { health: 100.0, max_health: 100.0, grid_position: cell, ..default() },
                Sprite { color: Color::srgb(0.5, 0.6, 0.7), custom_size: Some(Vec2::splat(66.0)), ..default() },
                Transform::default(),
                Visibility::Inherited,
                ChildOf(ship),
            ))
            .id()
    }

    fn ship(app: &mut App) -> Entity {
        app.world_mut().spawn((Transform::default(), Visibility::Inherited, super::super::ship_roof::Scars::default())).id()
    }

    fn copy_of(app: &App, block: Entity) -> Entity {
        app.world().get::<FarCopyOf>(block).expect("no copy").0
    }

    fn color_of(app: &App, copy: Entity) -> LinearRgba {
        app.world().get::<Sprite>(copy).unwrap().color.to_linear()
    }

    #[test]
    fn it_shows_only_far_out() {
        assert_eq!(art_alpha(1.8), 0.0);
        assert_eq!(art_alpha(5.5), 0.0, "the Cosmoteer roof has the mid zoom to itself");
        assert_eq!(art_alpha(ART_SOLID), 1.0);
        assert_eq!(art_alpha(20.0), 1.0);
    }

    /// Close in, nobody pays: no copies at all.
    #[test]
    fn no_copies_are_made_close_in() {
        let mut app = app(3.0);
        let s = ship(&mut app);
        let b = block(&mut app, s, IVec2::ZERO);
        app.update();
        app.update();
        assert!(app.world().get::<FarCopyOf>(b).is_none());
    }

    /// The whole point: a hit shows the frame it lands.
    #[test]
    fn a_hit_darkens_its_copy_at_once() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let (hit, other) = (block(&mut app, s, IVec2::ZERO), block(&mut app, s, IVec2::X));
        app.update();
        let (copy, other_copy) = (copy_of(&app, hit), copy_of(&app, other));
        app.update();
        let before = color_of(&app, copy);
        let other_before = color_of(&app, other_copy);

        app.world_mut().get_mut::<HullSegment>(hit).unwrap().health = 20.0;
        app.update();

        let after = color_of(&app, copy);
        assert!(after.blue < before.blue * 0.7, "not darker: {before:?} to {after:?}");
        assert!(after.red / after.blue > before.red / before.blue, "not hotter: {before:?} to {after:?}");
        assert_eq!(color_of(&app, other_copy), other_before, "a block that wasn't hit changed");
    }

    /// Repairs show too, not only damage.
    #[test]
    fn a_repair_brings_the_colour_back() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let b = block(&mut app, s, IVec2::ZERO);
        app.update();
        let copy = copy_of(&app, b);
        app.update();
        let whole = color_of(&app, copy);
        app.world_mut().get_mut::<HullSegment>(b).unwrap().health = 10.0;
        app.update();
        app.world_mut().get_mut::<HullSegment>(b).unwrap().health = 100.0;
        app.update();
        assert_eq!(color_of(&app, copy), whole);
    }

    /// A block that dies takes its copy with it: the hole is real.
    #[test]
    fn a_dead_block_takes_its_copy_with_it() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let b = block(&mut app, s, IVec2::ZERO);
        app.update();
        let copy = copy_of(&app, b);
        app.world_mut().entity_mut(b).despawn();
        app.update();
        assert!(app.world().get_entity(copy).is_err());
    }

    /// Built while you're zoomed out, a block still shows.
    #[test]
    fn a_new_block_gets_a_copy() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        block(&mut app, s, IVec2::ZERO);
        app.update();
        let late = block(&mut app, s, IVec2::Y);
        app.update();
        assert!(app.world().get::<FarCopyOf>(late).is_some());
    }

    /// Fully out the art stops drawing but the copy doesn't; coming back in,
    /// the art returns and the copy goes away.
    #[test]
    fn copies_stand_in_far_out_and_step_back_close_in() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let b = block(&mut app, s, IVec2::ZERO);
        app.update();
        app.update();
        let copy = copy_of(&app, b);
        assert!(app.world().get::<RenderLayers>(b).is_some(), "the art still draws");
        assert!(app.world().get::<RenderLayers>(copy).is_none(), "the copy was hidden with it");

        app.world_mut().resource_mut::<CameraState>().zoom = 3.0;
        app.update();
        app.update();
        assert!(app.world().get::<RenderLayers>(b).is_none(), "the art didn't come back");
        assert_eq!(*app.world().get::<Visibility>(copy).unwrap(), Visibility::Hidden);
    }

    /// Fading in and out doesn't lose a damaged block's colour.
    #[test]
    fn fading_keeps_a_copys_colour() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        let b = block(&mut app, s, IVec2::ZERO);
        app.update();
        let copy = copy_of(&app, b);
        app.world_mut().get_mut::<HullSegment>(b).unwrap().health = 20.0;
        app.update();
        let hurt = color_of(&app, copy);
        app.world_mut().resource_mut::<CameraState>().zoom = (ART_FROM + ART_SOLID) / 2.0;
        app.update();
        assert!(color_of(&app, copy).alpha < hurt.alpha);
        app.world_mut().resource_mut::<CameraState>().zoom = 20.0;
        app.update();
        assert_eq!(color_of(&app, copy), hurt);
    }

    /// A wound glows, and stops when it's repaired.
    #[test]
    fn scars_glow_and_clear() {
        let mut app = app(20.0);
        let s = ship(&mut app);
        block(&mut app, s, IVec2::ZERO);
        app.update();
        let count = |app: &mut App| app.world_mut().query_filtered::<(), With<FarEmber>>().iter(app.world()).count();
        assert_eq!(count(&mut app), 0);
        app.world_mut().get_mut::<super::super::ship_roof::Scars>(s).unwrap().1.insert(IVec2::X);
        app.update();
        app.update();
        assert!(count(&mut app) > 0, "a torn edge didn't glow");
        app.world_mut().get_mut::<super::super::ship_roof::Scars>(s).unwrap().1.clear();
        app.update();
        app.update();
        assert_eq!(count(&mut app), 0);
    }

    /// Solid art is its own colour; a half-empty wedge is half see-through,
    /// and the empty half doesn't turn it black.
    #[test]
    fn the_average_ignores_empty_pixels() {
        let rect = Rect::new(0.0, 0.0, 16.0, 16.0);
        let solid = average_of(rect, |_, _| LinearRgba::new(0.2, 0.4, 0.6, 1.0));
        assert!((solid.red - 0.2).abs() < 1e-4 && (solid.alpha - 1.0).abs() < 1e-4);
        let wedge = average_of(rect, |x, y| if x + y < 16 { LinearRgba::new(0.2, 0.4, 0.6, 1.0) } else { LinearRgba::NONE });
        assert!((wedge.red - 0.2).abs() < 1e-3, "{wedge:?}");
        assert!(wedge.alpha > 0.35 && wedge.alpha < 0.65, "{wedge:?}");
        assert_eq!(average_of(rect, |_, _| LinearRgba::NONE), LinearRgba::NONE);
    }
}
