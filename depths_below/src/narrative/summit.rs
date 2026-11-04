//! The Summit Record: what the player is chasing.
//!
//! Before the Cascade there was a peace summit, and its record was cut into
//! pieces that the factions each took home. The Abbess of the Silence, at the
//! capital nearest Haven, asks the ship to bring them to her. Each step is
//! held by one named contact at a faction capital further out, and each can be
//! taken two ways: a deal, or by force. Both are ordinary contact work on the
//! live galaxy (diplomacy::jobs::story_offer); what makes it the story is what
//! happens when it's done. A piece of the record arrives as a reading card,
//! the Abbess says what it means over the comm, and the HUD points at the
//! next capital.
//!
//! The words are data (`data/summit.json`) so they can be edited without
//! touching this file. The demo stops after `DEMO_LAST_STEP`.
//!
//! Progress is a few numbers on `Contacts` (it saves and resets with the
//! people who hold the pieces), and the HUD line is derived from it every
//! frame, so nothing here can drift out of step with a loaded save.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::ai_ship::components::AiShipType;
use crate::celestial::resources::GalaxyMap;
use crate::diplomacy::contacts::{self, ContactDef, Contacts};
use crate::diplomacy::jobs::{Jobs, StoryMark, SummitStepWon, MINUTES_PRICE};
use crate::diplomacy::{FactionStanding, StandingTier};
use crate::resources::{Inventory, ItemType, Unlocks};
use crate::states::GameState;

use super::reader::LogQueue;

/// The last step this build carries. One number, so the demo can stop at 3
/// or at 4 after the playtest with friends.
pub const DEMO_LAST_STEP: u8 = 4;
/// Steps in the whole chain, as the player is told it ("1 of 7").
pub const STEPS_IN_ALL: u8 = 7;
/// The story's hub.
pub const ABBESS: &str = "TheSilence_the_abbess";
/// Seconds of flight before the Silence hail you. Long enough to have left
/// the dock and looked around; the reader holds it until training is over.
const HAIL_AFTER: f32 = 20.0;
/// Where an Unfriendly faction sits once the Abbess has spoken for you:
/// just inside Unfriendly, so their station will let you dock.
const SPOKEN_FOR: f32 = -35.0;

#[derive(Deserialize)]
pub struct Card {
    pub title: String,
    pub text: String,
    /// The one bold line for a player who doesn't read the card.
    #[serde(default)]
    pub gist: String,
}

#[derive(Deserialize)]
pub struct StepDef {
    pub name: String,
    /// Id of the contact who holds the step (data/contacts.json).
    pub contact: String,
    pub pitch: String,
    pub deal: String,
    pub force: String,
    pub piece_title: String,
    /// The piece in one bold line, under ten words: what a player who skips
    /// the card still takes away. Also flashed on the HUD.
    pub gist: String,
    pub piece: String,
    pub abbess: String,
}

#[derive(Deserialize)]
pub struct SummitData {
    pub hail: Card,
    pub commission: String,
    pub steps: Vec<StepDef>,
    pub demo_end: Card,
}

/// Every word of the chain, from `data/summit.json`.
pub fn data() -> &'static SummitData {
    static DATA: OnceLock<SummitData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../../data/summit.json")).expect("data/summit.json is malformed"))
}

/// The last step this build plays.
pub fn last_step() -> u8 {
    DEMO_LAST_STEP.min(data().steps.len() as u8)
}

/// Step `n`, counted from 1.
pub fn step(n: u8) -> Option<&'static StepDef> {
    (n >= 1).then(|| data().steps.get(n as usize - 1)).flatten()
}

/// The contact who holds step `n`.
pub fn contact_of(n: u8) -> Option<&'static ContactDef> {
    let id = &step(n)?.contact;
    contacts::all().iter().find(|c| &c.id == id)
}

/// How far along the chain the player is. Saved on `Contacts`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SummitProgress {
    /// The Silence have called you.
    pub hailed: bool,
    /// The Abbess has asked you for the pieces.
    pub commissioned: bool,
    /// Steps finished.
    pub done: u8,
    /// The end-of-demo card has been shown.
    pub ended: bool,
}

impl SummitProgress {
    /// The step being worked on, once the Abbess has asked and while this
    /// build has one left.
    pub fn current(&self) -> Option<u8> {
        (self.commissioned && self.done < last_step()).then_some(self.done + 1)
    }

    /// Whether `def` holds the step being worked on.
    pub fn holds_current(&self, def: &ContactDef) -> bool {
        self.current().and_then(contact_of).is_some_and(|c| c.id == def.id)
    }
}

/// The system a faction's capital is in (or, if it has lost its capital,
/// any it still holds).
pub fn capital_of(galaxy: &GalaxyMap, faction: AiShipType) -> Option<(u32, String)> {
    let mine = || galaxy.systems.iter().filter(|s| s.faction == Some(faction));
    mine().find(|s| s.capital).or_else(|| mine().next()).map(|s| (s.id, s.name.clone()))
}

/// A line with its `{capital}` (that faction's) and `{price}` filled in.
fn fill(text: &str, galaxy: &GalaxyMap, capital: Option<AiShipType>) -> String {
    let place = capital.and_then(|f| capital_of(galaxy, f)).map_or("a station further out".to_string(), |(_, n)| n);
    text.replace("{capital}", &place).replace("{price}", &MINUTES_PRICE.to_string())
}

/// What the step's contact says when they offer it.
pub fn pitch(n: u8, galaxy: &GalaxyMap) -> String {
    step(n).map_or(String::new(), |s| fill(&s.pitch, galaxy, contact_of(n).map(|c| c.faction)))
}

/// What the Abbess says when she asks for the pieces.
pub fn commission(galaxy: &GalaxyMap) -> String {
    fill(&data().commission, galaxy, contact_of(1).map(|c| c.faction))
}

/// What the Abbess says after piece `n`: what it means, and where to go next.
pub fn abbess_after(n: u8, galaxy: &GalaxyMap) -> String {
    step(n).map_or(String::new(), |s| fill(&s.abbess, galaxy, contact_of(n + 1).map(|c| c.faction)))
}

/// The story's line on the HUD, and the system it points at.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct SummitObjective {
    pub system: Option<u32>,
    pub line: Option<String>,
}

/// Work out the HUD line from where the chain stands.
pub fn objective(progress: &SummitProgress, work: &Jobs, galaxy: &GalaxyMap) -> SummitObjective {
    let name_of = |id: u32| galaxy.systems.iter().find(|s| s.id == id).map_or("somewhere".to_string(), |s| s.name.clone());
    if !progress.hailed && !progress.commissioned {
        return SummitObjective::default();
    }
    let head = format!("SUMMIT RECORD  {} of {}", progress.done, STEPS_IN_ALL);
    if !progress.commissioned {
        let Some((system, name)) = capital_of(galaxy, AiShipType::TheSilence) else { return SummitObjective::default() };
        return SummitObjective { system: Some(system), line: Some(format!("{head}   the Abbess wants you, at {name}")) };
    }
    let Some(n) = progress.current() else {
        return SummitObjective { system: None, line: Some(format!("{head}   the rest lies beyond this demo")) };
    };
    if let Some(job) = work.story().filter(|j| j.story.is_some_and(|m| m.step == n)) {
        let system = job.system();
        return SummitObjective {
            system: Some(system),
            line: Some(format!("{head}   {} ({}), {}", job.title, job.progress(), name_of(system))),
        };
    }
    let (Some(def), Some(who)) = (step(n), contact_of(n)) else { return SummitObjective::default() };
    match capital_of(galaxy, who.faction) {
        Some((system, name)) => SummitObjective {
            system: Some(system),
            line: Some(format!("{head}   {}: see {} at {name}", def.name, who.name)),
        },
        None => SummitObjective { system: None, line: Some(format!("{head}   {}: {} has gone to ground", def.name, who.name)) },
    }
}

fn update_objective(contacts: Res<Contacts>, work: Res<Jobs>, galaxy: Res<GalaxyMap>, mut obj: ResMut<SummitObjective>) {
    let want = objective(&contacts.summit, &work, &galaxy);
    if *obj != want {
        *obj = want;
    }
}

/// A short while after you first fly, the Silence call.
fn hail_from_the_silence(
    time: Res<Time>,
    mut contacts: ResMut<Contacts>,
    galaxy: Res<GalaxyMap>,
    mut queue: ResMut<LogQueue>,
    mut waited: Local<f32>,
) {
    // Read through Deref so a quiet frame doesn't mark Contacts changed (the
    // comm panel redraws on that).
    if contacts.summit.hailed || contacts.summit.commissioned {
        *waited = 0.0;
        return;
    }
    *waited += time.delta_secs();
    if *waited < HAIL_AFTER {
        return;
    }
    contacts.summit.hailed = true;
    let hail = &data().hail;
    queue.push_card("INCOMING", hail.title.clone(), fill(&hail.text, &galaxy, Some(AiShipType::TheSilence)));
    queue.set_gist(&hail.title, hail.gist.clone());
}

/// Plating that comes with a step.
fn grant_plating(unlocks: &mut Unlocks, key: &str, label: &str, notes: &mut MessageWriter<crate::events::ShowNotification>) {
    if unlocks.hull_types.iter().any(|h| h == key) {
        return;
    }
    unlocks.hull_types.push(key.to_string());
    notes.write(crate::events::ShowNotification {
        message: format!("{label} plating unlocked."),
        notification_type: crate::events::NotificationType::Success,
        duration: 6.0,
    });
}

/// What a won step costs and pays, beyond the piece itself.
fn consequences(
    mark: StoryMark,
    standing: &mut FactionStanding,
    unlocks: &mut Unlocks,
    inventory: &mut Inventory,
    notes: &mut MessageWriter<crate::events::ShowNotification>,
) {
    use AiShipType::*;
    // Done their way, a step makes you a friend of the faction that held it,
    // and a friend can shop at their High Command's Military tab
    // (diplomacy::signatures). Taken by force, that door stays shut.
    if !mark.forced {
        if let Some(who) = contact_of(mark.step) {
            let v = standing.value(who.faction);
            if v < StandingTier::FRIENDLY_FROM {
                standing.change(who.faction, StandingTier::FRIENDLY_FROM - v, format!("{} vouches for you", who.name));
                notes.write(crate::events::ShowNotification {
                    message: format!(
                        "{} counts you a friend now: their Military tab is open to you.",
                        crate::ai_ship::components::faction_display_name(who.faction)
                    ),
                    notification_type: crate::events::NotificationType::Success,
                    duration: 7.0,
                });
            }
        }
    }
    match (mark.step, mark.forced) {
        (1, true) => standing.change(MerchantPrinces, -20.0, "You robbed their summit courier"),
        (2, forced) => {
            if forced {
                standing.change(FreeHaulersCompact, -20.0, "You took their black box");
            }
            grant_plating(unlocks, "titanium", "Titanium", notes);
            // The next piece is with the Choir, who shoot strangers on sight.
            let v = standing.value(BrokenChoir);
            if v < SPOKEN_FOR {
                standing.change(BrokenChoir, SPOKEN_FOR - v, "The Abbess spoke for you");
            }
        }
        (3, true) => standing.change(BrokenChoir, -15.0, "You robbed one of their ghost ships"),
        (4, forced) => {
            if forced {
                inventory.remove_item(ItemType::BioSample, 1);
                standing.change(StellarPreserve, -25.0, "You sold their specimen ark to the Collective");
                standing.change(SynthesisCollective, 15.0, "You sold them the Preserve's specimen ark");
            }
            grant_plating(unlocks, "composite", "Composite", notes);
        }
        _ => {}
    }
}

/// A step was won: the piece, the Abbess's reading of it, what it pays, and
/// at the end of this build, the closing card.
#[allow(clippy::too_many_arguments)]
fn advance(
    mut won: MessageReader<SummitStepWon>,
    mut contacts: ResMut<Contacts>,
    mut standing: ResMut<FactionStanding>,
    mut unlocks: ResMut<Unlocks>,
    mut inventory: ResMut<Inventory>,
    mut queue: ResMut<LogQueue>,
    galaxy: Res<GalaxyMap>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    for SummitStepWon(mark) in won.read().copied() {
        // A second report of a step already counted (or one out of order)
        // changes nothing.
        if mark.step != contacts.summit.done + 1 {
            continue;
        }
        let Some(def) = step(mark.step) else { continue };
        info!("SUMMIT step {} won ({})", mark.step, if mark.forced { "by force" } else { "by deal" });
        contacts.summit.done = mark.step;
        // A faction the dark took entirely can't hand its piece over: the
        // Abbess pulls it out herself, and not all of it comes.
        let lost = contact_of(mark.step).is_some_and(|c| capital_of(&galaxy, c.faction).is_none());
        let piece = if lost { format!("{}\n\n(Recovered from the dark by the Abbess. Some of it did not come back.)", def.piece) } else { def.piece.clone() };
        queue.push_card("A PIECE OF THE SUMMIT RECORD", def.piece_title.clone(), piece);
        queue.set_gist(&def.piece_title, def.gist.clone());
        queue.push_card("OVER THE COMM", "THE ABBESS".into(), abbess_after(mark.step, &galaxy));
        consequences(mark, &mut standing, &mut unlocks, &mut inventory, &mut notes);
        notes.write(crate::events::ShowNotification {
            message: format!("Summit record {} of {}: {}", mark.step, STEPS_IN_ALL, def.gist),
            notification_type: crate::events::NotificationType::Success,
            duration: 10.0,
        });
        if contacts.summit.done >= last_step() && !contacts.summit.ended {
            contacts.summit.ended = true;
            let end = &data().demo_end;
            queue.push_card("END OF THE DEMO", end.title.clone(), end.text.clone());
        }
    }
}

/// One row of the Summit Record board (the Log window, L).
#[derive(Debug, Clone, PartialEq)]
pub enum BoardRow {
    /// A piece you have: its number, name and bold line.
    Found(u8, String, String),
    /// The step you're on: its number, name and where to go.
    Next(u8, String, String),
    /// Not yet: its number.
    Unknown(u8),
}

/// The board: what you've learned, what's next, and how much is left. Read
/// at a glance by someone who never opens a card.
pub fn board(progress: &SummitProgress, galaxy: &GalaxyMap) -> Vec<BoardRow> {
    (1..=STEPS_IN_ALL)
        .map(|n| match step(n) {
            Some(s) if n <= progress.done => BoardRow::Found(n, s.name.clone(), s.gist.clone()),
            Some(s) if progress.current() == Some(n) => {
                let where_ = contact_of(n)
                    .map(|c| format!("see {} at {}", c.name, capital_of(galaxy, c.faction).map_or("?".into(), |(_, name)| name)))
                    .unwrap_or_default();
                BoardRow::Next(n, s.name.clone(), where_)
            }
            _ => BoardRow::Unknown(n),
        })
        .collect()
}

/// Where the summit station was: out along the road to the Choir's capital
/// (step 3), so the tear lies on the way outward and never over Haven (the
/// middle of all the capitals sat right on top of it). Worked out from the
/// galaxy, never saved.
pub fn summit_site(galaxy: &GalaxyMap) -> Vec2 {
    capital_of(galaxy, AiShipType::BrokenChoir)
        .and_then(|(id, _)| galaxy.systems.iter().find(|s| s.id == id))
        .map_or(Vec2::new(1_500_000.0, 0.0), |s| s.galaxy_pos * 0.55)
}

/// The tear on the galaxy map where the summit station was, once the black
/// box has said what happened there (step 2), wider with each step after:
/// galaxy units of radius, or None before step 2.
pub fn scar_radius(steps_done: u8) -> Option<f32> {
    (steps_done >= 2).then(|| 250_000.0 + (steps_done - 2) as f32 * 200_000.0)
}

/// The step's faction is gone (the dark took all of it): nobody is left to
/// trade the piece or take it from, so the Abbess recovers it, broken.
fn pieces_from_the_dark(contacts: Res<Contacts>, galaxy: Res<GalaxyMap>, mut won: MessageWriter<SummitStepWon>, mut sent: Local<Option<u8>>) {
    let Some(n) = contacts.summit.current() else { return };
    if *sent == Some(n) {
        return;
    }
    let Some(who) = contact_of(n) else { return };
    if capital_of(&galaxy, who.faction).is_none() && !galaxy.systems.is_empty() {
        *sent = Some(n);
        info!("SUMMIT step {n}: {:?} is gone; the Abbess recovers the piece", who.faction);
        won.write(SummitStepWon(StoryMark { step: n, forced: false }));
    }
}

/// What the old, rich factions call you once the summit minutes are out
/// (step 1): they know what happened to your people, and you are the last.
fn the_last_one(faction: AiShipType) -> Option<&'static str> {
    match faction {
        AiShipType::GildedThrone => Some("Gilded Throne patrol: \"So the last one is still flying. Our clients will want to know.\""),
        AiShipType::MerchantPrinces => Some("Merchant Princes harbourmaster: \"The last one. We had you priced as lost.\""),
        AiShipType::EternalHegemony => Some("Eternal Hegemony, on every channel: \"The last one. Stand down and be counted.\""),
        _ => None,
    }
}

/// After step 1, the first time each old faction sees you, it says so.
fn old_ones_hail(
    contacts: Res<Contacts>,
    galaxy: Res<GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut last: Local<Option<u32>>,
    mut heard: Local<Vec<AiShipType>>,
) {
    if streaming.loaded_system == *last {
        return;
    }
    *last = streaming.loaded_system;
    if contacts.summit.done < 1 {
        return;
    }
    let Some(sys) = streaming.loaded_system.and_then(|id| galaxy.systems.iter().find(|s| s.id == id)) else { return };
    let Some(who) = sys.haunt.or(sys.faction).filter(|f| !heard.contains(f)) else { return };
    let Some(line) = the_last_one(who) else { return };
    heard.push(who);
    notes.write(crate::events::ShowNotification {
        message: line.to_string(),
        notification_type: crate::events::NotificationType::Warning,
        duration: 9.0,
    });
}

/// Debug override, in the house style (`DEPTHS_CASCADE`): `DEPTHS_SUMMIT=2`
/// starts with the Abbess met and two steps done, so a late step can be
/// looked at without flying the whole chain. Applied once per launch.
fn pin_from_env(mut contacts: ResMut<Contacts>, mut standing: ResMut<FactionStanding>, mut done: Local<bool>) {
    if *done {
        return;
    }
    *done = true;
    let Some(n) = std::env::var("DEPTHS_SUMMIT").ok().and_then(|v| v.parse::<u8>().ok()) else { return };
    let n = n.min(last_step());
    contacts.summit = SummitProgress { hailed: true, commissioned: true, done: n, ended: n >= last_step() };
    // As step 2 would have: the Choir let you dock.
    let choir = standing.value(AiShipType::BrokenChoir);
    if n >= 2 && choir < SPOKEN_FOR {
        standing.change(AiShipType::BrokenChoir, SPOKEN_FOR - choir, "DEPTHS_SUMMIT");
    }
    info!("SUMMIT pinned by DEPTHS_SUMMIT: {} steps done", n);
}

pub struct SummitPlugin;

impl Plugin for SummitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SummitObjective>()
            .add_systems(Update, (pin_from_env, hail_from_the_silence, old_ones_hail, pieces_from_the_dark).chain().run_if(in_state(GameState::Exploring)))
            .add_systems(Update, (advance, update_objective).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diplomacy::contacts::ContactRole;

    /// Every step this build plays must be written, held by a story contact
    /// who exists, and have a capital to find them at.
    #[test]
    fn every_step_is_written_and_reachable() {
        assert!(last_step() >= 1);
        assert_eq!(last_step(), DEMO_LAST_STEP, "the demo stops before the steps that are written run out");
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        for n in 1..=last_step() {
            let s = step(n).unwrap();
            let who = contact_of(n).unwrap_or_else(|| panic!("step {n}'s contact {} is not in contacts.json", s.contact));
            assert!(who.story, "{} holds a step but isn't marked as story", who.name);
            assert!(capital_of(&galaxy, who.faction).is_some(), "nowhere to find {}", who.name);
            for text in [&s.pitch, &s.piece, &s.abbess, &s.piece_title, &s.deal, &s.force] {
                assert!(!text.is_empty(), "step {n} has an empty line");
            }
        }
        let hub = contacts::all().iter().find(|c| c.id == ABBESS).expect("the Abbess is in contacts.json");
        assert_eq!(hub.role, ContactRole::Hub);
    }

    /// The game's font has no em-dash or curly quotes; they render as boxes.
    #[test]
    fn the_words_are_plain_ascii() {
        let d = data();
        let mut all = vec![&d.hail.title, &d.hail.text, &d.commission, &d.demo_end.title, &d.demo_end.text];
        for s in &d.steps {
            all.extend([&s.name, &s.pitch, &s.deal, &s.force, &s.piece_title, &s.piece, &s.abbess, &s.gist]);
            assert!(!s.gist.is_empty() && s.gist.split_whitespace().count() <= 10, "{}: the bold line must be under ten words", s.name);
        }
        for text in all {
            assert!(text.is_ascii(), "not plain ASCII: {text}");
        }
    }

    /// Placeholders are always filled: the player must never read "{capital}".
    #[test]
    fn no_placeholder_reaches_the_player() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let mut said = vec![commission(&galaxy), fill(&data().hail.text, &galaxy, Some(AiShipType::TheSilence))];
        for n in 1..=last_step() {
            said.push(pitch(n, &galaxy));
            said.push(abbess_after(n, &galaxy));
        }
        for line in said {
            assert!(!line.contains('{'), "unfilled: {line}");
        }
    }

    /// The story walks outward: each step's capital is no nearer Haven than
    /// the Silence's, where it starts.
    #[test]
    fn the_chain_walks_outward_from_haven() {
        for seed in [1u64, 42, 777] {
            let galaxy = crate::celestial::galaxy::generate_galaxy_map(seed);
            let dist = |f: AiShipType| {
                let (id, _) = capital_of(&galaxy, f).unwrap();
                galaxy.systems.iter().find(|s| s.id == id).unwrap().galaxy_pos.length()
            };
            let home = dist(AiShipType::TheSilence);
            for n in 1..=last_step() {
                assert!(dist(contact_of(n).unwrap().faction) >= home, "seed {seed}: step {n} is nearer Haven than the Abbess");
            }
        }
    }

    /// A fresh run has no story line until the Silence call, then points at
    /// the Abbess, then at each step's contact in turn, and at the end says so.
    #[test]
    fn the_hud_line_follows_the_chain() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let work = Jobs::default();
        let mut p = SummitProgress::default();
        assert_eq!(objective(&p, &work, &galaxy), SummitObjective::default());
        p.hailed = true;
        let silence = capital_of(&galaxy, AiShipType::TheSilence).unwrap().0;
        assert_eq!(objective(&p, &work, &galaxy).system, Some(silence));
        p.commissioned = true;
        for n in 1..=last_step() {
            let o = objective(&p, &work, &galaxy);
            let who = contact_of(n).unwrap();
            assert!(o.line.as_ref().unwrap().contains(&who.name), "step {n}: {:?}", o.line);
            assert_eq!(o.system, capital_of(&galaxy, who.faction).map(|c| c.0));
            p.done = n;
        }
        let end = objective(&p, &work, &galaxy);
        assert!(end.line.unwrap().contains("beyond this demo"));
        assert_eq!(end.system, None);
    }

    fn story_app(progress: SummitProgress) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SummitStepWon>()
            .add_message::<crate::events::ShowNotification>()
            .insert_resource(crate::celestial::galaxy::generate_galaxy_map(42))
            .insert_resource(Contacts { summit: progress, ..default() })
            .init_resource::<FactionStanding>()
            .init_resource::<Unlocks>()
            .init_resource::<Inventory>()
            .init_resource::<LogQueue>()
            .init_resource::<Jobs>()
            .init_resource::<SummitObjective>()
            .add_systems(Update, (advance, update_objective).chain());
        app
    }

    fn win(app: &mut App, step: u8, forced: bool) {
        app.world_mut().write_message(SummitStepWon(StoryMark { step, forced }));
        app.update();
    }

    /// Winning a step delivers its piece and the Abbess's reading, in that
    /// order, and moves the chain on. Winning it twice does nothing more.
    #[test]
    fn a_won_step_delivers_the_piece_then_the_abbess() {
        let mut app = story_app(SummitProgress { hailed: true, commissioned: true, ..default() });
        win(&mut app, 1, false);
        let q = &app.world().resource::<LogQueue>().pending;
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].0, step(1).unwrap().piece_title);
        assert!(q[1].0.contains("ABBESS"));
        assert_eq!(app.world().resource::<Contacts>().summit.done, 1);
        win(&mut app, 1, false);
        assert_eq!(app.world().resource::<LogQueue>().pending.len(), 2, "a step counted twice");
        // Out of order is ignored too.
        win(&mut app, 3, false);
        assert_eq!(app.world().resource::<Contacts>().summit.done, 1);
    }

    /// Step 2 pays Titanium and gets you into the Choir's dock; step 4 pays
    /// Composite. Force costs standing with the people you took it from.
    #[test]
    fn steps_pay_plating_and_force_costs_standing() {
        let mut app = story_app(SummitProgress { hailed: true, commissioned: true, ..default() });
        let princes = app.world().resource::<FactionStanding>().value(AiShipType::MerchantPrinces);
        win(&mut app, 1, true);
        assert!(app.world().resource::<FactionStanding>().value(AiShipType::MerchantPrinces) < princes);
        win(&mut app, 2, false);
        assert!(app.world().resource::<Unlocks>().hull_types.iter().any(|h| h == "titanium"));
        let choir = app.world().resource::<FactionStanding>().tier(AiShipType::BrokenChoir);
        assert_ne!(choir, crate::diplomacy::StandingTier::Hostile, "the Choir still won't let you dock for step 3");
        win(&mut app, 3, false);
        app.world_mut().resource_mut::<Inventory>().add_item(ItemType::BioSample, 1);
        win(&mut app, 4, true);
        assert!(app.world().resource::<Unlocks>().hull_types.iter().any(|h| h == "composite"));
        assert!(!app.world().resource::<Inventory>().items.contains_key(&ItemType::BioSample), "the Collective didn't take the ark");
    }

    /// The deal way makes you a friend of the step's faction, which opens its
    /// military market; force leaves you short of it.
    #[test]
    fn a_deal_opens_the_market_and_force_does_not() {
        let mut app = story_app(SummitProgress { hailed: true, commissioned: true, ..default() });
        win(&mut app, 1, false);
        let s = app.world().resource::<FactionStanding>();
        assert_eq!(s.tier(AiShipType::MerchantPrinces), StandingTier::Friendly);
        win(&mut app, 2, true);
        let s = app.world().resource::<FactionStanding>();
        assert_ne!(s.tier(AiShipType::FreeHaulersCompact), StandingTier::Friendly);
        // The Choir, hostile at the start, become friends through the Cantor.
        win(&mut app, 3, false);
        assert_eq!(app.world().resource::<FactionStanding>().tier(AiShipType::BrokenChoir), StandingTier::Friendly);
    }

    /// The last step of the build closes with the end-of-demo card, once.
    #[test]
    fn the_demo_ends_with_a_card_once() {
        let mut app = story_app(SummitProgress { hailed: true, commissioned: true, done: last_step() - 1, ended: false });
        win(&mut app, last_step(), false);
        let q = &app.world().resource::<LogQueue>().pending;
        assert_eq!(q.last().unwrap().0, data().demo_end.title);
        assert!(app.world().resource::<Contacts>().summit.ended);
        let o = app.world().resource::<SummitObjective>();
        assert!(o.line.as_ref().unwrap().contains("beyond this demo"));
    }

    /// Only the old, rich factions call you the last one, in plain words.
    #[test]
    fn only_the_old_ones_know() {
        for f in crate::celestial::territory::TERRITORIAL {
            let line = the_last_one(f);
            assert_eq!(line.is_some(), matches!(f, AiShipType::GildedThrone | AiShipType::MerchantPrinces), "{f:?}");
        }
        let eternal = the_last_one(AiShipType::EternalHegemony).unwrap();
        assert!(eternal.is_ascii() && eternal.contains("last one"));
    }

    /// No scar until the black box (step 2); after it, wider every step.
    #[test]
    fn the_scar_opens_at_step_two_and_grows() {
        assert_eq!(scar_radius(0), None);
        assert_eq!(scar_radius(1), None);
        assert!(scar_radius(3).unwrap() > scar_radius(2).unwrap());
        // Never over Haven, at its widest in the demo.
        for seed in [1u64, 42, 777] {
            let galaxy = crate::celestial::galaxy::generate_galaxy_map(seed);
            let site = summit_site(&galaxy);
            assert!(site.length() < crate::celestial::galaxy::GALAXY_RADIUS);
            assert!(site.length() > scar_radius(last_step()).unwrap() * 0.8, "seed {seed}: the scar covers Haven");
        }
    }

    /// The board shows what's found, what's next, and hides the rest.
    #[test]
    fn the_board_reads_at_a_glance() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let p = SummitProgress { hailed: true, commissioned: true, done: 1, ended: false };
        let rows = board(&p, &galaxy);
        assert_eq!(rows.len(), STEPS_IN_ALL as usize);
        assert!(matches!(&rows[0], BoardRow::Found(1, _, gist) if gist == &step(1).unwrap().gist));
        assert!(matches!(&rows[1], BoardRow::Next(2, _, w) if w.contains(&contact_of(2).unwrap().name)));
        assert!(rows[2..].iter().all(|r| matches!(r, BoardRow::Unknown(_))));
    }

    /// A step whose faction the dark took entirely arrives anyway, broken.
    #[test]
    fn a_lost_faction_still_gives_up_its_piece() {
        let mut app = story_app(SummitProgress { hailed: true, commissioned: true, ..default() });
        app.add_systems(Update, pieces_from_the_dark.before(advance));
        let princes = contact_of(1).unwrap().faction;
        for s in app.world_mut().resource_mut::<GalaxyMap>().systems.iter_mut().filter(|s| s.faction == Some(princes)) {
            s.faction = None;
            s.capital = false;
        }
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Contacts>().summit.done, 1);
        let q = &app.world().resource::<LogQueue>().pending;
        assert!(q[0].1.contains("Recovered from the dark"), "{}", q[0].1);
    }

    /// The progress survives a save: it rides on Contacts, which is saved.
    #[test]
    fn progress_round_trips_through_the_save_format() {
        let mut c = Contacts::default();
        c.summit = SummitProgress { hailed: true, commissioned: true, done: 2, ended: false };
        let back: Contacts = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.summit, c.summit);
        // And a save from before the story existed loads as a fresh chain.
        let old: Contacts = serde_json::from_str(r#"{"known":[]}"#).unwrap();
        assert_eq!(old.summit, SummitProgress::default());
    }
}
