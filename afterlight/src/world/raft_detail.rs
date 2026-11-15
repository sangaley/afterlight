//! Stations up close: the real blocks over the painted pictures.
//!
//! A station's ships, arms and sections are drawn as pictures painted at a
//! few pixels a block (world::raft_art) -- right from afar, soft once the
//! camera comes in, where the player's own ship shows every block's art. So
//! close in, the pieces under the camera get their blocks back: one sprite a
//! block from the same art the picture was painted from, laid over the
//! picture and under its lights, and only for what's on screen (a station is
//! far too many blocks to draw whole). The picture's shadow still falls round
//! each piece, and its lit windows still show over the blocks.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::raft::{design, turn, CELL};
use super::raft_art::RaftHull;
use crate::camera::MainCamera;

/// Camera scale where the blocks start to show over the pictures...
pub const DETAIL_FROM: f32 = 4.5;
/// ...and where they cover them.
pub const DETAIL_SOLID: f32 = 3.0;
/// Pixels a block in the block art.
const ART_PX: f32 = 378.0;

/// How much of the blocks show at a camera scale: 0 far out, 1 close in.
pub fn detail_alpha(scale: f32) -> f32 {
    let t = ((DETAIL_FROM - scale) / (DETAIL_FROM - DETAIL_SOLID)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One block drawn over a piece's picture.
#[derive(Component)]
pub struct DetailBlock;

/// The stretch of the world the blocks are drawn for, and the pieces done.
#[derive(Resource, Default)]
pub struct DetailWindow {
    area: Option<Rect>,
    done: HashSet<Entity>,
}

/// A block's colour over its art: the steel pulled toward the station's
/// gunmetal, the outer plate in the owner's paint, glass and panels their own.
fn block_colour(cell: &super::raft::DesignCell, accent: bool, theme: &super::station_style::Theme) -> Color {
    if let Some(t) = cell.tint {
        return Color::srgb(t[0], t[1], t[2]);
    }
    if let (true, Some(l)) = (cell.outer, theme.livery) {
        let p = if accent { l.accent } else { l.base }.to_srgba();
        return Color::srgb((p.red * 1.3).min(1.0), (p.green * 1.3).min(1.0), (p.blue * 1.3).min(1.0));
    }
    let t = theme.tint;
    let dim = if cell.outer { 0.82 } else { 1.0 };
    Color::srgb(0.72 * t[0] * dim, 0.76 * t[1] * dim, 0.88 * t[2] * dim)
}

/// Lay blocks over the pieces on screen when the camera is close, take them
/// off when it pulls out, and fade them between.
#[allow(clippy::type_complexity)]
pub fn update_station_detail(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut window: ResMut<DetailWindow>,
    camera: Query<(&GlobalTransform, &Projection), With<MainCamera>>,
    screens: Query<&Window, With<PrimaryWindow>>,
    hulls: Query<(Entity, &RaftHull, &Sprite, &GlobalTransform), Without<DetailBlock>>,
    mut blocks: Query<(Entity, &mut Sprite), With<DetailBlock>>,
) {
    let Ok((cam, projection)) = camera.single() else { return };
    let scale = match projection {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    let alpha = detail_alpha(scale);
    if alpha <= 0.0 {
        if window.area.take().is_some() {
            for (e, _) in &blocks {
                commands.entity(e).despawn();
            }
            window.done.clear();
        }
        return;
    }
    for (_, mut sprite) in &mut blocks {
        if (sprite.color.alpha() - alpha).abs() > 0.01 {
            sprite.color.set_alpha(alpha);
        }
    }

    // What's on screen, and a margin round it so a little panning is free.
    let size = screens.single().map_or(Vec2::new(1280.0, 720.0), |w| Vec2::new(w.width(), w.height())) * scale;
    let centre = cam.translation().truncate();
    let view = Rect::from_center_half_size(centre, size * 0.5);
    let fresh = match window.area {
        Some(area) => !(area.contains(view.min) && area.contains(view.max)),
        None => true,
    };
    if fresh {
        for (e, _) in &blocks {
            commands.entity(e).despawn();
        }
        window.done.clear();
        window.area = Some(Rect::from_center_half_size(centre, size * 0.9));
    }
    let Some(area) = window.area else { return };

    for (entity, hull, sprite, global) in &hulls {
        // The lights layer and pictures still being painted have no blocks.
        if hull.lights || sprite.color == Color::NONE || window.done.contains(&entity) {
            continue;
        }
        let Some(d) = design(hull.key.0) else { continue };
        let at = global.translation().truncate();
        let reach = d.span().max_element() as f32 * CELL * 0.75;
        if Rect::from_center_half_size(at, Vec2::splat(reach)).intersect(area).is_empty() {
            continue;
        }
        window.done.insert(entity);
        let theme = super::station_style::theme(super::raft_art::painted_as(hull.key).2);
        let quarter = hull.key.1;
        let (_, rotation, _) = global.to_scale_rotation_translation();
        let mut kids = Vec::new();
        for c in &d.cells {
            let local = turn(c.at.as_vec2() - d.centre(), quarter) * CELL;
            let world = at + (rotation * local.extend(0.0)).truncate();
            if !area.contains(world) {
                continue;
            }
            let accent = theme.livery.is_some_and(|l| l.accent_at(c.at, d.min, d.max));
            let mut colour = block_colour(c, accent, &theme);
            colour.set_alpha(alpha);
            let (pc, pr, pw, ph) = c.part;
            let mut block = if c.tint.is_some() {
                Sprite::from_color(colour, Vec2::splat(CELL))
            } else {
                Sprite {
                    image: assets.load(c.sprite),
                    color: colour,
                    // A part of a bigger sprite: its square of the art.
                    rect: (pw > 1 || ph > 1).then(|| Rect::new(pc as f32 * ART_PX, pr as f32 * ART_PX, (pc as f32 + 1.0) * ART_PX, (pr as f32 + 1.0) * ART_PX)),
                    ..default()
                }
            };
            block.custom_size = Some(Vec2::splat(CELL));
            kids.push(
                commands
                    .spawn((block, Transform::from_translation(local.extend(0.0005)).with_rotation(Quat::from_rotation_z(quarter as f32 * std::f32::consts::FRAC_PI_2)), DetailBlock))
                    .id(),
            );
        }
        if !kids.is_empty() {
            commands.entity(entity).add_children(&kids);
        }
    }
}

/// Off the screens that show stations, no blocks are kept.
pub fn clear_station_detail(mut commands: Commands, mut window: ResMut<DetailWindow>, blocks: Query<Entity, With<DetailBlock>>) {
    for e in &blocks {
        commands.entity(e).despawn();
    }
    window.area = None;
    window.done.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_show_only_close_in() {
        assert_eq!(detail_alpha(6.0), 0.0);
        assert_eq!(detail_alpha(DETAIL_FROM), 0.0);
        assert_eq!(detail_alpha(DETAIL_SOLID), 1.0);
        assert_eq!(detail_alpha(1.0), 1.0);
    }
}
