//! Stations up close, sharp.
//!
//! A station's ships, arms and sections are painted pictures (world::raft_art)
//! at a few pixels a block -- right from afar, where a block is a few pixels
//! on screen, and stretched to mush once the camera comes in. So the pieces on
//! screen while the camera is close are painted again at several times the
//! pixels, on a thread of their own, and swapped in when they're ready; they
//! go back to the plain picture when the camera pulls out or moves on, and the
//! sharp ones nobody has wanted for a while are let go.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::raft_art::{RaftBakes, RaftHull};
use crate::camera::MainCamera;

/// Camera scale below which pieces on screen are painted sharp: the plain
/// pictures are one pixel to a screen pixel at about 11.
pub const FINE_FROM: f32 = 9.0;
/// Seconds a sharp picture is kept after it was last wanted.
const KEEP_S: f32 = 20.0;

/// On a piece showing its sharp picture.
#[derive(Component)]
pub struct ShowingFine;

/// Swap the pieces on screen to sharp pictures close in, and back again.
#[allow(clippy::type_complexity)]
pub fn update_station_detail(
    mut commands: Commands,
    time: Res<Time>,
    bakes: Option<ResMut<RaftBakes>>,
    camera: Query<(&GlobalTransform, &Projection), With<MainCamera>>,
    screens: Query<&Window, With<PrimaryWindow>>,
    mut hulls: Query<(Entity, &RaftHull, &mut Sprite, &GlobalTransform, Has<ShowingFine>)>,
) {
    let Some(mut bakes) = bakes else { return };
    let Ok((cam, projection)) = camera.single() else { return };
    let scale = match projection {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    let now = time.elapsed_secs();
    let close = scale < FINE_FROM;
    let size = screens.single().map_or(Vec2::new(1280.0, 720.0), |w| Vec2::new(w.width(), w.height())) * scale;
    // What's on screen, with a margin so panning doesn't show the swap.
    let view = Rect::from_center_half_size(cam.translation().truncate(), size * 0.75);
    for (entity, hull, mut sprite, global, showing) in &mut hulls {
        if sprite.color == Color::NONE {
            continue;
        }
        let half = sprite.custom_size.unwrap_or(Vec2::ZERO) * 0.5;
        let on_screen = !Rect::from_center_half_size(global.translation().truncate(), half).intersect(view).is_empty();
        if close && on_screen {
            bakes.ask_fine(hull.key, now);
            if !showing {
                if let Some((body, glow, size)) = bakes.fine_get(&hull.key) {
                    sprite.image = if hull.lights { glow.clone() } else { body.clone() };
                    sprite.custom_size = Some(*size);
                    commands.entity(entity).insert(ShowingFine);
                }
            }
        } else if showing {
            if let Some((body, glow, size)) = bakes.done_get(&hull.key) {
                sprite.image = if hull.lights { glow.clone() } else { body.clone() };
                sprite.custom_size = Some(*size);
            }
            commands.entity(entity).remove::<ShowingFine>();
        }
    }
    bakes.release_fine(now, KEEP_S);
}

#[cfg(test)]
mod tests {
    use super::super::raft::design;
    use super::super::raft_art::{bake, bake_at, fine_px};

    /// A sharp picture covers the same ground as the plain one, at more
    /// pixels: swapping one for the other mustn't move or grow the piece.
    #[test]
    fn a_sharp_picture_is_the_same_piece() {
        let key = super::super::sections::arm_segment(40, 6, 1);
        let d = design(key).unwrap();
        let theme = crate::world::station_style::theme(3);
        let plain = bake(d, 1, 7, theme);
        let sharp = bake_at(d, 1, 7, theme, fine_px(d));
        assert!(sharp.body.width() > plain.body.width() * 2, "{} vs {}", sharp.body.width(), plain.body.width());
        let (p, s) = (plain.size, sharp.size);
        // Only the margin round it, for its shadow, may differ a little.
        assert!((p - s).abs().max_element() < super::super::raft::CELL * 2.0, "{p} vs {s}");
    }
}
