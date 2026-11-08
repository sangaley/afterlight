use bevy::prelude::*;
use crate::components::*;
use crate::events::*;

/// Marker for the overlay legend UI node
#[derive(Component)]
pub(crate) struct DamageOverlayLegend;

/// Toggles the damage overlay on/off when O is pressed during Exploring
pub fn toggle_damage_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    ship_query: Query<(Entity, Option<&DamageOverlayVisible>), With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !keyboard.just_pressed(KeyCode::KeyO) {
        return;
    }

    let Ok((entity, overlay)) = ship_query.single() else { return };

    if overlay.is_some() {
        commands.entity(entity).remove::<DamageOverlayVisible>();
        notifications.write(ShowNotification {
            message: "Damage overlay OFF".into(),
            notification_type: NotificationType::Info,
            duration: 1.0,
        });
    } else {
        commands.entity(entity).insert(DamageOverlayVisible);
        notifications.write(ShowNotification {
            message: "Damage overlay ON - green=OK, yellow=damaged, red=critical, gray=destroyed".into(),
            notification_type: NotificationType::Info,
            duration: 2.5,
        });
    }
}

/// Spawns the legend UI when overlay becomes visible
pub fn spawn_overlay_legend(
    mut commands: Commands,
    ship_query: Query<&DamageOverlayVisible, Added<DamageOverlayVisible>>,
    existing_legend: Query<Entity, With<DamageOverlayLegend>>,
    right_column: Query<Entity, With<crate::ui::RightColumn>>,
) {
    if ship_query.is_empty() {
        return;
    }
    // Don't double-spawn
    if !existing_legend.is_empty() {
        return;
    }

    // Use representative colors from the actual gradient at midpoints
    let entries = [
        ("100-60%", damage_color_smooth(0.80)),
        (" 60-30%", damage_color_smooth(0.45)),
        (" 30-1% ", damage_color_smooth(0.15)),
        ("   0%  ", damage_color_smooth(0.00)),
    ];

    // Stacked in the right-hand column with the tracker and notices. Pinned
    // absolutely at the top right, it sat over both of them -- including the
    // "Damage overlay ON" notice its own key press raised -- and over the
    // bottom of the top bar.
    let legend = commands.spawn((
        (Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0)),
                row_gap: Val::Px(3.0),
                ..default()
            }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7))),
        DamageOverlayLegend,
    )).with_children(|parent| {
        // Title
        parent.spawn((Text::new("DMG OVERLAY"), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::WHITE)));
        for (label, color) in entries {
            // Make swatch fully opaque so it's visible in the legend
            let swatch_color = color.with_alpha(0.9);
            parent.spawn((Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })).with_children(|row| {
                // Color swatch
                row.spawn((Node {
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        ..default()
                    }, BackgroundColor(swatch_color)));
                // Label
                row.spawn((Text::new(label), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(Color::srgba(0.8, 0.8, 0.8, 1.0))));
            });
        }
    }).id();
    if let Ok(column) = right_column.single() {
        commands.entity(column).insert_children(0, &[legend]);
    }
}

/// Despawns the legend UI when overlay is removed or ship is gone.
/// Also handles cleanup on state transitions (game over, etc).
pub fn despawn_overlay_legend(
    mut commands: Commands,
    ship_query: Query<(Entity, Option<&DamageOverlayVisible>), With<Ship>>,
    legend_query: Query<Entity, With<DamageOverlayLegend>>,
) {
    if legend_query.is_empty() {
        return; // Nothing to clean up
    }

    // If no ship exists or ship has no overlay, remove legend
    let should_remove = match ship_query.single() {
        Ok((_, Some(_))) => false, // Ship exists with overlay — keep legend
        _ => true,                 // No ship or no overlay — remove legend
    };

    if should_remove {
        for entity in legend_query.iter() {
            commands.entity(entity).despawn();
        }
    }
}

/// Cleanup legend on game over / main menu transitions
pub fn cleanup_overlay_on_exit(
    mut commands: Commands,
    legend_query: Query<Entity, With<DamageOverlayLegend>>,
    overlay_query: Query<Entity, With<DamageOverlaySprite>>,
) {
    for entity in legend_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in overlay_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Updates damage overlay sprites on modules and hull segments when visible
pub fn update_damage_overlay(
    ship_query: Query<&Children, (With<Ship>, With<DamageOverlayVisible>)>,
    module_query: Query<(Entity, &Module), Without<DamageOverlaySprite>>,
    hull_query: Query<(Entity, &HullSegment), (Without<Module>, Without<DamageOverlaySprite>)>,
    mut overlay_sprites: Query<(&ChildOf, &mut Sprite), With<DamageOverlaySprite>>,
    existing_parents: Query<&ChildOf, With<DamageOverlaySprite>>,
    module_transforms: Query<&Transform, (With<Module>, Without<DamageOverlaySprite>)>,
    mut commands: Commands,
) {
    let Ok(children) = ship_query.single() else { return };

    // Collect which parent entities already have overlays
    let mut parents_with_overlay: bevy::platform::collections::HashSet<Entity> = bevy::platform::collections::HashSet::new();
    for parent in existing_parents.iter() {
        parents_with_overlay.insert(parent.parent());
    }

    // Update existing overlay colors
    for (parent, mut sprite) in overlay_sprites.iter_mut() {
        if let Ok((_, module)) = module_query.get(parent.parent()) {
            let ratio = health_ratio(module.health, module.max_health);
            sprite.color = damage_color_smooth(ratio);
        } else if let Ok((_, hull)) = hull_query.get(parent.parent()) {
            let ratio = health_ratio(hull.health, hull.max_health);
            sprite.color = damage_color_smooth(ratio);
        }
    }

    // Spawn overlays for entities that don't have one yet
    for child in children.iter() {
        if parents_with_overlay.contains(&child) {
            continue;
        }

        if let Ok((entity, module)) = module_query.get(child) {
            let ratio = health_ratio(module.health, module.max_health);
            let (size, offset) = match module_transforms.get(entity) {
                Ok(transform) => module_overlay_geometry(module, transform),
                Err(_) => (rotated_size(module.size, module.rotation), Vec2::ZERO),
            };
            spawn_overlay(&mut commands, entity, damage_color_smooth(ratio), size, offset);
        } else if let Ok((entity, hull)) = hull_query.get(child) {
            let ratio = health_ratio(hull.health, hull.max_health);
            spawn_overlay(&mut commands, entity, damage_color_smooth(ratio), IVec2::ONE, Vec2::ZERO);
        }
    }
}

/// Removes all overlay sprites when the overlay is toggled off
pub fn cleanup_damage_overlay(
    ship_query: Query<&Ship, Without<DamageOverlayVisible>>,
    overlay_query: Query<Entity, With<DamageOverlaySprite>>,
    mut commands: Commands,
    mut cleaned: Local<bool>,
) {
    if ship_query.is_empty() {
        // No ship without overlay — either no ship or overlay is active
        *cleaned = false;
        return;
    }

    // Ship exists without DamageOverlayVisible — clean up once
    if *cleaned {
        return;
    }

    for entity in overlay_query.iter() {
        commands.entity(entity).despawn();
    }
    *cleaned = true;
}

// ============================================================================
// HELPERS
// ============================================================================

fn health_ratio(health: f32, max_health: f32) -> f32 {
    if max_health > 0.0 { (health / max_health).clamp(0.0, 1.0) } else { 1.0 }
}

/// Returns the visual size of a module accounting for rotation.
/// East/West rotations swap width and height.
fn rotated_size(size: IVec2, rotation: Rotation) -> IVec2 {
    match rotation {
        Rotation::North | Rotation::South => size,
        Rotation::East | Rotation::West => IVec2::new(size.y, size.x),
    }
}

/// Size (in cells, along the module's OWN axes) and local offset for a
/// module's overlay tile.
///
/// The tile is a child of the module, which already carries its rotation --
/// including the art's base offset -- so sizing it from the world-rotated
/// footprint turned it a second time: on East/West multi-cell blocks it lay
/// 90 degrees across the cells it was meant to cover. Barrels and nozzles
/// also lengthen the module's sprite and shift its centre; the offset undoes
/// that so the tile sits on the footprint, not on the gun.
fn module_overlay_geometry(module: &Module, transform: &Transform) -> (IVec2, Vec2) {
    let world = rotated_size(module.size, module.rotation);
    let angle = transform.rotation.to_euler(EulerRot::XYZ).2;
    let quarter = ((angle / std::f32::consts::FRAC_PI_2).round() as i32).rem_euclid(4);
    let local = if quarter % 2 == 1 { IVec2::new(world.y, world.x) } else { world };
    let (overhang, protrude) = crate::sprite_map::sprite_overhang(module.module_type);
    (local, Vec2::new(0.0, -protrude * overhang * 0.5))
}

/// Overlay z, local to its block. Above the block's own sprite and its turret
/// barrel (0.3), below crew (`CREW_Z` 0.6) once the block's own z is added --
/// at 0.5 the tiles covered the crew walking past and tied with them on hull.
const OVERLAY_Z: f32 = 0.35;

fn spawn_overlay(commands: &mut Commands, parent: Entity, color: Color, size: IVec2, offset: Vec2) {
    let w = 60.0 + (size.x - 1).max(0) as f32 * 66.0;
    let h = 60.0 + (size.y - 1).max(0) as f32 * 66.0;

    let overlay = commands.spawn((
        (Sprite {
                color,
                custom_size: Some(Vec2::new(w, h)),
                ..default()
            }, Transform::from_xyz(offset.x, offset.y, OVERLAY_Z)),
        DamageOverlaySprite,
    )).id();

    commands.entity(overlay).insert(ChildOf(parent));
}

/// Smooth color interpolation based on health ratio.
/// Lerps between green→yellow→red→gray for a continuous gradient.
fn damage_color_smooth(ratio: f32) -> Color {
    let alpha = 0.3;

    if ratio <= 0.0 {
        // Destroyed: dark gray
        return Color::srgba(0.3, 0.3, 0.3, 0.5);
    }

    if ratio <= 0.30 {
        // 0-30%: red to yellow
        let t = ratio / 0.30;
        return Color::srgba(
            0.9,
            lerp(0.1, 0.6, t),
            0.1,
            alpha + 0.15 * (1.0 - t), // more opaque at lower health
        );
    }

    if ratio <= 0.60 {
        // 30-60%: yellow to light green
        let t = (ratio - 0.30) / 0.30;
        return Color::srgba(
            lerp(0.9, 0.4, t),
            lerp(0.6, 0.8, t),
            lerp(0.1, 0.2, t),
            alpha,
        );
    }

    // 60-100%: green (fades out as health approaches full)
    let t = (ratio - 0.60) / 0.40;
    Color::srgba(
        lerp(0.3, 0.1, t),
        lerp(0.8, 0.7, t),
        lerp(0.2, 0.1, t),
        alpha * (1.0 - t * 0.6), // almost invisible at full health
    )
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Both block overlays go off at a berth. Their systems only run in flight,
/// so docking with one on left frozen tinted tiles and the legend over the
/// builder, and the key that turns them off does nothing there.
pub fn overlays_off_at_berth(
    mut commands: Commands,
    ship_query: Query<Entity, With<Ship>>,
) {
    for ship in &ship_query {
        commands.entity(ship).remove::<(DamageOverlayVisible, PressureOverlayVisible)>();
    }
}
