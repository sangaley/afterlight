//! Contact jobs: work on the living galaxy.
//!
//! Talk to a contact in a starport's Comm directory and they offer you
//! work; every job points at something the simulation is already doing, so
//! finishing one moves the world:
//!
//! | Job | Who offers it | What you do |
//! | --- | --- | --- |
//! | Bounty | Military, outposts | Destroy a named enemy fleet nearby |
//! | Hold the line | Military | Destroy hostiles in their contested system before time runs out |
//! | Escort | Trade | See one of their convoys to its next stop, staying close |
//! | Delivery | Trade, outposts | Carry sealed cargo to another of their stations |
//! | Raid | Underworld | Destroy a rival's convoy haulers |
//! | Survey | Underworld, outposts, the Abbess | Spend a minute in a system and report |
//!
//! A job pays on the spot when it's done: credits, the contact's regard,
//! standing with their faction, and for fighting jobs a push on the border.
//! Failing costs a little regard. Three jobs at a time.

use bevy::prelude::*;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use super::contacts::{ContactDef, ContactRole, Contacts};
use super::FactionStanding;
use crate::ai_ship::components::{factions_hostile, faction_display_name, AiShipType, SimBehavior, WorldSimulation};
use crate::ai_ship::fleet::{role_of, FleetKind, Mission};
use crate::ai_ship::roster::ClassRole;
use crate::celestial::resources::{GalaxyMap, SystemStreamingManager};
use crate::celestial::territory::BorderWar;
use crate::resources::{Currency, ItemType};

/// Jobs you can hold at once.
pub const MAX_ACTIVE: usize = 3;
/// How long holding the line lasts.
const HOLD_SECONDS: f32 = 15.0 * 60.0;
/// How long a survey takes.
const SURVEY_SECONDS: f32 = 60.0;
/// How close you have to stay to a convoy you're escorting.
const ESCORT_RANGE: f32 = 15_000.0;
/// How near its stop a convoy counts as arrived.
const ARRIVED: f32 = 4_000.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum JobKind {
    Bounty { fleet: u32, system: u32 },
    Raid { fleet: u32, system: u32 },
    Escort { fleet: u32, system: u32, stop: Vec2, kept_close: bool },
    HoldTheLine { system: u32, kills: u32, needed: u32, left: f32 },
    Delivery { to_system: u32, item: ItemType, count: u32 },
    Survey { system: u32, seconds: f32 },
    /// Destroy `needed` of one faction's ships in a system, no clock. The
    /// story's way of saying "take it from them" when no convoy is handy.
    Strike { system: u32, target: AiShipType, kills: u32, needed: u32 },
    /// Spend a while in a system with cargo aboard, and someone there takes
    /// it off your hands.
    Handoff { system: u32, item: ItemType, count: u32, seconds: f32 },
    /// Steal one of `victim`'s signature modules (diplomacy::signatures) for
    /// the giver: destroy one of their ships in `system`, and the sealed
    /// crate comes aboard; then dock at the giver's station in `to_system`.
    Theft { system: u32, victim: AiShipType, signature: String, taken: bool, to_system: u32 },
}

/// A step of the Summit Record, and which way the player chose to take it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryMark {
    pub step: u8,
    pub forced: bool,
}

/// One step of the story was won (narrative::summit turns it into the next).
#[derive(Message, Clone, Copy, Debug)]
pub struct SummitStepWon(pub StoryMark);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Offered,
    Active,
    Done,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: u32,
    /// The contact who gave it.
    pub giver: String,
    pub faction: AiShipType,
    pub kind: JobKind,
    pub title: String,
    pub brief: String,
    pub reward: u32,
    pub status: JobStatus,
    /// Set on the Summit Record's own work. Story work doesn't count against
    /// the three-job limit and isn't offered or lapsed like ordinary work.
    #[serde(default)]
    pub story: Option<StoryMark>,
}

impl Job {
    /// The system the work happens in.
    pub fn system(&self) -> u32 {
        match self.kind {
            JobKind::Bounty { system, .. }
            | JobKind::Raid { system, .. }
            | JobKind::Escort { system, .. }
            | JobKind::HoldTheLine { system, .. }
            | JobKind::Survey { system, .. }
            | JobKind::Strike { system, .. }
            | JobKind::Handoff { system, .. } => system,
            JobKind::Delivery { to_system, .. } => to_system,
            JobKind::Theft { system, taken, to_system, .. } => if taken { to_system } else { system },
        }
    }

    /// One line on how it stands, for the tracker and the Comm directory.
    pub fn progress(&self) -> String {
        match &self.kind {
            JobKind::HoldTheLine { kills, needed, left, .. } => {
                format!("{kills}/{needed} destroyed, {}:{:02} left", (*left as u32) / 60, (*left as u32) % 60)
            }
            JobKind::Survey { seconds, .. } => format!("{:.0}/{:.0}s scanned", seconds, SURVEY_SECONDS),
            JobKind::Escort { kept_close, .. } => {
                if *kept_close { "with the convoy".into() } else { "join the convoy".into() }
            }
            JobKind::Delivery { item, count, .. } => format!("{} aboard", cargo_count(*item, *count)),
            JobKind::Bounty { .. } | JobKind::Raid { .. } => "hunting".into(),
            JobKind::Strike { kills, needed, .. } => format!("{kills}/{needed} destroyed"),
            JobKind::Handoff { seconds, .. } => format!("{:.0}/{:.0}s waiting", seconds, HANDOFF_SECONDS),
            JobKind::Theft { taken: false, victim, .. } => format!("take it off a {} ship", faction_display_name(*victim)),
            JobKind::Theft { taken: true, .. } => "crate aboard: dock to hand it over".into(),
        }
    }
}

/// How long you wait in a system for a handoff.
pub const HANDOFF_SECONDS: f32 = 45.0;

/// Every job offered to you, in hand, or just settled.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Jobs {
    pub list: Vec<Job>,
    #[serde(default)]
    pub next_id: u32,
}

impl Jobs {
    pub fn active(&self) -> impl Iterator<Item = &Job> {
        self.list.iter().filter(|j| j.status == JobStatus::Active)
    }

    /// Ordinary work in hand: what the three-job limit counts.
    pub fn held(&self) -> usize {
        self.active().filter(|j| j.story.is_none()).count()
    }

    /// The job a contact has open with you: offered or in hand. Story work is
    /// asked for separately (`story`).
    pub fn of(&self, giver: &str) -> Option<&Job> {
        self.list
            .iter()
            .find(|j| j.giver == giver && j.story.is_none() && matches!(j.status, JobStatus::Offered | JobStatus::Active))
    }

    /// The Summit Record work in hand, if any.
    pub fn story(&self) -> Option<&Job> {
        self.active().find(|j| j.story.is_some())
    }

    /// Take on a step of the story. Only one at a time: taking the other way
    /// drops the first. Cargo goes aboard through `load`.
    pub fn begin_story(&mut self, mut job: Job, load: impl FnOnce(ItemType, u32) -> bool) -> Result<(), &'static str> {
        if let JobKind::Delivery { item, count, .. } | JobKind::Handoff { item, count, .. } = job.kind {
            // Changing your mind about the same step keeps what's already aboard.
            let aboard = self.story().is_some_and(|j| {
                matches!(j.kind, JobKind::Delivery { item: i, .. } | JobKind::Handoff { item: i, .. } if i == item)
            });
            if !aboard && !load(item, count) {
                return Err("There's no room in your hold for the cargo.");
            }
        }
        self.list.retain(|j| j.story.is_none());
        job.status = JobStatus::Active;
        self.push(job);
        Ok(())
    }

    /// Take a contact's offered job, if you have room. A delivery's cargo
    /// goes aboard through `load`, which says whether the hold took it.
    pub fn accept(&mut self, giver: &str, load: impl FnOnce(ItemType, u32) -> bool) -> Result<(), &'static str> {
        if self.held() >= MAX_ACTIVE {
            return Err("You already have three jobs in hand.");
        }
        let Some(job) = self.list.iter_mut().find(|j| j.giver == giver && j.status == JobStatus::Offered) else {
            return Err("They have nothing for you.");
        };
        if let JobKind::Delivery { item, count, .. } = job.kind {
            if !load(item, count) {
                return Err("There's no room in your hold for the cargo.");
            }
        }
        job.status = JobStatus::Active;
        Ok(())
    }

    /// Offers you walked away from lapse; work in hand stays.
    pub fn drop_offers(&mut self) {
        self.list.retain(|j| j.status != JobStatus::Offered);
    }

    pub fn push(&mut self, mut job: Job) {
        self.next_id += 1;
        job.id = self.next_id;
        self.list.push(job);
    }
}

/// What a faction ships when it ships something sealed.
fn cargo_of(faction: AiShipType) -> ItemType {
    use AiShipType::*;
    match faction {
        TerranHegemony => ItemType::AmmoCrate,
        GildedThrone | CorpseStars => ItemType::RareAlloy,
        MerchantPrinces | BrokenChoir => ItemType::AncientArtifact,
        FreeHaulersCompact => ItemType::FuelCell,
        TheSilence => ItemType::Crystal,
        SynthesisCollective | StellarPreserve => ItemType::BioSample,
        _ => ItemType::ScrapMetal,
    }
}

/// "A Terran Hegemony patrol", "The Silence's patrol".
fn a_fleet_of(faction: AiShipType, what: &str) -> String {
    let name = faction_display_name(faction);
    if name.starts_with("The ") { format!("{name}'s {what}") } else { format!("A {name} {what}") }
}

/// "3 Ammo Crates", "2 crates of Rare Alloy".
fn cargo_count(item: ItemType, count: u32) -> String {
    match item {
        ItemType::ScrapMetal | ItemType::RareAlloy => format!("{count} crates of {}", item.name()),
        _ => format!("{count} {}s", item.name()),
    }
}

/// The kinds of work a contact gives, by their role.
fn kinds_for(role: ContactRole) -> &'static [&'static str] {
    match role {
        ContactRole::Military => &["bounty", "hold"],
        ContactRole::Trade => &["escort", "delivery"],
        ContactRole::Underworld => &["raid", "survey", "theft"],
        ContactRole::Outpost => &["bounty", "delivery", "survey"],
        ContactRole::Hub => &["survey"],
    }
}

/// The world as a job is written against.
pub struct World<'a> {
    pub sim: &'a WorldSimulation,
    pub galaxy: &'a GalaxyMap,
    pub war: &'a BorderWar,
    pub here: u32,
}

/// Write a job for a contact from what the galaxy is doing right now, or
/// None if there's nothing of their kind to be had.
pub fn offer_for(def: &ContactDef, relationship: f32, world: &World, rng: &mut impl Rng) -> Option<Job> {
    let name_of = |id: u32| world.galaxy.systems.iter().find(|s| s.id == id).map_or("somewhere".to_string(), |s| s.name.clone());
    let mut near = vec![world.here];
    near.extend(crate::celestial::galaxy::nearest_neighbors(world.galaxy, world.here, 3));
    let alive = |fleet: u32| world.sim.ships.iter().filter(|s| s.fleet == fleet && s.behavior != SimBehavior::Dead).collect::<Vec<_>>();
    let blocks = |fleet: u32| -> u32 {
        alive(fleet).iter().map(|s| crate::ai_ship::spawner::design_blocks(s.faction, s.class) as u32).sum()
    };
    // Better pay for people who trust you.
    let pay = |base: f32| ((base * (1.0 + relationship / 100.0)) / 10.0).round() as u32 * 10;
    let faction = def.faction;
    let job = |kind: JobKind, title: String, brief: String, reward: u32| Job {
        id: 0,
        giver: def.id.clone(),
        faction,
        kind,
        title,
        brief,
        reward,
        status: JobStatus::Offered,
        story: None,
    };

    let mut kinds: Vec<&str> = kinds_for(def.role).to_vec();
    kinds.shuffle(rng);
    for kind in kinds {
        match kind {
            "bounty" => {
                let targets: Vec<_> = world
                    .sim
                    .fleets
                    .iter()
                    .filter(|f| near.contains(&f.system_id) && f.mission == Mission::None)
                    .filter(|f| matches!(f.kind, FleetKind::Patrol | FleetKind::Raiders | FleetKind::BattleGroup))
                    .filter(|f| factions_hostile(faction, f.faction) && !alive(f.id).is_empty())
                    // The things that haunt the deep are the jackpot board's,
                    // not a contact's errand.
                    .filter(|f| !crate::celestial::territory::ROAMING.contains(&f.faction))
                    .collect();
                if let Some(f) = targets.choose(rng) {
                    let n = alive(f.id).len();
                    let what = match f.kind { FleetKind::BattleGroup => "battle group", FleetKind::Raiders => "raider band", _ => "patrol" };
                    return Some(job(
                        JobKind::Bounty { fleet: f.id, system: f.system_id },
                        format!("Bounty: {} {what}", faction_display_name(f.faction)),
                        format!("{} of {n} in {}. Destroy every hull.", a_fleet_of(f.faction, what), name_of(f.system_id)),
                        pay(300.0 + blocks(f.id) as f32 * 0.6),
                    ));
                }
            }
            "raid" => {
                let targets: Vec<_> = world
                    .sim
                    .fleets
                    .iter()
                    .filter(|f| near.contains(&f.system_id) && f.kind == FleetKind::Convoy)
                    .filter(|f| factions_hostile(faction, f.faction))
                    .filter(|f| alive(f.id).iter().any(|s| role_of(s.faction, s.class) == ClassRole::Civilian))
                    .collect();
                if let Some(f) = targets.choose(rng) {
                    let haulers = alive(f.id).iter().filter(|s| role_of(s.faction, s.class) == ClassRole::Civilian).count();
                    return Some(job(
                        JobKind::Raid { fleet: f.id, system: f.system_id },
                        format!("Raid: {} convoy", faction_display_name(f.faction)),
                        format!("{} of {haulers} haulers in {}. Make sure none of them arrive.", a_fleet_of(f.faction, "convoy"), name_of(f.system_id)),
                        pay(400.0 + haulers as f32 * 150.0),
                    ));
                }
            }
            "escort" => {
                let convoys: Vec<_> = world
                    .sim
                    .fleets
                    .iter()
                    .filter(|f| f.system_id == world.here && f.kind == FleetKind::Convoy && f.faction == faction && f.route.len() >= 2)
                    .filter(|f| alive(f.id).iter().any(|s| role_of(s.faction, s.class) == ClassRole::Civilian))
                    .collect();
                if let Some(f) = convoys.choose(rng) {
                    return Some(job(
                        JobKind::Escort { fleet: f.id, system: f.system_id, stop: f.destination, kept_close: false },
                        "Escort: our convoy".to_string(),
                        format!("One of our convoys is running through {}. Stay within 15 km of it until it makes its next stop.", name_of(f.system_id)),
                        pay(350.0 + alive(f.id).len() as f32 * 60.0),
                    ));
                }
            }
            "hold" => {
                let contested: Vec<(u32, f32)> = near
                    .iter()
                    .filter(|id| world.galaxy.systems.iter().any(|s| s.id == **id && s.faction == Some(faction)))
                    .filter_map(|id| world.war.contest(*id).map(|c| (*id, c.claimed())))
                    .filter(|(_, claimed)| *claimed >= 5.0)
                    .collect();
                if let Some(&(system, _)) = contested.choose(rng) {
                    let needed = rng.gen_range(3..=6);
                    return Some(job(
                        JobKind::HoldTheLine { system, kills: 0, needed, left: HOLD_SECONDS },
                        format!("Hold the line at {}", name_of(system)),
                        format!("{} is slipping. Destroy {needed} of the ships fighting us for it within fifteen minutes.", name_of(system)),
                        pay(500.0 + needed as f32 * 90.0),
                    ));
                }
            }
            "delivery" => {
                let others: Vec<u32> = world
                    .galaxy
                    .systems
                    .iter()
                    .filter(|s| s.faction == Some(faction) && s.id != world.here)
                    .map(|s| s.id)
                    .collect();
                if let Some(&to) = others.choose(rng) {
                    let item = cargo_of(faction);
                    let count = rng.gen_range(2..=4);
                    return Some(job(
                        JobKind::Delivery { to_system: to, item, count },
                        format!("Delivery to {}", name_of(to)),
                        format!("Carry {}, sealed, to our station at {}. Don't sell them.", cargo_count(item, count), name_of(to)),
                        pay(250.0 + count as f32 * 60.0),
                    ));
                }
            }
            "theft" => {
                // An enemy of theirs with a military market worth robbing,
                // and the nearest system it holds.
                let here_pos = world.galaxy.systems.iter().find(|s| s.id == world.here).map_or(Vec2::ZERO, |s| s.galaxy_pos);
                let targets: Vec<_> = world
                    .galaxy
                    .systems
                    .iter()
                    .filter_map(|s| s.faction.map(|f| (s, f)))
                    .filter(|(s, f)| s.id != world.here && factions_hostile(faction, *f) && super::signatures::of(*f).next().is_some())
                    .collect();
                let nearest = targets.iter().min_by(|a, b| a.0.galaxy_pos.distance(here_pos).total_cmp(&b.0.galaxy_pos.distance(here_pos)));
                if let Some(&(sys, victim)) = nearest {
                    let wares: Vec<_> = super::signatures::of(victim).collect();
                    if let Some(sig) = wares.choose(rng) {
                        return Some(job(
                            JobKind::Theft { system: sys.id, victim, signature: sig.id.to_string(), taken: false, to_system: world.here },
                            format!("Theft: a {} {}", faction_display_name(victim), sig.name),
                            format!(
                                "We want to know how {} builds a {}. Take one off one of their ships in {} and bring it here, sealed.",
                                faction_display_name(victim),
                                sig.name,
                                sys.name
                            ),
                            pay(900.0 + sig.price as f32 * 0.3),
                        ));
                    }
                }
            }
            "survey" => {
                let elsewhere: Vec<u32> = near.iter().copied().filter(|id| *id != world.here).collect();
                if let Some(&system) = elsewhere.choose(rng) {
                    return Some(job(
                        JobKind::Survey { system, seconds: 0.0 },
                        format!("Survey {}", name_of(system)),
                        format!("Take a minute in {} and tell us what's moving there.", name_of(system)),
                        pay(300.0),
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

/// What a step of the Summit Record asks of you, one way or the other.
#[derive(Clone, Debug)]
pub enum StoryOffer {
    /// Pay this much, here, and it's done.
    Pay(u32),
    Work(Job),
}

/// Credits the Princes want for the summit minutes.
pub const MINUTES_PRICE: u32 = 2500;

/// The work for step `step` of the Summit Record, taken the deal way or by
/// force, written against what the galaxy is doing right now. Every step
/// prefers the fitting job on a real fleet and falls back to one that can
/// always be done, so the story never waits on a convoy that isn't there.
pub fn story_offer(step: u8, forced: bool, def: &ContactDef, world: &World, rng: &mut impl Rng) -> StoryOffer {
    use AiShipType::*;
    let name_of = |id: u32| world.galaxy.systems.iter().find(|s| s.id == id).map_or("somewhere".to_string(), |s| s.name.clone());
    let mut near = vec![world.here];
    near.extend(crate::celestial::galaxy::nearest_neighbors(world.galaxy, world.here, 3));
    let alive = |fleet: u32| world.sim.ships.iter().filter(|s| s.fleet == fleet && s.behavior != SimBehavior::Dead).collect::<Vec<_>>();
    let haulers = |fleet: u32| alive(fleet).iter().filter(|s| role_of(s.faction, s.class) == ClassRole::Civilian).count();
    let here_pos = world.galaxy.systems.iter().find(|s| s.id == world.here).map_or(Vec2::ZERO, |s| s.galaxy_pos);
    // A faction's systems, nearest first, and not this one if it has others.
    let systems_of = |f: AiShipType| -> Vec<u32> {
        let mut v: Vec<_> = world.galaxy.systems.iter().filter(|s| s.faction == Some(f)).collect();
        v.sort_by(|a, b| a.galaxy_pos.distance(here_pos).total_cmp(&b.galaxy_pos.distance(here_pos)));
        let ids: Vec<u32> = v.iter().map(|s| s.id).collect();
        let away: Vec<u32> = ids.iter().copied().filter(|id| *id != world.here).collect();
        if away.is_empty() { ids } else { away }
    };
    let convoy_of = |f: AiShipType, rng: &mut _| {
        let convoys: Vec<_> = world
            .sim
            .fleets
            .iter()
            .filter(|c| near.contains(&c.system_id) && c.kind == FleetKind::Convoy && c.faction == f && haulers(c.id) > 0)
            .collect();
        convoys.choose(rng).map(|c| (c.id, c.system_id, c.destination, c.route.len()))
    };
    let mark = StoryMark { step, forced };
    let job = |kind: JobKind, title: String, brief: String, reward: u32| {
        StoryOffer::Work(Job { id: 0, giver: def.id.clone(), faction: def.faction, kind, title, brief, reward, status: JobStatus::Active, story: Some(mark) })
    };
    // Take it from them: their convoy if one is near, else their ships.
    let take_from = |victim: AiShipType, needed: u32, what: &str, rng: &mut _| {
        if let Some((fleet, system, _, _)) = convoy_of(victim, rng) {
            return job(
                JobKind::Raid { fleet, system },
                format!("{what}: raid the convoy"),
                format!("{} is running through {}. Stop every hauler in it and take what it carries.", a_fleet_of(victim, "convoy"), name_of(system)),
                400,
            );
        }
        let system = systems_of(victim).first().copied().unwrap_or(world.here);
        job(
            JobKind::Strike { system, target: victim, kills: 0, needed },
            format!("{what}: by force"),
            format!("Destroy {needed} {} ship{} in {} and take it from the wreckage.", faction_display_name(victim), if needed == 1 { "" } else { "s" }, name_of(system)),
            400,
        )
    };
    let listen = |rng: &mut _| {
        let elsewhere: Vec<u32> = near.iter().copied().filter(|id| *id != world.here).collect();
        let system = elsewhere.choose(rng).copied().unwrap_or(world.here);
        (system, name_of(system))
    };

    match (step, forced) {
        (1, false) => StoryOffer::Pay(MINUTES_PRICE),
        (1, true) => take_from(MerchantPrinces, 2, "The Quiet Ones", rng),
        (2, false) => {
            if let Some((fleet, system, stop, _)) = convoy_of(FreeHaulersCompact, rng).filter(|c| c.3 >= 2) {
                return job(
                    JobKind::Escort { fleet, system, stop, kept_close: false },
                    "Salvage Rights: see a convoy home".into(),
                    format!("A Compact convoy is running through {}. Stay within 15 km of it until it makes its next stop.", name_of(system)),
                    500,
                );
            }
            let to = systems_of(FreeHaulersCompact).first().copied().unwrap_or(world.here);
            job(
                JobKind::Delivery { to_system: to, item: ItemType::ScrapMetal, count: 3 },
                "Salvage Rights: carry the salvage".into(),
                format!("Carry {}, sealed, to the Compact's station at {}.", cargo_count(ItemType::ScrapMetal, 3), name_of(to)),
                500,
            )
        }
        (2, true) => take_from(FreeHaulersCompact, 2, "Salvage Rights", rng),
        (3, false) => {
            let contested: Vec<u32> = near
                .iter()
                .copied()
                .filter(|id| world.galaxy.systems.iter().any(|s| s.id == *id && s.faction == Some(BrokenChoir)))
                .filter(|id| world.war.contest(*id).is_some())
                .collect();
            if let Some(&system) = contested.choose(rng) {
                return job(
                    JobKind::HoldTheLine { system, kills: 0, needed: 4, left: HOLD_SECONDS },
                    format!("Isolated Frequency: hold {}", name_of(system)),
                    format!("{} is slipping from the Choir. Destroy 4 of the ships fighting them for it within fifteen minutes.", name_of(system)),
                    600,
                );
            }
            // No border slipping today: a fleet that's after them, then.
            let enemies: Vec<_> = world
                .sim
                .fleets
                .iter()
                .filter(|f| near.contains(&f.system_id) && f.mission == Mission::None)
                .filter(|f| matches!(f.kind, FleetKind::Patrol | FleetKind::Raiders | FleetKind::BattleGroup))
                .filter(|f| factions_hostile(BrokenChoir, f.faction) && !alive(f.id).is_empty())
                .filter(|f| !crate::celestial::territory::ROAMING.contains(&f.faction))
                .collect();
            if let Some(f) = enemies.choose(rng) {
                return job(
                    JobKind::Bounty { fleet: f.id, system: f.system_id },
                    format!("Isolated Frequency: clear {}", name_of(f.system_id)),
                    format!("{} is pressing on the Choir in {}. Destroy every hull.", a_fleet_of(f.faction, "fleet"), name_of(f.system_id)),
                    600,
                );
            }
            // Nobody at all: sit on the border and listen instead.
            let (system, name) = listen(rng);
            job(
                JobKind::Survey { system, seconds: 0.0 },
                format!("Isolated Frequency: listen at {name}"),
                format!("Sit a minute in {name} with your receivers open, so the Choir can tune you."),
                400,
            )
        }
        (3, true) => take_from(BrokenChoir, 1, "Isolated Frequency", rng),
        (4, false) => {
            let mut homes: Vec<_> = world.galaxy.systems.iter().filter(|s| s.faction == Some(StellarPreserve) && s.id != world.here).collect();
            homes.sort_by(|a, b| b.galaxy_pos.distance(here_pos).total_cmp(&a.galaxy_pos.distance(here_pos)));
            if let Some(to) = homes.first() {
                return job(
                    JobKind::Delivery { to_system: to.id, item: ItemType::BioSample, count: 1 },
                    "What It Eats: carry the ark".into(),
                    format!("Carry the ark to the Preserve's station at {}. Don't linger near other ships.", to.name),
                    600,
                );
            }
            let (system, name) = listen(rng);
            job(
                JobKind::Survey { system, seconds: 0.0 },
                "What It Eats: keep it alone".into(),
                format!("Take the ark to {name} and keep it there, alone, for a minute."),
                400,
            )
        }
        (4, true) => {
            // The Collective come to you for it.
            let system = systems_of(SynthesisCollective).first().copied().unwrap_or(world.here);
            job(
                JobKind::Handoff { system, item: ItemType::BioSample, count: 1, seconds: 0.0 },
                "What It Eats: sell the ark".into(),
                format!("Take the ark into {} and wait. The Collective will come for it.", name_of(system)),
                1500,
            )
        }
        // Steps past the demo aren't written yet.
        _ => {
            let (system, name) = listen(rng);
            job(JobKind::Survey { system, seconds: 0.0 }, format!("Survey {name}"), format!("Take a minute in {name}."), 300)
        }
    }
}

/// What finishing (or failing) moves.
const DONE_REGARD: f32 = 8.0;
const DONE_STANDING: f32 = 3.0;
const FAILED_REGARD: f32 = -5.0;
/// Border control a won fight buys the giver.
const DONE_PUSH: f32 = 6.0;

/// Keep every job in hand up to date, and settle the ones that are done or
/// lost.
#[allow(clippy::too_many_arguments)]
pub fn track_jobs(
    time: Res<Time>,
    mut jobs: ResMut<Jobs>,
    world: (Res<WorldSimulation>, ResMut<GalaxyMap>, ResMut<BorderWar>, Res<SystemStreamingManager>),
    mut contacts: ResMut<Contacts>,
    mut standing: ResMut<FactionStanding>,
    mut currency: ResMut<Currency>,
    player: Query<(Entity, &Transform), With<crate::components::Ship>>,
    mut deaths: MessageReader<crate::events::AiShipDestroyed>,
    victims: Query<&crate::ai_ship::components::AiShipState>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut story: MessageWriter<SummitStepWon>,
) {
    let (sim, mut galaxy, mut war, streaming) = world;
    let dt = time.delta_secs();
    let here = streaming.loaded_system;
    let you = player.single().ok();
    // Kills you made this frame, by faction, in the system you're in.
    let kills: Vec<AiShipType> = deaths
        .read()
        .filter(|d| victims.get(d.entity).is_ok_and(|s| you.is_some_and(|(p, _)| s.last_attacker == Some(p))))
        .map(|d| d.ship_type)
        .collect();
    let alive = |fleet: u32| sim.ships.iter().filter(|s| s.fleet == fleet && s.behavior != SimBehavior::Dead).collect::<Vec<_>>();

    for job in jobs.list.iter_mut().filter(|j| j.status == JobStatus::Active) {
        let outcome: Option<bool> = match &mut job.kind {
            // The target has to go while you're there: a fleet someone else
            // finished off-screen isn't your kill.
            JobKind::Bounty { fleet, system } => {
                if sim.fleet(*fleet).is_none() {
                    Some(false)
                } else if alive(*fleet).is_empty() {
                    Some(here == Some(*system))
                } else {
                    None
                }
            }
            JobKind::Raid { fleet, system } => {
                let haulers = alive(*fleet).iter().filter(|s| role_of(s.faction, s.class) == ClassRole::Civilian).count();
                if sim.fleet(*fleet).is_none() {
                    Some(false)
                } else if haulers == 0 {
                    Some(here == Some(*system))
                } else {
                    None
                }
            }
            JobKind::Escort { fleet, stop, kept_close, .. } => {
                let m = alive(*fleet);
                let haulers = m.iter().filter(|s| role_of(s.faction, s.class) == ClassRole::Civilian).count();
                if let Some((_, tf)) = you {
                    if m.iter().any(|s| s.position.distance(tf.translation.truncate()) < ESCORT_RANGE) {
                        *kept_close = true;
                    }
                }
                if haulers == 0 || sim.fleet(*fleet).is_none() {
                    Some(false)
                } else if *kept_close && m.iter().any(|s| s.position.distance(*stop) < ARRIVED) {
                    Some(true)
                } else {
                    None
                }
            }
            JobKind::HoldTheLine { system, kills: k, needed, left } => {
                if here == Some(*system) {
                    *k += kills.iter().filter(|v| factions_hostile(job.faction, **v)).count() as u32;
                }
                *left -= dt;
                if *k >= *needed { Some(true) } else if *left <= 0.0 { Some(false) } else { None }
            }
            JobKind::Survey { system, seconds } => {
                if here == Some(*system) {
                    *seconds += dt;
                }
                (*seconds >= SURVEY_SECONDS).then_some(true)
            }
            JobKind::Strike { system, target, kills: k, needed } => {
                if here == Some(*system) {
                    *k += kills.iter().filter(|v| **v == *target).count() as u32;
                }
                (*k >= *needed).then_some(true)
            }
            JobKind::Handoff { system, seconds, .. } => {
                if here == Some(*system) {
                    *seconds += dt;
                }
                (*seconds >= HANDOFF_SECONDS).then_some(true)
            }
            // Taken in a fight here; handed over at the dock (deliver_cargo).
            JobKind::Theft { system, victim, taken, signature, .. } => {
                if !*taken && here == Some(*system) && kills.contains(victim) {
                    *taken = true;
                    let name = super::signatures::get(signature).map_or("module", |s| s.name);
                    notes.write(crate::events::ShowNotification {
                        message: format!("A sealed crate is aboard: their {name}. Take it back to be paid."),
                        notification_type: crate::events::NotificationType::Success,
                        duration: 5.0,
                    });
                }
                None
            }
            // Settled at the dock (deliver_cargo).
            JobKind::Delivery { .. } => None,
        };
        let Some(won) = outcome else { continue };
        if let Some(mark) = settle(job, won, &mut contacts, &mut standing, &mut currency, &mut galaxy, &mut war, &mut notes) {
            story.write(SummitStepWon(mark));
        }
    }
    // Settled jobs leave the list; a contact offers new work next time.
    jobs.list.retain(|j| matches!(j.status, JobStatus::Offered | JobStatus::Active));
}

/// Pay out, or not, and say so. A won step of the story is handed back, for
/// narrative::summit to carry on from.
#[allow(clippy::too_many_arguments)]
fn settle(
    job: &mut Job,
    won: bool,
    contacts: &mut Contacts,
    standing: &mut FactionStanding,
    currency: &mut Currency,
    galaxy: &mut GalaxyMap,
    war: &mut BorderWar,
    notes: &mut MessageWriter<crate::events::ShowNotification>,
) -> Option<StoryMark> {
    let who = super::contacts::all().iter().find(|c| c.id == job.giver).map_or("Your contact".to_string(), |c| c.name.clone());
    info!("JOB {} for {who}: {} ({} cr)", if won { "done" } else { "failed" }, job.title, job.reward);
    if won && job.story.is_some_and(|m| m.forced) {
        // Taken from them, not done for them: the pay is what you took, and
        // what it costs you is the story's to say (summit::advance).
        job.status = JobStatus::Done;
        currency.credits += job.reward;
        notes.write(crate::events::ShowNotification {
            message: format!("{} done: +{} credits", job.title, job.reward),
            notification_type: crate::events::NotificationType::Success,
            duration: 5.0,
        });
        return job.story;
    }
    if won {
        job.status = JobStatus::Done;
        currency.credits += job.reward;
        let s = contacts.state_mut(&job.giver);
        s.relationship = (s.relationship + DONE_REGARD).min(100.0);
        standing.change(job.faction, DONE_STANDING, format!("You did a job for {who}"));
        let fought = match job.kind {
            JobKind::Bounty { system, .. } | JobKind::Raid { system, .. } | JobKind::HoldTheLine { system, .. } => Some(system),
            _ => None,
        };
        if let Some(system) = fought {
            war.push(galaxy, system, job.faction, DONE_PUSH);
        }
        notes.write(crate::events::ShowNotification {
            message: format!("{} done for {who}: +{} credits", job.title, job.reward),
            notification_type: crate::events::NotificationType::Success,
            duration: 5.0,
        });
    } else {
        job.status = JobStatus::Failed;
        let s = contacts.state_mut(&job.giver);
        s.relationship = (s.relationship + FAILED_REGARD).max(0.0);
        notes.write(crate::events::ShowNotification {
            message: format!("{} failed. {who} won't forget it.", job.title),
            notification_type: crate::events::NotificationType::Warning,
            duration: 5.0,
        });
        return None;
    }
    job.story
}

/// Docked at one of the giver's stations, in the system the cargo was
/// meant for: hand it over.
#[allow(clippy::too_many_arguments)]
pub fn deliver_cargo(
    mut jobs: ResMut<Jobs>,
    market: Res<crate::resources::MarketEvents>,
    streaming: Res<SystemStreamingManager>,
    mut inventory: ResMut<crate::resources::Inventory>,
    mut contacts: ResMut<Contacts>,
    mut standing: ResMut<FactionStanding>,
    mut currency: ResMut<Currency>,
    world: (ResMut<GalaxyMap>, ResMut<BorderWar>),
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut story: MessageWriter<SummitStepWon>,
) {
    let (mut galaxy, mut war) = world;
    let Some(faction) = market.dock_terms.as_ref().and_then(|t| t.faction) else { return };
    let Some(here) = streaming.loaded_system else { return };
    for job in jobs.list.iter_mut().filter(|j| j.status == JobStatus::Active && j.faction == faction) {
        if let JobKind::Theft { taken: true, to_system, victim, .. } = job.kind {
            if to_system == here {
                standing.change(victim, -6.0, "Their work was stolen and sold");
                if let Some(mark) = settle(job, true, &mut contacts, &mut standing, &mut currency, &mut galaxy, &mut war, &mut notes) {
                    story.write(SummitStepWon(mark));
                }
            }
            continue;
        }
        let JobKind::Delivery { to_system, item, count } = job.kind else { continue };
        if to_system != here {
            continue;
        }
        if inventory.items.get(&item).copied().unwrap_or(0) < count {
            notes.write(crate::events::ShowNotification {
                message: format!("{}: the cargo isn't aboard any more.", job.title),
                notification_type: crate::events::NotificationType::Warning,
                duration: 4.0,
            });
            settle(job, false, &mut contacts, &mut standing, &mut currency, &mut galaxy, &mut war, &mut notes);
            continue;
        }
        inventory.remove_item(item, count);
        if let Some(mark) = settle(job, true, &mut contacts, &mut standing, &mut currency, &mut galaxy, &mut war, &mut notes) {
            story.write(SummitStepWon(mark));
        }
    }
    jobs.list.retain(|j| matches!(j.status, JobStatus::Offered | JobStatus::Active));
}

/// Finishing a job on the old contract board counts with the real faction
/// behind it: the Navy's work is the Hegemony's, the Salvage Guild's the
/// Compact's, the Research Institute's the Preserve's.
pub fn standing_from_contracts(
    mut done: MessageReader<crate::events::ContractTurnedIn>,
    mut standing: ResMut<FactionStanding>,
) {
    for c in done.read() {
        standing.change(c.faction.real(), 2.0, format!("You finished a contract for them ({})", c.faction.name()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_at_hegemony_capital() -> (WorldSimulation, GalaxyMap, u32) {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let here = galaxy.systems.iter().find(|s| s.capital && s.faction == Some(AiShipType::TerranHegemony)).unwrap().id;
        let mut sim = WorldSimulation::default();
        crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy, here);
        for n in crate::celestial::galaxy::nearest_neighbors(&galaxy, here, 3) {
            crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy, n);
        }
        (sim, galaxy, here)
    }

    #[test]
    fn every_role_gets_work_and_it_points_at_something_real() {
        let (sim, galaxy, here) = world_at_hegemony_capital();
        let war = BorderWar::default();
        let world = World { sim: &sim, galaxy: &galaxy, war: &war, here };
        let mut rng = rand::thread_rng();
        let hegemony: Vec<_> = super::super::contacts::all().iter().filter(|c| c.faction == AiShipType::TerranHegemony).collect();
        for def in hegemony {
            let job = (0..10).find_map(|_| offer_for(def, 0.0, &world, &mut rng));
            let job = job.unwrap_or_else(|| panic!("{} had nothing to offer", def.name));
            assert!(job.reward > 0);
            match job.kind {
                JobKind::Bounty { fleet, .. } | JobKind::Raid { fleet, .. } | JobKind::Escort { fleet, .. } => {
                    let f = sim.fleet(fleet).expect("the job's fleet exists");
                    if !matches!(job.kind, JobKind::Escort { .. }) {
                        assert!(factions_hostile(AiShipType::TerranHegemony, f.faction), "a job against a friend");
                    }
                }
                JobKind::Delivery { to_system, .. } => assert_ne!(to_system, here),
                _ => {}
            }
        }
    }

    #[test]
    fn three_jobs_at_a_time() {
        let mut jobs = Jobs::default();
        for i in 0..4 {
            jobs.push(Job {
                id: 0, giver: format!("c{i}"), faction: AiShipType::TerranHegemony,
                kind: JobKind::Survey { system: 1, seconds: 0.0 },
                title: "t".into(), brief: "b".into(), reward: 10, status: JobStatus::Offered, story: None,
            });
        }
        for i in 0..3 {
            assert!(jobs.accept(&format!("c{i}"), |_, _| true).is_ok());
        }
        assert!(jobs.accept("c3", |_, _| true).is_err());
        jobs.drop_offers();
        assert_eq!(jobs.list.len(), 3, "the unaccepted offer lapses");
    }

    fn app_with(jobs: Vec<Job>, here: u32) -> App {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::events::AiShipDestroyed>()
            .add_message::<crate::events::ShowNotification>()
            .add_message::<SummitStepWon>()
            .init_resource::<WorldSimulation>()
            .insert_resource(galaxy)
            .init_resource::<BorderWar>()
            .insert_resource(SystemStreamingManager { loaded_system: Some(here), ..default() })
            .init_resource::<Contacts>()
            .init_resource::<FactionStanding>()
            .init_resource::<Currency>()
            .init_resource::<crate::resources::Inventory>()
            .insert_resource(Jobs { list: jobs, next_id: 1 });
        app
    }

    fn active(kind: JobKind) -> Job {
        Job {
            id: 1, giver: "TerranHegemony_captain_idris_vale".into(), faction: AiShipType::TerranHegemony, kind,
            title: "t".into(), brief: "b".into(), reward: 100, status: JobStatus::Active, story: None,
        }
    }

    #[test]
    fn a_survey_counts_only_time_spent_there() {
        let mut app = app_with(vec![active(JobKind::Survey { system: 5, seconds: SURVEY_SECONDS - 0.5 })], 4);
        app.add_systems(Update, track_jobs);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Jobs>().active().count(), 1, "surveying from the wrong system");
        app.world_mut().resource_mut::<SystemStreamingManager>().loaded_system = Some(5);
        let JobKind::Survey { seconds, .. } = &mut app.world_mut().resource_mut::<Jobs>().list[0].kind else { unreachable!() };
        *seconds = SURVEY_SECONDS;
        app.update();
        assert_eq!(app.world().resource::<Currency>().credits, Currency::default().credits + 100);
    }

    #[test]
    fn holding_the_line_runs_out_of_time() {
        let mut app = app_with(vec![active(JobKind::HoldTheLine { system: 5, kills: 1, needed: 4, left: 0.0 })], 5);
        app.add_systems(Update, track_jobs);
        app.update();
        let regard = app.world().resource::<Contacts>().relationship("TerranHegemony_captain_idris_vale");
        assert!(app.world().resource::<Jobs>().list.is_empty());
        assert_eq!(regard, 0.0, "regard can't go below nothing");
        assert_eq!(app.world().resource::<Currency>().credits, Currency::default().credits);
    }

    #[test]
    fn cargo_is_handed_over_at_the_right_station() {
        let job = active(JobKind::Delivery { to_system: 7, item: ItemType::AmmoCrate, count: 3 });
        let mut app = app_with(vec![job], 7);
        app.world_mut().resource_mut::<crate::resources::Inventory>().add_item(ItemType::AmmoCrate, 3);
        let mut market = crate::resources::MarketEvents::default();
        market.dock_terms = Some(crate::resources::DockTerms {
            faction: Some(AiShipType::TerranHegemony), name: "x".into(), standing: "Neutral", buy: 1.0, sell: 1.0,
        });
        app.insert_resource(market).add_systems(Update, deliver_cargo);
        app.update();
        assert!(app.world().resource::<Jobs>().list.is_empty());
        assert!(!app.world().resource::<crate::resources::Inventory>().items.contains_key(&ItemType::AmmoCrate));
        assert_eq!(app.world().resource::<Currency>().credits, Currency::default().credits + 100);
    }

    #[test]
    fn a_bounty_pays_when_the_fleet_is_gone() {
        let (mut sim, galaxy, here) = world_at_hegemony_capital();
        let war = BorderWar::default();
        let world = World { sim: &sim, galaxy: &galaxy, war: &war, here };
        let mut rng = rand::thread_rng();
        let captain = super::super::contacts::all().iter().find(|c| c.name == "Captain Idris Vale").unwrap();
        let job = (0..40).find_map(|_| offer_for(captain, 0.0, &world, &mut rng).filter(|j| matches!(j.kind, JobKind::Bounty { .. })));
        let mut job = job.expect("the Hegemony capital has enemies in reach");
        let JobKind::Bounty { fleet, system } = job.kind else { unreachable!() };
        job.status = JobStatus::Active;
        for s in sim.ships.iter_mut().filter(|s| s.fleet == fleet) {
            s.behavior = SimBehavior::Dead;
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::events::AiShipDestroyed>()
            .add_message::<crate::events::ShowNotification>()
            .add_message::<SummitStepWon>()
            .insert_resource(sim)
            .insert_resource(galaxy)
            .init_resource::<BorderWar>()
            .insert_resource(SystemStreamingManager { loaded_system: Some(system), ..default() })
            .init_resource::<Contacts>()
            .init_resource::<FactionStanding>()
            .init_resource::<Currency>()
            .insert_resource(Jobs { list: vec![job.clone()], next_id: 1 })
            .add_systems(Update, track_jobs);
        let before = app.world().resource::<Currency>().credits;
        app.update();
        assert_eq!(app.world().resource::<Currency>().credits, before + job.reward);
        assert!(app.world().resource::<Jobs>().list.is_empty(), "a settled job leaves the list");
        assert!(app.world().resource::<Contacts>().relationship(&captain.id) > 0.0);
    }

    /// Every step of the story, both ways, at the contact's own capital in a
    /// populated galaxy: the deal for step 1 is a price, everything else is
    /// work marked as that step, in a system that exists, against the right
    /// people.
    #[test]
    fn every_story_step_offers_both_ways_and_points_somewhere_real() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let mut rng = rand::thread_rng();
        for step in 1..=crate::narrative::summit::last_step() {
            let def = crate::narrative::summit::contact_of(step).unwrap();
            let here = crate::celestial::territory::capital_of(&galaxy, def.faction).unwrap();
            let mut sim = WorldSimulation::default();
            crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy, here);
            for n in crate::celestial::galaxy::nearest_neighbors(&galaxy, here, 3) {
                crate::ai_ship::simulation::ensure_system_population(&mut sim, &galaxy, n);
            }
            let war = BorderWar::default();
            let world = World { sim: &sim, galaxy: &galaxy, war: &war, here };
            for forced in [false, true] {
                match story_offer(step, forced, def, &world, &mut rng) {
                    StoryOffer::Pay(price) => assert!(step == 1 && !forced && price > 0, "step {step} asked for money"),
                    StoryOffer::Work(job) => {
                        assert_eq!(job.story, Some(StoryMark { step, forced }));
                        assert_eq!(job.status, JobStatus::Active);
                        assert!(galaxy.systems.iter().any(|s| s.id == job.system()), "step {step}: no such system");
                        match job.kind {
                            JobKind::Strike { system, target, .. } => {
                                let owner = galaxy.systems.iter().find(|s| s.id == system).unwrap().faction;
                                assert_eq!(owner, Some(target), "step {step}: striking {target:?} where they don't live");
                            }
                            JobKind::Delivery { to_system, .. } => assert_ne!(to_system, here),
                            JobKind::Handoff { system, .. } => {
                                let owner = galaxy.systems.iter().find(|s| s.id == system).unwrap().faction;
                                assert_eq!(owner, Some(AiShipType::SynthesisCollective));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    /// A theft: the crate comes aboard when one of the victim's ships dies
    /// in the right system, and it pays at the giver's dock, at the cost of
    /// the victim's standing.
    #[test]
    fn a_theft_is_taken_in_a_fight_and_paid_at_the_dock() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let here = crate::celestial::territory::capital_of(&galaxy, AiShipType::RecursiveKingdom).unwrap();
        let world_sim = WorldSimulation::default();
        let war = BorderWar::default();
        let world = World { sim: &world_sim, galaxy: &galaxy, war: &war, here };
        let underworld = super::super::contacts::all()
            .iter()
            .find(|c| c.faction == AiShipType::RecursiveKingdom && c.role == ContactRole::Underworld)
            .unwrap();
        let mut rng = rand::thread_rng();
        let job = (0..60)
            .find_map(|_| offer_for(underworld, 0.0, &world, &mut rng).filter(|j| matches!(j.kind, JobKind::Theft { .. })))
            .expect("an underworld contact never offered a theft");
        let JobKind::Theft { victim, ref signature, to_system, .. } = job.kind else { unreachable!() };
        assert!(factions_hostile(AiShipType::RecursiveKingdom, victim));
        assert_eq!(super::super::signatures::get(signature).unwrap().faction, victim);
        assert_eq!(to_system, here);

        let mut job = job.clone();
        job.status = JobStatus::Active;
        let JobKind::Theft { taken, .. } = &mut job.kind else { unreachable!() };
        *taken = true;
        let mut app = app_with(vec![job.clone()], here);
        let mut market = crate::resources::MarketEvents::default();
        market.dock_terms = Some(crate::resources::DockTerms {
            faction: Some(AiShipType::RecursiveKingdom), name: "x".into(), standing: "Neutral", buy: 1.0, sell: 1.0,
        });
        let before = app.world().resource::<FactionStanding>().value(victim);
        app.insert_resource(market).add_systems(Update, deliver_cargo);
        app.update();
        assert!(app.world().resource::<Jobs>().list.is_empty(), "the theft wasn't handed over");
        assert_eq!(app.world().resource::<Currency>().credits, Currency::default().credits + job.reward);
        assert!(app.world().resource::<FactionStanding>().value(victim) < before);
    }

    /// A strike counts only the named faction's ships, in its system.
    #[test]
    fn a_strike_counts_its_targets_only() {
        let mut job = active(JobKind::Strike { system: 5, target: AiShipType::BrokenChoir, kills: 0, needed: 1 });
        job.story = Some(StoryMark { step: 3, forced: true });
        let mut app = app_with(vec![job], 5);
        app.add_systems(Update, track_jobs);
        app.update();
        assert_eq!(app.world().resource::<Jobs>().active().count(), 1, "won with no kills");
        let JobKind::Strike { kills, .. } = &mut app.world_mut().resource_mut::<Jobs>().list[0].kind else { unreachable!() };
        *kills = 1;
        app.update();
        assert!(app.world().resource::<Jobs>().list.is_empty());
        let won = app.world().resource::<Messages<SummitStepWon>>();
        assert_eq!(won.len(), 1, "a won story job didn't report its step");
        // Force earns no regard from the people you took it from.
        assert_eq!(app.world().resource::<Contacts>().relationship("TerranHegemony_captain_idris_vale"), 0.0);
    }
}
