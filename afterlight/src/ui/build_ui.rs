use bevy::prelude::*;
use smallvec::SmallVec;
use crate::states::BuildState;
use crate::resources::*;
use crate::components::*;
use crate::building::{GridOccupancy, ModuleRegistry};
use crate::events::*;
use crate::ui::theme::{ThemeColors, ThemeFonts};

// ============================================================================
// BUILD UI COLOR PALETTE — aliases into theme.rs (several of these were
// hand-copied near-duplicates of theme constants; now the real thing, so a
// future palette tweak in theme.rs propagates here instead of silently
// diverging), plus a few build-specific additions with no theme equivalent.
// ============================================================================

pub const COLOR_BG_DARK: Color = ThemeColors::BG_PANEL;
pub const COLOR_BG_PANEL: Color = ThemeColors::BG_INPUT;
pub const COLOR_BG_PANEL_LIGHT: Color = ThemeColors::BG_ELEVATED;
pub const COLOR_BORDER: Color = ThemeColors::BORDER_DEFAULT;
pub const COLOR_BORDER_LIGHT: Color = ThemeColors::BORDER_BRIGHT;
pub const COLOR_TITLE_BAR: Color = ThemeColors::BG_INPUT;
pub const COLOR_BUTTON: Color = ThemeColors::BG_ELEVATED;
pub const COLOR_BUTTON_HOVER: Color = ThemeColors::BG_HOVER;
pub const COLOR_BUTTON_PRESSED: Color = ThemeColors::BG_PRESSED;
pub const COLOR_BUTTON_ACTIVE: Color = ThemeColors::ACCENT_BLUE;
pub const COLOR_GRID_EMPTY: Color = ThemeColors::BG_PANEL;
pub const _COLOR_GRID_OCCUPIED: Color = Color::srgb(0.15, 0.40, 0.25);
pub const COLOR_GRID_HOVER: Color = ThemeColors::BG_HOVER;
pub const COLOR_TEXT_PRIMARY: Color = ThemeColors::TEXT_PRIMARY;
pub const COLOR_TEXT_SECONDARY: Color = ThemeColors::TEXT_SECONDARY;
pub const COLOR_TEXT_ACTIVE: Color = ThemeColors::ACCENT_BLUE;
pub const _COLOR_WARNING: Color = ThemeColors::ACCENT_ORANGE;
pub const _COLOR_SUCCESS: Color = ThemeColors::ACCENT_GREEN;
pub const COLOR_DANGER: Color = ThemeColors::ACCENT_RED;
pub const COLOR_COMPONENT_WEAPON: Color = Color::srgb(0.85, 0.35, 0.25);
pub const COLOR_COMPONENT_ENGINE: Color = Color::srgb(0.35, 0.65, 0.85);
pub const COLOR_COMPONENT_REACTOR: Color = Color::srgb(0.85, 0.75, 0.25);
pub const COLOR_COMPONENT_LIFE: Color = Color::srgb(0.35, 0.85, 0.55);

// ============================================================================
// MARKER COMPONENTS
// ============================================================================

#[derive(Component)]
pub(crate) struct BuildGridLine;

#[derive(Component)]
pub(crate) struct ModuleBuildOutline;

#[derive(Component)]
pub(crate) struct PowerFlowIndicator;

#[derive(Component)]
pub(crate) struct ModuleTooltip;

#[derive(Component)]
pub(crate) struct DeleteHighlight;

#[derive(Component)]
pub(crate) struct BuildPanelRoot;

#[derive(Component)]
pub(crate) struct BuildModeText;

#[derive(Component)]
pub(crate) struct BuildItemName;

#[derive(Component)]
pub(crate) struct BuildStatsText;

#[derive(Component)]
pub(crate) struct BuildRotationText;

#[derive(Component)]
pub(crate) struct BuildMaterialText;

#[derive(Component)]
pub(crate) struct BuildDescText;

#[derive(Component)]
pub(crate) struct BuildSummaryText;

#[derive(Component)]
pub(crate) struct ControlsHelpText;

/// The build-mode instructions, stacked down the right-hand edge.
///
/// They used to share the HUD's bottom strip with flight controls, which the
/// build panel then covered -- the hints were legible only as ghosts behind
/// the item slots. A vertical list at the side has room for one instruction
/// per line and never fights the panel for the same pixels.
#[derive(Component)]
pub(crate) struct BuildHintText;

#[derive(Component)]
pub(crate) struct CategoryTab {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct CategoryTabBg;

/// The icon image inside a category tab, so the highlight system can tint it
/// along with the label.
#[derive(Component)]
pub(crate) struct CategoryTabIcon;

#[derive(Component)]
pub(crate) struct ItemSlot {
    pub index: usize,
}

#[derive(Component)]
pub(crate) struct ItemSlotBg;

#[derive(Component)]
pub(crate) struct ItemSlotsContainer;

/// Marker for the customization panel root
#[derive(Component)]
pub(crate) struct CustomizationPanelRoot;

/// Marker for a customization slider with its property key
#[derive(Component)]
pub(crate) struct CustomizationSlider {
    pub property_key: String,
}

/// Marker for stat display elements in customization panel
#[derive(Component)]
pub(crate) struct CustomizationStatDisplay {
    pub stat_name: String,
}

/// Marker for slider value text
#[derive(Component)]
pub(crate) struct SliderValueText {
    pub property_key: String,
}

/// Marker for build validation reason text (world-space)
#[derive(Component)]
pub(crate) struct BuildValidationText;

/// Marker for component placement panel root
#[derive(Component)]
pub(crate) struct ComponentPlacementPanelRoot;

/// Marker for component palette item
#[derive(Component)]
pub(crate) struct ComponentPaletteItem {
    pub piece_type: ComponentPieceType,
}

/// Marker for internal grid cell
#[derive(Component)]
pub(crate) struct InternalGridCell {
    pub grid_pos: IVec2,
}

/// Marker for piece context menu
#[derive(Component)]
pub(crate) struct PieceContextMenu;

/// Marker for context menu option button
#[derive(Component)]
pub(crate) struct ContextMenuOption {
    pub option_type: ContextMenuOptionType,
}

/// Types of context menu options
#[derive(Clone, Debug)]
pub enum ContextMenuOptionType {
    CustomizeOne,
    CustomizeGroup(usize), // count of pieces in group
    Remove,
}

/// Marker for piece customization panel (when customizing from placed pieces)
#[derive(Component)]
pub(crate) struct PieceCustomizationPanelRoot;

/// Marker for customization slider in piece panel
#[derive(Component)]
#[allow(dead_code)]
pub(crate) struct PieceCustomizationSlider {
    pub property_key: String,
}

// ============================================================================
// GHOST PREVIEW (world-space sprite in Placing mode)
// ============================================================================

pub fn spawn_build_ghost(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    build_state: Res<BuildingState>,
    ship_query: Query<Entity, With<Ship>>,
) {
    let Ok(ship) = ship_query.single() else { return };

    let path = match build_state.current_selection() {
        BuildSelection::Hull(_) => {
            Some(crate::sprite_map::hull_sprite_path(build_state.hull_material))
        }
        BuildSelection::Module(mt) => {
            crate::sprite_map::module_sprite_path(mt)
        }
    };
    let texture = path.map(|p| asset_server.load(p)).unwrap_or_default();

    // Parented to the ship (same fix as the grid lines — see
    // spawn_build_grid_lines) so ghost_pos-based positioning below lands on
    // the ship's actual local grid instead of world-space-absolute
    // coordinates that only lined up when the ship sat at world origin.
    commands.spawn((
        (Sprite {
                image: texture,
                color: Color::srgba(0.0, 1.0, 0.0, 0.4),
                custom_size: Some(Vec2::new(48.0, 48.0)),
                ..default()
            }, Transform::from_xyz(0.0, 0.0, 0.3), Visibility::Hidden),
        BuildGhost,
        ChildOf(ship),
    ));

    // Extra tiles for non-rectangular footprints (see BuildGhostCell) — hidden
    // unless the current selection has a footprint override with more cells.
    // Supports up to 5-cell shapes (the plus-pentomino) — 1 main + 4 extra.
    for i in 1..5 {
        commands.spawn((
            (Sprite {
                    color: Color::srgba(0.0, 1.0, 0.0, 0.4),
                    custom_size: Some(Vec2::new(60.0, 60.0)),
                    ..default()
                }, Transform::from_xyz(0.0, 0.0, 0.3), Visibility::Hidden),
            BuildGhostCell(i),
            ChildOf(ship),
        ));
    }

    // Validation reason text (ship-local, follows ghost)
    commands.spawn((
        Text2d::new(""),
        TextFont { font_size: FontSize::Px(13.0), ..default() },
        TextColor(Color::srgb(1.0, 0.4, 0.4)),
        Transform::from_xyz(0.0, -40.0, 0.35),
        Visibility::Hidden,
        BuildValidationText,
        ChildOf(ship),
    ));
}

pub fn despawn_build_ghost(
    mut commands: Commands,
    query: Query<Entity, With<BuildGhost>>,
    cell_query: Query<Entity, With<BuildGhostCell>>,
    validation_query: Query<Entity, With<BuildValidationText>>,
    // The hover tooltip is only refreshed while Placing, so leaving with the
    // cursor on a module stranded the last one in space for good.
    tooltip_query: Query<Entity, With<ModuleTooltip>>,
) {
    for entity in query.iter().chain(cell_query.iter()).chain(validation_query.iter()).chain(tooltip_query.iter()) {
        commands.entity(entity).despawn();
    }
}

pub fn update_build_ghost(
    build_state: Res<BuildingState>,
    registry: Res<ModuleRegistry>,
    mut ghost_query: Query<(&mut Transform, &mut Sprite, &mut Visibility), (With<BuildGhost>, Without<BuildValidationText>, Without<BuildGhostCell>)>,
    mut cell_query: Query<(&BuildGhostCell, &mut Transform, &mut Sprite, &mut Visibility), (Without<BuildGhost>, Without<BuildValidationText>)>,
    mut validation_query: Query<(&mut Transform, &mut Text, &mut Visibility, &mut TextColor), (With<BuildValidationText>, Without<BuildGhost>, Without<BuildGhostCell>)>,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut last_selection: Local<Option<String>>,
) {
    let Ok((mut transform, mut sprite, mut visibility)) = ghost_query.single_mut() else {
        return;
    };

    // Nothing under the cursor, nothing to preview. Without this the ghost
    // parked at the ship's origin -- the exact centre of the screen, because
    // the camera follows the ship -- and pulsed there for the whole session,
    // whether or not the player was pointing at anything.
    // A pre-made weapon shows its own ghost, every part (building::gun_stamp).
    if !build_state.cursor_on_grid || build_state.premade_index().is_some() {
        *visibility = Visibility::Hidden;
        for (_, _, _, mut c_vis) in cell_query.iter_mut() {
            *c_vis = Visibility::Hidden;
        }
        if let Ok((_, _, mut v_vis, _)) = validation_query.single_mut() {
            *v_vis = Visibility::Hidden;
        }
        return;
    }
    *visibility = Visibility::Visible;

    let selection = build_state.current_selection();
    let ghost_pos = build_state.ghost_position;
    let rotation = build_state.rotation;
    // Over a turret deck the ghost has to stand above the turret (z 1.2 and
    // up), not down among the hull blocks under it.
    let ghost_z = if build_state.deck != crate::resources::BuildDeck::Hull { 2.5 } else { 0.3 };

    // Footprint cells beyond the first (index 0, covered by the main ghost sprite
    // above) — populated only for modules with a non-rectangular footprint override.
    let mut extra_cells: SmallVec<[IVec2; 4]> = SmallVec::new();

    match selection {
        BuildSelection::Hull(_) => {
            transform.translation.x = ghost_pos.x as f32 * 66.0;
            transform.translation.y = ghost_pos.y as f32 * 66.0 - 33.0;
            transform.translation.z = ghost_z;
            transform.rotation = Quat::IDENTITY;
            sprite.custom_size = Some(Vec2::new(64.0, 64.0));
        }
        BuildSelection::Module(mt) => {
            let def = registry.get(mt);
            let footprint = crate::building::footprints::footprint_override(mt);
            let cells = GridOccupancy::cells_for(ghost_pos, def.size, rotation, footprint);

            if let Some(_offsets) = footprint {
                // Non-rectangular: main sprite covers just the first cell,
                // extra tiles (below) cover the rest — never over-claims a cell.
                let first = cells.first().copied().unwrap_or(ghost_pos);
                transform.translation.x = first.x as f32 * 66.0;
                transform.translation.y = first.y as f32 * 66.0 - 33.0;
                transform.translation.z = ghost_z;
                transform.rotation = Quat::IDENTITY;
                sprite.custom_size = Some(Vec2::new(60.0, 60.0));
                extra_cells = cells.iter().skip(1).copied().collect();
            } else {
                let (min_x, max_x, min_y, max_y) = cells.iter().fold(
                    (i32::MAX, i32::MIN, i32::MAX, i32::MIN),
                    |(mnx, mxx, mny, mxy), c| {
                        (mnx.min(c.x), mxx.max(c.x), mny.min(c.y), mxy.max(c.y))
                    },
                );
                let center_x = (min_x as f32 + max_x as f32) / 2.0 * 66.0;
                let center_y = (min_y as f32 + max_y as f32) / 2.0 * 66.0 - 33.0;

                let visual_angle = rotation.to_radians()
                    + crate::sprite_map::sprite_base_rotation(mt);
                // Un-rotate the rotated cell bounds by the FINAL visual
                // angle (cell rotation + texture base offset) — see
                // spawn_module for the full story; anything else lays
                // multi-cell ghosts 90° across their claimed cells.
                let bounds_w = (max_x - min_x) as f32;
                let bounds_h = (max_y - min_y) as f32;
                let quarter = ((visual_angle / std::f32::consts::FRAC_PI_2).round() as i32)
                    .rem_euclid(4);
                let (cells_w, cells_h) = if quarter % 2 == 1 {
                    (bounds_h, bounds_w)
                } else {
                    (bounds_w, bounds_h)
                };
                let sprite_w = 48.0 + cells_w * 66.0;
                let sprite_h = 48.0 + cells_h * 66.0;

                transform.translation.x = center_x;
                transform.translation.y = center_y;
                transform.translation.z = ghost_z;
                transform.rotation = Quat::from_rotation_z(visual_angle);
                sprite.custom_size = Some(Vec2::new(sprite_w, sprite_h));
            }
        }
    }

    // Swap texture when selection changes (sprite-based ghost preview)
    let selection_key = build_state.selection_name().to_string();
    if *last_selection != Some(selection_key.clone()) {
        *last_selection = Some(selection_key);
        let path = match build_state.current_selection() {
            BuildSelection::Hull(_) => {
                Some(crate::sprite_map::hull_sprite_path(build_state.hull_material))
            }
            BuildSelection::Module(mt) => {
                crate::sprite_map::module_sprite_path(mt)
            }
        };
        if let Some(p) = path {
            sprite.image = asset_server.load(p);
        }
    }

    // Animated pulse with category-colored tint — shared by the main ghost
    // sprite and any extra footprint tiles so they read as one shape.
    // A slow breath, not a flash. This was 0.45 +- 0.15 at four radians a
    // second, which reads as blinking rather than as a highlight.
    let pulse = 0.46 + 0.06 * (time.elapsed_secs() * 1.6).sin();
    let tile_color = if build_state.is_valid_placement {
        let cat_color = category_color(build_state.current_category());
        Color::srgba(cat_color.to_srgba().red, cat_color.to_srgba().green, cat_color.to_srgba().blue, pulse)
    } else {
        Color::srgba(1.0, 0.0, 0.0, pulse * 0.7)
    };
    sprite.color = tile_color;

    // Position/show extra footprint tiles, hide unused slots
    for (cell_marker, mut c_transform, mut c_sprite, mut c_vis) in cell_query.iter_mut() {
        if let Some(&cell) = extra_cells.get(cell_marker.0 - 1) {
            c_transform.translation.x = cell.x as f32 * 66.0;
            c_transform.translation.y = cell.y as f32 * 66.0 - 33.0;
            c_transform.translation.z = ghost_z;
            c_sprite.color = tile_color;
            *c_vis = Visibility::Visible;
        } else {
            *c_vis = Visibility::Hidden;
        }
    }

    // Update validation reason text position and content
    if let Ok((mut v_transform, mut v_text, mut v_vis, mut v_color)) = validation_query.single_mut() {
        if let Some(reason) = &build_state.placement_reason {
            v_transform.translation = Vec3::new(
                transform.translation.x,
                transform.translation.y - 40.0,
                ghost_z + 0.05,
            );
            v_text.0 = reason.clone();
            // Red refuses the placement; amber allows it and warns. A blocked
            // silo is legal to build and reads differently from "you cannot
            // put that there".
            v_color.0 = if build_state.is_valid_placement {
                Color::srgb(1.0, 0.75, 0.25)
            } else {
                Color::srgb(1.0, 0.4, 0.4)
            };
            *v_vis = Visibility::Visible;
        } else {
            *v_vis = Visibility::Hidden;
        }
    }
}

// ============================================================================
// DELETE HIGHLIGHT (world-space sprite in Deleting mode)
// ============================================================================

pub fn spawn_delete_highlight(
    mut commands: Commands,
    ship_query: Query<Entity, With<Ship>>,
) {
    let Ok(ship) = ship_query.single() else { return };
    // Parented to the ship — same fix as spawn_build_ghost.
    commands.spawn((
        (Sprite {
                color: Color::srgba(1.0, 0.0, 0.0, 0.3),
                custom_size: Some(Vec2::new(64.0, 64.0)),
                ..default()
            }, Transform::from_xyz(0.0, 0.0, 0.3), Visibility::Hidden),
        DeleteHighlight,
        ChildOf(ship),
    ));
}

pub fn despawn_delete_highlight(
    mut commands: Commands,
    query: Query<Entity, With<DeleteHighlight>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn update_delete_highlight(
    build_state: Res<BuildingState>,
    occupancy: Res<GridOccupancy>,
    mut query: Query<(&mut Transform, &mut Sprite, &mut Visibility), With<DeleteHighlight>>,
) {
    let Ok((mut transform, mut sprite, mut visibility)) = query.single_mut() else {
        return;
    };

    *visibility = Visibility::Visible;

    let ghost_pos = build_state.ghost_position;
    transform.translation.x = ghost_pos.x as f32 * 66.0;
    transform.translation.y = ghost_pos.y as f32 * 66.0 - 33.0;
    transform.translation.z = if build_state.deck != crate::resources::BuildDeck::Hull { 2.5 } else { 0.3 };
    sprite.custom_size = Some(Vec2::new(64.0, 64.0));

    if occupancy.cells.contains_key(&ghost_pos) {
        sprite.color = Color::srgba(1.0, 0.1, 0.1, 0.5);
    } else {
        sprite.color = Color::srgba(1.0, 0.0, 0.0, 0.15);
    }
}

// ============================================================================
// GRID LINES OVERLAY
// ============================================================================

pub fn spawn_build_grid_lines(
    mut commands: Commands,
    ship_query: Query<Entity, With<Ship>>,
) {
    let Ok(ship) = ship_query.single() else { return };

    let grid_size = 66.0_f32;
    let extent = 10; // 10 cells in each direction
    let line_color = Color::srgba(0.3, 0.4, 0.5, 0.15);

    // Parented to the ship — these lines are drawn at ship-LOCAL coordinates
    // (matching hull/module tiles, which are also ship children), so they
    // inherit the ship's actual position and rotation instead of sitting
    // fixed at world origin. Previously world-space-absolute, so the grid
    // only lined up with the ship when it happened to be sitting exactly at
    // (0,0) with zero rotation — never true once you've actually flown
    // anywhere before opening the builder.

    // Vertical lines
    for x in -extent..=extent {
        commands.spawn((
            (Sprite {
                    color: line_color,
                    custom_size: Some(Vec2::new(1.0, grid_size * (extent * 2 + 1) as f32)),
                    ..default()
                }, Transform::from_xyz(
                    x as f32 * grid_size - grid_size * 0.5,
                    -33.0,
                    0.05,
                )),
            BuildGridLine,
            ChildOf(ship),
        ));
    }

    // Horizontal lines
    for y in -extent..=extent {
        commands.spawn((
            (Sprite {
                    color: line_color,
                    custom_size: Some(Vec2::new(grid_size * (extent * 2 + 1) as f32, 1.0)),
                    ..default()
                }, Transform::from_xyz(
                    0.0,
                    y as f32 * grid_size - 33.0 - grid_size * 0.5,
                    0.05,
                )),
            BuildGridLine,
            ChildOf(ship),
        ));
    }
}

pub fn despawn_build_grid_lines(
    mut commands: Commands,
    query: Query<Entity, With<BuildGridLine>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// MODULE OUTLINES IN BUILD MODE
// ============================================================================

pub fn spawn_module_outlines(
    mut commands: Commands,
    module_query: Query<(&Module, &ChildOf)>,
    ship_query: Query<Entity, With<Ship>>,
    registry: Res<ModuleRegistry>,
) {
    let Ok(ship) = ship_query.single() else { return };

    for (module, parent) in module_query.iter() {
        // Player's own ship only — Module is shared with AI ships, and this
        // used to draw an outline for every module in the world regardless
        // of owner.
        if parent.parent() != ship { continue; }

        let def = registry.get(module.module_type);
        let cat = module.module_type.category();
        let cat_color = module_category_color(cat);

        // Sized and placed from the CELLS the module occupies, not from its
        // sprite transform. A weapon's sprite is nudged outward so its barrel
        // overhangs the hull, and copying that offset put the outline off its
        // own module by however far the barrel stuck out -- which is why only
        // some of them looked wrong. Rotation matters too: a 3x2 bridge turned
        // east occupies a 2x3 footprint, and the raw `def.size` describes
        // neither its extent nor its centre once it is turned.
        let cells = crate::building::ShipGrid::cells_for(
            module.grid_position,
            def.size,
            module.rotation,
            crate::building::footprints::footprint_override(module.module_type),
        );
        let (mut lo, mut hi) = (IVec2::MAX, IVec2::MIN);
        for c in cells.iter() {
            lo = lo.min(*c);
            hi = hi.max(*c);
        }
        let span = (hi - lo) + IVec2::ONE;
        let outline_size = Vec2::new(
            span.x as f32 * 66.0 + 6.0,
            span.y as f32 * 66.0 + 6.0,
        );
        // Same cell convention as every block: (x*66, y*66 - 33).
        let centre = Vec2::new(
            (lo.x as f32 + hi.x as f32) * 0.5 * 66.0,
            (lo.y as f32 + hi.y as f32) * 0.5 * 66.0 - 33.0,
        );

        // Parented to the ship (same fix as spawn_build_grid_lines) — this
        // copies the module's ship-LOCAL translation but was spawning
        // world-space-absolute, so it only lined up with its module when
        // the ship sat at world origin with zero rotation.
        commands.spawn((
            (Sprite {
                    color: Color::srgba(cat_color.to_srgba().red, cat_color.to_srgba().green, cat_color.to_srgba().blue, 0.4),
                    custom_size: Some(outline_size),
                    ..default()
                }, Transform::from_xyz(centre.x, centre.y, 0.15)),
            ModuleBuildOutline,
            ChildOf(ship),
        ));
    }
}

pub fn despawn_module_outlines(
    mut commands: Commands,
    query: Query<Entity, With<ModuleBuildOutline>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// POWER FLOW INDICATORS
// ============================================================================

pub fn spawn_power_indicators(
    mut commands: Commands,
    module_query: Query<(&Module, &Transform, &ChildOf)>,
    ship_query: Query<Entity, With<Ship>>,
    registry: Res<ModuleRegistry>,
) {
    let Ok(ship) = ship_query.single() else { return };

    for (module, module_transform, parent) in module_query.iter() {
        // Player's own ship only (same reasoning as spawn_module_outlines).
        if parent.parent() != ship { continue; }

        let def = registry.get(module.module_type);
        if def.power_generation <= 0.0 && def.power_consumption <= 0.0 {
            continue;
        }

        let (text_str, color) = if def.power_generation > 0.0 {
            (format!("+{:.0}", def.power_generation), Color::srgb(0.3, 0.9, 0.3))
        } else {
            (format!("-{:.0}", def.power_consumption), Color::srgb(0.9, 0.3, 0.3))
        };

        // Parented to the ship (same fix as spawn_module_outlines) —
        // was world-space-absolute using ship-local translation values.
        commands.spawn((
            Text2d::new(text_str),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(color),
            TextLayout::justify(Justify::Center),
            Transform::from_xyz(
                module_transform.translation.x,
                module_transform.translation.y + 30.0,
                0.5,
            ),
            PowerFlowIndicator,
            ChildOf(ship),
        ));
    }
}

pub fn despawn_power_indicators(
    mut commands: Commands,
    query: Query<Entity, With<PowerFlowIndicator>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// MODULE HOVER TOOLTIP
// ============================================================================

pub fn update_module_tooltip(
    mut commands: Commands,
    existing: Query<Entity, With<ModuleTooltip>>,
    build_state: Res<BuildingState>,
    occupancy: Res<GridOccupancy>,
    module_query: Query<&Module>,
    registry: Res<ModuleRegistry>,
    mouse: Res<ButtonInput<MouseButton>>,
    ship_query: Query<Entity, With<Ship>>,
    guns: Query<&crate::combat::gun_assembly::GunAssembly>,
) {
    // Despawn old tooltip
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    let Ok(ship) = ship_query.single() else { return };

    // Don't show tooltip while clicking
    if mouse.pressed(MouseButton::Left) || mouse.pressed(MouseButton::Right) {
        return;
    }

    let ghost_pos = build_state.ghost_position;

    if let Some(&entity) = occupancy.cells.get(&ghost_pos) {
        if let Ok(module) = module_query.get(entity) {
            let def = registry.get(module.module_type);
            let power_str = if def.power_generation > 0.0 {
                format!(" | Pwr:+{:.0}", def.power_generation)
            } else if def.power_consumption > 0.0 {
                format!(" | Pwr:-{:.0}", def.power_consumption)
            } else {
                String::new()
            };
            let mut tooltip_text = format!(
                "{} | HP:{:.0}/{:.0}{}",
                module.module_type.name(),
                module.health,
                module.max_health,
                power_str,
            );
            // Part of a gun built from parts: say what the whole gun does,
            // and the first thing wrong with it, while you're building it.
            if let Some(gun) = guns.iter().find(|g| g.members.contains(&entity)) {
                let st = &gun.stats;
                tooltip_text.push_str(&format!(
                    "\n{:.0} mm gun | {:.0} m/s | {:.0} dmg | {:.1} rpm | {} loader{} | barrel {} m | {} shells ({} ready)",
                    st.gauge_mm, st.velocity_mps, st.damage, st.rpm,
                    st.working_loaders, if st.working_loaders == 1 { "" } else { "s" },
                    gun.parts.barrel_m, gun.max_ammo(), st.ready_rack,
                ));
                if let Some(problem) = gun.problems.first() {
                    tooltip_text.push_str(&format!("\n{}", problem));
                }
            }

            commands.spawn((
                Text2d::new(tooltip_text),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::WHITE),
                TextLayout::justify(Justify::Center),
                // Ship-local, like the ghost and grid it labels: placed in
                // world space it only lined up at a berth near the origin,
                // and at the framing zoom 13px text needs the label scale.
                Transform::from_xyz(
                    ghost_pos.x as f32 * 66.0,
                    ghost_pos.y as f32 * 66.0 - 33.0 + 45.0,
                    1.0,
                ),
                ChildOf(ship),
                crate::camera::ZoomInvariantText,
                ModuleTooltip,
            ));
        }
    }
}

/// Map ModuleCategory to a color for outlines
fn module_category_color(cat: ModuleCategory) -> Color {
    match cat {
        ModuleCategory::Power => Color::srgb(0.8, 0.6, 0.1),
        ModuleCategory::Propulsion => Color::srgb(0.2, 0.5, 0.8),
        ModuleCategory::LifeSupport => Color::srgb(0.2, 0.7, 0.4),
        ModuleCategory::Control => Color::srgb(0.6, 0.6, 0.8),
        ModuleCategory::Weapons => Color::srgb(0.8, 0.2, 0.2),
        // Brass: the colour the gun parts use for anything carrying shells.
        ModuleCategory::Guns => Color::srgb(0.79, 0.6, 0.29),
        ModuleCategory::Lasers => Color::srgb(0.85, 0.35, 0.35),
        ModuleCategory::Detection => Color::srgb(0.3, 0.7, 0.7),
        ModuleCategory::Storage => Color::srgb(0.6, 0.5, 0.3),
        ModuleCategory::Crew => Color::srgb(0.7, 0.5, 0.7),
        ModuleCategory::Utility => Color::srgb(0.5, 0.6, 0.5),
        ModuleCategory::Structural => Color::srgb(0.5, 0.5, 0.55),
    }
}

// ============================================================================
// COSMOTEER-STYLE BOTTOM PANEL
// ============================================================================

/// Color for each build category tab
fn category_color(cat: BuildCategory) -> Color {
    match cat {
        BuildCategory::Hull => Color::srgb(0.5, 0.5, 0.55),
        BuildCategory::Power => Color::srgb(0.8, 0.6, 0.1),
        BuildCategory::Propulsion => Color::srgb(0.2, 0.5, 0.8),
        BuildCategory::LifeSupport => Color::srgb(0.2, 0.7, 0.4),
        BuildCategory::Control => Color::srgb(0.6, 0.6, 0.8),
        BuildCategory::Weapons => Color::srgb(0.8, 0.2, 0.2),
        BuildCategory::Guns => Color::srgb(0.79, 0.6, 0.29),
        BuildCategory::Lasers => Color::srgb(0.85, 0.35, 0.35),
        BuildCategory::Detection => Color::srgb(0.3, 0.7, 0.7),
        BuildCategory::Storage => Color::srgb(0.6, 0.5, 0.3),
        BuildCategory::Crew => Color::srgb(0.7, 0.5, 0.7),
        BuildCategory::Utility => Color::srgb(0.5, 0.6, 0.5),
        BuildCategory::Structural => Color::srgb(0.62, 0.62, 0.64),
        BuildCategory::Premade => Color::srgb(0.9, 0.6, 0.9),
    }
}

/// Short label for each category (fits in tab)
/// Icon for a build category. Rendered from tools/art/icons (Inkscape SVG),
/// monochrome so the UI can tint them per state.
fn category_icon(cat: BuildCategory) -> &'static str {
    match cat {
        BuildCategory::Hull => "ui/icons/cat_hull.png",
        BuildCategory::Power => "ui/icons/cat_power.png",
        BuildCategory::Propulsion => "ui/icons/cat_propulsion.png",
        BuildCategory::LifeSupport => "ui/icons/cat_lifesupport.png",
        BuildCategory::Control => "ui/icons/cat_control.png",
        BuildCategory::Weapons => "ui/icons/cat_weapons.png",
        // No icon of its own yet; shares the weapons one until the gun-part
        // art lands.
        BuildCategory::Guns => "ui/icons/cat_weapons.png",
        BuildCategory::Lasers => "ui/icons/cat_weapons.png",
        BuildCategory::Detection => "ui/icons/cat_detection.png",
        BuildCategory::Storage => "ui/icons/cat_storage.png",
        BuildCategory::Crew => "ui/icons/cat_crew.png",
        BuildCategory::Utility => "ui/icons/cat_utility.png",
        BuildCategory::Structural => "ui/icons/cat_structural.png",
        BuildCategory::Premade => "ui/icons/cat_custom.png",
    }
}

fn category_short_name(cat: BuildCategory) -> &'static str {
    match cat {
        BuildCategory::Hull => "HULL",
        BuildCategory::Power => "PWR",
        BuildCategory::Propulsion => "PROP",
        BuildCategory::LifeSupport => "LIFE",
        BuildCategory::Control => "CTRL",
        BuildCategory::Weapons => "WEAP",
        BuildCategory::Guns => "GUNS",
        BuildCategory::Lasers => "LSR",
        BuildCategory::Detection => "DET",
        BuildCategory::Storage => "STOR",
        BuildCategory::Crew => "CREW",
        BuildCategory::Utility => "UTIL",
        BuildCategory::Structural => "STRU",
        BuildCategory::Premade => "KITS",
    }
}

/// Item-strip geometry. Shared by the slot spawner and the scroll system —
/// scrolling a slot into view has to know the pitch it's scrolling by.
const SLOT_SIZE: f32 = 60.0;
const SLOT_GAP: f32 = 6.0;
const SLOT_PAD: f32 = 8.0;

pub fn spawn_build_panel(
    mut commands: Commands,
    registry: Res<ModuleRegistry>,
    assets: Res<AssetServer>,
) {
    // === ROOT: full-width bottom bar ===
    commands
        .spawn((
            (Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    // Just clear of the HUD's controls strip. The action
                    // toolbar used to sit between them and forced this up to
                    // 86, which pushed the whole panel into the middle of the
                    // screen; it is hidden during build mode now, since the
                    // hint list carries the same keys.
                    bottom: Val::Px(26.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                }),
            BuildPanelRoot,
        ))
        .with_children(|root| {
            // Instructions, down the right edge, clear of everything else.
            root.spawn((Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(12.0),
                    // Above the panel entirely. The root is a column -- a
                    // 32px tab row over a 116px info row -- so anything
                    // anchored below 148 sits inside the info panel, which is
                    // a later sibling and therefore drawn over the top of it.
                    bottom: Val::Px(156.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::new(
                        Val::Px(10.0), Val::Px(10.0),
                        Val::Px(8.0), Val::Px(8.0),
                    ),
                    ..default()
                }, BackgroundColor(ThemeColors::HUD_BG_SOLID)))
            .with_children(|hints| {
                hints.spawn((
                    (
                        Text::new(""),
                        TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                        TextColor(ThemeColors::TEXT_MUTED),
                    ),
                    BuildHintText,
                ));
            });

            // === TOP ROW: Category tabs ===
            root.spawn((Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(32.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                }, BackgroundColor(Color::srgba(0.04, 0.05, 0.10, 0.95))))
            .with_children(|tabs_row| {
                // Mode indicator on the left
                tabs_row.spawn((
                    Text::new(" PLACING "), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(Color::BLACK), Node {
                            padding: UiRect::new(
                                Val::Px(8.0), Val::Px(8.0),
                                Val::Px(6.0), Val::Px(6.0),
                            ),
                            ..default()
                        }, BackgroundColor(Color::srgb(0.0, 1.0, 0.0)),
                    BuildModeText,
                ));

                // Spacer
                tabs_row.spawn((Node {
                        width: Val::Px(8.0),
                        ..default()
                    }));

                // Category tabs
                for (i, cat) in BuildCategory::ALL.iter().enumerate() {
                    tabs_row.spawn((
                        (Node {
                                padding: UiRect::new(
                                    Val::Px(9.0), Val::Px(9.0),
                                    Val::Px(3.0), Val::Px(3.0),
                                ),
                                margin: UiRect::right(Val::Px(2.0)),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(5.0),
                                border: UiRect::bottom(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(ThemeColors::BG_CARD),
                            BorderColor::all(Color::NONE)),
                        CategoryTab { index: i },
                        CategoryTabBg,
                        Interaction::default(),
                    ))
                    .with_children(|tab| {
                        tab.spawn((
                            ImageNode::new(assets.load(category_icon(*cat))),
                            Node { width: Val::Px(18.0), height: Val::Px(18.0), ..default() },
                            CategoryTabIcon,
                        ));
                        tab.spawn((
                            Text::new(category_short_name(*cat)),
                            TextFont { font_size: FontSize::Px(ThemeFonts::BODY_SMALL), ..default() },
                            TextColor(ThemeColors::TEXT_MUTED),
                        ));
                    });
                }
            });

            // === BOTTOM ROW: Items + Info ===
            // 90px cut the description off mid-sentence on anything longer
            // than a line and a half, which is most of them.
            root.spawn((Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(116.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                }, BackgroundColor(Color::srgba(0.03, 0.04, 0.09, 0.94))))
            .with_children(|content| {
                // LEFT: Item slots — a horizontal strip that actually scrolls.
                // Categories like Weapons hold more 60px slots than 60% of any
                // window can show, and with plain clip() everything past the
                // right edge was unreachable by mouse (see scroll_item_slots).
                content.spawn((
                    (Node {
                            width: Val::Percent(60.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(Val::Px(SLOT_PAD)),
                            column_gap: Val::Px(SLOT_GAP),
                            overflow: Overflow::scroll_x(),
                            ..default()
                        }),
                    ScrollPosition::default(),
                    ItemSlotsContainer,
                ))
                .with_children(|items_area| {
                    // Spawn initial item slots (will be rebuilt when category changes)
                    spawn_item_slots(items_area, &BuildCategory::Hull, &registry, &crate::combat::gun_design::WeaponKits::default());
                });

                // Vertical separator
                content.spawn((Node {
                        width: Val::Px(1.0),
                        height: Val::Percent(80.0),
                        align_self: AlignSelf::Center,
                        ..default()
                    }, BackgroundColor(Color::srgba(0.20, 0.23, 0.30, 0.4))));

                // RIGHT: Info panel
                content.spawn((Node {
                        width: Val::Percent(40.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(8.0)),
                        row_gap: Val::Px(2.0),
                        ..default()
                    }))
                .with_children(|info| {
                    // Item name
                    info.spawn((
                        (Text::new("Outer Hull"), TextFont { font_size: FontSize::Px(18.0), ..default() }, TextColor(Color::WHITE)),
                        BuildItemName,
                    ));

                    // Stats
                    info.spawn((
                        (Text::new("HP: 100 | Size: 1x1"), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(Color::srgb(0.6, 0.6, 0.65))),
                        BuildStatsText,
                    ));

                    // Rotation
                    info.spawn((
                        (Text::new("R: Rotate | North"), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgb(0.45, 0.55, 0.65))),
                        BuildRotationText,
                    ));

                    // Material (hull only)
                    info.spawn((
                        (Text::new("Material: Steel (200m)"), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgb(0.5, 0.65, 0.5))),
                        BuildMaterialText,
                    ));

                    // Description
                    info.spawn((
                        (Text::new(""), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(Color::srgb(0.55, 0.55, 0.6))),
                        BuildDescText,
                    ));

                    // Power & shielding summary
                    info.spawn((
                        (Text::new(""), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(Color::srgb(0.5, 0.6, 0.7))),
                        BuildSummaryText,
                    ));
                });
            });
        });
}

/// A pre-made weapon's slot: a short label from its name, coloured by what
/// it is (gun, point defence, missiles, laser).
fn kit_slot(kit: &crate::combat::gun_design::GunDesign) -> (String, Color) {
    let has = |t: ModuleType| kit.parts.iter().any(|p| p.module_type == t);
    let color = if has(ModuleType::LaserCore) {
        Color::srgb(0.70, 0.45, 0.95)
    } else if has(ModuleType::LaunchPad) || has(ModuleType::RailLauncher) || has(ModuleType::VlsSilo) {
        Color::srgb(0.85, 0.35, 0.30)
    } else if kit.point_defence || has(ModuleType::MissileWarner) {
        Color::srgb(0.35, 0.75, 0.85)
    } else {
        Color::srgb(0.85, 0.55, 0.25)
    };
    (kit.name.chars().filter(|c| !c.is_whitespace()).take(4).collect(), color)
}

/// Spawns colored item slots for a given category
fn spawn_item_slots(
    parent: &mut ChildSpawnerCommands,
    category: &BuildCategory,
    registry: &ModuleRegistry,
    kits: &crate::combat::gun_design::WeaponKits,
) {
    match category {
        BuildCategory::Hull => {
            // Order must match HULL_LAYERS then HULL_PLATING in resources.rs —
            // BuildingState::current_selection indexes straight through both.
            // HULL_LAYERS is [Outer, Inner, Hallway, Void, BulkheadDoor] and
            // HULL_PLATING is [AngledArmorPlate, AngledHullPlate] — seven, not
            // six. This list had drifted since Hallway was inserted, so every
            // slot from the third on placed something other than its label:
            // "VOD" laid a Hallway, "BLK" laid Void, "ANG" laid a Bulkhead
            // Door, and Angled Hull Plate had no slot at all.
            let hull_items = [
                ("OUT", Color::srgb(0.4, 0.4, 0.5)),    // Outer
                ("INN", Color::srgb(0.3, 0.3, 0.4)),    // Inner
                ("HAL", Color::srgb(0.28, 0.32, 0.30)), // Hallway
                ("VOD", Color::srgb(0.15, 0.15, 0.2)),  // Void
                ("BLK", Color::srgb(0.5, 0.4, 0.3)),    // BulkheadDoor
                ("ANG", crate::sprite_map::HULL_TONE),  // AngledArmorPlate
                ("AHP", crate::sprite_map::HULL_TONE),  // AngledHullPlate
            ];
            for (i, (label, color)) in hull_items.iter().enumerate() {
                spawn_single_slot(parent, i, label, *color, 0);
            }
        }
        BuildCategory::Premade => {
            for (i, kit) in kits.0.iter().enumerate() {
                let (label, color) = kit_slot(kit);
                spawn_single_slot(parent, i, &label, color, kit.cost(registry));
            }
        }
        _ => {
            {
                let types = category.items();
                for (i, mt) in types.iter().enumerate() {
                    let def = registry.get(*mt);
                    // Use first 3 chars of name as label
                    let label: String = def.name.chars().take(4).collect();
                    spawn_single_slot(parent, i, &label, def.color, def.cost);
                }
            }
        }
    }
}

fn spawn_single_slot(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    label: &str,
    color: Color,
    cost: u32,
) {
    parent.spawn((
        (Node {
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                min_width: Val::Px(SLOT_SIZE),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            }, BackgroundColor(color), BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.0))),
        ItemSlot { index },
        ItemSlotBg,
        Interaction::default(),
    ))
    .with_children(|slot| {
        slot.spawn((Text::new(label), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(Color::WHITE)));
        if cost > 0 {
            // Build cost — the mockup's "◆ cost" tag on each module.
            slot.spawn((Text::new(format!("*{}", cost)), TextFont { font_size: FontSize::Px(9.0), ..default() }, TextColor(Color::srgb(0.95, 0.85, 0.4))));
        }
    });
}

/// Horizontal scrolling for the item strip. Categories hold more slots than
/// fit (Weapons is the worst offender), and everything past the right edge
/// used to be simply unreachable — the strip was clipped, not scrolled.
/// Mouse wheel over the strip scrolls it; cycling with [ / ] (or clicking a
/// tab) drags the selected slot back into view so keyboard users never end up
/// pointed at a slot they can't see.
pub fn scroll_item_slots(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    windows: Query<&Window>,
    build_state: Res<BuildingState>,
    mut container: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform, &mut ScrollPosition), With<ItemSlotsContainer>>,
    // The selection is tracked by hand rather than with BuildingState::
    // is_changed(): update_ghost_preview writes the ghost position into that
    // same resource nearly every frame, so change detection would fire
    // constantly and yank the strip back from wherever the wheel put it.
    mut last_selection: Local<Option<(usize, usize)>>,
) {
    use bevy::input::mouse::MouseScrollUnit;

    let Ok((node, transform, mut scroll)) = container.single_mut() else { return };

    // ComputedNode is physical pixels; ScrollPosition is logical.
    let inv = node.inverse_scale_factor;
    let view_w = node.size.x * inv;
    let content_w = node.content_size.x * inv;
    let max_scroll = (content_w - view_w).max(0.0);

    let cursor = windows.single().ok()
        .and_then(|w| w.cursor_position().map(|p| p * w.scale_factor()));
    let over_strip = cursor
        .and_then(|c| node.normalize_point(*transform, c))
        .is_some_and(|n| n.x.abs() <= 0.5 && n.y.abs() <= 0.5);

    let mut target = scroll.0.x;
    for event in wheel.read() {
        if !over_strip { continue; }
        let step = match event.unit {
            MouseScrollUnit::Line => SLOT_SIZE + SLOT_GAP,
            MouseScrollUnit::Pixel => 1.0,
        };
        // Either axis scrolls the strip — a plain wheel only reports y, while
        // a trackpad's horizontal swipe reports x.
        target -= (event.y + event.x) * step;
    }

    // Keep the selected slot visible when the selection moves under the strip.
    //
    // Only act once the strip has been measured. A node spawned this frame
    // reports zero size until layout runs, which makes max_scroll zero and
    // clamps the computed target back to the left edge -- and because the
    // selection was recorded anyway, the correction never ran again and the
    // strip sat pinned at slot 0 with the highlight somewhere off-screen.
    let selection = (build_state.category_index, build_state.selected_index);
    if view_w > 0.0 && *last_selection != Some(selection) {
        *last_selection = Some(selection);
        let left = build_state.selected_index as f32 * (SLOT_SIZE + SLOT_GAP);
        let right = left + SLOT_SIZE + 2.0 * SLOT_PAD;
        if left < target {
            target = left;
        } else if right > target + view_w {
            target = right - view_w;
        }
    }

    let target = target.clamp(0.0, max_scroll);
    if (target - scroll.0.x).abs() > 0.5 {
        scroll.0.x = target;
    }
}

pub fn despawn_build_panel(
    mut commands: Commands,
    query: Query<Entity, With<BuildPanelRoot>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// UPDATE SYSTEMS
// ============================================================================

/// Updates the panel visuals: highlights, text, item slots
pub fn update_build_panel(
    mut commands: Commands,
    build_state: Res<BuildingState>,
    current_build_state: Res<State<BuildState>>,
    registry: Res<ModuleRegistry>,
    currency: Res<Currency>,
    // Text queries
    mode_q: Query<Entity, With<BuildModeText>>,
    item_name_q: Query<Entity, With<BuildItemName>>,
    stats_q: Query<Entity, With<BuildStatsText>>,
    rot_q: Query<Entity, With<BuildRotationText>>,
    mat_q: Query<Entity, With<BuildMaterialText>>,
    mut text_query: Query<(&mut Text, &mut TextColor)>,
    // Tab highlighting
    mut tabs: (
        Query<(&CategoryTab, &mut BackgroundColor, &mut BorderColor, &Children),
              (With<CategoryTabBg>, Without<ItemSlotBg>)>,
        Query<&mut ImageNode, With<CategoryTabIcon>>,
    ),
    // Item slot highlighting
    mut slot_query: Query<(&ItemSlot, &mut BorderColor),
                          (With<ItemSlotBg>, Without<CategoryTabBg>)>,
    // Item slots container (for rebuilding)
    container_query: Query<(Entity, &Children), With<ItemSlotsContainer>>,
    (mut last_category, kits): (Local<Option<usize>>, Res<crate::combat::gun_design::WeaponKits>),
) {
    let category = build_state.current_category();
    let cat_index = build_state.category_index % BuildCategory::ALL.len();
    let sel_index = build_state.selected_index;

    // Mode indicator
    if let Ok(entity) = mode_q.single() {
        if let Ok((mut text, _)) = text_query.get_mut(entity) {
            let (label, _color) = match current_build_state.get() {
                BuildState::Placing => (" PLACING ", Color::srgb(0.0, 1.0, 0.0)),
                BuildState::Deleting => (" DELETE ", Color::srgb(1.0, 0.0, 0.0)),
                _ => (" BUILD ", Color::WHITE),
            };
            text.0 = label.to_string();
        }
    }

    // Category tab highlighting
    let (ref mut tab_query, ref mut tab_icon_q) = tabs;
    for (tab, mut bg, mut border, children) in tab_query.iter_mut() {
        let is_active = tab.index == cat_index;
        let cat = BuildCategory::ALL[tab.index];
        // The icon carries the category's identity, so the tab itself only has
        // to say "selected" -- an accent underline rather than a slab of
        // saturated colour per category.
        let (bg_col, fg, under) = if is_active {
            (ThemeColors::BG_ELEVATED, ThemeColors::TEXT_PRIMARY, category_color(cat))
        } else {
            (ThemeColors::BG_CARD, ThemeColors::TEXT_MUTED, Color::NONE)
        };
        *bg = bg_col.into();
        *border = BorderColor::all(under);
        for child in children.iter() {
            if let Ok((_, mut text_color)) = text_query.get_mut(child) {
                text_color.0 = fg;
            }
            if let Ok(mut icon) = tab_icon_q.get_mut(child) {
                icon.color = fg;
            }
        }
    }

    // Rebuild item slots if category changed
    if *last_category != Some(cat_index) || (category == BuildCategory::Premade && kits.is_changed()) {
        *last_category = Some(cat_index);
        if let Ok((container_entity, children)) = container_query.single() {
            // Despawn old slots
            for child in children.iter() {
                commands.entity(child).despawn();
            }
            // Spawn new slots
            commands.entity(container_entity).with_children(|parent| {
                spawn_item_slots(parent, &category, &registry, &kits);
            });
        }
    }

    // Item slot highlighting (white border on selected)
    for (slot, mut border) in slot_query.iter_mut() {
        if slot.index == sel_index {
            *border = Color::WHITE.into();
        } else {
            *border = Color::srgba(0.0, 0.0, 0.0, 0.0).into();
        }
    }

    // Item name
    if let Ok(entity) = item_name_q.single() {
        if let Ok((mut text, mut text_color)) = text_query.get_mut(entity) {
            text.0 = match build_state.premade_index().and_then(|i| kits.0.get(i)) {
                Some(kit) => kit.name.clone(),
                None if category == BuildCategory::Premade => "No pre-made weapons in designs/kits".to_string(),
                None => match build_state.current_selection() {
                    // A signature version picked with V (diplomacy::signatures).
                    BuildSelection::Module(mt) => match build_state.signature_for(mt) {
                        Some(sig) => format!("{} ({})", sig.name, mt.name()),
                        None => build_state.selection_name().to_string(),
                    },
                    _ => build_state.selection_name().to_string(),
                },
            };
            // Color the name with the category color
            text_color.0 = category_color(category);
        }
    }

    // Stats — cost is color-coded by affordability against the CURRENT
    // credits balance, so "can I afford this" is visible here instead of
    // only surfacing once placement is actually attempted (the world-space
    // validation message near the ghost, building/mod.rs).
    if let Ok(entity) = stats_q.single() {
        if let Ok((mut text, mut text_color)) = text_query.get_mut(entity) {
            let kit = build_state.premade_index().and_then(|i| kits.0.get(i));
            let (line, cost) = match (kit, build_state.current_selection()) {
                (Some(kit), _) => {
                    let cost = kit.cost(&registry);
                    (format!("{} parts | Cost: {}c | Click or Enter to place, R to turn", kit.parts.len(), cost), cost)
                }
                (None, BuildSelection::Hull(_)) => {
                    let mat = build_state.hull_material;
                    (format!(
                        "HP: {:.0} | Size: 1x1 | Rad Shield: {:.0} | Cost: {}c",
                        100.0 * mat.health_multiplier(),
                        mat.radiation_shielding(),
                        mat.cost()
                    ), mat.cost())
                }
                (None, BuildSelection::Module(mt)) if build_state.signature_for(mt).is_some() => {
                    let def = registry.get(mt);
                    let sig = build_state.signature_for(mt).unwrap();
                    let cost = (def.cost as f32 * sig.build).round() as u32;
                    (format!(
                        "HP: {:.0} | Cost: {}c | + {} | - {} | V: next version",
                        def.health * sig.health, cost, sig.edge, sig.cost
                    ), cost)
                }
                (None, BuildSelection::Module(mt)) => {
                    let def = registry.get(mt);
                    let mut s = format!(
                        "HP: {:.0} | Size: {}x{} | Cost: {}c",
                        def.health, def.size.x, def.size.y, def.cost
                    );
                    if def.power_generation > 0.0 {
                        s.push_str(&format!(" | Power: +{:.0}", def.power_generation));
                    }
                    if def.power_consumption > 0.0 {
                        s.push_str(&format!(" | Power: -{:.0}", def.power_consumption));
                    }
                    if def.customizable {
                        s.push_str(" | Customizable (G/P)");
                    }
                    (s, def.cost)
                }
            };
            text.0 = line;
            text_color.0 = if currency.credits < cost { ThemeColors::ACCENT_RED } else { ThemeColors::TEXT_SECONDARY };
        }
    }

    // Rotation
    if let Ok(entity) = rot_q.single() {
        if let Ok((mut text, _)) = text_query.get_mut(entity) {
            text.0 = format!("R: Rotate | {:?}", build_state.rotation);
        }
    }

    // Material (hull only)
    if let Ok(entity) = mat_q.single() {
        if let Ok((mut text, mut text_color)) = text_query.get_mut(entity) {
            if category == BuildCategory::Hull {
                let mat = build_state.hull_material;
                text.0 =
                    format!("M: Material | {} (Shield: {:.0})", mat.name(), mat.radiation_shielding());
                text_color.0 = Color::srgb(0.5, 0.65, 0.5);
            } else {
                text.0 = String::new();
            }
        }
    }

}

/// Updates description text and power/shielding summary (separate system to stay under 16 params)
pub fn update_build_info(
    build_state: Res<BuildingState>,
    registry: Res<ModuleRegistry>,
    desc_q: Query<Entity, With<BuildDescText>>,
    summary_q: Query<Entity, With<BuildSummaryText>>,
    mut text_query: Query<(&mut Text, &mut TextColor)>,
    module_query: Query<&Module>,
    hull_query: Query<&HullSegment>,
    currency: Res<Currency>,
    kits: Res<crate::combat::gun_design::WeaponKits>,
) {
    // Description
    if let Ok(entity) = desc_q.single() {
        if let Ok((mut text, _)) = text_query.get_mut(entity) {
            let kit = build_state.premade_index().and_then(|i| kits.0.get(i));
            text.0 = match (kit, build_state.current_selection()) {
                (Some(kit), _) => kit.description.clone(),
                (None, BuildSelection::Hull(layer)) => {
                    match layer {
                        HullLayer::Outer => "Primary hull plating. First line of defense against radiation and debris.",
                        HullLayer::Inner => "Secondary hull layer. Adds redundancy.",
                        HullLayer::Hallway => "Walkable decking. The ONLY surface crew can cross - every post needs a hallway route or it goes unmanned.",
                        HullLayer::Void => "Empty space between hulls. Absorbs damage.",
                        HullLayer::BulkheadDoor => "Airtight door. Isolates depressurized sections.",
                    }.to_string()
                }
                (None, BuildSelection::Module(mt)) => {
                    registry.get(mt).description.to_string()
                }
            };
        }
    }

    // Power & shielding summary
    if let Ok(entity) = summary_q.single() {
        if let Ok((mut text, mut text_color)) = text_query.get_mut(entity) {
            let mut total_gen = 0.0_f32;
            let mut total_use = 0.0_f32;
            // The blocks' own figures, not the registry's: a signature block
            // (diplomacy::signatures) makes or draws its own share.
            for module in module_query.iter() {
                total_gen += module.power_generation;
                total_use += module.power_consumption;
            }

            let weakest_hull = hull_query.iter()
                .map(|h| h.radiation_shielding)
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let mut summary = format!("Power: +{:.0} / -{:.0} | Credits: {}", total_gen, total_use, currency.credits);
            if let Some(shielding) = weakest_hull {
                summary.push_str(&format!(" | Rad shielding: {:.0}", shielding));
            }

            let balance = total_gen - total_use;
            text_color.0 = if balance >= 0.0 {
                Color::srgb(0.4, 0.7, 0.5)
            } else {
                Color::srgb(0.8, 0.4, 0.3)
            };
            text.0 = summary;
        }
    }
}

/// Handle mouse clicks on category tabs and item slots
pub fn build_panel_click(
    mut build_state: ResMut<BuildingState>,
    tab_query: Query<(&CategoryTab, &Interaction), Changed<Interaction>>,
    slot_query: Query<(&ItemSlot, &Interaction), Changed<Interaction>>,
) {
    // Category tab clicks
    for (tab, interaction) in tab_query.iter() {
        if *interaction == Interaction::Pressed {
            build_state.category_index = tab.index;
            build_state.selected_index = 0;
        }
    }

    // Item slot clicks
    for (slot, interaction) in slot_query.iter() {
        if *interaction == Interaction::Pressed {
            build_state.selected_index = slot.index;
        }
    }
}

// ============================================================================
// CONTROLS HELP (context-sensitive bottom bar text)
// ============================================================================

pub fn update_controls_help(
    current_build_state: Res<State<BuildState>>,
    game_state: Res<State<crate::states::GameState>>,
    mut help_query: Query<&mut Text, (With<ControlsHelpText>, Without<BuildHintText>)>,
    mut hint_query: Query<&mut Text, (With<BuildHintText>, Without<ControlsHelpText>)>,
) {
    // Build instructions go down the right edge, one to a line. The bottom
    // strip is left EMPTY while building rather than carrying them too: the
    // build panel sits over it, so anything written there is hidden behind the
    // item slots and reads as two overlapping labels.
    let building = *game_state.get() != crate::states::GameState::Exploring;
    if let Ok(mut hint) = hint_query.single_mut() {
        hint.0 = if building {
            match current_build_state.get() {
                BuildState::Placing => {
                    "Click to place\nDrag to paint\nRight-click to remove\n\nR  rotate\nM  material\nV  signature version\nTab  category\nPgUp/PgDn  turret / hull deck\nF2  power view\nEsc  done"
                }
                BuildState::Deleting => "Click or drag to remove\n\nX  leave delete mode\nEsc  done",
                BuildState::PlacingComponent => {
                    "Click a piece\nthen click the grid\n\nRight-click for options\nEsc  done"
                }
                BuildState::CustomizingPiece => "Arrow keys adjust\n\nEsc  done",
                _ => "",
            }
            .to_string()
        } else {
            String::new()
        };
    }

    let Ok(mut text) = help_query.single_mut() else {
        return;
    };

    // This strip used to carry a thirteen-item keyboard reference card, which
    // meant the interface taught itself by asking players to memorize it.
    // Every discrete action now has a labelled button in the action bar, so
    // all that belongs here are the CONTINUOUS inputs a button cannot express
    // — holding a direction, holding a trigger — plus what the mouse does.
    if *game_state.get() == crate::states::GameState::Exploring {
        text.0 = "WASD  thrust and strafe      Shift  brake      Mouse  aim      Space  fire".to_string();
        return;
    }

    // Nothing here while the build panel is up; the side list has it.
    text.0 = String::new();
}

// ============================================================================
// CUSTOMIZATION PANEL
// ============================================================================

pub fn spawn_customization_panel(
    mut commands: Commands,
    customization_state: Res<CustomizationState>,
    existing_panel: Query<Entity, With<CustomizationPanelRoot>>,
) {
    // Despawn existing panel if customization is not active
    if !customization_state.active {
        for entity in existing_panel.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Don't spawn if panel already exists
    if !existing_panel.is_empty() {
        return;
    }

    // Right-side overlay panel
    let panel_root = commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(400.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(15.0),
                ..default()
            }, BackgroundColor(Color::srgba(0.1, 0.1, 0.15, 0.95))),
        CustomizationPanelRoot,
    )).id();

    // Title
    commands.entity(panel_root).with_children(|parent| {
        parent.spawn((Text::new(format!("Customize {}", customization_state.module_type.name())), TextFont { font_size: FontSize::Px(24.0), ..default() }, TextColor(Color::WHITE)));

        // Sliders based on module category
        spawn_sliders_for_category(parent, &customization_state);

        // Stats preview
        spawn_stats_preview(parent, &customization_state);

        // Controls help
        parent.spawn((Text::new("Arrow Keys: Adjust | Enter: Apply | ESC: Cancel"), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.7, 0.7, 0.7))));
    });
}

fn spawn_sliders_for_category(
    parent: &mut ChildSpawnerCommands,
    customization_state: &CustomizationState,
) {
    parent.spawn((Text::new("Properties:"), TextFont { font_size: FontSize::Px(18.0), ..default() }, TextColor(Color::srgb(0.9, 0.9, 0.9))));

    match customization_state.module_type.category() {
        ModuleCategory::Weapons => {
            spawn_slider(parent, "barrel_length", "Barrel Length", 2.0, 10.0, customization_state);
            spawn_slider(parent, "caliber", "Caliber (cm)", 5.0, 15.0, customization_state);
            spawn_slider(parent, "chamber_pressure", "Chamber Pressure", 50.0, 250.0, customization_state);
        }
        ModuleCategory::Propulsion => {
            spawn_slider(parent, "efficiency", "Efficiency", 0.5, 2.0, customization_state);
            spawn_slider(parent, "propeller_count", "Propeller Count", 2.0, 8.0, customization_state);
            spawn_slider(parent, "propeller_pitch", "Propeller Pitch", 0.5, 2.0, customization_state);
        }
        ModuleCategory::Power => {
            spawn_slider(parent, "enrichment", "Fuel Enrichment", 1.0, 3.0, customization_state);
            spawn_slider(parent, "fuel_rod_count", "Fuel Rod Count", 1.0, 8.0, customization_state);
            spawn_slider(parent, "coolant_flow", "Coolant Flow", 50.0, 200.0, customization_state);
        }
        ModuleCategory::LifeSupport => {
            spawn_slider(parent, "filter_size", "Filter Size", 0.5, 2.0, customization_state);
            spawn_slider(parent, "absorber_efficiency", "Absorber Efficiency", 0.5, 2.0, customization_state);
        }
        _ => {}
    }
}

fn spawn_slider(
    parent: &mut ChildSpawnerCommands,
    property_key: &str,
    label: &str,
    min: f32,
    max: f32,
    customization_state: &CustomizationState,
) {
    let current_value = customization_state.properties
        .get(property_key)
        .copied()
        .unwrap_or((min + max) / 2.0);

    // Slider container
    parent.spawn((
        (Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..default()
            }),
    )).with_children(|slider_parent| {
        // Label with value
        slider_parent.spawn((
            (Text::new(format!("{}: {:.1}", label, current_value)), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(COLOR_TEXT_SECONDARY)),
            SliderValueText {
                property_key: property_key.to_string(),
            },
        ));

        // Slider bar background — same visual language as the weapon
        // tuning window's sliders (windows/tuning.rs), just not the same
        // widget: different field types (arbitrary customization
        // properties here vs. WeaponTuning there), so unifying would mean
        // merging two independently-evolved interaction systems rather than
        // a layout change. Shared theme colors get them looking consistent
        // without that risk.
        slider_parent.spawn((Node {
                width: Val::Px(360.0),
                height: Val::Px(20.0),
                ..default()
            }, BackgroundColor(ThemeColors::BG_INPUT))).with_children(|bar_parent| {
            // Slider fill (represents current value)
            let fill_percent = if max > min {
                ((current_value - min) / (max - min) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            bar_parent.spawn((
                (Node {
                        width: Val::Percent(fill_percent),
                        height: Val::Percent(100.0),
                        ..default()
                    }, BackgroundColor(ThemeColors::ACCENT_BLUE)),
                CustomizationSlider {
                    property_key: property_key.to_string(),
                },
            ));
        });
    });
}

fn spawn_stats_preview(
    parent: &mut ChildSpawnerCommands,
    customization_state: &CustomizationState,
) {
    parent.spawn((Text::new("Stats Preview:"), TextFont { font_size: FontSize::Px(18.0), ..default() }, TextColor(COLOR_TEXT_PRIMARY)));

    // Display stats based on module category
    match customization_state.module_type.category() {
        ModuleCategory::Weapons => {
            if let Some(ref weapon_stats) = customization_state.preview_stats.weapon {
                spawn_stat_row(parent, "Damage", weapon_stats.damage);
                spawn_stat_row(parent, "Range", weapon_stats.range);
                spawn_stat_row(parent, "Fire Rate", weapon_stats.fire_rate);
                spawn_stat_row(parent, "Power Cost", weapon_stats.power_cost);
            }
        }
        ModuleCategory::Propulsion => {
            if let Some(ref engine_stats) = customization_state.preview_stats.engine {
                spawn_stat_row(parent, "Thrust", engine_stats.thrust);
                spawn_stat_row(parent, "Fuel Efficiency", engine_stats.fuel_efficiency);
                spawn_stat_row(parent, "Noise", engine_stats.noise);
            }
        }
        ModuleCategory::Power => {
            if let Some(ref reactor_stats) = customization_state.preview_stats.reactor {
                spawn_stat_row(parent, "Power Output", reactor_stats.power_output);
                spawn_stat_row(parent, "Heat", reactor_stats.heat_generation);
                spawn_stat_row(parent, "Explosion Risk", reactor_stats.explosion_risk);
            }
        }
        ModuleCategory::LifeSupport => {
            if let Some(ref ls_stats) = customization_state.preview_stats.life_support {
                spawn_stat_row(parent, "O2 Generation", ls_stats.o2_generation);
                spawn_stat_row(parent, "CO2 Filtering", ls_stats.co2_filtering);
                spawn_stat_row(parent, "Crew Capacity", ls_stats.crew_capacity as f32);
            }
        }
        _ => {}
    }
}

fn spawn_stat_row(parent: &mut ChildSpawnerCommands, name: &str, value: f32) {
    parent.spawn((
        (Text::new(format!("  {}: {:.1}", name, value)), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(ThemeColors::STATUS_OK)),
        CustomizationStatDisplay {
            stat_name: name.to_string(),
        },
    ));
}

// ============================================================================
// CUSTOMIZATION INTERACTION
// ============================================================================

/// Handle customization panel input (arrow keys to adjust sliders, Enter to apply, ESC to cancel)
pub fn handle_customization_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut customization_state: ResMut<CustomizationState>,
    build_state: Res<BuildingState>,
    mut place_events: MessageWriter<PlaceModuleRequest>,
) {
    if !customization_state.active {
        return;
    }

    // ESC: Cancel customization
    if keyboard.just_pressed(KeyCode::Escape) {
        customization_state.cancel();
        return;
    }

    // Enter: Apply customization and place module
    if keyboard.just_pressed(KeyCode::Enter) {
        apply_customization(
            &customization_state,
            &build_state,
            &mut place_events,
        );
        customization_state.cancel();
        return;
    }

    // Arrow keys: Adjust property values
    if keyboard.just_pressed(KeyCode::ArrowLeft) || keyboard.just_pressed(KeyCode::ArrowDown) {
        adjust_focused_property(&mut customization_state, -0.5);
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) || keyboard.just_pressed(KeyCode::ArrowUp) {
        adjust_focused_property(&mut customization_state, 0.5);
    }
}

/// Adjust the currently focused property (for now, adjust barrel_length as default)
fn adjust_focused_property(customization_state: &mut CustomizationState, delta: f32) {
    // For simplicity, adjust the first property in the list
    let property_keys: Vec<String> = customization_state.properties.keys().cloned().collect();

    if let Some(first_key) = property_keys.first() {
        if let Some(&current_value) = customization_state.properties.get(first_key) {
            let new_value = (current_value + delta).max(0.0);
            customization_state.update_property(first_key, new_value);
        }
    }
}

/// Apply customization by creating a PlaceModuleRequest with custom ship-components
fn apply_customization(
    customization_state: &CustomizationState,
    build_state: &BuildingState,
    place_events: &mut MessageWriter<PlaceModuleRequest>,
) {
    // Build ship-components from current property values
    let subcomponents = customization_state.build_subcomponents();

    // Generate a custom name based on module type and properties
    let custom_name = format!("Custom {}", customization_state.module_type.name());

    place_events.write(PlaceModuleRequest {
        module_type: customization_state.module_type,
        grid_position: build_state.ghost_position,
        rotation: build_state.rotation,
        custom_name: Some(custom_name),
        subcomponents: Some(subcomponents),
            extras: None,
        free: false,
    });
}

/// Update customization panel UI to reflect current state
pub fn update_customization_panel(
    customization_state: Res<CustomizationState>,
    mut slider_query: Query<(&CustomizationSlider, &mut Node)>,
    mut value_text_query: Query<(&SliderValueText, &mut Text)>,
    mut stat_query: Query<(&CustomizationStatDisplay, &mut Text), Without<SliderValueText>>,
) {
    if !customization_state.active {
        return;
    }

    // Update slider fill widths
    for (slider, mut style) in slider_query.iter_mut() {
        if let Some(&value) = customization_state.properties.get(&slider.property_key) {
            // Get min/max for this property (hardcoded for now)
            let (min, max) = get_property_range(&slider.property_key);
            let fill_percent = if max > min {
                ((value - min) / (max - min) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            style.width = Val::Percent(fill_percent);
        }
    }

    // Update value text
    for (value_text, mut text) in value_text_query.iter_mut() {
        if let Some(&value) = customization_state.properties.get(&value_text.property_key) {
            let label = get_property_label(&value_text.property_key);
            text.0 = format!("{}: {:.1}", label, value);
        }
    }

    // Update stats
    match customization_state.module_type.category() {
        ModuleCategory::Weapons => {
            if let Some(ref weapon_stats) = customization_state.preview_stats.weapon {
                update_stat_text(&mut stat_query, "Damage", weapon_stats.damage);
                update_stat_text(&mut stat_query, "Range", weapon_stats.range);
                update_stat_text(&mut stat_query, "Fire Rate", weapon_stats.fire_rate);
                update_stat_text(&mut stat_query, "Power Cost", weapon_stats.power_cost);
            }
        }
        ModuleCategory::Propulsion => {
            if let Some(ref engine_stats) = customization_state.preview_stats.engine {
                update_stat_text(&mut stat_query, "Thrust", engine_stats.thrust);
                update_stat_text(&mut stat_query, "Fuel Efficiency", engine_stats.fuel_efficiency);
                update_stat_text(&mut stat_query, "Noise", engine_stats.noise);
            }
        }
        ModuleCategory::Power => {
            if let Some(ref reactor_stats) = customization_state.preview_stats.reactor {
                update_stat_text(&mut stat_query, "Power Output", reactor_stats.power_output);
                update_stat_text(&mut stat_query, "Heat", reactor_stats.heat_generation);
                update_stat_text(&mut stat_query, "Explosion Risk", reactor_stats.explosion_risk);
            }
        }
        ModuleCategory::LifeSupport => {
            if let Some(ref ls_stats) = customization_state.preview_stats.life_support {
                update_stat_text(&mut stat_query, "O2 Generation", ls_stats.o2_generation);
                update_stat_text(&mut stat_query, "CO2 Filtering", ls_stats.co2_filtering);
                update_stat_text(&mut stat_query, "Crew Capacity", ls_stats.crew_capacity as f32);
            }
        }
        _ => {}
    }
}

fn update_stat_text(
    stat_query: &mut Query<(&CustomizationStatDisplay, &mut Text), Without<SliderValueText>>,
    stat_name: &str,
    value: f32,
) {
    for (stat_display, mut text) in stat_query.iter_mut() {
        if stat_display.stat_name == stat_name {
            text.0 = format!("  {}: {:.1}", stat_name, value);
        }
    }
}

fn get_property_range(property_key: &str) -> (f32, f32) {
    match property_key {
        "barrel_length" => (2.0, 10.0),
        "caliber" => (5.0, 15.0),
        "chamber_pressure" => (50.0, 250.0),
        "efficiency" => (0.5, 2.0),
        "propeller_count" => (2.0, 8.0),
        "propeller_pitch" => (0.5, 2.0),
        "enrichment" => (1.0, 3.0),
        "fuel_rod_count" => (1.0, 8.0),
        "coolant_flow" => (50.0, 200.0),
        "filter_size" => (0.5, 2.0),
        "absorber_efficiency" => (0.5, 2.0),
        _ => (0.0, 1.0),
    }
}

fn get_property_label(property_key: &str) -> &'static str {
    match property_key {
        "barrel_length" => "Barrel Length",
        "caliber" => "Caliber (cm)",
        "chamber_pressure" => "Chamber Pressure",
        "efficiency" => "Efficiency",
        "propeller_count" => "Propeller Count",
        "propeller_pitch" => "Propeller Pitch",
        "enrichment" => "Fuel Enrichment",
        "fuel_rod_count" => "Fuel Rod Count",
        "coolant_flow" => "Coolant Flow",
        "filter_size" => "Filter Size",
        "absorber_efficiency" => "Absorber Efficiency",
        _ => "Unknown",
    }
}

// ============================================================================
// COMPONENT PLACEMENT PANEL
// ============================================================================

/// Spawn component placement panel when in PlacingComponent mode
pub fn spawn_component_placement_panel(
    mut commands: Commands,
    placement_state: Res<ComponentPlacementState>,
    existing_panel: Query<Entity, With<ComponentPlacementPanelRoot>>,
) {
    // Despawn existing panel if not active
    if !placement_state.active {
        for entity in existing_panel.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Don't spawn if panel already exists
    if !existing_panel.is_empty() {
        return;
    }

    // Spawn root panel container (darkened backdrop)
    commands.spawn((
        (Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7))),
        ComponentPlacementPanelRoot,
    )).with_children(|parent| {
        // Main panel container with industrial borders
        parent.spawn((Node {
                width: Val::Px(804.0),  
                height: Val::Px(604.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            }, BackgroundColor(COLOR_BG_DARK), BorderColor::all(COLOR_BORDER_LIGHT))).with_children(|border| {
            border.spawn((Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(3.0)),
                    row_gap: Val::Px(2.0),
                    ..default()
                }, BackgroundColor(COLOR_BG_PANEL))).with_children(|main| {
            // Title bar - industrial style with rivets effect
            main.spawn((Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(44.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                }, BackgroundColor(COLOR_TITLE_BAR), BorderColor::all(COLOR_BORDER))).with_children(|title_bar| {
                title_bar.spawn((Text::new(format!("[ COMPONENT BUILDER: {} ]", placement_state.module_type.name().to_uppercase())), TextFont { font_size: FontSize::Px(20.0), ..default() }, TextColor(COLOR_TEXT_ACTIVE)));
            });

            // Content row with gap
            main.spawn((Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(3.0),
                    ..default()
                })).with_children(|panel| {
            // Left side: Component palette with industrial panel
            panel.spawn((Node {
                    width: Val::Px(200.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                }, BackgroundColor(COLOR_BG_PANEL_LIGHT), BorderColor::all(COLOR_BORDER))).with_children(|palette| {
                // Title with industrial brackets
                palette.spawn((Text::new("[ COMPONENTS ]"), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(COLOR_TEXT_ACTIVE)));

                // Separator line
                palette.spawn((Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(2.0),
                        ..default()
                    }, BackgroundColor(COLOR_BORDER)));

                // Get available piece types for this module
                let piece_types = get_available_pieces(placement_state.module_type.category());

                // Get component type color for this category
                let category_color = get_category_color(placement_state.module_type.category());

                // Spawn button for each piece type with industrial style
                for piece_type in piece_types {
                    palette.spawn((
                        (Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(36.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            }, BackgroundColor(COLOR_BUTTON), BorderColor::all(COLOR_BORDER)),
                        ComponentPaletteItem { piece_type: piece_type.clone() },
                    )).with_children(|button| {
                        // Color indicator strip
                        button.spawn((Node {
                                width: Val::Px(3.0),
                                height: Val::Percent(100.0),
                                margin: UiRect::right(Val::Px(6.0)),
                                ..default()
                            }, BackgroundColor(category_color)));

                        // Component name
                        button.spawn((Text::new(piece_type.name().to_uppercase()), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(COLOR_TEXT_PRIMARY)));
                    });
                }
            });

            // Center: Internal grid with industrial panel
            panel.spawn((Node {
                    width: Val::Px(400.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                }, BackgroundColor(COLOR_BG_DARK), BorderColor::all(COLOR_BORDER))).with_children(|center| {
                // Grid title with industrial brackets
                center.spawn((Text::new(format!("[ INTERNAL GRID: {} ]", placement_state.module_type.name().to_uppercase())), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(COLOR_TEXT_ACTIVE)));

                // 4x4 grid with industrial borders
                center.spawn((Node {
                        width: Val::Px(328.0),  
                        height: Val::Px(328.0),
                        flex_direction: FlexDirection::Column,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    }, BackgroundColor(COLOR_BG_PANEL_LIGHT), BorderColor::all(COLOR_BORDER_LIGHT))).with_children(|grid| {
                    for y in 0..4 {
                        grid.spawn((Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(25.0),
                                flex_direction: FlexDirection::Row,
                                ..default()
                            })).with_children(|row| {
                            for x in 0..4 {
                                row.spawn((
                                    (Node {
                                            width: Val::Percent(25.0),
                                            height: Val::Percent(100.0),
                                            border: UiRect::all(Val::Px(1.0)),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        }, BackgroundColor(COLOR_GRID_EMPTY), BorderColor::all(COLOR_BORDER)),
                                    InternalGridCell {
                                        grid_pos: IVec2::new(x, y),
                                    },
                                ));
                            }
                        });
                    }
                });

                // Controls help with industrial styling
                center.spawn((Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    }, BackgroundColor(COLOR_BG_PANEL), BorderColor::all(COLOR_BORDER))).with_children(|help| {
                    help.spawn((Text::new("> Select component from list\n> Click grid to place\n> Right-click for options\n\n[ENTER] Finalize | [ESC] Cancel"), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(COLOR_TEXT_SECONDARY)));
                });
            });

            // Right side: Placed pieces list with industrial panel
            panel.spawn((Node {
                    width: Val::Px(180.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                }, BackgroundColor(COLOR_BG_PANEL_LIGHT), BorderColor::all(COLOR_BORDER))).with_children(|list| {
                // Title with bracket style
                list.spawn((Text::new(format!("[ PLACED: {} ]", placement_state.placed_pieces.len())), TextFont { font_size: FontSize::Px(16.0), ..default() }, TextColor(COLOR_TEXT_ACTIVE)));

                // Separator
                list.spawn((Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(2.0),
                        ..default()
                    }, BackgroundColor(COLOR_BORDER)));

                // List placed pieces with category color
                let category_color = get_category_color(placement_state.module_type.category());
                for piece in &placement_state.placed_pieces {
                    list.spawn((Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        }, BackgroundColor(COLOR_BG_PANEL), BorderColor::all(COLOR_BORDER))).with_children(|item| {
                        // Color indicator
                        item.spawn((Node {
                                width: Val::Px(2.0),
                                height: Val::Percent(100.0),
                                margin: UiRect::right(Val::Px(4.0)),
                                ..default()
                            }, BackgroundColor(category_color)));

                        // Piece info
                        item.spawn((Text::new(format!("{}\n@({},{})", piece.piece_type.name().to_uppercase(), piece.internal_position.x, piece.internal_position.y)), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(COLOR_TEXT_SECONDARY)));
                    });
                }
            });
            });
            });
        });
    });
}

/// Get available component pieces for a module category
fn get_available_pieces(category: ModuleCategory) -> Vec<ComponentPieceType> {
    use crate::components::ComponentPieceType;

    match category {
        ModuleCategory::Weapons => vec![
            ComponentPieceType::Barrel,
            ComponentPieceType::Chamber,
            ComponentPieceType::Loader,
            ComponentPieceType::Magazine,
        ],
        ModuleCategory::Propulsion => vec![
            ComponentPieceType::CombustionChamber,
            ComponentPieceType::Propeller,
            ComponentPieceType::FuelTank,
            ComponentPieceType::CoolingSystem,
        ],
        ModuleCategory::Power => vec![
            ComponentPieceType::FuelRod,
            ComponentPieceType::CoolantPipe,
            ComponentPieceType::Shielding,
            ComponentPieceType::ControlRod,
        ],
        ModuleCategory::LifeSupport => vec![
            ComponentPieceType::ScrubberFilter,
            ComponentPieceType::CO2Absorber,
            ComponentPieceType::AirCirculation,
        ],
        _ => vec![],
    }
}

/// Get color for module category (for visual differentiation)
fn get_category_color(category: ModuleCategory) -> Color {
    match category {
        ModuleCategory::Weapons => COLOR_COMPONENT_WEAPON,
        ModuleCategory::Propulsion => COLOR_COMPONENT_ENGINE,
        ModuleCategory::Power => COLOR_COMPONENT_REACTOR,
        ModuleCategory::LifeSupport => COLOR_COMPONENT_LIFE,
        _ => COLOR_BORDER,
    }
}

/// Handle component palette and grid interactions
pub fn handle_component_placement_input(
    mut placement_state: ResMut<ComponentPlacementState>,
    palette_query: Query<(&ComponentPaletteItem, &Interaction), Changed<Interaction>>,
    grid_query: Query<(&InternalGridCell, &Interaction), Changed<Interaction>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    // Handle palette clicks (select piece type)
    for (item, interaction) in palette_query.iter() {
        if *interaction == Interaction::Pressed {
            placement_state.select_piece(item.piece_type.clone());
        }
    }

    // Handle grid clicks (place/remove pieces)
    for (cell, interaction) in grid_query.iter() {
        if *interaction == Interaction::Pressed {
            if mouse_button.pressed(MouseButton::Right) {
                // Right-click: remove piece
                placement_state.remove_piece(cell.grid_pos);
            } else {
                // Left-click: place piece
                if let Some(piece_type) = placement_state.selected_piece_type.clone() {
                    placement_state.place_piece(cell.grid_pos, piece_type);
                }
            }
        }
    }
}

/// Update palette button colors to show selection and hover (industrial style)
pub fn update_component_palette_visuals(
    placement_state: Res<ComponentPlacementState>,
    mut palette_query: Query<(&ComponentPaletteItem, &Interaction, &mut BackgroundColor)>,
) {
    for (item, interaction, mut color) in palette_query.iter_mut() {
        let is_selected = placement_state.selected_piece_type.as_ref()
            .map(|selected| std::mem::discriminant(&item.piece_type) == std::mem::discriminant(selected))
            .unwrap_or(false);

        *color = match (is_selected, *interaction) {
            (true, Interaction::Hovered) => COLOR_BUTTON_ACTIVE,    // Selected + hover - bright industrial yellow
            (true, _) => Color::srgb(0.75, 0.65, 0.22).into(),       // Selected - darker yellow
            (false, Interaction::Hovered) => COLOR_BUTTON_HOVER,    // Hover only
            (false, Interaction::Pressed) => COLOR_BUTTON_PRESSED,  // Pressed
            (false, _) => COLOR_BUTTON,                             // Normal
        }.into();
    }
}

/// Update context menu button colors on hover (industrial style)
pub fn update_context_menu_visuals(
    mut menu_query: Query<(&ContextMenuOption, &Interaction, &mut BackgroundColor)>,
) {
    for (_option, interaction, mut color) in menu_query.iter_mut() {
        *color = match *interaction {
            Interaction::Hovered => COLOR_BUTTON_HOVER,
            Interaction::Pressed => COLOR_BUTTON_PRESSED,
            Interaction::None => COLOR_BUTTON,
        }.into();
    }
}

/// Update grid cell colors based on placed pieces (industrial style)
pub fn update_component_grid_visuals(
    placement_state: Res<ComponentPlacementState>,
    mut grid_query: Query<(&InternalGridCell, &mut BackgroundColor, &Interaction)>,
) {
    let category_color = get_category_color(placement_state.module_type.category());

    for (cell, mut color, interaction) in grid_query.iter_mut() {
        // Check if this cell is occupied
        let is_occupied = placement_state.placed_pieces.iter().any(|p| {
            let end_pos = p.internal_position + p.size;
            cell.grid_pos.x >= p.internal_position.x && cell.grid_pos.x < end_pos.x
                && cell.grid_pos.y >= p.internal_position.y && cell.grid_pos.y < end_pos.y
        });

        *color = if is_occupied {
            // Occupied cells show category color (brighter on hover)
            match *interaction {
                Interaction::Hovered => category_color,
                _ => Color::srgb(
                    category_color.to_srgba().red * 0.7,
                    category_color.to_srgba().green * 0.7,
                    category_color.to_srgba().blue * 0.7,
                ),
            }
        } else {
            // Empty cells show industrial grid (lighter on hover)
            match *interaction {
                Interaction::Hovered => COLOR_GRID_HOVER,
                _ => COLOR_GRID_EMPTY,
            }
        }.into();
    }
}

/// Handle keyboard input for component placement
pub fn handle_component_placement_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut placement_state: ResMut<ComponentPlacementState>,
    build_state: Res<BuildingState>,
    mut build_state_next: ResMut<NextState<BuildState>>,
    mut place_events: MessageWriter<PlaceModuleRequest>,
) {
    if !placement_state.active {
        return;
    }

    // Escape: Cancel
    if keyboard.just_pressed(KeyCode::Escape) {
        placement_state.cancel();
        build_state_next.set(BuildState::Inactive);
    }

    // Enter: Finalize and place module
    if keyboard.just_pressed(KeyCode::Enter) {
        let pieces = placement_state.finalize();

        // Convert pieces to subcomponents
        let subcomponents: Vec<SubComponentType> = pieces.iter().map(|piece| {
            match piece.piece_type {
                ComponentPieceType::Barrel => SubComponentType::BarrelComponent {
                    length: 5.0,
                    caliber: 50.0,
                    thickness: 5.0,
                },
                ComponentPieceType::Chamber => SubComponentType::ChamberComponent {
                    volume: 50.0,
                    pressure: 100.0,
                },
                ComponentPieceType::Loader => SubComponentType::LoaderComponent {
                    mechanism: crate::components::LoaderMechanism::Automatic,
                    speed: 1.0,
                },
                ComponentPieceType::Magazine => SubComponentType::MagazineComponent {
                    capacity: 10,
                },
                ComponentPieceType::CombustionChamber => SubComponentType::CombustionChamber {
                    efficiency: 1.0,
                },
                ComponentPieceType::Propeller => SubComponentType::PropellerBlade {
                    pitch: 1.0,
                    count: 4,
                },
                ComponentPieceType::FuelTank => SubComponentType::FuelTank {
                    capacity: 100.0,
                },
                ComponentPieceType::CoolingSystem => SubComponentType::CombustionChamber {
                    efficiency: 1.5,
                },
                ComponentPieceType::FuelRod => SubComponentType::FuelRod {
                    enrichment: 1.0,
                    count: 4,
                },
                ComponentPieceType::CoolantPipe => SubComponentType::Coolant {
                    flow_rate: 100.0,
                },
                ComponentPieceType::Shielding => SubComponentType::Shielding {
                    thickness: 5.0,
                },
                ComponentPieceType::ControlRod => SubComponentType::FuelRod {
                    enrichment: 0.5,
                    count: 1,
                },
                ComponentPieceType::ScrubberFilter => SubComponentType::OxygenScrubber {
                    filter_size: 1.0,
                },
                ComponentPieceType::CO2Absorber => SubComponentType::CO2Absorber {
                    efficiency: 1.0,
                },
                ComponentPieceType::AirCirculation => SubComponentType::OxygenScrubber {
                    filter_size: 0.5,
                },
            }
        }).collect();

        // Create custom module name
        let custom_name = format!("Custom {}", placement_state.module_type.name());

        // Send place request
        place_events.write(PlaceModuleRequest {
            module_type: placement_state.module_type,
            grid_position: IVec2::ZERO, // Will be set by placement system
            rotation: build_state.rotation,
            custom_name: Some(custom_name),
            subcomponents: Some(subcomponents),
            extras: None,
            free: false,
        });

        build_state_next.set(BuildState::Placing);
    }
}

// ============================================================================
// PIECE CONTEXT MENU & GROUP CUSTOMIZATION
// ============================================================================

/// Detect right-clicks on placed pieces and show context menu
pub fn show_piece_context_menu(
    mut commands: Commands,
    placement_state: Res<ComponentPlacementState>,
    grid_query: Query<(&InternalGridCell, &Interaction), Changed<Interaction>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    existing_menu: Query<Entity, With<PieceContextMenu>>,
    mut customization_state: ResMut<PieceCustomizationState>,
) {
    // Despawn existing menu if any
    for entity in existing_menu.iter() {
        commands.entity(entity).despawn();
    }

    // Check for right-clicks on grid cells
    for (cell, interaction) in grid_query.iter() {
        if *interaction == Interaction::Pressed && mouse_button.just_pressed(MouseButton::Right) {
            // Check if there's a piece at this position
            if let Some(piece_idx) = placement_state.placed_pieces.iter().position(|p| {
                let end_pos = p.internal_position + p.size;
                cell.grid_pos.x >= p.internal_position.x && cell.grid_pos.x < end_pos.x
                    && cell.grid_pos.y >= p.internal_position.y && cell.grid_pos.y < end_pos.y
            }) {
                // Get connected pieces
                let connected = placement_state.get_connected_pieces(cell.grid_pos);
                let piece = &placement_state.placed_pieces[piece_idx];

                // Spawn context menu with industrial styling
                commands.spawn((
                    (Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(400.0), 
                            top: Val::Px(300.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(6.0)),
                            row_gap: Val::Px(3.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        }, BackgroundColor(COLOR_BG_PANEL), BorderColor::all(COLOR_BORDER_LIGHT)),
                    PieceContextMenu,
                )).with_children(|menu| {
                    // Option 1: Customize this piece
                    menu.spawn((
                        (Node {
                                width: Val::Px(220.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            }, BackgroundColor(COLOR_BUTTON), BorderColor::all(COLOR_BORDER)),
                        ContextMenuOption {
                            option_type: ContextMenuOptionType::CustomizeOne,
                        },
                    )).with_children(|button| {
                        button.spawn((Text::new(format!("> CUSTOMIZE {}", piece.piece_type.name().to_uppercase())), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(COLOR_TEXT_PRIMARY)));
                    });

                    // Option 2: Customize all connected pieces (if more than one)
                    if connected.len() > 1 {
                        menu.spawn((
                            (Node {
                                    width: Val::Px(220.0),
                                    height: Val::Px(32.0),
                                    justify_content: JustifyContent::FlexStart,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(8.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                }, BackgroundColor(COLOR_BUTTON), BorderColor::all(COLOR_BORDER)),
                            ContextMenuOption {
                                option_type: ContextMenuOptionType::CustomizeGroup(connected.len()),
                            },
                        )).with_children(|button| {
                            button.spawn((Text::new(format!("> CUSTOMIZE ALL {} CONNECTED", connected.len())), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(COLOR_TEXT_ACTIVE)));
                        });
                    }

                    // Option 3: Remove piece (warning style)
                    menu.spawn((
                        (Node {
                                width: Val::Px(220.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            }, BackgroundColor(Color::srgb(0.25, 0.15, 0.15)), BorderColor::all(COLOR_DANGER)),
                        ContextMenuOption {
                            option_type: ContextMenuOptionType::Remove,
                        },
                    )).with_children(|button| {
                        button.spawn((Text::new("> REMOVE PIECE"), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(COLOR_DANGER)));
                    });
                });

                // Store the position for later use
                customization_state.target_position = cell.grid_pos;
                customization_state.connected_pieces = connected;
            }
        }
    }
}

/// Handle context menu option clicks
pub fn handle_context_menu_input(
    mut commands: Commands,
    mut placement_state: ResMut<ComponentPlacementState>,
    mut customization_state: ResMut<PieceCustomizationState>,
    option_query: Query<(&ContextMenuOption, &Interaction), Changed<Interaction>>,
    menu_query: Query<Entity, With<PieceContextMenu>>,
) {
    for (option, interaction) in option_query.iter() {
        if *interaction == Interaction::Pressed {
            match &option.option_type {
                ContextMenuOptionType::CustomizeOne => {
                    // Start customizing single piece
                    let connected = vec![customization_state.connected_pieces[0]];
                    let target_pos = customization_state.target_position;
                    customization_state.start_customizing(
                        target_pos,
                        connected,
                        false,
                    );
                }
                ContextMenuOptionType::CustomizeGroup(_count) => {
                    // Start customizing all connected pieces
                    let connected = customization_state.connected_pieces.clone();
                    let target_pos = customization_state.target_position;
                    customization_state.start_customizing(
                        target_pos,
                        connected,
                        true,
                    );
                }
                ContextMenuOptionType::Remove => {
                    // Remove the piece
                    let target_pos = customization_state.target_position;
                    placement_state.remove_piece(target_pos);
                }
            }

            // Despawn the context menu
            for entity in menu_query.iter() {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Spawn piece customization panel when customizing from context menu
pub fn spawn_piece_customization_panel(
    mut commands: Commands,
    customization_state: Res<PieceCustomizationState>,
    placement_state: Res<ComponentPlacementState>,
    existing_panel: Query<Entity, With<PieceCustomizationPanelRoot>>,
) {
    // Despawn existing panel if not active
    if !customization_state.active {
        for entity in existing_panel.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Don't spawn if panel already exists
    if !existing_panel.is_empty() {
        return;
    }

    // Get the piece type
    let piece_idx = customization_state.connected_pieces.first().copied().unwrap_or(0);
    if piece_idx >= placement_state.placed_pieces.len() {
        return;
    }

    let piece = &placement_state.placed_pieces[piece_idx];
    let piece_type = &piece.piece_type;

    // Spawn customization panel
    commands.spawn((
        (Node {
                width: Val::Px(400.0),
                height: Val::Px(500.0),
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                top: Val::Px(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                row_gap: Val::Px(10.0),
                ..default()
            }, BackgroundColor(Color::srgb(0.15, 0.15, 0.18))),
        PieceCustomizationPanelRoot,
    )).with_children(|panel| {
        // Title
        let title = if customization_state.customize_group {
            format!("Customize {} x {} pieces", piece_type.name(), customization_state.connected_pieces.len())
        } else {
            format!("Customize {}", piece_type.name())
        };

        panel.spawn((Text::new(title), TextFont { font_size: FontSize::Px(20.0), ..default() }, TextColor(Color::srgb(0.9, 0.9, 0.9))));

        // Get properties for this piece type
        let properties = get_piece_properties(piece_type);

        // Spawn sliders for each property
        for (key, (_min, _max, default_val)) in properties {
            panel.spawn((Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    ..default()
                })).with_children(|prop_group| {
                // Label
                prop_group.spawn((Text::new(get_property_label(&key)), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.8, 0.8, 0.8))));

                // Slider (placeholder - would need actual slider component)
                prop_group.spawn((
                    (Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(20.0),
                            ..default()
                        }, BackgroundColor(Color::srgb(0.3, 0.3, 0.35))),
                    PieceCustomizationSlider {
                        property_key: key.clone(),
                    },
                ));

                // Value display
                prop_group.spawn((Text::new(format!("{:.1}", default_val)), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgb(0.7, 0.7, 0.7))));
            });
        }

        // Controls help
        panel.spawn((Text::new("TODO: Use arrow keys to adjust values\n[Enter] Apply to pieces | [Esc] Cancel"), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.9, 0.7, 0.5))));
    });
}

/// Get properties and their ranges for a piece type
fn get_piece_properties(piece_type: &ComponentPieceType) -> Vec<(String, (f32, f32, f32))> {
    match piece_type {
        ComponentPieceType::Barrel => vec![
            ("length".to_string(), (2.0, 10.0, 5.0)),
            ("caliber".to_string(), (5.0, 15.0, 50.0)),
            ("thickness".to_string(), (2.0, 8.0, 5.0)),
        ],
        ComponentPieceType::Chamber | ComponentPieceType::CombustionChamber => vec![
            ("pressure".to_string(), (50.0, 250.0, 100.0)),
            ("volume".to_string(), (25.0, 100.0, 50.0)),
        ],
        ComponentPieceType::Loader => vec![
            ("speed".to_string(), (0.5, 2.0, 1.0)),
        ],
        ComponentPieceType::Propeller => vec![
            ("pitch".to_string(), (0.5, 2.0, 1.0)),
            ("count".to_string(), (2.0, 8.0, 4.0)),
        ],
        ComponentPieceType::FuelRod => vec![
            ("enrichment".to_string(), (1.0, 3.0, 1.0)),
        ],
        ComponentPieceType::CoolantPipe => vec![
            ("flow_rate".to_string(), (50.0, 200.0, 100.0)),
        ],
        ComponentPieceType::Shielding => vec![
            ("thickness".to_string(), (2.0, 10.0, 5.0)),
        ],
        ComponentPieceType::ScrubberFilter => vec![
            ("filter_size".to_string(), (0.5, 2.0, 1.0)),
        ],
        ComponentPieceType::CO2Absorber => vec![
            ("efficiency".to_string(), (0.5, 2.0, 1.0)),
        ],
        _ => vec![],
    }
}

/// Handle keyboard input for piece customization
pub fn handle_piece_customization_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut customization_state: ResMut<PieceCustomizationState>,
    mut placement_state: ResMut<ComponentPlacementState>,
) {
    if !customization_state.active {
        return;
    }

    // Escape: Cancel
    if keyboard.just_pressed(KeyCode::Escape) {
        customization_state.cancel();
    }

    // Enter: Apply customization
    if keyboard.just_pressed(KeyCode::Enter) {
        let properties = customization_state.apply();

        // Apply properties to the target piece(s)
        for &piece_idx in &customization_state.connected_pieces {
            if piece_idx < placement_state.placed_pieces.len() {
                let piece = &mut placement_state.placed_pieces[piece_idx];
                for (key, value) in &properties {
                    piece.properties.insert(key.clone(), *value);
                }
            }
        }

        customization_state.cancel();
    }

    // TODO: Arrow keys to adjust property values
}
