use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};
use std::collections::{VecDeque};
use crate::components::*;  // includes BulkheadSealed
use crate::resources::PowerGraph;

/// A detected room inside the ship
#[derive(Debug, Clone)]
pub struct Room {
    pub id: usize,
    pub tiles: Vec<IVec2>,
    pub air_level: f32,       // 1.0 = pressurized, 0.0 = vacuum
    pub is_breached: bool,
    pub has_power: bool,
}

/// Resource tracking all detected rooms
#[derive(Resource, Default)]
pub struct RoomMap {
    pub rooms: Vec<Room>,
    /// Maps grid position -> room id
    pub tile_to_room: HashMap<IVec2, usize>,
}

/// Grid size constant matching the rest of the building system
const GRID_SIZE: f32 = 66.0;

/// Converts a Transform to a grid position
pub fn transform_to_grid(t: &Transform) -> IVec2 {
    IVec2::new(
        (t.translation.x / GRID_SIZE).round() as i32,
        ((t.translation.y + 33.0) / GRID_SIZE).round() as i32,
    )
}

/// Flood-fill room detection.
///
/// A room is a connected group of cells that CARRY SOMETHING — a module or a
/// hallway. It is not "space enclosed by walls", and nothing here tests for
/// enclosure. Bare space is never interior however much wall you ring it with:
/// no id, no air, no fire, nowhere a crew member can stand. Laying decking is
/// what turns a gap into a place. (See room_shape_tests.)
///
/// Inner hull only ever SEPARATES two groups that would otherwise touch; a
/// sealed bulkhead does the same for as long as it stays shut. Outer hull and
/// Void are not walls at all to this fill — they are simply not interior, so
/// it never reaches them.
/// PLAYER SHIP ONLY: hull/module queries span every ship in the world (grid
/// positions are ship-local, so an AI ship's tiles routinely collide
/// numerically with the player's). Unscoped, AI hull/modules leaked into the
/// player's flood-fill — a wall from a ship the player has never seen could
/// block a room, or an AI module could get counted as player interior space.
pub fn detect_rooms(
    hull_query: &Query<(&HullSegment, &Transform, &ChildOf)>,
    module_query: &Query<(&Module, &Transform, &ChildOf)>,
    sealed_positions: &HashSet<IVec2>,
    player_ship: Entity,
) -> RoomMap {
    // Collect all hull tile positions by layer
    let mut inner_hull_positions: HashSet<IVec2> = HashSet::new();
    let mut outer_hull_positions: HashSet<IVec2> = HashSet::new();
    // Hallways are open space, not structure: they carry air and fire and
    // join the compartments either side of them into one room.
    let mut hallway_positions: HashSet<IVec2> = HashSet::new();
    let mut all_hull_positions: HashSet<IVec2> = HashSet::new();

    for (hull, transform, parent) in hull_query.iter() {
        if parent.parent() != player_ship { continue; }
        let grid = transform_to_grid(transform);
        all_hull_positions.insert(grid);
        match hull.hull_layer {
            HullLayer::Inner => { inner_hull_positions.insert(grid); }
            HullLayer::BulkheadDoor => {
                // Only sealed bulkheads act as walls; unsealed are passable
                if sealed_positions.contains(&grid) {
                    inner_hull_positions.insert(grid);
                }
            }
            HullLayer::Hallway => { hallway_positions.insert(grid); }
            HullLayer::Outer => { outer_hull_positions.insert(grid); }
            HullLayer::Void => {}
        }
    }

    // Collect module positions as "interior" cells -- but only the ones that
    // actually enclose breathable volume. Armour plating is a module too, and
    // counting it made most of every ship's "rooms" out of exterior hull
    // plating. See `ModuleType::holds_atmosphere`.
    let mut module_positions: HashSet<IVec2> = HashSet::new();
    for (module, _transform, parent) in module_query.iter() {
        if parent.parent() != player_ship { continue; }
        if !module.module_type.holds_atmosphere() { continue; }
        module_positions.insert(module.grid_position);
    }

    // Anything sealed is a wall, whatever kind of block it is. The match above
    // only consults `sealed_positions` for BulkheadDoor hull segments, so a
    // sealed MODULE -- which is what an emergency bulkhead and a flood valve
    // both are -- would otherwise stay ordinary interior space and divide
    // nothing.
    inner_hull_positions.extend(sealed_positions.iter().copied());

    // Interior space is anything a person or a lungful of air can occupy:
    // the modules themselves plus the hallways joining them.
    // Sealed blocks are subtracted, not merely treated as walls. The flood
    // below starts from every interior cell in turn, so a sealed valve left in
    // the interior set seeds its own fill and rejoins the two sides it is
    // supposed to be keeping apart -- the neighbour test never gets a say.
    let interior_positions: HashSet<IVec2> = module_positions
        .union(&hallway_positions)
        .copied()
        .filter(|p| !inner_hull_positions.contains(p))
        .collect();

    // Flood-fill from each unvisited interior cell. Connected interior cells
    // (adjacent, not separated by inner hull) form a room.
    let mut visited: HashSet<IVec2> = HashSet::new();
    let mut rooms = Vec::new();
    let mut tile_to_room = HashMap::new();

    for &pos in &interior_positions {
        if visited.contains(&pos) {
            continue;
        }

        // BFS flood fill
        let mut queue = VecDeque::new();
        let mut room_tiles = Vec::new();
        queue.push_back(pos);
        visited.insert(pos);

        while let Some(current) = queue.pop_front() {
            room_tiles.push(current);

            // Check 4 neighbors
            for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let neighbor = current + offset;
                if visited.contains(&neighbor) {
                    continue;
                }
                // Stop at inner hull boundaries (walls)
                if inner_hull_positions.contains(&neighbor) {
                    continue;
                }
                // Only flood into other interior space
                if interior_positions.contains(&neighbor) {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }

        let room_id = rooms.len();
        for &tile in &room_tiles {
            tile_to_room.insert(tile, room_id);
        }

        rooms.push(Room {
            id: room_id,
            tiles: room_tiles,
            air_level: 1.0,
            is_breached: false,
            has_power: false,
        });
    }

    RoomMap { rooms, tile_to_room }
}

/// System that recalculates rooms when hull tiles change, preserving flood state.
/// Uses tile-set intersection: each tile remembers its old room's flood state,
/// and the new room inherits the worst (highest water) state from overlapping tiles.
///
/// Only when the player ship's layout changed (`layout::LayoutVersion`: a
/// block placed, destroyed, removed, or a bulkhead sealed or opened). It used
/// to re-flood every room every frame, whatever happened -- the single most
/// expensive system in a capital-sized fight.
pub fn update_room_map(
    ship_query: Query<(Entity, &super::layout::LayoutVersion), With<Ship>>,
    hull_query: Query<(&HullSegment, &Transform, &ChildOf)>,
    module_query: Query<(&Module, &Transform, &ChildOf)>,
    sealed_query: Query<(&HullSegment, &Transform, &ChildOf), With<BulkheadSealed>>,
    sealed_modules: Query<(&Module, &ChildOf), With<BulkheadSealed>>,
    mut room_map: ResMut<RoomMap>,
    mut built_from: Local<Option<(Entity, super::layout::LayoutVersion)>>,
) {
    let Ok((player_ship, version)) = ship_query.single() else { return };
    if *built_from == Some((player_ship, *version)) {
        return;
    }
    *built_from = Some((player_ship, *version));

    // Build set of sealed bulkhead positions.
    //
    // Modules count as well as hull segments. Emergency bulkheads and flood
    // valves are MODULES, and `fire::emergency_bulkhead_system` has always
    // inserted `BulkheadSealed` on them -- but this query only ever looked at
    // hull segments, so the marker was read by nobody and the ship's automatic
    // containment gear did precisely nothing.
    let sealed_positions: HashSet<IVec2> = sealed_query
        .iter()
        .filter(|(_, _, parent)| parent.parent() == player_ship)
        .map(|(_, transform, _)| transform_to_grid(transform))
        .chain(
            sealed_modules
                .iter()
                .filter(|(_, parent)| parent.parent() == player_ship)
                .map(|(module, _)| module.grid_position),
        )
        .collect();

    // Save air state for every tile in every room
    let mut tile_air_state: HashMap<IVec2, (f32, bool)> = HashMap::new();
    for room in room_map.rooms.iter() {
        if room.air_level < 1.0 || room.is_breached {
            for &tile in &room.tiles {
                tile_air_state.insert(tile, (room.air_level, room.is_breached));
            }
        }
    }

    *room_map = detect_rooms(&hull_query, &module_query, &sealed_positions, player_ship);

    // Restore air state: new room inherits the lowest air level from any
    // overlapping old tile, and is_breached if any overlapping tile was breached.
    for room in room_map.rooms.iter_mut() {
        let mut min_air = 1.0_f32;
        let mut any_breached = false;
        for tile in &room.tiles {
            if let Some(&(air_level, is_breached)) = tile_air_state.get(tile) {
                min_air = min_air.min(air_level);
                any_breached = any_breached || is_breached;
            }
        }
        if min_air < 1.0 || any_breached {
            room.air_level = min_air;
            room.is_breached = any_breached;
        }
    }
}

/// System that checks power connectivity using the PowerGraph.
/// A room has power if any of its tiles are in the power graph.
pub fn update_room_power(
    mut room_map: ResMut<RoomMap>,
    power_graph: Res<PowerGraph>,
) {
    for room in room_map.rooms.iter_mut() {
        room.has_power = room.tiles.iter().any(|t| power_graph.powered_tiles.contains(t));
    }
}

#[cfg(test)]
mod room_shape_tests {
    use super::*;
    use crate::building::grid_to_local;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<RoomMap>();
        app.add_plugins(crate::building::layout::LayoutPlugin);
        app.add_systems(Update, update_room_map);
        app
    }

    fn hull(app: &mut App, ship: Entity, cell: IVec2, layer: HullLayer) {
        app.world_mut()
            .spawn((
                HullSegment { grid_position: cell, hull_layer: layer, ..default() },
                Transform::from_translation(grid_to_local(cell).extend(0.1)),
            ))
            .insert(ChildOf(ship));
    }

    fn room_of(app: &App, cell: IVec2) -> Option<usize> {
        app.world().resource::<RoomMap>().tile_to_room.get(&cell).copied()
    }

    fn room_count(app: &App) -> usize {
        app.world().resource::<RoomMap>().rooms.len()
    }

    /// The rule the flood fill actually implements is NOT "space enclosed by
    /// walls". Interior space is the set of cells that carry a module or a
    /// hallway; the fill starts only from those and only spreads into those.
    /// Enclosure never enters into it. A bare cell ringed by wall on every
    /// side is not a room, it is nothing — no id, no air, no fire, and no
    /// crew member can be in it.
    #[test]
    fn a_bare_cell_ringed_by_walls_is_not_a_room() {
        let mut app = app();
        let ship = app.world_mut().spawn(Ship).id();

        // Eight inner-hull cells around a bare centre.
        for y in 0..3 {
            for x in 0..3 {
                if (x, y) == (1, 1) {
                    continue;
                }
                hull(&mut app, ship, IVec2::new(x, y), HullLayer::Inner);
            }
        }

        app.update();

        assert_eq!(room_of(&app, IVec2::new(1, 1)), None, "bare space became a room");
        assert_eq!(room_count(&app), 0, "walls alone produced a room");
    }

    /// Lay decking in that same hole and it becomes a room — a one-tile one,
    /// walled off from everything. The decking is what makes it a place.
    #[test]
    fn decking_in_the_hole_is_what_makes_it_a_room() {
        let mut app = app();
        let ship = app.world_mut().spawn(Ship).id();

        for y in 0..3 {
            for x in 0..3 {
                if (x, y) == (1, 1) {
                    continue;
                }
                hull(&mut app, ship, IVec2::new(x, y), HullLayer::Inner);
            }
        }
        hull(&mut app, ship, IVec2::new(1, 1), HullLayer::Hallway);

        app.update();

        assert_eq!(room_count(&app), 1, "decking did not make a room");
        assert!(room_of(&app, IVec2::new(1, 1)).is_some());
    }

    /// The roundabout: a closed loop of corridor with a hole in the middle.
    /// The whole ring is ONE room — the fill goes round it — and the hole is
    /// not part of it, because nothing was ever laid there. The ring does not
    /// "enclose" anything as far as room detection is concerned.
    #[test]
    fn a_loop_of_corridor_is_one_room_and_its_hole_is_nothing() {
        let mut app = app();
        let ship = app.world_mut().spawn(Ship).id();

        // 3x3 ring of decking, bare centre.
        for y in 0..3 {
            for x in 0..3 {
                if (x, y) == (1, 1) {
                    continue;
                }
                hull(&mut app, ship, IVec2::new(x, y), HullLayer::Hallway);
            }
        }

        app.update();

        assert_eq!(room_count(&app), 1, "the loop did not close into one room");
        let ring = room_of(&app, IVec2::new(0, 0)).expect("no room on the ring");
        // Every corridor cell is the same room, all the way round.
        for (x, y) in [(2, 0), (2, 2), (0, 2), (1, 0), (1, 2)] {
            assert_eq!(
                room_of(&app, IVec2::new(x, y)),
                Some(ring),
                "cell ({x},{y}) was not part of the same loop"
            );
        }
        assert_eq!(room_of(&app, IVec2::new(1, 1)), None, "the hole became interior");
    }

    /// Outer hull is not a wall to the flood fill — it is simply not interior.
    /// Only Inner (and a SEALED bulkhead) separate one room from another, which
    /// is why a hallway running between two module clusters merges them.
    #[test]
    fn only_inner_hull_divides_rooms() {
        let mut app = app();
        let ship = app.world_mut().spawn(Ship).id();

        // A corridor of five, cut in the middle by one inner-hull cell.
        for x in [0, 1, 3, 4] {
            hull(&mut app, ship, IVec2::new(x, 0), HullLayer::Hallway);
        }
        hull(&mut app, ship, IVec2::new(2, 0), HullLayer::Inner);

        app.update();

        assert_eq!(room_count(&app), 2, "an inner-hull wall did not split the corridor");
        assert_ne!(
            room_of(&app, IVec2::new(0, 0)),
            room_of(&app, IVec2::new(4, 0)),
            "both sides of the wall ended up in one room"
        );
    }
}
