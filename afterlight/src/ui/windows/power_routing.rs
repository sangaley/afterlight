use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::resources::{PowerAllocation, PowerChannel, PowerChannels};
use crate::ui::theme::{ThemeColors, ThemeFonts};
use super::framework::*;

// ============================================================================
// POWER ROUTING WINDOW
// A floating panel that lets the player split reactor output between Weapons,
// Shields and Engines. Each channel is an independent, freely-draggable 0–100%
// draw; asking for more than the reactor makes browns everything out (see
// update_power_allocation). Opened with U or the "Pwr" flight-toolbar button.
// ============================================================================

pub(crate) const WINDOW_ID: &str = "power_routing";

/// The draggable track of one channel's slider.
#[derive(Component)]
pub(crate) struct PowerSliderTrack {
    channel: PowerChannel,
}

/// Bar fill inside a channel slider — width tracks the requested %, color
/// tracks delivered performance.
#[derive(Component)]
pub(crate) struct PowerBarFill {
    channel: PowerChannel,
}

/// The "68%  ×1.4" readout on a channel row (requested level + delivered mult).
#[derive(Component)]
pub(crate) struct PowerValueText {
    channel: PowerChannel,
}

/// A preset button that snaps the whole allocation to a saved layout.
#[derive(Component)]
pub(crate) struct PowerPresetButton {
    alloc: PowerAllocation,
}

/// The "REACTOR … · DEMAND …" header line.
#[derive(Component)]
pub(crate) struct PowerReactorText;

/// Which channel's slider is mid-drag (survives the cursor sliding off the
/// track until the mouse button releases). Mirrors tuning's ActiveSliderDrag.
#[derive(Resource, Default)]
pub struct PowerSliderDrag(pub Option<PowerChannel>);

fn channel_accent(channel: PowerChannel) -> Color {
    match channel {
        PowerChannel::Weapons => ThemeColors::ACCENT_ORANGE,
        PowerChannel::Shields => ThemeColors::ACCENT_CYAN,
        PowerChannel::Engines => ThemeColors::ACCENT_GREEN,
    }
}

/// Delivered-performance color: green boosted, channel-accent near nominal,
/// orange starved, red effectively offline.
fn perf_color(channel: PowerChannel, mult: f32) -> Color {
    if mult >= 1.15 {
        ThemeColors::ACCENT_GREEN
    } else if mult >= 0.9 {
        channel_accent(channel)
    } else if mult >= 0.4 {
        ThemeColors::ACCENT_ORANGE
    } else {
        ThemeColors::ACCENT_RED
    }
}

/// Toggle the window with U (also driven by the "Pwr" toolbar button, which
/// synthesizes KeyU). Closing despawns the whole window subtree by its id.
pub fn toggle_power_window(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<(Entity, &FloatingWindow)>,
) {
    if !keyboard.just_pressed(KeyCode::KeyU) {
        return;
    }
    if let Some((entity, _)) = windows.iter().find(|(_, w)| w.id == WINDOW_ID) {
        commands.entity(entity).despawn();
        return;
    }
    spawn_power_window(&mut commands);
}

fn spawn_power_window(commands: &mut Commands) {
    let content = spawn_floating_window(
        commands,
        WINDOW_ID,
        "Power Routing",
        Vec2::new(300.0, 0.0),
        Vec2::new(24.0, 150.0),
    );

    // Reactor / demand header.
    let header = commands
        .spawn((
            Text::new("REACTOR - - DEMAND -"),
            TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
            TextColor(ThemeColors::TEXT_MUTED),
            Node { margin: UiRect::bottom(Val::Px(4.0)), ..default() },
            PowerReactorText,
        ))
        .id();
    commands.entity(content).add_child(header);

    for channel in PowerChannel::ALL {
        spawn_channel_row(commands, content, channel);
    }

    // Presets.
    let hint = commands
        .spawn((
            Text::new("PRESETS - drag a bar for fine control"),
            TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
            TextColor(ThemeColors::TEXT_MUTED),
            Node { margin: UiRect::top(Val::Px(8.0)), ..default() },
        ))
        .id();
    commands.entity(content).add_child(hint);

    let preset_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();
    // Sums kept modest so a healthy single reactor serves them without brownout.
    let presets: [(&str, PowerAllocation); 4] = [
        ("BAL", PowerAllocation { weapons: 50.0, shields: 50.0, engines: 50.0 }),
        ("ATK", PowerAllocation { weapons: 80.0, shields: 50.0, engines: 20.0 }),
        ("DEF", PowerAllocation { weapons: 40.0, shields: 80.0, engines: 30.0 }),
        ("RUN", PowerAllocation { weapons: 30.0, shields: 40.0, engines: 80.0 }),
    ];
    for (label, alloc) in presets {
        let btn = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(20.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(ThemeColors::BG_ELEVATED),
                BorderColor::all(ThemeColors::BORDER_DEFAULT),
                Button,
                Interaction::default(),
                PowerPresetButton { alloc },
            ))
            .id();
        let txt = commands
            .spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
                TextColor(ThemeColors::TEXT_SECONDARY),
            ))
            .id();
        commands.entity(btn).add_child(txt);
        commands.entity(preset_row).add_child(btn);
    }
    commands.entity(content).add_child(preset_row);
}

/// One channel: `LABEL … 50% ×1.0` over a draggable `[===fill===|tick    ]` bar.
fn spawn_channel_row(commands: &mut Commands, content: Entity, channel: PowerChannel) {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();

    // Top line: label + "% ×mult" readout.
    let label_line = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    let label = commands
        .spawn((
            Text::new(channel.name()),
            TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
            TextColor(channel_accent(channel)),
        ))
        .id();
    let value = commands
        .spawn((
            Text::new("200W - 20% - x1.0"),
            TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
            TextColor(ThemeColors::TEXT_PRIMARY),
            PowerValueText { channel },
        ))
        .id();
    commands.entity(label_line).add_children(&[label, value]);

    // Draggable track. FocusPolicy::Block so a drag over the bar doesn't leak
    // to anything behind it; the fill/tick Pass so they don't eat the drag.
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.11, 0.9)),
            Interaction::None,
            Button,
            FocusPolicy::Block,
            PowerSliderTrack { channel },
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(channel_accent(channel)),
            FocusPolicy::Pass,
            PowerBarFill { channel },
        ))
        .id();
    // Baseline tick at 50% — the ×1.0 "normal" reference.
    let tick = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Px(0.0),
                width: Val::Px(2.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(ThemeColors::BORDER_BRIGHT),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(track).add_children(&[fill, tick]);

    commands.entity(row).add_children(&[label_line, track]);
    commands.entity(content).add_child(row);
}

/// Press on a track starts a drag; the channel's level follows the cursor
/// until the button releases, even if the cursor slides off the bar. Gives
/// arbitrary (non-stepped) allocations like 23% / 68% / 0%.
pub fn power_slider_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<PowerSliderDrag>,
    // UiGlobalTransform, NOT GlobalTransform: Bevy 0.19 doesn't keep the classic
    // GlobalTransform in sync for UI nodes (same note as tuning_slider_drag).
    tracks: Query<(&PowerSliderTrack, &Interaction, &ComputedNode, &bevy::ui::UiGlobalTransform)>,
    windows: Query<&Window>,
    mut alloc: ResMut<PowerAllocation>,
) {
    if mouse.just_released(MouseButton::Left) {
        drag.0 = None;
    }

    let Ok(window) = windows.single() else { return };
    // cursor_position() is logical pixels; ComputedNode/UiGlobalTransform are
    // physical — scale up or every hit lands wrong on Retina.
    let Some(cursor) = window.cursor_position().map(|p| p * window.scale_factor()) else { return };

    // Start a drag on whichever track was just pressed.
    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        for (track, interaction, _, _) in tracks.iter() {
            if *interaction == Interaction::Pressed {
                drag.0 = Some(track.channel);
                break;
            }
        }
    }

    let Some(active) = drag.0 else { return };
    if !mouse.pressed(MouseButton::Left) {
        drag.0 = None;
        return;
    }

    for (track, _, node, transform) in tracks.iter() {
        if track.channel != active {
            continue;
        }
        let Some(norm) = node.normalize_point(*transform, cursor) else { continue };
        let fraction = (norm.x + 0.5).clamp(0.0, 1.0);
        alloc.set(active, fraction * 100.0);
        return;
    }
}

/// Preset buttons snap the whole allocation.
pub fn power_preset_click(
    buttons: Query<(&Interaction, &PowerPresetButton), Changed<Interaction>>,
    mut alloc: ResMut<PowerAllocation>,
) {
    for (interaction, btn) in buttons.iter() {
        if *interaction == Interaction::Pressed {
            *alloc = btn.alloc;
        }
    }
}

/// Hover/press feedback on the preset buttons.
pub fn power_button_hover(
    mut presets: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<PowerPresetButton>),
    >,
) {
    for (interaction, mut bg) in presets.iter_mut() {
        *bg = crate::ui::theme::button_color_for_interaction(interaction).into();
    }
}

/// Live readout: bar widths/colors, "% ×mult" text, reactor/demand header.
/// Only touches anything when the window is open.
pub fn power_window_refresh(
    alloc: Res<PowerAllocation>,
    channels: Res<PowerChannels>,
    mut bars: Query<(&PowerBarFill, &mut Node, &mut BackgroundColor)>,
    mut values: Query<(&PowerValueText, &mut Text, &mut TextColor), Without<PowerReactorText>>,
    mut reactor: Query<(&mut Text, &mut TextColor), (With<PowerReactorText>, Without<PowerValueText>)>,
) {
    for (bar, mut node, mut bg) in bars.iter_mut() {
        let requested = alloc.get(bar.channel);
        node.width = Val::Percent(requested.clamp(0.0, 100.0));
        *bg = perf_color(bar.channel, channels.mult(bar.channel)).into();
    }

    for (marker, mut text, mut color) in values.iter_mut() {
        // Watts this channel is drawing from the reactor (its requested load),
        // and what slice of total generation that is. The three reactor-% add
        // past 100% exactly when you've over-committed (a brownout).
        let watts = (alloc.get(marker.channel) / 50.0) * crate::ship::POWER_PER_MULT;
        let reactor_pct = if channels.supply > 0.0 {
            format!("{:.0}%", watts / channels.supply * 100.0)
        } else {
            "--".to_string()
        };
        let m = channels.mult(marker.channel);
        let effect = if m < 0.4 {
            "OFF".to_string()
        } else {
            format!("x{:.1}", m)
        };
        text.0 = format!("{:.0}W - {} - {}", watts, reactor_pct, effect);
        color.0 = perf_color(marker.channel, m);
    }

    if let Ok((mut text, mut color)) = reactor.single_mut() {
        if channels.brownout {
            text.0 = format!(
                "REACTOR {:.0} - DEMAND {:.0} - BROWNOUT",
                channels.supply, channels.demand
            );
            color.0 = ThemeColors::ACCENT_RED;
        } else {
            text.0 = format!(
                "REACTOR {:.0} - DEMAND {:.0}",
                channels.supply, channels.demand
            );
            color.0 = ThemeColors::TEXT_MUTED;
        }
    }
}
