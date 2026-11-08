use bevy::prelude::*;
use crate::components::*;
use crate::ui::windows::inspection::{InspectionWindow, spawn_inspection_window, CustomizeButton, StructuralInfo};
use crate::ui::windows::customization::spawn_deep_customization_window;
use super::customization::parameters::{CustomizationRegistry, ModuleCustomization};
use super::customization::custom_presets::CustomPresetLibrary;
use super::multiblock::components::{MachineBlock, BarrelStress, CascadeRisk, BlockRole};
use super::GridOccupancy;

/// Right-click a placed module → open inspection floating window
pub fn right_click_inspect(
    mouse: Res<ButtonInput<MouseButton>>,
    windows_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    module_query: Query<(Entity, &Module, Option<&ModuleCustomization>)>,
    machine_query: Query<(&MachineBlock, Option<&BarrelStress>, Option<&CascadeRisk>)>,
    occupancy: Res<GridOccupancy>,
    registry: Res<CustomizationRegistry>,
    custom_presets: Res<CustomPresetLibrary>,
    existing_inspections: Query<&InspectionWindow>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }

    let Ok(window) = windows_query.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };

    let Some(cursor_pos) = window.cursor_position() else { return };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else { return };

    // Convert world pos to grid pos
    let grid_x = (world_pos.x / 66.0).round() as i32;
    let grid_y = ((world_pos.y + 33.0) / 66.0).round() as i32;
    let grid_pos = IVec2::new(grid_x, grid_y);

    // Check if there's a module at this grid position
    let Some(&module_entity) = occupancy.cells.get(&grid_pos) else { return };
    let Ok((entity, module, customization)) = module_query.get(module_entity) else { return };

    // Don't open duplicate inspection windows for the same module
    for existing in existing_inspections.iter() {
        if existing.module_entity == entity {
            return;
        }
    }

    let structural = structural_info_for(entity, &machine_query);

    let custom_key = format!("weapon_{:?}", module.module_type).to_lowercase();
    let empty_presets: Vec<crate::building::customization::presets::Preset> = Vec::new();
    let saved_builds = custom_presets.presets.get(&custom_key).unwrap_or(&empty_presets);

    // Spawn inspection window at cursor position (offset so it doesn't cover the module)
    spawn_inspection_window(
        &mut commands,
        entity,
        module.module_type,
        customization,
        &registry,
        structural,
        saved_builds,
        Vec2::new(cursor_pos.x + 20.0, cursor_pos.y.max(50.0)),
    );
}

/// Computes the cascade-risk summary to show for an inspected module:
/// - A barrel-extension block reports its own joint stress.
/// - A weapon core reports the worst (most-stressed) joint across its whole barrel chain.
/// Returns None for modules with no barrel chain attached (nothing to warn about).
fn structural_info_for(
    entity: Entity,
    machine_query: &Query<(&MachineBlock, Option<&BarrelStress>, Option<&CascadeRisk>)>,
) -> Option<StructuralInfo> {
    let (block, stress, cascade) = machine_query.get(entity).ok()?;

    match block.role {
        BlockRole::Barrel => {
            let stress = stress?;
            let cascade_damage = cascade.map(|c| c.cascade_damage).unwrap_or(35.0);
            Some(StructuralInfo {
                chain_length: stress.load,
                worst_cascade_chance: stress.effective_cascade_chance,
                cascade_damage,
            })
        }
        BlockRole::Core => {
            let mut chain_length = 0u32;
            let mut worst_chance = 0.0f32;
            let mut worst_damage = 0.0f32;

            for (other_block, other_stress, other_cascade) in machine_query.iter() {
                if other_block.role != BlockRole::Barrel || other_block.connected_core != Some(entity) {
                    continue;
                }
                chain_length += 1;
                if let Some(s) = other_stress {
                    if s.effective_cascade_chance >= worst_chance {
                        worst_chance = s.effective_cascade_chance;
                        worst_damage = other_cascade.map(|c| c.cascade_damage).unwrap_or(35.0);
                    }
                }
            }

            if chain_length == 0 {
                None
            } else {
                Some(StructuralInfo {
                    chain_length,
                    worst_cascade_chance: worst_chance,
                    cascade_damage: worst_damage,
                })
            }
        }
        _ => None,
    }
}

/// When a CustomizeButton is clicked, open the Tier 3 deep customization window
pub fn handle_customize_click(
    buttons: Query<(&CustomizeButton, &Interaction), Changed<Interaction>>,
    module_query: Query<(&Module, Option<&ModuleCustomization>)>,
    registry: Res<CustomizationRegistry>,
    mut commands: Commands,
) {
    for (btn, interaction) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let Ok((module, customization)) = module_query.get(btn.module_entity) else { continue };

        let custom_key = format!("weapon_{:?}", module.module_type).to_lowercase();
        let Some(def) = registry.get(&custom_key) else { continue };

        // Find the slot and current option
        let Some(slot_def) = def.slots.iter().find(|s| s.slot_name == btn.slot_name) else { continue };
        let selection = customization
            .and_then(|c| c.slot_selections.get(&btn.slot_name))
            .copied()
            .unwrap_or(slot_def.default_option);
        let option = &slot_def.options[selection];

        spawn_deep_customization_window(
            &mut commands,
            btn.module_entity,
            &btn.slot_name,
            option,
            customization,
            Vec2::new(400.0, 100.0),
        );
    }
}
