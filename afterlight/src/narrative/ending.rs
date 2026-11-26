//! The demo's last sequence, built from the run.
//!
//! There are no named endings. When the ship carries the four pieces to the
//! berth left empty at the summit site (narrative::site), the record plays
//! itself out, and what it says is put together from facts about this run:
//! which way each piece was taken, who was buried, how many ships were
//! destroyed, whether Marlow is alive, how much of the map has gone dark.
//! Each beat is one condition and a line or two. The beats are data
//! (`data/ending.json`), written in pairs that cover both cases, so every
//! run gets a sequence of about the same length and no two quite alike.
//!
//! The screen it plays on is THE TRUTH's (narrative::truth): the same slow
//! fade, in the same game state, so nothing autosaves or pops up over it.
//! `Playing` says which script that screen is running. The far-edge TRUTH
//! itself stays what it was: the secret ending.
//!
//! The same player runs the short sequence for the dark reaching the ship
//! (narrative::ward): until now that message had no listener.

use std::sync::OnceLock;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::Deserialize;

use crate::celestial::resources::GalaxyMap;
use crate::components::{CrewMember, Ship};
use crate::diplomacy::contacts::Contacts;
use crate::narrative::story::MarlowFate;
use crate::resources::{Currency, Statistics};
use crate::states::GameState;

use super::reader::LogQueue;

/// Seconds between one line of a part and the next coming in.
const LINE_EVERY: f32 = 2.2;
/// The running time the whole sequence must stay inside, whatever the run.
pub const SHORTEST: f32 = 60.0;
pub const LONGEST: f32 = 120.0;

/// One part of a sequence: an optional big line, and lines under it that
/// come in one at a time.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Segment {
    #[serde(default)]
    pub title: String,
    pub seconds: f32,
    pub lines: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct BeatDef {
    pub id: String,
    pub when: String,
    pub seconds: f32,
    #[serde(default)]
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Deserialize)]
pub struct CardDef {
    pub title: String,
    #[serde(default)]
    pub gist: String,
    pub text: String,
}

#[derive(Deserialize)]
pub struct WayWords {
    pub deal: String,
    pub forced: String,
}

#[derive(Deserialize)]
pub struct EndingData {
    pub max_beats: usize,
    /// What the Abbess says after the fourth piece.
    pub carry: CardDef,
    pub open: Vec<Segment>,
    pub beats: Vec<BeatDef>,
    pub close: Vec<Segment>,
    /// The dark had the ship and threw it home.
    pub reached: Vec<Segment>,
    pub way_words: Vec<WayWords>,
    pub end_card: CardDef,
    /// The HUD's standing line once the sequence has played.
    pub after: String,
}

pub fn data() -> &'static EndingData {
    static DATA: OnceLock<EndingData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../../data/ending.json")).expect("data/ending.json is malformed"))
}

/// What the ending knows about a run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunFacts {
    /// How each step this build plays was taken: true is by force. Steps
    /// not recorded count as deals.
    pub ways: Vec<bool>,
    /// Pieces of the record in hand.
    pub pieces: u8,
    pub marlow_alive: bool,
    /// Crew committed to space.
    pub buried: u32,
    /// Ships destroyed.
    pub kills: u32,
    /// Systems the dark has taken, and how many there are in all.
    pub dark: u32,
    pub systems: u32,
    pub credits: u32,
    /// Times the dark had the ship and threw it home.
    pub reached: u32,
    /// Logs read.
    pub read: u32,
    /// Crew aboard now.
    pub crew: u32,
}

impl RunFacts {
    pub fn lit(&self) -> u32 {
        self.systems.saturating_sub(self.dark)
    }

    /// A line with the run's numbers filled in.
    pub fn fill(&self, text: &str) -> String {
        text.replace("{buried}", &self.buried.to_string())
            .replace("{kills}", &self.kills.to_string())
            .replace("{dark}", &self.dark.to_string())
            .replace("{lit}", &self.lit().to_string())
            .replace("{credits}", &self.credits.to_string())
            .replace("{crew}", &self.crew.to_string())
            .replace("{ways}", &self.ways_in_words())
    }

    /// "the minutes bought, the black box taken, ...": the pieces in hand.
    pub fn ways_in_words(&self) -> String {
        let words: Vec<&str> = data()
            .way_words
            .iter()
            .zip(self.ways.iter())
            .take(self.pieces as usize)
            .map(|(w, forced)| if *forced { w.forced.as_str() } else { w.deal.as_str() })
            .collect();
        if words.is_empty() { "none".to_string() } else { words.join(", ") }
    }

    fn number(&self, fact: &str) -> Option<u32> {
        Some(match fact {
            "buried" => self.buried,
            "kills" => self.kills,
            "dark" => self.dark,
            "credits" => self.credits,
            "reached" => self.reached,
            "read" => self.read,
            _ => return None,
        })
    }
}

/// A beat's condition, read from its `when`.
#[derive(Clone, Debug, PartialEq)]
pub enum Cond {
    Always,
    Deal(u8),
    Forced(u8),
    MarlowAlive,
    MarlowGone,
    AtLeast(String, u32),
    Under(String, u32),
}

pub fn parse(when: &str) -> Option<Cond> {
    let when = when.trim();
    match when {
        "always" => return Some(Cond::Always),
        "marlow_alive" => return Some(Cond::MarlowAlive),
        "marlow_gone" => return Some(Cond::MarlowGone),
        _ => {}
    }
    if let Some(n) = when.strip_prefix("deal:") {
        return n.parse().ok().map(Cond::Deal);
    }
    if let Some(n) = when.strip_prefix("forced:") {
        return n.parse().ok().map(Cond::Forced);
    }
    let known = |fact: &str| RunFacts::default().number(fact).is_some();
    if let Some((fact, n)) = when.split_once(">=") {
        return n.trim().parse().ok().filter(|_| known(fact.trim())).map(|n| Cond::AtLeast(fact.trim().to_string(), n));
    }
    if let Some((fact, n)) = when.split_once('<') {
        return n.trim().parse().ok().filter(|_| known(fact.trim())).map(|n| Cond::Under(fact.trim().to_string(), n));
    }
    None
}

pub fn holds(cond: &Cond, facts: &RunFacts) -> bool {
    let forced = |n: u8| (n >= 1).then(|| facts.ways.get(n as usize - 1).copied()).flatten().unwrap_or(false);
    let have = |n: u8| n >= 1 && n <= facts.pieces;
    match cond {
        Cond::Always => true,
        Cond::Deal(n) => have(*n) && !forced(*n),
        Cond::Forced(n) => have(*n) && forced(*n),
        Cond::MarlowAlive => facts.marlow_alive,
        Cond::MarlowGone => !facts.marlow_alive,
        Cond::AtLeast(fact, n) => facts.number(fact).is_some_and(|v| v >= *n),
        Cond::Under(fact, n) => facts.number(fact).is_some_and(|v| v < *n),
    }
}

/// The sequence for this run: the opening, every beat that is true of it
/// (in the order written, up to the cap), the close. Numbers filled in.
pub fn assemble(facts: &RunFacts) -> Vec<Segment> {
    let d = data();
    let filled = |title: &str, seconds: f32, lines: &[String]| Segment {
        title: facts.fill(title),
        seconds,
        lines: lines.iter().map(|l| facts.fill(l)).collect(),
    };
    let mut out: Vec<Segment> = d.open.iter().map(|s| filled(&s.title, s.seconds, &s.lines)).collect();
    out.extend(
        d.beats
            .iter()
            .filter(|b| parse(&b.when).is_some_and(|c| holds(&c, facts)))
            .take(d.max_beats)
            .map(|b| filled(&b.title, b.seconds, &b.lines)),
    );
    out.extend(d.close.iter().map(|s| filled(&s.title, s.seconds, &s.lines)));
    out
}

/// The short sequence for the dark throwing the ship home.
pub fn reached(facts: &RunFacts) -> Vec<Segment> {
    data().reached.iter().map(|s| Segment { title: facts.fill(&s.title), seconds: s.seconds, lines: s.lines.iter().map(|l| facts.fill(l)).collect() }).collect()
}

pub fn total(segments: &[Segment]) -> f32 {
    segments.iter().map(|s| s.seconds).sum()
}

/// What the screen shows at `t`: the big line (possibly empty), then the
/// lines of the part that is up, as many as have come in.
pub fn lines_at(segments: &[Segment], t: f32) -> Vec<String> {
    let mut start = 0.0;
    for seg in segments {
        if t < start + seg.seconds {
            let into = t - start;
            let shown = if into < 1.0 { 0 } else { (((into - 1.0) / LINE_EVERY) as usize + 1).min(seg.lines.len()) };
            let mut v = vec![seg.title.clone()];
            v.extend(seg.lines.iter().take(shown).cloned());
            return v;
        }
        start += seg.seconds;
    }
    Vec::new()
}

/// What happens when a sequence has played out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    /// The record has been heard: home to Haven, with the closing card.
    Home,
    /// Nothing more: back to where you were (the dark is already sending
    /// you home itself).
    Carry,
}

/// Which script the ending screen is running (narrative::truth draws it).
#[derive(Resource, Default)]
pub enum Playing {
    /// The far-edge ending, with its own hand-timed script.
    #[default]
    Truth,
    Assembled { segments: Vec<Segment>, after: After },
}

/// A sequence has just finished: what to do now that flight has resumed.
#[derive(Resource, Default)]
pub struct JustPlayed(pub Option<After>);

/// Everything the ending reads about the run, in one place: the last
/// sequence, its closing card and the game-over screen all tell the run
/// from the same facts.
#[derive(SystemParam)]
pub struct Run<'w, 's> {
    pub contacts: Res<'w, Contacts>,
    pub galaxy: Res<'w, GalaxyMap>,
    pub stats: Res<'w, Statistics>,
    pub dead: Option<Res<'w, crate::crew::burial::DriftingDead>>,
    pub currency: Res<'w, Currency>,
    crew: Query<'w, 's, (), (With<CrewMember>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
}

impl Run<'_, '_> {
    pub fn facts(&self) -> RunFacts {
        let steps = super::summit::last_step();
        let story = &self.contacts.story;
        RunFacts {
            ways: (1..=steps).map(|n| story.way(n).unwrap_or(false)).collect(),
            pieces: self.contacts.summit.done.min(steps),
            marlow_alive: story.marlow == MarlowFate::Aboard,
            buried: self.dead.as_ref().map_or(self.stats.crew_lost, |d| d.bodies.len() as u32),
            kills: story.ship_kills,
            dark: super::front::taken_count(&self.galaxy) as u32,
            systems: self.galaxy.systems.len() as u32,
            credits: self.currency.credits,
            reached: story.reached,
            read: self.stats.logs_found.len() as u32,
            crew: self.crew.iter().count() as u32,
        }
    }
}

/// The run in a few plain lines, for the screen a lost ship ends on.
pub fn run_lines(facts: &RunFacts) -> Vec<String> {
    let mut v = vec![format!("Summit Record     {} of {}", facts.pieces, super::summit::STEPS_IN_ALL)];
    if facts.pieces > 0 {
        v.push(format!("    {}", facts.ways_in_words()));
    }
    v.push(format!("Lost to the dark  {} of {} systems", facts.dark, facts.systems));
    v.push(format!("Ships destroyed   {}", facts.kills));
    v.push(format!("Crew buried       {}", facts.buried));
    v
}

/// Start the last sequence. The berth at the site calls this
/// (narrative::site), and it happens once a run.
pub fn play_the_record(run: &Run, playing: &mut Playing, next: &mut NextState<GameState>) {
    let facts = run.facts();
    info!("ENDING: the record plays ({:?})", facts);
    *playing = Playing::Assembled { segments: assemble(&facts), after: After::Home };
    next.set(GameState::Truth);
}

/// The dark had the ship (narrative::ward::thrown_out): count it, and say
/// so on the ending's screen. It is already sending you home.
fn the_dark_had_you(
    mut reached_you: MessageReader<super::ward::CascadeReachedYou>,
    mut contacts: ResMut<Contacts>,
    mut playing: ResMut<Playing>,
    mut next: ResMut<NextState<GameState>>,
) {
    if reached_you.read().count() == 0 {
        return;
    }
    contacts.story.reached += 1;
    info!("ENDING: the dark had the ship ({} times now)", contacts.story.reached);
    let facts = RunFacts { reached: contacts.story.reached, ..default() };
    *playing = Playing::Assembled { segments: reached(&facts), after: After::Carry };
    next.set(GameState::Truth);
}

/// The record has played, however it was started: it doesn't play again,
/// and the HUD moves on to what's left.
fn the_record_was_heard(just: Res<JustPlayed>, mut contacts: ResMut<Contacts>) {
    if just.0 == Some(After::Home) && !contacts.story.ending_seen {
        contacts.story.ending_seen = true;
    }
}

/// Back in flight after a sequence: the record heard means home, and the
/// card that closes the demo.
fn after_the_sequence(
    mut commands: Commands,
    mut just: ResMut<JustPlayed>,
    run: Run,
    ship: Query<Entity, (With<Ship>, Without<crate::celestial::components::WarpCharging>)>,
    mut cards: ResMut<LogQueue>,
) {
    let Some(after) = just.0.take() else { return };
    if after != After::Home {
        return;
    }
    let facts = run.facts();
    let card = &data().end_card;
    cards.push_card("END OF THE DEMO", card.title.clone(), facts.fill(&card.text));
    // Home, the way the dark sends you: the jump costs nothing.
    if let Ok(ship) = ship.single() {
        commands.entity(ship).insert(crate::celestial::components::WarpCharging {
            target: crate::celestial::resources::GalaxyWarpTarget::System(0),
            charge_timer: Timer::from_seconds(0.5, TimerMode::Once),
            fuel_cost: 0.0,
        });
    }
}

/// `AFTERLIGHT_ENDING=summit|reached|truth` plays that sequence a few
/// seconds into flight, with the run as it stands.
fn ending_from_env(
    time: Res<Time>,
    run: Run,
    mut playing: ResMut<Playing>,
    mut next: ResMut<NextState<GameState>>,
    mut reached_you: MessageWriter<super::ward::CascadeReachedYou>,
    mut waited: Local<f32>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Ok(which) = std::env::var("AFTERLIGHT_ENDING") else {
        *done = true;
        return;
    };
    *waited += time.delta_secs();
    if *waited < 6.0 {
        return;
    }
    *done = true;
    info!("ENDING: AFTERLIGHT_ENDING={which}");
    match which.as_str() {
        "summit" => play_the_record(&run, &mut playing, &mut next),
        "reached" => {
            reached_you.write(super::ward::CascadeReachedYou { system: None });
        }
        "truth" => {
            *playing = Playing::Truth;
            next.set(GameState::Truth);
        }
        other => warn!("AFTERLIGHT_ENDING: no sequence called '{other}'"),
    }
}

pub struct EndingPlugin;

impl Plugin for EndingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Playing>()
            .init_resource::<JustPlayed>()
            .add_systems(
                Update,
                (ending_from_env, the_dark_had_you, the_record_was_heard, after_the_sequence).chain().run_if(in_state(GameState::Exploring)),
            );
    }
}

#[cfg(test)]
mod ending_tests {
    use super::*;

    /// A handful of runs as different as they come.
    fn runs() -> Vec<RunFacts> {
        let base = RunFacts { ways: vec![false; 4], pieces: 4, systems: 31, crew: 240, ..default() };
        vec![
            base.clone(),
            RunFacts { ways: vec![true; 4], marlow_alive: true, buried: 12, kills: 80, dark: 20, credits: 90_000, reached: 3, read: 23, ..base.clone() },
            RunFacts { ways: vec![true, false, true, false], marlow_alive: true, buried: 1, kills: 19, dark: 5, ..base.clone() },
            RunFacts { ways: vec![false, true, false, true], buried: 0, kills: 20, dark: 6, credits: 5_000, ..base.clone() },
            // An old save picked up at the end: nothing recorded at all.
            RunFacts { ways: Vec::new(), pieces: 4, ..default() },
        ]
    }

    /// Whatever the run, there is a sequence, it has a part for each piece
    /// and one of each pair, and it is put together from this run's facts.
    #[test]
    fn every_run_assembles_an_ending() {
        let d = data();
        for b in &d.beats {
            assert!(parse(&b.when).is_some(), "{}: can't read 'when': {}", b.id, b.when);
        }
        let fixed = d.open.len() + d.close.len();
        for facts in runs() {
            let seq = assemble(&facts);
            let beats = seq.len() - fixed;
            assert!((8..=d.max_beats).contains(&beats), "{beats} beats for {facts:?}");
            for seg in &seq {
                for text in seg.lines.iter().chain([&seg.title]) {
                    assert!(!text.contains('{'), "unfilled: {text}");
                    assert!(text.is_ascii(), "not plain ASCII: {text}");
                }
            }
        }
        // The two ways of taking a piece say different things.
        let (dealt, forced) = (assemble(&runs()[0]), assemble(&runs()[1]));
        assert_ne!(dealt[fixed - d.close.len()].lines, forced[fixed - d.close.len()].lines);
        // Each step's two beats, and each pair, can't both be true.
        for facts in runs() {
            for pair in [("deal:1", "forced:1"), ("deal:4", "forced:4"), ("marlow_alive", "marlow_gone"), ("buried>=1", "buried<1"), ("kills>=20", "kills<20"), ("dark>=6", "dark<6")] {
                let (a, b) = (holds(&parse(pair.0).unwrap(), &facts), holds(&parse(pair.1).unwrap(), &facts));
                assert!(a != b, "{pair:?} for {facts:?}");
            }
        }
    }

    /// Between one and two minutes, whatever the run, and the short one for
    /// the dark is short.
    #[test]
    fn the_ending_fits_its_budget() {
        for facts in runs() {
            let t = total(&assemble(&facts));
            assert!((SHORTEST..=LONGEST).contains(&t), "{t} s for {facts:?}");
        }
        let short = total(&reached(&RunFacts::default()));
        assert!((10.0..=30.0).contains(&short), "{short} s");
    }

    /// Every part has time for all its lines to come in and be read, the
    /// screen has room for them, and it is never blank for long.
    #[test]
    fn no_beat_is_dead_air_for_long() {
        let d = data();
        let all = d.open.iter().chain(&d.close).chain(&d.reached).map(|s| (s.seconds, s.lines.len())).chain(d.beats.iter().map(|b| (b.seconds, b.lines.len())));
        for (seconds, lines) in all {
            assert!((1..=4).contains(&lines), "a part with {lines} lines");
            let last_in = 1.0 + (lines as f32 - 1.0) * LINE_EVERY;
            assert!(seconds >= last_in + 2.5, "{lines} lines in {seconds} s: the last has no time to be read");
        }
        let seq = assemble(&runs()[1]);
        let mut t = 0.0;
        let mut blank = 0.0f32;
        while t < total(&seq) {
            let shown = lines_at(&seq, t);
            assert!(shown.len() <= 6, "more lines than the screen has");
            blank = if shown.iter().all(|l| l.is_empty()) { blank + 0.25 } else { 0.0 };
            assert!(blank <= 1.5, "blank for {blank} s at {t}");
            t += 0.25;
        }
        assert!(lines_at(&seq, total(&seq) + 1.0).is_empty());
    }

    /// The closing card and the lost-ship screen tell the run as it was.
    #[test]
    fn the_run_is_told_as_it_was() {
        let facts = &runs()[2];
        let card = facts.fill(&data().end_card.text);
        assert!(!card.contains('{'), "{card}");
        assert!(card.contains("taken from the courier") && card.contains("the black box earned"), "{card}");
        let lines = run_lines(facts);
        assert!(lines[0].contains("4 of 7"));
        assert!(lines.iter().any(|l| l.contains("5 of 31")));
        // A run that ended early says only what it has.
        let early = RunFacts { ways: vec![true, false, false, false], pieces: 1, systems: 31, ..default() };
        assert!(run_lines(&early)[1].contains("taken from the courier"));
        assert!(!run_lines(&early)[1].contains("black box"));
        assert_eq!(run_lines(&RunFacts::default()).len(), 4, "no pieces, no list of them");
        assert_eq!(data().way_words.len(), crate::narrative::summit::last_step() as usize);
        for text in [&data().carry.title, &data().carry.text, &data().carry.gist, &data().end_card.title, &data().end_card.text, &data().after] {
            assert!(text.is_ascii() && !text.is_empty());
        }
    }
}
