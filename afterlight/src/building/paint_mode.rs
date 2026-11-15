//! Paint mode: the builder's brush for your ship's paint.
//!
//! N in build mode (or the Paint button) puts the roof over your ship as it
//! looks from out in space, and the mouse paints it instead of placing
//! blocks:
//!
//! - Left click or drag paints a tile with the brush picked: the ship's
//!   main colour, its second, its trim, or bare deck. Auto takes a tile's
//!   paint off, back to the ship's own pattern; right click does the same.
//! - The ship's three colours are picked from a palette, or all at once
//!   from a ready-made scheme. Warm rooms paints each room in a warm colour
//!   by what it is.
//!
//! The paint is the ship's (vfx::ship_roof::ShipPaint): saved with it, and
//! written into a design saved with Ctrl+S.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::MainCamera;
use crate::components::Ship;
use crate::events::{NotificationType, ShowNotification};
use crate::states::BuildState;
use crate::ui::theme::{Sheet, ThemeColors, ThemeFonts};
use crate::vfx::ship_roof::{scheme_named, ShipPaint, Slot};

/// The key that turns paint mode on and off in the builder.
pub const PAINT_KEY: KeyCode = KeyCode::KeyN;

#[derive(Resource)]
pub struct PaintMode {
    pub on: bool,
    pub brush: Brush,
    /// Which of the ship's colours the palette sets.
    pub editing: Channel,
}

impl Default for PaintMode {
    fn default() -> Self {
        Self { on: false, brush: Brush::Paint(Slot::Second), editing: Channel::Main }
    }
}

/// What a click lays on a tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Brush {
    Paint(Slot),
    /// Back to the ship's own pattern.
    Auto,
}

/// One of the ship's three colours.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    Main,
    Second,
    Trim,
}

impl Channel {
    fn get(self, paint: &ShipPaint) -> [f32; 3] {
        match self {
            Channel::Main => paint.main,
            Channel::Second => paint.second,
            Channel::Trim => paint.trim,
        }
    }

    fn set(self, paint: &mut ShipPaint, colour: [f32; 3]) {
        match self {
            Channel::Main => paint.main = colour,
            Channel::Second => paint.second = colour,
            Channel::Trim => paint.trim = colour,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Channel::Main => "Main",
            Channel::Second => "Second",
            Channel::Trim => "Trim",
        }
    }
}

/// The palette: plating greys, warm and cold paints, bright trims.
const PALETTE: [[f32; 3]; 24] = [
    [0.86, 0.86, 0.84], [0.70, 0.68, 0.63], [0.52, 0.52, 0.54], [0.36, 0.37, 0.40], [0.24, 0.25, 0.28], [0.14, 0.15, 0.17],
    [0.82, 0.76, 0.62], [0.74, 0.62, 0.45], [0.80, 0.64, 0.24], [0.92, 0.70, 0.26], [0.86, 0.46, 0.18], [0.66, 0.33, 0.18],
    [0.52, 0.13, 0.13], [0.95, 0.22, 0.18], [0.55, 0.35, 0.90], [0.75, 0.35, 0.95], [0.24, 0.31, 0.47], [0.40, 0.45, 0.65],
    [0.42, 0.85, 1.00], [0.25, 0.85, 0.75], [0.30, 0.80, 0.45], [0.50, 0.75, 0.40], [0.95, 0.85, 0.20], [0.98, 0.96, 0.92],
];

#[derive(Component)]
pub struct PaintWindow;
#[derive(Component)]
pub struct PaintToggleButton;
#[derive(Component)]
pub struct PaintBrushButton(Brush);
#[derive(Component)]
pub struct PaintChannelButton(Channel);
#[derive(Component)]
pub struct PaintSwatch([f32; 3]);
#[derive(Component)]
pub struct PaintSchemeButton(&'static str);
#[derive(Component)]
pub struct PaintWarmToggle;
#[derive(Component)]
pub struct PaintClearButton;

/// N (or the Paint button) in build mode: paint mode on or off. Leaving
/// build mode leaves it too.
pub fn toggle_paint_mode(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    state: Res<State<BuildState>>,
    mut paint: ResMut<PaintMode>,
    ships: Query<(Entity, Has<ShipPaint>), With<Ship>>,
    toggles: Query<&Interaction, (Changed<Interaction>, With<PaintToggleButton>)>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if *state.get() == BuildState::Inactive {
        if paint.on {
            paint.on = false;
        }
        return;
    }
    let clicked = toggles.iter().any(|i| *i == Interaction::Pressed);
    if !(keyboard.just_pressed(PAINT_KEY) || clicked) {
        return;
    }
    paint.on = !paint.on;
    if paint.on {
        if let Ok((ship, false)) = ships.single() {
            let mut fresh = ShipPaint::default();
            // AFTERLIGHT_PAINT_DEMO=1 (filming): a few tiles brushed by hand
            // -- a trim stripe down the middle and second-colour bars -- as a
            // player might.
            if std::env::var("AFTERLIGHT_PAINT_DEMO").ok().as_deref() == Some("1") {
                for x in -30i32..30 {
                    for y in -20i32..20 {
                        let slot = if y.abs() <= 1 && x % 4 != 0 {
                            Some(Slot::Trim)
                        } else if x.rem_euclid(8) < 2 && y.abs() < 9 {
                            Some(Slot::Second)
                        } else {
                            None
                        };
                        if let Some(slot) = slot {
                            fresh.slots.push((IVec2::new(x, y), slot));
                        }
                    }
                }
            }
            commands.entity(ship).insert(fresh);
        }
    }
    notifications.write(ShowNotification {
        message: if paint.on {
            "Paint mode: click or drag to paint, right-click for auto. N to leave.".into()
        } else {
            "Paint mode off.".into()
        },
        notification_type: NotificationType::Info,
        duration: 2.5,
    });
}

/// Paint the tile under the cursor, or drag across many.
#[allow(clippy::too_many_arguments)]
pub fn paint_tiles(
    paint: Res<PaintMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut ships: Query<(&GlobalTransform, &mut ShipPaint), With<Ship>>,
    ui: Query<&Interaction, With<PaintWindow>>,
) {
    if !paint.on {
        return;
    }
    let (left, right) = (mouse.pressed(MouseButton::Left), mouse.pressed(MouseButton::Right));
    if !left && !right {
        return;
    }
    // Clicks on the paint window are the window's.
    if ui.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let (Ok(window), Ok((cam, cam_gt)), Ok((ship_gt, mut ship_paint))) = (windows.single(), camera.single(), ships.single_mut()) else { return };
    let Some(cell) = super::cursor_to_ship_grid(window, cam, cam_gt, ship_gt) else { return };
    let want = if right { None } else { match paint.brush { Brush::Paint(slot) => Some(slot), Brush::Auto => None } };
    let at = ship_paint.slots.iter().position(|(c, _)| *c == cell);
    // Only touch the paint when the tile changes: the roof is laid again
    // on every change.
    match (at, want) {
        (Some(i), Some(slot)) if ship_paint.slots[i].1 != slot => ship_paint.slots[i].1 = slot,
        (Some(i), None) => {
            ship_paint.slots.swap_remove(i);
        }
        (None, Some(slot)) => ship_paint.slots.push((cell, slot)),
        _ => {}
    }
}

/// The paint window's buttons.
#[allow(clippy::type_complexity)]
pub fn paint_window_clicks(
    mut paint: ResMut<PaintMode>,
    mut ships: Query<&mut ShipPaint, With<Ship>>,
    clicks: Query<
        (
            &Interaction,
            Option<&PaintBrushButton>,
            Option<&PaintChannelButton>,
            Option<&PaintSwatch>,
            Option<&PaintSchemeButton>,
            Has<PaintWarmToggle>,
            Has<PaintClearButton>,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, brush, channel, swatch, scheme, warm, clear) in clicks.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(b) = brush {
            paint.brush = b.0;
        }
        if let Some(c) = channel {
            paint.editing = c.0;
        }
        let Ok(mut ship_paint) = ships.single_mut() else { continue };
        if let Some(s) = swatch {
            let editing = paint.editing;
            editing.set(&mut ship_paint, s.0);
        }
        if let Some(name) = scheme {
            let slots = std::mem::take(&mut ship_paint.slots);
            *ship_paint = ShipPaint::from_scheme(scheme_named(name.0));
            ship_paint.slots = slots;
        }
        if warm {
            ship_paint.warm = !ship_paint.warm;
        }
        if clear {
            ship_paint.slots.clear();
        }
    }
}

fn colour(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

/// Show the paint window while paint mode is on, drawn again whenever the
/// brush, the colour being set or the paint itself changes.
pub fn sync_paint_window(
    mut commands: Commands,
    paint: Res<PaintMode>,
    ships: Query<Ref<ShipPaint>, With<Ship>>,
    open: Query<Entity, With<PaintWindow>>,
) {
    let ship_paint = ships.single().ok();
    let changed = paint.is_changed() || ship_paint.as_ref().is_some_and(|p| p.is_changed());
    if !changed && (paint.on == !open.is_empty()) {
        return;
    }
    for e in open.iter() {
        commands.entity(e).try_despawn();
    }
    let (true, Some(ship_paint)) = (paint.on, ship_paint) else { return };
    spawn_paint_window(&mut commands, &paint, &ship_paint);
}

fn label(text: impl Into<String>, size: f32, colour: Color) -> impl Bundle {
    (Text::new(text), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(colour))
}

fn button_node(width: f32, height: f32, picked: bool) -> impl Bundle {
    (
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(if picked { 2.0 } else { 1.0 })),
            ..default()
        },
        Button,
        BorderColor::all(if picked { Sheet::PICKED } else { Sheet::RULE }),
    )
}

fn spawn_paint_window(commands: &mut Commands, paint: &PaintMode, ship_paint: &ShipPaint) {
    let row = || Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(4.0), ..default() };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(96.0),
                width: Val::Px(262.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Sheet::BG),
            BorderColor::all(Sheet::EDGE),
            Interaction::default(),
            PaintWindow,
        ))
        .with_children(|w| {
            w.spawn(label("PAINT", ThemeFonts::BODY, Sheet::TITLE));

            w.spawn(label("BRUSH", ThemeFonts::CAPTION, Sheet::LABEL));
            w.spawn(row()).with_children(|r| {
                let brushes = [
                    (Brush::Paint(Slot::Main), "Main", Some(ship_paint.main)),
                    (Brush::Paint(Slot::Second), "2nd", Some(ship_paint.second)),
                    (Brush::Paint(Slot::Trim), "Trim", Some(ship_paint.trim)),
                    (Brush::Paint(Slot::Bare), "Bare", Some([0.2, 0.21, 0.23])),
                    (Brush::Auto, "Auto", None),
                ];
                for (brush, name, swatch) in brushes {
                    r.spawn((button_node(44.0, 30.0, paint.brush == brush), BackgroundColor(swatch.map_or(Sheet::ROW_PICKED, colour)), PaintBrushButton(brush)))
                        .with_children(|b| {
                            b.spawn((label(name, ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY), TextShadow::default()));
                        });
                }
            });

            w.spawn(label(format!("COLOURS -- picking {}", paint.editing.name()), ThemeFonts::CAPTION, Sheet::LABEL));
            w.spawn(row()).with_children(|r| {
                for channel in [Channel::Main, Channel::Second, Channel::Trim] {
                    r.spawn((button_node(78.0, 24.0, paint.editing == channel), BackgroundColor(colour(channel.get(ship_paint))), PaintChannelButton(channel)))
                        .with_children(|b| {
                            b.spawn((label(channel.name(), ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY), TextShadow::default()));
                        });
                }
            });
            w.spawn(row()).with_children(|r| {
                for c in PALETTE {
                    let picked = paint.editing.get(ship_paint) == c;
                    r.spawn((button_node(37.0, 18.0, picked), BackgroundColor(colour(c)), PaintSwatch(c)));
                }
            });

            w.spawn(label("SCHEMES", ThemeFonts::CAPTION, Sheet::LABEL));
            w.spawn(row()).with_children(|r| {
                for (name, key) in [("Salvager", "salvager"), ("Raider", "raider"), ("Navy", "navy")] {
                    r.spawn((button_node(78.0, 22.0, false), BackgroundColor(Sheet::ROW_PICKED), PaintSchemeButton(key)))
                        .with_children(|b| {
                            b.spawn(label(name, ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                        });
                }
            });
            w.spawn(row()).with_children(|r| {
                r.spawn((button_node(120.0, 22.0, ship_paint.warm), BackgroundColor(Sheet::ROW_PICKED), PaintWarmToggle)).with_children(|b| {
                    b.spawn(label(if ship_paint.warm { "Warm rooms: on" } else { "Warm rooms: off" }, ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                });
                r.spawn((button_node(120.0, 22.0, false), BackgroundColor(Sheet::ROW_PICKED), PaintClearButton)).with_children(|b| {
                    b.spawn(label("Clear hand paint", ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                });
            });
            w.spawn(label(
                format!("{} tiles hand-painted. Click or drag to paint,\nright-click for auto. N leaves.", ship_paint.slots.len()),
                ThemeFonts::CAPTION,
                ThemeColors::TEXT_MUTED,
            ));
        });
}

/// The Paint button, beside the build panel's hints: paint mode for those
/// who don't know N.
#[derive(Component)]
pub struct PaintButtonRoot;

pub fn spawn_paint_button(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(96.0),
                padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(6.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            Button,
            BackgroundColor(Sheet::BG),
            BorderColor::all(Sheet::EDGE),
            PaintToggleButton,
            PaintButtonRoot,
        ))
        .with_children(|b| {
            b.spawn(label("Paint (N)", ThemeFonts::CAPTION, Sheet::TITLE));
        });
}

pub fn despawn_paint_ui(mut commands: Commands, ui: Query<Entity, Or<(With<PaintButtonRoot>, With<PaintWindow>)>>) {
    for e in ui.iter() {
        commands.entity(e).try_despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ships_paint_survives_a_save() {
        let mut paint = ShipPaint::from_scheme(scheme_named("raider"));
        paint.slots.push((IVec2::new(3, -2), Slot::Trim));
        paint.warm = true;
        let json = serde_json::to_string(&paint).unwrap();
        let back: ShipPaint = serde_json::from_str(&json).unwrap();
        assert_eq!(back, paint);
        assert_eq!(back.stamp(), paint.stamp());
    }

    #[test]
    fn a_painted_tile_changes_the_stamp() {
        let mut paint = ShipPaint::from_scheme(scheme_named("navy"));
        let before = paint.stamp();
        paint.slots.push((IVec2::ZERO, Slot::Second));
        assert_ne!(paint.stamp(), before, "the roof wouldn't be laid again");
    }

    #[test]
    fn an_older_design_without_paint_still_loads() {
        let json = r#"{"name":"old","hull_cells":[],"modules":[],"created_at":"x","version":2}"#;
        let bp: crate::building::blueprint::Blueprint = serde_json::from_str(json).unwrap();
        assert!(bp.paint.is_none());
    }
}
