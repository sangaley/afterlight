use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use crate::ui::theme::*;

// ============================================================================
// RADIAL CONTEXT MENU
// Right-click → circular menu with context-specific actions.
// Each slice is a clickable option. Mouse angle selects the slice.
// ============================================================================

/// Marker for the radial menu root
#[derive(Component)]
pub struct RadialMenu {
    pub center: Vec2,
    pub options: Vec<RadialOption>,
    pub selected: Option<usize>,
}

/// A single option in the radial menu
#[derive(Clone)]
pub struct RadialOption {
    pub label: String,
    pub icon: String,
    pub action: RadialAction,
    pub color: Color,
}

/// What happens when an option is selected
#[derive(Clone, Debug)]
pub enum RadialAction {
    RadarPing,
    WarpCharge,
    ToggleMap,
    ToggleLog,
    RepairModule(Entity),
    PowerToggle(Entity),
    InspectModule(Entity),
    Dismiss,
}

/// Marker for individual radial slice visuals
#[derive(Component)]
pub struct RadialSlice {
    pub index: usize,
}

/// Marker for the radial menu label text
#[derive(Component)]
pub struct RadialLabel;

const RADIAL_INNER_RADIUS: f32 = 30.0;
const RADIAL_OUTER_RADIUS: f32 = 80.0;
const RADIAL_LABEL_RADIUS: f32 = 55.0;

/// Spawn a radial menu at the given screen position with the given options
pub fn spawn_radial_menu(
    commands: &mut Commands,
    center: Vec2,
    options: Vec<RadialOption>,
) {
    let option_count = options.len();
    if option_count == 0 { return; }

    let menu = commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center.x - RADIAL_OUTER_RADIUS),
                top: Val::Px(center.y - RADIAL_OUTER_RADIUS),
                width: Val::Px(RADIAL_OUTER_RADIUS * 2.0),
                height: Val::Px(RADIAL_OUTER_RADIUS * 2.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            }, ZIndex(500)),
        RadialMenu {
            center,
            options: options.clone(),
            selected: None,
        },
    )).id();

    // Center dot
    let center_dot = commands.spawn(
        (Node {
                position_type: PositionType::Absolute,
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                left: Val::Px(RADIAL_OUTER_RADIUS - 4.0),
                top: Val::Px(RADIAL_OUTER_RADIUS - 4.0),
                ..default()
            }, BackgroundColor(ThemeColors::ACCENT_BLUE)),
    ).id();
    commands.entity(menu).add_child(center_dot);

    // Spawn slice labels around the circle
    let angle_step = TAU / option_count as f32;
    for (i, option) in options.iter().enumerate() {
        let angle = -PI / 2.0 + angle_step * i as f32 + angle_step / 2.0;
        let label_x = RADIAL_OUTER_RADIUS + RADIAL_LABEL_RADIUS * angle.cos() - 30.0;
        let label_y = RADIAL_OUTER_RADIUS + RADIAL_LABEL_RADIUS * angle.sin() - 8.0;

        // Slice background (positioned around the circle)
        let slice = commands.spawn((
            (Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(label_x),
                    top: Val::Px(label_y),
                    padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(3.0), Val::Px(3.0)),
                    ..default()
                }, BackgroundColor(ThemeColors::BG_ELEVATED)),
            RadialSlice { index: i },
            Interaction::None,
        )).id();

        // Icon + label
        let text = commands.spawn(
            (Text::new(format!("{} {}", option.icon, option.label)), TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() }, TextColor(option.color)),
        ).id();

        commands.entity(slice).add_child(text);
        commands.entity(menu).add_child(slice);
    }

    // Center label (shows currently hovered option)
    let label = commands.spawn((
        (Text::new(""), TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() }, TextColor(ThemeColors::TEXT_MUTED), Node { position_type: PositionType::Absolute,
            left: Val::Px(RADIAL_OUTER_RADIUS - 40.0),
            top: Val::Px(RADIAL_OUTER_RADIUS + RADIAL_OUTER_RADIUS + 4.0),
            ..default() }),
        RadialLabel,
    )).id();
    commands.entity(menu).add_child(label);
}

/// System: update radial menu based on mouse position, highlight selected slice
pub fn update_radial_menu(
    windows: Query<&Window>,
    mut menu_query: Query<(&mut RadialMenu, &Children)>,
    mut slice_query: Query<(&RadialSlice, &mut BackgroundColor)>,
    ui_scale: Res<bevy::ui::UiScale>,
) {
    let Some(cursor_pos) = windows.single().ok()
        .and_then(|w| crate::ui::cursor_in_ui(w, &ui_scale))
    else { return };

    for (mut menu, _children) in menu_query.iter_mut() {
        let delta = cursor_pos - menu.center;
        let dist = delta.length();

        if dist < RADIAL_INNER_RADIUS || dist > RADIAL_OUTER_RADIUS * 1.5 {
            menu.selected = None;
        } else {
            let angle = delta.y.atan2(delta.x) + PI / 2.0;
            let angle = if angle < 0.0 { angle + TAU } else { angle };
            let count = menu.options.len();
            let slice_index = ((angle / TAU) * count as f32) as usize % count;
            menu.selected = Some(slice_index);
        }

        // Update slice colors
        for (slice, mut bg) in slice_query.iter_mut() {
            *bg = if menu.selected == Some(slice.index) {
                ThemeColors::BG_PRESSED.into()
            } else {
                ThemeColors::BG_ELEVATED.into()
            };
        }
    }
}

/// System: spawn radial menu on right-click during Exploring.
/// Right-clicking one of OUR crew-station blocks instead toggles its
/// keep-manned pin (crew there never leave for salvage details).
pub fn spawn_radial_on_right_click(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    existing: Query<Entity, With<RadialMenu>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    ship_query: Query<Entity, With<crate::components::Ship>>,
    station_query: Query<
        (Entity, &GlobalTransform, &crate::components::Module, &ChildOf, Has<crate::components::KeepManned>),
        With<crate::components::CrewStation>,
    >,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
    // Right-click is the weapons lock first and a menu second — see
    // combat::targeting::aim_lock. If the click went to the guns, it isn't
    // ours this frame.
    aim_lock: Res<crate::combat::targeting::AimLock>,
    ui_scale: Res<bevy::ui::UiScale>,
) {
    if !mouse.just_pressed(MouseButton::Right) { return; }
    if aim_lock.click_consumed { return; }

    // Close existing menu if one is open
    for entity in existing.iter() {
        commands.entity(entity).despawn();
        return; // Don't open a new one, just close
    }

    let Some(cursor_pos) = windows.single().ok()
        .and_then(|w| w.cursor_position())
    else { return };

    // Crew-station block under the cursor? Toggle its pin, no menu.
    let world_pos = camera_query.single().ok()
        .and_then(|(cam, gt)| cam.viewport_to_world_2d(gt, cursor_pos).ok());
    if let (Ok(ship), Some(world_pos)) = (ship_query.single(), world_pos) {
        for (entity, gt, module, parent, pinned) in station_query.iter() {
            if parent.0 != ship {
                continue;
            }
            if gt.translation().truncate().distance(world_pos) < 33.0 {
                let name = module.module_type.name();
                if pinned {
                    commands.entity(entity).remove::<crate::components::KeepManned>();
                    notifications.write(crate::events::ShowNotification {
                        message: format!("{} unpinned - its crew may leave for salvage.", name),
                        notification_type: crate::events::NotificationType::Info,
                        duration: 2.5,
                    });
                } else {
                    commands.entity(entity).insert(crate::components::KeepManned);
                    notifications.write(crate::events::ShowNotification {
                        message: format!("{} pinned - will stay manned during salvage.", name),
                        notification_type: crate::events::NotificationType::Success,
                        duration: 2.5,
                    });
                }
                return;
            }
        }
    }

    // The menu is a UI node, so it is placed (and later hit-tested) in UI
    // units; the world pick above needs the raw cursor.
    spawn_radial_menu(&mut commands, cursor_pos / ui_scale.0.max(0.01), space_radial_options());
}

/// System: close radial menu on left click or escape, execute selected action
pub fn radial_menu_input(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    menu_query: Query<(Entity, &RadialMenu)>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    let Ok((entity, menu)) = menu_query.single() else { return };

    // Close on escape
    if keyboard.just_pressed(KeyCode::Escape) {
        commands.entity(entity).despawn();
        return;
    }

    // Execute on left click
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(selected) = menu.selected {
            let option = &menu.options[selected];
            match &option.action {
                RadialAction::RadarPing => {
                    // Simulate Z key press for radar ping
                    notifications.write(crate::events::ShowNotification {
                        message: "Radar ping sent! Press Z for manual ping.".into(),
                        notification_type: crate::events::NotificationType::Info,
                        duration: 2.0,
                    });
                }
                RadialAction::WarpCharge => {
                    notifications.write(crate::events::ShowNotification {
                        message: "Pick a destination on the map (M), then press G to warp.".into(),
                        notification_type: crate::events::NotificationType::Info,
                        duration: 3.0,
                    });
                }
                RadialAction::ToggleMap => {
                    notifications.write(crate::events::ShowNotification {
                        message: "Press N to toggle system map.".into(),
                        notification_type: crate::events::NotificationType::Info,
                        duration: 2.0,
                    });
                }
                RadialAction::ToggleLog => {
                    notifications.write(crate::events::ShowNotification {
                        message: "Press L to toggle event log.".into(),
                        notification_type: crate::events::NotificationType::Info,
                        duration: 2.0,
                    });
                }
                RadialAction::PowerToggle(_module_entity) => {
                    notifications.write(crate::events::ShowNotification {
                        message: "Module power toggled.".into(),
                        notification_type: crate::events::NotificationType::Info,
                        duration: 2.0,
                    });
                }
                RadialAction::InspectModule(_) => {
                    notifications.write(crate::events::ShowNotification {
                        message: "Right-click module in build mode to inspect.".into(),
                        notification_type: crate::events::NotificationType::Info,
                        duration: 2.0,
                    });
                }
                RadialAction::RepairModule(_) => {
                    notifications.write(crate::events::ShowNotification {
                        message: "Crew dispatched for repair.".into(),
                        notification_type: crate::events::NotificationType::Info,
                        duration: 2.0,
                    });
                }
                RadialAction::Dismiss => {}
            }
        }
        commands.entity(entity).despawn();
    }
}

/// Default radial options for open space (no module selected)
pub fn space_radial_options() -> Vec<RadialOption> {
    vec![
        RadialOption {
            label: "Radar Ping".into(),
            icon: "o".into(),
            action: RadialAction::RadarPing,
            color: ThemeColors::ACCENT_CYAN,
        },
        RadialOption {
            label: "Warp".into(),
            icon: ">".into(),
            action: RadialAction::WarpCharge,
            color: ThemeColors::ACCENT_PURPLE,
        },
        RadialOption {
            label: "System Map".into(),
            icon: "#".into(),
            action: RadialAction::ToggleMap,
            color: ThemeColors::ACCENT_BLUE,
        },
        RadialOption {
            label: "Event Log".into(),
            icon: "=".into(),
            action: RadialAction::ToggleLog,
            color: ThemeColors::TEXT_SECONDARY,
        },
    ]
}

/// Radial options when right-clicking a module
pub fn module_radial_options(module_entity: Entity) -> Vec<RadialOption> {
    vec![
        RadialOption {
            label: "Inspect".into(),
            icon: "*".into(),
            action: RadialAction::InspectModule(module_entity),
            color: ThemeColors::ACCENT_BLUE,
        },
        RadialOption {
            label: "Power".into(),
            icon: "!".into(),
            action: RadialAction::PowerToggle(module_entity),
            color: ThemeColors::ACCENT_YELLOW,
        },
        RadialOption {
            label: "Repair".into(),
            icon: "+".into(),
            action: RadialAction::RepairModule(module_entity),
            color: ThemeColors::ACCENT_GREEN,
        },
        RadialOption {
            label: "Close".into(),
            icon: "X".into(),
            action: RadialAction::Dismiss,
            color: ThemeColors::TEXT_MUTED,
        },
    ]
}
