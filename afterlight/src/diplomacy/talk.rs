//! How the world talks about the story.
//!
//! Contacts had three fixed lines each, their written pitch had nothing to
//! do with the job they then handed you, and nobody at a bar had ever heard
//! of the Summit Record. This module is the small amount of logic that
//! fixes that; the words are data (`data/talk.json`, and each contact's own
//! `prefers` and `reactive` in `data/contacts.json`).
//!
//! - **What a contact says** (`quote`): their own lines as before, except
//!   that when the work they have for you isn't the work their pitch is
//!   about, they say what the work is; and on every other talk, if
//!   something is true of the story or the dark that they have a line for,
//!   they say that. A contact whose faction held a piece remembers how you
//!   took it.
//! - **What a job's brief adds** (`aside`): one sentence, when the dark is
//!   close or the story has come far enough that people talk.
//! - **What the bar has heard** (`rumour`): one line a visit about what you
//!   did, or what the dark has done.

use std::collections::HashMap;
use std::sync::OnceLock;

use bevy::prelude::*;
use serde::Deserialize;

use super::contacts::{self, ContactDef, Contacts, Reactive};
use super::jobs::{JobKind, Jobs};
use crate::celestial::resources::GalaxyMap;
use crate::narrative::ending::{self, RunFacts};
use crate::narrative::summit;

#[derive(Deserialize)]
struct BriefDef {
    /// A job kind (`kind_name`), or "any".
    kind: String,
    when: String,
    line: String,
}

#[derive(Deserialize)]
struct TalkData {
    role_offer: HashMap<String, HashMap<String, String>>,
    role_reactive: HashMap<String, Vec<Reactive>>,
    briefs: Vec<BriefDef>,
    rumours: Vec<Reactive>,
}

fn data() -> &'static TalkData {
    static DATA: OnceLock<TalkData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../../data/talk.json")).expect("data/talk.json is malformed"))
}

/// A job's kind, as the data names it.
pub fn kind_name(kind: &JobKind) -> &'static str {
    match kind {
        JobKind::Bounty { .. } => "bounty",
        JobKind::Raid { .. } => "raid",
        JobKind::Escort { .. } => "escort",
        JobKind::HoldTheLine { .. } => "hold",
        JobKind::Delivery { .. } => "delivery",
        JobKind::Survey { .. } => "survey",
        JobKind::Strike { .. } => "strike",
        JobKind::Handoff { .. } => "handoff",
        JobKind::Theft { .. } => "theft",
    }
}

/// What is true of the story where you are standing: what the lines'
/// `when` is asked of.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub facts: RunFacts,
    /// The Abbess has asked for the pieces to be carried to the site, and
    /// it hasn't been done.
    pub carry: bool,
    /// The record has played.
    pub heard: bool,
    /// Jumps from here to the nearest system the dark has taken.
    pub dark_hops: Option<u32>,
}

impl Scene {
    pub fn at(contacts: &Contacts, galaxy: &GalaxyMap, system: Option<u32>) -> Scene {
        let steps = summit::last_step();
        let story = &contacts.story;
        Scene {
            facts: RunFacts {
                ways: (1..=steps).map(|n| story.way(n).unwrap_or(false)).collect(),
                pieces: contacts.summit.done.min(steps),
                kills: story.ship_kills,
                dark: crate::narrative::front::taken_count(galaxy) as u32,
                systems: galaxy.systems.len() as u32,
                reached: story.reached,
                ..default()
            },
            carry: story.carry && !story.ending_seen,
            heard: story.ending_seen,
            dark_hops: system.and_then(|id| crate::narrative::front::hops_from_dark(galaxy, id)),
        }
    }
}

/// Whether a `when` can be read at all (`holder` is whether the line
/// belongs to someone who could have been dealt with or robbed).
fn readable(when: &str) -> bool {
    let when = when.trim();
    matches!(when, "always" | "carry" | "heard" | "dealt" | "robbed")
        || when.strip_prefix("after:").is_some_and(|n| n.parse::<u8>().is_ok())
        || when.strip_prefix("dark_within:").is_some_and(|n| n.parse::<u32>().is_ok())
        || ending::parse(when).is_some()
}

/// Whether a line's `when` is true here. `step` is the piece the speaker's
/// faction held, if it held one (for "dealt" and "robbed").
fn holds(when: &str, scene: &Scene, step: Option<u8>) -> bool {
    let when = when.trim();
    let took = |forced: bool| step.is_some_and(|k| ending::holds(&if forced { ending::Cond::Forced(k) } else { ending::Cond::Deal(k) }, &scene.facts));
    match when {
        "always" => true,
        "carry" => scene.carry,
        "heard" => scene.heard,
        "dealt" => took(false),
        "robbed" => took(true),
        _ => {
            if let Some(n) = when.strip_prefix("after:").and_then(|n| n.parse::<u8>().ok()) {
                return scene.facts.pieces >= n;
            }
            if let Some(n) = when.strip_prefix("dark_within:").and_then(|n| n.parse::<u32>().ok()) {
                return scene.dark_hops.is_some_and(|h| h <= n);
            }
            ending::parse(when).is_some_and(|c| ending::holds(&c, &scene.facts))
        }
    }
}

/// The piece of the record a contact's faction held, if it held one this
/// build plays.
fn step_of(def: &ContactDef) -> Option<u8> {
    (1..=summit::last_step()).find(|n| summit::contact_of(*n).is_some_and(|c| c.faction == def.faction))
}

/// The last of `lines` that is true here: lines are written in the order
/// the story goes, so the latest thing is what's on people's minds.
fn last_true<'a>(lines: &'a [Reactive], scene: &Scene, step: Option<u8>) -> Option<&'a str> {
    lines.iter().rev().find(|r| holds(&r.when, scene, step)).map(|r| r.line.as_str())
}

/// What a contact has to say about the story or the dark: their own line
/// for it, or failing that their role's.
pub fn reaction(def: &ContactDef, scene: &Scene) -> Option<&'static str> {
    let step = step_of(def);
    let own: &'static [Reactive] = &def_static(def).reactive;
    last_true(own, scene, step).or_else(|| data().role_reactive.get(def.role.label()).and_then(|lines| last_true(lines, scene, None)))
}

/// The compiled-in contact behind a borrowed one (they all are: contacts::all).
fn def_static(def: &ContactDef) -> &'static ContactDef {
    contacts::all().iter().find(|c| c.id == def.id).expect("a contact that isn't in contacts.json")
}

/// What their role says about a kind of work, for when it isn't the work
/// their own pitch is about.
fn role_offer(def: &ContactDef, kind: &str) -> Option<&'static str> {
    let d = data();
    d.role_offer.get(def.role.label()).and_then(|k| k.get(kind)).or_else(|| d.role_offer.get("any").and_then(|k| k.get(kind))).map(String::as_str)
}

/// What a contact says when you look at them in the Comm directory.
pub fn quote(def: &ContactDef, contacts: &Contacts, work: &Jobs, scene: &Scene) -> String {
    let state = contacts.state(&def.id);
    let (rel, talks) = (state.map_or(0.0, |s| s.relationship), state.map_or(0, |s| s.talks));
    // Every other talk, once you've met: what's on their mind.
    if talks >= 2 && talks % 2 == 0 {
        if let Some(line) = reaction(def, scene) {
            return line.to_string();
        }
    }
    let base = contacts::line_for(def, rel, talks);
    // Their pitch is about one kind of work. If what they have for you is
    // another kind, say that instead: the words and the WORK box agree.
    if std::ptr::eq(base, def.offer.as_str()) {
        if let Some(job) = work.of(&def.id) {
            let kind = kind_name(&job.kind);
            if def.prefers.as_deref() != Some(kind) {
                if let Some(line) = role_offer(def, kind) {
                    return line.to_string();
                }
            }
        }
    }
    base.to_string()
}

/// One more sentence for an ordinary job's brief, when the dark is close
/// or the story has people talking. The last true line for this kind of
/// job, one written for the kind before one written for any.
pub fn aside(kind: &JobKind, scene: &Scene) -> Option<&'static str> {
    let name = kind_name(kind);
    let pick = |want: &str| data().briefs.iter().rev().find(|b| b.kind == want && holds(&b.when, scene, None)).map(|b| b.line.as_str());
    pick(name).or_else(|| pick("any"))
}

/// What the bar has heard about you, or about the dark: one line a visit.
pub fn rumour(scene: &Scene) -> Option<String> {
    last_true(&data().rumours, scene, None).map(|line| scene.facts.fill(line))
}

#[cfg(test)]
mod talk_tests {
    use super::*;
    use crate::ai_ship::components::AiShipType;
    use crate::diplomacy::contacts::ContactRole;
    use crate::diplomacy::jobs::{Job, JobStatus};

    fn scene(pieces: u8, ways: [bool; 4]) -> Scene {
        Scene { facts: RunFacts { ways: ways.to_vec(), pieces, systems: 31, ..default() }, ..default() }
    }

    fn offered(def: &ContactDef, kind: JobKind) -> Jobs {
        let mut work = Jobs::default();
        work.push(Job { id: 0, giver: def.id.clone(), faction: def.faction, kind, title: "t".into(), brief: "b".into(), reward: 1, status: JobStatus::Offered, story: None });
        work
    }

    /// Every `when` in the data can be read, every line is plain and short,
    /// and every kind of work a role can hand out has words for it.
    #[test]
    fn no_placeholder_reaches_the_player() {
        let d = data();
        let all_lines = d
            .role_reactive
            .values()
            .flatten()
            .chain(&d.rumours)
            .map(|r| (&r.when, &r.line))
            .chain(d.briefs.iter().map(|b| (&b.when, &b.line)))
            .chain(contacts::all().iter().flat_map(|c| c.reactive.iter().map(|r| (&r.when, &r.line))));
        let busy = Scene { facts: RunFacts { ways: vec![true; 4], pieces: 4, dark: 9, systems: 31, kills: 40, ..default() }, carry: true, heard: true, dark_hops: Some(1) };
        for (when, line) in all_lines {
            assert!(readable(when), "can't read 'when': {when}");
            assert!(line.is_ascii() && !line.is_empty(), "not plain ASCII: {line}");
            assert!(line.split_whitespace().count() <= 30, "too long for a line of talk: {line}");
            assert!(!busy.facts.fill(line).contains('{'), "unfilled: {line}");
        }
        for words in d.role_offer.values().flat_map(|k| k.values()) {
            assert!(words.is_ascii() && !words.is_empty());
        }
        for role in [ContactRole::Military, ContactRole::Trade, ContactRole::Underworld, ContactRole::Outpost, ContactRole::Hub] {
            for kind in crate::diplomacy::jobs::kinds_for(role) {
                assert!(d.role_offer.get(role.label()).is_some_and(|k| k.contains_key(*kind)), "{} has no words for {kind} work", role.label());
            }
            assert!(d.role_reactive.contains_key(role.label()), "{} has no reactions", role.label());
        }
        // A contact's preferred work is work their role gives.
        for c in contacts::all() {
            if let Some(p) = &c.prefers {
                assert!(crate::diplomacy::jobs::kinds_for(c.role).contains(&p.as_str()), "{} prefers {p}, which a {} contact never gives", c.name, c.role.label());
            }
        }
        assert!(rumour(&busy).is_some_and(|r| !r.contains('{')));
    }

    /// A contact's pitch is said when the work on offer is the work it is
    /// about; when it isn't, they say what the work is. And their own kind
    /// of work is what they offer more often than not.
    #[test]
    fn a_contact_offers_what_they_talk_about() {
        let vale = contacts::all().iter().find(|c| c.name == "Captain Idris Vale").unwrap();
        assert_eq!(vale.prefers.as_deref(), Some("bounty"), "the battle-group man doesn't prefer bounties");
        let mut people = Contacts::default();
        people.talk(vale);
        people.talk(vale); // past the greeting: the pitch
        people.state_mut(&vale.id).talks = 3;
        let quiet = scene(0, [false; 4]);
        let bounty = offered(vale, JobKind::Bounty { fleet: 1, system: 2 });
        assert_eq!(quote(vale, &people, &bounty, &quiet), vale.offer, "his own pitch, for his own kind of work");
        let hold = offered(vale, JobKind::HoldTheLine { system: 2, kills: 0, needed: 3, left: 60.0 });
        let said = quote(vale, &people, &hold, &quiet);
        assert_ne!(said, vale.offer, "he talked about a battle group and handed over a border");
        assert!(said.contains("slipping"), "{said}");
        // With nothing on offer, the pitch stands.
        assert_eq!(quote(vale, &people, &Jobs::default(), &quiet), vale.offer);

        // Where there is a fleet to hunt, the bounty is what he offers.
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let here = galaxy.systems.iter().find(|s| s.capital && s.faction == Some(AiShipType::TerranHegemony)).unwrap().id;
        let mut sim = crate::ai_ship::components::WorldSimulation::default();
        crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy, here);
        for n in crate::celestial::galaxy::nearest_neighbors(&galaxy, here, 3) {
            crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy, n);
        }
        let war = crate::celestial::territory::BorderWar::default();
        let world = crate::diplomacy::jobs::World { sim: &sim, galaxy: &galaxy, war: &war, here };
        let mut rng = rand::thread_rng();
        let kinds: Vec<&str> = (0..200).filter_map(|_| crate::diplomacy::jobs::offer_for(vale, 0.0, &world, &mut rng)).map(|j| kind_name(&j.kind)).collect();
        assert!(kinds.len() >= 100);
        // More often than a coin toss between his two kinds of work, and
        // not so often that he never offers the other.
        let bounties = kinds.iter().filter(|k| **k == "bounty").count();
        assert!(bounties * 100 >= kinds.len() * 65, "only {bounties} of {} offers were his kind of work", kinds.len());
    }

    /// Whoever held a piece remembers how you took it; everyone else has
    /// their role's view of the story and the dark; and it alternates with
    /// their ordinary lines instead of replacing them.
    #[test]
    fn contacts_react_to_what_you_did() {
        let quay = summit::contact_of(1).unwrap();
        let mut people = Contacts::default();
        people.state_mut(&quay.id).talks = 4;
        let none = Jobs::default();
        let before = quote(quay, &people, &none, &scene(0, [false; 4]));
        let dealt = quote(quay, &people, &none, &scene(1, [false; 4]));
        let robbed = quote(quay, &people, &none, &scene(1, [true, false, false, false]));
        assert_eq!(before, quay.offer, "he reacted to something that hasn't happened");
        assert!(dealt != before && robbed != before && dealt != robbed, "dealt: {dealt} / robbed: {robbed}");
        // On the talk between, his ordinary line.
        people.state_mut(&quay.id).talks = 5;
        assert_eq!(quote(quay, &people, &none, &scene(1, [false; 4])), quay.offer);

        // Someone with no lines of their own, with the dark two jumps off.
        let pell = contacts::all().iter().find(|c| c.role == ContactRole::Outpost && c.reactive.is_empty()).unwrap();
        people.state_mut(&pell.id).talks = 2;
        let near = Scene { dark_hops: Some(2), ..scene(0, [false; 4]) };
        assert!(quote(pell, &people, &none, &near).contains("lights went out"));
        let far = Scene { dark_hops: Some(6), ..scene(0, [false; 4]) };
        assert_eq!(quote(pell, &people, &none, &far), pell.offer);
        // Those who hold the pieces this build doesn't reach say so, once
        // you have all it does.
        let reyes = contacts::all().iter().find(|c| c.id.contains("reyes")).unwrap();
        people.state_mut(&reyes.id).talks = 2;
        assert_ne!(quote(reyes, &people, &none, &scene(4, [false; 4])), reyes.offer, "the Admiral has nothing to say to a ship with four pieces");
    }

    /// The bar hears nothing of the record before there is anything to
    /// hear, and after the first piece it hears how you got it.
    #[test]
    fn the_bar_hears_of_the_record_after_step_one() {
        assert_eq!(rumour(&scene(0, [false; 4])), None);
        let bought = rumour(&scene(1, [false; 4])).expect("nobody heard the minutes were bought");
        let taken = rumour(&scene(1, [true, false, false, false])).expect("nobody heard about the courier");
        assert!(bought.contains("paid in full") && taken.contains("courier"), "{bought} / {taken}");
        // Later news pushes out earlier, and the dark is counted.
        assert!(rumour(&scene(3, [false; 4])).unwrap().contains("Choir"));
        let dark = Scene { facts: RunFacts { dark: 7, systems: 31, ..default() }, ..default() };
        let said = rumour(&dark).unwrap();
        assert!(said.contains("7 systems") && said.contains("24 left"), "{said}");
        // A job near the dark says so; far from it, and early, it doesn't.
        let delivery = JobKind::Delivery { to_system: 3, item: crate::resources::ItemType::Crystal, count: 2 };
        assert_eq!(aside(&delivery, &scene(0, [false; 4])), None);
        let near = Scene { dark_hops: Some(1), ..scene(0, [false; 4]) };
        assert!(aside(&delivery, &near).unwrap().contains("before the dark"));
        assert!(aside(&JobKind::Theft { system: 1, victim: AiShipType::GildedThrone, signature: String::new(), taken: false, to_system: 2 }, &scene(2, [false; 4])).unwrap().contains("tear"));
    }
}
