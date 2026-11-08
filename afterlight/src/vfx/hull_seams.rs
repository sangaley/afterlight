//! Hull tiles that join up instead of stacking their frames.
//!
//! Every hull texture carries its own frame and corner fastenings, so two
//! neighbours drew two frames and a ship read as a grid of loose plates rather
//! than one surface. Each tile now picks one of sixteen versions of itself
//! from which of its four neighbours use the same texture, with the frame
//! lifted off the sides that touch a match.
//!
//! Two tiles match when they would load the same image, not when their layer
//! or material fields happen to be equal. That is the rule that makes Outer
//! and Inner steel merge with each other -- they are the same picture -- while
//! a corridor meeting a wall keeps its line, which is exactly where a line
//! belongs.
//!
//! A destroyed tile is not a neighbour. Blow a hole in a hull and the frame
//! comes back around the edge of the hole on its own, because the cells beside
//! it stop finding a match there.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::ai_ship::components::AiShip;
use crate::components::{HullDestroyed, HullSegment, Ship};

/// Which of the four neighbours show the same face, as a bitmask.
///
/// 1 north, 2 south, 4 west, 8 east. North is grid +y, which is the TOP of the
/// image: a Bevy sprite draws row 0 at +y, and the generator erases rows 0..n
/// for bit 1. Get that pair backwards and the seams open on the wrong edges --
/// which looks almost right, and is the reason this is a function with a test
/// rather than four lines inside a loop.
fn neighbour_mask(
    at: impl Fn(IVec2) -> Option<&'static str>,
    cell: IVec2,
    mine: &'static str,
) -> u8 {
    let mut mask = 0u8;
    for (bit, step) in [
        (1u8, IVec2::new(0, 1)),
        (2, IVec2::new(0, -1)),
        (4, IVec2::new(-1, 0)),
        (8, IVec2::new(1, 0)),
    ] {
        if at(cell + step) == Some(mine) {
            mask |= bit;
        }
    }
    mask
}

/// Rewrites a ship's hull tile textures to the variants their neighbours call
/// for, when that ship's layout changes (`building::layout`) -- so holes
/// re-frame themselves on the frame they appear.
///
/// It used to be gated on the world's live tile count and then re-texture
/// every hull tile of every ship, wrecks included, whenever any one changed.
/// Per ship now, and a tile whose texture is already right isn't touched.
pub fn update_hull_seams(
    assets: Res<AssetServer>,
    ships: Query<(Entity, &crate::building::layout::LayoutVersion, &Children), Or<(With<Ship>, With<AiShip>)>>,
    mut hull: Query<(&HullSegment, &mut Sprite), Without<HullDestroyed>>,
    epoch: Res<crate::building::layout::LayoutEpoch>,
    mut seen_epoch: Local<Option<crate::building::layout::LayoutEpoch>>,
    mut built: Local<crate::building::layout::BuiltFrom>,
) {
    if *seen_epoch == Some(*epoch) {
        return;
    }
    *seen_epoch = Some(*epoch);

    for (ship, version, children) in ships.iter() {
        if !built.is_stale(ship, version) {
            continue;
        }
        built.mark(ship, version);

        // Which texture stands at each cell of this ship.
        let mut face: HashMap<IVec2, &'static str> = HashMap::default();
        for child in children.iter() {
            if let Ok((segment, _)) = hull.get(child) {
                face.insert(
                    segment.grid_position,
                    crate::sprite_map::hull_layer_sprite_path(segment.material, segment.hull_layer),
                );
            }
        }

        for child in children.iter() {
            let Ok((segment, mut sprite)) = hull.get_mut(child) else { continue };
            let cell = segment.grid_position;
            let mine = crate::sprite_map::hull_layer_sprite_path(segment.material, segment.hull_layer);
            let mask = neighbour_mask(|c| face.get(&c).copied(), cell, mine);
            let image = assets.load(crate::sprite_map::hull_layer_sprite_variant(
                segment.material,
                segment.hull_layer,
                mask,
            ));
            if sprite.image != image {
                sprite.image = image;
            }
        }
    }
    built.retain(|e| ships.contains(e));
}

#[cfg(test)]
mod tests {
    use super::neighbour_mask;
    use bevy::prelude::IVec2;
    use crate::components::{HullLayer, HullMaterial};

    const PLATE: &str = "sprites/hull/hull_steel.png";
    const DECK: &str = "sprites/hull/hull_hallway.png";

    fn grid<'a>(
        pairs: &'a [(i32, i32, &'static str)],
    ) -> impl Fn(IVec2) -> Option<&'static str> + 'a {
        move |c: IVec2| {
            pairs.iter().find(|(x, y, _)| *x == c.x && *y == c.y).map(|(_, _, s)| *s)
        }
    }

    /// North is +y and it is bit 1. The generator erases the top rows of the
    /// image for that bit, and a Bevy sprite puts image row 0 at +y, so these
    /// two have to agree or every seam opens on the wrong side.
    #[test]
    fn north_is_positive_y() {
        let at = grid(&[(0, 1, PLATE)]);
        assert_eq!(neighbour_mask(&at, IVec2::ZERO, PLATE), 1);
    }

    #[test]
    fn each_side_has_its_own_bit() {
        for (dx, dy, bit) in [(0, 1, 1u8), (0, -1, 2), (-1, 0, 4), (1, 0, 8)] {
            let cells = [(dx, dy, PLATE)];
            let at = grid(&cells);
            assert_eq!(
                neighbour_mask(&at, IVec2::ZERO, PLATE),
                bit,
                "neighbour at ({dx},{dy}) should be bit {bit}"
            );
        }
    }

    /// A tile surrounded by its own kind shows no frame at all.
    #[test]
    fn fully_enclosed_opens_every_side() {
        let at = grid(&[(0, 1, PLATE), (0, -1, PLATE), (-1, 0, PLATE), (1, 0, PLATE)]);
        assert_eq!(neighbour_mask(&at, IVec2::ZERO, PLATE), 15);
    }

    /// A different texture is not a match, so the line between a corridor and
    /// a wall stays drawn. That boundary is worth seeing.
    #[test]
    fn a_different_texture_is_not_a_neighbour() {
        let at = grid(&[(0, 1, DECK), (1, 0, PLATE)]);
        assert_eq!(neighbour_mask(&at, IVec2::ZERO, PLATE), 8);
    }

    /// Nothing beside it means the full frame, which is variant 00.
    #[test]
    fn alone_keeps_its_whole_frame() {
        let at = grid(&[]);
        assert_eq!(neighbour_mask(&at, IVec2::ZERO, PLATE), 0);
    }

    use crate::sprite_map::{hull_layer_sprite_path, hull_layer_sprite_variant};

    /// Variant 00 must name a real file and read as the untouched plate, so a
    /// tile spawned with the plain texture does not flash a frame before the
    /// first correction lands.
    #[test]
    fn variant_zero_is_the_plain_tile() {
        let v = hull_layer_sprite_variant(HullMaterial::Steel, HullLayer::Outer, 0);
        assert_eq!(v, "sprites/hull/hull_steel_00.png");
    }

    /// The variant path is derived from the base path, so a layer that changes
    /// texture cannot end up loading another texture's seams.
    #[test]
    fn variants_follow_the_texture_their_layer_uses() {
        for layer in [HullLayer::Outer, HullLayer::Inner, HullLayer::Hallway] {
            let base = hull_layer_sprite_path(HullMaterial::Titanium, layer)
                .strip_suffix(".png")
                .unwrap()
                .to_string();
            for mask in 0..16u8 {
                let v = hull_layer_sprite_variant(HullMaterial::Titanium, layer, mask);
                assert!(
                    v.starts_with(&base),
                    "{layer:?} mask {mask} loaded {v}, which is not a variant of {base}"
                );
            }
        }
    }

    /// Hallway is one texture for every material, so its variants must be too.
    /// Otherwise a steel deck and a titanium deck beside each other would fail
    /// to merge despite being the same picture.
    #[test]
    fn decking_is_one_texture_whatever_it_is_made_of() {
        let a = hull_layer_sprite_variant(HullMaterial::Steel, HullLayer::Hallway, 9);
        let b = hull_layer_sprite_variant(HullMaterial::AbyssalAlloy, HullLayer::Hallway, 9);
        assert_eq!(a, b);
    }

    /// Every mask a tile can be asked for has a file behind it.
    #[test]
    fn all_sixteen_variants_exist_on_disk() {
        for material in [
            HullMaterial::Steel,
            HullMaterial::Titanium,
            HullMaterial::Composite,
            HullMaterial::AbyssalAlloy,
        ] {
            for layer in [HullLayer::Outer, HullLayer::Hallway] {
                for mask in 0..16u8 {
                    let rel = hull_layer_sprite_variant(material, layer, mask);
                    let path = std::path::Path::new("assets").join(&rel);
                    assert!(
                        path.exists(),
                        "{path:?} is missing -- run tools/art/gen_hull_tiles.py"
                    );
                }
            }
        }
    }
}
