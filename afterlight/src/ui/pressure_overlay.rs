//! Where the air is, and where it is going.
//!
//! A flow field cannot be tuned by looking at it sideways, so this is as much
//! a development instrument as a player readout: tiles are tinted by pressure
//! and each one draws a short line the way its air is moving.
//!
//! Built alongside `damage_overlay`, and deliberately exclusive with it --
//! both tint the same blocks.

use bevy::prelude::*;

use crate::building::{grid_to_local, rooms::RoomMap, GRID_SIZE};
use crate::components::*;
use crate::events::*;
use crate::ship::air::AirField;

/// One tinted cell of the overlay, parented to the ship.
#[derive(Component)]
pub struct PressureTile(pub IVec2);

/// Marker for the legend UI node.
#[derive(Component)]
pub(crate) struct PressureOverlayLegend;

/// Pale blue where there is air, near-black where there is none.
fn pressure_color(pressure: f32) -> Color {
    let p = pressure.clamp(0.0, 1.0);
    Color::srgba(
        0.06 + 0.39 * p,
        0.07 + 0.65 * p,
        0.11 + 0.84 * p,
        0.42 - 0.12 * p,
    )
}

/// Q toggles the pressure overlay, turning the damage overlay off if it is up.
pub fn toggle_pressure_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    ship_query: Query<(Entity, Option<&PressureOverlayVisible>), With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !keyboard.just_pressed(KeyCode::KeyQ) {
        return;
    }
    let Ok((entity, overlay)) = ship_query.single() else { return };

    if overlay.is_some() {
        commands.entity(entity).remove::<PressureOverlayVisible>();
        notifications.write(ShowNotification {
            message: "Pressure overlay OFF".into(),
            notification_type: NotificationType::Info,
            duration: 1.0,
        });
    } else {
        commands
            .entity(entity)
            .insert(PressureOverlayVisible)
            .remove::<DamageOverlayVisible>();
        notifications.write(ShowNotification {
            message: "Pressure overlay ON - blue=pressurized, black=vacuum, lines show airflow".into(),
            notification_type: NotificationType::Info,
            duration: 2.5,
        });
    }
}

/// Keeps one tinted sprite per interior tile while the overlay is up.
pub fn update_pressure_overlay(
    mut commands: Commands,
    ship_query: Query<(Entity, Has<PressureOverlayVisible>), With<Ship>>,
    room_map: Res<RoomMap>,
    air: Res<AirField>,
    mut tiles: Query<(Entity, &PressureTile, &mut Sprite)>,
) {
    let Ok((ship, visible)) = ship_query.single() else { return };

    if !visible {
        for (entity, _, _) in tiles.iter() {
            commands.entity(entity).try_despawn();
        }
        return;
    }

    let mut seen: std::collections::HashSet<IVec2> = std::collections::HashSet::new();
    for (entity, tile, mut sprite) in tiles.iter_mut() {
        // A tile that was shot away takes its swatch with it.
        if !room_map.tile_to_room.contains_key(&tile.0) {
            commands.entity(entity).try_despawn();
            continue;
        }
        seen.insert(tile.0);
        sprite.color = pressure_color(air.pressure.get(&tile.0).copied().unwrap_or(1.0));
    }

    for &cell in room_map.tile_to_room.keys() {
        if seen.contains(&cell) {
            continue;
        }
        let pressure = air.pressure.get(&cell).copied().unwrap_or(1.0);
        let swatch = commands
            .spawn((
                Sprite {
                    color: pressure_color(pressure),
                    custom_size: Some(Vec2::splat(GRID_SIZE - 6.0)),
                    ..default()
                },
                // Under the crew (0.6) so people stay readable on top of it,
                // over the blocks it describes.
                Transform::from_translation(grid_to_local(cell).extend(0.45)),
                PressureTile(cell),
            ))
            .id();
        commands.entity(swatch).insert(ChildOf(ship));
    }
}

/// How many trails ride the field at once. Deliberately few: a screenful of
/// lines is a texture, and a texture does not tell you which way anything is
/// going.
const TRAILS: usize = 55;

/// Points kept per trail. Long trails are what make a route legible -- you can
/// follow one from the compartment it is draining to the hole it leaves by.
const TRAIL_POINTS: usize = 30;

/// World units per second at flow 1.0. The trails move at the air's pace, so
/// the picture reads as moving rather than drawn.
const DRIFT_SPEED: f32 = 150.0;

/// Seconds before a trail is retired and reseeded somewhere else, so the field
/// keeps being resampled instead of settling into fixed grooves.
const TRAIL_LIFE: f32 = 5.0;

/// Below this the air is not really moving and the trail is noise.
const FLOW_FLOOR: f32 = 0.02;

/// Air speed that draws at full brightness.
const FULL_BRIGHT: f32 = 1.0;

/// One drifting thread of air, and where it has just been.
struct FlowTrail {
    head: Vec2,
    points: Vec<Vec2>,
    age: f32,
}

/// The trails currently riding the field.
#[derive(Resource, Default)]
pub struct FlowTrails {
    trails: Vec<FlowTrail>,
}

/// Flow at an arbitrary ship-local point, blended from the four tile centres
/// around it.
///
/// Sampling the containing tile alone gives every trail inside one cell an
/// identical heading, so they run as straight parallel darts and snap
/// direction at the tile border. Blending is what lets a line bend, and what
/// lets two trails converge smoothly instead of crossing.
fn sample_flow(air: &AirField, local: Vec2) -> Vec2 {
    let gx = local.x / GRID_SIZE;
    let gy = (local.y + 33.0) / GRID_SIZE;
    let (x0, y0) = (gx.floor(), gy.floor());
    let (fx, fy) = (gx - x0, gy - y0);

    let at = |dx: i32, dy: i32| -> Vec2 {
        let cell = IVec2::new(x0 as i32 + dx, y0 as i32 + dy);
        air.flow.get(&cell).copied().unwrap_or(Vec2::ZERO)
    };

    let bottom = at(0, 0) * (1.0 - fx) + at(1, 0) * fx;
    let top = at(0, 1) * (1.0 - fx) + at(1, 1) * fx;
    bottom * (1.0 - fy) + top * fy
}

/// Pressure at a ship-local point, for how heavy to draw the line there.
fn sample_pressure(air: &AirField, local: Vec2) -> f32 {
    let cell = crate::building::local_to_grid(local);
    air.pressure.get(&cell).copied().unwrap_or(0.0)
}

/// Drifts the trails along the field and draws them.
///
/// Advection rather than a drawing: each trail is carried by the air at the
/// air's own speed, and the line you see is simply where it has been. Trails
/// that fall into the same current converge and run together on their own --
/// no merging logic, just two threads in one stream -- and a vent shows up as
/// the place they all end.
///
/// Line weight is the pressure they are passing through, so a full compartment
/// emptying draws heavy and the last of the air draws thin. Gizmos have no
/// per-line width, so weight is drawn as parallel strands.
pub fn draw_pressure_flow(
    mut gizmos: Gizmos,
    time: Res<Time>,
    ship_query: Query<(&GlobalTransform, Has<PressureOverlayVisible>), With<Ship>>,
    room_map: Res<RoomMap>,
    air: Res<AirField>,
    mut trails: ResMut<FlowTrails>,
) {
    let Ok((ship_gt, visible)) = ship_query.single() else { return };
    if !visible {
        trails.trails.clear();
        return;
    }

    let interior: Vec<IVec2> = room_map.tile_to_room.keys().copied().collect();
    if interior.is_empty() {
        trails.trails.clear();
        return;
    }

    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    // A fresh trail starts somewhere random inside the ship, aged at random so
    // they do not all expire on the same frame and blink together.
    let mut seed = |rng: &mut rand::rngs::ThreadRng| {
        use rand::Rng;
        let cell = interior[rng.gen_range(0..interior.len())];
        let jitter = Vec2::new(
            rng.gen_range(-0.5..0.5) * GRID_SIZE,
            rng.gen_range(-0.5..0.5) * GRID_SIZE,
        );
        FlowTrail {
            head: grid_to_local(cell) + jitter,
            points: Vec::with_capacity(TRAIL_POINTS),
            age: rng.gen_range(0.0..TRAIL_LIFE),
        }
    };

    while trails.trails.len() < TRAILS {
        let t = seed(&mut rng);
        trails.trails.push(t);
    }

    let to_world = |p: Vec2| ship_gt.transform_point(p.extend(0.5)).truncate();

    for trail in trails.trails.iter_mut() {
        trail.age += dt;

        let flow = sample_flow(&air, trail.head);
        let speed = flow.length();

        // Retired when it runs out of time, drifts out of the ship, or ends up
        // somewhere the air is still.
        let stalled = speed < FLOW_FLOOR;
        let outside = !room_map
            .tile_to_room
            .contains_key(&crate::building::local_to_grid(trail.head));
        if trail.age > TRAIL_LIFE || outside || (stalled && trail.points.is_empty()) {
            *trail = seed(&mut rng);
            continue;
        }

        if !stalled {
            trail.head += (flow / speed) * (speed * DRIFT_SPEED).min(GRID_SIZE * 4.0) * dt;
            trail.points.push(trail.head);
            if trail.points.len() > TRAIL_POINTS {
                trail.points.remove(0);
            }
        }

        // Draw what it has covered. Brightness rises toward the head, so the
        // leading end is the direction of travel without needing an arrow.
        for i in 1..trail.points.len() {
            let a = trail.points[i - 1];
            let b = trail.points[i];
            let along = i as f32 / trail.points.len() as f32;

            let here = sample_flow(&air, b).length();
            let alpha = (here / FULL_BRIGHT).clamp(0.06, 1.0) * along.powf(1.6) * 0.95;
            let colour = Color::srgba(1.0, 1.0, 1.0, alpha);

            gizmos.line_2d(to_world(a), to_world(b), colour);

            // Weight = how much air is there. Parallel strands, because gizmo
            // lines have no width of their own.
            let strands = (sample_pressure(&air, b) * 3.0).round() as i32;
            if strands > 0 {
                if let Some(dir) = (b - a).try_normalize() {
                    let side = Vec2::new(-dir.y, dir.x);
                    for n in 1..=strands {
                        let off = side * (n as f32 * 1.15);
                        let faded = Color::srgba(1.0, 1.0, 1.0, alpha * 0.55);
                        gizmos.line_2d(to_world(a + off), to_world(b + off), faded);
                        gizmos.line_2d(to_world(a - off), to_world(b - off), faded);
                    }
                }
            }
        }
    }
}

/// Drops the overlay's sprites when the run ends.
pub fn cleanup_pressure_overlay_on_exit(
    mut commands: Commands,
    tiles: Query<Entity, With<PressureTile>>,
    legend: Query<Entity, With<PressureOverlayLegend>>,
) {
    for entity in tiles.iter().chain(legend.iter()) {
        commands.entity(entity).try_despawn();
    }
}

#[cfg(test)]
mod overlay_tests {
    use super::*;

    fn field(cells: &[(IVec2, Vec2)]) -> AirField {
        let mut air = AirField::default();
        for &(cell, flow) in cells {
            air.flow.insert(cell, flow);
        }
        air
    }

    /// `sample_flow` rolls its own grid mapping -- `floor` plus a fraction,
    /// rather than `local_to_grid`'s `round` -- because bilinear blending needs
    /// the fraction, not the nearest cell. The two must still agree at a cell
    /// centre, and they only do while both carry the same 33-unit Y offset.
    /// Drop it from one of them and the whole overlay draws half a tile out.
    #[test]
    fn flow_at_a_cell_centre_is_that_cells_own_flow() {
        for cell in [IVec2::ZERO, IVec2::new(3, 2), IVec2::new(-4, -2), IVec2::new(0, -7)] {
            let air = field(&[(cell, Vec2::new(0.8, -0.3))]);
            let sampled = sample_flow(&air, grid_to_local(cell));
            assert!(
                sampled.distance(Vec2::new(0.8, -0.3)) < 1e-4,
                "cell {cell} sampled as {sampled}, not its own flow -- grid mapping is skewed"
            );
        }
    }

    /// Halfway between two centres reads as the blend of both. This is what
    /// lets a trail bend instead of snapping direction at each tile border.
    #[test]
    fn flow_between_two_cells_is_the_blend_of_them() {
        let air = field(&[(IVec2::ZERO, Vec2::X), (IVec2::new(1, 0), Vec2::NEG_X)]);
        let midpoint = grid_to_local(IVec2::ZERO) + Vec2::X * (GRID_SIZE / 2.0);

        let sampled = sample_flow(&air, midpoint);
        assert!(
            sampled.length() < 1e-4,
            "two opposed cells blended to {sampled}, expected them to cancel"
        );

        let quarter = grid_to_local(IVec2::ZERO) + Vec2::X * (GRID_SIZE / 4.0);
        assert!(
            sample_flow(&air, quarter).x > 0.4,
            "a quarter of the way across should still read mostly like the cell it is in"
        );
    }

    /// Nothing outside the field. A trail that drifts off the ship has to come
    /// to rest rather than inherit whatever the last cell was doing.
    #[test]
    fn flow_outside_the_field_is_still() {
        let air = field(&[(IVec2::ZERO, Vec2::X * 0.9)]);
        let far = grid_to_local(IVec2::new(40, 40));
        assert_eq!(sample_flow(&air, far), Vec2::ZERO, "air moving where there is no ship");
    }

    /// Pressure reads the containing cell, and an undescribed cell reads as
    /// VACUUM -- opposite to `AirField::mean`, which reads an unknown tile as
    /// full so a ship with no detected rooms never looks like it is
    /// suffocating. Here the default has to be empty: off the ship there is no
    /// air, and drawing it heavy would outline the hull in bright lines.
    #[test]
    fn pressure_samples_the_containing_cell_and_defaults_to_vacuum() {
        let mut air = AirField::default();
        air.pressure.insert(IVec2::new(2, -1), 0.6);

        let centre = grid_to_local(IVec2::new(2, -1));
        assert_eq!(sample_pressure(&air, centre), 0.6);
        assert_eq!(
            sample_pressure(&air, centre + Vec2::splat(GRID_SIZE * 0.3)),
            0.6,
            "a point well inside the cell sampled a different cell"
        );
        assert_eq!(
            sample_pressure(&air, grid_to_local(IVec2::new(9, 9))),
            0.0,
            "an undescribed cell read as holding air"
        );
    }

    /// Vacuum draws darker AND more opaque than air: the tint is how the
    /// overlay says "there is nothing here", so the empty end of the scale has
    /// to be the one you notice. The alpha term runs backwards on purpose.
    #[test]
    fn vacuum_draws_darker_and_heavier_than_full_air() {
        let empty = pressure_color(0.0).to_srgba();
        let full = pressure_color(1.0).to_srgba();

        assert!(empty.blue < full.blue, "vacuum is not darker than air");
        assert!(empty.alpha > full.alpha, "vacuum is not more opaque than air");
        assert_eq!(
            pressure_color(-5.0).to_srgba(),
            empty,
            "pressure below zero was not clamped"
        );
        assert_eq!(
            pressure_color(9.0).to_srgba(),
            full,
            "pressure above full was not clamped"
        );
    }
}
