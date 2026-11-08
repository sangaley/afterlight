use bevy::prelude::*;
use crate::events::{ShowNotification, NotificationType};
use super::framework::*;

// ============================================================================
// NOTIFICATION LOG — scrollable history of all game notifications
// ============================================================================

#[derive(Component)]
pub struct NotificationLogWindow;

#[derive(Component)]
pub struct NotificationLogContent;

/// Stores notification history
#[derive(Resource)]
pub struct NotificationHistory {
    pub entries: Vec<NotificationEntry>,
    pub max_entries: usize,
}

pub struct NotificationEntry {
    pub message: String,
    pub notification_type: NotificationType,
    pub timestamp: f32,
}

impl Default for NotificationHistory {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 100,
        }
    }
}

/// Record notifications into history
pub fn record_notifications(
    mut history: ResMut<NotificationHistory>,
    mut events: MessageReader<ShowNotification>,
    time: Res<Time>,
) {
    for event in events.read() {
        history.entries.push(NotificationEntry {
            message: event.message.clone(),
            notification_type: event.notification_type,
            timestamp: time.elapsed_secs(),
        });

        // Trim to max
        while history.entries.len() > history.max_entries {
            history.entries.remove(0);
        }
    }
}

/// Toggle notification log with L key
pub fn toggle_notification_log(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<(Entity, &FloatingWindow)>,
    history: Res<NotificationHistory>,
    contacts: Res<crate::diplomacy::contacts::Contacts>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
    clock: Option<Res<crate::celestial::resources::GalaxyState>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyL) {
        return;
    }

    // By the window root, as with the map: the marker sits on the content
    // node, and despawning only that left an empty framed window behind.
    if let Some((entity, _)) = windows.iter().find(|(_, w)| w.id == "notif_log") {
        commands.entity(entity).despawn();
        return;
    }

    let content = spawn_floating_window(
        &mut commands,
        "notif_log",
        "Log",
        Vec2::new(420.0, 470.0),
        Vec2::new(840.0, 50.0),
    );

    // The Summit Record board, once the Silence have called: what you've
    // learned, what's next, how much is left (narrative::summit::board).
    let progress = &contacts.summit;
    if progress.hailed || progress.commissioned {
        use crate::narrative::summit::BoardRow;
        let board = commands.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            margin: UiRect::bottom(Val::Px(10.0)),
            ..default()
        }).id();
        let line = |text: String, size: f32, color: Color| (Text::new(text), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color));
        let head = commands.spawn(line("THE SUMMIT RECORD".into(), 12.0, Color::srgb(0.95, 0.55, 0.20))).id();
        commands.entity(board).add_child(head);
        if !progress.commissioned {
            let row = commands.spawn(line("The Abbess of the Silence wants to hear you.".into(), 13.0, Color::srgb(0.85, 0.85, 0.9))).id();
            commands.entity(board).add_child(row);
        }
        for row in crate::narrative::summit::board(progress, &galaxy) {
            let (text, color) = match row {
                BoardRow::Found(n, name, gist) => (format!("{n}  {name}: {gist}"), Color::srgb(0.85, 0.85, 0.9)),
                BoardRow::Next(n, name, where_) => (format!("{n}  {name}: {where_}"), Color::srgb(0.95, 0.75, 0.35)),
                BoardRow::Unknown(n) => (format!("{n}  ..."), Color::srgb(0.4, 0.4, 0.45)),
            };
            let row = commands.spawn(line(text, 13.0, color)).id();
            commands.entity(board).add_child(row);
        }
        commands.entity(content).add_child(board);
    }

    // The falls you've heard of, newest first (narrative::news). The map
    // forgets them; this doesn't.
    let now = clock.map_or(0.0, |c| c.galaxy_time);
    let falls = dark_record(&galaxy, now);
    if !falls.is_empty() {
        let record = commands.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            margin: UiRect::bottom(Val::Px(10.0)),
            ..default()
        }).id();
        let line = |text: String, size: f32, color: Color| (Text::new(text), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color));
        let head = commands.spawn(line("THE DARK".into(), 12.0, Color::srgb(0.6, 0.45, 0.8))).id();
        commands.entity(record).add_child(head);
        for text in falls {
            let row = commands.spawn(line(text, 13.0, Color::srgb(0.7, 0.7, 0.78))).id();
            commands.entity(record).add_child(row);
        }
        commands.entity(content).add_child(record);
    }

    // Find root window and mark it
    commands.entity(content).insert(NotificationLogWindow);

    // Scrollable content
    let scroll_area = commands.spawn((
        (Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::ColumnReverse, 
                overflow: Overflow::clip_y(),
                max_height: Val::Px(260.0),
                ..default()
            }),
        NotificationLogContent,
    )).id();

    // Populate with history (last 30 entries)
    let start = history.entries.len().saturating_sub(30);
    for entry in &history.entries[start..] {
        let color = match entry.notification_type {
            NotificationType::Info => Color::srgb(0.5, 0.7, 0.8),
            NotificationType::Warning => Color::srgb(0.8, 0.7, 0.3),
            NotificationType::Danger => Color::srgb(0.9, 0.3, 0.3),
            NotificationType::Success => Color::srgb(0.3, 0.8, 0.4),
        };

        let minutes = (entry.timestamp / 60.0) as u32;
        let seconds = (entry.timestamp % 60.0) as u32;

        let row = commands.spawn(
            (Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::vertical(Val::Px(1.0)),
                    ..default()
                }),
        ).id();

        let text = commands.spawn(
            (Text::new(format!("[{:02}:{:02}] {}", minutes, seconds, entry.message)), TextFont { font_size: FontSize::Px(11.0), ..default() }, TextColor(color)),
        ).id();

        commands.entity(row).add_child(text);
        commands.entity(scroll_area).add_child(row);
    }

    commands.entity(content).add_child(scroll_area);
}

/// One line per system you've heard the dark took, newest first.
pub fn dark_record(galaxy: &crate::celestial::resources::GalaxyMap, now: f64) -> Vec<String> {
    use crate::narrative::news::{forgotten, Forgot};
    let mut falls: Vec<_> = galaxy.systems.iter().filter_map(|s| s.heard.taken_at.map(|t| (t, s))).collect();
    falls.sort_by(|a, b| b.0.total_cmp(&a.0));
    falls
        .into_iter()
        .map(|(t, s)| {
            let when = if t <= 0.0 {
                "where it began".to_string()
            } else {
                match ((now - t) / 60.0).max(0.0) as u32 {
                    0 => "heard just now".to_string(),
                    m => format!("heard {m} min ago"),
                }
            };
            let gone = if forgotten(s, now) == Forgot::Name { ", gone from your map" } else { "" };
            format!("{}: taken, {when}{gone}", s.name)
        })
        .collect()
}

#[cfg(test)]
mod toggle_tests {
    use super::*;

    #[test]
    fn l_opens_the_log_and_l_closes_all_of_it() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<NotificationHistory>();
        app.init_resource::<crate::diplomacy::contacts::Contacts>();
        app.insert_resource(crate::celestial::galaxy::generate_galaxy_map(42));
        app.add_systems(Update, toggle_notification_log);
        let press = |app: &mut App| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::KeyL);
            keys.clear();
            keys.press(KeyCode::KeyL);
            app.update();
        };
        let nodes = |app: &mut App| app.world_mut().query::<&Node>().iter(app.world()).count();

        press(&mut app);
        assert!(nodes(&mut app) > 0, "L did not open the log");
        press(&mut app);
        assert_eq!(nodes(&mut app), 0, "the window frame outlived the close");
    }

    /// Once the Silence have called, the log opens on the Summit Record board.
    #[test]
    fn the_log_opens_on_the_summit_record() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<NotificationHistory>();
        let mut contacts = crate::diplomacy::contacts::Contacts::default();
        contacts.summit.hailed = true;
        app.insert_resource(contacts);
        app.insert_resource(crate::celestial::galaxy::generate_galaxy_map(42));
        app.add_systems(Update, toggle_notification_log);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyL);
        app.update();
        let texts: Vec<String> = app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).collect();
        assert!(texts.iter().any(|t| t == "THE SUMMIT RECORD"), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains("Abbess")));
    }

    /// The record keeps every fall you heard of, newest first, and says when
    /// the map has let one go.
    #[test]
    fn the_dark_record_lists_what_you_heard() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        assert!(dark_record(&galaxy, 0.0).is_empty());
        galaxy.systems[3].heard.taken_at = Some(0.0);
        galaxy.systems[7].heard.taken_at = Some(600.0);
        let rows = dark_record(&galaxy, 600.0 + crate::narrative::news::FORGET_NAME);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].starts_with(&galaxy.systems[7].name));
        assert!(rows[1].contains("where it began") && rows[1].contains("gone from your map"));
    }
}
