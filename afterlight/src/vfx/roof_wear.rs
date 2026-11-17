//! Damage on the roof.
//!
//! Zoomed out, a ship is its roof (ship_roof), and the roof only changed when
//! a block was gone: a block shot down to a sliver of health looked brand new
//! until the moment it broke. Most hits never break anything, so fights left
//! no mark at all. Here every hurt block that's still standing gets a scorch
//! laid over the roof above it, faint for a graze and dark for a block nearly
//! gone. The scorches are soft blotches a little bigger than a cell, so a
//! cluster of hits reads as one stain and not as a grid.
//!
//! A block gone is ship_roof's (Scars, the hole itself); its scorch goes with
//! it. Out where the far pictures stand in (far_art), they show the same wear
//! themselves, so the scorches fade as those fade in.

use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::ai_ship::components::AiShip;
use crate::building::{footprints, ShipGrid, GRID_SIZE};
use crate::camera::CameraState;
use crate::components::{DestroyedModule, HullDestroyed, HullSegment, Module, PressureOverlayVisible, Ship};

/// Above the roof, its decals and its edges; below guns lifted over it.
const WEAR_Z: f32 = 0.75;
/// Soot: burnt metal, a shade warmer than black.
const SOOT: (f32, f32, f32) = (0.06, 0.04, 0.03);
/// How dark a block all but gone is.
const DARKEST: f32 = 0.82;
/// Steps of wear: a block repaired or hit a little more only redraws when it
/// crosses one.
const STEPS: f32 = 10.0;
/// A scorch's side, in cells: over two, so neighbours run together into one
/// stain (at 1.45 a row of hurt blocks read as a row of dots).
const SPREAD: f32 = 2.4;
const VARIANTS: usize = 4;

/// A scorch over the block it marks.
#[derive(Component)]
pub struct RoofWear {
    block: Entity,
    ship: Entity,
    level: u8,
}

/// The blotch shapes, made once.
#[derive(Resource)]
pub struct WearImages(Vec<Handle<Image>>);

impl FromWorld for WearImages {
    fn from_world(world: &mut World) -> Self {
        let mut images = world.resource_mut::<Assets<Image>>();
        WearImages((0..VARIANTS).map(|seed| images.add(blotch(seed as u32))).collect())
    }
}

fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

/// -1..1 each way, from a hash.
fn jitter(h: u32) -> Vec2 {
    Vec2::new(((h >> 17) & 0xff) as f32 / 127.5 - 1.0, ((h >> 25) & 0x7f) as f32 / 63.5 - 1.0)
}

/// Smooth value noise in 0..1.
fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let v = |dx: i32, dy: i32| (hash(xi + dx, yi + dy, seed) & 0xffff) as f32 / 65535.0;
    let top = v(0, 0) + (v(1, 0) - v(0, 0)) * s(fx);
    let bottom = v(0, 1) + (v(1, 1) - v(0, 1)) * s(fx);
    top + (bottom - top) * s(fy)
}

/// How much of a scorch covers a point, its centre 0,0 and its edge at 1: full
/// in the middle, a ragged soft edge.
pub fn coverage(x: f32, y: f32, seed: u32) -> f32 {
    let r = (x * x + y * y).sqrt();
    let rag = noise(x * 3.0 + 7.0, y * 3.0 + 3.0, seed) * 0.6 + noise(x * 7.0, y * 7.0, seed + 11) * 0.4;
    let edge = 0.5 + 0.5 * rag;
    let t = ((edge - r) / 0.7).clamp(0.0, 1.0);
    // Grain inside, so it reads as soot and not as paint.
    let grain = 0.8 + 0.2 * noise(x * 11.0, y * 11.0, seed + 5);
    t * t * (3.0 - 2.0 * t) * grain
}

fn blotch(seed: u32) -> Image {
    const SIDE: u32 = 48;
    let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for py in 0..SIDE {
        for px in 0..SIDE {
            let x = (px as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0;
            let y = (py as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0;
            let a = coverage(x, y, seed);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0).round() as u8]);
        }
    }
    Image::new(
        Extent3d { width: SIDE, height: SIDE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

/// How worn a block shows, in steps: 0 for untouched (no scorch), STEPS for
/// all but gone.
pub fn wear_level(health: f32, max: f32) -> u8 {
    if max <= 0.0 || health >= max {
        return 0;
    }
    let lost = (1.0 - (health / max).clamp(0.0, 1.0)) * STEPS;
    // A graze still shows: anything hurt is at least one step.
    (lost.ceil() as u8).clamp(1, STEPS as u8)
}

/// The scorch's darkness for a level: grazes faint, the last hits darkest.
pub fn wear_alpha(level: u8) -> f32 {
    let t = level as f32 / STEPS;
    DARKEST * (0.18 + 0.82 * t.powf(1.3))
}

/// Lay, darken, lighten and lift the scorches as blocks are hit and mended.
#[allow(clippy::type_complexity)]
pub fn wear_roofs(
    mut commands: Commands,
    images: Res<WearImages>,
    hurt_hull: Query<(Entity, &HullSegment, &Transform, &ChildOf), (Changed<HullSegment>, Without<HullDestroyed>)>,
    hurt_modules: Query<(Entity, &Module, &Transform, &ChildOf), (Changed<Module>, Without<DestroyedModule>)>,
    ships: Query<(), Or<(With<Ship>, With<AiShip>)>>,
    mut index: Local<HashMap<Entity, Entity>>,
    mut scorches: Query<&mut RoofWear>,
) {
    let mut mark = |commands: &mut Commands, block: Entity, ship: Entity, level: u8, at: Vec2, cells: Vec2, cell: IVec2| {
        if let Some(&wear) = index.get(&block) {
            if let Ok(mut w) = scorches.get_mut(wear) {
                if level == 0 {
                    commands.entity(wear).try_despawn();
                    index.remove(&block);
                } else if w.level != level {
                    // fade_wear sets the darkness from the level.
                    w.level = level;
                }
                return;
            }
            index.remove(&block);
        }
        if level == 0 {
            return;
        }
        let h = hash(cell.x, cell.y, 1);
        let turn = (h % 4) as f32 * std::f32::consts::FRAC_PI_2 + ((h >> 8) % 100) as f32 * 0.003;
        let wear = commands
            .spawn((
                Sprite {
                    image: images.0[(h as usize >> 4) % VARIANTS].clone(),
                    color: Color::srgba(SOOT.0, SOOT.1, SOOT.2, 0.0),
                    custom_size: Some(cells * GRID_SIZE * SPREAD),
                    flip_x: h & 0x10000 != 0,
                    ..default()
                },
                // Off centre a little, so a row of hits doesn't line up.
                Transform::from_translation((at + jitter(h) * GRID_SIZE * 0.35).extend(WEAR_Z)).with_rotation(Quat::from_rotation_z(turn)),
                RoofWear { block, ship, level },
                ChildOf(ship),
            ))
            .id();
        index.insert(block, wear);
    };
    for (block, hull, tf, parent) in hurt_hull.iter() {
        if ships.get(parent.parent()).is_err() {
            continue;
        }
        let level = wear_level(hull.health, hull.max_health);
        mark(&mut commands, block, parent.parent(), level, tf.translation.truncate(), Vec2::ONE, hull.grid_position);
    }
    for (block, module, tf, parent) in hurt_modules.iter() {
        if ships.get(parent.parent()).is_err() {
            continue;
        }
        let level = wear_level(module.health, module.max_health);
        let cells = ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprints::footprint_override(module.module_type));
        let (lo, hi) = cells.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        let span = if lo.x <= hi.x { (hi - lo + IVec2::ONE).as_vec2() } else { Vec2::ONE };
        mark(&mut commands, block, parent.parent(), level, tf.translation.truncate(), span, module.grid_position);
    }
}

/// A scorch's alpha follows the roof under it: in with the roof as the camera
/// pulls back, out as the far picture takes over. A block gone (or gone off
/// with a broken piece) takes its scorch with it.
#[allow(clippy::type_complexity)]
pub fn fade_wear(
    mut commands: Commands,
    camera: Res<CameraState>,
    inside: Res<super::ship_roof::SeeInside>,
    view: super::ship_roof::PlayerRoofView,
    owners: Query<(Has<Ship>, Has<PressureOverlayVisible>)>,
    standing: Query<&ChildOf, (Or<(With<HullSegment>, With<Module>)>, Without<HullDestroyed>, Without<DestroyedModule>)>,
    mut scorches: Query<(Entity, &RoofWear, &mut Sprite)>,
) {
    let far = 1.0 - super::far_art::art_alpha(camera.zoom);
    for (wear, w, mut sprite) in scorches.iter_mut() {
        if standing.get(w.block).map(|p| p.parent()) != Ok(w.ship) {
            commands.entity(wear).try_despawn();
            continue;
        }
        let Ok((mine, air)) = owners.get(w.ship) else {
            commands.entity(wear).try_despawn();
            continue;
        };
        let roof = if mine {
            view.alpha(if inside.on || air { 0.0 } else { super::ship_roof::roof_alpha(camera.zoom) })
        } else if super::ship_roof::roof_all() {
            super::ship_roof::roof_alpha(camera.zoom)
        } else {
            0.0
        };
        let want = roof * far * wear_alpha(w.level);
        if (sprite.color.alpha() - want).abs() > 0.004 {
            sprite.color.set_alpha(want);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graze_shows_faintly_and_a_block_nearly_gone_shows_darkest() {
        assert_eq!(wear_level(100.0, 100.0), 0);
        assert_eq!(wear_level(99.0, 100.0), 1);
        assert_eq!(wear_level(1.0, 100.0), STEPS as u8);
        assert_eq!(wear_level(0.0, 0.0), 0);
        let (faint, dark) = (wear_alpha(1), wear_alpha(STEPS as u8));
        assert!(faint > 0.1 && faint < 0.25, "a graze reads, quietly: {faint}");
        assert!((dark - DARKEST).abs() < 1e-4);
        for l in 1..STEPS as u8 {
            assert!(wear_alpha(l) < wear_alpha(l + 1));
        }
    }

    #[test]
    fn a_scorch_is_solid_in_the_middle_and_gone_at_its_edge() {
        for seed in 0..VARIANTS as u32 {
            assert!(coverage(0.0, 0.0, seed) > 0.5);
            assert_eq!(coverage(1.0, 0.0, seed), 0.0);
            assert_eq!(coverage(0.0, -1.0, seed), 0.0);
        }
    }
}
