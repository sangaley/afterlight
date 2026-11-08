use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::states::BuildState;
use crate::building::{cursor_to_ship_grid, footprints, GridOccupancy, ModuleRegistry};
use crate::building::multiblock::components::*;

// ============================================================================
// COPY/PASTE SELECTION SYSTEM
// Ctrl+click to select modules. Ctrl+C copies. Ctrl+V pastes. R rotates the
// pending paste. Handles multi-block weapon chains as a unit.
//
// All cursor→grid math goes through cursor_to_ship_grid (ship-local space),
// and all highlight/ghost sprites are children of the ship — world-space
// versions of both only lined up while the ship sat at the world origin.
// ============================================================================

/// A copied module definition
#[derive(Clone, Debug)]
pub struct ClipboardEntry {
    pub module_type: ModuleType,
    pub offset: IVec2,  // Relative to selection origin
    pub rotation: Rotation,
    pub has_machine_block: bool,
    pub machine_role: Option<BlockRole>,
}

/// Resource tracking clipboard and selection
#[derive(Resource, Default)]
pub struct BuildClipboard {
    /// Currently selected module entities
    pub selected: Vec<Entity>,
    /// Copied module data (ready to paste)
    pub copied: Vec<ClipboardEntry>,
    /// Whether we're in paste mode
    pub paste_mode: bool,
    /// Origin point of the copy (for offset calculation)
    pub copy_origin: IVec2,
}

/// Marker for selected module visual highlight
#[derive(Component)]
pub struct SelectionHighlight;

/// All grid cells an entry occupies when pasted at `origin`.
fn entry_cells(
    entry: &ClipboardEntry,
    origin: IVec2,
    registry: &ModuleRegistry,
) -> smallvec::SmallVec<[IVec2; 4]> {
    let size = registry.get(entry.module_type).size;
    let footprint = footprints::footprint_override(entry.module_type);
    GridOccupancy::cells_for(origin + entry.offset, size, entry.rotation, footprint)
}

/// System: Ctrl+click to toggle select modules, Ctrl+C to copy, Ctrl+V to
/// paste, Escape to back out one level (paste → selection → build mode)
pub fn clipboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera>>,
    ship_query: Query<(Entity, &GlobalTransform), (With<Ship>, Without<Camera>)>,
    mut clipboard: ResMut<BuildClipboard>,
    occupancy: Res<GridOccupancy>,
    module_query: Query<
        (Entity, &Module, Option<&MachineBlock>),
        Without<crate::ai_ship::components::OwnedByAiShip>,
    >,
    mut commands: Commands,
    existing_highlights: Query<Entity, With<SelectionHighlight>>,
    mut notifications: MessageWriter<ShowNotification>,
    current_state: Res<State<BuildState>>,
    mut next_build_state: ResMut<NextState<BuildState>>,
) {
    if *current_state.get() == BuildState::Inactive { return; }
    let Ok((ship, ship_gt)) = ship_query.single() else { return };

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    // === CTRL+CLICK: Toggle select ===
    if ctrl && mouse.just_pressed(MouseButton::Left) && !clipboard.paste_mode {
        let Ok(window) = windows.single() else { return };
        let Ok((camera, camera_transform)) = camera_query.single() else { return };
        let Some(grid_pos) = cursor_to_ship_grid(window, camera, camera_transform, ship_gt)
        else { return };

        if let Some(&entity) = occupancy.cells.get(&grid_pos) {
            if clipboard.selected.contains(&entity) {
                clipboard.selected.retain(|&e| e != entity);
            } else if module_query.get(entity).is_ok() {
                clipboard.selected.push(entity);

                // Auto-select entire multi-block chain
                if let Ok((_, _module, Some(machine_block))) = module_query.get(entity) {
                    if machine_block.role == BlockRole::Core {
                        // Select all blocks connected to this core
                        for (other_entity, _, other_block) in module_query.iter() {
                            if let Some(other_block) = other_block {
                                if other_block.connected_core == Some(entity)
                                    && !clipboard.selected.contains(&other_entity)
                                {
                                    clipboard.selected.push(other_entity);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Update highlights (children of the ship, one tile per occupied cell)
        for entity in existing_highlights.iter() {
            commands.entity(entity).despawn();
        }
        for &entity in &clipboard.selected {
            if let Ok((_, module, _)) = module_query.get(entity) {
                let footprint = footprints::footprint_override(module.module_type);
                let cells = GridOccupancy::cells_for(
                    module.grid_position, module.size, module.rotation, footprint,
                );
                for cell in cells {
                    commands.spawn((
                        (Sprite {
                                color: Color::srgba(0.3, 0.6, 1.0, 0.3),
                                custom_size: Some(Vec2::splat(64.0)),
                                ..default()
                            }, Transform::from_xyz(
                                cell.x as f32 * 66.0,
                                cell.y as f32 * 66.0 - 33.0,
                                0.9,
                            )),
                        SelectionHighlight,
                        ChildOf(ship),
                    ));
                }
            }
        }
        return;
    }

    // === CTRL+C: Copy selection ===
    if ctrl && keyboard.just_pressed(KeyCode::KeyC) && !clipboard.selected.is_empty() {
        // Collect data first to avoid borrow conflicts
        let selected_entities: Vec<Entity> = clipboard.selected.clone();

        let mut entries: Vec<ClipboardEntry> = Vec::new();
        let mut min_pos = IVec2::new(i32::MAX, i32::MAX);

        for &entity in &selected_entities {
            if let Ok((_, module, _)) = module_query.get(entity) {
                min_pos.x = min_pos.x.min(module.grid_position.x);
                min_pos.y = min_pos.y.min(module.grid_position.y);
            }
        }

        for &entity in &selected_entities {
            if let Ok((_, module, machine_block)) = module_query.get(entity) {
                entries.push(ClipboardEntry {
                    module_type: module.module_type,
                    offset: module.grid_position - min_pos,
                    rotation: module.rotation,
                    has_machine_block: machine_block.is_some(),
                    machine_role: machine_block.map(|mb| mb.role),
                });
            }
        }

        let count = entries.len();
        clipboard.copied = entries;
        clipboard.copy_origin = min_pos;

        notifications.write(ShowNotification {
            message: format!("Copied {} modules", count),
            notification_type: NotificationType::Success,
            duration: 2.0,
        });
    }

    // === CTRL+V: Enter paste mode ===
    if ctrl && keyboard.just_pressed(KeyCode::KeyV) && !clipboard.copied.is_empty() {
        clipboard.paste_mode = true;
        notifications.write(ShowNotification {
            message: "Paste mode - click to place, R to rotate, Escape to cancel".into(),
            notification_type: NotificationType::Info,
            duration: 3.0,
        });
    }

    // === R: Rotate pending paste 90° clockwise ===
    if clipboard.paste_mode && keyboard.just_pressed(KeyCode::KeyR) {
        for entry in clipboard.copied.iter_mut() {
            entry.offset = IVec2::new(entry.offset.y, -entry.offset.x);
            entry.rotation = entry.rotation.rotate_cw();
        }
    }

    // === ESCAPE: back out one level — paste, then selection, then build mode ===
    if keyboard.just_pressed(KeyCode::Escape) {
        if clipboard.paste_mode {
            clipboard.paste_mode = false;
        } else if !clipboard.selected.is_empty() {
            clipboard.selected.clear();
            for entity in existing_highlights.iter() {
                commands.entity(entity).despawn();
            }
        } else {
            next_build_state.set(BuildState::Inactive);
        }
    }

    // === CTRL+A: Select all (player modules only) ===
    if ctrl && keyboard.just_pressed(KeyCode::KeyA) {
        clipboard.selected.clear();
        for (entity, _, _) in module_query.iter() {
            clipboard.selected.push(entity);
        }
        notifications.write(ShowNotification {
            message: format!("Selected all {} modules", clipboard.selected.len()),
            notification_type: NotificationType::Info,
            duration: 2.0,
        });
    }
}

/// System: execute paste when clicking during paste mode
pub fn clipboard_paste(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera>>,
    ship_query: Query<(Entity, &GlobalTransform), (With<Ship>, Without<Camera>)>,
    mut clipboard: ResMut<BuildClipboard>,
    occupancy: Res<GridOccupancy>,
    registry: Res<ModuleRegistry>,
    currency: ResMut<Currency>,
    // Blocks of OUR ship, by parentage. The hull count was unscoped, so every
    // AI ship's plating in the world counted against the player's limit.
    module_count: Query<&ChildOf, With<Module>>,
    hull_count: Query<&ChildOf, With<HullSegment>>,
    mut place_events: MessageWriter<PlaceModuleRequest>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !clipboard.paste_mode { return; }
    if !mouse.just_pressed(MouseButton::Left) { return; }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };
    let Ok((ship, ship_gt)) = ship_query.single() else { return };
    let Some(grid_pos) = cursor_to_ship_grid(window, camera, camera_transform, ship_gt)
    else { return };

    // Check total cost
    let total_cost: u32 = clipboard.copied.iter()
        .map(|entry| registry.get(entry.module_type).cost)
        .sum();

    if currency.credits < total_cost {
        notifications.write(ShowNotification {
            message: format!("Not enough credits! Need {}c, have {}c", total_cost, currency.credits),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
        return;
    }

    // Block limit — a paste is the easy way to blow straight past it
    let block_count = module_count.iter().chain(hull_count.iter())
        .filter(|parent| parent.parent() == ship)
        .count();
    if block_count + clipboard.copied.len() > crate::combat::limits::MAX_SHIP_BLOCKS {
        notifications.write(ShowNotification {
            message: format!(
                "Paste would exceed block limit ({}/{})",
                block_count + clipboard.copied.len(),
                crate::combat::limits::MAX_SHIP_BLOCKS
            ),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
        return;
    }

    // Check every cell of every module's footprint, not just origins —
    // origin-only let 2x1 modules paste half-overlapped
    let all_free = clipboard.copied.iter().all(|entry| {
        entry_cells(entry, grid_pos, &registry)
            .iter()
            .all(|cell| !occupancy.cells.contains_key(cell))
    });

    if !all_free {
        notifications.write(ShowNotification {
            message: "Cannot paste - some positions are occupied".into(),
            notification_type: NotificationType::Warning,
            duration: 2.0,
        });
        return;
    }

    // Place all modules
    let mut placed = 0u32;
    for entry in &clipboard.copied {
        let target = grid_pos + entry.offset;
        place_events.write(PlaceModuleRequest {
            module_type: entry.module_type,
            grid_position: target,
            rotation: entry.rotation,
            custom_name: None,
            subcomponents: None,
            extras: None,
            free: false,
        });
        placed += 1;
    }

    clipboard.paste_mode = false;
    notifications.write(ShowNotification {
        message: format!("Pasted {} modules (-{}c)", placed, total_cost),
        notification_type: NotificationType::Success,
        duration: 2.0,
    });
}

/// Marker for paste ghost preview sprites
#[derive(Component)]
pub struct PasteGhostBlock;

/// Show ghost preview during paste mode
pub fn paste_ghost_preview(
    mut commands: Commands,
    clipboard: Res<BuildClipboard>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera>>,
    ship_query: Query<(Entity, &GlobalTransform), (With<Ship>, Without<Camera>)>,
    occupancy: Res<GridOccupancy>,
    registry: Res<ModuleRegistry>,
    existing_ghosts: Query<Entity, With<PasteGhostBlock>>,
) {
    // Despawn old ghosts
    for entity in existing_ghosts.iter() {
        commands.entity(entity).despawn();
    }

    if !clipboard.paste_mode || clipboard.copied.is_empty() { return; }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };
    let Ok((ship, ship_gt)) = ship_query.single() else { return };
    let Some(grid_pos) = cursor_to_ship_grid(window, camera, camera_transform, ship_gt)
    else { return };

    for entry in &clipboard.copied {
        for cell in entry_cells(entry, grid_pos, &registry) {
            let color = if occupancy.cells.contains_key(&cell) {
                Color::srgba(0.8, 0.2, 0.2, 0.3)
            } else {
                Color::srgba(0.3, 0.7, 1.0, 0.3)
            };

            commands.spawn((
                (Sprite {
                        color,
                        custom_size: Some(Vec2::splat(60.0)),
                        ..default()
                    }, Transform::from_xyz(
                        cell.x as f32 * 66.0,
                        cell.y as f32 * 66.0 - 33.0,
                        0.95,
                    )),
                PasteGhostBlock,
                ChildOf(ship),
            ));
        }
    }
}
