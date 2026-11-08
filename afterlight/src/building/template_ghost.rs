use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::states::BuildState;
use crate::building::GridOccupancy;
use crate::building::multiblock::build_helpers::weapon_templates;
use crate::building::templates::TemplateState;

// ============================================================================
// TEMPLATE GHOST PREVIEW + CHAIN DELETE
// ============================================================================

/// Marker for template ghost preview sprites
#[derive(Component)]
pub struct TemplateGhostBlock;

/// Show ghost sprites for the selected template at the cursor position
pub fn update_template_ghost(
    mut commands: Commands,
    template_state: Res<TemplateState>,
    build_state: Res<BuildingState>,
    occupancy: Res<GridOccupancy>,
    existing_ghosts: Query<Entity, With<TemplateGhostBlock>>,
    current_state: Res<State<BuildState>>,
) {
    // Despawn old ghosts
    for entity in existing_ghosts.iter() {
        commands.entity(entity).despawn();
    }

    if *current_state.get() != BuildState::Placing { return; }
    let Some(idx) = template_state.selected_index else { return; };

    let templates = weapon_templates();
    if idx >= templates.len() { return; }

    let template = &templates[idx];
    let origin = build_state.ghost_position;
    let rotation = build_state.rotation;

    // Spawn ghost for core
    let core_valid = !occupancy.cells.contains_key(&origin);
    spawn_ghost_block(&mut commands, origin, if core_valid { Color::srgba(0.3, 0.8, 0.3, 0.3) } else { Color::srgba(0.8, 0.2, 0.2, 0.3) });

    // Spawn ghosts for extensions — rotate offsets based on build rotation
    for (_, offset) in &template.blocks {
        let rotated_offset = rotate_offset(*offset, rotation);
        let pos = origin + rotated_offset;
        let color = if occupancy.cells.contains_key(&pos) {
            Color::srgba(0.8, 0.2, 0.2, 0.3)
        } else {
            Color::srgba(0.3, 0.8, 0.3, 0.3)
        };
        spawn_ghost_block(&mut commands, pos, color);
    }
}

/// Rotate a grid offset based on Rotation (template assumes East-facing by default)
fn rotate_offset(offset: IVec2, rotation: Rotation) -> IVec2 {
    match rotation {
        Rotation::East => offset,                             // Default — no change
        Rotation::North => IVec2::new(-offset.y, offset.x),  // 90° CCW
        Rotation::West => IVec2::new(-offset.x, -offset.y),  // 180°
        Rotation::South => IVec2::new(offset.y, -offset.x),  // 90° CW
    }
}

fn spawn_ghost_block(commands: &mut Commands, grid_pos: IVec2, color: Color) {
    let world_x = grid_pos.x as f32 * 66.0;
    let world_y = grid_pos.y as f32 * 66.0 - 33.0;

    commands.spawn((
        (Sprite {
                color,
                custom_size: Some(Vec2::splat(60.0)),
                ..default()
            }, Transform::from_xyz(world_x, world_y, 0.95)),
        TemplateGhostBlock,
    ));
}

/// When deleting a weapon core, auto-delete all connected extension blocks
pub fn chain_delete_system(
    mut commands: Commands,
    mut removed_modules: RemovedComponents<Module>,
    block_query: Query<(Entity, &crate::building::multiblock::components::MachineBlock, &Module)>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    for removed_entity in removed_modules.read() {
        // Check if any blocks were connected to this entity as their core
        let mut to_remove: Vec<Entity> = Vec::new();
        for (entity, block, _module) in block_query.iter() {
            if block.connected_core == Some(removed_entity) {
                to_remove.push(entity);
            }
        }

        if !to_remove.is_empty() {
            let count = to_remove.len();
            for entity in to_remove {
                commands.entity(entity).despawn();
            }
            notifications.write(crate::events::ShowNotification {
                message: format!("Chain deleted: {} connected blocks removed", count),
                notification_type: crate::events::NotificationType::Info,
                duration: 2.0,
            });
        }
    }
}
