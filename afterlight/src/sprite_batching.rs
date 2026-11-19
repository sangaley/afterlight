//! Fewer draw calls for the same picture.
//!
//! Bevy draws 2D sprites back to front by z, and a run of neighbours that use
//! the same image goes to the GPU as one draw. Sprites at exactly the same z
//! keep the order they were found in, so a ship's blocks -- dozens of images,
//! all on a few layers -- alternate image every few sprites and nearly every
//! one becomes its own draw. Beside two battle groups that was ~27k sprites in
//! ~670 draws, and switching images between them was a fifth of the frame.
//!
//! Here the extracted sprites are put in order of (z, image) before they are
//! queued, so each layer draws image by image. Nothing changes depth, so
//! anything above or below anything else stays there; only sprites at exactly
//! the same z can change places, and their order was never chosen -- it was
//! whatever order the ECS found them in.

use bevy::prelude::*;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::sprite_render::{ExtractedSprite, ExtractedSprites};

pub struct SpriteBatchingPlugin;

/// AFTERLIGHT_NO_SPRITE_GROUPING=1: draw in the order found, as before, to
/// compare the two.
pub fn grouping_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("AFTERLIGHT_NO_SPRITE_GROUPING").is_err())
}

impl Plugin for SpriteBatchingPlugin {
    fn build(&self, app: &mut App) {
        if !grouping_on() {
            return;
        }
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(Render, group_sprites_by_image.before(RenderSystems::Queue));
        }
    }
}

/// A z that sorts as an integer: -0.0 and 0.0 alike, negatives below.
fn z_key(z: f32) -> u32 {
    let bits = (z + 0.0).to_bits();
    if bits & 0x8000_0000 != 0 { !bits } else { bits | 0x8000_0000 }
}

fn group_sprites_by_image(
    mut extracted: ResMut<ExtractedSprites>,
    mut order: Local<Vec<(u32, AssetId<Image>, u32)>>,
    mut slots: Local<Vec<Option<ExtractedSprite>>>,
) {
    let sprites = &mut extracted.sprites;
    if sprites.len() < 2 {
        return;
    }
    order.clear();
    order.extend(sprites.iter().enumerate().map(|(i, s)| (z_key(s.transform.translation().z), s.image_handle_id, i as u32)));
    // The index last: sprites with the same z and image keep their order.
    order.sort_unstable();
    if order.windows(2).all(|w| w[0].2 < w[1].2) {
        return; // already in that order
    }
    // Moved out and back in their new order (they can't be copied).
    slots.clear();
    slots.extend(sprites.drain(..).map(Some));
    sprites.extend(order.iter().map(|&(_, _, i)| slots[i as usize].take().expect("each sprite moves once")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z_keys_sort_like_the_numbers() {
        let zs = [-500.0, -1.5, -0.001, -0.0, 0.0, 0.001, 0.253, 0.255, 1.23, 2.15, 900.0];
        for w in zs.windows(2) {
            assert!(z_key(w[0]) <= z_key(w[1]), "{} vs {}", w[0], w[1]);
            if w[0] != w[1] {
                assert!(z_key(w[0]) < z_key(w[1]), "{} vs {}", w[0], w[1]);
            }
        }
        assert_eq!(z_key(-0.0), z_key(0.0));
    }
}
