use bevy::prelude::*;
use crate::resources::*;
use crate::events::*;
use crate::states::BuildState;
use crate::building::GridOccupancy;
use crate::building::multiblock::build_helpers::weapon_templates;

// ============================================================================
// WEAPON TEMPLATE PLACEMENT
// Press T during build mode to cycle through templates.
// Press Enter to place the selected template at the ghost position.
// ============================================================================

/// Resource tracking current template selection
#[derive(Resource, Default)]
pub struct TemplateState {
    pub selected_index: Option<usize>,
}

/// System: T cycles templates, Enter places them
pub fn template_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut template_state: ResMut<TemplateState>,
    build_state: Res<BuildingState>,
    occupancy: Res<GridOccupancy>,
    currency: ResMut<Currency>,
    registry: Res<crate::building::ModuleRegistry>,
    mut place_events: MessageWriter<PlaceModuleRequest>,
    mut notifications: MessageWriter<ShowNotification>,
    current_build_state: Res<State<BuildState>>,
    // Saved gun designs follow the built-in templates in the cycle
    // (building::gun_stamp places them).
    gun_designs: Res<crate::combat::gun_design::GunDesigns>,
) {
    if *current_build_state.get() == BuildState::Inactive { return; }

    let templates = weapon_templates();
    let total = templates.len() + gun_designs.0.len();
    if total == 0 { return; }

    // T = cycle through templates, then saved gun designs
    if keyboard.just_pressed(KeyCode::KeyT) {
        let next = match template_state.selected_index {
            Some(i) => {
                if i + 1 >= total { None } else { Some(i + 1) }
            }
            None => Some(0),
        };
        template_state.selected_index = next;

        if let Some(design) = next.and_then(|i| i.checked_sub(templates.len())).and_then(|i| gun_designs.0.get(i)) {
            notifications.write(ShowNotification {
                message: format!("Gun design: {} ({}c) - {}. R turns it, Enter places it.", design.name, design.cost(&registry), design.summary()),
                notification_type: NotificationType::Info,
                duration: 4.0,
            });
        } else if let Some(idx) = next {
            let t = &templates[idx];
            notifications.write(ShowNotification {
                message: format!("Template: {} ({}c) - {}", t.name, t.cost(&registry), t.description),
                notification_type: NotificationType::Info,
                duration: 3.0,
            });
        } else {
            notifications.write(ShowNotification {
                message: "Templates off".into(),
                notification_type: NotificationType::Info,
                duration: 1.5,
            });
        }
    }

    // Enter = place selected template at ghost position
    if keyboard.just_pressed(KeyCode::Enter) {
        let Some(idx) = template_state.selected_index else { return };
        // A saved gun design: building::gun_stamp's.
        let Some(template) = templates.get(idx) else { return };
        let cost = template.cost(&registry);

        let origin = build_state.ghost_position;
        let rotation = build_state.rotation;

        // Check affordability
        if currency.credits < cost {
            notifications.write(ShowNotification {
                message: format!("Not enough credits! Need {}c", cost),
                notification_type: NotificationType::Warning,
                duration: 2.0,
            });
            return;
        }

        // Check all positions are free
        let all_free = std::iter::once(IVec2::ZERO)
            .chain(template.blocks.iter().map(|(_, offset)| *offset))
            .all(|offset| {
                let pos = origin + offset;
                !occupancy.cells.contains_key(&pos)
            });

        if !all_free {
            notifications.write(ShowNotification {
                message: "Cannot place template - positions occupied".into(),
                notification_type: NotificationType::Warning,
                duration: 2.0,
            });
            return;
        }

        // Place core
        place_events.write(PlaceModuleRequest {
            module_type: template.core,
            grid_position: origin,
            rotation,
            custom_name: None,
            subcomponents: None,
            extras: None,
            free: false,
        });

        // Place extension blocks
        for (block_type, offset) in &template.blocks {
            place_events.write(PlaceModuleRequest {
                module_type: *block_type,
                grid_position: origin + *offset,
                rotation,
                custom_name: None,
                subcomponents: None,
            extras: None,
                free: false,
            });
        }

        notifications.write(ShowNotification {
            message: format!("Placed {} (-{}c)", template.name, cost),
            notification_type: NotificationType::Success,
            duration: 2.0,
        });
    }
}
