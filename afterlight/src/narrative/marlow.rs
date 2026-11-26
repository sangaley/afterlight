//! Chief Marlow, the deck chief.
//!
//! He was only a voice on the training card. Now he is aboard: the first
//! berth of a new crew is his (crew::names::MARLOW), he walks to a post like
//! anyone, and he can die like anyone. While he lives he says one short line
//! at the story's moments, as a toast, never a card: the hail, each piece of
//! the record, the first system gone dark, the ship built like yours. When
//! he is dead he says nothing, and the ship has to speak for itself: the
//! shakedown and the first-time tips fall back to the ship's own flat voice.
//!
//! Every word is data (`data/marlow.json`). His fate rides on the story
//! record (narrative::story), so it saves; a save from before he was aboard
//! loads as Unknown, which is the ship's voice.

use std::collections::HashMap;
use std::sync::OnceLock;

use bevy::prelude::*;
use serde::Deserialize;

use crate::components::CrewMember;
use crate::crew::names::MARLOW;
use crate::diplomacy::contacts::Contacts;
use crate::events::{CrewDied, NotificationType, ShowNotification};
use crate::narrative::story::MarlowFate;

use super::reader::LogQueue;

/// Seconds between two of his lines, so a busy minute isn't a monologue.
const BETWEEN_LINES: f32 = 5.0;

#[derive(Deserialize)]
struct TwoVoices {
    marlow: String,
    ship: String,
}

#[derive(Deserialize)]
struct TipWords {
    title: String,
    marlow: String,
    ship: String,
}

#[derive(Deserialize)]
pub struct MarlowData {
    pub name: String,
    kicker: String,
    ship_kicker: String,
    /// How a line of his is shown: `{line}` is what he says.
    said_as: String,
    pub death: String,
    pub burial: String,
    moments: HashMap<String, String>,
    tips: HashMap<String, TipWords>,
    tutorial: Vec<TwoVoices>,
}

/// Every word of his, and the ship's in his place, from `data/marlow.json`.
pub fn data() -> &'static MarlowData {
    static DATA: OnceLock<MarlowData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../../data/marlow.json")).expect("data/marlow.json is malformed"))
}

/// Whether he is alive and aboard: his voice, or the ship's.
pub fn aboard(contacts: &Contacts) -> bool {
    contacts.story.marlow == MarlowFate::Aboard
}

/// Who a training or tip card is from.
pub fn kicker(aboard: bool) -> &'static str {
    if aboard { &data().kicker } else { &data().ship_kicker }
}

/// The shakedown's line for step `step` (crate::tutorial::STEPS order).
pub fn tutorial_line(step: usize, aboard: bool) -> &'static str {
    data().tutorial.get(step).map_or("", |t| if aboard { t.marlow.as_str() } else { t.ship.as_str() })
}

/// How many shakedown steps are written.
pub fn tutorial_steps() -> usize {
    data().tutorial.len()
}

/// A first-time tip's title and text (crate::tips), in whichever voice.
pub fn tip(id: &str, aboard: bool) -> Option<(&'static str, &'static str)> {
    data().tips.get(id).map(|t| (t.title.as_str(), if aboard { t.marlow.as_str() } else { t.ship.as_str() }))
}

/// What he says at a moment, as it is shown, if he is there to say it.
pub fn line(moment: &str, contacts: &Contacts) -> Option<String> {
    if !aboard(contacts) {
        return None;
    }
    data().moments.get(moment).map(|l| data().said_as.replace("{line}", l))
}

/// Something has happened that he might have a word about. Anything may send
/// one (narrative::beats does); the ones that follow from where the story
/// stands are noticed here (`watch_the_story`). Each is said once a run.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarlowMoment(pub &'static str);

/// Moments waiting to be said, and how long since the last.
#[derive(Resource, Default)]
pub struct MarlowQueue {
    waiting: Vec<&'static str>,
    quiet_for: f32,
}

/// The key a said moment is remembered under, among the story's fired beats.
fn said_key(moment: &str) -> String {
    format!("marlow:{moment}")
}

/// Is he aboard? A new crew's first berth carries his name, and finding him
/// there is what puts him aboard; a save from before he existed has no such
/// hand and stays Unknown. When he dies, it is said once, and it is final.
fn keep_his_fate(
    mut contacts: ResMut<Contacts>,
    crew: Query<&CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
    mut deaths: MessageReader<CrewDied>,
    mut notes: MessageWriter<ShowNotification>,
) {
    let died = deaths.read().any(|d| d.name == MARLOW);
    match contacts.story.marlow {
        MarlowFate::Unknown => {
            if !died && crew.iter().any(|c| c.name == MARLOW) {
                contacts.story.marlow = MarlowFate::Aboard;
                info!("MARLOW is aboard");
            }
        }
        MarlowFate::Aboard if died => {
            contacts.story.marlow = MarlowFate::Dead;
            info!("MARLOW is dead");
            notes.write(ShowNotification { message: data().death.clone(), notification_type: NotificationType::Danger, duration: 10.0 });
        }
        _ => {}
    }
}

/// The moments that follow from where the story stands: read off the saved
/// state every frame, so nothing else has to remember to tell him, and a
/// loaded save neither repeats a line nor loses one.
#[allow(clippy::too_many_arguments)]
fn watch_the_story(
    contacts: Res<Contacts>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
    dead: Option<Res<crate::crew::burial::DriftingDead>>,
    mirror: Option<Res<super::doppelganger::Doppelganger>>,
    ward: Option<Res<super::ward::WardState>>,
    mut moments: MessageWriter<MarlowMoment>,
) {
    if !aboard(&contacts) {
        return;
    }
    let (summit, story) = (&contacts.summit, &contacts.story);
    let mut now: Vec<&'static str> = Vec::new();
    if summit.hailed {
        now.push("hail");
    }
    if summit.commissioned {
        now.push("commission");
    }
    if story.errands_given > super::summit::ERRANDS {
        now.push("errands_done");
    }
    for (n, moment) in ["piece_1", "piece_2", "piece_3", "piece_4"].into_iter().enumerate() {
        if summit.done as usize > n {
            now.push(moment);
        }
    }
    if story.carry {
        now.push("carry");
    }
    if !story.heard_last_one.is_empty() {
        now.push("last_one");
    }
    // News of a fall that reached you during the run; where the dark began
    // is known from the first minute and is nobody's news.
    if galaxy.systems.iter().any(|s| s.heard.taken_at.is_some_and(|t| t > 1.0)) {
        now.push("first_dark");
    }
    if dead.is_some_and(|d| !d.bodies.is_empty()) {
        now.push("first_burial");
    }
    if mirror.is_some_and(|m| m.spent) {
        now.push("mirror");
    }
    if ward.is_some_and(|w| w.failing > 0.0) {
        now.push("ward_failing");
    }
    if story.reached > 0 {
        now.push("reached");
    }
    for moment in now {
        if !story.beats_fired.contains(&said_key(moment)) {
            moments.write(MarlowMoment(moment));
        }
    }
}

/// Say what's waiting, one line at a time: never over a card (the piece of
/// the record is read first, then he has his word), and never two at once.
fn say_his_piece(
    time: Res<Time>,
    mut asked: MessageReader<MarlowMoment>,
    mut queue: ResMut<MarlowQueue>,
    mut contacts: ResMut<Contacts>,
    cards: Res<LogQueue>,
    tutorial: Option<Res<crate::tutorial::Tutorial>>,
    mut notes: MessageWriter<ShowNotification>,
) {
    for MarlowMoment(moment) in asked.read().copied() {
        if !queue.waiting.contains(&moment) && !contacts.story.beats_fired.contains(&said_key(moment)) {
            queue.waiting.push(moment);
        }
    }
    queue.quiet_for += time.delta_secs();
    let training = tutorial.is_some_and(|t| t.active);
    if queue.waiting.is_empty() || queue.quiet_for < BETWEEN_LINES || !cards.pending.is_empty() || training {
        return;
    }
    let moment = queue.waiting.remove(0);
    // Dead men say nothing, and what he never said isn't remembered as said.
    let Some(message) = line(moment, &contacts) else { return };
    contacts.story.beats_fired.push(said_key(moment));
    queue.quiet_for = 0.0;
    info!("MARLOW ({moment}): {message}");
    notes.write(ShowNotification { message, notification_type: NotificationType::Info, duration: 9.0 });
}

/// `AFTERLIGHT_MARLOW=dead` plays with him already gone (the ship's voice),
/// through a new run's reset too; `=aboard` puts him aboard on a save that
/// never had him.
fn fate_from_env(mut contacts: ResMut<Contacts>) {
    static WANT: OnceLock<Option<MarlowFate>> = OnceLock::new();
    let want = WANT.get_or_init(|| match std::env::var("AFTERLIGHT_MARLOW").ok().as_deref() {
        Some("dead") => Some(MarlowFate::Dead),
        Some("aboard") => Some(MarlowFate::Aboard),
        _ => None,
    });
    // Read through Deref: nothing is marked changed on a frame that changes nothing.
    let now = contacts.story.marlow;
    let set = match want {
        Some(MarlowFate::Dead) if now != MarlowFate::Dead => MarlowFate::Dead,
        Some(MarlowFate::Aboard) if now == MarlowFate::Unknown => MarlowFate::Aboard,
        _ => return,
    };
    contacts.story.marlow = set;
    info!("MARLOW set by AFTERLIGHT_MARLOW: {set:?}");
}

fn forget_on_new_run(mut queue: ResMut<MarlowQueue>) {
    *queue = MarlowQueue::default();
}

pub struct MarlowPlugin;

impl Plugin for MarlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MarlowQueue>()
            .add_message::<MarlowMoment>()
            .add_systems(Update, fate_from_env)
            .add_systems(
                Update,
                (keep_his_fate, watch_the_story, say_his_piece)
                    .chain()
                    .after(fate_from_env)
                    .run_if(in_state(crate::states::GameState::Exploring)),
            )
            .add_systems(OnEnter(crate::states::GameState::MainMenu), forget_on_new_run);
    }
}

#[cfg(test)]
mod marlow_tests {
    use super::*;

    fn app(fate: MarlowFate) -> App {
        let mut app = App::new();
        let mut contacts = Contacts::default();
        contacts.story.marlow = fate;
        app.add_message::<MarlowMoment>()
            .add_message::<CrewDied>()
            .add_message::<ShowNotification>()
            .init_resource::<Time>()
            .init_resource::<MarlowQueue>()
            .init_resource::<LogQueue>()
            .insert_resource(contacts)
            .insert_resource(crate::celestial::galaxy::generate_galaxy_map(42))
            .add_systems(Update, (keep_his_fate, watch_the_story, say_his_piece).chain());
        app
    }

    /// Run long enough for one line to be due.
    fn wait(app: &mut App) {
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(BETWEEN_LINES + 0.1));
        app.update();
    }

    /// What has been said since the last look.
    fn said(app: &mut App) -> Vec<String> {
        app.world_mut().resource_mut::<Messages<ShowNotification>>().drain().map(|n| n.message).collect()
    }

    fn hand(app: &mut App, name: &str) -> Entity {
        app.world_mut()
            .spawn(CrewMember { name: name.to_string(), health: 100.0, max_health: 100.0, oxygen: 100.0, morale: 100.0, state: crate::components::CrewState::Idle })
            .id()
    }

    /// A new crew's first berth is his, and finding him there puts him
    /// aboard. A crew without him (an old save) leaves the ship speaking.
    #[test]
    fn marlow_is_aboard_a_new_run() {
        assert_eq!(crate::crew::names::starting_name(0), MARLOW);
        assert_eq!(data().name, MARLOW);
        let mut with = app(MarlowFate::Unknown);
        hand(&mut with, MARLOW);
        with.update();
        assert_eq!(with.world().resource::<Contacts>().story.marlow, MarlowFate::Aboard);
        let mut without = app(MarlowFate::Unknown);
        hand(&mut without, "Jones");
        without.update();
        assert_eq!(without.world().resource::<Contacts>().story.marlow, MarlowFate::Unknown);
    }

    /// His name is his alone: no berth past the first and no hired hand is
    /// dealt it.
    #[test]
    fn nobody_else_is_dealt_his_name() {
        assert!(!crate::crew::names::SURNAMES.contains(&MARLOW));
        let crew: Vec<String> = (0..300).map(crate::crew::names::starting_name).collect();
        assert_eq!(crew.iter().filter(|n| n.as_str() == MARLOW).count(), 1);
        let mut rng = rand::thread_rng();
        for _ in 0..200 {
            assert_ne!(crate::crew::names::unused_name(&crew[..40], &mut rng), MARLOW);
        }
    }

    /// He has a word at the story's moments: once each, one at a time, and
    /// not over a card that's waiting to be read.
    #[test]
    fn he_says_each_thing_once_and_waits_for_the_card() {
        let mut app = app(MarlowFate::Aboard);
        app.world_mut().resource_mut::<Contacts>().summit.hailed = true;
        app.world_mut().resource_mut::<LogQueue>().push_card("INCOMING", "THE SILENCE".into(), "text".into());
        wait(&mut app);
        assert!(said(&mut app).is_empty(), "he talked over the hail card");
        app.world_mut().resource_mut::<LogQueue>().pending.clear();
        wait(&mut app);
        let first = said(&mut app);
        assert_eq!(first.len(), 1);
        assert!(first[0].starts_with("Marlow:") && first[0].contains("Silence"), "{first:?}");
        // Said is said: nothing more comes of the same state.
        wait(&mut app);
        wait(&mut app);
        assert!(said(&mut app).is_empty(), "he repeated himself");
        // Two things at once come out one at a time.
        {
            let mut c = app.world_mut().resource_mut::<Contacts>();
            c.summit.commissioned = true;
            c.summit.done = 1;
        }
        wait(&mut app);
        assert_eq!(said(&mut app).len(), 1);
        wait(&mut app);
        assert_eq!(said(&mut app).len(), 1);
        wait(&mut app);
        assert!(said(&mut app).is_empty());
        // And a save in between loses nothing and repeats nothing.
        let saved = serde_json::to_string(app.world().resource::<Contacts>()).unwrap();
        let mut again = self::app(MarlowFate::Aboard);
        again.insert_resource(serde_json::from_str::<Contacts>(&saved).unwrap());
        wait(&mut again);
        wait(&mut again);
        assert!(said(&mut again).is_empty(), "a loaded save heard it all again");
    }

    /// When he dies it is said once, he is gone for good, and the moments
    /// after it pass in silence.
    #[test]
    fn a_dead_marlow_says_nothing() {
        let mut app = app(MarlowFate::Aboard);
        let him = hand(&mut app, MARLOW);
        app.world_mut().write_message(CrewDied { crew: him, name: MARLOW.into(), cause: crate::events::CrewDamageSource::Suffocation });
        app.update();
        assert_eq!(app.world().resource::<Contacts>().story.marlow, MarlowFate::Dead);
        assert!(said(&mut app).iter().any(|m| m == &data().death));
        app.world_mut().despawn(him);
        app.world_mut().resource_mut::<Contacts>().summit.hailed = true;
        wait(&mut app);
        wait(&mut app);
        assert!(said(&mut app).is_empty(), "a dead man had a word");
        // Somebody else dying, or a namesake on another roster, changes nothing.
        let mut other = self::app(MarlowFate::Aboard);
        let jones = hand(&mut other, "Jones");
        other.world_mut().write_message(CrewDied { crew: jones, name: "Jones".into(), cause: crate::events::CrewDamageSource::Suffocation });
        other.update();
        assert_eq!(other.world().resource::<Contacts>().story.marlow, MarlowFate::Aboard);
        assert!(line("hail", &Contacts::default()).is_none(), "a save from before him has his voice");
    }

    /// The shakedown and the tips are written in both voices, and the right
    /// one is used.
    #[test]
    fn tips_fall_back_to_the_ship() {
        assert_eq!(tutorial_steps(), crate::tutorial::step_count(), "the shakedown's words and its steps disagree");
        for step in 0..tutorial_steps() {
            let (his, ships) = (tutorial_line(step, true), tutorial_line(step, false));
            assert!(!his.is_empty() && !ships.is_empty() && his != ships, "step {step}");
            assert!(!ships.contains("Marlow") && !ships.contains(" we ") && !ships.contains("We're"), "step {step}: the ship says 'we': {ships}");
        }
        assert!(tutorial_line(0, true).contains("Marlow"));
        for id in ["breach", "turret_deck", "brownout"] {
            let (title, his) = tip(id, true).unwrap_or_else(|| panic!("no tip {id}"));
            let (_, ships) = tip(id, false).unwrap();
            assert!(!title.is_empty() && his != ships);
        }
        assert_ne!(kicker(true), kicker(false));
        assert!(tip("no_such_tip", true).is_none());
    }

    /// The game's font has no em-dash or curly quotes; they render as boxes.
    #[test]
    fn the_words_are_plain_ascii() {
        let d = data();
        let mut all: Vec<&String> = vec![&d.name, &d.kicker, &d.ship_kicker, &d.said_as, &d.death, &d.burial];
        all.extend(d.moments.values());
        for t in d.tips.values() {
            all.extend([&t.title, &t.marlow, &t.ship]);
        }
        for t in &d.tutorial {
            all.extend([&t.marlow, &t.ship]);
        }
        for text in all {
            assert!(text.is_ascii() && !text.is_empty(), "not plain ASCII: {text}");
        }
        // Moments are toasts: short.
        for (id, l) in &d.moments {
            assert!(l.split_whitespace().count() <= 24, "{id} is too long for a toast: {l}");
        }
        // Every moment the story watches for is written.
        for id in ["hail", "commission", "errands_done", "piece_1", "piece_2", "piece_3", "piece_4", "carry", "last_one", "first_dark", "first_burial", "mirror", "ward_failing", "reached"] {
            assert!(d.moments.contains_key(id), "no line for {id}");
        }
    }
}
