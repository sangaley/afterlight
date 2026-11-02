//! Who holds which systems.
//!
//! Systems used to be dealt to factions by distance from Haven, about three
//! each, with nothing tying a faction's systems together: no capital, no
//! borders, nowhere that was "their space". Now each faction that holds
//! ground has a capital and a cluster of systems grown out from it, and
//! where two clusters meet is a border.
//!
//! - **Territorial factions** (the eight combat factions that hold ground,
//!   plus the two rival trading factions) each get a capital and grow a
//!   cluster round it. Capitals are dealt weakest-nearest, so the walk out
//!   from Haven still gets harder the further you go.
//! - **The bosses** (the Eternal Hegemony and the Shepherd) hold nothing.
//!   They haunt the deepest systems instead: a haunted system keeps its
//!   owner, but the boss is what makes it dangerous, and its danger is the
//!   boss's.
//! - **Borders move** (see MOVING BORDERS below): a war tick a minute of
//!   play, a system changing hands every hour or two somewhere.
//! - **Danger** is per system: its owner's power, or its haunt's. The story
//!   follows the danger of the system you're in (narrative::logs).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::resources::{GalaxyMap, StarSystemDef};
use crate::ai_ship::components::{faction_power, factions_hostile, AiShipType};

/// The factions that hold ground, weakest first: the order capitals are
/// dealt outward from Haven.
pub const TERRITORIAL: [AiShipType; 10] = [
    AiShipType::TheSilence,
    AiShipType::MerchantPrinces,
    AiShipType::FreeHaulersCompact,
    AiShipType::RecursiveKingdom,
    AiShipType::BrokenChoir,
    AiShipType::StellarPreserve,
    AiShipType::SynthesisCollective,
    AiShipType::GildedThrone,
    AiShipType::CorpseStars,
    AiShipType::TerranHegemony,
];

/// The bosses, which hold nothing and haunt the deep systems.
pub const ROAMING: [AiShipType; 2] = [AiShipType::EternalHegemony, AiShipType::Shepherd];

/// How many of the deepest systems each boss haunts (Shepherd the deepest).
const HAUNTS: [(AiShipType, usize); 2] = [(AiShipType::Shepherd, 1), (AiShipType::EternalHegemony, 2)];

/// How many systems count as a system's neighbours, for borders.
pub const NEIGHBOURS: usize = 3;

/// A system's danger: its haunting boss's power, or its owner's.
pub fn danger_of(def: &StarSystemDef) -> f32 {
    def.haunt.or(def.faction).map_or(0.0, faction_power)
}

/// Deal the generated systems out. `systems[0]` is Haven and stays nobody's;
/// the rest are sorted nearest-first from Haven.
pub fn assign_territory(systems: &mut [StarSystemDef]) {
    let rest: Vec<usize> = (1..systems.len()).collect();
    if rest.is_empty() {
        return;
    }
    let n = rest.len();
    let per = n.div_ceil(TERRITORIAL.len());

    // Capitals, weakest nearest: faction i takes a system from its own band
    // of distance ranks, the one farthest from the capitals already placed,
    // so capitals spread across the map instead of lining up.
    let mut owner: Vec<Option<usize>> = vec![None; systems.len()];
    let mut capitals: Vec<usize> = Vec::new();
    for (f, _) in TERRITORIAL.iter().enumerate() {
        let lo = (f * n) / TERRITORIAL.len();
        let hi = (((f + 1) * n) / TERRITORIAL.len()).max(lo + 1).min(n);
        let pick = (lo..hi)
            .map(|rank| rest[rank])
            .filter(|&i| owner[i].is_none())
            .max_by(|&a, &b| {
                let spread = |i: usize| {
                    capitals
                        .iter()
                        .map(|&c| systems[c].galaxy_pos.distance(systems[i].galaxy_pos))
                        .fold(f32::MAX, f32::min)
                };
                spread(a).total_cmp(&spread(b))
            });
        let Some(capital) = pick.or_else(|| rest.iter().copied().find(|&i| owner[i].is_none())) else { break };
        owner[capital] = Some(f);
        capitals.push(capital);
    }

    // Grow each cluster out from its capital: the closest unclaimed system
    // to any faction still under its share goes to that faction, again and
    // again, so every cluster is a contiguous patch of the map.
    let mut held: Vec<usize> = vec![1; TERRITORIAL.len()];
    loop {
        let mut best: Option<(f32, usize, usize)> = None;
        for &s in &rest {
            if owner[s].is_some() {
                continue;
            }
            for f in 0..TERRITORIAL.len() {
                if held[f] >= per {
                    continue;
                }
                let d = rest
                    .iter()
                    .filter(|&&o| owner[o] == Some(f))
                    .map(|&o| systems[o].galaxy_pos.distance(systems[s].galaxy_pos))
                    .fold(f32::MAX, f32::min);
                if best.map_or(true, |(bd, ..)| d < bd) {
                    best = Some((d, s, f));
                }
            }
        }
        let Some((_, s, f)) = best else { break };
        owner[s] = Some(f);
        held[f] += 1;
    }

    // Haunts: the deepest systems, by distance rank from Haven.
    let mut deepest = rest.iter().rev().copied();
    let mut haunt: Vec<Option<AiShipType>> = vec![None; systems.len()];
    for (boss, count) in HAUNTS {
        for _ in 0..count {
            if let Some(i) = deepest.next() {
                haunt[i] = Some(boss);
            }
        }
    }

    for &i in &rest {
        let def = &mut systems[i];
        def.faction = owner[i].map(|f| TERRITORIAL[f]);
        def.capital = capitals.contains(&i);
        def.haunt = haunt[i];
        def.danger_tier = danger_of(def);
    }
}

/// The factions holding the systems next to this one, other than its own
/// owner: who it borders. Empty for a system deep in its owner's space.
pub fn border_factions(map: &GalaxyMap, system_id: u32) -> Vec<AiShipType> {
    let Some(def) = map.systems.iter().find(|s| s.id == system_id) else { return Vec::new() };
    let mut out: Vec<AiShipType> = Vec::new();
    for id in super::galaxy::nearest_neighbors(map, system_id, NEIGHBOURS) {
        let Some(other) = map.systems.iter().find(|s| s.id == id).and_then(|s| s.faction) else { continue };
        if Some(other) != def.faction && !out.contains(&other) {
            out.push(other);
        }
    }
    out
}

/// The system a faction rules from, if it holds one.
pub fn capital_of(map: &GalaxyMap, faction: AiShipType) -> Option<u32> {
    map.systems.iter().find(|s| s.capital && s.faction == Some(faction)).map(|s| s.id)
}

// ===========================================================================
// MOVING BORDERS
// ===========================================================================
//
// Every border system has a control score per faction contesting it: its
// owner's and each hostile neighbour's, summing to 100. Once a minute of
// play the off-screen war moves them, weighed by each side's power (a
// trading faction fights with half its ally's strength too; a capital
// defends at two and a half times its owner's; an empire is stretched over
// what it holds), with some luck either way. When a claimant's share passes
// the owner's, the system changes hands. Evenly matched neighbours rarely
// move each other at all; a strong faction against a weak one takes a system
// in an hour or two of play; somewhere a system changes hands every hour or
// two. What the player does in a system pushes its scores: kills, a
// destroyed station, trade.

/// A border system's shares, by faction. The owner's is whatever the
/// claimants don't hold.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Contest {
    pub system_id: u32,
    pub claims: Vec<(AiShipType, f32)>,
}

impl Contest {
    pub fn claimed(&self) -> f32 {
        self.claims.iter().map(|(_, c)| c).sum()
    }

    pub fn share(&self, faction: AiShipType) -> f32 {
        self.claims.iter().find(|(f, _)| *f == faction).map_or(0.0, |(_, c)| *c)
    }

    fn share_mut(&mut self, faction: AiShipType) -> &mut f32 {
        if let Some(i) = self.claims.iter().position(|(f, _)| *f == faction) {
            return &mut self.claims[i].1;
        }
        self.claims.push((faction, 0.0));
        &mut self.claims.last_mut().unwrap().1
    }

    /// Keep the claims within 0..=100 in total, scaling them down together.
    fn normalise(&mut self) {
        for (_, c) in &mut self.claims {
            *c = c.clamp(0.0, 100.0);
        }
        let total = self.claimed();
        if total > 100.0 {
            for (_, c) in &mut self.claims {
                *c *= 100.0 / total;
            }
        }
        self.claims.retain(|(_, c)| *c > 0.01);
    }
}

/// The off-screen war over the borders, and what it has done lately.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct BorderWar {
    pub contests: Vec<Contest>,
    /// Systems that changed hands, newest last: (system, from, to).
    pub news: Vec<(u32, Option<AiShipType>, AiShipType)>,
    /// Seconds of play towards the next war tick.
    #[serde(skip)]
    pub clock: f32,
}

/// A system changed hands.
#[derive(Message, Clone, Copy, Debug)]
pub struct SystemChangedHands {
    pub system_id: u32,
    pub from: Option<AiShipType>,
    pub to: AiShipType,
}

/// Seconds of play between war ticks.
pub const WAR_TICK: f32 = 60.0;
/// Control a side gains per tick at full advantage (all the power on its
/// side). Terran against the Silence moves about 0.43 a tick: a system in
/// roughly two hours of play. Even sides move only by luck.
const WAR_RATE: f32 = 0.45;
/// Luck either way per tick, at most.
const WAR_LUCK: f32 = 0.3;
/// A capital defends at this much of its owner's power: a faction's last
/// stand is hard to break, so factions rarely die out.
const CAPITAL_GUARD: f32 = 2.5;
/// What the old owner keeps of a system it just lost: enough to fight back.
const LOSER_KEEPS: f32 = 20.0;
const NEWS_LEN: usize = 12;

/// What a faction fights a border war with: its own power, and half a
/// trading faction's ally's (an ally helps; it doesn't commit its navy).
pub fn war_power(faction: AiShipType) -> f32 {
    faction_power(faction) + faction.ally().map_or(0.0, |a| faction_power(a) * ALLY_SHARE)
}

/// How much of its strength an ally puts into its client's border wars.
const ALLY_SHARE: f32 = 0.5;

/// The share of the galaxy a faction starts with: three systems.
const HOME_SHARE: f32 = 3.0;

/// War power stretched over what a faction holds: an empire of six fights
/// each border at about 70%, a faction down to its last system at 170%.
/// Without it the strongest faction snowballed, every conquest a new border
/// to push, until it held half the map.
pub fn stretched_power(map: &GalaxyMap, faction: AiShipType) -> f32 {
    let held = map.systems.iter().filter(|s| s.faction == Some(faction)).count().max(1) as f32;
    war_power(faction) * (HOME_SHARE / held).sqrt()
}

impl BorderWar {
    pub fn contest(&self, system_id: u32) -> Option<&Contest> {
        self.contests.iter().find(|c| c.system_id == system_id)
    }

    fn contest_mut(&mut self, system_id: u32) -> &mut Contest {
        if let Some(i) = self.contests.iter().position(|c| c.system_id == system_id) {
            return &mut self.contests[i];
        }
        self.contests.push(Contest { system_id, claims: Vec::new() });
        self.contests.last_mut().unwrap()
    }

    /// One tick of the war, everywhere. Returns the systems that changed
    /// hands (already applied to `map`).
    pub fn tick(&mut self, map: &mut GalaxyMap, rng: &mut impl rand::Rng) -> Vec<SystemChangedHands> {
        let ids: Vec<u32> = map.systems.iter().filter(|s| s.faction.is_some()).map(|s| s.id).collect();
        for id in ids {
            let Some(owner) = map.systems.iter().find(|s| s.id == id).and_then(|s| s.faction) else { continue };
            let capital = map.systems.iter().any(|s| s.id == id && s.capital);
            // A faction's last system doesn't fall to the off-screen war: its
            // claimants' hold fades instead. With systems spread out every
            // holding has more hostile neighbours, and small factions died in
            // over half of all galaxies inside ten hours of play. You can still
            // finish one yourself (`push`).
            let last_stand = map.systems.iter().filter(|s| s.faction == Some(owner)).count() <= 1;
            let rivals: Vec<AiShipType> = border_factions(map, id)
                .into_iter()
                .filter(|f| !last_stand && TERRITORIAL.contains(f) && factions_hostile(owner, *f))
                .collect();
            let defend = stretched_power(map, owner) * if capital { CAPITAL_GUARD } else { 1.0 };
            {
                let contest = self.contest_mut(id);
                // A claimant no longer next door loses its hold slowly.
                for (f, c) in &mut contest.claims {
                    if !rivals.contains(f) {
                        *c -= WAR_RATE;
                    }
                }
            }
            let attacks: Vec<(AiShipType, f32)> = rivals.iter().map(|&r| (r, stretched_power(map, r))).collect();
            let contest = self.contest_mut(id);
            for (rival, attack) in attacks {
                let edge = (attack - defend) / (attack + defend).max(0.01);
                let luck = rng.gen_range(-WAR_LUCK..=WAR_LUCK);
                *contest.share_mut(rival) += WAR_RATE * edge + luck;
            }
            contest.normalise();
        }
        self.settle(map)
    }

    /// Push a faction's hold on a system: positive strengthens it, negative
    /// weakens it. The owner's share is what the claimants don't hold, so
    /// pushing the owner moves the claimants the other way (the strongest
    /// hostile neighbour takes what the owner loses).
    pub fn push(&mut self, map: &mut GalaxyMap, system_id: u32, faction: AiShipType, amount: f32) -> Vec<SystemChangedHands> {
        let Some(owner) = map.systems.iter().find(|s| s.id == system_id).and_then(|s| s.faction) else { return Vec::new() };
        if faction == owner {
            let contest = self.contest_mut(system_id);
            if amount >= 0.0 {
                // Strengthening the owner thins every claim evenly.
                let total = contest.claimed().max(0.01);
                for (_, c) in &mut contest.claims {
                    *c -= amount * (*c / total);
                }
            } else {
                let rivals: Vec<AiShipType> = border_factions(map, system_id)
                    .into_iter()
                    .filter(|f| TERRITORIAL.contains(f) && factions_hostile(owner, *f))
                    .collect();
                let contest = self.contest_mut(system_id);
                let Some(&taker) = rivals
                    .iter()
                    .max_by(|a, b| war_power(**a).total_cmp(&war_power(**b)))
                else {
                    return Vec::new();
                };
                *contest.share_mut(taker) -= amount;
            }
            self.contest_mut(system_id).normalise();
        } else {
            let contest = self.contest_mut(system_id);
            if contest.share(faction) > 0.0 || amount > 0.0 {
                *contest.share_mut(faction) += amount;
                contest.normalise();
            }
        }
        self.settle(map)
    }

    /// Hand over every system a claimant now holds more of than its owner.
    fn settle(&mut self, map: &mut GalaxyMap) -> Vec<SystemChangedHands> {
        let mut flips = Vec::new();
        for contest in &mut self.contests {
            let owner_share = 100.0 - contest.claimed();
            let Some(&(winner, share)) = contest.claims.iter().max_by(|a, b| a.1.total_cmp(&b.1)) else { continue };
            if share <= owner_share {
                continue;
            }
            let Some(sys) = map.systems.iter().find(|s| s.id == contest.system_id) else { continue };
            let from = sys.faction;
            contest.claims.retain(|(f, _)| *f != winner);
            contest.claims.iter_mut().for_each(|(_, c)| *c *= 0.5);
            if let Some(loser) = from {
                *contest.share_mut(loser) += LOSER_KEEPS;
            }
            contest.normalise();
            flips.push(SystemChangedHands { system_id: contest.system_id, from, to: winner });
        }
        for flip in &flips {
            hand_over(map, flip.system_id, flip.to);
            self.news.push((flip.system_id, flip.from, flip.to));
        }
        if self.news.len() > NEWS_LEN {
            let drop = self.news.len() - NEWS_LEN;
            self.news.drain(..drop);
        }
        flips
    }
}

/// Give a system to `to`. A capital that falls moves to its old owner's
/// nearest remaining system (none left: the faction is landless until it
/// takes one back).
pub fn hand_over(map: &mut GalaxyMap, system_id: u32, to: AiShipType) {
    let Some(i) = map.systems.iter().position(|s| s.id == system_id) else { return };
    let from = map.systems[i].faction;
    let was_capital = map.systems[i].capital;
    let pos = map.systems[i].galaxy_pos;
    {
        let sys = &mut map.systems[i];
        sys.faction = Some(to);
        sys.capital = false;
        sys.danger_tier = danger_of(sys);
    }
    if was_capital {
        if let Some(loser) = from {
            let next = map
                .systems
                .iter()
                .enumerate()
                .filter(|(_, s)| s.faction == Some(loser))
                .min_by(|(_, a), (_, b)| a.galaxy_pos.distance(pos).total_cmp(&b.galaxy_pos.distance(pos)))
                .map(|(j, _)| j);
            if let Some(j) = next {
                map.systems[j].capital = true;
            }
        }
    }
    // A faction that had no capital left (it lost its last system before)
    // and takes one back rules from it.
    if capital_of(map, to).is_none() {
        map.systems[i].capital = true;
    }
}

/// Runs the border war and lets the player push it.
pub struct TerritoryPlugin;

impl Plugin for TerritoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BorderWar>()
            .add_message::<SystemChangedHands>()
            .add_systems(
                Update,
                (border_war_system, player_pushes_borders)
                    .run_if(in_state(crate::states::GameState::Exploring)),
            );
    }
}

/// DEPTHS_WAR_SPEED=<n>: run the border war n times faster (test switch).
fn war_speed() -> f32 {
    std::env::var("DEPTHS_WAR_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0)
}

/// Say a system changed hands, and pass it on.
fn announce(
    flips: Vec<SystemChangedHands>,
    map: &GalaxyMap,
    out: &mut MessageWriter<SystemChangedHands>,
    notes: &mut MessageWriter<crate::events::ShowNotification>,
) {
    use crate::ai_ship::components::faction_display_name;
    for flip in flips {
        let Some(sys) = map.systems.iter().find(|s| s.id == flip.system_id) else { continue };
        info!("BORDER {} ({}) {:?} -> {:?}", sys.name, sys.id, flip.from, flip.to);
        // Only news you'd hear: a system on your charts.
        if sys.discovery != super::resources::SystemDiscovery::Unknown {
            notes.write(crate::events::ShowNotification {
                message: format!("{} has fallen to the {}", sys.name, faction_display_name(flip.to)),
                notification_type: crate::events::NotificationType::Warning,
                duration: 5.0,
            });
        }
        out.write(flip);
    }
}

/// The off-screen war: a tick a minute of play.
fn border_war_system(
    time: Res<Time>,
    mut war: ResMut<BorderWar>,
    mut map: ResMut<GalaxyMap>,
    mut out: MessageWriter<SystemChangedHands>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
    mut speed: Local<Option<f32>>,
) {
    let speed = *speed.get_or_insert_with(war_speed);
    war.clock += time.delta_secs() * speed;
    let mut rng = rand::thread_rng();
    while war.clock >= WAR_TICK {
        war.clock -= WAR_TICK;
        let flips = war.tick(&mut map, &mut rng);
        announce(flips, &map, &mut out, &mut notes);
    }
}

/// Control a kill moves, per point of standing it costs (a capital ship is
/// 25 points of standing, so 5 of control).
const KILL_PUSH: f32 = 0.2;
/// Control a destroyed station moves.
const STATION_PUSH: f32 = 25.0;

/// What you destroy weakens its faction's hold on the system you're in.
fn player_pushes_borders(
    mut deaths: MessageReader<crate::events::AiShipDestroyed>,
    victims: Query<(
        &crate::ai_ship::components::AiShipState,
        Option<&crate::ai_ship::components::AiShipClass>,
        Has<crate::diplomacy::stations::StationCore>,
        Has<crate::diplomacy::stations::StationPlatform>,
    )>,
    player: Query<Entity, With<crate::components::Ship>>,
    streaming: Res<super::resources::SystemStreamingManager>,
    mut war: ResMut<BorderWar>,
    mut map: ResMut<GalaxyMap>,
    mut out: MessageWriter<SystemChangedHands>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    let (Ok(player), Some(system)) = (player.single(), streaming.loaded_system) else {
        deaths.clear();
        return;
    };
    for death in deaths.read() {
        let Ok((state, class, core, platform)) = victims.get(death.entity) else { continue };
        if state.last_attacker != Some(player) {
            continue;
        }
        let victim = death.ship_type;
        let amount = if core {
            STATION_PUSH
        } else {
            let role = class.and_then(|c| crate::ai_ship::roster::find(victim, c.0)).map(|c| c.role);
            crate::diplomacy::kill_penalty(role, false, platform) * KILL_PUSH
        };
        let flips = war.push(&mut map, system, victim, -amount);
        announce(flips, &map, &mut out, &mut notes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::celestial::galaxy::generate_galaxy_map;

    #[test]
    fn every_territorial_faction_holds_a_capital_and_its_share() {
        for seed in [1, 7, 42, 99, 1234] {
            let map = generate_galaxy_map(seed);
            for f in TERRITORIAL {
                let held = map.systems.iter().filter(|s| s.faction == Some(f)).count();
                assert!((2..=4).contains(&held), "seed {seed}: {f:?} holds {held}");
                assert!(capital_of(&map, f).is_some(), "seed {seed}: {f:?} has no capital");
            }
            for f in ROAMING {
                assert!(map.systems.iter().all(|s| s.faction != Some(f)), "seed {seed}: boss {f:?} holds ground");
                assert!(map.systems.iter().any(|s| s.haunt == Some(f)), "seed {seed}: {f:?} haunts nothing");
            }
            assert!(map.systems[0].faction.is_none() && map.systems[0].haunt.is_none(), "Haven is nobody's");
        }
    }

    #[test]
    fn danger_still_grows_outward() {
        // Mean danger of the nearest third is below the farthest third.
        let map = generate_galaxy_map(42);
        let mut rest: Vec<_> = map.systems[1..].iter().collect();
        rest.sort_by(|a, b| a.galaxy_pos.length().total_cmp(&b.galaxy_pos.length()));
        let third = rest.len() / 3;
        let mean = |s: &[&StarSystemDef]| s.iter().map(|d| d.danger_tier).sum::<f32>() / s.len() as f32;
        assert!(mean(&rest[..third]) < mean(&rest[rest.len() - third..]));
        // The deepest system is the Shepherd's.
        assert_eq!(rest.last().unwrap().haunt, Some(AiShipType::Shepherd));
    }

    #[test]
    fn some_systems_are_borders() {
        let map = generate_galaxy_map(42);
        let borders = map.systems[1..].iter().filter(|s| !border_factions(&map, s.id).is_empty()).count();
        assert!(borders >= 5, "only {borders} border systems");
        assert!(borders < map.systems.len() - 1, "every system is a border");
    }

    use crate::celestial::resources::SystemDiscovery;
    use rand::SeedableRng;

    /// A row of systems a million units apart, owned as given; the first
    /// in each faction's run is its capital.
    fn row(owners: &[AiShipType]) -> GalaxyMap {
        let mut systems = vec![crate::celestial::galaxy::haven_def()];
        systems[0].galaxy_pos = Vec2::new(-50_000_000.0, 0.0);
        for (i, &f) in owners.iter().enumerate() {
            let id = i as u32 + 1;
            let mut def = crate::celestial::galaxy::haven_def();
            def.id = id;
            def.name = format!("S{id}");
            def.galaxy_pos = Vec2::new(i as f32 * 1_000_000.0, 0.0);
            def.faction = Some(f);
            def.capital = i == 0 || owners[i - 1] != f;
            def.discovery = SystemDiscovery::Located;
            def.danger_tier = danger_of(&def);
            systems.push(def);
        }
        GalaxyMap { systems, galaxy_seed: 1 }
    }

    #[test]
    fn a_strong_faction_takes_a_weak_neighbour_in_hours() {
        use AiShipType::*;
        let mut map = row(&[TerranHegemony, TerranHegemony, TheSilence, TheSilence, TheSilence]);
        let mut war = BorderWar::default();
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let mut first = None;
        for minute in 0..240 {
            if !war.tick(&mut map, &mut rng).is_empty() && first.is_none() {
                first = Some(minute);
            }
        }
        let minute = first.expect("four hours and the Hegemony took nothing");
        assert!((60..=180).contains(&minute), "the first system fell after {minute} minutes");
        assert!(map.systems.iter().filter(|s| s.faction == Some(TerranHegemony)).count() >= 3);
        assert!(!war.news.is_empty());
    }

    #[test]
    fn even_neighbours_hold_for_hours() {
        use AiShipType::*;
        // The Collective (1.5) against the Preserve (1.2): close to even.
        for seed in 0..5 {
            let mut map = row(&[SynthesisCollective, SynthesisCollective, StellarPreserve, StellarPreserve]);
            let mut war = BorderWar::default();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            for _ in 0..120 {
                assert!(war.tick(&mut map, &mut rng).is_empty(), "seed {seed}: a border moved in under two hours");
            }
        }
    }

    #[test]
    fn a_fallen_capital_moves() {
        use AiShipType::*;
        let mut map = row(&[TheSilence, TheSilence, TerranHegemony]);
        assert!(map.systems[1].capital);
        hand_over(&mut map, 1, TerranHegemony);
        assert_eq!(map.systems[1].faction, Some(TerranHegemony));
        assert!(!map.systems[1].capital);
        assert!(map.systems[2].capital, "the Silence's capital moved to its next system");
        assert_eq!(capital_of(&map, TheSilence), Some(2));
    }

    #[test]
    fn your_kills_push_a_border() {
        use AiShipType::*;
        let mut map = row(&[TheSilence, TheSilence, TerranHegemony]);
        let mut war = BorderWar::default();
        // System 2 borders the Hegemony; killing the Silence there helps it.
        war.push(&mut map, 2, TheSilence, -30.0);
        assert!((war.contest(2).unwrap().share(TerranHegemony) - 30.0).abs() < 1e-3);
        // Enough of it and the system falls.
        let flips = war.push(&mut map, 2, TheSilence, -30.0);
        assert_eq!(flips.len(), 1);
        assert_eq!(map.systems[2].faction, Some(TerranHegemony));
        // The loser keeps a foothold to fight back from.
        assert!(war.contest(2).unwrap().share(TheSilence) >= LOSER_KEEPS - 1e-3);
    }

    #[test]
    fn the_war_survives_a_save() {
        use AiShipType::*;
        let mut map = row(&[TheSilence, TheSilence, TerranHegemony]);
        let mut war = BorderWar::default();
        war.push(&mut map, 2, TheSilence, -20.0);
        let json = serde_json::to_string(&war).unwrap();
        let back: BorderWar = serde_json::from_str(&json).unwrap();
        assert!((back.contest(2).unwrap().share(TerranHegemony) - 20.0).abs() < 1e-3);
    }

    /// Across many galaxies, not a lucky few: on the layout before this rule
    /// a faction died inside ten hours in one galaxy in four, and with systems
    /// further apart in more than one in two.
    #[test]
    fn the_war_alone_never_wipes_a_faction_out() {
        for seed in 0..50u64 {
            let mut map = crate::celestial::galaxy::generate_galaxy_map(seed * 31 + 11);
            let mut war = BorderWar::default();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            for _ in 0..600 {
                war.tick(&mut map, &mut rng);
            }
            for f in TERRITORIAL {
                assert!(map.systems.iter().any(|s| s.faction == Some(f)), "seed {seed}: {f:?} was wiped out");
            }
        }
    }

    /// The rule protects a faction from the background war, not from you.
    #[test]
    fn you_can_still_take_a_factions_last_system() {
        use AiShipType::*;
        let mut map = row(&[TheSilence, TheSilence, TerranHegemony]);
        let mut war = BorderWar::default();
        let flips = war.push(&mut map, 3, TheSilence, 60.0);
        assert_eq!(flips.len(), 1);
        assert_eq!(map.systems.iter().filter(|s| s.faction == Some(TerranHegemony)).count(), 0);
    }

    /// Ten hours of war on a real galaxy: borders move, but no faction ends
    /// up owning the map.
    #[test]
    fn no_faction_swallows_the_galaxy() {
        for seed in 0..4u64 {
            let mut map = crate::celestial::galaxy::generate_galaxy_map(seed + 11);
            let mut war = BorderWar::default();
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let mut flips = 0;
            for _ in 0..600 {
                flips += war.tick(&mut map, &mut rng).len();
            }
            let most = TERRITORIAL
                .iter()
                .map(|f| map.systems.iter().filter(|s| s.faction == Some(*f)).count())
                .max()
                .unwrap();
            let landless = TERRITORIAL.iter().filter(|f| !map.systems.iter().any(|s| s.faction == Some(**f))).count();
            println!("seed {seed}: {flips} systems changed hands in ten hours; the largest holds {most}; {landless} landless");
            assert_eq!(landless, 0, "seed {seed}: a faction was wiped out in ten hours");
            assert!(flips > 0, "seed {seed}: ten hours and nothing moved");
            assert!(most <= 7, "seed {seed}: one faction holds {most} of 30 after ten hours");
        }
    }
}
