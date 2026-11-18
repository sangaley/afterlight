//! Deck variation: corridors, open deck and rooms told apart.
//!
//! Lifted to 2.4 the walkway tread, the structural tiles and the modules'
//! own plates all sat at one mid grey, so a ship read as one floor. Three
//! layers, each switchable (AFTERLIGHT_DECK=shade,runner,tint picks which):
//!
//! - `shade`: a soft shadow along every walkway edge that meets a wall or a
//!   part, so the walkways read as lanes sunk between raised rooms.
//! - `runner`: a darker centre lane down the corridors -- walkways up to
//!   three cells wide -- so a route reads as a route; wider halls stay open
//!   deck.
//! - `tint`: each module's plate leans a little toward its system's light
//!   (interior_light.rs does it, with `room_tint` here), so the reactor room
//!   is warm, the engine room cool, the medbay rose.
//!
//! Rebuilt per ship when its layout changes, like the path lights.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use crate::ai_ship::components::AiShip;
use crate::building::layout::{BuiltFrom, LayoutEpoch, LayoutVersion};
use crate::building::{grid_to_local, GRID_SIZE};
use crate::components::{DestroyedModule, HullDestroyed, HullSegment, Module, ModuleType, Ship};

use super::path_lights::walkway_cells;

/// Over the deck (0.1), under the path lights (0.15).
const RUNNER_Z: f32 = 0.11;
const SHADE_Z: f32 = 0.12;
const SHADE_DEPTH: f32 = 20.0;
/// How dark the shade is at the wall's foot.
const SHADE_ALPHA: f32 = 0.8;

fn layers() -> &'static str {
    static L: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    L.get_or_init(|| std::env::var("AFTERLIGHT_DECK").unwrap_or_else(|_| "shade,runner,tint".into()))
}

fn on(layer: &str) -> bool {
    layers().split(',').any(|l| l.trim() == layer)
}

/// The colour a module's plate is multiplied by: a lean toward its system's
/// light, or None for parts with no light of their own.
pub fn room_tint(t: ModuleType) -> Option<Color> {
    static K: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    if !on("tint") {
        return None;
    }
    let k = *K.get_or_init(|| std::env::var("AFTERLIGHT_ROOM_TINT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.32));
    let (rgb, _) = super::module_glow::glow_colour(t)?;
    let top = rgb.iter().copied().max().unwrap_or(1).max(1) as f32;
    let lean = |c: u8| 1.0 - k + k * (c as f32 / top);
    Some(Color::LinearRgba(LinearRgba::rgb(lean(rgb[0]), lean(rgb[1]), lean(rgb[2]))))
}

/// The corridors' centre lanes: for each, the cell it is drawn from, whether
/// the corridor runs along x, and how far across (in cells) the lane's
/// centre sits from that cell's -- 0 for an odd width, 0.5 for an even one.
/// One per cross-section, so a two-wide corridor gets one lane down its
/// middle, not two. A stub shorter than three cells gets none: lanes on
/// every little spur read as stray dashes.
pub fn lanes(walkway: &HashSet<IVec2>) -> Vec<(IVec2, bool, f32)> {
    let run = |c: IVec2, step: IVec2| {
        let mut n = 0;
        while walkway.contains(&(c + step * (n + 1))) {
            n += 1;
        }
        n
    };
    let mut out = Vec::new();
    for &c in walkway {
        let (left, right) = (run(c, IVec2::NEG_X), run(c, IVec2::X));
        let (down, up) = (run(c, IVec2::NEG_Y), run(c, IVec2::Y));
        let (wide_x, wide_y) = (left + right + 1, down + up + 1);
        // Along x: the cross-section is the column through it.
        let (along_x, below, width) = if wide_y <= 3 && wide_x > wide_y && wide_x >= 3 {
            (true, down, wide_y)
        } else if wide_x <= 3 && wide_y > wide_x && wide_y >= 3 {
            (false, left, wide_x)
        } else {
            continue;
        };
        if below == (width - 1) / 2 {
            out.push((c, along_x, (width - 1) as f32 / 2.0 - below as f32));
        }
    }
    out.sort_by_key(|(c, x, _)| (c.x, c.y, *x));
    out
}

/// One of a ship's deck overlays.
#[derive(Component)]
pub struct DeckMark;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn lay_deck(
    mut commands: Commands,
    assets: Res<AssetServer>,
    ships: Query<(Entity, &LayoutVersion, &Children), Or<(With<Ship>, With<AiShip>)>>,
    hull: Query<&HullSegment, Without<HullDestroyed>>,
    modules: Query<&Module, Without<DestroyedModule>>,
    epoch: Res<LayoutEpoch>,
    mut seen_epoch: Local<Option<LayoutEpoch>>,
    mut built: Local<BuiltFrom>,
    mut laid: Local<HashMap<Entity, Vec<Entity>>>,
) {
    let (shade, runner) = (on("shade"), on("runner"));
    if !(shade || runner) || *seen_epoch == Some(*epoch) {
        return;
    }
    *seen_epoch = Some(*epoch);
    for (ship, version, children) in &ships {
        if !built.is_stale(ship, version) {
            continue;
        }
        built.mark(ship, version);
        for old in laid.remove(&ship).unwrap_or_default() {
            commands.entity(old).try_despawn();
        }
        let walkway = walkway_cells(children, &hull, &modules);
        let mut spawned = Vec::new();
        if runner {
            for (cell, along_x, across) in lanes(&walkway) {
                let off = if along_x { Vec2::new(0.0, across) } else { Vec2::new(across, 0.0) } * GRID_SIZE;
                let at = grid_to_local(cell) + off;
                let turn = if along_x { 0.0 } else { std::f32::consts::FRAC_PI_2 };
                spawned.push(
                    commands
                        .spawn((
                            Sprite {
                                image: assets.load("sprites/deck/runner.png"),
                                color: Color::srgba(0.78, 0.84, 0.95, 0.4),
                                // A hair long, so cell meets cell without a seam.
                                custom_size: Some(Vec2::new(GRID_SIZE + 0.5, GRID_SIZE)),
                                ..default()
                            },
                            Transform::from_translation(at.extend(RUNNER_Z)).with_rotation(Quat::from_rotation_z(turn)),
                            DeckMark,
                            ChildOf(ship),
                        ))
                        .id(),
                );
            }
        }
        if shade {
            for &cell in &walkway {
                for d in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                    if walkway.contains(&(cell + d)) {
                        continue;
                    }
                    let dv = d.as_vec2();
                    let at = grid_to_local(cell) + dv * (GRID_SIZE - SHADE_DEPTH) * 0.5;
                    // The texture's dark edge is its top (+y); turn it to face the wall.
                    let turn = dv.y.atan2(dv.x) - std::f32::consts::FRAC_PI_2;
                    spawned.push(
                        commands
                            .spawn((
                                Sprite {
                                    image: assets.load("sprites/deck/shade.png"),
                                    color: Color::srgba(1.0, 1.0, 1.0, SHADE_ALPHA),
                                    custom_size: Some(Vec2::new(GRID_SIZE, SHADE_DEPTH)),
                                    ..default()
                                },
                                Transform::from_translation(at.extend(SHADE_Z)).with_rotation(Quat::from_rotation_z(turn)),
                                DeckMark,
                                ChildOf(ship),
                            ))
                            .id(),
                    );
                }
            }
        }
        if !spawned.is_empty() {
            laid.insert(ship, spawned);
        }
    }
    built.retain(|e| ships.contains(e));
    laid.retain(|e, _| ships.contains(*e));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: i32, y0: i32, w: i32, h: i32) -> HashSet<IVec2> {
        let mut s = HashSet::default();
        for x in x0..x0 + w {
            for y in y0..y0 + h {
                s.insert(IVec2::new(x, y));
            }
        }
        s
    }

    #[test]
    fn one_wide_corridor_has_a_lane_down_every_cell() {
        let l = lanes(&rect(0, 0, 5, 1));
        assert_eq!(l.len(), 5);
        assert!(l.iter().all(|(_, along_x, across)| *along_x && *across == 0.0));
    }

    #[test]
    fn two_wide_has_one_lane_between_its_rows() {
        let l = lanes(&rect(0, 0, 5, 2));
        assert_eq!(l.len(), 5);
        assert!(l.iter().all(|(c, _, across)| c.y == 0 && *across == 0.5));
    }

    #[test]
    fn three_wide_runs_down_the_middle_and_vertical_turns() {
        let l = lanes(&rect(0, 0, 6, 3));
        assert!(l.iter().all(|(c, along_x, across)| c.y == 1 && *along_x && *across == 0.0));
        let v = lanes(&rect(0, 0, 1, 4));
        assert!(v.iter().all(|(_, along_x, _)| !*along_x));
    }

    #[test]
    fn a_stub_gets_no_lane() {
        assert!(lanes(&rect(0, 0, 2, 1)).is_empty());
    }

    #[test]
    fn a_wide_hall_is_open_deck() {
        assert!(lanes(&rect(0, 0, 5, 5)).is_empty());
    }
}
