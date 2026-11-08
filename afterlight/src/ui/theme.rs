use bevy::prelude::*;

// ============================================================================
// UNIFIED UI THEME
// Single source of truth for all game UI styling.
// Every color, font size, spacing, and border references this file.
// Dark sci-fi aesthetic — clean, readable, atmospheric.
// ============================================================================

/// Core color palette
pub struct ThemeColors;

impl ThemeColors {
    // --- Backgrounds (darkest to lightest) ---
    pub const BG_VOID: Color = Color::srgba(0.01, 0.02, 0.05, 0.98);       // Fullscreen overlays (pause)
    /// The main menu only. Thin enough to let the parallax starfield through.
    ///
    /// BG_VOID at 0.98 is right for the pause overlay, which has to hide a
    /// live battle behind it. On the menu it hid the starfield the game
    /// already draws, leaving text on a flat colour.
    pub const BG_MENU: Color = Color::srgba(0.01, 0.02, 0.05, 0.72);
    pub const BG_PANEL: Color = Color::srgba(0.04, 0.05, 0.10, 0.95);      // Panel backgrounds
    pub const BG_CARD: Color = Color::srgba(0.06, 0.08, 0.14, 0.92);       // Cards, group containers
    pub const BG_INPUT: Color = Color::srgba(0.08, 0.10, 0.18, 0.90);      // Input fields, slots
    pub const BG_ELEVATED: Color = Color::srgba(0.10, 0.13, 0.22, 0.95);   // Buttons, interactive elements
    pub const BG_HOVER: Color = Color::srgba(0.14, 0.17, 0.28, 1.0);       // Hover state
    pub const BG_PRESSED: Color = Color::srgba(0.18, 0.22, 0.35, 1.0);     // Pressed state

    // --- Borders ---
    pub const BORDER_SUBTLE: Color = Color::srgba(0.15, 0.18, 0.25, 0.5);  // Barely visible separation
    pub const BORDER_DEFAULT: Color = Color::srgba(0.22, 0.26, 0.35, 0.7); // Standard border
    pub const BORDER_BRIGHT: Color = Color::srgba(0.35, 0.40, 0.55, 0.8);  // Highlighted border
    pub const BORDER_ACTIVE: Color = Color::srgba(0.4, 0.6, 1.0, 0.8);     // Selected/active border

    // --- Text ---
    pub const TEXT_PRIMARY: Color = Color::srgba(0.88, 0.90, 0.95, 1.0);   // Main readable text
    pub const TEXT_SECONDARY: Color = Color::srgba(0.60, 0.64, 0.70, 1.0); // Labels, less important
    // Hints, disabled. Was 0.40/0.43/0.50 -- about 3.9:1 on the HUD bar, under
    // the 4.5:1 floor for small text, and the HUD's meter labels and control
    // hints are all small text. Nudged up just past the floor; still well
    // below TEXT_SECONDARY, so it keeps reading as the quiet tier.
    pub const TEXT_MUTED: Color = Color::srgba(0.46, 0.49, 0.56, 1.0);
    pub const TEXT_TITLE: Color = Color::srgba(0.70, 0.82, 1.0, 1.0);      // Titles, headers

    // --- Accent colors (functional) ---
    pub const ACCENT_BLUE: Color = Color::srgb(0.30, 0.55, 1.0);           // Primary action, links
    pub const ACCENT_CYAN: Color = Color::srgb(0.25, 0.75, 0.85);          // Oxygen, info
    pub const ACCENT_GREEN: Color = Color::srgb(0.30, 0.80, 0.45);         // Health, success, hull
    pub const ACCENT_YELLOW: Color = Color::srgb(0.90, 0.75, 0.25);        // Power, credits, warning
    pub const ACCENT_ORANGE: Color = Color::srgb(0.95, 0.55, 0.20);        // Fuel, medium warning
    pub const ACCENT_RED: Color = Color::srgb(0.90, 0.25, 0.25);           // Danger, damage
    pub const ACCENT_PURPLE: Color = Color::srgb(0.60, 0.45, 0.85);        // Special, systems

    // --- Status indicators ---
    pub const STATUS_OK: Color = Color::srgb(0.35, 0.75, 0.50);
    pub const STATUS_WARN: Color = Color::srgb(0.90, 0.70, 0.20);
    pub const STATUS_DANGER: Color = Color::srgb(0.90, 0.30, 0.25);
    pub const STATUS_CRITICAL: Color = Color::srgb(1.0, 0.15, 0.15);

    // --- HUD specific ---
    pub const HUD_BG: Color = Color::srgba(0.02, 0.03, 0.08, 0.88);
    /// Fully opaque HUD ground, for the panels that sit over the play area.
    ///
    /// The translucent HUD_BG is right where there is empty void behind it.
    /// It is wrong over the middle of the screen: the bottom toolbar and the
    /// weapon rack sit on top of the ship, the station and passing planets,
    /// and at 12% transmission a bright station module reads straight through
    /// a button. Controls have to look like controls, not like something the
    /// world is showing through.
    pub const HUD_BG_SOLID: Color = Color::srgb(0.02, 0.03, 0.08);
    pub const HUD_SEPARATOR: Color = Color::srgba(0.20, 0.23, 0.30, 0.4);

    // --- Notification backgrounds ---
    pub const NOTIF_INFO_BG: Color = Color::srgba(0.05, 0.08, 0.18, 0.90);
    pub const NOTIF_WARN_BG: Color = Color::srgba(0.18, 0.15, 0.03, 0.90);
    pub const NOTIF_DANGER_BG: Color = Color::srgba(0.25, 0.04, 0.04, 0.92);
    pub const NOTIF_SUCCESS_BG: Color = Color::srgba(0.03, 0.15, 0.06, 0.90);
}

/// Font size scale — consistent typographic hierarchy
pub struct ThemeFonts;

impl ThemeFonts {
    pub const DISPLAY: f32 = 64.0;    // Game title only
    pub const H1: f32 = 42.0;         // Screen titles (PAUSED, VICTORY)
    pub const H2: f32 = 24.0;         // Section headers
    pub const H3: f32 = 18.0;         // Sub-headers, large HUD values
    pub const BODY: f32 = 14.0;       // Standard readable text
    pub const BODY_SMALL: f32 = 12.0;  // Compact text, descriptions
    pub const CAPTION: f32 = 11.0;     // Labels, metadata
    pub const TINY: f32 = 10.0;        // Min/max values, subtle info (9 was below legible on the HUD)
}

/// Spacing constants — consistent rhythm
pub struct ThemeSpacing;

impl ThemeSpacing {
    pub const XS: f32 = 2.0;
    pub const SM: f32 = 4.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 12.0;
    pub const XL: f32 = 16.0;
    pub const XXL: f32 = 24.0;
    pub const SECTION: f32 = 32.0;
}

/// Border radii (Bevy 0.11 doesn't support border-radius on NodeBundle,
/// but we keep these for future use and as documentation)
pub struct ThemeBorders;

impl ThemeBorders {
    pub const THIN: f32 = 1.0;
    pub const DEFAULT: f32 = 1.5;
    pub const THICK: f32 = 2.0;
}

/// Animation durations (for systems that lerp colors/positions)
pub struct ThemeAnim;

impl ThemeAnim {
    pub const FAST: f32 = 0.1;
    pub const NORMAL: f32 = 0.2;
    pub const SLOW: f32 = 0.4;
    pub const NOTIFICATION_FADE: f32 = 0.5;
}

// ============================================================================
// HELPER FUNCTIONS — for consistent UI construction
// ============================================================================

/// Create a standard panel background node
pub fn panel_style() -> Node {
    Node {
        padding: UiRect::all(Val::Px(ThemeSpacing::MD)),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(ThemeSpacing::SM),
        ..default()
    }
}

/// Create a standard button style
pub fn button_style() -> Node {
    Node {
        padding: UiRect::new(
            Val::Px(ThemeSpacing::LG),
            Val::Px(ThemeSpacing::LG),
            Val::Px(ThemeSpacing::SM),
            Val::Px(ThemeSpacing::SM),
        ),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

/// Create a horizontal divider
pub fn spawn_divider(commands: &mut Commands, parent: Entity) {
    let divider = commands.spawn(
        (Node {
                width: Val::Percent(100.0),
                height: Val::Px(ThemeBorders::THIN),
                margin: UiRect::vertical(Val::Px(ThemeSpacing::SM)),
                ..default()
            }, BackgroundColor(ThemeColors::BORDER_SUBTLE)),
    ).id();
    commands.entity(parent).add_child(divider);
}

/// Create a section header text
pub fn spawn_section_header(commands: &mut Commands, parent: Entity, text: &str) {
    let header = commands.spawn(
        (Text::new(text.to_uppercase()), TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() }, TextColor(ThemeColors::TEXT_MUTED), Node { margin: UiRect::new(Val::Px(0.0), Val::Px(0.0), Val::Px(ThemeSpacing::SM), Val::Px(ThemeSpacing::XS)),
            ..default() }),
    ).id();
    commands.entity(parent).add_child(header);
}

/// Create a key hint (like "[ENTER] Start Game")
pub fn spawn_key_hint(commands: &mut Commands, parent: Entity, key: &str, action: &str) {
    let row = commands.spawn(
        (Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(ThemeSpacing::MD),
                align_items: AlignItems::Center,
                ..default()
            }),
    ).id();

    let key_badge = commands.spawn(
        (Node {
                padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(2.0), Val::Px(2.0)),
                min_width: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            }, BackgroundColor(ThemeColors::BG_ELEVATED)),
    ).id();

    let key_text = commands.spawn(
        (Text::new(key), TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() }, TextColor(ThemeColors::ACCENT_BLUE)),
    ).id();

    let action_text = commands.spawn(
        (Text::new(action), TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() }, TextColor(ThemeColors::TEXT_PRIMARY)),
    ).id();

    commands.entity(key_badge).add_child(key_text);
    commands.entity(row).add_children(&[key_badge, action_text]);
    commands.entity(parent).add_child(row);
}

/// Standard hover color transition for buttons
pub fn button_color_for_interaction(interaction: &Interaction) -> Color {
    match interaction {
        Interaction::Hovered => ThemeColors::BG_HOVER,
        Interaction::Pressed => ThemeColors::BG_PRESSED,
        Interaction::None => ThemeColors::BG_ELEVATED,
    }
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    fn luminance(c: Color) -> f32 {
        let l = c.to_linear();
        0.2126 * l.red + 0.7152 * l.green + 0.0722 * l.blue
    }

    fn contrast(fg: Color, bg: Color) -> f32 {
        let (a, b) = (luminance(fg), luminance(bg));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Muted text is what the HUD's small labels and control hints are drawn
    /// in, so it has to clear the small-text floor on every surface it sits on.
    #[test]
    fn muted_text_is_legible_on_hud_and_panels() {
        for (name, bg) in [
            ("HUD_BG_SOLID", ThemeColors::HUD_BG_SOLID),
            ("BG_PANEL", ThemeColors::BG_PANEL),
            ("BG_VOID", ThemeColors::BG_VOID),
        ] {
            let ratio = contrast(ThemeColors::TEXT_MUTED, bg);
            assert!(ratio >= 4.5, "TEXT_MUTED on {name} is {ratio:.2}:1");
        }
    }

    /// And it stays the quiet tier: clearly dimmer than secondary text.
    #[test]
    fn muted_stays_below_secondary() {
        assert!(luminance(ThemeColors::TEXT_MUTED) < 0.75 * luminance(ThemeColors::TEXT_SECONDARY));
    }
}
