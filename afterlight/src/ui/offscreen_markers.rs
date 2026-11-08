use bevy::prelude::*;
use std::collections::HashMap;

use crate::ai_ship::components::{AiShip, AiShipTarget, AiShipWreck};
use crate::camera::MainCamera;
use crate::components::{LogEntry, Ship};
use super::theme::{ThemeColors, ThemeFonts};

// ============================================================================
// OFF-SCREEN SHIP MARKERS
//
// Playtesting found that in a fight the enemy was almost never on screen: the
// starter ship filled the whole view, AI gunners hold 8-10k units out, and
// nothing said where the shots were coming from. Cosmoteer pins a small arrow
// to the screen edge for every ship out of view; this is that, kept quiet --
// a chevron and a range, dimmer the further away the ship is.
// ============================================================================

/// Ships further out than this get no marker. Well past the 8-10k standoff AI
/// gunners hold, short of the whole streamed-in neighbourhood.
pub const MARKER_RANGE: f32 = 70_000.0;

/// Size of one marker (chevron over a range label), in UI pixels.
const MARKER_SIZE: Vec2 = Vec2::new(56.0, 32.0);

/// How far in from each screen edge the marker centres sit. Top and bottom are
/// deeper so markers clear the vitals bar and the control hints.
#[derive(Clone, Copy, Debug)]
pub struct EdgeInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// Top clears the 65px vitals bar by the marker's half-height and a margin;
/// bottom clears the action toolbar, which stands 30px up and ~45px tall --
/// at 72/64 a marker's range label sat on the bar's bottom line or on the
/// Map/Systems buttons.
pub const INSETS: EdgeInsets = EdgeInsets { left: 34.0, right: 34.0, top: 90.0, bottom: 100.0 };

/// One marker on screen, pointing at `target`.
#[derive(Component)]
pub struct OffscreenMarker {
    pub target: Entity,
    chevron: Entity,
    label: Entity,
}

/// Where to pin the marker for something `offset` pixels from the screen
/// centre (screen space: +x right, +y down), or None when it is in view.
/// Returns the marker centre and the direction it points, in radians
/// clockwise from +x.
pub fn edge_point(offset: Vec2, screen: Vec2, insets: EdgeInsets) -> Option<(Vec2, f32)> {
    let half = screen / 2.0;
    if offset.x.abs() <= half.x && offset.y.abs() <= half.y {
        return None;
    }
    let dir = offset.normalize_or_zero();
    if dir == Vec2::ZERO {
        return None;
    }
    let min = Vec2::new(insets.left, insets.top);
    let max = Vec2::new(screen.x - insets.right, screen.y - insets.bottom);
    // Walk out from the centre along the bearing until the inset rectangle.
    let reach = |d: f32, lo: f32, hi: f32, c: f32| {
        if d > 0.0 { (hi - c) / d } else if d < 0.0 { (lo - c) / d } else { f32::INFINITY }
    };
    let t = reach(dir.x, min.x, max.x, half.x).min(reach(dir.y, min.y, max.y, half.y)).max(0.0);
    Some((half + dir * t, dir.y.atan2(dir.x)))
}

/// The expedition-record marker: amber, the trail's colour, distinct from the
/// red and grey of ships.
const RECORD_COLOR: Color = Color::srgba(0.90, 0.72, 0.35, 0.85);

/// Closer ships draw stronger; the furthest ones are barely there.
fn marker_alpha(distance: f32) -> f32 {
    let t = ((distance - 4_000.0) / (MARKER_RANGE - 4_000.0)).clamp(0.0, 1.0);
    0.9 - 0.5 * t
}

fn marker_color(hostile: bool, distance: f32) -> Color {
    let base = if hostile { ThemeColors::STATUS_DANGER } else { ThemeColors::TEXT_SECONDARY };
    base.with_alpha(marker_alpha(distance))
}

/// The chevron is a square with two borders, so it points up-right; a quarter
/// turn back to the right lines it up with the bearing.
fn chevron_rotation(bearing: f32) -> UiTransform {
    UiTransform::from_rotation(Rot2::radians(bearing + std::f32::consts::FRAC_PI_4))
}

fn marker_node(centre: Vec2) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(centre.x - MARKER_SIZE.x / 2.0),
        top: Val::Px(centre.y - MARKER_SIZE.y / 2.0),
        width: Val::Px(MARKER_SIZE.x),
        height: Val::Px(MARKER_SIZE.y),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(3.0),
        ..default()
    }
}

pub fn update_offscreen_markers(
    mut commands: Commands,
    camera: Query<(&Transform, &Projection), With<MainCamera>>,
    windows: Query<&Window>,
    player: Query<(Entity, &Transform), With<Ship>>,
    ships: Query<(Entity, &Transform, Option<&AiShipTarget>), (With<AiShip>, Without<AiShipWreck>)>,
    mut markers: Query<(Entity, &OffscreenMarker, &mut Node)>,
    mut chevrons: Query<(&mut UiTransform, &mut BorderColor)>,
    mut labels: Query<(&mut Text, &mut TextColor)>,
    trail: (
        Res<crate::narrative::expedition::Expedition>,
        Res<crate::celestial::resources::SystemStreamingManager>,
        Query<(Entity, &Transform, &LogEntry)>,
    ),
    ui_scale: Res<bevy::ui::UiScale>,
) {
    let (Ok((cam_tf, projection)), Some(window), Ok((player, player_tf))) =
        (camera.single(), windows.iter().next(), player.single())
    else {
        return;
    };
    // World units per UI unit. Markers are UI nodes, and `Val::Px` is
    // multiplied by UiScale, so at 120% the right- and bottom-edge markers
    // were pushed off the screen.
    let ui = ui_scale.0.max(0.01);
    let scale = match projection {
        Projection::Orthographic(o) => o.scale * ui,
        _ => ui,
    };
    if scale <= 0.0 {
        return;
    }
    let screen = Vec2::new(window.width(), window.height()) / ui;
    let cam = cam_tf.translation.truncate();
    let home = player_tf.translation.truncate();

    // target -> (centre, bearing, colour, range text)
    let mut wanted: HashMap<Entity, (Vec2, f32, Color, String)> = HashMap::new();
    for (entity, tf, target) in &ships {
        let pos = tf.translation.truncate();
        let distance = pos.distance(home);
        if distance > MARKER_RANGE {
            continue;
        }
        let world = pos - cam;
        let Some((centre, bearing)) = edge_point(Vec2::new(world.x, -world.y) / scale, screen, INSETS) else {
            continue;
        };
        let hostile = target.is_some_and(|t| t.entity == Some(player));
        wanted.insert(entity, (centre, bearing, marker_color(hostile, distance), super::format_range_km(distance)));
    }

    // The next expedition record, when it lies in this system: an amber
    // marker at any range, and over the derelict itself once it's in view.
    let (expedition, streaming, carriers) = &trail;
    if let Some((system, title)) = expedition.next {
        if streaming.loaded_system == Some(system) {
            if let Some((entity, tf, _)) = carriers.iter().find(|(_, _, log)| log.title == title) {
                let pos = tf.translation.truncate();
                let world = pos - cam;
                let offset = Vec2::new(world.x, -world.y) / scale;
                let (centre, bearing) = edge_point(offset, screen, INSETS)
                    .unwrap_or((screen / 2.0 + offset - Vec2::new(0.0, 36.0), std::f32::consts::FRAC_PI_2));
                let label = format!("record {}", super::format_range_km(pos.distance(home)));
                wanted.insert(entity, (centre, bearing, RECORD_COLOR, label));
            }
        }
    }

    for (marker_entity, marker, mut node) in &mut markers {
        let Some((centre, bearing, color, range)) = wanted.remove(&marker.target) else {
            commands.entity(marker_entity).despawn();
            continue;
        };
        let placed = marker_node(centre);
        node.left = placed.left;
        node.top = placed.top;
        if let Ok((mut rotation, mut border)) = chevrons.get_mut(marker.chevron) {
            *rotation = chevron_rotation(bearing);
            *border = BorderColor::all(color);
        }
        if let Ok((mut text, mut text_color)) = labels.get_mut(marker.label) {
            if text.0 != range {
                text.0 = range;
            }
            text_color.0 = color;
        }
    }

    for (target, (centre, bearing, color, range)) in wanted {
        let chevron = commands.spawn((
            Node {
                width: Val::Px(9.0),
                height: Val::Px(9.0),
                border: UiRect { top: Val::Px(2.0), right: Val::Px(2.0), ..default() },
                ..default()
            },
            BorderColor::all(color),
            chevron_rotation(bearing),
        )).id();
        let label = commands.spawn((
            Text::new(range),
            TextFont { font_size: FontSize::Px(ThemeFonts::TINY.max(11.0)), ..default() },
            TextColor(color),
        )).id();
        commands
            .spawn((marker_node(centre), OffscreenMarker { target, chevron, label }))
            .add_children(&[chevron, label]);
    }
}

/// Markers belong to flight. Leaving it -- pause, dock, map -- takes them down.
pub fn clear_offscreen_markers(mut commands: Commands, markers: Query<Entity, With<OffscreenMarker>>) {
    for marker in &markers {
        commands.entity(marker).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_ship::components::AiShipType;

    const SCREEN: Vec2 = Vec2::new(1280.0, 720.0);

    #[test]
    fn nothing_on_screen_gets_a_marker() {
        assert!(edge_point(Vec2::new(600.0, 300.0), SCREEN, INSETS).is_none());
        assert!(edge_point(Vec2::ZERO, SCREEN, INSETS).is_none());
    }

    #[test]
    fn a_ship_dead_ahead_to_the_right_pins_to_the_right_edge() {
        let (centre, bearing) = edge_point(Vec2::new(5000.0, 0.0), SCREEN, INSETS).unwrap();
        assert!((centre.x - (SCREEN.x - INSETS.right)).abs() < 0.01, "{centre}");
        assert!((centre.y - SCREEN.y / 2.0).abs() < 0.01, "{centre}");
        assert!(bearing.abs() < 1e-4);
    }

    /// Above the screen means up the marker goes, clear of the vitals bar.
    #[test]
    fn a_ship_above_pins_below_the_top_bar() {
        let (centre, bearing) = edge_point(Vec2::new(0.0, -4000.0), SCREEN, INSETS).unwrap();
        assert!((centre.y - INSETS.top).abs() < 0.01, "{centre}");
        assert!((bearing + std::f32::consts::FRAC_PI_2).abs() < 1e-4, "points up, got {bearing}");
    }

    /// Corners clamp to whichever edge the bearing reaches first; the marker
    /// never leaves the inset rectangle.
    #[test]
    fn markers_stay_inside_the_insets_in_every_direction() {
        for i in 0..72 {
            let a = i as f32 * std::f32::consts::TAU / 72.0;
            let (c, _) = edge_point(Vec2::from_angle(a) * 9000.0, SCREEN, INSETS).unwrap();
            assert!(c.x >= INSETS.left - 0.01 && c.x <= SCREEN.x - INSETS.right + 0.01, "{a}: {c}");
            assert!(c.y >= INSETS.top - 0.01 && c.y <= SCREEN.y - INSETS.bottom + 0.01, "{a}: {c}");
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_systems(Update, update_offscreen_markers);
        app.init_resource::<bevy::ui::UiScale>();
        app.init_resource::<crate::narrative::expedition::Expedition>();
        app.init_resource::<crate::celestial::resources::SystemStreamingManager>();
        app.world_mut().spawn(Window::default()); // 1280 x 720
        app.world_mut().spawn((MainCamera, Transform::default(), Projection::Orthographic(OrthographicProjection {
            scale: 2.0,
            ..OrthographicProjection::default_2d()
        })));
        app.world_mut().spawn((Ship, Transform::default()));
        app
    }

    fn markers(app: &mut App) -> Vec<Entity> {
        let mut q = app.world_mut().query::<&OffscreenMarker>();
        q.iter(app.world()).map(|m| m.target).collect()
    }

    /// The whole loop: a ship out of view gets one marker, a ship in view and
    /// a wreck get none, and the marker goes when its ship comes into view.
    #[test]
    fn markers_follow_the_ships_they_point_at() {
        let mut app = app();
        // Scale 2 on 1280x720 shows +-1280 x +-720 world units.
        let out_of_view = app.world_mut().spawn((AiShip, AiShipType::TerranHegemony, Transform::from_xyz(6000.0, 0.0, 0.0))).id();
        app.world_mut().spawn((AiShip, Transform::from_xyz(500.0, 200.0, 0.0)));
        app.world_mut().spawn((
            AiShip,
            AiShipWreck { ship_type: AiShipType::TerranHegemony, loot_remaining: 1, intact_frac: 1.0 },
            Transform::from_xyz(-6000.0, 0.0, 0.0),
        ));
        app.world_mut().spawn((AiShip, Transform::from_xyz(0.0, MARKER_RANGE + 1000.0, 0.0)));

        app.update();
        app.update();
        assert_eq!(markers(&mut app), vec![out_of_view]);

        app.world_mut().get_mut::<Transform>(out_of_view).unwrap().translation.x = 900.0;
        app.update();
        assert!(markers(&mut app).is_empty(), "marker outlived its ship coming into view");
    }

    /// A ship shooting at you reads red; anyone else stays grey.
    #[test]
    fn ships_targeting_the_player_draw_red() {
        let mut app = app();
        let player = {
            let mut q = app.world_mut().query_filtered::<Entity, With<Ship>>();
            q.single(app.world()).unwrap()
        };
        app.world_mut().spawn((AiShip, Transform::from_xyz(6000.0, 0.0, 0.0), AiShipTarget { entity: Some(player), ..default() }));
        app.update();
        app.update();
        let label = {
            let mut q = app.world_mut().query::<&OffscreenMarker>();
            q.single(app.world()).unwrap().label
        };
        let color = app.world().get::<TextColor>(label).unwrap().0.to_srgba();
        assert!(color.red > color.green * 2.0, "hostile marker is not red: {color:?}");
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "6.0 km");
    }
}

#[cfg(test)]
mod record_marker_tests {
    use super::*;

    /// The next expedition record gets an amber marker when it lies in the
    /// loaded system -- at any range, unlike ships.
    #[test]
    fn the_next_record_is_marked_in_its_system() {
        let mut app = App::new();
        app.add_systems(Update, update_offscreen_markers);
        app.init_resource::<bevy::ui::UiScale>();
        app.world_mut().spawn(Window::default());
        app.world_mut().spawn((MainCamera, Transform::default(), Projection::Orthographic(OrthographicProjection::default_2d())));
        app.world_mut().spawn((Ship, Transform::default()));
        let title = crate::narrative::logs::LOG_ENTRIES[0].title;
        let carrier = app.world_mut().spawn((
            Transform::from_xyz(60_000.0, 0.0, 0.0),
            LogEntry { title: title.to_string(), text: String::new(), depth_hint: 0.0 },
        )).id();
        let mut exp = crate::narrative::expedition::Expedition::default();
        exp.next = Some((0, title));
        app.insert_resource(exp);
        let mut streaming = crate::celestial::resources::SystemStreamingManager::default();
        streaming.loaded_system = Some(0);
        app.insert_resource(streaming);
        app.update();
        app.update();
        let mut q = app.world_mut().query::<&OffscreenMarker>();
        let targets: Vec<Entity> = q.iter(app.world()).map(|m| m.target).collect();
        assert_eq!(targets, vec![carrier]);
    }
}
