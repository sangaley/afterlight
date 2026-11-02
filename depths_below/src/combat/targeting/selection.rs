use bevy::prelude::*;
use crate::components::*;
use crate::events::*;
use crate::ai_ship::components::{AiShip, AiShipWreck};

// ============================================================================
// TARGET SELECTION SYSTEM
// Tab cycles targets. Middle-click selects under cursor.
// Selected target gets bracket. All auto-fire weapons shoot at it.
// ============================================================================

/// Resource tracking the currently selected target
#[derive(Resource, Default)]
pub struct TargetSelection {
    pub target: Option<Entity>,
    pub target_type: TargetType,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetType {
    #[default]
    None,
    Creature,
    Ship,
}

/// Visual bracket around selected target
#[derive(Component)]
pub struct TargetBracket;

/// HUD element showing target info
#[derive(Component)]
pub struct TargetInfoText;

/// System: `\` cycles through valid targets, closest first.
///
/// Was bound to Tab and left unregistered, because Tab is the radar key and
/// the target-lock riding on it was unwanted. It has a key of its own now:
/// with nothing manually locked the guns engage the nearest enemy by
/// themselves (see `targeting::auto_engage`), and this is how you tell them
/// to pick a different one.
pub fn cycle_target(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<TargetSelection>,
    ship_query: Query<&Transform, With<Ship>>,
    creature_query: Query<(Entity, &Transform, &Creature), Without<Ship>>,
    // Without<AiShipWreck>: a defeated hull keeps its AiShip component (see
    // ai_ship::wreck — the wreck is the same entity), so without this filter
    // `\` cycles onto derelicts and calls them "Hostile Ship". The creature
    // branch below has always skipped the dead; this is the same rule.
    ai_ship_query: Query<(Entity, &Transform), (With<AiShip>, Without<AiShipWreck>, Without<Ship>)>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !keyboard.just_pressed(KeyCode::Backslash) { return; }
    // Shift + `\` marks every hostile instead (auto_engage::mark_all_input).
    if keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) { return; }

    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    // Build sorted list of all valid targets by distance
    let mut targets: Vec<(Entity, f32, &str, TargetType)> = Vec::new();

    for (entity, transform, creature) in creature_query.iter() {
        if creature.health <= 0.0 { continue; }
        let dist = ship_pos.distance(transform.translation.truncate());
        if dist > 4000.0 { continue; } // Max targeting range
        let name = match creature.creature_type {
            CreatureType::VoidDrifter => "Void Drifter",
            CreatureType::Stalker => "Stalker",
            CreatureType::Leviathan => "LEVIATHAN",
            CreatureType::ParasiteSwarm => "Parasite",
        };
        targets.push((entity, dist, name, TargetType::Creature));
    }

    for (entity, transform) in ai_ship_query.iter() {
        let dist = ship_pos.distance(transform.translation.truncate());
        if dist > 27_000.0 { continue; } // Was 2000 — shorter than the new weapon/standoff ranges, couldn't lock a ship holding its own distance
        targets.push((entity, dist, "Hostile Ship", TargetType::Ship));
    }

    // Sort by distance
    targets.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    if targets.is_empty() {
        selection.target = None;
        selection.target_type = TargetType::None;
        notifications.write(ShowNotification {
            message: "No targets in range".into(),
            notification_type: NotificationType::Info,
            duration: 1.5,
        });
        return;
    }

    // Find current target in list, cycle to next
    let current_idx = selection.target
        .and_then(|current| targets.iter().position(|(e, _, _, _)| *e == current));

    let next_idx = match current_idx {
        Some(i) => (i + 1) % targets.len(),
        None => 0,
    };

    let (entity, dist, name, target_type) = targets[next_idx];
    selection.target = Some(entity);
    selection.target_type = target_type;

    notifications.write(ShowNotification {
        message: format!("Target: {} ({:.0}m)", name, dist),
        notification_type: NotificationType::Warning,
        duration: 2.0,
    });
}

/// System: middle-click to select target under cursor
pub fn click_select_target(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut selection: ResMut<TargetSelection>,
    creature_query: Query<(Entity, &Transform, &Creature), Without<Ship>>,
    // Same rule as cycle_target: a wreck is still an AiShip entity, and
    // middle-clicking one should not select it as a hostile.
    ai_ship_query: Query<(Entity, &Transform), (With<AiShip>, Without<AiShipWreck>, Without<Ship>)>,
) {
    if !mouse.just_pressed(MouseButton::Middle) { return; }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };
    let Some(cursor) = window.cursor_position()
        .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())
    else { return };

    let click_pos = cursor;
    let select_radius = 100.0;

    // Check creatures first
    let mut closest: Option<(Entity, f32, TargetType)> = None;
    for (entity, transform, creature) in creature_query.iter() {
        if creature.health <= 0.0 { continue; }
        let dist = click_pos.distance(transform.translation.truncate());
        if dist < select_radius {
            if closest.map_or(true, |(_, d, _)| dist < d) {
                closest = Some((entity, dist, TargetType::Creature));
            }
        }
    }

    // Check AI ships
    for (entity, transform) in ai_ship_query.iter() {
        let dist = click_pos.distance(transform.translation.truncate());
        if dist < select_radius {
            if closest.map_or(true, |(_, d, _)| dist < d) {
                closest = Some((entity, dist, TargetType::Ship));
            }
        }
    }

    if let Some((entity, _, target_type)) = closest {
        selection.target = Some(entity);
        selection.target_type = target_type;
    } else {
        // Click on nothing = deselect
        selection.target = None;
        selection.target_type = TargetType::None;
    }
}

/// System: draw bracket around selected target
pub fn draw_target_bracket(
    mut commands: Commands,
    selection: Res<TargetSelection>,
    existing_brackets: Query<Entity, With<TargetBracket>>,
    transform_query: Query<&Transform>,
    creature_query: Query<&Creature>,
    shield_query: Query<&crate::combat::shields::ShipShield>,
    camera: Query<&Projection, With<crate::camera::MainCamera>>,
) {
    // Despawn old brackets
    for entity in existing_brackets.iter() {
        commands.entity(entity).try_despawn();
    }

    let Some(target) = selection.target else { return };
    let Ok(transform) = transform_query.get(target) else {
        return;
    };
    let mut pos = transform.translation.truncate();

    // Get target size for bracket
    let size = if let Ok(creature) = creature_query.get(target) {
        match creature.creature_type {
            CreatureType::Leviathan => 180.0,
            CreatureType::Stalker => 60.0,
            _ => 30.0,
        }
    } else if let Ok(shield) = shield_query.get(target) {
        // A ship is framed by its real extent. A fixed 80-unit box around
        // the ROOT -- which often sits at one end of the layout -- bracketed
        // a corner of a battleship and missed the hull entirely. The bubble
        // is fitted to the live blocks (shields::refresh_shield_extents).
        pos = shield.world_center(transform);
        shield.radius * 2.0
    } else {
        80.0 // Ship default
    };

    let bracket_size = size + 20.0;
    // Stroke keeps its designed on-screen weight at any zoom; at the
    // framing zoom a 2-unit line is a fraction of a pixel.
    let thickness = 2.0 * match camera.single() {
        Ok(Projection::Orthographic(o)) => crate::camera::label_scale(o.scale),
        _ => 1.0,
    };
    let arm_length = bracket_size * 0.3;

    // Draw 4 corner brackets
    let corners = [
        (Vec2::new(-bracket_size / 2.0, bracket_size / 2.0), true, true),   // Top-left
        (Vec2::new(bracket_size / 2.0, bracket_size / 2.0), true, false),   // Top-right
        (Vec2::new(-bracket_size / 2.0, -bracket_size / 2.0), false, true), // Bottom-left
        (Vec2::new(bracket_size / 2.0, -bracket_size / 2.0), false, false), // Bottom-right
    ];

    let bracket_color = match selection.target_type {
        TargetType::Creature => Color::srgb(0.9, 0.3, 0.2),
        TargetType::Ship => Color::srgb(0.9, 0.6, 0.2),
        TargetType::None => Color::WHITE,
    };

    for (offset, is_top, is_left) in corners {
        // Horizontal arm
        let h_x = if is_left { offset.x + arm_length / 2.0 } else { offset.x - arm_length / 2.0 };
        commands.spawn((
            (Sprite {
                    color: bracket_color,
                    custom_size: Some(Vec2::new(arm_length, thickness)),
                    ..default()
                }, Transform::from_xyz(pos.x + h_x, pos.y + offset.y, 1.0)),
            TargetBracket,
        ));

        // Vertical arm
        let v_y = if is_top { offset.y - arm_length / 2.0 } else { offset.y + arm_length / 2.0 };
        commands.spawn((
            (Sprite {
                    color: bracket_color,
                    custom_size: Some(Vec2::new(thickness, arm_length)),
                    ..default()
                }, Transform::from_xyz(pos.x + offset.x, pos.y + v_y, 1.0)),
            TargetBracket,
        ));
    }
}
