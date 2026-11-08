use bevy::prelude::*;
use crate::building::customization::parameters::*;
use super::framework::*;
use super::tooltip::Tooltip;

// ============================================================================
// DEEP CUSTOMIZATION WINDOW (Tier 3)
// Click a ship-component → floating window with parameter sliders.
// Green zone indicators, live stat preview, presets, undo, warnings.
// ============================================================================

/// Marker for the deep customization window
#[derive(Component)]
pub struct DeepCustomizationWindow {
    pub module_entity: Entity,
    pub slot_name: String,
}

/// A single parameter slider row
#[derive(Component)]
pub struct ParameterSlider {
    pub param_key: String,
    pub min: f32,
    pub max: f32,
    pub optimal_min: f32,
    pub optimal_max: f32,
}

/// The draggable handle of a slider
#[derive(Component)]
pub struct SliderHandle {
    pub param_key: String,
    pub is_dragging: bool,
}

/// Text display showing the current value of a parameter
#[derive(Component)]
pub struct ParameterValueText {
    pub param_key: String,
}

/// The green zone visual indicator on a slider
#[derive(Component)]
pub struct GreenZoneBar {
    pub param_key: String,
}

/// Warning text that appears when a parameter is outside optimal range
#[derive(Component)]
pub struct ParameterWarning {
    pub param_key: String,
}

/// Undo button — reverts to last saved config
#[derive(Component)]
pub struct UndoButton {
    pub module_entity: Entity,
    pub slot_name: String,
}

/// Spawn a deep customization window for a specific ship-component slot
pub fn spawn_deep_customization_window(
    commands: &mut Commands,
    module_entity: Entity,
    slot_name: &str,
    option: &SubComponentOption,
    customization: Option<&ModuleCustomization>,
    position: Vec2,
) {
    let window_id = format!("deep_{}_{:?}", slot_name, module_entity);
    let title = format!("{} - {}", slot_name, option.name);

    let content = spawn_floating_window(
        commands,
        &window_id,
        &title,
        Vec2::new(360.0, 450.0),
        position,
    );

    commands.entity(content).insert(DeepCustomizationWindow {
        module_entity,
        slot_name: slot_name.to_string(),
    });

    // Option description
    let desc = commands.spawn(
        (Text::new(&option.description), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(WindowStyle::TEXT_DIM)),
    ).id();
    commands.entity(content).add_child(desc);

    // Separator
    let sep = commands.spawn(
        (Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                ..default()
            }, BackgroundColor(WindowStyle::BORDER_COLOR)),
    ).id();
    commands.entity(content).add_child(sep);

    // Parameters section
    spawn_window_section(commands, content, "Parameters");

    for param_def in &option.parameters {
        let param_key = format!("{}.{}", slot_name, param_def.name);
        let current_value = customization
            .and_then(|c| c.parameter_values.get(&param_key))
            .copied()
            .unwrap_or(param_def.default);

        spawn_parameter_slider(commands, content, &param_key, param_def, current_value);
    }

    // Bottom bar: Undo + Reset to Defaults
    let bottom_bar = commands.spawn(
        (Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                column_gap: Val::Px(6.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            }),
    ).id();

    // Reset button
    let reset_btn = commands.spawn((
        (Node {
                padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(4.0), Val::Px(4.0)),
                ..default()
            }, BackgroundColor(Color::srgba(0.15, 0.12, 0.10, 1.0))),
        UndoButton {
            module_entity,
            slot_name: slot_name.to_string(),
        },
        Tooltip {
            text: "Reset all parameters to defaults".into(),
            detail: None,
        },
    )).id();

    let reset_text = commands.spawn(
        (Text::new("Reset to Default"), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(Color::srgb(0.7, 0.5, 0.4))),
    ).id();

    commands.entity(reset_btn).add_child(reset_text);
    commands.entity(bottom_bar).add_child(reset_btn);
    commands.entity(content).add_child(bottom_bar);
}

/// Spawn a single parameter slider with label, bar, handle, value, green zone, and warning
fn spawn_parameter_slider(
    commands: &mut Commands,
    parent: Entity,
    param_key: &str,
    param_def: &ParameterDef,
    current_value: f32,
) {
    let param_key_str = param_key.to_string();
    let normalized = ((current_value - param_def.min) / (param_def.max - param_def.min)).clamp(0.0, 1.0);

    // Container for this parameter
    let container = commands.spawn(
        (Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                margin: UiRect::vertical(Val::Px(3.0)),
                ..default()
            }),
    ).id();

    // Top row: param name + current value
    let top_row = commands.spawn(
        (Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            }),
    ).id();

    let name_text = commands.spawn((
        (Text::new(&param_def.name), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(WindowStyle::TEXT_COLOR)),
        Tooltip {
            text: param_def.description.clone(),
            detail: Some(format!("Affects: {}", param_def.affects)),
        },
    )).id();

    // Determine value color based on optimal range
    let is_optimal = current_value >= param_def.optimal_min && current_value <= param_def.optimal_max;
    let value_color = if is_optimal {
        Color::srgb(0.4, 0.8, 0.5) // Green — in range
    } else {
        Color::srgb(0.9, 0.7, 0.3) // Yellow — outside optimal
    };

    let value_text = commands.spawn((
        (Text::new(format!("{:.1}{}", current_value, param_def.unit)), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(value_color)),
        ParameterValueText { param_key: param_key_str.clone() },
    )).id();

    commands.entity(top_row).add_children(&[name_text, value_text]);

    // Slider bar
    let slider_bar = commands.spawn((
        (Node {
                width: Val::Percent(100.0),
                height: Val::Px(16.0),
                position_type: PositionType::Relative,
                ..default()
            }, BackgroundColor(Color::srgba(0.05, 0.06, 0.10, 1.0))),
        ParameterSlider {
            param_key: param_key_str.clone(),
            min: param_def.min,
            max: param_def.max,
            optimal_min: param_def.optimal_min,
            optimal_max: param_def.optimal_max,
        },
        Interaction::None,
    )).id();

    // Green zone overlay
    let green_start = ((param_def.optimal_min - param_def.min) / (param_def.max - param_def.min)).clamp(0.0, 1.0);
    let green_end = ((param_def.optimal_max - param_def.min) / (param_def.max - param_def.min)).clamp(0.0, 1.0);
    let green_width = green_end - green_start;

    let green_zone = commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(green_start * 100.0),
                width: Val::Percent(green_width * 100.0),
                height: Val::Percent(100.0),
                ..default()
            }, BackgroundColor(Color::srgba(0.2, 0.5, 0.3, 0.2))),
        GreenZoneBar { param_key: param_key_str.clone() },
    )).id();

    // Fill bar (shows current value position)
    let fill = commands.spawn(
        (Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                width: Val::Percent(normalized * 100.0),
                height: Val::Percent(100.0),
                ..default()
            }, BackgroundColor(Color::srgba(0.3, 0.5, 0.8, 0.5))),
    ).id();

    // Handle (draggable)
    let handle = commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(normalized * 100.0 - 1.5),
                width: Val::Px(6.0),
                height: Val::Percent(100.0),
                ..default()
            }, BackgroundColor(Color::srgb(0.7, 0.8, 1.0))),
        SliderHandle {
            param_key: param_key_str.clone(),
            is_dragging: false,
        },
    )).id();

    commands.entity(slider_bar).add_children(&[green_zone, fill, handle]);

    // Min/max labels
    let range_row = commands.spawn(
        (Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            }),
    ).id();

    let min_label = commands.spawn(
        (Text::new(format!("{:.0}", param_def.min)), TextFont { font_size: FontSize::Px(9.0), ..default() }, TextColor(Color::srgba(0.4, 0.4, 0.5, 0.7))),
    ).id();

    let max_label = commands.spawn(
        (Text::new(format!("{:.0}", param_def.max)), TextFont { font_size: FontSize::Px(9.0), ..default() }, TextColor(Color::srgba(0.4, 0.4, 0.5, 0.7))),
    ).id();

    commands.entity(range_row).add_children(&[min_label, max_label]);

    // Warning text (hidden unless outside optimal)
    if !is_optimal {
        let warning = commands.spawn((
            (Text::new(if current_value < param_def.optimal_min {
                    format!("! Below recommended ({:.0}{})", param_def.optimal_min, param_def.unit)
                } else {
                    format!("! Above recommended ({:.0}{})", param_def.optimal_max, param_def.unit)
                }), TextFont { font_size: FontSize::Px(9.0), ..default() }, TextColor(Color::srgb(0.9, 0.6, 0.2))),
            ParameterWarning { param_key: param_key_str },
        )).id();
        commands.entity(container).add_children(&[top_row, slider_bar, range_row, warning]);
    } else {
        commands.entity(container).add_children(&[top_row, slider_bar, range_row]);
    }

    commands.entity(parent).add_child(container);
}

/// System: handle slider bar clicks to set parameter values.
/// Writes the new value to the ModuleCustomization component on the module entity.
pub fn slider_click_system(
    sliders: Query<(&ParameterSlider, &Interaction, &ComputedNode, &GlobalTransform)>,
    windows: Query<&Window>,
    deep_windows: Query<&DeepCustomizationWindow>,
    mut customization_query: Query<&mut crate::building::customization::parameters::ModuleCustomization>,
    mut value_text_query: Query<(&ParameterValueText, &mut Text, &mut TextColor)>,
) {
    let Some(cursor_pos) = windows.single().ok()
        .and_then(|w| w.cursor_position())
    else {
        return;
    };

    for (slider, interaction, node, global_transform) in sliders.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let bar_pos = global_transform.translation().truncate();
        let bar_size = node.size();
        if bar_size.x <= 0.0 { continue; }

        let local_x = cursor_pos.x - (bar_pos.x - bar_size.x / 2.0);
        let normalized = (local_x / bar_size.x).clamp(0.0, 1.0);
        let value = slider.min + normalized * (slider.max - slider.min);

        // Find the module entity from the deep customization window
        for deep_window in deep_windows.iter() {
            if let Ok(mut customization) = customization_query.get_mut(deep_window.module_entity) {
                customization.parameter_values.insert(slider.param_key.clone(), value);
            }
        }

        // Update the value display text
        for (value_text, mut text, mut text_color) in value_text_query.iter_mut() {
            if value_text.param_key == slider.param_key {
                let is_optimal = value >= slider.optimal_min && value <= slider.optimal_max;
                text.0 = format!("{:.1}", value);
                text_color.0 = if is_optimal {
                    Color::srgb(0.4, 0.8, 0.5)
                } else {
                    Color::srgb(0.9, 0.7, 0.3)
                };
            }
        }
    }
}

/// System: hover feedback on undo/reset buttons
pub fn undo_button_hover(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (With<UndoButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in buttons.iter_mut() {
        *bg = match interaction {
            Interaction::Hovered => Color::srgba(0.22, 0.18, 0.15, 1.0).into(),
            Interaction::Pressed => Color::srgba(0.30, 0.22, 0.18, 1.0).into(),
            Interaction::None => Color::srgba(0.15, 0.12, 0.10, 1.0).into(),
        };
    }
}
