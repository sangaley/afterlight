use bevy::prelude::*;

// ============================================================================
// TOOLTIP SYSTEM
// Hover over any entity with a Tooltip component → popup with info text.
// Auto-positions near cursor. Disappears on mouse leave.
// ============================================================================

/// Add this component to any UI element to give it a hover tooltip
#[derive(Component)]
pub struct Tooltip {
    pub text: String,
    /// Optional secondary line (smaller, dimmer — for stat breakdowns)
    pub detail: Option<String>,
}

/// The actual tooltip popup entity (only one exists at a time)
#[derive(Component)]
pub struct TooltipPopup;

/// Tracks which entity is currently showing a tooltip
#[derive(Resource, Default)]
pub struct TooltipState {
    pub active_entity: Option<Entity>,
    pub show_delay: f32,
    pub hover_timer: f32,
}

const TOOLTIP_DELAY: f32 = 0.3; // Seconds before tooltip appears
const TOOLTIP_OFFSET: Vec2 = Vec2::new(12.0, 16.0); // Offset from cursor

/// System: detect hover on Tooltip entities, show/hide popup
pub fn tooltip_system(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<TooltipState>,
    tooltip_query: Query<(Entity, &Interaction, &Tooltip), Changed<Interaction>>,
    hovering_query: Query<(Entity, &Interaction, &Tooltip)>,
    existing_popup: Query<Entity, With<TooltipPopup>>,
    windows: Query<&Window>,
    ui_scale: Res<bevy::ui::UiScale>,
) {
    let cursor_pos = windows.single().ok()
        .and_then(|w| crate::ui::cursor_in_ui(w, &ui_scale))
        .unwrap_or(Vec2::ZERO);

    // Check for newly hovered tooltip entities
    for (entity, interaction, _tooltip) in tooltip_query.iter() {
        match interaction {
            Interaction::Hovered => {
                if state.active_entity != Some(entity) {
                    state.active_entity = Some(entity);
                    state.hover_timer = 0.0;
                    // Despawn existing popup
                    for popup in existing_popup.iter() {
                        commands.entity(popup).despawn();
                    }
                }
            }
            Interaction::None => {
                if state.active_entity == Some(entity) {
                    state.active_entity = None;
                    state.hover_timer = 0.0;
                    for popup in existing_popup.iter() {
                        commands.entity(popup).despawn();
                    }
                }
            }
            _ => {}
        }
    }

    // Tick hover timer and spawn tooltip after delay
    if let Some(active_entity) = state.active_entity {
        state.hover_timer += time.delta_secs();

        if state.hover_timer >= TOOLTIP_DELAY {
            // Check if popup already exists
            if existing_popup.is_empty() {
                if let Ok((_, _, tooltip)) = hovering_query.get(active_entity) {
                    spawn_tooltip_popup(&mut commands, &tooltip.text, tooltip.detail.as_deref(), cursor_pos);
                }
            } else {
                // Update position of existing popup
                // (position updates handled by tooltip_position_system)
            }
        }
    }
}

/// System: keep tooltip popup near cursor
pub fn tooltip_position_system(
    mut popup_query: Query<&mut Node, With<TooltipPopup>>,
    windows: Query<&Window>,
    ui_scale: Res<bevy::ui::UiScale>,
) {
    let Some(cursor_pos) = windows.single().ok()
        .and_then(|w| crate::ui::cursor_in_ui(w, &ui_scale))
    else {
        return;
    };

    for mut style in popup_query.iter_mut() {
        style.left = Val::Px(cursor_pos.x + TOOLTIP_OFFSET.x);
        style.top = Val::Px(cursor_pos.y + TOOLTIP_OFFSET.y);
    }
}

fn spawn_tooltip_popup(
    commands: &mut Commands,
    text: &str,
    detail: Option<&str>,
    cursor_pos: Vec2,
) {
    let popup = commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor_pos.x + TOOLTIP_OFFSET.x),
                top: Val::Px(cursor_pos.y + TOOLTIP_OFFSET.y),
                padding: UiRect::all(Val::Px(8.0)),
                max_width: Val::Px(300.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            }, BackgroundColor(Color::srgba(0.05, 0.07, 0.12, 0.95)), ZIndex(1000)),
        TooltipPopup,
    )).id();

    // Main text
    let main_text = commands.spawn(
        (Text::new(text), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgb(0.85, 0.88, 0.92))),
    ).id();
    commands.entity(popup).add_child(main_text);

    // Detail text (if provided)
    if let Some(detail_str) = detail {
        let detail_text = commands.spawn(
            (Text::new(detail_str), TextFont { font_size: FontSize::Px(10.0), ..default() }, TextColor(Color::srgb(0.55, 0.58, 0.62))),
        ).id();
        commands.entity(popup).add_child(detail_text);
    }
}
