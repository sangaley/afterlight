//! Traders going about their business, so a system looks lived in.
//!
//! A trader flies to a real station, stops at its berth a while, and goes on
//! to another, or out of the system altogether, to arrive in a neighbour's.
//! New ones keep setting out while a system is short of them, so losing a few
//! doesn't leave it dead. Who trades where follows the map: both trading
//! houses work anywhere that won't shoot them, and a navy or a noble house
//! runs freight through its own space. Where everybody is at war with them,
//! there's next to nobody: an empty lane says something too.
//!
//! Only the system you're in has stations with places to be. A trader in a
//! neighbouring system is simulated flying between points round its star.

use bevy::prelude::*;
use rand::Rng;

use super::components::*;
use super::fleet::{self, FleetKind, TradeRun};
use crate::celestial::resources::{GalaxyMap, StarSystemDef, SystemStreamingManager};
use crate::components::Ship;

/// Seconds between runs of this system.
const TICK_S: f32 = 1.0;
/// Seconds at a berth, least and most.
const DWELL_S: (f32, f32) = (45.0, 120.0);
/// A fleet's centre this close to its point has arrived.
const ARRIVE: f32 = 2_500.0;
/// Room left between a station's hull and a trader at its berth.
const BERTH_MARGIN: f32 = 3_000.0;
/// How far out from the star ships jump in and out.
const JUMP_RADIUS: f32 = 90_000.0;
/// Chance that a trader leaving a berth leaves the system.
const LEAVE_CHANCE: f32 = 0.25;
/// Off-screen pace. Stations can be 400 km apart: crossing at the old 60 u/s
/// took two hours, and nobody ever saw anyone arrive anywhere.
const CRUISE: (f32, f32) = (600.0, 900.0);
/// On-screen pace, at least. Going by thrust a laden freighter on patrol speed
/// made 80 u/s: 40 seconds to cross one ship-length, which from any distance
/// looked parked. Its escorts go a little faster, to keep up.
pub const SPEED_ON_SCREEN: f32 = 380.0;
pub const ESCORT_SPEED_ON_SCREEN: f32 = 460.0;

/// Fleets of traders a system keeps going. An empty lane where it's at war
/// with both trading houses.
pub fn trade_quota(system: &StarSystemDef) -> usize {
    // Nobody trades in the dark.
    if crate::narrative::front::is_taken(system) || trade_factions(system.faction).is_empty() {
        0
    } else if system.haunt.is_some() {
        3
    } else if system.capital {
        16
    } else {
        12
    }
}

/// Who runs freight through a system owned by `owner`.
pub fn trade_factions(owner: Option<AiShipType>) -> Vec<AiShipType> {
    use AiShipType::*;
    let mut who: Vec<AiShipType> = [MerchantPrinces, FreeHaulersCompact]
        .into_iter()
        .filter(|f| owner.is_none_or(|o| !factions_hostile(*f, o)))
        .collect();
    if let Some(o) = owner.filter(|o| matches!(o, TerranHegemony | GildedThrone)) {
        who.push(o);
    }
    who
}

/// How many trader fleets each system has, the lost ones too.
///
/// Only crewed fleets were counted, so a convoy lost to raiders was replaced
/// at once -- and then came back from the dead a few minutes later as well.
/// A lane the pirates work was the busiest in the galaxy, and no system's
/// traders ever stopped growing. A lost convoy keeps its place in the count
/// until it is struck off (`simulation::prune_the_lost`), and then one
/// new one sets out for it.
pub fn trader_counts(fleets: &[fleet::SimFleet]) -> std::collections::HashMap<u32, usize> {
    let mut counts: std::collections::HashMap<u32, usize> = Default::default();
    for fleet in fleets.iter().filter(|f| f.trade.is_some()) {
        *counts.entry(fleet.system_id).or_insert(0) += 1;
    }
    counts
}

/// The fleets on trade runs, for the helm.
#[derive(Resource, Default)]
pub struct TradingFleets(pub std::collections::HashSet<u32>);

/// The next place to go from `from`: any stop but the one it just left.
pub fn next_stop<'a>(stops: &'a [Stop], left: &str, rng: &mut impl Rng) -> Option<&'a Stop> {
    let others: Vec<&Stop> = stops.iter().filter(|s| s.name != left).collect();
    if others.is_empty() {
        stops.first()
    } else {
        Some(others[rng.gen_range(0..others.len())])
    }
}

/// A place a trader can call at.
#[derive(Clone, Debug)]
pub struct Stop {
    pub name: String,
    pub pos: Vec2,
    /// How far it reaches from `pos` at its furthest.
    pub radius: f32,
    /// Its solid parts, as circles measured from `pos`. Empty when all that
    /// is known of it is `radius`.
    pub shape: Vec<(Vec2, f32)>,
    /// Whether you dock along a lane running east from it (a Raft station).
    pub lane: bool,
    /// Stars and planets near it, as (centre, radius): no berth inside one.
    /// A starport sits a few km off its planet's surface.
    pub avoid: Vec<(Vec2, f32)>,
    /// How it is moving: a starport rides its planet.
    pub vel: Vec2,
    /// Where traders moor here, if they can.
    pub port: Option<PortId>,
}

/// How many visiting traders a port berths at once. Haven is the biggest;
/// the others have a handful of berths kept for visitors, the rest of the
/// city being its own.
pub fn port_berths(port: PortId) -> usize {
    match port {
        PortId::Station(0) => 8,
        _ => 5,
    }
}

/// Longest a trader waits outside a full port before trying another.
const ROADS_WAIT: f32 = 180.0;

/// AFTERLIGHT_PORT_OFF=1: traders stop outside and wait, as before berths.
fn ports_off() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_PORT_OFF").ok().as_deref() == Some("1"))
}

/// AFTERLIGHT_PORT_TRACE=1: a PORT line for every mooring, wait and cast-off.
pub fn port_trace() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_PORT_TRACE").ok().as_deref() == Some("1"))
}

/// Whether there is room at `port` for `want` more ships, with `in_port` there.
pub fn room_at(port: PortId, in_port: usize, want: usize) -> bool {
    want > 0 && in_port + want <= port_berths(port)
}

/// Which of a fleet's ships moor: its working hulls -- haulers, freighters,
/// liners -- not its guards, and not one with a price on its head (someone
/// is coming for it, and it can't hide in a berth).
pub fn moors(ship: &SimulatedShip) -> bool {
    ship.behavior != SimBehavior::Dead
        && ship.bounty_id.is_none()
        && fleet::role_of(ship.faction, ship.class) == super::roster::ClassRole::Civilian
}

/// Half the width of the strip a trader's berth keeps clear of hull.
const BERTH_CORRIDOR: f32 = 1_500.0;
/// Room a trader's berth keeps from the lane you dock along.
const LANE_CLEAR: f32 = 2_500.0;

impl Stop {
    /// How far its hull reaches from its middle toward `dir`.
    fn reach(&self, dir: Vec2) -> f32 {
        if self.shape.is_empty() {
            self.radius
        } else {
            crate::ship::collision::hull_reach(&self.shape, dir, BERTH_CORRIDOR).unwrap_or(self.radius)
        }
    }

    /// Where to stop, coming from `from`: on the near side, off its hull,
    /// and out of the lane you dock along.
    ///
    /// It was `radius + 3 km` on the bearing the trader came in on, and a
    /// Raft's `radius` is only its east arm: its other arms reach up to twice
    /// as far. From the west, north or south the berth lay among them, where
    /// steering round the station pushed the ship sideways for good.
    pub fn berth(&self, from: Vec2) -> Vec2 {
        let dir = (from - self.pos).normalize_or(Vec2::NEG_X);
        let first = self.berth_on(dir);
        if self.is_open(first) {
            return first;
        }
        // The planet is that way: the nearest bearing either side that isn't.
        for step in 1..=9 {
            for sign in [1.0, -1.0] {
                let turned = Vec2::from_angle(sign * step as f32 * 20.0_f32.to_radians()).rotate(dir);
                let at = self.berth_on(turned);
                if self.is_open(at) {
                    return at;
                }
            }
        }
        first
    }

    /// Whether a trader coming from `from` can keep the berth it was given.
    ///
    /// The berth for a bearing is not a smooth thing: a degree to one side
    /// an arm lies that way and the berth is out past its tip, a degree to
    /// the other it doesn't and the berth is close in. Worked out afresh
    /// every second from where the ship had got to, it jumped 7 km back and
    /// forth as a trader came up the edge of an arm. A berth that is still a
    /// good one, still on its side of the station, is kept.
    pub fn keeps(&self, berth: Vec2, from: Vec2) -> bool {
        let off = berth - self.pos;
        let dist = off.length();
        // Something that moves takes its berths with it.
        if self.vel.length() > 1.0 || dist < 1.0 {
            return false;
        }
        let near_side = off.dot(from - self.pos) > 0.0;
        let in_reach = dist <= self.radius + 2.0 * BERTH_MARGIN;
        let off_the_hull = self.shape.iter().all(|(c, r)| off.distance(*c) >= r + BERTH_CORRIDOR)
            && (!self.shape.is_empty() || dist >= self.radius + BERTH_MARGIN - 1.0);
        let keep = crate::world::raft::LANE_HALF_WIDTH + LANE_CLEAR;
        let out_of_the_lane = !self.lane || off.x <= 0.0 || off.y.abs() >= keep - 1.0;
        near_side && in_reach && off_the_hull && out_of_the_lane && self.is_open(berth)
    }

    /// Whether a berth at `at` is clear of every star and planet nearby.
    fn is_open(&self, at: Vec2) -> bool {
        self.avoid.iter().all(|(c, r)| at.distance(*c) >= r + BERTH_MARGIN)
    }

    /// The berth on the bearing `dir` from its middle.
    fn berth_on(&self, dir: Vec2) -> Vec2 {
        let off = dir * (self.reach(dir) + BERTH_MARGIN);
        let keep = crate::world::raft::LANE_HALF_WIDTH + LANE_CLEAR;
        if !self.lane || off.x <= 0.0 || off.y.abs() >= keep {
            return self.pos + off;
        }
        // In the lane: step out of it to the side it came from, and stand
        // off the hull on that bearing instead.
        let side = if off.y < 0.0 { -1.0 } else { 1.0 };
        let aside = Vec2::new(off.x, side * keep);
        let dir = aside.normalize();
        self.pos + dir * (self.reach(dir) + BERTH_MARGIN).max(aside.length())
    }
}

/// `at`, moved off this system's star and out of every planet's path, with
/// `margin` to spare: somewhere a ship can be told to go.
pub fn clear_point(def: &StarSystemDef, at: Vec2, margin: f32) -> Vec2 {
    let roll = crate::celestial::galaxy::system_roll(def);
    let planets = roll.planet_paths();
    crate::celestial::poi::KeepClear { center: def.local_center, star_radius: roll.star_radius(), planets: &planets }
        .nudge(at, margin)
}

/// Room a jump point keeps from anything solid: a convoy's width and more.
const JUMP_MARGIN: f32 = 6_000.0;

/// Where a ship leaves this system for `to`: out on the bearing to it, clear
/// of the star and the planets. It was a flat 90 km from the star's centre,
/// and stars are 40 to 150 km in radius: most jump points were inside one.
pub fn jump_point(from: &StarSystemDef, to: &StarSystemDef, sign: f32) -> Vec2 {
    let at = from.local_center + (to.galaxy_pos - from.galaxy_pos).normalize_or(Vec2::X) * JUMP_RADIUS * sign;
    clear_point(from, at, JUMP_MARGIN)
}

/// A point round the star for a trader with no station to make for.
fn open_point(def: &StarSystemDef, rng: &mut impl Rng) -> Vec2 {
    let at = def.local_center + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(30_000.0..120_000.0);
    clear_point(def, at, JUMP_MARGIN)
}

/// What a trader is called: the same every time you meet it, and nothing to
/// save, since it comes from its fleet.
pub fn trader_name(id: u32, faction: AiShipType) -> String {
    const FIRST: [&str; 16] = ["Patient", "Long", "Quiet", "Second", "Honest", "Far", "Slow", "Lucky", "Grey", "Last", "Steady", "Old", "Bright", "Hard", "Fair", "Late"];
    const SECOND: [&str; 16] = ["Margin", "Haul", "Ledger", "Tide", "Promise", "Harvest", "Passage", "Wager", "Anchor", "Reckoning", "Bargain", "Season", "Mercy", "Ballast", "Errand", "Return"];
    let prefix = match faction {
        AiShipType::MerchantPrinces => "MV",
        AiShipType::FreeHaulersCompact => "FH",
        AiShipType::TerranHegemony => "THS",
        AiShipType::GildedThrone => "GTV",
        _ => "SS",
    };
    // Mixed so neighbouring ids don't read as a list.
    let n = id.wrapping_mul(2_654_435_761);
    format!("{prefix} {} {}", FIRST[(n >> 8) as usize % 16], SECOND[(n >> 20) as usize % 16])
}

/// What it's carrying, the same way.
pub fn trader_cargo(id: u32) -> &'static str {
    const CARGO: [&str; 10] = ["ore", "fuel", "grain", "reactor parts", "medical stores", "water", "machine tools", "scrap", "hull plate", "passengers"];
    CARGO[(id.wrapping_mul(40_503) >> 6) as usize % 10]
}

/// The line under a trader's name.
pub fn trader_label(fleet: &fleet::SimFleet, class: Option<&str>) -> String {
    let what = class.unwrap_or("Freighter");
    let name = trader_name(fleet.id, fleet.faction);
    // What's really in its hold (world::economy); the old made-up word if
    // nothing is.
    let cargo = match fleet.trade.as_ref().and_then(|t| t.cargo) {
        Some(c) => format!("{:.0} {}", c.units, c.item.name().to_lowercase()),
        None => format!("{}, in ballast", trader_cargo(fleet.id)),
    };
    match fleet.trade.as_ref() {
        Some(run) if run.refugees => format!("{name}, {what}: refugees, for {}", run.bound_for),
        Some(run) if run.dwell > 0.0 => format!("{name}, {what}: {cargo}, in port"),
        Some(run) if !run.bound_for.is_empty() => format!("{name}, {what}: {cargo} for {}", run.bound_for),
        _ => format!("{name}, {what}: {cargo}"),
    }
}

/// Fly every trader in the systems that are running, and keep them coming.
#[allow(clippy::too_many_arguments)]
pub fn run_traffic(
    time: Res<Time>,
    mut next: Local<f32>,
    mut sim: ResMut<WorldSimulation>,
    streaming: Res<SystemStreamingManager>,
    galaxy: Res<GalaxyMap>,
    stations: Res<crate::world::home_base::SystemStations>,
    ports: Res<crate::diplomacy::stations::FactionStations>,
    player: Query<&Transform, With<Ship>>,
    mut trading: ResMut<TradingFleets>,
    mut leaders: Query<(&Transform, &crate::components::Velocity, &mut AiShipNav, &super::fleet::FleetMember)>,
    hulls: Query<(&crate::world::home_base::Station, &Transform, &crate::ship::collision::Collider)>,
    nav_bodies: Option<Res<super::nav::NavBodies>>,
    trade: (ResMut<crate::resources::MarketEvents>, Commands, Query<(Entity, &super::fleet::FleetMember, &AiShipType, Option<&AiShipClass>, Has<CarriedCargo>)>),
    mut flips: MessageReader<crate::celestial::territory::SystemChangedHands>,
    mut lost: Local<Vec<(Vec<PortId>, Option<AiShipType>)>>,
) {
    let (mut market, mut commands, crews) = trade;
    // Read every frame (a message lives two): the ports of a system that
    // changed hands, to be cleared of traders the new owner would shoot.
    for flip in flips.read() {
        if let Some(def) = galaxy.systems.iter().find(|s| s.id == flip.system_id) {
            let mut here: Vec<PortId> = crate::world::home_base::station_sites(def).iter().map(|s| PortId::Station(s.index)).collect();
            here.push(PortId::Starport(flip.system_id));
            lost.push((here, Some(flip.to)));
        }
    }
    if crate::demo::skip_ai_ship_spawn() && std::env::var("AFTERLIGHT_TRAFFIC").is_err() {
        return;
    }
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + TICK_S;
    let Some(loaded) = streaming.loaded_system else { return };
    let mut rng = rand::thread_rng();
    let player_pos = player.single().ok().map(|t| t.translation.truncate());
    let active = |id: u32| id == loaded || streaming.warm_systems.contains(&id);

    // Places to call at, here in the system you're in.
    let mut stops: Vec<Stop> = stations
        .sites
        .iter()
        .map(|s| {
            // Its hull as it stands, where it has been built.
            let shape = hulls
                .iter()
                .find(|(station, ..)| station.index == s.index)
                .map(|(_, tf, c)| c.circles.iter().map(|(at, r)| ((tf.rotation * at.extend(0.0)).truncate(), *r)).collect())
                .unwrap_or_default();
            Stop { name: s.name.clone(), pos: s.pos, radius: crate::world::home_base::station_extent(s.index), shape, lane: true, avoid: Vec::new(), vel: Vec2::ZERO, port: Some(PortId::Station(s.index)) }
        })
        .collect();
    if let Some((at, ring)) = ports.harbour(loaded) {
        let owner = galaxy.systems.iter().find(|s| s.id == loaded).and_then(|s| s.faction);
        let name = owner.map_or("the starport".to_string(), |f| format!("{} starport", faction_display_name(f)));
        // The planet it sits by, and how fast the two of them are going.
        let bodies = nav_bodies.as_deref().map_or(&[][..], |b| b.loaded.as_slice());
        let avoid = bodies.iter().filter(|b| b.solid && b.center.distance(at) < ring + b.radius + 30_000.0).map(|b| (b.center, b.radius)).collect();
        let vel = bodies.iter().find(|b| !b.solid && b.center.distance(at) < 1_000.0).map_or(Vec2::ZERO, |b| b.vel);
        stops.push(Stop { name, pos: at, radius: ring, shape: Vec::new(), lane: false, avoid, vel, port: Some(PortId::Starport(loaded)) });
    }

    let sim: &mut WorldSimulation = &mut sim;
    // A starport shot down: everyone moored there cuts loose.
    for &system in &ports.destroyed {
        lost.push((vec![PortId::Starport(system)], None));
    }
    for (gone, owner) in lost.drain(..) {
        let n = cut_loose(sim, &gone, owner);
        if n > 0 && port_trace() {
            info!("PORT {n} ships cut loose from {gone:?}");
        }
    }
    let manned: std::collections::HashSet<u32> =
        sim.ships.iter().filter(|s| s.fleet != 0 && s.behavior != SimBehavior::Dead).map(|s| s.fleet).collect();
    let live: std::collections::HashSet<u32> =
        sim.ships.iter().filter(|s| s.fleet != 0 && s.spawned).map(|s| s.fleet).collect();

    // A convoy made before this ran (or by a border crossing) joins in.
    for fleet in sim.fleets.iter_mut() {
        if fleet.kind == FleetKind::Convoy && fleet.trade.is_none() && active(fleet.system_id) {
            fleet.trade = Some(TradeRun::default());
            fleet.route.clear();
            fleet.cruise = rng.gen_range(CRUISE.0..CRUISE.1);
        }
    }

    // A trader on screen that can't get to its point (movement says so)
    // stops where it has got to: its fleet's point becomes where it is.
    let mut gave_up: std::collections::HashSet<u32> = Default::default();
    for (_, _, mut nav, member) in leaders.iter_mut().filter(|(.., m)| m.leader) {
        if nav.blocked && sim.fleet(member.fleet).is_some_and(|f| f.trade.is_some()) {
            nav.blocked = false;
            gave_up.insert(member.fleet);
        }
    }

    let mut jumps: Vec<(u32, u32, Vec2)> = Vec::new();
    let mut jumping_out: Vec<u32> = Vec::new();
    // Ports: who is in each, and which fleets have ships in a berth.
    let mut in_port: std::collections::HashMap<PortId, usize> = Default::default();
    let mut moored_of: std::collections::HashMap<u32, Vec<u32>> = Default::default();
    for s in sim.ships.iter().filter(|s| s.moored.is_some()) {
        *in_port.entry(s.moored.unwrap()).or_insert(0) += 1;
        moored_of.entry(s.fleet).or_default().push(s.id);
    }
    let mooring_crews: std::collections::HashMap<u32, Vec<(u32, bool)>> = {
        let mut m: std::collections::HashMap<u32, Vec<(u32, bool)>> = Default::default();
        for s in sim.ships.iter().filter(|s| s.fleet != 0 && s.moored.is_none() && moors(s)) {
            m.entry(s.fleet).or_default().push((s.id, s.spawned));
        }
        m
    };
    // The ports here are on the books: their stock drifts from now on.
    for port in stops.iter().filter_map(|s| s.port) {
        market.stock.touch(crate::world::economy::port_index(port));
    }
    // What traders are already carrying to each port.
    let mut pledged: std::collections::HashMap<(usize, crate::resources::ItemType), f32> = Default::default();
    for c in sim.fleets.iter().filter_map(|f| f.trade.as_ref().and_then(|t| t.cargo)) {
        *pledged.entry((c.dest, c.item)).or_insert(0.0) += c.units;
    }
    // (fleet, port) to moor this tick; ships whose time in port is up.
    let mut to_moor: Vec<(u32, PortId)> = Vec::new();
    let mut casting: Vec<u32> = Vec::new();
    for fleet in sim.fleets.iter_mut().filter(|f| f.trade.is_some() && active(f.system_id) && manned.contains(&f.id)) {
        let here = fleet.system_id == loaded;
        let is_live = live.contains(&fleet.id);
        let position = fleet.position;
        let Some(run) = fleet.trade.as_mut() else { continue };

        // Stations ride their planets: keep aiming at where it is now.
        if here && run.dwell <= 0.0 && run.leaving.is_none() {
            if let Some(stop) = stops.iter().find(|s| s.name == run.bound_for) {
                if !stop.keeps(fleet.destination, position) {
                    fleet.destination = stop.berth(position);
                }
            }
        }
        if gave_up.contains(&fleet.id) && run.dwell <= 0.0 {
            fleet.destination = position;
        }
        // Still has ships in a berth with its time up: they are casting off;
        // it waits for them.
        if run.dwell <= 0.0 && moored_of.contains_key(&fleet.id) {
            fleet.velocity = Vec2::ZERO;
            fleet.destination = position;
            casting.extend(moored_of[&fleet.id].iter().copied());
            continue;
        }
        if run.dwell > 0.0 {
            run.dwell -= TICK_S;
            fleet.velocity = Vec2::ZERO;
            fleet.destination = position;
            // In port at something that moves: stay with it. A starport and
            // its planet go 150 u/s, and a trader left where it stopped was
            // a few km behind in a minute, or under the planet.
            if let Some(stop) = stops.iter().find(|s| here && s.name == run.last_stop && s.vel.length() > 1.0) {
                fleet.destination = stop.berth(position);
                if !is_live {
                    fleet.position = fleet.destination;
                }
            }
        } else if position.distance(fleet.destination) <= ARRIVE {
            match run.leaving {
                Some(to) if !is_live => jumps.push((fleet.id, to, position)),
                // On screen at the jump: it goes, where you can see it go.
                // (One on screen used to think better of it and stay, and
                // refugees waited there until nobody was watching.)
                Some(_) => {
                    fleet.velocity = Vec2::ZERO;
                    jumping_out.push(fleet.id);
                }
                // Already arrived and done here (its ships just backed out
                // of their berths): it's leaving, not arriving again. Taken
                // as a second arrival, it lost which port it had been at
                // and sat out a second stay outside.
                None if run.bound_for.is_empty() => {}
                None => {
                    // Unloads its cargo where it was going.
                    let arrived_at = here.then(|| stops.iter().find(|s| s.name == run.bound_for).and_then(|s| s.port)).flatten();
                    if let (Some(at), Some(c)) = (arrived_at, run.cargo) {
                        let idx = crate::world::economy::port_index(at);
                        market.stock.add(idx, c.item, c.units);
                        run.cargo = None;
                        if crate::world::economy::econ_trace() {
                            info!("ECON {} delivers {:.0} {} to {}", trader_name(fleet.id, fleet.faction), c.units, c.item.name(), run.bound_for);
                        }
                    }
                    // Into a berth, if the port has room for its working
                    // ships; otherwise it waits outside, and in the end
                    // tries somewhere else.
                    let port = (here && !ports_off()).then(|| stops.iter().find(|s| s.name == run.bound_for).and_then(|s| s.port)).flatten();
                    let crew = mooring_crews.get(&fleet.id).map_or(0, |c| c.len());
                    let mut moored = false;
                    if let Some(port) = port.filter(|_| crew > 0) {
                        let there = in_port.get(&port).copied().unwrap_or(0);
                        if room_at(port, there, crew) {
                            *in_port.entry(port).or_insert(0) += crew;
                            to_moor.push((fleet.id, port));
                            run.waited = 0.0;
                            moored = true;
                        } else if run.waited < ROADS_WAIT {
                            run.waited += TICK_S;
                            // Holding where it is: every fleet queuing for the
                            // same port made for the same point by the lane,
                            // and their ships jostled there and stalled.
                            fleet.velocity = Vec2::ZERO;
                            fleet.destination = position;
                            if port_trace() && run.waited <= TICK_S {
                                info!("PORT {} waits outside {} ({there} of {} berths taken)", trader_name(fleet.id, fleet.faction), run.bound_for, port_berths(port));
                            }
                            continue;
                        } else {
                            run.waited = 0.0;
                            if port_trace() {
                                info!("PORT {} gives up waiting at {}", trader_name(fleet.id, fleet.faction), run.bound_for);
                            }
                        }
                    }
                    run.leaving = None;
                    run.last_stop = std::mem::take(&mut run.bound_for);
                    run.dwell = if moored || port.is_none() { rng.gen_range(DWELL_S.0..DWELL_S.1) } else { 0.0 };
                    fleet.velocity = Vec2::ZERO;
                }
            }
        }
        // Time to go: another stop, or out -- once its ships are out of their
        // berths. Choosing in the same tick its stay ran out, it counted an
        // empty hold (the haulers still moored) and never loaded anything.
        if run.dwell <= 0.0 && run.bound_for.is_empty() && run.leaving.is_none() && !moored_of.contains_key(&fleet.id) {
            let Some(from) = galaxy.systems.iter().find(|s| s.id == fleet.system_id) else { continue };
            let leave = (rng.gen::<f32>() < LEAVE_CHANCE)
                .then(|| streaming.warm_systems.iter().copied().chain([loaded]).filter(|s| *s != fleet.system_id).collect::<Vec<u32>>())
                .filter(|o| !o.is_empty())
                .map(|o| o[rng.gen_range(0..o.len())])
                .and_then(|to| galaxy.systems.iter().find(|s| s.id == to))
                .filter(|to| trade_quota(to) > 0 && trade_factions(to.faction).contains(&fleet.faction));
            if let Some(to) = leave {
                run.leaving = Some(to.id);
                run.bound_for = to.name.clone();
                fleet.destination = jump_point(from, to, 1.0);
            } else if here {
                // Where to, by what pays: load what's cheap here and take it
                // where it's dear (world::economy::choose_run). Now and then,
                // or with nothing worth carrying, anywhere.
                let origin = stops.iter().find(|s| s.name == run.last_stop).and_then(|s| s.port).map(crate::world::economy::port_index);
                let candidates: Vec<(usize, f32)> =
                    stops.iter().filter_map(|s| Some((crate::world::economy::port_index(s.port?), s.pos.distance(position)))).collect();
                let hold = crate::world::economy::hold_of(mooring_crews.get(&fleet.id).map_or(0, |c| c.len()));
                let deal = origin
                    .filter(|_| rng.gen::<f32>() > 0.1 && hold > 0.0)
                    .and_then(|o| crate::world::economy::choose_run(&market.stock, o, &candidates, &pledged, hold, &mut rng).map(|d| (o, d)));
                if crate::world::economy::econ_trace() {
                    info!(
                        "ECON {} leaves {:?} (port {:?}), hold {:.0}, {} places to go: {:?}",
                        trader_name(fleet.id, fleet.faction), run.last_stop, origin, hold, candidates.len(), deal.map(|(_, d)| d)
                    );
                }
                let chosen = deal.and_then(|(o, (dest, item, units))| {
                    stops.iter().find(|s| s.port.map(crate::world::economy::port_index) == Some(dest)).map(|s| (s, o, dest, item, units))
                });
                let stop = match chosen {
                    Some((stop, origin, dest, item, units)) => {
                        market.stock.add(origin, item, -units);
                        *pledged.entry((dest, item)).or_insert(0.0) += units;
                        run.cargo = Some(super::fleet::Cargo { item, units, dest });
                        if crate::world::economy::econ_trace() {
                            info!("ECON {} loads {:.0} {} at {} for {}", trader_name(fleet.id, fleet.faction), units, item.name(), run.last_stop, stop.name);
                        }
                        stop
                    }
                    None => {
                        let Some(stop) = next_stop(&stops, &run.last_stop, &mut rng) else {
                            fleet.destination = open_point(from, &mut rng);
                            continue;
                        };
                        stop
                    }
                };
                run.bound_for = stop.name.clone();
                fleet.destination = stop.berth(position);
            } else {
                // A neighbouring system's stations aren't there to fly to.
                run.bound_for = from.name.clone();
                fleet.destination = open_point(from, &mut rng);
            }
        }
    }

    // On screen at its jump point: off screen, this frame (sync folds it
    // back), and through the jump on the next turn of this.
    sim.fold_back.extend(jumping_out);

    // Into the berths: a ship on screen is taken off it at the gate (sync
    // does that, and the station tows it in); one out of sight just moors.
    for (fleet_id, port) in to_moor {
        let name = sim.fleet(fleet_id).map(|f| trader_name(f.id, f.faction)).unwrap_or_default();
        let mut n = 0;
        for (id, live) in mooring_crews.get(&fleet_id).cloned().unwrap_or_default() {
            if let Some(ship) = sim.ships.iter_mut().find(|s| s.id == id) {
                ship.moored = Some(port);
                ship.velocity = Vec2::ZERO;
                n += 1;
            }
            if live {
                sim.stow.push(id);
            }
        }
        if port_trace() {
            info!("PORT {name} moors {n} at {port:?}");
        }
    }
    // Out of the berths: the stations back out those they have; the rest
    // are let go (ai_ship::traffic::let_go).
    for id in casting {
        if !sim.cast_off.contains(&id) {
            sim.cast_off.push(id);
        }
    }

    // Through the jump: out of one system, into the next.
    let mut flashes: Vec<(u32, Vec2, bool, AiShipType)> = Vec::new();
    for (id, to, left_from) in jumps {
        let Some(fleet) = sim.fleets.iter_mut().find(|f| f.id == id) else { continue };
        let Some((from, dest)) = galaxy.systems.iter().find(|s| s.id == fleet.system_id).zip(galaxy.systems.iter().find(|s| s.id == to)) else { continue };
        let arrive = jump_point(dest, from, 1.0);
        // Not out of nowhere, in front of you: a jump in is seen from a
        // distance. (It was held back inside 72 km, where its far picture
        // would have shown it coming.)
        if to == loaded && player_pos.is_some_and(|p| p.distance(arrive) < super::simulation::ARRIVAL_RANGE) {
            continue;
        }
        flashes.push((fleet.system_id, left_from, false, fleet.faction));
        flashes.push((to, arrive, true, fleet.faction));
        fleet.system_id = to;
        fleet.position = arrive;
        fleet.home_zone = arrive;
        fleet.velocity = Vec2::ZERO;
        fleet.destination = arrive;
        if let Some(run) = fleet.trade.as_mut() {
            run.leaving = None;
            run.refugees = false;
            run.bound_for.clear();
            run.last_stop.clear();
            run.dwell = 1.0;
        }
        let at = fleet.position;
        for ship in sim.ships.iter_mut().filter(|s| s.fleet == id) {
            ship.system_id = to;
            ship.position = at + ship.slot;
            ship.destination = at;
            ship.velocity = Vec2::ZERO;
        }
    }

    for (system, at, arriving, faction) in flashes {
        sim.flash(system, at, arriving, faction);
    }

    trading.0.clear();
    trading.0.extend(sim.fleets.iter().filter(|f| f.trade.is_some()).map(|f| f.id));

    // The haulers on screen carry their fleet's cargo, for their wrecks.
    let carrying: std::collections::HashMap<u32, crate::resources::ItemType> = sim
        .fleets
        .iter()
        .filter_map(|f| Some((f.id, f.trade.as_ref()?.cargo?.item)))
        .collect();
    for (e, member, faction, class, has) in crews.iter() {
        let hauler = fleet::role_of(*faction, class.map(|c| c.0)) == super::roster::ClassRole::Civilian;
        match carrying.get(&member.fleet).filter(|_| hauler) {
            Some(item) => {
                commands.entity(e).try_insert(CarriedCargo(*item));
            }
            None if has => {
                commands.entity(e).try_remove::<CarriedCargo>();
            }
            None => {}
        }
    }

    // Ships on screen are flown by their leader: give it the same orders.
    let orders: std::collections::HashMap<u32, Vec2> = sim
        .fleets
        .iter()
        .filter(|f| f.trade.is_some() && live.contains(&f.id))
        .map(|f| (f.id, f.destination))
        .collect();
    for (tf, velocity, mut nav, member) in leaders.iter_mut().filter(|(.., m)| m.leader) {
        if let Some(to) = orders.get(&member.fleet) {
            if crate::combat::gun_assembly::trace() && (now % 5.0) < TICK_S {
                info!("TRADER {} on screen at {:.0},{:.0} going {:.0} u/s, {:.0} km from its point", member.fleet, tf.translation.x, tf.translation.y, velocity.0.length(), tf.translation.truncate().distance(*to) / 1000.0);
            }
            if nav.waypoints.len() != 1 || nav.waypoints[0] != *to {
                nav.waypoints = vec![*to];
                nav.current_waypoint = 0;
                nav.destination = Some(*to);
            }
        }
    }

    // Keep them coming, a couple at a time.
    let counts = trader_counts(&sim.fleets);
    let systems: Vec<u32> = streaming.warm_systems.iter().copied().chain([loaded]).collect();
    let mut arrivals: Vec<(u32, Vec2, AiShipType)> = Vec::new();
    for id in systems {
        let Some(system) = galaxy.systems.iter().find(|s| s.id == id) else { continue };
        let quota = trade_quota(system);
        let have = counts.get(&id).copied().unwrap_or(0);
        for _ in have..quota.min(have + 2) {
            let who = trade_factions(system.faction);
            let faction = who[rng.gen_range(0..who.len())];
            // Half run alone, half in convoy with guards.
            let wanted = if rng.gen::<bool>() { FleetKind::Convoy } else { FleetKind::Patrol };
            let mut held = fleet::Held::new();
            let Some(hulls) = fleet::compose(faction, wanted, &mut rng, &mut held).filter(|h| !h.is_empty()) else { continue };
            // Already on its way somewhere, and not on top of you.
            let stop = (id == loaded && !stops.is_empty()).then(|| stops[rng.gen_range(0..stops.len())].clone());
            // In by a jump from next door, like anyone else. They used to
            // begin 55-110 km from the station they were bound for, on any
            // bearing, as if they had always been there.
            let at = super::simulation::jump_in_point(&galaxy, system, Some(faction), &mut rng);
            if id == loaded && player_pos.is_some_and(|p| p.distance(at) < super::simulation::ARRIVAL_RANGE) {
                continue;
            }
            arrivals.push((id, at, faction));
            let (mut new_fleet, members) = fleet::new_fleet(faction, FleetKind::Convoy, &hulls, id, at, at, 1.0e9, &mut rng);
            new_fleet.cruise = rng.gen_range(CRUISE.0..CRUISE.1);
            new_fleet.trade = Some(TradeRun {
                bound_for: stop.as_ref().map_or_else(String::new, |s| s.name.clone()),
                ..default()
            });
            new_fleet.destination = stop.as_ref().map_or(at, |s| s.berth(at));
            sim.add_fleet(new_fleet, members);
        }
    }
    for (system, at, faction) in arrivals {
        sim.flash(system, at, true, faction);
    }
    if crate::combat::gun_assembly::trace() && (now % 10.0) < TICK_S {
        let docked = sim.fleets.iter().filter(|f| f.system_id == loaded && f.trade.as_ref().is_some_and(|t| t.dwell > 0.0)).count();
        let bound: Vec<String> = sim.fleets.iter().filter(|f| f.system_id == loaded && f.trade.is_some()).take(4)
            .map(|f| format!("{} to {} {:.0} km off", trader_name(f.id, f.faction), f.trade.as_ref().map_or("", |t| t.bound_for.as_str()), f.position.distance(f.destination) / 1000.0)).collect();
        info!("TRAFFIC {} trader fleets here, {} in port, {} stops: {:?}", counts.get(&loaded).copied().unwrap_or(0), docked, stops.len(), bound);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(name: &str, x: f32) -> Stop {
        Stop { name: name.into(), pos: Vec2::new(x, 0.0), radius: 1_000.0, shape: Vec::new(), lane: false, avoid: Vec::new(), vel: Vec2::ZERO, port: None }
    }

    #[test]
    fn a_trader_goes_somewhere_else_next() {
        let stops = [stop("Haven", 0.0), stop("Ostrava", 50_000.0)];
        let mut rng = rand::thread_rng();
        for _ in 0..20 {
            assert_eq!(next_stop(&stops, "Haven", &mut rng).unwrap().name, "Ostrava");
        }
        // With one place to go, it goes there.
        assert_eq!(next_stop(&stops[..1], "Haven", &mut rng).unwrap().name, "Haven");
        assert!(next_stop(&[], "Haven", &mut rng).is_none());
    }

    #[test]
    fn it_stops_on_the_near_side_off_the_hull() {
        let berth = stop("Haven", 0.0).berth(Vec2::new(40_000.0, 0.0));
        assert_eq!(berth, Vec2::new(1_000.0 + BERTH_MARGIN, 0.0));
    }

    /// A hub with four arms of different lengths, the east one shortest
    /// (as a Raft's is: its `radius`), and a lane to dock along.
    fn raft() -> Stop {
        let mut shape = vec![(Vec2::ZERO, 2_000.0)];
        for (dir, tip) in [(Vec2::X, 10_000.0), (Vec2::Y, 15_000.0), (Vec2::NEG_X, 17_000.0), (Vec2::NEG_Y, 19_000.0)] {
            let mut d = 2_000.0;
            while d <= tip - 700.0 {
                shape.push((dir * d, 700.0));
                d += 1_000.0;
            }
        }
        Stop { name: "Raft".into(), pos: Vec2::new(5_000.0, -3_000.0), radius: 19_000.0, shape, lane: true, avoid: Vec::new(), vel: Vec2::ZERO, port: None }
    }

    #[test]
    fn a_berth_is_outside_every_part_of_the_station() {
        let s = raft();
        for i in 0..64 {
            let from = s.pos + Vec2::from_angle(std::f32::consts::TAU * i as f32 / 64.0) * 80_000.0;
            let berth = s.berth(from);
            for (c, r) in &s.shape {
                let gap = berth.distance(s.pos + *c) - r;
                assert!(gap >= BERTH_CORRIDOR - 1.0, "bearing {i}: {gap:.0} u off a part of the hull");
            }
        }
    }

    /// Coming up the edge of an arm, the berth it was given is the berth it
    /// keeps: it doesn't jump between out-past-the-tip and close-in as the
    /// bearing wavers.
    #[test]
    fn a_berth_once_given_is_kept_while_it_is_still_good() {
        // A hub, and one big thing moored 12 km north of it.
        let s = Stop {
            name: "Raft".into(),
            pos: Vec2::new(5_000.0, -3_000.0),
            radius: 14_000.0,
            shape: vec![(Vec2::ZERO, 2_000.0), (Vec2::new(0.0, 12_000.0), 1_500.0)],
            lane: true,
            avoid: Vec::new(),
            vel: Vec2::ZERO,
            port: None,
        };
        // A degree either side of where it comes into the strip the berth
        // keeps clear: out past it, or close in by the hub.
        let from = |deg: f32| s.pos + Vec2::from_angle((90.0 - deg).to_radians()) * 60_000.0;
        // (West of north, away from the lane, which has rules of its own.)
        let (clear, inside) = (from(-15.5), from(-13.5));
        let (a, b) = (s.berth(clear), s.berth(inside));
        assert!(a.distance(b) > 5_000.0, "the two bearings give berths {:.0} apart; no jump to test", a.distance(b));
        // Whichever it had, it keeps, from either bearing.
        for berth in [a, b] {
            assert!(s.keeps(berth, clear) && s.keeps(berth, inside), "lost a good berth at {}", berth - s.pos);
        }
        // Not one on the far side of the station, in the lane, in the hull,
        // or nowhere near.
        assert!(!s.keeps(a, s.pos + Vec2::new(0.0, -60_000.0)));
        assert!(!s.keeps(s.pos + Vec2::new(14_000.0, 500.0), s.pos + Vec2::new(60_000.0, 500.0)));
        assert!(!s.keeps(s.pos + Vec2::new(0.0, 9_500.0), clear));
        assert!(!s.keeps(s.pos + Vec2::new(0.0, 90_000.0), clear));
        assert!(!s.keeps(s.pos, clear));
        // A starport moves: its berths are worked out afresh.
        let port = Stop { vel: Vec2::new(150.0, 0.0), ..s.clone() };
        assert!(!port.keeps(a, clear));
    }

    #[test]
    fn a_berth_between_two_arms_is_nearer_than_the_longest_arm() {
        let s = raft();
        // Coming in between the east and north arms: nothing but the hub
        // lies that way.
        let from = s.pos + Vec2::new(1.0, 1.0).normalize() * 80_000.0;
        let off = s.berth(from).distance(s.pos);
        assert!(off < 9_000.0, "stood {off:.0} off the middle, as far as its longest arm");
    }

    #[test]
    fn no_berth_is_in_the_lane_you_dock_along() {
        let s = raft();
        let keep = crate::world::raft::LANE_HALF_WIDTH + LANE_CLEAR;
        for i in 0..64 {
            let from = s.pos + Vec2::from_angle(std::f32::consts::TAU * i as f32 / 64.0) * 80_000.0;
            let off = s.berth(from) - s.pos;
            assert!(off.x <= 0.0 || off.y.abs() >= keep - 1.0, "bearing {i}: berth at {off} is in the lane");
        }
        // A starport has no lane: its berth stays on the bearing.
        let port = Stop { lane: false, ..raft() };
        let off = port.berth(port.pos + Vec2::X * 80_000.0) - port.pos;
        assert!(off.y.abs() < 1.0);
    }

    /// Every way out of every system: clear of its star and of the path of
    /// every planet, with a convoy's width to spare.
    #[test]
    fn a_jump_point_is_outside_the_star_and_every_planets_path() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let mut checked = 0;
        for from in &galaxy.systems {
            let roll = crate::celestial::galaxy::system_roll(from);
            let planets = roll.planet_paths();
            let keep = crate::celestial::poi::KeepClear { center: from.local_center, star_radius: roll.star_radius(), planets: &planets };
            for id in crate::celestial::galaxy::nearest_neighbors(&galaxy, from.id, 3) {
                let to = galaxy.systems.iter().find(|s| s.id == id).unwrap();
                let at = jump_point(from, to, 1.0);
                assert!(at.distance(from.local_center) > roll.star_radius(), "{} to {}: inside the star", from.name, to.name);
                assert!(keep.is_clear_by(at, JUMP_MARGIN - 1.0), "{} to {}: something solid comes by", from.name, to.name);
                checked += 1;
            }
        }
        assert!(checked > 60, "{checked}");
    }

    /// A starport sits a few km off its planet: whichever way a trader
    /// comes from, its berth is not in the planet.
    #[test]
    fn no_berth_is_inside_the_planet_the_station_sits_by() {
        let planet = (Vec2::new(-40_000.0, 0.0), 25_000.0);
        let port = Stop { name: "port".into(), pos: Vec2::ZERO, radius: 12_000.0, shape: Vec::new(), lane: false, avoid: vec![planet], vel: Vec2::ZERO, port: None };
        for i in 0..64 {
            let from = Vec2::from_angle(std::f32::consts::TAU * i as f32 / 64.0) * 200_000.0;
            let berth = port.berth(from);
            assert!(berth.distance(planet.0) >= planet.1 + BERTH_MARGIN - 1.0, "bearing {i}: berth {:.0} from the planet's middle", berth.distance(planet.0));
            assert!(berth.length() >= 12_000.0 + BERTH_MARGIN - 1.0);
        }
        // From the far side it stays on the bearing it came in on.
        assert!(port.berth(Vec2::new(200_000.0, 0.0)).y.abs() < 1.0);
    }

    /// A convoy with all hands lost still holds its place: it isn't replaced
    /// while it is waiting to be struck off.
    #[test]
    fn a_lost_convoy_keeps_its_place_in_the_count() {
        let mut fleets = Vec::new();
        for (id, system, trading) in [(1, 4, true), (2, 4, true), (3, 4, false), (4, 9, true)] {
            let mut f = fleet::SimFleet::new(FleetKind::Convoy, AiShipType::MerchantPrinces, system, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
            f.id = id;
            f.trade = trading.then(TradeRun::default);
            fleets.push(f);
        }
        // Nothing here says who is alive: none of them need be.
        let counts = trader_counts(&fleets);
        assert_eq!(counts.get(&4), Some(&2));
        assert_eq!(counts.get(&9), Some(&1));
        assert_eq!(counts.get(&7), None);
    }

    #[test]
    fn nobody_trades_in_the_dark() {
        let mut galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let system = galaxy.systems.iter_mut().find(|s| s.faction == Some(AiShipType::TerranHegemony)).unwrap();
        assert!(trade_quota(system) > 0);
        system.dark = crate::narrative::front::TAKEN;
        assert_eq!(trade_quota(system), 0);
    }

    #[test]
    fn nobody_trades_where_they_get_shot_at() {
        use AiShipType::*;
        assert!(trade_factions(None).contains(&MerchantPrinces), "Haven trades with everyone");
        assert!(trade_factions(Some(TerranHegemony)).contains(&TerranHegemony), "a navy runs its own freight");
        for owner in [BrokenChoir, RecursiveKingdom] {
            for f in trade_factions(Some(owner)) {
                assert!(!factions_hostile(f, owner), "{f:?} in {owner:?} space");
            }
        }
    }

    #[test]
    fn a_ship_keeps_its_name() {
        let name = trader_name(41, AiShipType::MerchantPrinces);
        assert_eq!(name, trader_name(41, AiShipType::MerchantPrinces));
        assert!(name.starts_with("MV "));
        let names: std::collections::HashSet<String> = (0..50).map(|i| trader_name(i, AiShipType::MerchantPrinces)).collect();
        assert!(names.len() > 20, "enough of them to feel like a fleet: {}", names.len());
    }

    #[test]
    fn its_label_says_where_it_is_going() {
        let mut f = fleet::SimFleet::new(FleetKind::Convoy, AiShipType::MerchantPrinces, 0, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
        f.trade = Some(TradeRun { bound_for: "Haven Station".into(), ..default() });
        assert!(trader_label(&f, Some("Bulk Freighter")).contains("for Haven Station"));
        f.trade = Some(TradeRun { dwell: 5.0, ..default() });
        assert!(trader_label(&f, Some("Bulk Freighter")).contains("in port"));
    }
}

/// Take back the ships a station has backed out of their berths: afloat
/// where it left them. First thing, before any berth is handed out, or a
/// ship just backed out is still "moored" and is given a berth again.
pub fn take_back_released(sim: &mut WorldSimulation) {
    let mut released: Vec<(u32, Vec2)> = std::mem::take(&mut sim.released);
    // The picture and its lights both report it.
    released.sort_by_key(|(id, _)| *id);
    released.dedup_by_key(|(id, _)| *id);
    for (id, at) in released {
        sim.cast_off.retain(|c| *c != id);
        if let Some(ship) = sim.ships.iter_mut().find(|s| s.id == id) {
            ship.moored = None;
            ship.position = at;
            ship.spawned = false;
            if port_trace() {
                info!("PORT ship {id} backed out of its berth at {:.0},{:.0}", at.x, at.y);
            }
        }
        sim.towed.remove(&id);
    }
}

/// Cut loose every ship moored at `ports` -- all of them when the port is
/// gone, only those `owner` is at war with when it has changed hands -- and
/// send their fleets on. Returns how many ships.
pub fn cut_loose(sim: &mut WorldSimulation, ports: &[PortId], owner: Option<AiShipType>) -> usize {
    let mut fleets: Vec<u32> = Vec::new();
    let mut n = 0;
    for ship in sim.ships.iter_mut() {
        let Some(at) = ship.moored.filter(|p| ports.contains(p)) else { continue };
        let _ = at;
        if owner.is_some_and(|o| !factions_hostile(ship.faction, o)) {
            continue;
        }
        ship.moored = None;
        ship.spawned = false;
        sim.towed.remove(&ship.id);
        fleets.push(ship.fleet);
        n += 1;
    }
    sim.cast_off.retain(|id| sim.ships.iter().any(|s| s.id == *id && s.moored.is_some()));
    for fleet in sim.fleets.iter_mut().filter(|f| fleets.contains(&f.id)) {
        if let Some(run) = fleet.trade.as_mut() {
            run.dwell = 0.0;
        }
    }
    n
}

/// A trader on screen with nobody leading it -- its lead hauler moored, or
/// shot -- gets a new lead: the member nearest its formation's middle. Only
/// a leader is given the fleet's orders (`run_traffic`), so without one a
/// convoy whose ships came back out of their berths never moved on.
pub fn promote_trader_leaders(
    time: Res<Time>,
    mut next: Local<f32>,
    trading: Res<TradingFleets>,
    mut members: Query<(&mut super::fleet::FleetMember, &AiShipState)>,
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 1.0;
    let mut led: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut best: std::collections::HashMap<u32, f32> = std::collections::HashMap::new();
    for (m, state) in members.iter() {
        if state.is_destroyed || !trading.0.contains(&m.fleet) {
            continue;
        }
        if m.leader {
            led.insert(m.fleet);
        }
        let d = m.slot.length();
        let b = best.entry(m.fleet).or_insert(f32::MAX);
        *b = b.min(d);
    }
    let mut done: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for (mut m, state) in members.iter_mut() {
        if state.is_destroyed || led.contains(&m.fleet) || done.contains(&m.fleet) {
            continue;
        }
        if best.get(&m.fleet).is_some_and(|b| m.slot.length() <= *b) {
            m.leader = true;
            done.insert(m.fleet);
        }
    }
}

/// Let go of moored ships whose time is up and that no station is drawing
/// in a berth (`drawn` says who is): afloat where they are.
pub fn let_go(sim: &mut WorldSimulation, drawn: &std::collections::HashSet<u32>) {
    take_back_released(sim);
    let cast: Vec<u32> = std::mem::take(&mut sim.cast_off);
    for id in cast {
        if drawn.contains(&id) {
            // A station has it, and is backing it out.
            sim.cast_off.push(id);
            continue;
        }
        if let Some(ship) = sim.ships.iter_mut().find(|s| s.id == id && s.moored.is_some()) {
            ship.moored = None;
            ship.spawned = false;
        }
        sim.towed.remove(&id);
    }
}

#[cfg(test)]
mod port_tests {
    use super::*;

    #[test]
    fn a_full_port_turns_a_convoy_away_and_an_empty_one_takes_it() {
        let haven = PortId::Station(0);
        let other = PortId::Station(14);
        assert!(room_at(haven, 0, 3));
        assert!(room_at(haven, 5, 3));
        assert!(!room_at(haven, 6, 3), "over Haven's berths");
        assert!(!room_at(other, 3, 3));
        assert!(!room_at(other, 0, 0), "nothing to moor");
        assert!(port_berths(haven) > port_berths(other));
    }

    /// Haulers moor; their guards, and a hauler with a bounty on it, don't.
    #[test]
    fn only_working_ships_moor() {
        let hauler = crate::ai_ship::roster::roster(AiShipType::FreeHaulersCompact)
            .iter()
            .find(|c| c.role == crate::ai_ship::roster::ClassRole::Civilian)
            .expect("the Compact flies haulers");
        let mut ship = SimulatedShip::patrolling(0, AiShipType::FreeHaulersCompact, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
        ship.class = Some(hauler.key);
        assert!(moors(&ship));
        ship.bounty_id = Some(3);
        assert!(!moors(&ship));
        let guard = crate::ai_ship::roster::roster(AiShipType::TerranHegemony)
            .iter()
            .find(|c| c.role == crate::ai_ship::roster::ClassRole::Line)
            .unwrap();
        let mut g = SimulatedShip::patrolling(0, AiShipType::TerranHegemony, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
        g.class = Some(guard.key);
        assert!(!moors(&g));
    }

    /// Ships cast off: one a station is drawing waits for it to back out;
    /// the rest are afloat at once; and those backed out are afloat where
    /// the station put them.
    #[test]
    fn ships_cast_off_drawn_or_not() {
        let mut sim = WorldSimulation::default();
        for i in 0..3 {
            let mut s = SimulatedShip::patrolling(0, AiShipType::FreeHaulersCompact, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
            s.moored = Some(PortId::Station(0));
            s.position = Vec2::new(i as f32, 0.0);
            sim.add(s);
        }
        let ids: Vec<u32> = sim.ships.iter().map(|s| s.id).collect();
        sim.cast_off = ids.clone();
        let drawn: std::collections::HashSet<u32> = [ids[0]].into_iter().collect();
        let_go(&mut sim, &drawn);
        assert!(sim.ships[0].moored.is_some() && sim.cast_off == vec![ids[0]], "the drawn one was let go before it backed out");
        assert!(sim.ships[1].afloat() && sim.ships[2].afloat());
        // Reported twice (picture and lights), and still listed to cast off.
        sim.released.push((ids[0], Vec2::new(500.0, 40.0)));
        sim.released.push((ids[0], Vec2::new(500.0, 40.0)));
        take_back_released(&mut sim);
        assert!(sim.cast_off.is_empty(), "a ship backed out is still waiting to cast off");
        assert!(sim.ships[0].afloat());
        assert_eq!(sim.ships[0].position, Vec2::new(500.0, 40.0));
    }

    /// A starport lost: every ship moored there cuts loose and its fleet
    /// goes on. A system changing hands: only those the new owner would shoot.
    #[test]
    fn traders_cut_loose_when_a_port_falls() {
        let mut sim = WorldSimulation::default();
        let port = PortId::Starport(4);
        for faction in [AiShipType::MerchantPrinces, AiShipType::FreeHaulersCompact] {
            let fleet = fleet::SimFleet::new(FleetKind::Convoy, faction, 4, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
            let mut s = SimulatedShip::patrolling(4, faction, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
            s.moored = Some(port);
            let id = sim.add_fleet(fleet, vec![s]);
            sim.fleets.iter_mut().find(|f| f.id == id).unwrap().trade = Some(TradeRun { dwell: 60.0, ..default() });
        }
        // The Compact and the Princes are at war: the Princes' new system
        // throws the Compact out and keeps its own.
        assert!(factions_hostile(AiShipType::MerchantPrinces, AiShipType::FreeHaulersCompact));
        assert_eq!(cut_loose(&mut sim, &[port], Some(AiShipType::MerchantPrinces)), 1);
        let compact = sim.ships.iter().find(|s| s.faction == AiShipType::FreeHaulersCompact).unwrap();
        assert!(compact.afloat());
        assert_eq!(sim.fleet(compact.fleet).unwrap().trade.as_ref().unwrap().dwell, 0.0);
        // The port shot down: everyone.
        assert_eq!(cut_loose(&mut sim, &[port], None), 1);
        assert!(sim.ships.iter().all(|s| s.afloat()));
    }

    /// A moored ship saves as moored; an old save, without it, loads afloat.
    #[test]
    fn a_berth_is_saved() {
        let mut sim = WorldSimulation::default();
        let mut s = SimulatedShip::patrolling(0, AiShipType::FreeHaulersCompact, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
        s.moored = Some(PortId::Starport(7));
        sim.add(s);
        let json = serde_json::to_string(&sim.snapshot()).unwrap();
        let back: SavedWorld = serde_json::from_str(&json).unwrap();
        assert_eq!(back.ships[0].moored, Some(PortId::Starport(7)));
        let old = json.replace(",\"moored\":{\"Starport\":7}", "");
        assert!(!old.contains("moored"), "{old}");
        let back: SavedWorld = serde_json::from_str(&old).unwrap();
        assert!(back.ships[0].afloat());
    }
}
