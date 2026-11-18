//! Path lights on the walkways, the way an airliner's floor lights show you
//! the way out in the dark.
//!
//! Not on every tile: a lamp post stands at each inside corner of the
//! walkway floor -- where a wall juts into it -- with its light pooled on
//! the deck and two short strips running off along the wall's two faces. That
//! puts one at every turn, two flanking every T, four at a crossing, and none
//! down a straight run or inside an open room, at any corridor width.
//!
//! Walkway is bare `Hallway` decking with no part standing on it. Lights
//! are rebuilt per ship when its layout changes, so a corner shot away goes
//! dark with it.
//!
//! AFTERLIGHT_PATH_LIGHTS=0 turns them off; AFTERLIGHT_PATH_GLOW=<x> sets
//! how strongly they light the deck, for tuning.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use crate::ai_ship::components::AiShip;
use crate::building::layout::{BuiltFrom, LayoutEpoch, LayoutVersion};
use crate::building::{footprints, grid_to_local, ShipGrid, GRID_SIZE};
use crate::components::{DestroyedModule, HullDestroyed, HullLayer, HullSegment, Module, Ship};

use super::module_glow::GlowMaterial;

/// A crisp, faintly cool white: the systems have the colours, and a warm
/// white was lost among the deck's orange studs.
const LAMP: [u8; 3] = [236, 242, 255];
/// Over the deck (0.1), under the modules (0.2+) and the crew (0.6).
const Z: f32 = 0.15;
/// How far into the floor the post stands from the wall's corner.
const POST_IN: f32 = 11.0;
const POST_SIZE: f32 = 13.0;
const POOL_SIZE: f32 = 92.0;
/// The strips: how long they run along the wall, how thick, and how far off
/// the wall's face.
const STRIP_LEN: f32 = 44.0;
const STRIP_THICK: f32 = 6.0;
const STRIP_IN: f32 = 3.5;

fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("AFTERLIGHT_PATH_LIGHTS").ok().as_deref() != Some("0"))
}

fn pool_strength() -> f32 {
    static G: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *G.get_or_init(|| std::env::var("AFTERLIGHT_PATH_GLOW").ok().and_then(|v| v.parse().ok()).unwrap_or(0.5))
}

/// Every inside corner of a walkway: the cell the post stands in and the
/// diagonal it faces, toward the wall that juts in there.
pub fn inside_corners(walkway: &HashSet<IVec2>) -> Vec<(IVec2, IVec2)> {
    let mut out = Vec::new();
    for &c in walkway {
        for d in [IVec2::new(1, 1), IVec2::new(1, -1), IVec2::new(-1, 1), IVec2::new(-1, -1)] {
            if walkway.contains(&(c + IVec2::new(d.x, 0)))
                && walkway.contains(&(c + IVec2::new(0, d.y)))
                && !walkway.contains(&(c + d))
            {
                out.push((c, d));
            }
        }
    }
    out.sort_by_key(|(c, d)| (c.x, c.y, d.x, d.y));
    out
}

/// One of a ship's path lights.
#[derive(Component)]
pub struct PathLight;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn lay_path_lights(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GlowMaterial>>,
    ships: Query<(Entity, &LayoutVersion, &Children), Or<(With<Ship>, With<AiShip>)>>,
    hull: Query<&HullSegment, Without<HullDestroyed>>,
    modules: Query<&Module, Without<DestroyedModule>>,
    epoch: Res<LayoutEpoch>,
    mut seen_epoch: Local<Option<LayoutEpoch>>,
    mut built: Local<BuiltFrom>,
    mut lit: Local<HashMap<Entity, Vec<Entity>>>,
    mut pool: Local<Option<(Handle<Mesh>, Handle<GlowMaterial>)>>,
) {
    if !enabled() || *seen_epoch == Some(*epoch) {
        return;
    }
    *seen_epoch = Some(*epoch);
    let (quad, glow) = pool
        .get_or_insert_with(|| {
            let c = Color::srgb_u8(LAMP[0], LAMP[1], LAMP[2]).to_linear() * pool_strength();
            (
                meshes.add(Rectangle::from_size(Vec2::splat(POOL_SIZE))),
                materials.add(GlowMaterial { colour: c, breath: Vec4::ZERO, mask: assets.load("sprites/path_light/pool.png") }),
            )
        })
        .clone();
    let lamp = Color::srgb_u8(LAMP[0], LAMP[1], LAMP[2]);

    for (ship, version, children) in &ships {
        if !built.is_stale(ship, version) {
            continue;
        }
        built.mark(ship, version);
        for old in lit.remove(&ship).unwrap_or_default() {
            commands.entity(old).try_despawn();
        }

        let mut floor: HashSet<IVec2> = HashSet::default();
        let mut parts: HashSet<IVec2> = HashSet::default();
        for child in children.iter() {
            if let Ok(h) = hull.get(child) {
                if h.hull_layer == HullLayer::Hallway {
                    floor.insert(h.grid_position);
                }
            } else if let Ok(m) = modules.get(child) {
                parts.extend(ShipGrid::cells_for(m.grid_position, m.size, m.rotation, footprints::footprint_override(m.module_type)));
            }
        }
        let walkway: HashSet<IVec2> = floor.difference(&parts).copied().collect();

        let mut spawned = Vec::new();
        for (cell, d) in inside_corners(&walkway) {
            let dv = d.as_vec2();
            let corner = grid_to_local(cell) + dv * (GRID_SIZE * 0.5);
            let at = corner - dv * POST_IN;
            spawned.push(commands.spawn((Mesh2d(quad.clone()), MeshMaterial2d(glow.clone()), Transform::from_translation(at.extend(Z)), PathLight, ChildOf(ship))).id());
            spawned.push(
                commands
                    .spawn((
                        Sprite { image: assets.load("sprites/path_light/post.png"), color: lamp, custom_size: Some(Vec2::splat(POST_SIZE)), ..default() },
                        Transform::from_translation(at.extend(Z + 0.002)),
                        PathLight,
                        ChildOf(ship),
                    ))
                    .id(),
            );
            // Along the wall's two faces, away from the corner, each just
            // off its face on the walkway side.
            for (run, off) in [(Vec2::new(0.0, dv.y), Vec2::new(-dv.x, 0.0)), (Vec2::new(dv.x, 0.0), Vec2::new(0.0, -dv.y))] {
                let mid = corner + off * STRIP_IN + run * (STRIP_LEN * 0.5);
                spawned.push(
                    commands
                        .spawn((
                            Sprite {
                                image: assets.load("sprites/path_light/strip.png"),
                                color: lamp,
                                custom_size: Some(Vec2::new(STRIP_LEN, STRIP_THICK)),
                                ..default()
                            },
                            Transform::from_translation(mid.extend(Z + 0.001)).with_rotation(Quat::from_rotation_z(run.y.atan2(run.x))),
                            PathLight,
                            ChildOf(ship),
                        ))
                        .id(),
                );
            }
        }
        if !spawned.is_empty() {
            lit.insert(ship, spawned);
        }
    }
    built.retain(|e| ships.contains(e));
    lit.retain(|e, _| ships.contains(*e));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(list: &[(i32, i32)]) -> HashSet<IVec2> {
        list.iter().map(|&(x, y)| IVec2::new(x, y)).collect()
    }

    #[test]
    fn a_straight_run_has_no_lights() {
        let w = cells(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert!(inside_corners(&w).is_empty());
        // Two wide, too.
        let w = cells(&[(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)]);
        assert!(inside_corners(&w).is_empty());
    }

    #[test]
    fn a_turn_has_one() {
        // ##
        // #.
        let w = cells(&[(0, 0), (0, 1), (1, 1), (0, -1), (2, 1)]);
        assert_eq!(inside_corners(&w), vec![(IVec2::new(0, 1), IVec2::new(1, -1))]);
    }

    #[test]
    fn a_tee_has_two_and_a_crossing_four() {
        let tee = cells(&[(-1, 0), (0, 0), (1, 0), (0, 1)]);
        assert_eq!(inside_corners(&tee).len(), 2);
        let cross = cells(&[(-1, 0), (0, 0), (1, 0), (0, 1), (0, -1)]);
        assert_eq!(inside_corners(&cross).len(), 4);
    }

    #[test]
    fn an_open_room_has_none() {
        let mut w = HashSet::default();
        for x in 0..4 {
            for y in 0..3 {
                w.insert(IVec2::new(x, y));
            }
        }
        assert!(inside_corners(&w).is_empty());
    }
}
