use bevy::prelude::*;
use crate::components::*;

// ============================================================================
// FIRE GROUP SYSTEM
// 4 groups assigned in build mode. Keys 1-4 fire each group.
// Hold for sustained fire. Unassigned weapons default to group 1.
// ============================================================================

/// Component on weapon modules — which fire group they belong to
#[derive(Component, Default)]
pub struct FireGroup {
    pub group: u8, // 0-3 (displayed as 1-4)
}

/// Component on weapon modules — whether this weapon is in intercept mode
#[derive(Component)]
pub struct InterceptMode;

/// Component on weapon modules — whether this weapon auto-fires at selected target
#[derive(Component)]
pub struct AutoFireMode;

/// Resource tracking which fire groups are currently firing
#[derive(Resource, Default)]
pub struct FireGroupState {
    pub firing: [bool; 4],
}

/// System: read fire inputs, set fire group state.
/// Space or left-click fires everything (matching the "Space: Fire" HUD
/// hint); 1-4 fire individual groups for players who assign them.
pub fn fire_group_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    interactions: Query<&Interaction>,
    mut state: ResMut<FireGroupState>,
    // A right-click lock fires the battery for you (see aim_lock) — the point
    // of picking a block is to keep working it while you fly, not to hold
    // Space at it. Manual fire still works, locked or not.
    lock: Res<super::aim_lock::AimLock>,
    marks: Res<super::auto_engage::MarkedTargets>,
    player_ship: Query<(&Transform, &Children), With<Ship>>,
    player_weapons: Query<(&Weapon, &Module), Without<crate::ai_ship::components::OwnedByAiShip>>,
) {
    // A left-click that lands on interactive UI (the HUD toolbar, panels, window
    // chrome) shouldn't also fire the guns — suppress mouse-fire while the cursor
    // is over any hovered/pressed UI element. Space still fires unconditionally.
    let over_ui = interactions.iter().any(|i| !matches!(i, Interaction::None));
    // Longest live gun aboard sets how far the auto-fire lock reaches.
    let auto = player_ship.single().ok().is_some_and(|(transform, children)| {
        let max_range = children.iter()
            .filter_map(|c| player_weapons.get(c).ok())
            .filter(|(_, module)| module.is_active && module.health > 0.0)
            .map(|(weapon, _)| weapon.range)
            .fold(0.0_f32, f32::max);
        super::aim_lock::auto_fire_engaged(&lock, transform.translation.truncate(), max_range)
    });

    // Marks fire the same way; each gun holds until it has one it can reach.
    let fire_all = auto
        || !marks.ships.is_empty()
        || keyboard.pressed(KeyCode::Space)
        || (mouse.pressed(MouseButton::Left) && !over_ui);
    state.firing[0] = keyboard.pressed(KeyCode::Digit1) || fire_all;
    state.firing[1] = keyboard.pressed(KeyCode::Digit2) || fire_all;
    state.firing[2] = keyboard.pressed(KeyCode::Digit3) || fire_all;
    state.firing[3] = keyboard.pressed(KeyCode::Digit4) || fire_all;
}

/// System: assign fire groups during build mode with Ctrl+1-4
pub fn assign_fire_group(
    keyboard: Res<ButtonInput<KeyCode>>,
    _mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera>>,
    ship_query: Query<&GlobalTransform, (With<Ship>, Without<Camera>)>,
    occupancy: Res<crate::building::GridOccupancy>,
    mut weapon_query: Query<(Entity, &Module, &mut FireGroup), With<Weapon>>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !ctrl { return; }

    let group = if keyboard.just_pressed(KeyCode::Digit1) { Some(0) }
        else if keyboard.just_pressed(KeyCode::Digit2) { Some(1) }
        else if keyboard.just_pressed(KeyCode::Digit3) { Some(2) }
        else if keyboard.just_pressed(KeyCode::Digit4) { Some(3) }
        else { None };

    let Some(group) = group else { return };

    // Find weapon under cursor — grid cells are ship-local, so the cursor
    // has to be converted through the ship's transform (see
    // building::cursor_to_ship_grid), not divided by the cell size in
    // world space
    let Ok(window) = windows.single() else { return };
    let Ok((camera, cam_transform)) = camera_query.single() else { return };
    let Ok(ship_gt) = ship_query.single() else { return };
    let Some(grid_pos) =
        crate::building::cursor_to_ship_grid(window, camera, cam_transform, ship_gt)
    else { return };

    if let Some(&entity) = occupancy.cells.get(&grid_pos) {
        if let Ok((_, module, mut fire_group)) = weapon_query.get_mut(entity) {
            fire_group.group = group;
            notifications.write(crate::events::ShowNotification {
                message: format!("{} assigned to Fire Group {}", module.module_type.name(), group + 1),
                notification_type: crate::events::NotificationType::Info,
                duration: 2.0,
            });
        }
    }
}
