//! A card you can actually read a log on.
//!
//! Logs used to arrive as an eight-second notification toast: 340px wide, body
//! text, capped at six on screen at once, and silently deduped if two said the
//! same thing within three seconds. Several entries run past two hundred
//! characters. They were being delivered in a widget that could not hold them
//! and then taken away before they could be finished.
//!
//! This is the tutorial card's shape, deliberately — same placement, same
//! dismiss idiom — because the game already taught the player that a centred
//! card near the top is something to read.

use bevy::prelude::*;

use crate::states::GameState;
use crate::ui::theme::{ThemeColors, ThemeFonts, ThemeSpacing};

/// Logs waiting to be read, oldest first.
///
/// A queue rather than a single slot: flying into a cluster of derelicts can
/// turn up two entries within a second of each other, and the toast path used
/// to drop the second one on the floor.
#[derive(Resource, Default)]
pub struct LogQueue {
    pub pending: Vec<(String, String)>,
    /// The small line above a card's title, by title, for cards that aren't
    /// recovered logs (the story's calls and pieces). Absent: "RECOVERED LOG".
    pub kickers: Vec<(String, &'static str)>,
    /// The one bold line under a card's title, by title (the story's cards):
    /// what a player who doesn't read the card still takes away.
    pub gists: Vec<(String, String)>,
}

impl LogQueue {
    pub fn push(&mut self, title: String, text: String) {
        self.pending.push((title, text));
    }

    /// A card that isn't a recovered log: `kicker` says what it is instead.
    pub fn push_card(&mut self, kicker: &'static str, title: String, text: String) {
        self.kickers.retain(|(t, _)| *t != title);
        self.kickers.push((title.clone(), kicker));
        self.pending.push((title, text));
    }

    /// Give a queued card its bold line.
    pub fn set_gist(&mut self, title: &str, gist: String) {
        self.gists.retain(|(t, _)| t != title);
        if !gist.is_empty() {
            self.gists.push((title.to_string(), gist));
        }
    }

    fn gist(&self, title: &str) -> Option<&str> {
        self.gists.iter().find(|(t, _)| t == title).map(|(_, g)| g.as_str())
    }

    fn kicker(&self, title: &str) -> &'static str {
        self.kickers.iter().find(|(t, _)| t == title).map_or("RECOVERED LOG", |(_, k)| k)
    }
}

/// Crate-visible so the autoplay director can see when a card is up.
#[derive(Component)]
pub(crate) struct LogCardRoot;
#[derive(Component)]
struct LogCardKicker;
#[derive(Component)]
struct LogCardTitle;
#[derive(Component)]
struct LogCardGist;
#[derive(Component)]
struct LogCardBody;
#[derive(Component)]
struct LogCardFooter;

fn spawn_log_card(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Below the objective card (top-left, two lines since the
                // story joined the records), which it used to overlap.
                top: Val::Px(150.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            // Above the tutorial card's 50, though the two no longer meet: logs
            // are held until training is over (see `drive_log_card`).
            ZIndex(55),
            Visibility::Hidden,
            LogCardRoot,
        ))
        .with_children(|wrapper| {
            // A sheet: the kicker and the close key in the title strip, then
            // the title, the bold line and the text.
            wrapper
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(540.0),
                        max_width: Val::Percent(92.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(crate::ui::theme::Sheet::BG.with_alpha(0.97)),
                    BorderColor::all(crate::ui::theme::Sheet::EDGE),
                ))
                .with_children(|card| {
                    card.spawn((
                        Node {
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                            border: UiRect::bottom(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(crate::ui::theme::Sheet::EDGE),
                    ))
                    .with_children(|strip| {
                        strip.spawn((
                            Text::new("RECOVERED LOG"),
                            TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
                            TextColor(ThemeColors::ACCENT_ORANGE),
                            LogCardKicker,
                        ));
                        strip.spawn((
                            Text::new("[Space] close"),
                            TextFont { font_size: FontSize::Px(ThemeFonts::TINY), ..default() },
                            TextColor(crate::ui::theme::Sheet::LABEL),
                            LogCardFooter,
                        ));
                    });
                    card.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(ThemeSpacing::SM),
                        padding: UiRect::new(Val::Px(14.0), Val::Px(14.0), Val::Px(10.0), Val::Px(14.0)),
                        ..default()
                    })
                    .with_children(|inner| {
                        inner.spawn((
                            Text::new(""),
                            TextFont { font_size: FontSize::Px(16.0), ..default() },
                            TextColor(crate::ui::theme::Sheet::TITLE),
                            LogCardTitle,
                        ));
                        inner.spawn((
                            Text::new(""),
                            TextFont { font_size: FontSize::Px(ThemeFonts::BODY), weight: FontWeight::BOLD, ..default() },
                            TextColor(Color::srgb(1.0, 0.914, 0.659)),
                            Node { display: Display::None, margin: UiRect::vertical(Val::Px(ThemeSpacing::XS)), ..default() },
                            LogCardGist,
                        ));
                        inner.spawn((
                            Text::new(""),
                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                            TextColor(ThemeColors::TEXT_SECONDARY),
                            LogCardBody,
                        ));
                    });
                });
        });
}

/// Shows the head of the queue, and pops it when the player closes it.
///
/// Space closes, not Escape — Escape pauses, and a reader that ate the pause
/// key would be worse than the toast it replaces.
#[allow(clippy::type_complexity)]
fn drive_log_card(
    mut queue: ResMut<LogQueue>,
    keys: Res<ButtonInput<KeyCode>>,
    mut root: Query<&mut Visibility, With<LogCardRoot>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<LogCardTitle>>,
        Query<&mut Text, With<LogCardBody>>,
        Query<&mut Text, With<LogCardFooter>>,
        Query<&mut Text, With<LogCardKicker>>,
        Query<(&mut Text, &mut Node), With<LogCardGist>>,
    )>,
    mut showing: Local<Option<String>>,
    tutorial: Option<Res<crate::tutorial::Tutorial>>,
) {
    let Ok(mut vis) = root.single_mut() else { return };

    // Hold every log while flight training is running. Both cards claim the
    // top-centre of the screen, and this one used to be drawn over the
    // tutorial on purpose ("the log is the thing that just happened"). A
    // playtest showed what that costs a new player: a recovered log sat on
    // top of the training instructions for over a minute, because nobody in
    // their first minute knows Space closes it. Both cards also read Space,
    // so one press advanced training AND dismissed the log.
    //
    // Nothing is lost by waiting -- the queue keeps every entry, and they
    // play out in order the moment training ends or is dismissed. Returning
    // before the Space check matters too: otherwise pressing Space for the
    // tutorial would silently clear a log the player never got to see.
    if tutorial.is_some_and(|t| t.active) {
        *vis = Visibility::Hidden;
        *showing = None;
        return;
    }

    let Some((title, body)) = queue.pending.first().cloned() else {
        *vis = Visibility::Hidden;
        *showing = None;
        return;
    };

    *vis = Visibility::Visible;

    // Only rewrite the card when the entry actually changes.
    // Keyed on the text as well as the title: a log's closing line
    // (narrative::leads) can land a frame after the card first shows.
    let key = format!("{title}\u{0}{}", body.len());
    if showing.as_deref() != Some(key.as_str()) {
        if let Ok(mut t) = texts.p0().single_mut() { **t = title.clone(); }
        if let Ok(mut t) = texts.p1().single_mut() { **t = body.clone(); }
        let kicker = queue.kicker(&title);
        if let Ok(mut t) = texts.p3().single_mut() { **t = kicker.to_string(); }
        let gist = queue.gist(&title).map(str::to_string);
        if let Ok((mut t, mut node)) = texts.p4().single_mut() {
            node.display = if gist.is_some() { Display::Flex } else { Display::None };
            **t = gist.unwrap_or_default();
        }
        let remaining = queue.pending.len().saturating_sub(1);
        let footer = if remaining > 0 {
            format!("[Space] close  ({remaining} more)")
        } else {
            "[Space] close".to_string()
        };
        if let Ok(mut t) = texts.p2().single_mut() { **t = footer; }
        *showing = Some(key);
    }

    if keys.just_pressed(KeyCode::Space) {
        queue.pending.remove(0);
        *showing = None;
    }
}

fn hide_log_card(mut root: Query<&mut Visibility, With<LogCardRoot>>) {
    if let Ok(mut v) = root.single_mut() {
        *v = Visibility::Hidden;
    }
}

/// Nothing should still be on screen once the run is over.
fn clear_log_queue(mut queue: ResMut<LogQueue>) {
    queue.pending.clear();
    queue.kickers.clear();
    queue.gists.clear();
}

pub struct LogReaderPlugin;

impl Plugin for LogReaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LogQueue>()
            .add_systems(Startup, spawn_log_card)
            .add_systems(Update, drive_log_card.run_if(in_state(GameState::Exploring)))
            .add_systems(OnEnter(GameState::GameOver), clear_log_queue)
            .add_systems(OnEnter(GameState::MainMenu), clear_log_queue)
            .add_systems(OnEnter(GameState::Truth), clear_log_queue)
            // The card only *drives* while Exploring, so without this it stays
            // on screen unattended once the state changes — it was still sitting
            // over the ending, offering "[Space] close".
            .add_systems(OnExit(GameState::Exploring), hide_log_card);
    }
}

#[cfg(test)]
mod log_card_tests {
    use super::*;
    use crate::tutorial::Tutorial;

    fn app(training: bool) -> App {
        let mut app = App::new();
        app.init_resource::<LogQueue>();
        app.init_resource::<ButtonInput<KeyCode>>();
        let mut t = Tutorial::default();
        if training {
            t.begin();
        }
        app.insert_resource(t);
        app.add_systems(Startup, spawn_log_card);
        app.add_systems(Update, drive_log_card);
        app.update(); // spawn the card
        app
    }

    fn card_visible(app: &mut App) -> bool {
        let mut q = app.world_mut().query_filtered::<&Visibility, With<LogCardRoot>>();
        matches!(q.single(app.world()).unwrap(), Visibility::Visible)
    }

    fn found(app: &mut App) {
        app.world_mut().resource_mut::<LogQueue>().push("Research Note: Acoustics".into(), "...".into());
    }

    /// The playtest: a recovered log covered the training instructions for
    /// over a minute. During training it must not appear at all.
    #[test]
    fn a_log_found_during_training_stays_hidden() {
        let mut app = app(true);
        found(&mut app);
        app.update();
        assert!(!card_visible(&mut app), "log card drawn over the flight tutorial");
    }

    /// Both cards read Space. Pressing it to advance training must not quietly
    /// throw away a log the player has not seen yet.
    #[test]
    fn space_during_training_does_not_eat_a_held_log() {
        let mut app = app(true);
        found(&mut app);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Space);
        app.update();
        assert_eq!(
            app.world().resource::<LogQueue>().pending.len(),
            1,
            "Space for the tutorial dismissed a log that was never shown"
        );
    }

    /// Held, not lost: once training is over the log plays out.
    #[test]
    fn a_held_log_appears_once_training_ends() {
        let mut app = app(true);
        found(&mut app);
        app.update();
        app.world_mut().resource_mut::<Tutorial>().active = false;
        app.update();
        assert!(card_visible(&mut app), "the held log never appeared after training");
    }

    /// Outside training nothing changes: a log shows straight away.
    #[test]
    fn outside_training_a_log_shows_immediately() {
        let mut app = app(false);
        found(&mut app);
        app.update();
        assert!(card_visible(&mut app));
    }
}
