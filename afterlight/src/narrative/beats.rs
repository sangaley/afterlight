//! Story beats: something happens on every leg.
//!
//! Between the Summit Record's steps there used to be nothing of the story
//! at all until the third: the instruments, the ship built like yours and
//! the dark all wait for the arc to climb. A beat is one small thing that
//! happens in flight, said as an instrument report or one overheard line:
//! a contact that isn't there, the wreck of a sister ship, a voice asking
//! for you by name. Each stretch between steps has at least one.
//!
//! The rules are few. Beats count seconds of flight (never docked, never
//! over a card, never in training), keep their distance from one another,
//! and never fire into a fight. After long enough with nothing, the next
//! one fires whatever its own gap says. Each fires once a run; which have
//! fired, and the clock they run on, ride on the story record
//! (narrative::story), so a save neither repeats one nor skips one.
//!
//! The beats themselves are data (`data/beats.json`). Things that already
//! happen on their own (the old factions calling you the last one, the
//! ship built like yours) tell this module when they do (`note`), so the
//! spacing holds across all of it.

use std::sync::OnceLock;

use bevy::prelude::*;
use serde::Deserialize;

use crate::ai_ship::components::{AiShip, AiShipTarget, AiShipType, AiShipWreck};
use crate::celestial::components::StarSystemMember;
use crate::components::Ship;
use crate::diplomacy::contacts::Contacts;
use crate::events::{NotificationType, ShowNotification};
use crate::narrative::story::StoryState;
use crate::narrative::summit::SummitProgress;
use crate::states::GameState;

use super::marlow::MarlowMoment;
use super::reader::LogQueue;

/// How near the wreck of the Meridian is put, and how near you have to come
/// to read her recorder.
const MERIDIAN_OUT: std::ops::Range<f32> = 9_000.0..13_000.0;
const RECORDER_RANGE: f32 = 2_500.0;
/// Not laid on a station's doorstep.
const STATION_KEEP_CLEAR: f32 = 8_000.0;
/// Whose hull the Meridian wears now: nobody's fleet, a hauler's grey.
const MERIDIAN_HULL: AiShipType = AiShipType::FreeHaulersCompact;
/// The key her recorder is remembered under once read.
const RECORDER_READ: &str = "meridian_read";

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum When {
    #[default]
    Anytime,
    AwayFromHaven,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Info,
    Warning,
}

#[derive(Deserialize, Debug)]
pub struct BeatDef {
    pub id: String,
    /// Pieces of the record in hand when it can fire.
    pub stage: u8,
    /// Stage 0 only: whether the Abbess has asked for the pieces yet.
    #[serde(default)]
    pub asked: Option<bool>,
    /// Seconds of flight since the last beat before this one may fire.
    pub gap: f32,
    #[serde(default)]
    pub when: When,
    #[serde(default)]
    pub toast: Option<String>,
    #[serde(default)]
    pub kind: Kind,
    /// Seconds of lying instruments, and how badly (1-3).
    #[serde(default)]
    pub jolt: Option<(f32, u8)>,
    /// How far the dark lurches (a story step's lurch is 0.5).
    #[serde(default)]
    pub push: Option<f32>,
    #[serde(default)]
    pub spawn: Option<String>,
    /// A moment for the chief to remark on (data/marlow.json).
    #[serde(default)]
    pub marlow: Option<String>,
}

#[derive(Deserialize)]
pub struct RecorderDef {
    pub kicker: String,
    pub title: String,
    pub gist: String,
    pub text: String,
}

#[derive(Deserialize)]
pub struct BeatData {
    /// Seconds of quiet after which the next beat fires regardless.
    pub max_gap: f32,
    pub beats: Vec<BeatDef>,
    pub meridian: RecorderDef,
}

/// Every beat, from `data/beats.json`.
pub fn data() -> &'static BeatData {
    static DATA: OnceLock<BeatData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../../data/beats.json")).expect("data/beats.json is malformed"))
}

/// Something of the story just happened by itself: the clock the beats keep
/// their distance by starts again from now.
pub fn note(story: &mut StoryState) {
    story.last_beat_at = story.flight;
}

/// Whether a beat belongs to where the chain stands.
fn of_this_stage(beat: &BeatDef, progress: &SummitProgress) -> bool {
    beat.stage == progress.done && beat.asked.is_none_or(|asked| asked == progress.commissioned)
}

/// The beat that should fire now, if any: the first unfired one of this
/// stage whose gap has passed and whose place is right, or after `max_gap`
/// of quiet the first unfired one whatever it asked for.
pub fn due<'a>(beats: &'a [BeatDef], max_gap: f32, progress: &SummitProgress, story: &StoryState, away_from_haven: bool) -> Option<&'a BeatDef> {
    let quiet = story.flight - story.last_beat_at;
    let mut waiting = beats.iter().filter(|b| of_this_stage(b, progress) && !story.beats_fired.contains(&b.id));
    let placed = |b: &&BeatDef| b.when == When::Anytime || away_from_haven;
    if quiet >= max_gap {
        return waiting.next();
    }
    waiting.find(|b| quiet >= b.gap && placed(b))
}

/// Whether this is a fit moment for a beat: nothing up to read, training
/// over, and nobody in range with their guns on you. `guns` is each armed
/// ship's position and what it is aiming at.
fn fit_moment(cards: &LogQueue, training: bool, you: Entity, here: Vec2, mut guns: impl Iterator<Item = (Vec2, Option<Entity>)>) -> bool {
    cards.pending.is_empty()
        && !training
        && !guns.any(|(at, target)| target == Some(you) && at.distance(here) < crate::crew::walking::COMBAT_RANGE)
}

/// The wreck of the CSS Meridian, the Halyard's sister.
#[derive(Component)]
pub struct Meridian;

/// `AFTERLIGHT_BEAT_SPEED=20` runs the beats' clock twenty times faster.
fn speed() -> f32 {
    static SPEED: OnceLock<f32> = OnceLock::new();
    *SPEED.get_or_init(|| std::env::var("AFTERLIGHT_BEAT_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0))
}

/// `AFTERLIGHT_BEAT=<id>` fires that beat as soon as it safely can, whatever
/// the stage: to look at one without flying to it.
fn forced() -> Option<&'static BeatDef> {
    static WANT: OnceLock<Option<String>> = OnceLock::new();
    let id = WANT.get_or_init(|| std::env::var("AFTERLIGHT_BEAT").ok()).as_ref()?;
    data().beats.iter().find(|b| b.id == *id)
}

/// Seconds of flight are the beats' clock: only out there, with nothing up
/// to read and training over (the same rule the dark moves by).
fn count_flight(time: Res<Time>, cards: Res<LogQueue>, tutorial: Option<Res<crate::tutorial::Tutorial>>, mut contacts: ResMut<Contacts>, mut unsaved: Local<f32>) {
    if !cards.pending.is_empty() || tutorial.is_some_and(|t| t.active) {
        return;
    }
    // Banked a second at a time, so Contacts isn't marked changed every frame.
    *unsaved += time.delta_secs() * speed();
    if *unsaved >= 1.0 {
        contacts.story.flight += std::mem::take(&mut *unsaved);
    }
}

/// Fire the beat that's due, if it's a fit moment for one.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_beats(
    mut commands: Commands,
    mut contacts: ResMut<Contacts>,
    cards: Res<LogQueue>,
    tutorial: Option<Res<crate::tutorial::Tutorial>>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    player: Query<(Entity, &Transform), With<Ship>>,
    hostiles: Query<(&Transform, &AiShipTarget), (With<AiShip>, Without<AiShipWreck>)>,
    places: (Res<crate::world::home_base::SystemStations>, Res<crate::diplomacy::stations::FactionStations>),
    art: (Res<crate::building::registry::ModuleRegistry>, Res<AssetServer>),
    mut out: (MessageWriter<ShowNotification>, MessageWriter<MarlowMoment>, ResMut<super::interference::Interference>, ResMut<super::front::DarkPush>),
    mut forced_done: Local<bool>,
) {
    let Ok((you, at)) = player.single() else { return };
    let here = at.translation.truncate();
    let guns = hostiles.iter().map(|(tf, target)| (tf.translation.truncate(), target.entity));
    if !fit_moment(&cards, tutorial.is_some_and(|t| t.active), you, here, guns) {
        return;
    }
    let Some(system) = streaming.loaded_system else { return };
    let force = forced().filter(|_| !*forced_done);
    let beat = match force {
        Some(b) => b,
        None => {
            let Some(b) = due(&data().beats, data().max_gap, &contacts.summit, &contacts.story, system != 0) else { return };
            b
        }
    };

    // Something to place first: if there's nowhere fit to put it, wait.
    if beat.spawn.as_deref() == Some("meridian") {
        let (stations, ports) = (&places.0, &places.1);
        let mut rng = rand::thread_rng();
        let angle = rand::Rng::gen_range(&mut rng, 0.0..std::f32::consts::TAU);
        let spot = here + Vec2::from_angle(angle) * rand::Rng::gen_range(&mut rng, MERIDIAN_OUT);
        let crowded = stations.sites.iter().any(|s| s.pos.distance(spot) < STATION_KEEP_CLEAR)
            || ports.post.is_some_and(|(_, at, extent)| at.distance(spot) < STATION_KEEP_CLEAR + extent);
        if crowded {
            return;
        }
        // She is built as you are: the same yard, the same year.
        let Some(hulk) = crate::ai_ship::spawner::spawn_mirror_ship(MERIDIAN_HULL, spot, &mut commands, &art.0, &art.1) else { return };
        commands
            .entity(hulk)
            .remove::<crate::ai_ship::components::MirrorShip>()
            .try_insert((
                Meridian,
                // Marked at the edge of the screen until her recorder is read.
                super::summit::StoryBeacon("Meridian".into()),
                super::dark_scene::FrozenHulk,
                StarSystemMember { system_id: system },
            ));
        info!("BEAT meridian: the wreck lies {:.0} u out", spot.distance(here));
    }

    let (notes, moments, instruments, push) = (&mut out.0, &mut out.1, &mut out.2, &mut out.3);
    if let Some(text) = &beat.toast {
        notes.write(ShowNotification {
            message: text.clone(),
            notification_type: if beat.kind == Kind::Warning { NotificationType::Warning } else { NotificationType::Info },
            duration: 9.0,
        });
    }
    if let Some((secs, phase)) = beat.jolt {
        instruments.jolt(secs, phase);
    }
    if let Some(systems) = beat.push {
        // In the front's own units, as a story step's lurch is (front::STORY_LURCH).
        push.0 += systems;
    }
    if let Some(moment) = beat.marlow.as_deref() {
        moments.write(MarlowMoment(moment));
    }
    info!("BEAT {} (stage {}, {:.0} s of flight)", beat.id, contacts.summit.done, contacts.story.flight);
    if force.is_some() {
        *forced_done = true;
    }
    if !contacts.story.beats_fired.contains(&beat.id) {
        contacts.story.beats_fired.push(beat.id.clone());
    }
    note(&mut contacts.story);
}

/// Come alongside the Meridian and her recorder is read: once.
fn read_the_recorder(
    mut commands: Commands,
    mut contacts: ResMut<Contacts>,
    player: Query<&Transform, With<Ship>>,
    wrecks: Query<&Transform, With<Meridian>>,
    marked: Query<Entity, (With<Meridian>, With<super::summit::StoryBeacon>)>,
    mut cards: ResMut<LogQueue>,
) {
    if wrecks.is_empty() || contacts.story.beats_fired.iter().any(|b| b == RECORDER_READ) {
        return;
    }
    let Ok(you) = player.single() else { return };
    let here = you.translation.truncate();
    if !wrecks.iter().any(|w| w.translation.truncate().distance(here) < RECORDER_RANGE) {
        return;
    }
    let r = &data().meridian;
    cards.push_card(&r.kicker, r.title.clone(), r.text.clone());
    cards.set_gist(&r.title, r.gist.clone());
    contacts.story.beats_fired.push(RECORDER_READ.to_string());
    for wreck in &marked {
        commands.entity(wreck).remove::<super::summit::StoryBeacon>();
    }
    info!("BEAT meridian: recorder read");
}

pub struct BeatsPlugin;

impl Plugin for BeatsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (count_flight, run_beats, read_the_recorder).chain().run_if(in_state(GameState::Exploring)));
    }
}

#[cfg(test)]
mod beat_tests {
    use super::*;

    fn at(done: u8, asked: bool) -> SummitProgress {
        SummitProgress { hailed: true, commissioned: asked, done, ended: false }
    }

    /// Every stretch of the chain has at least one beat of its own: before
    /// the Abbess has asked, after, and with each number of pieces in hand.
    #[test]
    fn every_stage_has_a_beat() {
        let beats = &data().beats;
        assert!(beats.iter().any(|b| of_this_stage(b, &at(0, false))), "nothing happens before the Abbess asks");
        for done in 0..=crate::narrative::summit::last_step() {
            assert!(beats.iter().any(|b| of_this_stage(b, &at(done, true))), "nothing happens with {done} pieces in hand");
        }
        // Ids are unique (they are what's remembered), words are plain, and
        // every remark asked of the chief is one he has.
        let mut ids: Vec<&str> = beats.iter().map(|b| b.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), beats.len(), "two beats share an id");
        for b in beats {
            assert!(b.gap > 0.0 && b.gap < data().max_gap, "{}: gap {}", b.id, b.gap);
            if let Some(t) = &b.toast {
                assert!(t.is_ascii() && t.split_whitespace().count() <= 22, "{}: {t}", b.id);
            }
            if let Some(m) = b.marlow.as_deref() {
                let mut c = Contacts::default();
                c.story.marlow = crate::narrative::story::MarlowFate::Aboard;
                assert!(crate::narrative::marlow::line(m, &c).is_some(), "{}: the chief has no line for '{m}'", b.id);
            }
            if let Some(s) = b.spawn.as_deref() {
                assert_eq!(s, "meridian", "{}: nothing knows how to place '{s}'", b.id);
            }
        }
        let r = &data().meridian;
        for text in [&r.kicker, &r.title, &r.gist, &r.text] {
            assert!(text.is_ascii() && !text.is_empty());
        }
    }

    /// A beat waits out its gap from the last one, whatever fired it; with
    /// the gap passed it fires, and the next waits in turn.
    #[test]
    fn beats_keep_their_distance() {
        let d = data();
        let mut story = StoryState::default();
        let progress = at(2, true);
        let mine: Vec<&BeatDef> = d.beats.iter().filter(|b| of_this_stage(b, &progress)).collect();
        assert!(mine.len() >= 2, "stage 2 needs two beats for this test");
        assert!(due(&d.beats, d.max_gap, &progress, &story, true).is_none(), "fired at once");
        story.flight = mine[0].gap - 1.0;
        assert!(due(&d.beats, d.max_gap, &progress, &story, true).is_none(), "fired early");
        story.flight = mine[0].gap;
        assert_eq!(due(&d.beats, d.max_gap, &progress, &story, true).map(|b| b.id.as_str()), Some(mine[0].id.as_str()));
        story.beats_fired.push(mine[0].id.clone());
        note(&mut story);
        assert!(due(&d.beats, d.max_gap, &progress, &story, true).is_none(), "two at once");
        // Something else of the story happening restarts the wait.
        story.flight += mine[1].gap - 1.0;
        note(&mut story);
        story.flight += mine[1].gap - 1.0;
        assert!(due(&d.beats, d.max_gap, &progress, &story, true).is_none());
        story.flight += 1.0;
        assert_eq!(due(&d.beats, d.max_gap, &progress, &story, true).map(|b| b.id.as_str()), Some(mine[1].id.as_str()));
        // Another stage's beats never fire here.
        story.beats_fired.push(mine[1].id.clone());
        story.flight += 10_000.0;
        let left = due(&d.beats, d.max_gap, &progress, &story, true);
        assert!(left.is_none_or(|b| of_this_stage(b, &progress)));
    }

    /// A beat that wants you away from Haven waits for that; but after long
    /// enough with nothing, it comes anyway.
    #[test]
    fn long_quiet_ends_whatever_a_beat_asked_for() {
        let d = data();
        let progress = at(0, true);
        let meridian = d.beats.iter().find(|b| b.id == "meridian").unwrap();
        assert_eq!(meridian.when, When::AwayFromHaven);
        let mut story = StoryState::default();
        story.flight = meridian.gap + 1.0;
        assert!(due(&d.beats, d.max_gap, &progress, &story, false).is_none(), "the Meridian turned up at Haven");
        assert!(due(&d.beats, d.max_gap, &progress, &story, true).is_some());
        story.flight = d.max_gap;
        assert!(due(&d.beats, d.max_gap, &progress, &story, false).is_some(), "ten minutes of nothing");
    }

    /// What has fired is saved with the run: a load fires nothing twice, and
    /// carries on with what's left.
    #[test]
    fn a_fired_beat_stays_fired_after_a_load() {
        let d = data();
        let mut c = Contacts::default();
        c.summit = at(2, true);
        c.story.flight = 10_000.0;
        let first = due(&d.beats, d.max_gap, &c.summit, &c.story, true).unwrap().id.clone();
        c.story.beats_fired.push(first.clone());
        note(&mut c.story);
        let back: Contacts = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.story.last_beat_at, c.story.last_beat_at);
        let mut story = back.story.clone();
        story.flight += d.max_gap;
        let next = due(&d.beats, d.max_gap, &back.summit, &story, true).map(|b| b.id.clone());
        assert!(next.is_some() && next != Some(first), "{next:?}");
    }

    /// A card up, training running, or guns on you in range, and no beat
    /// fires; and the clock the beats run on stops for the first two.
    #[test]
    fn nothing_fires_in_a_fight_or_over_a_card() {
        let mut world = World::new();
        let (you, other) = (world.spawn_empty().id(), world.spawn_empty().id());
        let quiet = LogQueue::default();
        let mut reading = LogQueue::default();
        reading.push_card("X", "t".into(), "b".into());
        let near = Vec2::new(5_000.0, 0.0);
        let far = Vec2::new(crate::crew::walking::COMBAT_RANGE * 2.0, 0.0);
        assert!(fit_moment(&quiet, false, you, Vec2::ZERO, std::iter::empty()));
        assert!(!fit_moment(&reading, false, you, Vec2::ZERO, std::iter::empty()), "over a card");
        assert!(!fit_moment(&quiet, true, you, Vec2::ZERO, std::iter::empty()), "in training");
        assert!(!fit_moment(&quiet, false, you, Vec2::ZERO, [(near, Some(you))].into_iter()), "into a fight");
        // Guns on someone else, or on you from far off, are not your fight.
        assert!(fit_moment(&quiet, false, you, Vec2::ZERO, [(near, Some(other)), (near, None)].into_iter()));
        assert!(fit_moment(&quiet, false, you, Vec2::ZERO, [(far, Some(you))].into_iter()));

        let mut contacts = Contacts::default();
        contacts.story.flight = 100.0;
        let mut app = App::new();
        app.init_resource::<Time>().init_resource::<LogQueue>().insert_resource(contacts).add_systems(Update, count_flight);
        let tick = |app: &mut App| {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(5));
            app.update();
            app.world().resource::<Contacts>().story.flight
        };
        app.world_mut().resource_mut::<LogQueue>().push_card("X", "t".into(), "b".into());
        assert_eq!(tick(&mut app), 100.0, "the clock ran over a card");
        app.world_mut().resource_mut::<LogQueue>().pending.clear();
        assert!(tick(&mut app) > 100.0);
    }
}
