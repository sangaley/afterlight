use bevy::prelude::*;

use crate::ai_ship::components::{AiShip, AiShipState, BountyTarget, WorldSimulation};
use crate::components::Ship;
use super::{ContractObjective, ContractState};

// ============================================================================
// BOUNTY NAVIGATION — the minimap/radar dot tells you where a bounty is;
// these two pieces answer the two things players actually asked for: an
// always-visible pointer toward it without opening the map, and a way to
// tell which exact ship it is once you're standing next to a cluster of
// look-alikes.
// ============================================================================

/// HUD arrow that floats near the ship and points toward the nearest active
/// bounty target. Same visual language as home_base::BaseArrow (which points
/// home) but red, so the two are never confused for each other.
#[derive(Component)]
pub struct BountyArrow;

/// Floating label that tracks a specific tagged ship once it's a real
/// spawned entity — not parented to it, so it stays upright regardless of
/// how the ship is rotated.
#[derive(Component)]
pub struct BountyMarker {
    pub target: Entity,
}

/// Finds the current world position of every active, not-yet-destroyed
/// bounty target: the real spawned entity if it's in render range, else its
/// last known off-screen simulated position.
fn active_bounty_positions(
    contract_state: &ContractState,
    sim: &WorldSimulation,
    bounty_ship_query: &Query<(&Transform, &BountyTarget), With<AiShip>>,
) -> Vec<Vec2> {
    active_bounty_positions_with_id(contract_state, sim, bounty_ship_query)
        .into_iter()
        .map(|(pos, _)| pos)
        .collect()
}

/// Same as `active_bounty_positions` but keeps the bounty_id alongside each
/// position — used by the full map to visually distinguish "your" bounty
/// target(s) from ordinary hostile traffic.
pub fn active_bounty_positions_with_id(
    contract_state: &ContractState,
    sim: &WorldSimulation,
    bounty_ship_query: &Query<(&Transform, &BountyTarget), With<AiShip>>,
) -> Vec<(Vec2, u32)> {
    contract_state.active_contracts.iter()
        .filter_map(|c| match &c.objective {
            ContractObjective::DestroyShip { target_id, destroyed: false, .. } => {
                let pos = bounty_ship_query.iter()
                    .find(|(_, b)| b.0 == *target_id)
                    .map(|(t, _)| t.translation.truncate())
                    .or_else(|| sim.bounty_position(*target_id));
                pos.map(|p| (p, *target_id))
            }
            _ => None,
        })
        .collect()
}

pub fn spawn_bounty_arrow(
    mut commands: Commands,
    existing: Query<(), With<BountyArrow>>,
) {
    if !existing.is_empty() {
        return;
    }

    let arrow_root = commands.spawn((
        Transform::from_xyz(0.0, 0.0, 5.0),
        Visibility::Hidden,
        BountyArrow,
        crate::camera::ZoomInvariantText,
    )).id();
    let shaft = commands.spawn((
        Sprite { color: Color::srgba(1.0, 0.25, 0.25, 0.85), custom_size: Some(Vec2::new(34.0, 6.0)), ..default() },
        Transform::from_xyz(-8.0, 0.0, 0.0),
    )).id();
    let head = commands.spawn((
        Sprite { color: Color::srgba(1.0, 0.3, 0.3, 0.95), custom_size: Some(Vec2::new(14.0, 14.0)), ..default() },
        Transform {
            translation: Vec3::new(14.0, 0.0, 0.0),
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
            ..default()
        },
    )).id();
    commands.entity(arrow_root).add_children(&[shaft, head]);
}

/// Points the arrow from the ship toward the nearest active bounty target;
/// hidden when there's no active bounty or the target is already close.
pub fn update_bounty_arrow(
    ship_query: Query<(&Transform, Option<&crate::ship::collision::Collider>), (With<Ship>, Without<BountyArrow>)>,
    // Without<AiShip> here isn't just documentation — an arrow entity never
    // is an AiShip, but without this filter Bevy can't statically prove that
    // and panics (B0001) over the &mut Transform here vs the &Transform read
    // in bounty_ship_query below.
    mut arrow_query: Query<(&mut Transform, &mut Visibility), (With<BountyArrow>, Without<AiShip>)>,
    contract_state: Res<ContractState>,
    sim: Res<WorldSimulation>,
    bounty_ship_query: Query<(&Transform, &BountyTarget), With<AiShip>>,
) {
    let Ok((ship_transform, collider)) = ship_query.single() else { return };
    let Ok((mut arrow_transform, mut vis)) = arrow_query.single_mut() else { return };
    let ship_pos = ship_transform.translation.truncate();

    let targets = active_bounty_positions(&contract_state, &sim, &bounty_ship_query);

    let Some(nearest) = targets.iter().min_by(|a, b| {
        ship_pos.distance_squared(**a)
            .partial_cmp(&ship_pos.distance_squared(**b))
            .unwrap_or(std::cmp::Ordering::Equal)
    }) else {
        *vis = Visibility::Hidden;
        return;
    };

    let dist = ship_pos.distance(*nearest);
    if dist < 500.0 {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    let dir = (*nearest - ship_pos).normalize_or_zero();
    let orbit = crate::world::home_base::nav_arrow_orbit(ship_transform, collider, dir);
    arrow_transform.translation.x = orbit.x;
    arrow_transform.translation.y = orbit.y;
    arrow_transform.rotation = Quat::from_rotation_z(dir.y.atan2(dir.x));
}

/// Spawns a floating "BOUNTY TARGET" label the moment a ship is tagged and
/// becomes real — the thing that actually answers "which one do I shoot"
/// when several look-alike ships are sitting together.
pub fn spawn_bounty_markers(
    mut commands: Commands,
    new_targets: Query<(Entity, &Transform), Added<BountyTarget>>,
) {
    for (entity, transform) in new_targets.iter() {
        commands.spawn((
            Text2d::new("* BOUNTY TARGET *"),
            TextFont { font_size: FontSize::Px(20.0), ..default() },
            TextColor(Color::srgb(1.0, 0.25, 0.25)),
            Transform::from_translation(transform.translation + Vec3::new(0.0, 140.0, 6.0)),
            BountyMarker { target: entity },
            crate::camera::ZoomInvariantText,
        ));
    }
}

/// Follows the tagged ship's position (not its rotation, so the label stays
/// upright) and cleans itself up once the ship is destroyed or leaves
/// render range and despawns back to simulation.
pub fn update_bounty_markers(
    mut commands: Commands,
    mut markers: Query<(Entity, &mut Transform, &BountyMarker), Without<AiShip>>,
    ships: Query<(&Transform, &AiShipState), With<AiShip>>,
) {
    for (marker_entity, mut marker_transform, marker) in markers.iter_mut() {
        match ships.get(marker.target) {
            Ok((ship_transform, state)) if !state.is_destroyed => {
                marker_transform.translation.x = ship_transform.translation.x;
                marker_transform.translation.y = ship_transform.translation.y + 140.0;
            }
            _ => {
                commands.entity(marker_entity).despawn();
            }
        }
    }
}

