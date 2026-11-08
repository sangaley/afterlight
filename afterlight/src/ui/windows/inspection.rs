use bevy::prelude::*;
use crate::components::ModuleType;
use crate::building::customization::parameters::*;
use super::framework::*;
use super::tooltip::Tooltip;

// ============================================================================
// MODULE INSPECTION WINDOW (Tier 2)
// Right-click a module → floating window showing ship-component slots.
// Click a slot → dropdown to swap ship-component type.
// Stats update in real-time.
// ============================================================================

/// Marker for the module inspection window
#[derive(Component)]
pub struct InspectionWindow {
    pub module_entity: Entity,
    pub module_type: ModuleType,
}

/// Marker for a ship-component slot button in the inspection window
#[derive(Component)]
pub struct SlotButton {
    pub slot_name: String,
    pub window_id: String,
}

/// Marker for the slot options dropdown
#[derive(Component)]
pub struct SlotDropdown {
    pub slot_name: String,
}

/// Marker for an individual option in the dropdown
#[derive(Component)]
pub struct SlotOptionButton {
    pub slot_name: String,
    pub option_index: usize,
}

/// Marker for the "Customize" button that opens Tier 3
#[derive(Component)]
pub struct CustomizeButton {
    pub slot_name: String,
    pub module_entity: Entity,
}

/// Marker for a preset button
#[derive(Component)]
pub struct PresetButton {
    pub preset_index: usize,
    pub module_entity: Entity,
}

/// Marker for a player-saved custom preset button ("My Builds")
#[derive(Component)]
pub struct CustomPresetButton {
    pub preset_index: usize,
    pub module_entity: Entity,
}

/// Marker for the "Save Current Build" button
#[derive(Component)]
pub struct SaveBuildButton {
    pub module_entity: Entity,
}

/// Structural risk info for a multiblock barrel chain, computed by the caller
/// (either the inspected block's own stress, or the worst joint in a core's chain).
pub struct StructuralInfo {
    pub chain_length: u32,
    pub worst_cascade_chance: f32,
    pub cascade_damage: f32,
}

/// Renders the "Structural Integrity" section — makes the cascade-explosion
/// risk from stacking barrel-extension blocks visible before it bites you.
fn spawn_structural_section(commands: &mut Commands, content: Entity, info: &StructuralInfo) {
    spawn_window_section(commands, content, "Structural Integrity");

    let pct = (info.worst_cascade_chance * 100.0).round() as i32;
    let (risk_label, risk_color) = if info.worst_cascade_chance >= 0.5 {
        ("CRITICAL", Color::srgb(0.9, 0.25, 0.2))
    } else if info.worst_cascade_chance >= 0.25 {
        ("STRAINED", Color::srgb(0.9, 0.75, 0.2))
    } else {
        ("STABLE", Color::srgb(0.35, 0.85, 0.4))
    };

    let risk_row = commands.spawn(
        (Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            }, BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.8))),
    ).id();

    let status_text = commands.spawn(
        (Text::new(format!("{} - {}% cascade chance on hit", risk_label, pct)), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(risk_color)),
    ).id();

    let detail_text = commands.spawn(
        (Text::new(format!(
            "{} barrel segment(s) chained. The most-stressed joint would spread {:.0} damage to the next segment if destroyed.",
            info.chain_length, info.cascade_damage
        )), TextFont { font_size: FontSize::Px(10.0), ..default() }, TextColor(WindowStyle::TEXT_DIM)),
    ).id();

    commands.entity(risk_row).add_children(&[status_text, detail_text]);

    if info.worst_cascade_chance >= 0.25 {
        let tip_text = commands.spawn(
            (Text::new("Tip: a Reinforced Joint or Recoil Absorber placed adjacent lowers this."), TextFont { font_size: FontSize::Px(10.0), ..default() }, TextColor(Color::srgb(0.5, 0.7, 1.0))),
        ).id();
        commands.entity(risk_row).add_child(tip_text);
    }

    commands.entity(content).add_child(risk_row);
}

/// Spawn an inspection window for a specific module
pub fn spawn_inspection_window(
    commands: &mut Commands,
    module_entity: Entity,
    module_type: ModuleType,
    customization: Option<&ModuleCustomization>,
    registry: &CustomizationRegistry,
    structural: Option<StructuralInfo>,
    custom_presets: &[crate::building::customization::presets::Preset],
    position: Vec2,
) {
    let window_id = format!("inspect_{:?}", module_entity);
    let title = format!("{} - Configuration", module_type.name());

    let content = spawn_floating_window(
        commands,
        &window_id,
        &title,
        Vec2::new(320.0, 400.0),
        position,
    );

    commands.entity(content).insert(InspectionWindow {
        module_entity,
        module_type,
    });

    // Structural integrity — cascade risk from stacking barrel-extension blocks
    if let Some(info) = &structural {
        spawn_structural_section(commands, content, info);
    }

    // Get customization definition
    let custom_key = format!("weapon_{:?}", module_type).to_lowercase();
    let Some(def) = registry.get(&custom_key) else {
        // No customization available — show basic info
        spawn_window_section(commands, content, "No customization available");
        let info = commands.spawn(
            (Text::new("This module uses default configuration."), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(WindowStyle::TEXT_DIM)),
        ).id();
        commands.entity(content).add_child(info);
        return;
    };

    // Header: module name and type
    spawn_window_section(commands, content, "Sub-Components");

    // Show each slot with current selection
    for slot_def in &def.slots {
        let current_selection = customization
            .and_then(|c| c.slot_selections.get(&slot_def.slot_name))
            .copied()
            .unwrap_or(slot_def.default_option);

        let current_option = &slot_def.options[current_selection];

        // Slot row: [Slot Name] [Current Option ▼] [Customize →]
        let slot_row = commands.spawn(
            (Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(4.0)),
                    margin: UiRect::vertical(Val::Px(2.0)),
                    ..default()
                }, BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.8))),
        ).id();

        // Slot name label
        let slot_label = commands.spawn(
            (Text::new(&slot_def.slot_name), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(WindowStyle::TEXT_DIM)),
        ).id();

        // Button row
        let button_row = commands.spawn(
            (Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    ..default()
                }),
        ).id();

        // Current selection button (click to open dropdown)
        let selection_btn = commands.spawn((
            (Node {
                    flex_grow: 1.0,
                    padding: UiRect::all(Val::Px(4.0)),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Row,
                    ..default()
                }, BackgroundColor(Color::srgba(0.10, 0.13, 0.20, 1.0))),
            SlotButton {
                slot_name: slot_def.slot_name.clone(),
                window_id: window_id.clone(),
            },
            Tooltip {
                text: current_option.description.clone(),
                detail: Some(slot_def.description.clone()),
            },
        )).id();

        let selection_text = commands.spawn(
            (Text::new(&current_option.name), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(WindowStyle::TEXT_COLOR)),
        ).id();

        let dropdown_arrow = commands.spawn(
            (Text::new("v"), TextFont { font_size: FontSize::Px(10.0), ..default() }, TextColor(WindowStyle::TEXT_DIM)),
        ).id();

        commands.entity(selection_btn).add_children(&[selection_text, dropdown_arrow]);

        // Customize button (opens Tier 3)
        if !current_option.parameters.is_empty() {
            let customize_btn = commands.spawn((
                (Node {
                        padding: UiRect::horizontal(Val::Px(8.0)),
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    }, BackgroundColor(Color::srgba(0.15, 0.20, 0.35, 1.0))),
                CustomizeButton {
                    slot_name: slot_def.slot_name.clone(),
                    module_entity,
                },
                Tooltip {
                    text: format!("Deep customize {} - {} parameters", slot_def.slot_name, current_option.parameters.len()),
                    detail: None,
                },
            )).id();

            let customize_text = commands.spawn(
                (Text::new("*"), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.5, 0.7, 1.0))),
            ).id();

            commands.entity(customize_btn).add_child(customize_text);
            commands.entity(button_row).add_children(&[selection_btn, customize_btn]);
        } else {
            commands.entity(button_row).add_child(selection_btn);
        }

        // Stat modifiers preview
        if !current_option.stat_modifiers.is_empty() {
            let stats_row = commands.spawn(
                (Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(8.0),
                        ..default()
                    }),
            ).id();

            for (stat, modifier) in &current_option.stat_modifiers {
                let color = if *modifier > 1.0 {
                    Color::srgb(0.3, 0.8, 0.4) // Green = buff
                } else if *modifier < 1.0 {
                    Color::srgb(0.8, 0.4, 0.3) // Red = nerf
                } else {
                    WindowStyle::TEXT_DIM
                };

                let sign = if *modifier > 1.0 { "+" } else { "" };
                let pct = ((*modifier - 1.0) * 100.0) as i32;

                let stat_text = commands.spawn(
                    (Text::new(format!("{}{:}% {}", sign, pct, stat)), TextFont { font_size: FontSize::Px(10.0), ..default() }, TextColor(color)),
                ).id();
                commands.entity(stats_row).add_child(stat_text);
            }

            commands.entity(slot_row).add_children(&[slot_label, button_row, stats_row]);
        } else {
            commands.entity(slot_row).add_children(&[slot_label, button_row]);
        }

        commands.entity(content).add_child(slot_row);
    }

    // Presets section — built-in curated presets, plus the player's own saved builds.
    // This is where a beginner's one-click default and a pro's tuned monstrosity live
    // side by side.
    let presets = crate::building::customization::presets::presets_for(module_type);

    let sep = commands.spawn(
        (Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(6.0)),
                ..default()
            }, BackgroundColor(WindowStyle::BORDER_COLOR)),
    ).id();
    commands.entity(content).add_child(sep);

    if !presets.is_empty() {
        spawn_window_section(commands, content, "Quick Presets");

        let presets_row = commands.spawn(
            (Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                }),
        ).id();

        for (i, preset) in presets.iter().enumerate() {
            let btn = commands.spawn((
                (Node {
                        padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(3.0), Val::Px(3.0)),
                        ..default()
                    }, BackgroundColor(Color::srgba(0.12, 0.15, 0.22, 1.0))),
                PresetButton {
                    preset_index: i,
                    module_entity,
                },
                Tooltip {
                    text: preset.description.clone(),
                    detail: None,
                },
            )).id();

            let text = commands.spawn(
                (Text::new(preset.name.clone()), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(WindowStyle::TEXT_COLOR)),
            ).id();

            commands.entity(btn).add_child(text);
            commands.entity(presets_row).add_child(btn);
        }

        commands.entity(content).add_child(presets_row);
    }

    // My Builds — the player's own saved configurations for this weapon family
    if !custom_presets.is_empty() {
        spawn_window_section(commands, content, "My Builds");

        let custom_row = commands.spawn(
            (Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                }),
        ).id();

        for (i, preset) in custom_presets.iter().enumerate() {
            let btn = commands.spawn((
                (Node {
                        padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(3.0), Val::Px(3.0)),
                        ..default()
                    }, BackgroundColor(Color::srgba(0.14, 0.20, 0.14, 1.0))),
                CustomPresetButton {
                    preset_index: i,
                    module_entity,
                },
                Tooltip {
                    text: preset.description.clone(),
                    detail: None,
                },
            )).id();

            let text = commands.spawn(
                (Text::new(preset.name.clone()), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(WindowStyle::TEXT_COLOR)),
            ).id();

            commands.entity(btn).add_child(text);
            commands.entity(custom_row).add_child(btn);
        }

        commands.entity(content).add_child(custom_row);
    }

    // Save current configuration as a new named build
    let save_btn = commands.spawn((
        (Node {
                padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(4.0), Val::Px(4.0)),
                margin: UiRect::top(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                ..default()
            }, BackgroundColor(Color::srgba(0.16, 0.14, 0.22, 1.0))),
        SaveBuildButton { module_entity },
        Tooltip {
            text: "Save the current slot picks and parameters as a reusable build".into(),
            detail: None,
        },
    )).id();

    let save_text = commands.spawn(
        (Text::new("+ Save Current Build"), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(Color::srgb(0.7, 0.6, 0.9))),
    ).id();

    commands.entity(save_btn).add_child(save_text);
    commands.entity(content).add_child(save_btn);
}

/// System: handle slot button click — cycle to next option
pub fn slot_button_click(
    mut buttons: Query<(&SlotButton, &Interaction), Changed<Interaction>>,
    inspection_windows: Query<&InspectionWindow>,
    mut customization_query: Query<&mut crate::building::customization::parameters::ModuleCustomization>,
    registry: Res<crate::building::customization::parameters::CustomizationRegistry>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    for (slot_btn, interaction) in buttons.iter_mut() {
        if *interaction != Interaction::Pressed { continue; }

        // Find the inspection window this slot belongs to
        for window in inspection_windows.iter() {
            // Get the customization def for this module
            let key = format!("weapon_{:?}", window.module_type).to_lowercase();
            let Some(def) = registry.get(&key) else { continue };

            // Find the slot definition
            let Some(slot_def) = def.slots.iter().find(|s| s.slot_name == slot_btn.slot_name) else { continue };

            // Get or create customization component
            if let Ok(mut customization) = customization_query.get_mut(window.module_entity) {
                let current = customization.slot_selections
                    .get(&slot_btn.slot_name)
                    .copied()
                    .unwrap_or(slot_def.default_option);

                // Cycle to next option
                let next = (current + 1) % slot_def.options.len();
                customization.slot_selections.insert(slot_btn.slot_name.clone(), next);

                let option_name = &slot_def.options[next].name;
                notifications.write(crate::events::ShowNotification {
                    message: format!("{}: {} - close and reopen to see updated stats", slot_btn.slot_name, option_name),
                    notification_type: crate::events::NotificationType::Info,
                    duration: 2.0,
                });

                // Mark window for rebuild by despawning it (player will right-click to reopen)
                // A proper solution would rebuild inline, but this works for now
            }
        }
    }
}

/// System: handle slot button hover
pub fn slot_button_hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (With<SlotButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        *bg = match interaction {
            Interaction::Hovered => Color::srgba(0.15, 0.18, 0.28, 1.0).into(),
            Interaction::Pressed => Color::srgba(0.20, 0.24, 0.35, 1.0).into(),
            Interaction::None => Color::srgba(0.10, 0.13, 0.20, 1.0).into(),
        };
    }
}

/// System: handle customize button hover
pub fn customize_button_hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (With<CustomizeButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        *bg = match interaction {
            Interaction::Hovered => Color::srgba(0.20, 0.28, 0.45, 1.0).into(),
            Interaction::Pressed => Color::srgba(0.25, 0.35, 0.55, 1.0).into(),
            Interaction::None => Color::srgba(0.15, 0.20, 0.35, 1.0).into(),
        };
    }
}

/// System: handle preset button click — apply the preset to the module's customization
pub fn preset_button_click(
    mut buttons: Query<(&PresetButton, &Interaction), Changed<Interaction>>,
    inspection_windows: Query<&InspectionWindow>,
    mut customization_query: Query<&mut crate::building::customization::parameters::ModuleCustomization>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    for (preset_btn, interaction) in buttons.iter_mut() {
        if *interaction != Interaction::Pressed { continue; }

        // Find the inspection window this preset belongs to
        for window in inspection_windows.iter() {
            if window.module_entity != preset_btn.module_entity { continue; }

            let presets = crate::building::customization::presets::presets_for(window.module_type);
            let Some(preset) = presets.get(preset_btn.preset_index) else { continue };

            if let Ok(mut customization) = customization_query.get_mut(preset_btn.module_entity) {
                preset.apply(&mut customization);

                notifications.write(crate::events::ShowNotification {
                    message: format!("Preset applied: {} - close and reopen to see updated stats", preset.name),
                    notification_type: crate::events::NotificationType::Info,
                    duration: 2.0,
                });
            }
        }
    }
}

/// System: handle preset button hover
pub fn preset_button_hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (With<PresetButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        *bg = match interaction {
            Interaction::Hovered => Color::srgba(0.18, 0.22, 0.30, 1.0).into(),
            Interaction::Pressed => Color::srgba(0.22, 0.28, 0.38, 1.0).into(),
            Interaction::None => Color::srgba(0.12, 0.15, 0.22, 1.0).into(),
        };
    }
}

/// System: handle custom (player-saved) preset button click — apply it to the module
pub fn custom_preset_button_click(
    mut buttons: Query<(&CustomPresetButton, &Interaction), Changed<Interaction>>,
    inspection_windows: Query<&InspectionWindow>,
    mut customization_query: Query<&mut crate::building::customization::parameters::ModuleCustomization>,
    library: Res<crate::building::customization::custom_presets::CustomPresetLibrary>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    for (preset_btn, interaction) in buttons.iter_mut() {
        if *interaction != Interaction::Pressed { continue; }

        for window in inspection_windows.iter() {
            if window.module_entity != preset_btn.module_entity { continue; }

            let key = format!("weapon_{:?}", window.module_type).to_lowercase();
            let Some(saved) = library.presets.get(&key) else { continue };
            let Some(preset) = saved.get(preset_btn.preset_index) else { continue };

            if let Ok(mut customization) = customization_query.get_mut(preset_btn.module_entity) {
                preset.apply(&mut customization);

                notifications.write(crate::events::ShowNotification {
                    message: format!("Build applied: {} - close and reopen to see updated stats", preset.name),
                    notification_type: crate::events::NotificationType::Info,
                    duration: 2.0,
                });
            }
        }
    }
}

/// System: handle custom preset button hover
pub fn custom_preset_button_hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (With<CustomPresetButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        *bg = match interaction {
            Interaction::Hovered => Color::srgba(0.20, 0.28, 0.20, 1.0).into(),
            Interaction::Pressed => Color::srgba(0.26, 0.36, 0.26, 1.0).into(),
            Interaction::None => Color::srgba(0.14, 0.20, 0.14, 1.0).into(),
        };
    }
}

/// System: handle "Save Current Build" click — snapshot the module's current
/// customization into the custom preset library and persist it to disk.
pub fn save_build_button_click(
    mut buttons: Query<(&SaveBuildButton, &Interaction), Changed<Interaction>>,
    inspection_windows: Query<&InspectionWindow>,
    customization_query: Query<&crate::building::customization::parameters::ModuleCustomization>,
    mut library: ResMut<crate::building::customization::custom_presets::CustomPresetLibrary>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    for (save_btn, interaction) in buttons.iter_mut() {
        if *interaction != Interaction::Pressed { continue; }

        for window in inspection_windows.iter() {
            if window.module_entity != save_btn.module_entity { continue; }

            let Ok(customization) = customization_query.get(save_btn.module_entity) else { continue };
            let key = format!("weapon_{:?}", window.module_type).to_lowercase();
            let name = library.next_build_name(&key);

            let preset = crate::building::customization::presets::Preset {
                name: name.clone(),
                description: "Player-saved configuration.".into(),
                slot_selections: customization.slot_selections.clone(),
                parameter_values: customization.parameter_values.clone(),
            };

            library.presets.entry(key).or_default().push(preset);
            crate::building::customization::custom_presets::save_custom_presets(&library);

            notifications.write(crate::events::ShowNotification {
                message: format!("Saved as \"{}\" - close and reopen to see it under My Builds", name),
                notification_type: crate::events::NotificationType::Success,
                duration: 2.5,
            });
        }
    }
}

/// System: handle save-build button hover
pub fn save_build_button_hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (With<SaveBuildButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        *bg = match interaction {
            Interaction::Hovered => Color::srgba(0.22, 0.19, 0.30, 1.0).into(),
            Interaction::Pressed => Color::srgba(0.28, 0.24, 0.38, 1.0).into(),
            Interaction::None => Color::srgba(0.16, 0.14, 0.22, 1.0).into(),
        };
    }
}
