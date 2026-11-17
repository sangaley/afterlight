//! Paint mode: the builder's brush for your ship's paint.
//!
//! N in build mode (or the Paint button) puts the roof over your ship as it
//! looks from out in space, and the mouse paints it instead of placing
//! blocks:
//!
//! - Left click or drag paints a tile -- any tile, the bridge, a gun's or
//!   an angled plate's -- with the brush picked: a colour of your own, the
//!   ship's main colour, its second, its trim, or bare deck. Auto (or a
//!   right click) takes a tile's paint off, back to the ship's own pattern.
//! - A colour is picked from the wheel and its brightness bar, the palette,
//!   the colours used last, or typed as a number: `#C8642A`, `C64`, or
//!   `200 100 42`. Alt+click a painted tile takes its colour.
//! - The colour picker sets the brush's own colour, or any of the ship's
//!   three; a ready-made scheme sets all three at once. Warm rooms paints
//!   each room in a warm colour by what it is.
//! - With symmetry on (Y) a tile's mirror is painted with it.
//!
//! The paint is the ship's (vfx::ship_roof::ShipPaint): saved with it, and
//! written into a design saved with Ctrl+S.

use bevy::asset::RenderAssetUsages;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;

use crate::camera::MainCamera;
use crate::components::Ship;
use crate::events::{NotificationType, ShowNotification};
use crate::states::BuildState;
use crate::ui::theme::{Sheet, ThemeColors, ThemeFonts};
use crate::components::{HullSegment, Module};
use crate::vfx::paint_gen;
use crate::vfx::ship_roof::{scheme_named, ship_cells, Decal, DecalShape, ShipPaint, Slot};

/// The key that turns paint mode on and off in the builder.
pub const PAINT_KEY: KeyCode = KeyCode::KeyN;

/// How many of the colours used last are kept to hand.
const RECENT: usize = 10;

/// Bare deck's colour, as the roof lays it.
const BARE: [f32; 3] = [0.2, 0.21, 0.23];

#[derive(Resource)]
pub struct PaintMode {
    pub on: bool,
    pub brush: Brush,
    /// Which colour the picker sets: the brush's own or one of the ship's.
    pub editing: Channel,
    /// The brush's own colour.
    pub own: [u8; 3],
    /// The colour being picked as hue, saturation, value (0-1): kept apart
    /// from the colour so a hue isn't lost when it is dragged to black.
    pub hsv: [f32; 3],
    /// The colours painted with last, newest first.
    pub recent: Vec<[u8; 3]>,
    /// The colour number being typed, while its field is open.
    pub typing: Option<String>,
    /// The wheel or brightness bar being dragged.
    pub dragging: Option<Drag>,
    /// Painting decals (shapes over the tiles) rather than tiles.
    pub decals: bool,
    /// The decal brush: its shape, quarter turns (R), and layer 1-3.
    pub shape: DecalShape,
    pub turn: u8,
    pub layer: u8,
}

impl Default for PaintMode {
    fn default() -> Self {
        let own = [214, 110, 46];
        Self {
            on: false,
            brush: Brush::Own,
            editing: Channel::Own,
            own,
            hsv: rgb_to_hsv(to_f32(own)),
            recent: Vec::new(),
            typing: None,
            dragging: None,
            decals: false,
            shape: DecalShape::Diagonal,
            turn: 0,
            layer: 1,
        }
    }
}

/// What a click lays on a tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Brush {
    /// The brush's own colour, any colour at all.
    Own,
    /// One of the ship's colours, or bare deck: the tile changes with it.
    Paint(Slot),
    /// Back to the ship's own pattern.
    Auto,
}

/// A colour the picker sets.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Channel {
    Own,
    Main,
    Second,
    Trim,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drag {
    Wheel,
    Value,
}

impl Channel {
    fn name(self) -> &'static str {
        match self {
            Channel::Own => "Brush",
            Channel::Main => "Main",
            Channel::Second => "2nd",
            Channel::Trim => "Trim",
        }
    }
}

impl PaintMode {
    /// The colour the picker is showing.
    pub fn current(&self, ship: &ShipPaint) -> [u8; 3] {
        match self.editing {
            Channel::Own => self.own,
            Channel::Main => to_u8(ship.main),
            Channel::Second => to_u8(ship.second),
            Channel::Trim => to_u8(ship.trim),
        }
    }

    /// Set the colour the picker is on. Picking the brush's colour takes the
    /// brush up too. The ship's paint is only touched when it changes: the
    /// roof is laid again on every change.
    fn set(&mut self, ship: &mut Mut<ShipPaint>, c: [u8; 3], hsv: Option<[f32; 3]>) {
        self.hsv = hsv.unwrap_or_else(|| keep_hue(self.hsv, rgb_to_hsv(to_f32(c))));
        let f = to_f32(c);
        match self.editing {
            Channel::Own => {
                self.own = c;
                self.brush = Brush::Own;
            }
            Channel::Main if to_u8(ship.main) != c => ship.main = f,
            Channel::Second if to_u8(ship.second) != c => ship.second = f,
            Channel::Trim if to_u8(ship.trim) != c => ship.trim = f,
            _ => {}
        }
    }

    fn remember(&mut self, c: [u8; 3]) {
        if self.recent.first() == Some(&c) {
            return;
        }
        self.recent.retain(|r| *r != c);
        self.recent.insert(0, c);
        self.recent.truncate(RECENT);
    }
}

// ---------------------------------------------------------------- colours

pub fn to_u8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

pub fn to_f32(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| v as f32 / 255.0)
}

fn colour(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

fn colour8(c: [u8; 3]) -> Color {
    colour(to_f32(c))
}

/// `#RRGGBB`.
pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// A typed colour number: `#C8642A`, `c8642a`, `#C64`, or three numbers
/// 0-255 (`200 100 42`, `200,100,42`).
pub fn parse_colour(text: &str) -> Option<[u8; 3]> {
    let t = text.trim();
    let digits = t.strip_prefix('#').unwrap_or(t);
    let is_hex = !digits.is_empty() && digits.chars().all(|c| c.is_ascii_hexdigit());
    // Three numbers, when they're written apart.
    let numbers: Vec<&str> = t.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).collect();
    if numbers.len() == 3 && (!is_hex || t.contains([' ', ','])) {
        let mut out = [0u8; 3];
        for (o, n) in out.iter_mut().zip(&numbers) {
            *o = n.parse::<u16>().ok().filter(|v| *v <= 255)? as u8;
        }
        return Some(out);
    }
    if !is_hex {
        return None;
    }
    let nibble = |c: char| c.to_digit(16).unwrap() as u8;
    let d: Vec<char> = digits.chars().collect();
    match d.len() {
        6 => Some([0, 2, 4].map(|i| nibble(d[i]) * 16 + nibble(d[i + 1]))),
        3 => Some([0, 1, 2].map(|i| nibble(d[i]) * 17)),
        _ => None,
    }
}

pub fn rgb_to_hsv(c: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = c;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max <= 0.0 { 0.0 } else { d / max };
    [h, s, max]
}

pub fn hsv_to_rgb(hsv: [f32; 3]) -> [f32; 3] {
    let [h, s, v] = hsv;
    let h6 = h.rem_euclid(1.0) * 6.0;
    let i = h6.floor();
    let f = h6 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// A grey has no hue, and black no saturation: keep the old ones so the
/// wheel's mark doesn't jump to red.
fn keep_hue(old: [f32; 3], mut new: [f32; 3]) -> [f32; 3] {
    if new[2] < 0.004 {
        new[0] = old[0];
        new[1] = old[1];
    } else if new[1] < 0.004 {
        new[0] = old[0];
    }
    new
}

/// The palette: plating greys, warm and cold paints, bright trims.
const PALETTE: [[u8; 3]; 24] = [
    [219, 219, 214], [179, 173, 161], [133, 133, 138], [92, 94, 102], [61, 64, 71], [36, 38, 43],
    [209, 194, 158], [189, 158, 115], [204, 163, 61], [235, 179, 66], [219, 117, 46], [168, 84, 46],
    [133, 33, 33], [242, 56, 46], [140, 89, 230], [191, 89, 242], [61, 79, 120], [102, 115, 166],
    [107, 217, 255], [64, 217, 191], [77, 204, 115], [128, 191, 102], [242, 217, 51], [250, 245, 235],
];

// ---------------------------------------------------------------- the window's parts

#[derive(Component)]
pub struct PaintWindow;
#[derive(Component)]
pub struct PaintToggleButton;
#[derive(Component)]
pub struct PaintBrushButton(Brush);
#[derive(Component)]
pub struct PaintChannelButton(Channel);
#[derive(Component)]
pub struct PaintSwatch([u8; 3]);
#[derive(Component)]
pub struct PaintSchemeButton(&'static str);
#[derive(Component)]
pub struct PaintWarmToggle;
#[derive(Component)]
pub struct PaintClearButton;
#[derive(Component)]
pub struct PaintWheel;
#[derive(Component)]
pub struct PaintWheelMark;
#[derive(Component)]
pub struct PaintValueBar;
#[derive(Component)]
pub struct PaintValueMark;
#[derive(Component)]
pub struct PaintPreview;
#[derive(Component)]
pub struct PaintHexField;
#[derive(Component)]
pub struct PaintHexText;
#[derive(Component)]
pub struct PaintRgbText;
#[derive(Component)]
pub struct PaintCountText;
/// Tiles or decals.
#[derive(Component)]
pub struct PaintTabButton(bool);
#[derive(Component)]
pub struct PaintShapeButton(DecalShape);
/// A shape button's picture, tinted with the brush colour.
#[derive(Component)]
pub struct PaintShapeIcon;
#[derive(Component)]
pub struct PaintLayerButton(u8);
#[derive(Component)]
pub struct PaintLayerEye(u8);

const WHEEL: f32 = 112.0;
const BAR_W: f32 = 16.0;

/// The pictures the picker is drawn with, made once.
#[derive(Clone)]
pub struct PickerImages {
    wheel: Handle<Image>,
    value: Handle<Image>,
}

impl PickerImages {
    fn make(images: &mut Assets<Image>) -> Self {
        // The wheel: hue around, saturation out from white at the middle.
        let n = 160u32;
        let mut px = Vec::with_capacity((n * n * 4) as usize);
        for y in 0..n {
            for x in 0..n {
                let p = Vec2::new((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
                let r = p.length();
                let hue = (-p.y).atan2(p.x) / std::f32::consts::TAU;
                let [cr, cg, cb] = hsv_to_rgb([hue, r.min(1.0), 1.0]);
                // A soft rim, one pixel wide.
                let a = ((1.0 - r) * n as f32 / 2.0 + 0.5).clamp(0.0, 1.0);
                px.extend([cr, cg, cb, a].map(|v| (v * 255.0).round() as u8));
            }
        }
        let wheel = Image::new(
            Extent3d { width: n, height: n, depth_or_array_layers: 1 },
            TextureDimension::D2,
            px,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        // The brightness bar: white at the top to black, tinted by the hue.
        let (bw, h) = (8u32, 128u32);
        let mut px = Vec::with_capacity((bw * h * 4) as usize);
        for y in 0..h {
            let v = 1.0 - y as f32 / (h - 1) as f32;
            let b = (v * 255.0).round() as u8;
            for _ in 0..bw {
                px.extend([b, b, b, 255]);
            }
        }
        let value = Image::new(
            Extent3d { width: bw, height: h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            px,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        Self { wheel: images.add(wheel), value: images.add(value) }
    }
}

// ---------------------------------------------------------------- systems

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
    hull: Query<(&HullSegment, &ChildOf)>,
    modules: Query<(&Module, &ChildOf)>,
) {
    if *state.get() == BuildState::Inactive {
        if paint.on {
            paint.on = false;
            paint.typing = None;
            paint.dragging = None;
        }
        return;
    }
    let clicked = toggles.iter().any(|i| *i == Interaction::Pressed);
    if !(keyboard.just_pressed(PAINT_KEY) || clicked) {
        return;
    }
    paint.on = !paint.on;
    paint.typing = None;
    paint.dragging = None;
    if paint.on {
        if let Ok((ship, false)) = ships.single() {
            let mut fresh = ShipPaint::default();
            // AFTERLIGHT_PAINT_DEMO=1 (filming): a few tiles brushed by hand
            // -- a trim stripe down the middle, second-colour bars, and tiles
            // in colours of their own -- as a player might.
            if starts_in_livery() {
                roll_livery(&mut fresh, demo_seed(), &own_cells(ship, &hull, &modules));
            }
            if std::env::var("AFTERLIGHT_PAINT_DEMO").ok().as_deref() == Some("2") {
                paint.decals = true;
                paint.own = [170, 44, 28];
                paint.shape = DecalShape::Slash;
            }
            if std::env::var("AFTERLIGHT_PAINT_DEMO").ok().as_deref() == Some("1") {
                paint.recent = PALETTE[12..18].to_vec();
                for x in -30i32..30 {
                    for y in -20i32..20 {
                        let slot = if y.abs() <= 1 && x % 4 != 0 {
                            Some(Slot::Trim)
                        } else if x.rem_euclid(8) < 2 && y.abs() < 9 {
                            Some(Slot::Second)
                        } else if (x + y).rem_euclid(9) == 0 && y.abs() > 3 {
                            Some(Slot::Custom(PALETTE[x.rem_euclid(6) as usize + 12]))
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

/// While the colour number field is open, the keyboard types into it and
/// nothing else hears it: the builder's keys (B, digits, Escape...) stay
/// quiet. Runs right after input is read.
pub fn type_colour_number(
    mut paint: ResMut<PaintMode>,
    mut keys: MessageReader<KeyboardInput>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut ships: Query<&mut ShipPaint, With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !paint.on || paint.typing.is_none() {
        keys.clear();
        return;
    }
    let mut done = None;
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        let Some(buffer) = paint.typing.as_mut() else { break };
        match &ev.logical_key {
            Key::Enter => done = Some(true),
            Key::Escape => done = Some(false),
            Key::Backspace => {
                buffer.pop();
            }
            _ => {
                if let Some(text) = &ev.text {
                    for c in text.chars() {
                        if (c.is_ascii_hexdigit() || matches!(c, '#' | ' ' | ',')) && buffer.len() < 16 {
                            buffer.push(c.to_ascii_uppercase());
                        }
                    }
                }
            }
        }
    }
    keyboard.reset_all();
    match done {
        Some(true) => {
            let text = paint.typing.take().unwrap_or_default();
            commit_typed(&mut paint, &mut ships, &text, &mut notifications);
        }
        Some(false) => paint.typing = None,
        None => {}
    }
}

fn commit_typed(paint: &mut PaintMode, ships: &mut Query<&mut ShipPaint, With<Ship>>, text: &str, notifications: &mut MessageWriter<ShowNotification>) {
    if text.trim().is_empty() {
        return;
    }
    match (parse_colour(text), ships.single_mut()) {
        (Some(c), Ok(mut ship)) => paint.set(&mut ship, c, None),
        (None, _) => {
            notifications.write(ShowNotification {
                message: format!("\"{text}\" isn't a colour: try #C8642A or 200 100 42."),
                notification_type: NotificationType::Warning,
                duration: 3.0,
            });
        }
        _ => {}
    }
}

/// Dragging the wheel or the brightness bar.
pub fn drag_picker(
    mut paint: ResMut<PaintMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    wheel: Query<&RelativeCursorPosition, With<PaintWheel>>,
    bar: Query<&RelativeCursorPosition, (With<PaintValueBar>, Without<PaintWheel>)>,
    mut ships: Query<&mut ShipPaint, With<Ship>>,
) {
    if !paint.on {
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        if paint.dragging.is_some() {
            paint.dragging = None;
        }
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        if wheel.iter().any(|r| r.cursor_over) {
            paint.dragging = Some(Drag::Wheel);
        } else if bar.iter().any(|r| r.cursor_over) {
            paint.dragging = Some(Drag::Value);
        }
    }
    let Some(drag) = paint.dragging else { return };
    let Ok(mut ship) = ships.single_mut() else { return };
    let mut hsv = paint.hsv;
    match drag {
        Drag::Wheel => {
            let Some(n) = wheel.iter().next().and_then(|r| r.normalized) else { return };
            let p = n * 2.0;
            hsv[0] = ((-p.y).atan2(p.x) / std::f32::consts::TAU).rem_euclid(1.0);
            hsv[1] = p.length().min(1.0);
            // A dark colour would hide the wheel's pick: bring it up.
            if hsv[2] < 0.25 {
                hsv[2] = 1.0;
            }
        }
        Drag::Value => {
            let Some(n) = bar.iter().next().and_then(|r| r.normalized) else { return };
            hsv[2] = (0.5 - n.y).clamp(0.0, 1.0);
        }
    }
    if hsv != paint.hsv {
        let c = to_u8(hsv_to_rgb(hsv));
        paint.set(&mut ship, c, Some(hsv));
    }
}

/// Paint the tile under the cursor, or drag across many.
#[allow(clippy::too_many_arguments)]
pub fn paint_tiles(
    mut paint: ResMut<PaintMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    symmetry: Res<super::symmetry::SymmetryState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut ships: Query<(&GlobalTransform, &mut ShipPaint), With<Ship>>,
    ui: Query<&RelativeCursorPosition, With<PaintWindow>>,
    mut stroke: Local<bool>,
) {
    if !paint.on || paint.dragging.is_some() {
        return;
    }
    let (left, right) = (mouse.pressed(MouseButton::Left), mouse.pressed(MouseButton::Right));
    // A stroke that starts on the paint window is the window's, all of it.
    if mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right) {
        *stroke = !ui.iter().any(|r| r.cursor_over);
    }
    if (!left && !right) || !*stroke {
        return;
    }
    let (Ok(window), Ok((cam, cam_gt)), Ok((ship_gt, mut ship_paint))) = (windows.single(), camera.single(), ships.single_mut()) else { return };
    let Some(cell) = super::cursor_to_ship_grid(window, cam, cam_gt, ship_gt) else { return };
    // Alt+click: take a painted tile's colour for the brush.
    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    if alt {
        if mouse.just_pressed(MouseButton::Left) {
            if let Some((_, slot)) = ship_paint.slots.iter().find(|(c, _)| *c == cell).copied() {
                let c = match slot {
                    Slot::Custom(c) => c,
                    Slot::Main => to_u8(ship_paint.main),
                    Slot::Second => to_u8(ship_paint.second),
                    Slot::Trim => to_u8(ship_paint.trim),
                    Slot::Bare => to_u8(BARE),
                };
                paint.editing = Channel::Own;
                paint.set(&mut ship_paint, c, None);
            }
        }
        return;
    }
    if paint.decals {
        let decal = Decal { cell, shape: paint.shape, turn: paint.turn, flip: false, colour: paint.own, layer: paint.layer };
        let mut put = vec![decal];
        if symmetry.enabled {
            let m = super::symmetry::mirror_position(cell);
            if m != cell {
                put.push(decal.mirrored(m));
            }
        }
        let mut painted = false;
        for d in put {
            let at = ship_paint.decals.iter().position(|e| e.cell == d.cell && e.layer == d.layer);
            match (at, right) {
                (Some(i), true) => {
                    ship_paint.decals.remove(i);
                }
                (Some(i), false) if ship_paint.decals[i] != d => {
                    ship_paint.decals[i] = d;
                    painted = true;
                }
                (None, false) => {
                    ship_paint.decals.push(d);
                    painted = true;
                }
                _ => {}
            }
        }
        if painted {
            let own = paint.own;
            paint.remember(own);
        }
        return;
    }
    let want = if right {
        None
    } else {
        match paint.brush {
            Brush::Own => Some(Slot::Custom(paint.own)),
            Brush::Paint(slot) => Some(slot),
            Brush::Auto => None,
        }
    };
    let mut cells = vec![cell];
    if symmetry.enabled {
        let m = super::symmetry::mirror_position(cell);
        if m != cell {
            cells.push(m);
        }
    }
    let mut painted = false;
    for cell in cells {
        let at = ship_paint.slots.iter().position(|(c, _)| *c == cell);
        // Only touch the paint when the tile changes: the roof is laid
        // again on every change.
        match (at, want) {
            (Some(i), Some(slot)) if ship_paint.slots[i].1 != slot => {
                ship_paint.slots[i].1 = slot;
                painted = true;
            }
            (Some(i), None) => {
                ship_paint.slots.swap_remove(i);
            }
            (None, Some(slot)) => {
                ship_paint.slots.push((cell, slot));
                painted = true;
            }
            _ => {}
        }
    }
    if painted && paint.brush == Brush::Own && !right {
        let own = paint.own;
        paint.remember(own);
    }
}

/// The paint window's buttons.
#[allow(clippy::type_complexity)]
pub fn paint_window_clicks(
    mut paint: ResMut<PaintMode>,
    mut ships: Query<&mut ShipPaint, With<Ship>>,
    mouse: Res<ButtonInput<MouseButton>>,
    hex_field: Query<&RelativeCursorPosition, With<PaintHexField>>,
    clicks: Query<
        (
            &Interaction,
            Option<&PaintBrushButton>,
            Option<&PaintChannelButton>,
            Option<&PaintSwatch>,
            Option<&PaintSchemeButton>,
            Has<PaintWarmToggle>,
            Has<PaintClearButton>,
            Has<PaintHexField>,
        ),
        Changed<Interaction>,
    >,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !paint.on {
        return;
    }
    // A click anywhere but the colour number's field closes it, keeping
    // what was typed.
    if paint.typing.is_some() && mouse.just_pressed(MouseButton::Left) && !hex_field.iter().any(|r| r.cursor_over) {
        let text = paint.typing.take().unwrap_or_default();
        commit_typed(&mut paint, &mut ships, &text, &mut notifications);
    }
    for (interaction, brush, channel, swatch, scheme, warm, clear, hex) in clicks.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(b) = brush {
            paint.brush = b.0;
            if b.0 == Brush::Own {
                paint.editing = Channel::Own;
            }
        }
        if let Some(c) = channel {
            paint.editing = c.0;
        }
        if hex {
            paint.typing = Some(String::new());
        }
        let Ok(mut ship_paint) = ships.single_mut() else { continue };
        if let Some(s) = swatch {
            paint.set(&mut ship_paint, s.0, None);
        }
        if let Some(name) = scheme.filter(|n| n.0 != "random") {
            let (slots, decals, hidden) = (std::mem::take(&mut ship_paint.slots), std::mem::take(&mut ship_paint.decals), ship_paint.hidden);
            *ship_paint = ShipPaint::from_scheme(scheme_named(name.0));
            ship_paint.slots = slots;
            ship_paint.decals = decals;
            ship_paint.hidden = hidden;
        }
        if warm {
            ship_paint.warm = !ship_paint.warm;
        }
        if clear {
            if paint.decals {
                let layer = paint.layer;
                ship_paint.decals.retain(|d| d.layer != layer);
            } else {
                ship_paint.slots.clear();
            }
        }
    }
}

/// What the window is laid out by: when any of it changes the window is
/// built again. Colours and counts are kept up to date in place
/// (live_paint_window), so dragging the wheel doesn't rebuild it.
#[derive(PartialEq, Clone)]
pub struct Layout {
    brush: Brush,
    editing: Channel,
    warm: bool,
    typing: bool,
    recent: Vec<[u8; 3]>,
    decals: bool,
    shape: DecalShape,
    turn: u8,
    layer: u8,
    hidden: u8,
}

/// Show the paint window while paint mode is on.
pub fn sync_paint_window(
    mut commands: Commands,
    paint: Res<PaintMode>,
    ships: Query<&ShipPaint, With<Ship>>,
    open: Query<Entity, With<PaintWindow>>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut picker: Local<Option<PickerImages>>,
    mut laid: Local<Option<Layout>>,
) {
    let ship_paint = ships.single().ok();
    let want = match (paint.on, ship_paint) {
        (true, Some(p)) => Some(Layout {
            brush: paint.brush,
            editing: paint.editing,
            warm: p.warm,
            typing: paint.typing.is_some(),
            recent: paint.recent.clone(),
            decals: paint.decals,
            shape: paint.shape,
            turn: paint.turn,
            layer: paint.layer,
            hidden: p.hidden,
        }),
        _ => None,
    };
    if want == *laid && want.is_some() == !open.is_empty() {
        return;
    }
    for e in open.iter() {
        commands.entity(e).try_despawn();
    }
    *laid = want.clone();
    let (Some(_), Some(ship_paint)) = (want, ship_paint) else { return };
    let pics = picker.get_or_insert_with(|| PickerImages::make(&mut images)).clone();
    spawn_paint_window(&mut commands, &paint, ship_paint, &pics, &assets);
}

/// Keep the window's colours, marks and numbers up to date.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn live_paint_window(
    mut paint: ResMut<PaintMode>,
    ships: Query<&ShipPaint, With<Ship>>,
    mut brushes: Query<(&PaintBrushButton, &mut BackgroundColor, &Children), Without<PaintChannelButton>>,
    mut channels: Query<(&PaintChannelButton, &mut BackgroundColor, &Children), Without<PaintBrushButton>>,
    mut label_colours: Query<&mut TextColor>,
    mut swatches: Query<(&PaintSwatch, &mut BorderColor, &mut Node)>,
    mut preview: Query<&mut BackgroundColor, (With<PaintPreview>, Without<PaintBrushButton>, Without<PaintChannelButton>)>,
    mut texts: Query<(&mut Text, Has<PaintHexText>, Has<PaintRgbText>, Has<PaintCountText>)>,
    mut marks: Query<(&mut Node, Has<PaintWheelMark>), (Or<(With<PaintWheelMark>, With<PaintValueMark>)>, Without<PaintSwatch>)>,
    mut pictures: Query<(&mut ImageNode, Has<PaintWheel>), Or<(With<PaintWheel>, With<PaintValueBar>)>>,
) {
    if !paint.on {
        return;
    }
    let Ok(ship) = ships.single() else { return };
    let current = paint.current(ship);
    // The colour changed by other means (a scheme, a swatch, a typed
    // number): the wheel follows it.
    if to_u8(hsv_to_rgb(paint.hsv)) != current {
        let hsv = keep_hue(paint.hsv, rgb_to_hsv(to_f32(current)));
        paint.hsv = hsv;
    }
    let hsv = paint.hsv;
    // A button's label dark on a light paint, light on a dark one.
    let mut ink = |children: &Children, c: Color| {
        let s = c.to_srgba();
        let light = 0.299 * s.red + 0.587 * s.green + 0.114 * s.blue > 0.6;
        let want = TextColor(if light { Color::srgb(0.06, 0.07, 0.1) } else { ThemeColors::TEXT_PRIMARY });
        for child in children.iter() {
            if let Ok(mut t) = label_colours.get_mut(child) {
                t.set_if_neq(want);
            }
        }
    };
    for (b, mut bg, children) in &mut brushes {
        let c = match b.0 {
            Brush::Own => colour8(paint.own),
            Brush::Paint(Slot::Main) => colour(ship.main),
            Brush::Paint(Slot::Second) => colour(ship.second),
            Brush::Paint(Slot::Trim) => colour(ship.trim),
            Brush::Paint(_) => colour(BARE),
            Brush::Auto => Sheet::ROW_PICKED,
        };
        bg.set_if_neq(BackgroundColor(c));
        ink(children, c);
    }
    for (ch, mut bg, children) in &mut channels {
        let c = match ch.0 {
            Channel::Own => colour8(paint.own),
            Channel::Main => colour(ship.main),
            Channel::Second => colour(ship.second),
            Channel::Trim => colour(ship.trim),
        };
        bg.set_if_neq(BackgroundColor(c));
        ink(children, c);
    }
    for (s, mut border, mut node) in &mut swatches {
        let picked = s.0 == current;
        border.set_if_neq(BorderColor::all(if picked { Color::WHITE } else { Sheet::RULE }));
        let w = UiRect::all(Val::Px(if picked { 2.0 } else { 1.0 }));
        if node.border != w {
            node.border = w;
        }
    }
    for mut bg in &mut preview {
        bg.set_if_neq(BackgroundColor(colour8(current)));
    }
    for (mut text, is_hex, is_rgb, is_count) in &mut texts {
        let want = if is_hex {
            match &paint.typing {
                Some(t) => format!("{t}_"),
                None => hex(current),
            }
        } else if is_rgb {
            format!("RGB {} {} {}", current[0], current[1], current[2])
        } else if is_count {
            if paint.decals {
                format!("{} decals ({} on layer {})", ship.decals.len(), ship.decals.iter().filter(|d| d.layer == paint.layer).count(), paint.layer)
            } else {
                format!("{} tiles painted by hand", ship.slots.len())
            }
        } else {
            continue;
        };
        if text.0 != want {
            text.0 = want;
        }
    }
    for (mut node, is_wheel) in &mut marks {
        let (left, top) = if is_wheel {
            let a = hsv[0] * std::f32::consts::TAU;
            ((0.5 + hsv[1] * a.cos() * 0.5) * WHEEL - 6.0, (0.5 - hsv[1] * a.sin() * 0.5) * WHEEL - 6.0)
        } else {
            (-3.0, (1.0 - hsv[2]) * WHEEL - 2.0)
        };
        if node.left != Val::Px(left) || node.top != Val::Px(top) {
            node.left = Val::Px(left);
            node.top = Val::Px(top);
        }
    }
    for (mut image, is_wheel) in &mut pictures {
        let tint = if is_wheel {
            // The wheel as dark as the colour: what a pick on it gives.
            let v = hsv[2].max(0.25);
            colour([v, v, v])
        } else {
            colour(hsv_to_rgb([hsv[0], hsv[1], 1.0]))
        };
        if image.color != tint {
            image.color = tint;
        }
    }
}

// ---------------------------------------------------------------- layout

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

fn spawn_paint_window(commands: &mut Commands, paint: &PaintMode, ship_paint: &ShipPaint, pics: &PickerImages, assets: &AssetServer) {
    let row = || Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(4.0), ..default() };
    // A tight shadow: the labels sit on any colour the player picks.
    let shadowed = |text: &str| {
        (label(text.to_string(), ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY), TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK.with_alpha(0.35) })
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(96.0),
                width: Val::Px(290.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Sheet::BG),
            BorderColor::all(Sheet::EDGE),
            Interaction::default(),
            RelativeCursorPosition::default(),
            PaintWindow,
        ))
        .with_children(|w| {
            w.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|t| {
                t.spawn((label("PAINT", ThemeFonts::BODY, Sheet::TITLE), Node { margin: UiRect::right(Val::Px(6.0)), ..default() }));
                for (decals, name) in [(false, "Tiles"), (true, "Decals")] {
                    t.spawn((button_node(72.0, 22.0, paint.decals == decals), BackgroundColor(Sheet::ROW_PICKED), PaintTabButton(decals))).with_children(|b| {
                        b.spawn(label(name, ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                    });
                }
            });
            if paint.decals {
                // Decals: the shape, turned with R, and which layer it goes on.
                w.spawn(label("SHAPE  (R turns it)", ThemeFonts::CAPTION, Sheet::LABEL));
                w.spawn(Node { column_gap: Val::Px(4.0), row_gap: Val::Px(4.0), ..row() }).with_children(|r| {
                    for shape in DecalShape::ALL {
                        r.spawn((button_node(41.0, 27.0, paint.shape == shape), BackgroundColor(Color::srgb(0.07, 0.08, 0.11)), PaintShapeButton(shape))).with_children(|b| {
                            b.spawn((
                                Node { width: Val::Px(22.0), height: Val::Px(22.0), ..default() },
                                ImageNode::new(assets.load(shape.path())).with_color(colour8(paint.own)),
                                UiTransform { rotation: Rot2::radians(-(paint.turn as f32) * std::f32::consts::FRAC_PI_2), ..default() },
                                PaintShapeIcon,
                            ));
                        });
                    }
                });
                w.spawn(label("LAYER  (the eye shows or hides it)", ThemeFonts::CAPTION, Sheet::LABEL));
                w.spawn(row()).with_children(|r| {
                    for layer in 1..=3u8 {
                        let shown = ship_paint.hidden & (1 << (layer - 1)) == 0;
                        r.spawn((button_node(56.0, 22.0, paint.layer == layer), BackgroundColor(Sheet::ROW_PICKED), PaintLayerButton(layer))).with_children(|b| {
                            b.spawn(label(format!("Layer {layer}"), ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                        });
                        r.spawn((button_node(26.0, 22.0, false), BackgroundColor(if shown { Sheet::ROW_PICKED } else { Color::srgb(0.05, 0.05, 0.07) }), PaintLayerEye(layer))).with_children(|b| {
                            b.spawn(label(if shown { "o" } else { "-" }, ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                        });
                    }
                });
            }
            w.spawn(Node { display: if paint.decals { Display::None } else { Display::Flex }, ..row() }).with_children(|r| {
                let brushes = [
                    (Brush::Own, "Colour", 56.0),
                    (Brush::Paint(Slot::Main), "Main", 38.0),
                    (Brush::Paint(Slot::Second), "2nd", 38.0),
                    (Brush::Paint(Slot::Trim), "Trim", 38.0),
                    (Brush::Paint(Slot::Bare), "Bare", 38.0),
                    (Brush::Auto, "Auto", 38.0),
                ];
                for (brush, name, width) in brushes {
                    r.spawn((button_node(width, 26.0, paint.brush == brush), BackgroundColor(Sheet::ROW_PICKED), PaintBrushButton(brush)))
                        .with_children(|b| {
                            b.spawn(shadowed(name));
                        });
                }
            });

            // Which colour the picker sets (decals take the brush colour).
            let tiles_only = || Node { display: if paint.decals { Display::None } else { Display::Flex }, ..default() };
            w.spawn((label("PICKING THE COLOUR OF", ThemeFonts::CAPTION, Sheet::LABEL), tiles_only()));
            w.spawn(Node { display: if paint.decals { Display::None } else { Display::Flex }, ..row() }).with_children(|r| {
                for channel in [Channel::Own, Channel::Main, Channel::Second, Channel::Trim] {
                    r.spawn((button_node(64.0, 22.0, paint.editing == channel), BackgroundColor(Sheet::ROW_PICKED), PaintChannelButton(channel)))
                        .with_children(|b| {
                            b.spawn(shadowed(channel.name()));
                        });
                }
            });

            // The wheel, its brightness bar, and the colour as a number.
            w.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), ..default() }).with_children(|r| {
                r.spawn((
                    Node { width: Val::Px(WHEEL), height: Val::Px(WHEEL), ..default() },
                    ImageNode::new(pics.wheel.clone()),
                    Interaction::default(),
                    RelativeCursorPosition::default(),
                    PaintWheel,
                ))
                .with_children(|wh| {
                    wh.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(12.0),
                            height: Val::Px(12.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BorderColor::all(Color::WHITE),
                        Outline::new(Val::Px(1.0), Val::ZERO, Color::BLACK),
                        PaintWheelMark,
                    ));
                });
                r.spawn((
                    Node { width: Val::Px(BAR_W), height: Val::Px(WHEEL), ..default() },
                    ImageNode::new(pics.value.clone()),
                    Outline::new(Val::Px(1.0), Val::ZERO, Sheet::RULE),
                    Interaction::default(),
                    RelativeCursorPosition::default(),
                    PaintValueBar,
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node { position_type: PositionType::Absolute, width: Val::Px(BAR_W + 6.0), height: Val::Px(4.0), border: UiRect::all(Val::Px(1.0)), ..default() },
                        BorderColor::all(Color::WHITE),
                        Outline::new(Val::Px(1.0), Val::ZERO, Color::BLACK),
                        PaintValueMark,
                    ));
                });
                r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), width: Val::Px(124.0), ..default() }).with_children(|c| {
                    c.spawn((
                        Node { width: Val::Px(124.0), height: Val::Px(30.0), border: UiRect::all(Val::Px(1.0)), ..default() },
                        BackgroundColor(Sheet::ROW_PICKED),
                        BorderColor::all(Sheet::EDGE),
                        PaintPreview,
                    ));
                    let fixed = || Node { width: Val::Px(124.0), ..default() };
                    c.spawn((label("COLOUR NUMBER", ThemeFonts::CAPTION, Sheet::LABEL), fixed()));
                    c.spawn((
                        button_node(124.0, 24.0, paint.typing.is_some()),
                        BackgroundColor(ThemeColors::BG_INPUT),
                        RelativeCursorPosition::default(),
                        PaintHexField,
                    ))
                    .with_children(|f| {
                        f.spawn((label("", ThemeFonts::BODY_SMALL, ThemeColors::TEXT_PRIMARY), PaintHexText));
                    });
                    c.spawn((label("", ThemeFonts::CAPTION, ThemeColors::TEXT_SECONDARY), PaintRgbText, fixed()));
                    c.spawn((
                        label(
                            if paint.typing.is_some() { "Type it, then Enter.\nEsc cancels." } else { "Click, then type\n#C8642A or\n200 100 42" },
                            ThemeFonts::CAPTION,
                            ThemeColors::TEXT_MUTED,
                        ),
                        fixed(),
                    ));
                });
            });

            // The palette.
            w.spawn(Node { column_gap: Val::Px(3.0), row_gap: Val::Px(3.0), ..row() }).with_children(|r| {
                for c in PALETTE {
                    r.spawn((button_node(19.5, 16.0, false), BackgroundColor(colour8(c)), PaintSwatch(c)));
                }
            });
            if !paint.recent.is_empty() {
                w.spawn(Node { column_gap: Val::Px(3.0), align_items: AlignItems::Center, ..row() }).with_children(|r| {
                    r.spawn((label("LAST", ThemeFonts::CAPTION, Sheet::LABEL), Node { width: Val::Px(32.0), ..default() }));
                    for c in &paint.recent {
                        r.spawn((button_node(20.0, 16.0, false), BackgroundColor(colour8(*c)), PaintSwatch(*c)));
                    }
                });
            }

            w.spawn(row()).with_children(|r| {
                for (name, key) in [("Salvager", "salvager"), ("Raider", "raider"), ("Navy", "navy"), ("Random", "random")] {
                    r.spawn((button_node(64.0, 22.0, false), BackgroundColor(Sheet::ROW_PICKED), PaintSchemeButton(key)))
                        .with_children(|b| {
                            b.spawn(label(name, ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                        });
                }
            });
            w.spawn(row()).with_children(|r| {
                r.spawn((button_node(133.0, 22.0, ship_paint.warm), BackgroundColor(Sheet::ROW_PICKED), PaintWarmToggle)).with_children(|b| {
                    b.spawn(label(if ship_paint.warm { "Warm rooms: on" } else { "Warm rooms: off" }, ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                });
                r.spawn((button_node(133.0, 22.0, false), BackgroundColor(Sheet::ROW_PICKED), PaintClearButton)).with_children(|b| {
                    b.spawn(label(if paint.decals { format!("Clear layer {}", paint.layer) } else { "Clear hand paint".to_string() }, ThemeFonts::CAPTION, ThemeColors::TEXT_PRIMARY));
                });
            });
            let full = || Node { width: Val::Px(270.0), ..default() };
            w.spawn((label("", ThemeFonts::CAPTION, ThemeColors::TEXT_SECONDARY), PaintCountText, full()));
            w.spawn((
                label(
                    if paint.decals { "Click: place.  Right-click: remove (this layer).\nR: turn.  Y: mirror.  N: leave." } else { "Drag: paint.  Right-click: auto.\nAlt+click: take a tile's colour.\nY: mirror.  N: leave." },
                    ThemeFonts::CAPTION,
                    ThemeColors::TEXT_MUTED,
                ),
                full(),
            ));
        });
}

/// The decal side of the paint window: tabs, shapes, layers and their eyes,
/// and R to turn the shape.
#[allow(clippy::type_complexity)]
#[allow(clippy::type_complexity)]
pub fn decal_window_clicks(
    mut paint: ResMut<PaintMode>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ships: Query<(Entity, &mut ShipPaint), With<Ship>>,
    clicks: Query<
        (&Interaction, Option<&PaintTabButton>, Option<&PaintShapeButton>, Option<&PaintLayerButton>, Option<&PaintLayerEye>, Option<&PaintSchemeButton>),
        Changed<Interaction>,
    >,
    mut icons: Query<&mut ImageNode, With<PaintShapeIcon>>,
    hull: Query<(&HullSegment, &ChildOf)>,
    modules: Query<(&Module, &ChildOf)>,
) {
    if !paint.on {
        return;
    }
    if paint.decals && paint.typing.is_none() && keyboard.just_pressed(KeyCode::KeyR) {
        paint.turn = (paint.turn + 1) % 4;
    }
    for (interaction, tab, shape, layer, eye, scheme) in clicks.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(t) = tab {
            paint.decals = t.0;
            if t.0 {
                paint.editing = Channel::Own;
            }
        }
        if let Some(s) = shape {
            paint.shape = s.0;
        }
        if let Some(l) = layer {
            paint.layer = l.0;
        }
        if let Some(e) = eye {
            if let Ok((_, mut ship_paint)) = ships.single_mut() {
                ship_paint.hidden ^= 1 << (e.0 - 1);
            }
        }
        // Random: a livery of its own -- palette and pattern -- every press.
        if scheme.is_some_and(|s| s.0 == "random") {
            if let Ok((ship, mut ship_paint)) = ships.single_mut() {
                let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
                roll_livery(&mut ship_paint, seed, &own_cells(ship, &hull, &modules));
            }
        }
    }
    // The shape buttons wear the brush colour.
    let tint = colour8(paint.own);
    for mut icon in &mut icons {
        if icon.color != tint {
            icon.color = tint;
        }
    }
}

/// Your ship's cells, for a livery laid over its shape.
fn own_cells(ship: Entity, hull: &Query<(&HullSegment, &ChildOf)>, modules: &Query<(&Module, &ChildOf)>) -> bevy::platform::collections::HashSet<IVec2> {
    let h: Vec<&HullSegment> = hull.iter().filter(|(_, p)| p.parent() == ship).map(|(h, _)| h).collect();
    let m: Vec<&Module> = modules.iter().filter(|(_, p)| p.parent() == ship).map(|(m, _)| m).collect();
    ship_cells(&h, &m)
}

/// A generated livery (vfx::paint_gen) on your ship: its palette for the
/// three paints and its pattern in decals, replacing the decals it had.
/// Tiles painted by hand stay.
pub fn roll_livery(paint: &mut ShipPaint, seed: u64, cells: &bevy::platform::collections::HashSet<IVec2>) {
    let palette = paint_gen::palette(seed);
    let s = palette.scheme();
    paint.main = s.main.to_array();
    paint.second = s.second.to_array();
    paint.trim = s.trim.to_array();
    paint.warm = false;
    paint.decals = paint_gen::livery_decals(seed, &palette, cells);
    paint.hidden = 0;
}

/// Your ship's livery until you paint it: by default livery 8 -- ice blue
/// plating, cyan flanks and a white pinstripe, the one picked from the
/// first preview. AFTERLIGHT_PAINT_SEED picks another (filming).
fn demo_seed() -> u64 {
    std::env::var("AFTERLIGHT_PAINT_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(8)
}

/// Whether your ship starts in a generated livery: with the Cosmoteer look,
/// or AFTERLIGHT_PAINT_DEMO=2.
fn starts_in_livery() -> bool {
    crate::vfx::ship_roof::cosmo() || std::env::var("AFTERLIGHT_PAINT_DEMO").ok().as_deref() == Some("2")
}

/// In flight, a ship never painted wears its starting livery (`demo_seed`)
/// when `starts_in_livery`. Painted once, it keeps its own paint.
pub fn demo_paint_in_flight(
    mut commands: Commands,
    ships: Query<Entity, (With<Ship>, Without<ShipPaint>)>,
    hull: Query<(&HullSegment, &ChildOf)>,
    modules: Query<(&Module, &ChildOf)>,
) {
    if !starts_in_livery() {
        return;
    }
    for ship in &ships {
        let cells = own_cells(ship, &hull, &modules);
        if cells.is_empty() {
            continue;
        }
        let mut paint = ShipPaint::default();
        roll_livery(&mut paint, demo_seed(), &cells);
        commands.entity(ship).insert(paint);
    }
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
        paint.slots.push((IVec2::new(4, -2), Slot::Custom([200, 100, 42])));
        paint.warm = true;
        let json = serde_json::to_string(&paint).unwrap();
        let back: ShipPaint = serde_json::from_str(&json).unwrap();
        assert_eq!(back, paint);
        assert_eq!(back.stamp(), paint.stamp());
    }

    #[test]
    fn paint_saved_before_own_colours_still_loads() {
        let json = r#"{"main":[0.5,0.5,0.5],"second":[0.1,0.2,0.3],"trim":[1.0,0.5,0.0],"warm":false,"slots":[[[1,2],"Trim"],[[0,0],"Bare"]]}"#;
        let paint: ShipPaint = serde_json::from_str(json).unwrap();
        assert_eq!(paint.slots, vec![(IVec2::new(1, 2), Slot::Trim), (IVec2::ZERO, Slot::Bare)]);
    }

    #[test]
    fn a_painted_tile_changes_the_stamp() {
        let mut paint = ShipPaint::from_scheme(scheme_named("navy"));
        let before = paint.stamp();
        paint.slots.push((IVec2::ZERO, Slot::Second));
        assert_ne!(paint.stamp(), before, "the roof wouldn't be laid again");
        let second = paint.stamp();
        paint.slots[0].1 = Slot::Custom([1, 2, 3]);
        let custom = paint.stamp();
        assert_ne!(custom, second);
        paint.slots[0].1 = Slot::Custom([1, 2, 4]);
        assert_ne!(paint.stamp(), custom, "a new colour on the same tile must lay the roof again");
    }

    #[test]
    fn an_older_design_without_paint_still_loads() {
        let json = r#"{"name":"old","hull_cells":[],"modules":[],"created_at":"x","version":2}"#;
        let bp: crate::building::blueprint::Blueprint = serde_json::from_str(json).unwrap();
        assert!(bp.paint.is_none());
    }

    #[test]
    fn colour_numbers_are_read_every_usual_way() {
        assert_eq!(parse_colour("#C8642A"), Some([200, 100, 42]));
        assert_eq!(parse_colour("c8642a"), Some([200, 100, 42]));
        assert_eq!(parse_colour("#C64"), Some([204, 102, 68]));
        assert_eq!(parse_colour("200 100 42"), Some([200, 100, 42]));
        assert_eq!(parse_colour("200,100,42"), Some([200, 100, 42]));
        assert_eq!(parse_colour(" 0, 0 ,255 "), Some([0, 0, 255]));
        assert_eq!(parse_colour("300 0 0"), None);
        assert_eq!(parse_colour("#C864"), None);
        assert_eq!(parse_colour("hello"), None);
        assert_eq!(parse_colour(""), None);
        assert_eq!(hex([200, 100, 42]), "#C8642A");
    }

    #[test]
    fn the_wheel_gives_back_the_colour_it_was_set_to() {
        for c in PALETTE {
            let hsv = rgb_to_hsv(to_f32(c));
            assert_eq!(to_u8(hsv_to_rgb(hsv)), c, "{c:?} -> {hsv:?}");
        }
        // Pure hues land where the wheel draws them.
        assert!(rgb_to_hsv([1.0, 0.0, 0.0])[0].abs() < 1e-5);
        assert!((rgb_to_hsv([0.0, 1.0, 0.0])[0] - 1.0 / 3.0).abs() < 1e-5);
        assert!((rgb_to_hsv([0.0, 0.0, 1.0])[0] - 2.0 / 3.0).abs() < 1e-5);
    }

    #[test]
    fn dragging_to_black_and_back_keeps_the_hue() {
        let orange = rgb_to_hsv(to_f32([214, 110, 46]));
        let black = keep_hue(orange, rgb_to_hsv([0.0, 0.0, 0.0]));
        assert_eq!(black[0], orange[0]);
        assert_eq!(black[1], orange[1]);
        assert_eq!(black[2], 0.0);
    }

    #[test]
    fn colours_used_last_are_kept_newest_first_without_repeats() {
        let mut p = PaintMode::default();
        for c in [[1, 0, 0], [2, 0, 0], [1, 0, 0]] {
            p.remember(c);
        }
        assert_eq!(p.recent, vec![[1, 0, 0], [2, 0, 0]]);
        for i in 0..20u8 {
            p.remember([i, 9, 9]);
        }
        assert_eq!(p.recent.len(), RECENT);
        assert_eq!(p.recent[0], [19, 9, 9]);
    }
}
