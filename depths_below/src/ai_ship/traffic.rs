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
/// New traders start this far from the station they're bound for.
const ARRIVING_FROM: (f32, f32) = (55_000.0, 110_000.0);

/// Fleets of traders a system keeps going. An empty lane where it's at war
/// with both trading houses.
pub fn trade_quota(system: &StarSystemDef) -> usize {
    if trade_factions(system.faction).is_empty() {
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
    pub radius: f32,
}

impl Stop {
    /// Where to stop, coming from `from`: on the near side, off its hull.
    pub fn berth(&self, from: Vec2) -> Vec2 {
        self.pos + (from - self.pos).normalize_or(Vec2::X) * (self.radius + BERTH_MARGIN)
    }
}

/// Where a ship leaves this system for `to`: out on the bearing to it.
pub fn jump_point(from: &StarSystemDef, to: &StarSystemDef, sign: f32) -> Vec2 {
    from.local_center + (to.galaxy_pos - from.galaxy_pos).normalize_or(Vec2::X) * JUMP_RADIUS * sign
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
    let cargo = trader_cargo(fleet.id);
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
) {
    if crate::demo::skip_ai_ship_spawn() && std::env::var("DEPTHS_TRAFFIC").is_err() {
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
        .map(|s| Stop { name: s.name.clone(), pos: s.pos, radius: s.radius() })
        .collect();
    if let Some((system, at, extent)) = ports.post.filter(|(s, ..)| *s == loaded) {
        let owner = galaxy.systems.iter().find(|s| s.id == system).and_then(|s| s.faction);
        let name = owner.map_or("the starport".to_string(), |f| format!("{} starport", faction_display_name(f)));
        stops.push(Stop { name, pos: at, radius: extent });
    }

    let sim: &mut WorldSimulation = &mut sim;
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

    let mut jumps: Vec<(u32, u32, Vec2)> = Vec::new();
    for fleet in sim.fleets.iter_mut().filter(|f| f.trade.is_some() && active(f.system_id) && manned.contains(&f.id)) {
        let here = fleet.system_id == loaded;
        let is_live = live.contains(&fleet.id);
        let position = fleet.position;
        let Some(run) = fleet.trade.as_mut() else { continue };

        // Stations ride their planets: keep aiming at where it is now.
        if here && run.dwell <= 0.0 && run.leaving.is_none() {
            if let Some(stop) = stops.iter().find(|s| s.name == run.bound_for) {
                fleet.destination = stop.berth(position);
            }
        }
        if run.dwell > 0.0 {
            run.dwell -= TICK_S;
            fleet.velocity = Vec2::ZERO;
            fleet.destination = position;
        } else if position.distance(fleet.destination) <= ARRIVE {
            match run.leaving {
                // Going out of a system is out of your sight: one still on
                // screen thinks better of it and stays another while.
                Some(to) if !is_live => jumps.push((fleet.id, to, position)),
                // Refugees don't turn back: they wait at the jump until
                // nobody's watching, then go.
                Some(_) if run.refugees => fleet.velocity = Vec2::ZERO,
                _ => {
                    run.leaving = None;
                    run.last_stop = std::mem::take(&mut run.bound_for);
                    run.dwell = rng.gen_range(DWELL_S.0..DWELL_S.1);
                    fleet.velocity = Vec2::ZERO;
                }
            }
        }
        // Time to go: another stop, or out.
        if run.dwell <= 0.0 && run.bound_for.is_empty() && run.leaving.is_none() {
            let Some(from) = galaxy.systems.iter().find(|s| s.id == fleet.system_id) else { continue };
            let leave = (rng.gen::<f32>() < LEAVE_CHANCE)
                .then(|| streaming.warm_systems.iter().copied().chain([loaded]).filter(|s| *s != fleet.system_id).collect::<Vec<u32>>())
                .filter(|o| !o.is_empty())
                .map(|o| o[rng.gen_range(0..o.len())])
                .and_then(|to| galaxy.systems.iter().find(|s| s.id == to))
                .filter(|to| !trade_factions(to.faction).is_empty() && trade_factions(to.faction).contains(&fleet.faction));
            if let Some(to) = leave {
                run.leaving = Some(to.id);
                run.bound_for = to.name.clone();
                fleet.destination = jump_point(from, to, 1.0);
            } else if here {
                let Some(stop) = next_stop(&stops, &run.last_stop, &mut rng) else {
                    fleet.destination = from.local_center + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(30_000.0..120_000.0);
                    continue;
                };
                run.bound_for = stop.name.clone();
                fleet.destination = stop.berth(position);
            } else {
                // A neighbouring system's stations aren't there to fly to.
                run.bound_for = from.name.clone();
                fleet.destination = from.local_center + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(30_000.0..120_000.0);
            }
        }
    }

    // Through the jump: out of one system, into the next.
    for (id, to, _) in jumps {
        let Some(fleet) = sim.fleets.iter_mut().find(|f| f.id == id) else { continue };
        let Some((from, dest)) = galaxy.systems.iter().find(|s| s.id == fleet.system_id).zip(galaxy.systems.iter().find(|s| s.id == to)) else { continue };
        let arrive = jump_point(dest, from, 1.0);
        // Not out of nowhere, in front of you.
        if to == loaded && player_pos.is_some_and(|p| p.distance(arrive) < super::simulation::RENDER_DISTANCE * 1.5) {
            continue;
        }
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

    trading.0.clear();
    trading.0.extend(sim.fleets.iter().filter(|f| f.trade.is_some()).map(|f| f.id));

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
    let mut counts: std::collections::HashMap<u32, usize> = Default::default();
    for fleet in sim.fleets.iter().filter(|f| f.trade.is_some() && manned.contains(&f.id)) {
        *counts.entry(fleet.system_id).or_insert(0) += 1;
    }
    let systems: Vec<u32> = streaming.warm_systems.iter().copied().chain([loaded]).collect();
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
            let centre = stop.as_ref().map_or(system.local_center, |s| s.pos);
            let at = centre + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(ARRIVING_FROM.0..ARRIVING_FROM.1);
            if id == loaded && player_pos.is_some_and(|p| p.distance(at) < super::simulation::RENDER_DISTANCE * 1.1) {
                continue;
            }
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
        Stop { name: name.into(), pos: Vec2::new(x, 0.0), radius: 1_000.0 }
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
