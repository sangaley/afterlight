use bevy::prelude::*;
use crate::ai_ship::components::{AiShip, BountyTarget, WorldSimulation};
use crate::components::Ship;
use crate::celestial::components::*;
use crate::contracts::{ContractObjective, ContractState};
use crate::world::home_base::SystemStations;
use crate::ui::theme::ThemeColors;
use super::framework::*;

// ============================================================================
// SYSTEM MINIMAP — floating window showing star system from above
// ============================================================================

#[derive(Component)]
pub struct MinimapWindow;

#[derive(Component)]
pub struct MinimapShipDot;

#[derive(Component)]
pub struct MinimapBodyDot {
    pub entity: Entity,
}

/// Green station marker on the minimap — Haven Station and the resupply
/// outposts. Positioned relative to the player each frame, clamped to the
/// canvas edge when the station is beyond MINIMAP_RANGE so distant outposts
/// still show a direction instead of vanishing.
#[derive(Component)]
pub struct MinimapStationDot {
    pub world_pos: Vec2,
}

/// Marker on the minimap canvas so bounty dots can be added to it dynamically
/// (unlike station dots, the set of active bounties changes at runtime).
#[derive(Component)]
pub struct MinimapCanvas;

/// Red marker for an active bounty's tagged ship. Resolved live each frame
/// from either the real spawned entity (if in render range) or the
/// off-screen simulated position — see update_minimap.
#[derive(Component)]
pub struct MinimapBountyDot {
    pub target_id: u32,
}

const MINIMAP_SIZE: f32 = 200.0;
const MINIMAP_RANGE: f32 = 200_000.0; // World units visible on minimap

/// Toggle minimap with N key
pub fn toggle_minimap(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<(Entity, &FloatingWindow)>,
    stations: Res<SystemStations>,
) {
    if !keyboard.just_pressed(KeyCode::KeyN) {
        return;
    }

    // Close by the window root. MinimapWindow marks the content node inside
    // it, and despawning that left the frame and title bar on screen -- N
    // looked like it did nothing, and the next press stacked a second map.
    if let Some((entity, _)) = windows.iter().find(|(_, w)| w.id == "minimap") {
        commands.entity(entity).despawn();
        return;
    }

    // Spawn minimap floating window
    let content = spawn_floating_window(
        &mut commands,
        "minimap",
        "System Map",
        Vec2::new(MINIMAP_SIZE + 16.0, MINIMAP_SIZE + 50.0),
        Vec2::new(10.0, 100.0),
    );

    // Mark the window
    // Find the root FloatingWindow entity (parent of content's parent)
    commands.entity(content).insert(MinimapWindow);

    // Minimap canvas (dark background)
    let canvas = commands.spawn(
        (Node {
                width: Val::Px(MINIMAP_SIZE),
                height: Val::Px(MINIMAP_SIZE),
                position_type: PositionType::Relative,
                ..default()
            }, BackgroundColor(ThemeColors::HUD_BG), MinimapCanvas),
    ).id();

    // Ship dot (center initially, updates each frame)
    let ship_dot = commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                width: Val::Px(4.0),
                height: Val::Px(4.0),
                left: Val::Px(MINIMAP_SIZE / 2.0),
                top: Val::Px(MINIMAP_SIZE / 2.0),
                ..default()
            }, BackgroundColor(Color::srgb(0.2, 1.0, 0.3))),
        MinimapShipDot,
    )).id();

    commands.entity(canvas).add_child(ship_dot);

    spawn_station_dots(&mut commands, canvas, &stations);

    commands.entity(content).add_child(canvas);
}

/// Station dots for whichever system is loaded. Haven (index 0) gets the
/// bigger, brighter dot; every other station is a full station too, just not
/// home. Position is corrected every frame in update_minimap; the spawn-time
/// value just avoids a one-frame jump at the map's center.
fn spawn_station_dots(commands: &mut Commands, canvas: Entity, stations: &SystemStations) {
    for site in stations.sites.iter() {
        let (size, color) = if site.index == 0 {
            (8.0, Color::srgb(0.25, 1.0, 0.35))
        } else {
            (6.0, Color::srgb(0.2, 0.85, 0.35))
        };
        let dot = commands.spawn((
            (Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(size),
                    height: Val::Px(size),
                    left: Val::Px(MINIMAP_SIZE / 2.0 - size / 2.0),
                    top: Val::Px(MINIMAP_SIZE / 2.0 - size / 2.0),
                    ..default()
                }, BackgroundColor(color)),
            MinimapStationDot { world_pos: site.pos },
        )).id();
        commands.entity(canvas).add_child(dot);
    }
}

/// Update minimap dots based on ship position and celestial body positions
pub fn update_minimap(
    mut commands: Commands,
    ship_query: Query<&Transform, With<Ship>>,
    _body_query: Query<(Entity, &Transform, &CelestialBody)>,
    mut ship_dot_query: Query<&mut Node, (With<MinimapShipDot>, Without<MinimapBodyDot>, Without<MinimapStationDot>, Without<MinimapBountyDot>)>,
    mut station_dot_query: Query<(Entity, &mut Node, &MinimapStationDot)>,
    mut bounty_dot_query: Query<(Entity, &mut Node, &MinimapBountyDot), Without<MinimapStationDot>>,
    canvas_query: Query<Entity, With<MinimapCanvas>>,
    minimap_exists: Query<Entity, With<MinimapWindow>>,
    contract_state: Res<ContractState>,
    sim: Res<WorldSimulation>,
    bounty_ship_query: Query<(&Transform, &BountyTarget), With<AiShip>>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    stations: Res<SystemStations>,
) {
    if minimap_exists.is_empty() {
        return;
    }

    // Dots were made once, when the map opened, so after a jump they still
    // showed the old system's stations. Rebuild them when the set changes.
    if stations.is_changed() {
        if let Ok(canvas) = canvas_query.single() {
            for (entity, _, _) in station_dot_query.iter() {
                commands.entity(entity).despawn();
            }
            spawn_station_dots(&mut commands, canvas, &stations);
        }
    }

    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    // Ship is always at center
    for mut style in ship_dot_query.iter_mut() {
        style.left = Val::Px(MINIMAP_SIZE / 2.0 - 2.0);
        style.top = Val::Px(MINIMAP_SIZE / 2.0 - 2.0);
    }

    let half = MINIMAP_SIZE / 2.0;
    let place = |style: &mut Node, world_pos: Vec2| {
        let offset = world_pos - ship_pos;
        let clamped = if offset.length() > MINIMAP_RANGE {
            offset.normalize_or_zero() * MINIMAP_RANGE
        } else {
            offset
        };
        let px = (clamped.x / MINIMAP_RANGE) * half;
        let py = -(clamped.y / MINIMAP_RANGE) * half; // flip Y for screen space
        // Centre the dot on its point and keep the whole dot on the canvas;
        // clamping the top-left corner let edge dots hang off it.
        let size = match style.width { Val::Px(w) => w, _ => 0.0 };
        style.left = Val::Px((half + px - size / 2.0).clamp(0.0, MINIMAP_SIZE - size));
        style.top = Val::Px((half + py - size / 2.0).clamp(0.0, MINIMAP_SIZE - size));
    };

    // Every system has two full stations now, all in the loaded system's
    // own space (SystemStations follows the loaded system), so they show
    // wherever you are -- they used to be hidden outside Haven.
    for (_, mut style, dot) in station_dot_query.iter_mut() {
        place(&mut style, dot.world_pos);
    }

    // Active bounty targets: resolve each one's live position (spawned real
    // entity first, else the off-screen simulated position), sync dots to
    // match, and place them — same clamp-to-edge treatment as stations.
    // Only bounties whose ship is actually in THIS system are meaningful
    // here — a Warm-neighbor ship's position is relative to a different
    // system's local space entirely.
    let active_targets: Vec<(u32, Vec2)> = contract_state.active_contracts.iter()
        .filter_map(|c| match &c.objective {
            ContractObjective::DestroyShip { target_id, destroyed: false, .. } => {
                if sim.ships.iter().find(|s| s.bounty_id == Some(*target_id)).is_some_and(|s| Some(s.system_id) != streaming.loaded_system) {
                    return None;
                }
                let pos = bounty_ship_query.iter()
                    .find(|(_, b)| b.0 == *target_id)
                    .map(|(t, _)| t.translation.truncate())
                    .or_else(|| sim.bounty_position(*target_id));
                pos.map(|p| (*target_id, p))
            }
            _ => None,
        })
        .collect();

    for (entity, _, dot) in bounty_dot_query.iter() {
        if !active_targets.iter().any(|(id, _)| *id == dot.target_id) {
            commands.entity(entity).despawn();
        }
    }

    if let Ok(canvas) = canvas_query.single() {
        let existing_ids: Vec<u32> = bounty_dot_query.iter().map(|(_, _, d)| d.target_id).collect();
        for (id, _) in &active_targets {
            if !existing_ids.contains(id) {
                let dot = commands.spawn((
                    (Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(9.0),
                            height: Val::Px(9.0),
                            left: Val::Px(half),
                            top: Val::Px(half),
                            ..default()
                        }, BackgroundColor(Color::srgb(1.0, 0.15, 0.15))),
                    MinimapBountyDot { target_id: *id },
                )).id();
                commands.entity(canvas).add_child(dot);
            }
        }
    }

    for (_, mut style, dot) in bounty_dot_query.iter_mut() {
        if let Some((_, world_pos)) = active_targets.iter().find(|(id, _)| *id == dot.target_id) {
            place(&mut style, *world_pos);
        }
    }
}

#[cfg(test)]
mod toggle_tests {
    use super::*;

    #[test]
    fn n_opens_the_map_and_n_closes_all_of_it() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<SystemStations>();
        app.add_systems(Update, toggle_minimap);
        let press = |app: &mut App| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::KeyN);
            keys.clear();
            keys.press(KeyCode::KeyN);
            app.update();
        };
        let nodes = |app: &mut App| app.world_mut().query::<&Node>().iter(app.world()).count();

        press(&mut app);
        assert!(nodes(&mut app) > 0, "N did not open the map");
        press(&mut app);
        assert_eq!(nodes(&mut app), 0, "the window frame outlived the close");
    }
}
