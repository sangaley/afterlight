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
use crate::ai_ship::components::{faction_display_name, WorldSimulation};
use crate::celestial::territory::BorderWar;
use crate::diplomacy::jobs::{self, Job, JobKind, JobStatus, Jobs, StoryMark, SummitStepWon, MINUTES_PRICE};
use crate::diplomacy::{FactionStanding, StandingTier};
use crate::resources::{Currency, Inventory, ItemType, Unlocks};
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
/// The Abbess's first jobs, after she asks for the pieces: ordinary work
/// that pays the Princes' price for the minutes (step 1).
pub const ERRANDS: u8 = 3;
/// What each pays. With the credits a run starts on, the three cover
/// `MINUTES_PRICE` (see `the_abbess_first_work_covers_the_minutes`).
pub const ERRAND_PAY: [u32; ERRANDS as usize] = [600, 700, 800];

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

/// One of the Abbess's first jobs, as she words it.
#[derive(Deserialize)]
pub struct ErrandDef {
    pub title: String,
    pub brief: String,
}

#[derive(Deserialize)]
pub struct SummitData {
    pub hail: Card,
    pub commission: String,
    pub errands: Vec<ErrandDef>,
    /// The errand that can always be done, when the galaxy has nothing
    /// fitting for one of the three.
    pub errand_fallback: ErrandDef,
    /// Said over the comm as the next errand comes in, and when the last is
    /// done.
    pub errand_next: String,
    pub errands_done: String,
    /// What she says over a relay while she still wants you in person.
    pub relay_wait: String,
    /// What a free port's Comm tab says before anyone has called.
    pub relay_idle: String,
    /// Said when she has spoken for you to a faction that shut you out.
    pub spoken_for: String,
    pub steps: Vec<StepDef>,
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

/// Whether the Abbess is still handing out her first jobs: she has asked
/// for the pieces, the first isn't won yet, and she has work left.
pub fn errands_open(contacts: &Contacts) -> bool {
    contacts.summit.commissioned && contacts.summit.done == 0 && contacts.story.errands < ERRANDS
}

/// Whether the Abbess will ask for the pieces where you are talking to her.
/// At her own desk, always. Over a free port's relay only if she has no desk
/// left: the Silence can lose every system before you reach them, and the
/// chain must still be able to start.
pub fn commission_here(relay: bool, galaxy: &GalaxyMap) -> bool {
    !relay || capital_of(galaxy, AiShipType::TheSilence).is_none()
}

/// What she says over a relay while she still wants you in person.
pub fn relay_wait(galaxy: &GalaxyMap) -> String {
    fill(&data().relay_wait, galaxy, Some(AiShipType::TheSilence))
}

/// The Abbess speaks for you to the faction that holds the step you're on,
/// if it has shut you out: its standing comes up to just inside Unfriendly,
/// where its station lets you dock. Returns the faction if she had to.
pub fn speak_for(progress: &SummitProgress, standing: &mut FactionStanding) -> Option<AiShipType> {
    let who = progress.current().and_then(contact_of)?;
    let v = standing.value(who.faction);
    (v < SPOKEN_FOR).then(|| {
        standing.change(who.faction, SPOKEN_FOR - v, "The Abbess spoke for you");
        who.faction
    })
}

/// What the story's line points at once you're in its system.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Aim {
    /// Being there is the point (a survey, a strike, a wait).
    #[default]
    Nowhere,
    /// The system's starport: someone to see, or cargo to hand over.
    Port,
    /// A fleet (ai_ship::fleet id) to find.
    Fleet(u32),
}

/// The story's line on the HUD, the system it points at, and what to look
/// for there (the galaxy map and the edge-of-screen marker read these).
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct SummitObjective {
    pub system: Option<u32>,
    pub line: Option<String>,
    pub aim: Aim,
    /// A word or two for the marker and the map: who or what is there.
    pub tag: String,
    /// Where the goal is when it is no system: a point on the galaxy map
    /// (the summit site, narrative::site).
    pub place: Option<Vec2>,
}

/// A name's last word, for a marker with room for one: "Quay", "Cantor".
fn short(name: &str) -> &str {
    name.split_whitespace().last().unwrap_or(name)
}

/// What a job has you look for in its system.
fn aim_of(job: &Job) -> Aim {
    match job.kind {
        JobKind::Bounty { fleet, .. } | JobKind::Raid { fleet, .. } | JobKind::Escort { fleet, .. } => Aim::Fleet(fleet),
        JobKind::Delivery { .. } | JobKind::Theft { taken: true, .. } => Aim::Port,
        _ => Aim::Nowhere,
    }
}

/// Work out the HUD line from where the chain stands: one goal, the next
/// thing to do, and where. `credits` is what you have, for the one step that
/// has a price.
pub fn objective(contacts: &Contacts, work: &Jobs, galaxy: &GalaxyMap, standing: &FactionStanding, credits: u32) -> SummitObjective {
    let progress = &contacts.summit;
    let name_of = |id: u32| galaxy.systems.iter().find(|s| s.id == id).map_or("somewhere".to_string(), |s| s.name.clone());
    let at = |system: Option<u32>, line: String, aim: Aim, tag: &str| SummitObjective { system, line: Some(line), aim, tag: tag.to_string(), place: None };
    if !progress.hailed && !progress.commissioned {
        return SummitObjective::default();
    }
    let head = format!("SUMMIT RECORD  {} of {}", progress.done, STEPS_IN_ALL);
    let silence = capital_of(galaxy, AiShipType::TheSilence);
    if !progress.commissioned {
        return match &silence {
            Some((system, name)) => at(Some(*system), format!("{head}   the Abbess wants you, at {name}"), Aim::Port, "Abbess"),
            // Her house is gone before you reached it: she takes the call
            // on any free port's relay (commission_here).
            None => at(None, format!("{head}   the Abbess wants you: call her on Haven's relay"), Aim::Nowhere, ""),
        };
    }
    let Some(n) = progress.current() else {
        let story = &contacts.story;
        // The pieces this build has are in hand. First they are carried to
        // where the summit was; after that, the galaxy and the dark go on.
        if story.carry && !story.ending_seen {
            let line = format!("{head}   carry the record into the tear on your map, to the empty berth\nIt is dark there: the warp drive needs a Dark Ward aboard.");
            return SummitObjective { place: story.site, ..at(None, line, Aim::Nowhere, "the site") };
        }
        let lit = galaxy.systems.len().saturating_sub(super::front::taken_count(galaxy));
        let after = super::ending::data().after.replace("{lit}", &lit.to_string());
        return at(None, format!("{head}   {after}"), Aim::Nowhere, "");
    };
    if let Some(job) = work.story().filter(|j| j.story.is_some_and(|m| m.step == n)) {
        let system = job.system();
        return at(Some(system), format!("{head}   {} ({}), {}", job.title, job.progress(), name_of(system)), aim_of(job), "story");
    }
    let (Some(def), Some(who)) = (step(n), contact_of(n)) else { return SummitObjective::default() };
    let Some((system, name)) = capital_of(galaxy, who.faction) else {
        return at(None, format!("{head}   {}: {} has gone to ground", def.name, who.name), Aim::Nowhere, "");
    };
    let see = format!("{head}   {}: see {} at {name}", def.name, who.name);
    // They shoot you on sight and won't open the dock: the Abbess can fix
    // that from her desk or over a relay.
    if standing.tier(who.faction) == StandingTier::Hostile {
        let (to, via) = match &silence {
            Some((id, home)) => (Some(*id), format!("at {home} or on Haven's relay")),
            None => (None, "on Haven's relay".to_string()),
        };
        let shut = format!("{} will not let you dock. Call the Abbess, {via}: she can speak for you.", faction_display_name(who.faction));
        return at(to, format!("{see}\n{shut}"), Aim::Port, "Abbess");
    }
    // The minutes cost money, and her errands are how it's earned.
    if n == 1 && credits < MINUTES_PRICE {
        let price = format!("{MINUTES_PRICE} cr for the minutes, you have {credits}.");
        let errand = work.active().find(|j| j.giver == ABBESS && j.story.is_none()).filter(|_| errands_open(contacts));
        if let Some(job) = errand {
            let to = job.system();
            return at(Some(to), format!("{see}\n{price}  {} ({}), {}", job.title, job.progress(), name_of(to)), aim_of(job), "errand");
        }
        if errands_open(contacts) {
            return at(silence.map(|(id, _)| id), format!("{see}\n{price}  The Abbess has paying work for you."), Aim::Port, "Abbess");
        }
        return at(Some(system), format!("{see}\n{price}  Or take them by force."), Aim::Port, short(&who.name));
    }
    at(Some(system), see, Aim::Port, short(&who.name))
}

fn update_objective(
    contacts: Res<Contacts>,
    work: Res<Jobs>,
    galaxy: Res<GalaxyMap>,
    standing: Res<FactionStanding>,
    currency: Res<Currency>,
    mut obj: ResMut<SummitObjective>,
    at_site: Option<Res<super::site::AtSite>>,
    streaming: Option<Res<crate::celestial::resources::SystemStreamingManager>>,
) {
    let mut want = objective(&contacts, &work, &galaxy, &standing, currency.credits);
    // The first trip of the story is also the first jump anyone makes: say
    // how it is done, not only where to.
    if let Some(how) = how_to_reach_her(&contacts.summit, &want, streaming.as_ref().and_then(|s| s.loaded_system)) {
        want.line = want.line.map(|line| format!("{line}\n{how}"));
    }
    // There already: the line stops talking about getting there (the empty
    // berth has its own marker at the edge of the screen).
    if at_site.is_some_and(|s| s.0) && contacts.story.carry && !contacts.story.ending_seen {
        want.line = Some(format!("SUMMIT RECORD  {} of {}   bring the ship into the empty berth\nThe ward will not hold for long.", contacts.summit.done, STEPS_IN_ALL));
    }
    if *obj != want {
        *obj = want;
    }
}

/// The keys for the story's first trip, until the Abbess has been met:
/// to her system by the map and the jump drive, and once there, to her by
/// the dock and the Comm directory. None once she has asked for the pieces,
/// or if she has no house to go to.
pub fn how_to_reach_her(progress: &SummitProgress, goal: &SummitObjective, here: Option<u32>) -> Option<String> {
    if !progress.hailed || progress.commissioned {
        return None;
    }
    let system = goal.system?;
    Some(if here == Some(system) {
        "Dock at the Silence's starport [F], turn to the Comm directory [E], and talk to her [Enter].".to_string()
    } else {
        "Open the map [M], click that system, close the map and jump [G]. Then dock there [F].".to_string()
    })
}

/// Where the story's line points inside the system you're in: an unseen
/// entity that the edge-of-screen marker reads (ui::offscreen_markers), with
/// the word or two it shows. There is one while the goal is somewhere to go
/// in this system, and none otherwise.
#[derive(Component)]
pub struct StoryTarget(pub String);

/// Something else of the story's worth finding in this system, with the
/// word the edge-of-screen marker shows for it: the wreck of the Meridian
/// (narrative::beats). Marked the same way as the goal, beside it.
#[derive(Component)]
pub struct StoryBeacon(pub String);

/// Keep the story's target where the goal is: the system's starport for
/// someone to see or cargo to hand over, the middle of a fleet to be found.
fn place_story_target(
    mut commands: Commands,
    objective: Res<SummitObjective>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    stations: Res<crate::diplomacy::stations::FactionStations>,
    sim: Res<WorldSimulation>,
    mut target: Query<(Entity, &mut Transform, &mut StoryTarget)>,
) {
    let here = streaming.loaded_system;
    let at = objective.system.filter(|s| Some(*s) == here).and_then(|system| match objective.aim {
        Aim::Port => stations.post.filter(|p| p.0 == system).map(|p| p.1),
        Aim::Fleet(fleet) => {
            let ships: Vec<Vec2> = sim
                .ships
                .iter()
                .filter(|s| s.fleet == fleet && s.system_id == system && s.behavior != crate::ai_ship::components::SimBehavior::Dead)
                .map(|s| s.position)
                .collect();
            (!ships.is_empty()).then(|| ships.iter().sum::<Vec2>() / ships.len() as f32)
        }
        Aim::Nowhere => None,
    });
    match (at, target.single_mut()) {
        (Some(p), Ok((_, mut tf, mut tag))) => {
            tf.translation = p.extend(0.0);
            if tag.0 != objective.tag {
                tag.0 = objective.tag.clone();
            }
        }
        (Some(p), Err(_)) => {
            commands.spawn((Transform::from_translation(p.extend(0.0)), StoryTarget(objective.tag.clone())));
        }
        (None, Ok((entity, ..))) => commands.entity(entity).despawn(),
        (None, Err(_)) => {}
    }
}

/// The Abbess's first jobs come one after another over the comm: the first
/// is taken at her desk, and as each is finished the next is simply in hand,
/// so nobody flies back to her three times. When the last is done she says
/// the price is covered.
fn the_abbess_has_more_work(
    mut contacts: ResMut<Contacts>,
    mut work: ResMut<Jobs>,
    world: (Res<WorldSimulation>, Res<GalaxyMap>, Res<BorderWar>),
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    // Read through Deref so a quiet frame doesn't mark Contacts changed.
    let (done, given) = (contacts.story.errands, contacts.story.errands_given);
    if !contacts.summit.commissioned || contacts.summit.done > 0 || done == 0 || done != given {
        return;
    }
    let say = |notes: &mut MessageWriter<crate::events::ShowNotification>, message: String| {
        notes.write(crate::events::ShowNotification { message, notification_type: crate::events::NotificationType::Info, duration: 9.0 });
    };
    if done >= ERRANDS {
        contacts.story.errands_given = ERRANDS + 1;
        say(&mut notes, data().errands_done.clone());
        return;
    }
    let (sim, galaxy, war) = (&world.0, &world.1, &world.2);
    // With her house gone there is nowhere to send you from: the rest wait
    // until you ask her over a relay.
    let Some((home, _)) = capital_of(galaxy, AiShipType::TheSilence) else { return };
    let at = jobs::World { sim, galaxy, war, here: home };
    let mut job = jobs::errand_offer(done, &at, &mut rand::thread_rng());
    // Cargo can't come aboard over the comm.
    if matches!(job.kind, JobKind::Delivery { .. }) {
        return;
    }
    job.status = JobStatus::Active;
    info!("SUMMIT errand {} of {ERRANDS} over the comm: {} - {}", done + 1, job.title, job.brief);
    say(&mut notes, format!("{}  {}", data().errand_next, job.brief));
    work.push(job);
    contacts.story.errands_given = done + 1;
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
            notes.write(crate::events::ShowNotification {
                message: "The Abbess's Dark Ward pattern is in your build menu (Utility).".into(),
                notification_type: crate::events::NotificationType::Success,
                duration: 7.0,
            });
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
    mut work: ResMut<Jobs>,
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
        contacts.story.record_way(mark.step, mark.forced);
        // Whatever work was still in hand for this step is over: paying for
        // the minutes with a raid on the courier running left the raid
        // running, for nothing.
        if work.list.iter().any(|j| j.story.is_some_and(|m| m.step <= mark.step)) {
            work.list.retain(|j| !j.story.is_some_and(|m| m.step <= mark.step));
        }
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
        // The last piece this build has: she asks for them to be carried to
        // where the summit was (narrative::site), where the demo closes
        // (narrative::ending). No goodbye card: there is one more thing to do.
        if contacts.summit.done >= last_step() && !contacts.summit.ended {
            contacts.summit.ended = true;
            contacts.story.carry = true;
            let card = &super::ending::data().carry;
            queue.push_card("OVER THE COMM", card.title.clone(), card.text.clone());
            queue.set_gist(&card.title, card.gist.clone());
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
pub fn board(progress: &SummitProgress, work: &Jobs, galaxy: &GalaxyMap) -> Vec<BoardRow> {
    let name_of = |id: u32| galaxy.systems.iter().find(|s| s.id == id).map_or("somewhere".to_string(), |s| s.name.clone());
    (1..=STEPS_IN_ALL)
        .map(|n| match step(n) {
            Some(s) if n <= progress.done => BoardRow::Found(n, s.name.clone(), s.gist.clone()),
            Some(s) if progress.current() == Some(n) => {
                // The work in hand for it, if you've taken it; else who to see.
                let where_ = match work.story().filter(|j| j.story.is_some_and(|m| m.step == n)) {
                    Some(job) => format!("{} ({}), {}", job.title, job.progress(), name_of(job.system())),
                    None => contact_of(n)
                        .map(|c| format!("see {} at {}", c.name, capital_of(galaxy, c.faction).map_or("?".into(), |(_, name)| name)))
                        .unwrap_or_default(),
                };
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
fn pieces_from_the_dark(contacts: Res<Contacts>, galaxy: Res<GalaxyMap>, mut won: MessageWriter<SummitStepWon>) {
    let Some(n) = contacts.summit.current() else { return };
    let Some(who) = contact_of(n) else { return };
    // No memory of having sent it: `advance` counts a step once however
    // often it's reported, and a memory kept here outlived the run it was
    // made in.
    if capital_of(&galaxy, who.faction).is_none() && !galaxy.systems.is_empty() {
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
    mut contacts: ResMut<Contacts>,
    galaxy: Res<GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut last: Local<Option<u32>>,
) {
    if streaming.loaded_system == *last {
        return;
    }
    *last = streaming.loaded_system;
    if contacts.summit.done < 1 {
        return;
    }
    let Some(sys) = streaming.loaded_system.and_then(|id| galaxy.systems.iter().find(|s| s.id == id)) else { return };
    // Who has said it is part of the run (narrative::story), so it is said
    // once a run and survives a save.
    let Some(who) = sys.haunt.or(sys.faction).filter(|f| !contacts.story.heard_last_one.contains(f)) else { return };
    let Some(line) = the_last_one(who) else { return };
    contacts.story.heard_last_one.push(who);
    // It counts as something happening, for the beats' spacing.
    super::beats::note(&mut contacts.story);
    notes.write(crate::events::ShowNotification {
        message: line.to_string(),
        notification_type: crate::events::NotificationType::Warning,
        duration: 9.0,
    });
}

/// Debug override, in the house style (`AFTERLIGHT_CASCADE`): `AFTERLIGHT_SUMMIT=2`
/// starts with the Abbess met and two steps done, so a late step can be
/// looked at without flying the whole chain. `AFTERLIGHT_SUMMIT=0` is the
/// Abbess met and nothing done: her first jobs. Applied once per launch.
fn pin_from_env(mut contacts: ResMut<Contacts>, mut standing: ResMut<FactionStanding>, mut currency: ResMut<Currency>, mut done: Local<bool>) {
    if *done {
        return;
    }
    *done = true;
    // `AFTERLIGHT_CREDITS=2500` starts with that much: to look at the first
    // step with and without its price in hand.
    if let Some(credits) = std::env::var("AFTERLIGHT_CREDITS").ok().and_then(|v| v.parse::<u32>().ok()) {
        currency.credits = credits;
        info!("SUMMIT credits set by AFTERLIGHT_CREDITS: {credits}");
    }
    let Some(n) = std::env::var("AFTERLIGHT_SUMMIT").ok().and_then(|v| v.parse::<u8>().ok()) else { return };
    let n = n.min(last_step());
    contacts.summit = SummitProgress { hailed: true, commissioned: true, done: n, ended: n >= last_step() };
    // Steps skipped this way were deals, and her first jobs are behind you.
    contacts.story.ways = vec![false; n as usize];
    // And Marlow has had his say about all of it already.
    let mut said = vec!["hail", "commission"];
    if n > 0 {
        said.push("errands_done");
    }
    said.extend(["piece_1", "piece_2", "piece_3", "piece_4"].into_iter().take(n as usize));
    contacts.story.beats_fired.extend(said.into_iter().map(|m| format!("marlow:{m}")));
    if n > 0 {
        contacts.story.errands = ERRANDS;
        contacts.story.errands_given = ERRANDS + 1;
    }
    // With the last piece in hand she has asked for them to be carried.
    contacts.story.carry = n >= last_step();
    // As step 2 would have: the Choir let you dock.
    let choir = standing.value(AiShipType::BrokenChoir);
    if n >= 2 && choir < SPOKEN_FOR {
        standing.change(AiShipType::BrokenChoir, SPOKEN_FOR - choir, "AFTERLIGHT_SUMMIT");
    }
    info!("SUMMIT pinned by AFTERLIGHT_SUMMIT: {} steps done", n);
}

pub struct SummitPlugin;

impl Plugin for SummitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SummitObjective>()
            .add_systems(Update, (pin_from_env, hail_from_the_silence, old_ones_hail, pieces_from_the_dark).chain().run_if(in_state(GameState::Exploring)))
            .add_systems(Update, (advance, the_abbess_has_more_work, update_objective, place_story_target).chain());
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
        let mut all = vec![&d.hail.title, &d.hail.text, &d.commission];
        all.extend([&d.errand_fallback.title, &d.errand_fallback.brief, &d.errand_next, &d.errands_done, &d.relay_wait, &d.relay_idle, &d.spoken_for]);
        for e in &d.errands {
            all.extend([&e.title, &e.brief]);
        }
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
        let mut said = vec![commission(&galaxy), fill(&data().hail.text, &galaxy, Some(AiShipType::TheSilence)), relay_wait(&galaxy)];
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
        // Rich, and nobody's enemy: nothing stands between you and each step.
        let mut friends = FactionStanding::default();
        for n in 1..=last_step() {
            let f = contact_of(n).unwrap().faction;
            let v = friends.value(f);
            friends.change(f, -v, "test");
        }
        let see = |c: &Contacts| objective(c, &work, &galaxy, &friends, 10_000);
        let mut c = Contacts::default();
        assert_eq!(see(&c), SummitObjective::default());
        c.summit.hailed = true;
        let silence = capital_of(&galaxy, AiShipType::TheSilence).unwrap().0;
        assert_eq!(see(&c).system, Some(silence));
        c.summit.commissioned = true;
        for n in 1..=last_step() {
            let o = see(&c);
            let who = contact_of(n).unwrap();
            assert!(o.line.as_ref().unwrap().contains(&who.name), "step {n}: {:?}", o.line);
            assert_eq!(o.system, capital_of(&galaxy, who.faction).map(|c| c.0));
            assert_eq!(o.aim, Aim::Port, "step {n}: someone to see at a starport");
            c.summit.done = n;
        }
        // The last piece in hand and nothing asked (a save from before the
        // site existed, for the frame until it is): the line still says
        // something, and names how much of the galaxy is left.
        let end = see(&c);
        assert!(end.line.unwrap().contains("still lit"));
        assert_eq!(end.system, None);
        // Asked to carry them: the line points into the tear, at the site.
        c.story.carry = true;
        c.story.site = Some(Vec2::new(5.0, 6.0));
        let carry = see(&c);
        let line = carry.line.unwrap();
        assert!(line.contains("tear") && line.contains("Dark Ward"), "{line}");
        assert_eq!((carry.system, carry.place), (None, Some(Vec2::new(5.0, 6.0))));
        // The record heard: the standing line.
        c.story.ending_seen = true;
        let after = see(&c);
        assert!(after.line.unwrap().contains("still lit"));
        assert_eq!(after.place, None);
    }

    /// Until the Abbess has been met the line says how to get to her, by
    /// which keys: first the jump, then the dock and the Comm directory.
    /// After that it stops explaining.
    #[test]
    fn the_first_trip_says_how() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let (work, standing) = (Jobs::default(), FactionStanding::default());
        let mut c = Contacts::default();
        let goal = |c: &Contacts| objective(c, &work, &galaxy, &standing, 750);
        assert_eq!(how_to_reach_her(&c.summit, &goal(&c), Some(0)), None, "advice before anyone has called");
        c.summit.hailed = true;
        let silence = capital_of(&galaxy, AiShipType::TheSilence).unwrap().0;
        let away = how_to_reach_her(&c.summit, &goal(&c), Some(0)).expect("no word on how to get there");
        assert!(away.contains("[M]") && away.contains("[G]"), "{away}");
        let there = how_to_reach_her(&c.summit, &goal(&c), Some(silence)).unwrap();
        assert!(there.contains("[F]") && there.contains("Comm") && there.contains("[Enter]"), "{there}");
        assert!(away.is_ascii() && there.is_ascii());
        c.summit.commissioned = true;
        assert_eq!(how_to_reach_her(&c.summit, &goal(&c), Some(silence)), None, "still explaining after she has asked");
    }

    /// Short of the Princes' price, the line says what it costs and what you
    /// have, and points at the Abbess's work while she has any: at her, then
    /// at the errand in hand. With the price in your pocket it says neither.
    #[test]
    fn the_hud_says_what_the_minutes_cost() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let standing = FactionStanding::default();
        let silence = capital_of(&galaxy, AiShipType::TheSilence).unwrap().0;
        let princes = capital_of(&galaxy, contact_of(1).unwrap().faction).unwrap().0;
        let mut c = Contacts::default();
        c.summit = SummitProgress { hailed: true, commissioned: true, ..default() };
        let mut work = Jobs::default();

        let poor = objective(&c, &work, &galaxy, &standing, 750);
        let line = poor.line.unwrap();
        assert!(line.contains(&MINUTES_PRICE.to_string()) && line.contains("you have 750"), "{line}");
        assert!(line.contains("Abbess has paying work"), "{line}");
        assert_eq!(poor.system, Some(silence), "short of the price, the line points at her");

        // An errand in hand: the line names it and points where it is.
        let world_sim = WorldSimulation::default();
        let war = BorderWar::default();
        let at = jobs::World { sim: &world_sim, galaxy: &galaxy, war: &war, here: silence };
        let mut errand = jobs::errand_offer(2, &at, &mut rand::thread_rng());
        errand.status = JobStatus::Active;
        let (title, to) = (errand.title.clone(), errand.system());
        work.push(errand);
        c.story.errands_given = 1;
        let working = objective(&c, &work, &galaxy, &standing, 750);
        assert!(working.line.unwrap().contains(&title));
        assert_eq!(working.system, Some(to));

        // Paid up: just the broker.
        let rich = objective(&c, &work, &galaxy, &standing, MINUTES_PRICE);
        assert!(!rich.line.unwrap().contains("for the minutes"));
        assert_eq!(rich.system, Some(princes));

        // Her work done and the money spent anyway: the other way is named.
        c.story.errands = ERRANDS;
        work.list.clear();
        let spent = objective(&c, &work, &galaxy, &standing, 10);
        assert!(spent.line.unwrap().contains("by force"));
        assert_eq!(spent.system, Some(princes));
    }

    /// The three errands, on top of what a run starts with, pay for the
    /// minutes: the first step is never a grind of strangers' work.
    #[test]
    fn the_abbess_first_work_covers_the_minutes() {
        assert_eq!(data().errands.len(), ERRANDS as usize);
        let earned: u32 = ERRAND_PAY.iter().sum();
        assert!(Currency::default().credits + earned >= MINUTES_PRICE, "{earned} cr of errands doesn't reach {MINUTES_PRICE}");
        // Each is real work, hers, at its fixed pay, somewhere that exists
        // and isn't the room you're standing in -- in an empty galaxy and a
        // populated one.
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let here = capital_of(&galaxy, AiShipType::TheSilence).unwrap().0;
        let mut full = WorldSimulation::default();
        crate::ai_ship::simulation::ensure_system_population(&mut full, &galaxy, here);
        for n in crate::celestial::galaxy::nearest_neighbors(&galaxy, here, 3) {
            crate::ai_ship::simulation::ensure_system_population(&mut full, &galaxy, n);
        }
        let war = BorderWar::default();
        for sim in [&WorldSimulation::default(), &full] {
            let at = jobs::World { sim, galaxy: &galaxy, war: &war, here };
            for n in 0..ERRANDS {
                let job = jobs::errand_offer(n, &at, &mut rand::thread_rng());
                assert_eq!((job.giver.as_str(), job.reward, job.story), (ABBESS, ERRAND_PAY[n as usize], None));
                assert_eq!(job.status, JobStatus::Offered);
                assert_ne!(job.system(), here, "errand {n} sends you nowhere");
                assert!(galaxy.systems.iter().any(|s| s.id == job.system()));
                for text in [&job.title, &job.brief] {
                    assert!(!text.contains('{') && text.is_ascii(), "errand {n}: {text}");
                }
            }
        }
    }

    /// The first errand is taken at her desk; as each is done the next is
    /// in hand without a trip back, and after the third she says it's
    /// covered, once.
    #[test]
    fn the_errands_follow_one_another_over_the_comm() {
        let mut app = story_app(SummitProgress { hailed: true, commissioned: true, ..default() });
        app.init_resource::<WorldSimulation>().init_resource::<BorderWar>().add_systems(Update, the_abbess_has_more_work);
        // Nothing comes unasked before the first is taken and done.
        app.update();
        assert!(app.world().resource::<Jobs>().list.is_empty());
        let finish = |app: &mut App| {
            app.world_mut().resource_mut::<Contacts>().story.errands += 1;
            app.world_mut().resource_mut::<Jobs>().list.clear();
            app.update();
        };
        app.world_mut().resource_mut::<Contacts>().story.errands_given = 1;
        finish(&mut app);
        for done in 1..ERRANDS {
            let work = app.world().resource::<Jobs>();
            let job = work.of(ABBESS).unwrap_or_else(|| panic!("no errand came after the {done}th"));
            assert_eq!((job.status, job.reward), (JobStatus::Active, ERRAND_PAY[done as usize]));
            assert_eq!(app.world().resource::<Contacts>().story.errands_given, done + 1);
            // A second look changes nothing while it's in hand.
            app.update();
            assert_eq!(app.world().resource::<Jobs>().list.len(), 1);
            finish(&mut app);
        }
        assert!(app.world().resource::<Jobs>().list.is_empty(), "a fourth errand");
        let said = app.world().resource::<Messages<crate::events::ShowNotification>>().len();
        app.update();
        app.update();
        assert!(!errands_open(app.world().resource::<Contacts>()));
        assert!(said >= 1);
    }

    /// A faction that shoots you on sight can't be docked with: the line
    /// says so and sends you to the Abbess, who brings them up to where
    /// their station opens, and no further.
    #[test]
    fn a_hostile_holder_gets_a_hint() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let work = Jobs::default();
        let mut c = Contacts::default();
        c.summit = SummitProgress { hailed: true, commissioned: true, done: 2, ended: false };
        let choir = contact_of(3).unwrap().faction;
        let mut standing = FactionStanding::default();
        let v = standing.value(choir);
        standing.change(choir, -80.0 - v, "test");
        assert_eq!(standing.tier(choir), StandingTier::Hostile);
        let o = objective(&c, &work, &galaxy, &standing, 10_000);
        let line = o.line.unwrap();
        assert!(line.contains("will not let you dock") && line.contains("Abbess"), "{line}");
        assert_eq!(o.system, capital_of(&galaxy, AiShipType::TheSilence).map(|c| c.0));

        assert_eq!(speak_for(&c.summit, &mut standing), Some(choir));
        assert_ne!(standing.tier(choir), StandingTier::Hostile);
        assert!(standing.tier(choir) != StandingTier::Friendly, "she got you in the door, not a welcome");
        assert_eq!(speak_for(&c.summit, &mut standing), None, "spoken for twice");
        let o = objective(&c, &work, &galaxy, &standing, 10_000);
        assert!(!o.line.unwrap().contains("will not let you dock"));
    }

    /// If the Silence have lost everything before you reach them, the line
    /// still points somewhere, and she will ask for the pieces over a relay.
    /// While she has a house, she asks there.
    #[test]
    fn haven_relays_the_abbess() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let work = Jobs::default();
        let standing = FactionStanding::default();
        let mut c = Contacts::default();
        let haven = 0;
        assert!(contacts::comm_people(&galaxy, &c, haven, None).is_empty(), "the relay answers before anyone has called");
        c.summit.hailed = true;
        let on_relay = contacts::comm_people(&galaxy, &c, haven, None);
        assert_eq!(on_relay.len(), 1);
        assert_eq!(on_relay[0].id, ABBESS);
        assert!(!commission_here(true, &galaxy), "she asked over a relay with her own house standing");
        assert!(commission_here(false, &galaxy));

        for s in galaxy.systems.iter_mut().filter(|s| s.faction == Some(AiShipType::TheSilence)) {
            s.faction = None;
            s.capital = false;
        }
        let o = objective(&c, &work, &galaxy, &standing, 750);
        assert!(o.line.unwrap().contains("relay"), "the line went blank when the Silence fell");
        assert!(commission_here(true, &galaxy));
    }

    /// Paying for a step with its force job still running drops the job, and
    /// the way it was taken is remembered.
    #[test]
    fn paying_drops_the_force_job() {
        let mut app = story_app(SummitProgress { hailed: true, commissioned: true, ..default() });
        let raid = Job {
            id: 0,
            giver: step(1).unwrap().contact.clone(),
            faction: contact_of(1).unwrap().faction,
            kind: JobKind::Strike { system: 3, target: AiShipType::MerchantPrinces, kills: 0, needed: 2 },
            title: "t".into(),
            brief: "b".into(),
            reward: 400,
            status: JobStatus::Active,
            story: Some(StoryMark { step: 1, forced: true }),
        };
        let ordinary = Job { story: None, giver: "someone".into(), ..raid.clone() };
        app.world_mut().resource_mut::<Jobs>().push(raid);
        app.world_mut().resource_mut::<Jobs>().push(ordinary);
        win(&mut app, 1, false);
        let work = app.world().resource::<Jobs>();
        assert!(work.story().is_none(), "the raid on the courier is still running");
        assert_eq!(work.list.len(), 1, "ordinary work was dropped with it");
        let story = &app.world().resource::<Contacts>().story;
        assert_eq!(story.way(1), Some(false));
        win(&mut app, 2, true);
        assert_eq!(app.world().resource::<Contacts>().story.way(2), Some(true));
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
            // Enough for the minutes, so the plain tests see the plain line.
            .insert_resource(Currency { credits: 10_000 })
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

    /// The last step of the build doesn't say goodbye: the Abbess asks for
    /// the pieces to be carried to the site, once, and the line says so.
    #[test]
    fn the_last_piece_is_not_the_end() {
        let mut app = story_app(SummitProgress { hailed: true, commissioned: true, done: last_step() - 1, ended: false });
        win(&mut app, last_step(), false);
        let carry = &crate::narrative::ending::data().carry;
        let q = app.world().resource::<LogQueue>().pending.clone();
        assert_eq!(q.last().unwrap().0, carry.title);
        assert_eq!(q.iter().filter(|c| c.0 == carry.title).count(), 1);
        assert!(q.iter().all(|c| !c.1.contains("Thank you for playing")), "the goodbye came before the ending");
        let c = app.world().resource::<Contacts>();
        assert!(c.summit.ended && c.story.carry && !c.story.ending_seen);
        let o = app.world().resource::<SummitObjective>();
        assert!(o.line.as_ref().unwrap().contains("carry the record"), "{:?}", o.line);
        // Reported again, nothing more is said.
        win(&mut app, last_step(), false);
        assert_eq!(app.world().resource::<LogQueue>().pending.len(), q.len());
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

    /// The old ones say it once a run, to a ship that has the minutes, and a
    /// new run hears it again: who has said it is part of the run, not of the
    /// program that happens to still be open.
    #[test]
    fn a_new_run_hears_the_last_one_again() {
        use crate::celestial::resources::SystemStreamingManager;
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let princes = capital_of(&galaxy, AiShipType::MerchantPrinces).unwrap().0;
        let mut app = App::new();
        app.add_message::<crate::events::ShowNotification>()
            .insert_resource(galaxy)
            .insert_resource(SystemStreamingManager::default())
            .insert_resource(Contacts { summit: SummitProgress { hailed: true, commissioned: true, done: 1, ended: false }, ..default() })
            .add_systems(Update, old_ones_hail);
        let said = |app: &mut App| app.world_mut().resource_mut::<Messages<crate::events::ShowNotification>>().drain().count();
        let arrive = |app: &mut App, system: Option<u32>| {
            app.world_mut().resource_mut::<SystemStreamingManager>().loaded_system = system;
            app.update();
        };
        arrive(&mut app, Some(princes));
        assert_eq!(said(&mut app), 1, "the Princes didn't say it");
        assert_eq!(app.world().resource::<Contacts>().story.heard_last_one, vec![AiShipType::MerchantPrinces]);
        // Leaving and coming back: said is said.
        arrive(&mut app, None);
        arrive(&mut app, Some(princes));
        assert_eq!(said(&mut app), 0, "said twice in one run");
        // It survives a save...
        let saved = serde_json::to_string(app.world().resource::<Contacts>()).unwrap();
        let back: Contacts = serde_json::from_str(&saved).unwrap();
        assert_eq!(back.story.heard_last_one, vec![AiShipType::MerchantPrinces]);
        // ...and a new run starts with nobody having said anything.
        app.insert_resource(Contacts { summit: SummitProgress { hailed: true, commissioned: true, done: 1, ended: false }, ..default() });
        arrive(&mut app, None);
        arrive(&mut app, Some(princes));
        assert_eq!(said(&mut app), 1, "a new run didn't hear it");
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
        let mut work = Jobs::default();
        let rows = board(&p, &work, &galaxy);
        assert_eq!(rows.len(), STEPS_IN_ALL as usize);
        assert!(matches!(&rows[0], BoardRow::Found(1, _, gist) if gist == &step(1).unwrap().gist));
        assert!(matches!(&rows[1], BoardRow::Next(2, _, w) if w.contains(&contact_of(2).unwrap().name)));
        assert!(rows[2..].iter().all(|r| matches!(r, BoardRow::Unknown(_))));
        // With the step's work in hand, the row says the work, not the errand
        // of going to ask for it.
        work.push(Job {
            id: 0,
            giver: step(2).unwrap().contact.clone(),
            faction: contact_of(2).unwrap().faction,
            kind: JobKind::Survey { system: 3, seconds: 0.0 },
            title: "Salvage Rights: carry the salvage".into(),
            brief: String::new(),
            reward: 500,
            status: JobStatus::Active,
            story: Some(StoryMark { step: 2, forced: false }),
        });
        let rows = board(&p, &work, &galaxy);
        assert!(matches!(&rows[1], BoardRow::Next(2, _, w) if w.contains("carry the salvage")), "{:?}", rows[1]);
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
