use bevy::prelude::*;
use crate::components::*;
use crate::events::*;
use crate::resources::*;

// ============================================================================
// UNDO SYSTEM
// Placements (modules + hull) are recorded by the placement processors in
// building/mod.rs. Ctrl+Z removes the most recent placement and refunds its
// full cost.
//
// There is deliberately no redo, and removals are not undoable: respawning a
// removed module faithfully would need a snapshot of all its component state
// (ammo, heat, tuning, machine connections...). The old redo only moved
// credits around without spawning anything — worse than nothing.
// ============================================================================

/// A single recorded placement that can be undone
#[derive(Clone, Debug)]
pub enum BuildAction {
    PlaceModule {
        entity: Entity,
        module_type: ModuleType,
        cost: u32,
    },
    PlaceHull {
        entity: Entity,
        material: HullMaterial,
        cost: u32,
    },
}

/// Resource tracking recent placements
#[derive(Resource)]
pub struct BuildHistory {
    pub undo_stack: Vec<BuildAction>,
    pub max_history: usize,
}

impl Default for BuildHistory {
    fn default() -> Self {
        Self {
            undo_stack: Vec::new(),
            max_history: 50,
        }
    }
}

impl BuildHistory {
    pub fn record(&mut self, action: BuildAction) {
        self.undo_stack.push(action);
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
    }
}

/// System: Ctrl+Z removes the last placed block and refunds its cost
pub fn undo_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut history: ResMut<BuildHistory>,
    mut commands: Commands,
    mut currency: ResMut<Currency>,
    module_query: Query<&Module>,
    all_entities: Query<Entity>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !ctrl || !keyboard.just_pressed(KeyCode::KeyZ) {
        return;
    }

    // Entries whose entity is already gone (deleted in delete mode, blown up,
    // wiped by a blueprint load) are meaningless — skip past them to the most
    // recent placement that still exists.
    let action = loop {
        match history.undo_stack.pop() {
            Some(action) => {
                let entity = match &action {
                    BuildAction::PlaceModule { entity, .. } => *entity,
                    BuildAction::PlaceHull { entity, .. } => *entity,
                };
                if all_entities.contains(entity) {
                    break Some(action);
                }
            }
            None => break None,
        }
    };

    let Some(action) = action else {
        notifications.write(ShowNotification {
            message: "Nothing to undo".into(),
            notification_type: NotificationType::Info,
            duration: 1.5,
        });
        return;
    };

    match action {
        BuildAction::PlaceModule { entity, module_type, cost } => {
            // Same guard as delete mode: never undo away the last power source
            if module_type.category() == ModuleCategory::Power {
                let power_count = module_query.iter()
                    .filter(|m| m.module_type.category() == ModuleCategory::Power)
                    .count();
                if power_count <= 1 {
                    history.undo_stack.push(action);
                    notifications.write(ShowNotification {
                        message: "Cannot undo - last power source".into(),
                        notification_type: NotificationType::Warning,
                        duration: 2.0,
                    });
                    return;
                }
            }
            commands.entity(entity).despawn();
            currency.credits += cost;
            notifications.write(ShowNotification {
                message: format!("Undo: removed {} (+{}c)", module_type.name(), cost),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
        }
        BuildAction::PlaceHull { entity, material, cost } => {
            commands.entity(entity).despawn();
            currency.credits += cost;
            notifications.write(ShowNotification {
                message: format!("Undo: removed {} hull (+{}c)", material.name(), cost),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
        }
    }
}
