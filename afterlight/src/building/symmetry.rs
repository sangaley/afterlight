use bevy::prelude::*;
use crate::components::*;
use crate::events::*;
use crate::states::BuildState;

// ============================================================================
// SYMMETRY MODE
// Toggle with Y key during build mode.
// Places a mirror copy at (x, -y): port to starboard on a nose-right hull.
// Visual center line shown when active.
// ============================================================================

/// Resource tracking symmetry state
#[derive(Resource, Default)]
pub struct SymmetryState {
    pub enabled: bool,
}

/// Marker for the symmetry center line visual
#[derive(Component)]
pub struct SymmetryCenterLine;

/// Toggle symmetry with Y key
pub fn toggle_symmetry(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut symmetry: ResMut<SymmetryState>,
    mut commands: Commands,
    existing_line: Query<Entity, With<SymmetryCenterLine>>,
    mut notifications: MessageWriter<ShowNotification>,
    current_state: Res<State<BuildState>>,
    ship_query: Query<Entity, With<Ship>>,
) {
    if *current_state.get() == BuildState::Inactive { return; }

    if keyboard.just_pressed(KeyCode::KeyY) {
        symmetry.enabled = !symmetry.enabled;

        if symmetry.enabled {
            // Centre line along the mirror axis: grid row 0, whose centre sits
            // at ship-local y = -33 (hull and modules are offset half a cell).
            // Parented to the ship, so it lies on the hull wherever the hull
            // is -- it used to stand upright at the world origin.
            if let Ok(ship) = ship_query.single() {
                commands.spawn((
                    Sprite {
                        color: Color::srgba(0.3, 0.6, 1.0, 0.2),
                        custom_size: Some(Vec2::new(5000.0, 2.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, -33.0, 0.3),
                    ChildOf(ship),
                    SymmetryCenterLine,
                ));
            }
            notifications.write(ShowNotification {
                message: "Symmetry mode ON - blocks mirror port to starboard".into(),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
        } else {
            // Despawn center line
            for entity in existing_line.iter() {
                commands.entity(entity).despawn();
            }
            notifications.write(ShowNotification {
                message: "Symmetry mode OFF".into(),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
        }
    }
}

/// Get the mirrored position: reflected across the ship's long axis.
///
/// Ships build nose-right (engines at minimum x), so port and starboard are
/// +y and -y and grid row 0 is the centreline -- the starter pincer spans
/// rows -9..9. This used to reflect across x=0, which on a nose-right hull is
/// bow-to-stern: an engine placed at the rear got its copy at the nose.
pub fn mirror_position(pos: IVec2) -> IVec2 {
    IVec2::new(pos.x, -pos.y)
}

/// Get the mirrored rotation: across the long axis, a block facing port faces
/// starboard; one facing fore or aft keeps its heading.
pub fn mirror_rotation(rotation: Rotation) -> Rotation {
    match rotation {
        Rotation::North => Rotation::South,
        Rotation::South => Rotation::North,
        Rotation::East => Rotation::East,
        Rotation::West => Rotation::West,
    }
}

/// Where the mirror copy of a placement can go, if it can go anywhere.
///
/// Mirrors the whole footprint, not just the origin cell: a multi-cell block
/// grows from its origin, so a reflected origin alone lands the copy one cell
/// off. Finds the origin and rotation whose cells are exactly the reflected
/// set, then requires what the ghost required of the original -- every cell
/// free, clear of the original's own cells, touching the hull (or the
/// original), and `position_ok` at the new origin. `None` means place only
/// the original.
pub fn valid_mirror(
    origin: IVec2,
    size: IVec2,
    rotation: Rotation,
    footprint: Option<&[IVec2]>,
    occupancy: &crate::building::GridOccupancy,
    position_ok: impl Fn(IVec2) -> bool,
) -> Option<(IVec2, Rotation)> {
    use crate::building::GridOccupancy;
    use std::collections::HashSet;

    let original: HashSet<IVec2> =
        GridOccupancy::cells_for(origin, size, rotation, footprint).into_iter().collect();
    let want: HashSet<IVec2> = original.iter().map(|&c| mirror_position(c)).collect();
    // On the centreline the copy would land on the original.
    if !want.is_disjoint(&original) {
        return None;
    }
    let preferred = mirror_rotation(rotation);
    let rotations = [preferred, rotation, Rotation::North, Rotation::East, Rotation::South, Rotation::West];
    let (m_origin, m_rot) = rotations.iter().find_map(|&rot| {
        want.iter().copied().find(|&cand| {
            GridOccupancy::cells_for(cand, size, rot, footprint).into_iter().collect::<HashSet<_>>() == want
        }).map(|cand| (cand, rot))
    })?;

    if want.iter().any(|c| occupancy.cells.contains_key(c)) {
        return None;
    }
    let touches = want.iter().any(|&c| {
        [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y].iter().any(|&d| {
            occupancy.cells.contains_key(&(c + d)) || original.contains(&(c + d))
        })
    });
    if !touches || !position_ok(m_origin) {
        return None;
    }
    Some((m_origin, m_rot))
}

#[cfg(test)]
mod symmetry_tests {
    use super::*;
    use crate::building::GridOccupancy;

    fn hull_at(cells: &[IVec2]) -> GridOccupancy {
        let mut occ = GridOccupancy::default();
        for (i, &c) in cells.iter().enumerate() {
            occ.cells.insert(c, Entity::from_raw_u32(i as u32 + 1).unwrap());
        }
        occ
    }

    #[test]
    fn mirrors_port_to_starboard_not_bow_to_stern() {
        // A spine along row 0, x -3..3; place at (2, 1).
        let occ = hull_at(&(-3..=3).map(|x| IVec2::new(x, 0)).collect::<Vec<_>>());
        let got = valid_mirror(IVec2::new(2, 1), IVec2::ONE, Rotation::North, None, &occ, |_| true);
        assert_eq!(got, Some((IVec2::new(2, -1), Rotation::South)));
    }

    #[test]
    fn a_two_cell_block_mirrors_its_whole_footprint() {
        let occ = hull_at(&(-3..=3).map(|x| IVec2::new(x, 0)).collect::<Vec<_>>());
        let size = IVec2::new(1, 2);
        let (m_origin, m_rot) =
            valid_mirror(IVec2::new(1, 1), size, Rotation::North, None, &occ, |_| true).unwrap();
        let mut got: Vec<IVec2> = GridOccupancy::cells_for(m_origin, size, m_rot, None).into_iter().collect();
        got.sort_by_key(|c| (c.x, c.y));
        let mut want: Vec<IVec2> = GridOccupancy::cells_for(IVec2::new(1, 1), size, Rotation::North, None)
            .into_iter().map(mirror_position).collect();
        want.sort_by_key(|c| (c.x, c.y));
        assert_eq!(got, want);
    }

    #[test]
    fn no_copy_on_the_centreline_or_onto_a_filled_cell() {
        let occ = hull_at(&[IVec2::new(0, 0), IVec2::new(1, 0), IVec2::new(1, -1)]);
        assert_eq!(valid_mirror(IVec2::new(2, 0), IVec2::ONE, Rotation::North, None, &occ, |_| true), None);
        assert_eq!(valid_mirror(IVec2::new(1, 1), IVec2::ONE, Rotation::North, None, &occ, |_| true), None);
    }

    #[test]
    fn the_copy_obeys_position_rules() {
        let occ = hull_at(&(-3..=3).map(|x| IVec2::new(x, 0)).collect::<Vec<_>>());
        assert_eq!(valid_mirror(IVec2::new(2, 1), IVec2::ONE, Rotation::North, None, &occ, |_| false), None);
    }
}
