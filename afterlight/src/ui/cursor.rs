use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use super::theme::ThemeColors;

// ============================================================================
// CUSTOM CURSOR
// Replaces the default OS arrow with a small sci-fi reticle (ring + center
// dot + four tick marks) that tracks the real cursor position every frame.
// The OS cursor is hidden at the window level; this UI node is drawn on top
// of everything else instead.
// ============================================================================

#[derive(Component)]
pub(crate) struct CustomCursorIcon;

const CURSOR_SIZE: u32 = 28;

fn crosshair_image() -> Image {
    let size = CURSOR_SIZE;
    let mut data = vec![0u8; (size * size * 4) as usize];
    let center = size as f32 / 2.0;

    let Srgba { red, green, blue, .. } = ThemeColors::ACCENT_CYAN.to_srgba();
    let (r, g, b) = ((red * 255.0) as u8, (green * 255.0) as u8, (blue * 255.0) as u8);

    let outer_r = center - 3.0;
    let inner_r = outer_r * 0.42;
    let ring_thickness = 1.4;
    let dot_r = 1.5;
    let tick_len = 4.5;
    let tick_thickness = 1.2;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let ring_d = (dist - outer_r).abs();
            let ring_alpha = if ring_d < ring_thickness { 1.0 - ring_d / ring_thickness } else { 0.0 };

            let dot_alpha = if dist < dot_r { 1.0 } else { (dot_r + 1.0 - dist).clamp(0.0, 1.0) };

            let on_horiz_tick = dy.abs() < tick_thickness
                && dx.abs() > inner_r
                && dx.abs() < inner_r + tick_len;
            let on_vert_tick = dx.abs() < tick_thickness
                && dy.abs() > inner_r
                && dy.abs() < inner_r + tick_len;
            let tick_alpha = if on_horiz_tick || on_vert_tick { 1.0 } else { 0.0 };

            let alpha = ring_alpha.max(dot_alpha).max(tick_alpha);

            let idx = ((y * size + x) * 4) as usize;
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = (alpha * 255.0).round() as u8;
        }
    }

    Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Hides the OS cursor and spawns the reticle UI node that replaces it.
pub fn setup_custom_cursor(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut cursor_options: Query<&mut bevy::window::CursorOptions>,
) {
    if let Ok(mut options) = cursor_options.single_mut() {
        options.visible = false;
    }

    let handle = images.add(crosshair_image());
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(-100.0),
            top: Val::Px(-100.0),
            width: Val::Px(CURSOR_SIZE as f32),
            height: Val::Px(CURSOR_SIZE as f32),
            ..default()
        },
        ImageNode::new(handle),
        ZIndex(2000),
        CustomCursorIcon,
    ));
}

/// Tracks the real cursor position every frame. Hidden (rather than left
/// stale) whenever the cursor isn't over the window.
pub fn update_custom_cursor(
    windows_query: Query<&Window>,
    mut cursor_query: Query<&mut Node, With<CustomCursorIcon>>,
    state: Res<State<crate::states::GameState>>,
    ui_scale: Res<bevy::ui::UiScale>,
) {
    let Ok(window) = windows_query.single() else { return };
    let Ok(mut node) = cursor_query.single_mut() else { return };

    // The ending takes no input, so an aiming reticle floating over it is
    // just an object the player cannot use, sitting on top of the one screen
    // that has to be clean.
    if *state.get() == crate::states::GameState::Truth {
        node.display = Display::None;
        return;
    }

    match super::cursor_in_ui(window, &ui_scale) {
        Some(pos) => {
            node.display = Display::Flex;
            node.left = Val::Px(pos.x - CURSOR_SIZE as f32 / 2.0);
            node.top = Val::Px(pos.y - CURSOR_SIZE as f32 / 2.0);
        }
        None => node.display = Display::None,
    }
}
