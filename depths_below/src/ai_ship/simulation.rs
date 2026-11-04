use bevy::prelude::*;
use rand::Rng;
use crate::components::*;
use crate::building::ModuleRegistry;
use super::components::*;
use super::spawner;

/// Render distance - ships within this range get spawned as real entities.
/// Fights happen at 5-8 km from the hull (ai_ship::movement::fighting_band)
/// between ships kilometres long, and a missile reaches 12 km: at 10,000 a
/// sniper holding its range sat right on the edge of being real. 16,000
/// puts a ship on screen, as a sensor contact (camera::SENSOR_RANGE), well
/// before it's in reach.
pub const RENDER_DISTANCE: f32 = 48_000.0;

/// How many AI blocks (hull cells + modules) may exist as real entities at once.
///
/// A frame-time budget, not a gameplay limit: everything past it stays in the
/// simulation, still moving and still fighting, and materialises as soon as
/// room frees. Nearest to the player first.
///
/// It counted ships (8) when every ship was about a hundred blocks. Classes
/// now run from a 70-block skiff to a 2,300-block boss, so a ship count was
/// either a thin swarm or a frame-rate cliff. 6,000 is about what eight of the
/// old mid-size hulls cost, and holds a whole swarm or a few capitals.
const MAX_LIVE_AI_BLOCKS: usize = 9_000;

/// Most AI blocks brought into being in one frame. Spawning is a whole ship's
/// worth of entities at once, about a millisecond per hundred blocks: since
/// the ships were drawn twice as big a fleet arriving in one frame froze the
/// game for a tenth of a second. Past this it arrives over a few frames, the
/// rest of the fleet first in line (it's already partly on screen).
const SPAWN_BLOCKS_PER_FRAME: usize = 2_500;

/// RENDER_DISTANCE, or DEPTHS_AI_RANGE: a filming switch to make whole fleets
/// real far from the player, for wide shots of a war.
pub fn render_distance() -> f32 {
    static V: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("DEPTHS_AI_RANGE").ok().and_then(|v| v.parse().ok()).unwrap_or(RENDER_DISTANCE))
}

/// DESPAWN_DISTANCE, kept the same distance past render_distance().
fn despawn_distance() -> f32 {
    DESPAWN_DISTANCE + (render_distance() - RENDER_DISTANCE)
}

/// MAX_LIVE_AI_BLOCKS, or DEPTHS_AI_BLOCKS (filming).
fn max_live_ai_blocks() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("DEPTHS_AI_BLOCKS").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_LIVE_AI_BLOCKS))
}
/// Distance at which spawned entities get converted back to simulation. Was
/// 3500 — well inside the new ~8,160 max standoff distance, so a ship
/// holding a long-range fight would get yanked back to an abstract
/// simulated point (functionally "despawn") the moment it backed off to
/// actually use its weapon's range. 14,000 clears the max standoff with
/// real margin and stays under camera::cull_range (16,000) so a ship
/// doesn't visually vanish before it's converted back to simulation either.
const DESPAWN_DISTANCE: f32 = 60_000.0;

/// Initialize the world simulation with all factions in their territories
pub fn init_world_simulation(
    mut sim: ResMut<WorldSimulation>,
    galaxy_map: Res<crate::celestial::resources::GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
) {
    if sim.initialized {
        return;
    }
    sim.initialized = true;

    // DEPTHS_MOVETEST=1: bare movement sandbox. Normally zero AI ships, but
    // DEPTHS_MOVETEST_ENEMY=1 adds exactly one non-shooting dummy (see
    // ai_weapon_fire_system) close enough to immediately engage, so the
    // standoff/orbit "keep distance" behavior in ai_ship_movement_system can
    // be watched in isolation without the rest of the world simulation.
    // Broken Choir, not Terran Hegemony: Terran Hegemony is a ~10x16-cell battleship with a
    // full hull shell around every module — with the 45-unit "nearest
    // block" hit radius, shots just kept landing on whatever hull was
    // closest across that huge surface and never punched through to a
    // module, which read as "modules are invincible". Broken Choir is much
    // smaller (~6x10) while still holding a real standoff distance
    // (unlike Recursive Kingdom, which rams point-blank).
    if crate::demo::skip_ai_ship_spawn() {
        if std::env::var("DEPTHS_MOVETEST_ENEMY").ok().as_deref() == Some("1") {
            // DEPTHS_MOVETEST_ENEMY_FACTION overrides the dummy's faction for
            // behavior-tree testing (e.g. "The Silence", "Terran Hegemony") — defaults
            // to Broken Choir for the original damage-model testing use case.
            let faction = std::env::var("DEPTHS_MOVETEST_ENEMY_FACTION")
                .ok()
                .and_then(|f| faction_named(&f))
                .unwrap_or(AiShipType::BrokenChoir);
            let mut dummy = SimulatedShip::patrolling(
                0, faction, Vec2::new(500.0, 0.0), Vec2::new(500.0, 0.0), 2000.0, 0.0,
            );
            // DEPTHS_MOVETEST_ENEMY_CLASS=<key> picks a roster class (ai_ship::roster),
            // e.g. "bulwark"; unset flies the faction's original design.
            if let Ok(key) = std::env::var("DEPTHS_MOVETEST_ENEMY_CLASS") {
                dummy.class = super::roster::find(faction, &key).map(|c| c.key);
                if dummy.class.is_none() {
                    warn!("MOVETEST: {:?} has no class {:?}", faction, key);
                }
                // Classes run up to a hundred cells long; at 500 units one
                // would be born inside the player's own hull and be thrown
                // out of frame by the collision push.
                dummy.position = Vec2::new(3200.0, 0.0);
                dummy.home_zone = dummy.position;
                dummy.destination = dummy.position;
            }
            // A design file (DEPTHS_MOVETEST_ENEMY_DESIGN, ai_ship::spawner)
            // can be a whole warship: at 500 units it would be born inside
            // the player's hull, the same as a class.
            if std::env::var("DEPTHS_MOVETEST_ENEMY_DESIGN").is_ok() {
                dummy.position = Vec2::new(3200.0, 0.0);
                dummy.home_zone = dummy.position;
                dummy.destination = dummy.position;
            }
            let shoots = std::env::var("DEPTHS_MOVETEST_ENEMY_SHOOTS").ok().as_deref() == Some("1");
            info!(
                "MOVETEST: one {} {:?} {:?} dummy spawned at {:?}",
                if shoots { "shooting" } else { "non-shooting" },
                faction,
                dummy.class,
                dummy.position
            );
            sim.add(dummy);
        }

        // DEPTHS_MOVETEST_FLEET=<Faction>:<Kind> (Patrol, BattleGroup, Convoy,
        // StationGuard, Raiders): one whole fleet of that kind, 9 km off and
        // crossing ahead of you, for watching formations and fleet fights.
        // Several, separated by ';', alternate sides and meet head-on in the
        // middle -- two enemies make a battle to film.
        if let Ok(specs) = std::env::var("DEPTHS_MOVETEST_FLEET") {
            use super::fleet::FleetKind;
            for (i, spec) in specs.split(';').filter(|s| !s.trim().is_empty()).enumerate() {
                // "<Faction>:<Kind>@dx,dy[,deg][!]": placed at an offset from
                // the meeting point, facing deg, and with "!" holding there --
                // battle lines and ships idling round a station, for filming.
                let spec = spec.trim();
                let hold = spec.ends_with('!');
                let spec = spec.trim_end_matches('!');
                // "@dx,dy>tx,ty": fly from one offset to another and stop there.
                let (spec, travel_to) = match spec.split_once('>') {
                    Some((a, b)) => {
                        let n: Vec<f32> = b.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                        (a, (n.len() >= 2).then(|| Vec2::new(n[0], n[1])))
                    }
                    None => (spec, None),
                };
                let (spec, place) = match spec.split_once('@') {
                    Some((a, b)) => {
                        let n: Vec<f32> = b.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                        (a, (n.len() >= 2).then(|| (Vec2::new(n[0], n[1]), n.get(2).map(|d| d.to_radians()))))
                    }
                    None => (spec, None),
                };
                let (f, k) = spec.split_once(':').unwrap_or((spec, "Patrol"));
                // A class key in place of a fleet kind ("MerchantPrinces:hauler"):
                // that one ship, alone.
                let solo: Option<&'static str> = match k.trim() {
                    "BattleGroup" | "Convoy" | "StationGuard" | "Raiders" | "Patrol" => None,
                    key => Some(Box::leak(key.to_string().into_boxed_str())),
                };
                let kind = match k.trim() {
                    "BattleGroup" => FleetKind::BattleGroup,
                    "Convoy" => FleetKind::Convoy,
                    "StationGuard" => FleetKind::StationGuard,
                    "Raiders" => FleetKind::Raiders,
                    _ => FleetKind::Patrol,
                };
                let faction = faction_named(f.trim()).unwrap_or(AiShipType::TerranHegemony);
                // DEPTHS_FLEET_SEED=<n>: the same ships every run, so two test
                // fights can be compared at all.
                use rand::SeedableRng;
                let mut rng = match std::env::var("DEPTHS_FLEET_SEED").ok().and_then(|v| v.parse::<u64>().ok()) {
                    Some(seed) => rand::rngs::StdRng::seed_from_u64(seed + i as u64),
                    None => rand::rngs::StdRng::from_entropy(),
                };
                let mut held = super::fleet::Held::new();
                // Even: from the east heading west (the original single fleet);
                // odd: from the west heading east, a little offset.
                // DEPTHS_MOVETEST_FLEET_AT=<x>,<y>: meet there instead, starting
                // close enough to be at it within seconds -- for filming.
                let meet = std::env::var("DEPTHS_MOVETEST_FLEET_AT").ok()
                    .and_then(|v| v.split_once(',').and_then(|(x, y)| Some(Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))));
                let (centre, reach) = meet.map_or((Vec2::new(0.0, 4_000.0), 9_000.0), |m| (m, 3_200.0));
                let east = i % 2 == 0;
                let y = centre.y + (i / 2) as f32 * 2_500.0 + if east { 0.0 } else { 600.0 };
                let (start, heading, far) = if east {
                    (Vec2::new(centre.x + reach, y), std::f32::consts::PI, Vec2::new(centre.x - 30_000.0, y))
                } else {
                    (Vec2::new(centre.x - reach, y), 0.0, Vec2::new(centre.x + 30_000.0, y))
                };
                let (start, heading, far) = match place {
                    Some((offset, deg)) => {
                        let at = centre + offset;
                        let h = deg.unwrap_or(heading);
                        (at, h, if hold { at } else { at + Vec2::from_angle(h) * 30_000.0 })
                    }
                    None => (start, heading, far),
                };
                let (heading, far, hold) = match travel_to {
                    Some(to) => {
                        let to = centre + to;
                        let d = to - start;
                        (d.y.atan2(d.x), to, true)
                    }
                    None => (heading, far, hold),
                };
                let classes = match solo {
                    Some(key) => Some(vec![(faction, Some(key))]),
                    None => super::fleet::compose(faction, kind, &mut rng, &mut held),
                };
                match classes {
                    Some(classes) => {
                        let (mut fleet, members) = super::fleet::new_fleet(
                            faction, kind, &classes, 0, start, Vec2::ZERO, 40_000.0, &mut rng,
                        );
                        fleet.heading = heading;
                        fleet.destination = far;
                        fleet.route = if travel_to.is_some() { vec![far] } else if hold { vec![start] } else { vec![far, start] };
                        info!("MOVETEST: {:?} {:?} fleet of {}: {:?}", faction, kind, members.len(), classes);
                        sim.add_fleet(fleet, members);
                    }
                    None => warn!("MOVETEST: {:?} can't crew a {:?}", faction, kind),
                }
            }
        }

        // TEMP [AI_VS_AI_DIAGNOSTIC]: spawns a tight cluster of guaranteed
        // combat-capable ships (alternating Terran Hegemony / Recursive Kingdom / Broken Choir
        // / Gilded Throne, all "attack anything in range" factions per
        // ai_brain.rs) close enough together to be within engage range from
        // the start, for headlessly verifying AI-vs-AI combat actually
        // lands hits. Remove once the diagnosis is confirmed.
        if let Ok(count) = std::env::var("DEPTHS_AI_VS_AI_TEST").unwrap_or_default().parse::<usize>() {
            let factions = [AiShipType::TerranHegemony, AiShipType::RecursiveKingdom, AiShipType::BrokenChoir, AiShipType::GildedThrone];
            // DEPTHS_AI_VS_AI_AT=<x>,<y>: hold the fight away from you, to film it.
            let centre = std::env::var("DEPTHS_AI_VS_AI_AT").ok()
                .and_then(|v| v.split_once(',').and_then(|(x, y)| Some(Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))))
                .unwrap_or(Vec2::ZERO);
            for i in 0..count {
                let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
                let pos = centre + Vec2::new(angle.cos(), angle.sin()) * 400.0;
                sim.ships.push(SimulatedShip::patrolling(
                    0, factions[i % factions.len()], pos, pos, 2000.0, 0.0,
                ));
            }
            info!("AI_VS_AI_TEST: spawned {} combat-capable ships in a 400u cluster", count);
        }
        return;
    }

    // System-scoped faction population: each star system now carries its
    // own fixed faction + danger tier (StarSystemDef.faction/danger_tier,
    // assigned once at galaxy generation — celestial/galaxy.rs) instead of
    // the old model of all 10 territories jittered around a single shared
    // origin every game. Haven (system 0) has faction: None — always safe,
    // nothing spawns for it. Its initial Warm neighbors (set by
    // generate_galaxy_on_enter) get populated too, so they're already
    // "alive" the moment the game starts, not just on first visit.
    //
    // This also removes the old "ambient" (3 ships) and "roaming wanderer"
    // (24 ships) filler groups that used to surround the origin regardless
    // of faction — they existed to make the immediate spawn area feel
    // populated, which directly conflicted with Haven now being a
    // deliberately safe hub. The systems reachable by warp are where the
    // populated universe lives now.
    //
    // The loaded system, not system 0: those are the same on a new run, but
    // a save made anywhere else used to load into a system nobody populated
    // (it was neither Haven nor one of its own Warm neighbours) -- empty
    // until you jumped out and back. None is uncharted space: no one lives
    // there, only the Warm neighbours around it.
    if let Some(cur) = streaming.loaded_system {
        ensure_system_population(&mut sim, &galaxy_map, cur);
    }
    for warm_id in &streaming.warm_systems {
        ensure_system_population(&mut sim, &galaxy_map, *warm_id);
    }

    info!("World simulation initialized with {} AI vessels", sim.ships.len());
}

/// A faction by its type name, as the test switches spell it.
fn faction_named(name: &str) -> Option<AiShipType> {
    Some(match name {
        "StellarPreserve" => AiShipType::StellarPreserve,
        "SynthesisCollective" => AiShipType::SynthesisCollective,
        "CorpseStars" => AiShipType::CorpseStars,
        "TheSilence" => AiShipType::TheSilence,
        "TerranHegemony" => AiShipType::TerranHegemony,
        "GildedThrone" => AiShipType::GildedThrone,
        "RecursiveKingdom" => AiShipType::RecursiveKingdom,
        "EternalHegemony" => AiShipType::EternalHegemony,
        "Shepherd" => AiShipType::Shepherd,
        "BrokenChoir" => AiShipType::BrokenChoir,
        "MerchantPrinces" => AiShipType::MerchantPrinces,
        "FreeHaulersCompact" => AiShipType::FreeHaulersCompact,
        _ => return None,
    })
}

/// Spawns a system's faction population the FIRST time it's ever loaded
/// (Hot or Warm) — persistence-safe: if any SimulatedShip already carries
/// this system_id (from an earlier visit, however depleted by combat since
/// then), does nothing and leaves it exactly as it is. Called from
/// celestial::warp::execute_warp_jump whenever a system newly becomes Hot
/// or joins the Warm neighbor set.
pub fn ensure_system_population(sim: &mut WorldSimulation, galaxy_map: &crate::celestial::resources::GalaxyMap, system_id: u32) {
    if sim.ships.iter().any(|s| s.system_id == system_id) {
        return;
    }
    if let Some(system) = galaxy_map.systems.iter().find(|s| s.id == system_id) {
        // Nothing new arrives in the dark (narrative::front): what was there
        // when it was taken stays where it stopped.
        if crate::narrative::front::is_taken(system) {
            return;
        }
        let borders = crate::celestial::territory::border_factions(galaxy_map, system_id);
        spawn_system_faction_population(sim, system);
        spawn_visitors(sim, system, &borders);
    }
}

/// Spawns one star system's faction population — ship_count/radius pulled
/// from that faction's existing territory template (faction_territories()),
/// centered on the system's faction cluster point (offset clear of the
/// star itself — see celestial::galaxy::faction_cluster_center) instead of
/// a jittered-origin position. No-ops for a safe system (faction: None,
/// e.g. Haven).
fn spawn_system_faction_population(sim: &mut WorldSimulation, system: &crate::celestial::resources::StarSystemDef) {
    let Some(faction) = system.faction else { return };
    let Some(template) = faction_territories().into_iter().find(|t| t.faction == faction) else { return };
    let mut rng = rand::thread_rng();

    // How many, and arranged how.
    //
    // A system's population is a set of FLEETS (ai_ship::fleet): patrols,
    // battle groups, convoys, a station guard, raiders -- whatever the
    // faction's lore fields. It used to be loners, pairs and packs of random
    // classes, so a capital flew alone as often as not and a hauler fought
    // like a warship.
    //
    // Fleets spread over several holdings (see `faction_holdings`), the
    // cluster point keeping the largest share, each fleet in its own distance
    // band round its holding: a single clump was reported as "only in a
    // single pack and not spread around".
    let holdings = faction_holdings(system, &mut rng);
    // The danger the faction itself brings (a haunted system's danger is the
    // boss's, and the boss brings its own ships).
    let count = system_ship_count(template.ship_count, super::components::faction_power(faction));
    // A capital is where a faction keeps its strength.
    let count = if system.capital { (count as f32 * CAPITAL_SHARE).round() as usize } else { count };
    // How many of each capped role this system already holds.
    let mut held = super::fleet::Held::new();
    let has_station = crate::diplomacy::stations::station_plan(faction).is_some();
    let mut guarded = false;
    // Fleets placed so far, (centre, reach): a new one is set down clear of
    // them, never through another's formation.
    let mut placed_fleets: Vec<(Vec2, f32)> = Vec::new();
    // Fixed shares, not a roll per fleet: one holding mustn't take them all.
    for (h, (&home, quota)) in holdings.iter().zip(holding_quotas(count, holdings.len())).enumerate() {
        let mut placed = 0usize;
        while placed < quota {
            use super::fleet::FleetKind;
            // One guard per station; a convoy needs somewhere to run to.
            let Some((kind, classes)) = super::fleet::roll_fleet(faction, &mut rng, &mut held, |k| match k {
                FleetKind::StationGuard => has_station && !guarded,
                FleetKind::Convoy => holdings.len() > 1,
                _ => true,
            }) else {
                break;
            };

            let reach = classes
                .iter()
                .map(|&(f, c)| super::spawner::design_extent(f, c))
                .fold(0.0_f32, f32::max)
                * (classes.len() as f32 + 1.0)
                + 1_000.0;
            // Three bands rather than a uniform draw: uniform-in-a-disc actually
            // concentrates toward the rim, which is the opposite of varied.
            // A few tries for a spot clear of every fleet already placed;
            // then the field widens a little each try. Ships drawn twice as
            // big no longer always fit the old field, and the last try used
            // to be taken clear or not -- fleets set down through each other.
            let mut angle = 0.0;
            let mut anchor = home;
            for attempt in 0u32..60 {
                let band = match rng.gen_range(0..3) {
                    0 => rng.gen_range(0.10..0.30),
                    1 => rng.gen_range(0.35..0.60),
                    _ => rng.gen_range(0.65..0.95),
                };
                let widen = 1.0 + attempt.saturating_sub(12) as f32 * 0.05;
                angle = rng.gen_range(0.0..std::f32::consts::TAU);
                anchor = home + Vec2::new(angle.cos(), angle.sin()) * (template.radius * band * widen);
                if placed_fleets.iter().all(|&(c, r)| c.distance(anchor) > r + reach) {
                    break;
                }
            }
            placed_fleets.push((anchor, reach));
            let (mut fleet, members) = super::fleet::new_fleet(
                faction, kind, &classes, system.id, anchor, home, template.radius, &mut rng,
            );
            match kind {
                FleetKind::Convoy => {
                    // Out to another holding and back.
                    let other = holdings[(h + rng.gen_range(1..holdings.len())) % holdings.len()];
                    fleet.route = vec![home, other];
                    fleet.leg = 1;
                    fleet.destination = other;
                    fleet.home_zone = (home + other) * 0.5;
                    fleet.patrol_radius = home.distance(other) * 0.5 + template.radius;
                }
                FleetKind::StationGuard => {
                    // Until the station's place is known (it rides a planet;
                    // see tick_world_simulation), it holds the cluster point.
                    guarded = true;
                    fleet.home_zone = holdings[0];
                    fleet.position = holdings[0] + Vec2::from_angle(angle) * 3_000.0;
                    fleet.patrol_radius = GUARD_RADIUS;
                    fleet.destination = patrol_waypoint(&mut rng, fleet.home_zone, fleet.patrol_radius);
                }
                FleetKind::Raiders => {
                    // Ranging wide, after whatever's worth robbing.
                    fleet.patrol_radius = template.radius * 2.0;
                    fleet.destination = patrol_waypoint(&mut rng, home, fleet.patrol_radius);
                }
                _ => {
                    fleet.destination = patrol_waypoint(&mut rng, home, template.radius);
                }
            }
            placed += members.len();
            sim.add_fleet(fleet, members);
        }
    }

    // A capital keeps a home fleet: a battle group that holds by its starport
    // and never leaves (a trading faction's is its ally's).
    if system.capital && has_station {
        let flier = faction.ally().filter(|_| faction.is_trading()).unwrap_or(faction);
        let mut home_held = super::fleet::Held::new();
        if let Some(hulls) = super::fleet::compose(flier, super::fleet::FleetKind::BattleGroup, &mut rng, &mut home_held) {
            let at = holdings[0] + Vec2::new(0.0, 6_000.0);
            let (mut fleet, members) = super::fleet::new_fleet(
                faction, super::fleet::FleetKind::StationGuard, &hulls, system.id, at, holdings[0], HOME_FLEET_RADIUS, &mut rng,
            );
            fleet.destination = patrol_waypoint(&mut rng, holdings[0], HOME_FLEET_RADIUS);
            sim.add_fleet(fleet, members);
        }
    }
}

/// How far a capital's home fleet ranges round its starport.
const HOME_FLEET_RADIUS: f32 = 14_000.0;

/// How far a station guard ranges from its post, past the station's hull.
const GUARD_RADIUS: f32 = 8_000.0;

/// How much busier a faction's capital is than its other systems.
const CAPITAL_SHARE: f32 = 1.4;

/// The fleets a system holds that aren't its owner's: patrols from the
/// factions it borders (neighbours meet at border systems), and the boss
/// that haunts it, if one does.
fn spawn_visitors(sim: &mut WorldSimulation, system: &crate::celestial::resources::StarSystemDef, borders: &[AiShipType]) {
    use super::fleet::FleetKind;
    let mut rng = rand::thread_rng();
    // Out on the open bearings, away from the owner's cluster point and its
    // stations: the edge of the system, where a border is.
    let edge = |rng: &mut rand::rngs::ThreadRng| {
        system.local_center + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(OPEN_HOLDING_RANGE)
    };
    let stations: Vec<Vec2> = crate::world::home_base::station_sites(system).iter().map(|s| s.pos).collect();
    for &neighbour in borders {
        let Some(template) = faction_territories().into_iter().find(|t| t.faction == neighbour) else { continue };
        let mut held = super::fleet::Held::new();
        for _ in 0..rng.gen_range(1..=2) {
            let Some((kind, hulls)) = super::fleet::roll_fleet(neighbour, &mut rng, &mut held, |k| {
                matches!(k, FleetKind::Patrol | FleetKind::Raiders | FleetKind::Convoy)
            }) else {
                break;
            };
            let home = clear_of_stations(edge(&mut rng), &stations);
            let (mut fleet, members) =
                super::fleet::new_fleet(neighbour, kind, &hulls, system.id, home, home, template.radius * 1.5, &mut rng);
            if kind == FleetKind::Convoy {
                // A convoy crossing the border: through the system and out.
                let far = clear_of_stations(edge(&mut rng), &stations);
                fleet.route = vec![home, far];
                fleet.leg = 1;
                fleet.home_zone = (home + far) * 0.5;
                fleet.patrol_radius = home.distance(far) * 0.5 + template.radius;
                fleet.destination = far;
            } else {
                fleet.destination = patrol_waypoint(&mut rng, home, fleet.patrol_radius);
            }
            sim.add_fleet(fleet, members);
        }
    }
    if let Some(boss) = system.haunt {
        // The boss's battle group, ranging the whole system, and a patrol of
        // its own.
        for kind in [FleetKind::BattleGroup, FleetKind::Patrol] {
            let mut held = super::fleet::Held::new();
            let Some(hulls) = super::fleet::compose(boss, kind, &mut rng, &mut held) else { continue };
            let home = clear_of_stations(edge(&mut rng), &stations);
            let (mut fleet, members) =
                super::fleet::new_fleet(boss, kind, &hulls, system.id, home, system.local_center, OPEN_HOLDING_RANGE.end, &mut rng);
            fleet.destination = patrol_waypoint(&mut rng, system.local_center, fleet.patrol_radius);
            sim.add_fleet(fleet, members);
        }
    }
}

/// Share of a system's ships kept at the faction's cluster point; the rest
/// spread evenly over its other holdings.
const CLUSTER_SHARE: f32 = 0.35;

/// How many of `count` ships each of `holdings` keeps: `CLUSTER_SHARE` at
/// the first (the cluster point), the rest evenly over the others.
fn holding_quotas(count: usize, holdings: usize) -> Vec<usize> {
    if holdings <= 1 {
        return vec![count];
    }
    let cluster = ((count as f32 * CLUSTER_SHARE).round() as usize).clamp(1, count);
    let rest = count - cluster;
    let others = holdings - 1;
    std::iter::once(cluster)
        .chain((0..others).map(|k| rest / others + usize::from(k < rest % others)))
        .collect()
}

/// How far from each station its picket holds: out of sight of the berth a
/// jump arrives at (enemy edge markers reach 25 km), near enough that traffic
/// round a station is something you meet.
const STATION_PICKET: std::ops::Range<f32> = 30_000.0..55_000.0;

/// Holdings out on open bearings, and how far from the star. Past the largest
/// star (150 km radius) with room to spare.
const OPEN_HOLDINGS: usize = 3;
const OPEN_HOLDING_RANGE: std::ops::Range<f32> = 250_000.0..450_000.0;

/// Where a system's faction keeps its ships: its cluster point first, then a
/// picket off each station, then patrols on open bearings. None closer than
/// `STATION_PICKET.start` to any station -- the open bearings and the
/// cluster point both fall at station-like distances from the star.
fn faction_holdings(system: &crate::celestial::resources::StarSystemDef, rng: &mut impl Rng) -> Vec<Vec2> {
    let stations: Vec<Vec2> = crate::world::home_base::station_sites(system)
        .iter()
        .map(|site| site.pos)
        .collect();
    let mut holdings = vec![crate::celestial::galaxy::faction_cluster_center(system)];
    for &station in &stations {
        let bearing = rng.gen_range(0.0..std::f32::consts::TAU);
        holdings.push(station + Vec2::from_angle(bearing) * rng.gen_range(STATION_PICKET));
    }
    for _ in 0..OPEN_HOLDINGS {
        let bearing = rng.gen_range(0.0..std::f32::consts::TAU);
        holdings.push(system.local_center + Vec2::from_angle(bearing) * rng.gen_range(OPEN_HOLDING_RANGE));
    }
    holdings.into_iter().map(|h| clear_of_stations(h, &stations)).collect()
}

/// `pos`, pushed straight out from any station it's within
/// `STATION_PICKET.start` of.
fn clear_of_stations(mut pos: Vec2, stations: &[Vec2]) -> Vec2 {
    // A few passes: a push off one station can, rarely, land near the other.
    for _ in 0..4 {
        let Some(&near) = stations.iter().find(|s| s.distance(pos) < STATION_PICKET.start) else {
            break;
        };
        pos = near + (pos - near).normalize_or(Vec2::X) * STATION_PICKET.start;
    }
    pos
}

/// How many hulls a system's faction keeps on station.
///
/// The template count is the shallow-system baseline; `danger_tier` is the
/// authored near-weak/far-strong curve, so a deep hostile system is busier
/// than a border one instead of every system fielding the same handful.
///
/// Roughly double what it was (2.2x, 6-40): spread over a system's holdings,
/// that left a whole system with seven or eight hostiles on the map, and the
/// playtester wanted more. Only MAX_LIVE_AI_BLOCKS worth are ever full entities at
/// once, so the rest cost a position update each.
pub fn system_ship_count(template_count: usize, danger_tier: f32) -> usize {
    let scale = 1.0 + (danger_tier / 100.0).clamp(0.0, 2.0);
    ((template_count as f32 * 4.0 * scale).round() as usize).clamp(12, 72)
}

/// Close enough to call it arrived, and to pick the next leg.
const ARRIVE_RADIUS: f32 = 450.0;

/// Where a ship goes next.
///
/// Two in five legs head for something that matters — a station, a berth,
/// whatever the loaded system put on the board — approached to a loose
/// standoff rather than parked on top of it. The rest are ordinary patrol
/// legs inside the territory. The mix is the point: traffic that only ever
/// visited stations would read as a conveyor belt, and traffic that never did
/// is the random walk this replaced.
fn next_waypoint(rng: &mut impl Rng, home_zone: Vec2, patrol_radius: f32, anchors: &[(Vec2, f32)]) -> Vec2 {
    if !anchors.is_empty() && rng.gen::<f32>() < 0.4 {
        let (anchor, structure) = anchors[rng.gen_range(0..anchors.len())];
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let standoff = structure + rng.gen_range(600.0..2200.0);
        let want = anchor + Vec2::new(angle.cos(), angle.sin()) * standoff;
        // Only if it is somewhere this ship is allowed to be.
        if want.distance(home_zone) <= patrol_radius {
            return want;
        }
    }
    patrol_waypoint(rng, home_zone, patrol_radius)
}

/// Where a fleet goes next: a convoy's next stop, a guard's next leg round
/// its post, anyone else's next patrol leg.
fn next_fleet_waypoint(rng: &mut impl Rng, fleet: &mut super::fleet::SimFleet, anchors: &[(Vec2, f32)]) -> Vec2 {
    use super::fleet::FleetKind;
    match fleet.kind {
        FleetKind::Convoy if !fleet.route.is_empty() => {
            fleet.leg = (fleet.leg + 1) % fleet.route.len();
            fleet.route[fleet.leg]
        }
        FleetKind::StationGuard => patrol_waypoint(rng, fleet.home_zone, fleet.patrol_radius),
        _ => next_waypoint(rng, fleet.home_zone, fleet.patrol_radius, anchors),
    }
}

/// A point worth flying to inside a territory, for a ship with no better idea.
fn patrol_waypoint(rng: &mut impl Rng, center: Vec2, radius: f32) -> Vec2 {
    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let dist = rng.gen_range(0.15..0.95) * radius;
    center + Vec2::new(angle.cos(), angle.sin()) * dist
}


/// Tick the off-screen simulation: move ships, resolve encounters. Only
/// ships in the Hot (loaded) or Warm (nearest-neighbor) systems actually
/// tick — Cold systems' ships stay exactly where they are, frozen, until
/// something brings that system back into range. This is deliberately the
/// cheapest possible "state preserved while you're away" mechanism (nothing
/// to snapshot or restore) and fixes a real pre-existing perf issue: the
/// Phase 2 pairwise combat check below is O(n^2) over however many ships
/// are active — bounding that to Hot+Warm keeps it small regardless of how
/// large the galaxy grows, instead of silently scaling with total ship
/// count across every system that's ever existed.
pub fn tick_world_simulation(
    time: Res<Time>,
    mut sim: ResMut<WorldSimulation>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    stations: Res<crate::world::home_base::SystemStations>,
    faction_stations: Res<crate::diplomacy::stations::FactionStations>,
    player: Query<&Transform, With<Ship>>,
) {
    sim.tick_timer.tick(time.delta());
    if !sim.tick_timer.just_finished() {
        return;
    }

    let dt = sim.tick_timer.duration().as_secs_f32();
    let mut rng = rand::thread_rng();
    let player_pos = player.single().ok().map(|t| t.translation.truncate());

    // What is worth being near, in the system the player is actually in.
    // Patrols and raiders both gravitate to the same places a player does,
    // which is what makes the traffic read as purposeful rather than as
    // Brownian motion over an empty map.
    // (centre, structure radius): waypoints are taken from the edge, so
    // traffic loiters around a station rather than inside its hull.
    let anchors: Vec<(Vec2, f32)> = stations.sites.iter().map(|s| (s.pos, s.radius())).collect();

    let is_active_system = |system_id: u32| {
        streaming.loaded_system == Some(system_id) || streaming.warm_systems.contains(&system_id)
    };

    let sim: &mut WorldSimulation = &mut sim;
    let fleet_ids: std::collections::HashSet<u32> = sim.fleets.iter().map(|f| f.id).collect();
    // A fleet with a member on screen is flown by the real ships, not here.
    let live_fleets: std::collections::HashSet<u32> =
        sim.ships.iter().filter(|s| s.spawned && s.fleet != 0).map(|s| s.fleet).collect();
    let manned_fleets: std::collections::HashSet<u32> = sim
        .ships
        .iter()
        .filter(|s| s.fleet != 0 && s.behavior != SimBehavior::Dead)
        .map(|s| s.fleet)
        .collect();

    // Phase 1: Move off-screen ships that fly alone.
    for ship in sim.ships.iter_mut() {
        if ship.spawned || ship.behavior == SimBehavior::Dead || !is_active_system(ship.system_id) {
            continue;
        }
        if fleet_ids.contains(&ship.fleet) {
            continue;
        }

        // Steer for the destination, arrive, pick another.
        //
        // This was a random walk: a fixed 30 u/s shoved by a random turn on
        // one tick in ten. Net displacement over a minute was close to zero,
        // which is why the map read as dots that "barely move". A ship now
        // holds a course for somewhere, and the somewhere is chosen below.
        let to_dest = ship.destination - ship.position;
        let remaining = to_dest.length();
        if remaining <= ARRIVE_RADIUS {
            ship.destination = next_waypoint(&mut rng, ship.home_zone, ship.patrol_radius, &anchors);
        } else {
            // Turn toward the bearing rather than snapping to it, so a course
            // change reads as a turn.
            let want = to_dest / remaining * ship.cruise;
            ship.velocity = (ship.velocity * 0.75 + want * 0.25).clamp_length_max(ship.cruise);
        }

        ship.position += ship.velocity * dt;
        ship.fuel = (ship.fuel - 0.001 * dt).max(0.0);

        // The leash still applies: a destination outside the territory, or a
        // ship shoved out of it by combat, gets pulled back.
        if ship.position.distance(ship.home_zone) > ship.patrol_radius {
            ship.destination = patrol_waypoint(&mut rng, ship.home_zone, ship.patrol_radius * 0.6);
        }

        // Fuel exhaustion
        if ship.fuel <= 0.0 {
            ship.velocity *= 0.5;
        }
    }

    // Phase 1b: Move fleets, as one point; the members ride their slots.
    // The loaded system's station rides a planet, so its guard and its
    // convoys are pointed at wherever it is now.
    let post = faction_stations.post;
    for fleet in sim.fleets.iter_mut() {
        if !is_active_system(fleet.system_id) || live_fleets.contains(&fleet.id) || !manned_fleets.contains(&fleet.id) {
            continue;
        }
        use super::fleet::FleetKind;
        if let Some((_, at, extent)) = post.filter(|p| p.0 == fleet.system_id) {
            match fleet.kind {
                FleetKind::StationGuard => {
                    fleet.home_zone = at;
                    // The larger of the two: a capital's home fleet ranges wider.
                    fleet.patrol_radius = fleet.patrol_radius.max(extent + GUARD_RADIUS);
                    // Posted before anyone knew where the station would be:
                    // take station there, out of sight, rather than crossing
                    // half a system to get to it.
                    let unseen = |p: Vec2| player_pos.map_or(true, |pp| pp.distance(p) > render_distance() * 1.5);
                    let post_at = at + Vec2::from_angle(fleet.heading) * (extent + 3_000.0);
                    if fleet.position.distance(at) > 30_000.0 && unseen(fleet.position) && unseen(post_at) {
                        fleet.position = post_at;
                        fleet.destination = patrol_waypoint(&mut rng, at, fleet.patrol_radius);
                    }
                }
                FleetKind::Convoy if fleet.route.len() >= 2 => {
                    // The near end of the run is the station's berth, off its hull.
                    let berth = at + (fleet.route[1] - at).normalize_or(Vec2::X) * (extent + 4_000.0);
                    if fleet.leg == 0 {
                        fleet.destination = berth;
                    }
                    fleet.route[0] = berth;
                }
                _ => {}
            }
        }

        let to_dest = fleet.destination - fleet.position;
        let remaining = to_dest.length();
        let trading = fleet.trade.is_some();
        if trading && remaining <= ARRIVE_RADIUS {
            // ai_ship::traffic decides where a trader goes from here.
            fleet.velocity = Vec2::ZERO;
        } else if remaining <= ARRIVE_RADIUS {
            fleet.destination = next_fleet_waypoint(&mut rng, fleet, &anchors);
        } else {
            let want = to_dest / remaining * fleet.cruise;
            fleet.velocity = (fleet.velocity * 0.75 + want * 0.25).clamp_length_max(fleet.cruise);
        }
        fleet.position += fleet.velocity * dt;
        if fleet.velocity.length() > 1.0 {
            fleet.heading = fleet.velocity.to_angle();
        }
        if !trading && fleet.position.distance(fleet.home_zone) > fleet.patrol_radius {
            fleet.destination = match fleet.kind {
                // A convoy strayed off its run heads for its next stop.
                FleetKind::Convoy if !fleet.route.is_empty() => fleet.route[fleet.leg],
                _ => patrol_waypoint(&mut rng, fleet.home_zone, fleet.patrol_radius * 0.6),
            };
        }
    }
    for ship in sim.ships.iter_mut() {
        if ship.spawned || ship.behavior == SimBehavior::Dead || ship.fleet == 0 {
            continue;
        }
        if let Some(fleet) = sim.fleets.iter().find(|f| f.id == ship.fleet) {
            if live_fleets.contains(&fleet.id) {
                continue;
            }
            ship.position = fleet.slot_position(ship.slot);
            ship.velocity = fleet.velocity;
            ship.destination = fleet.destination;
        }
    }

    // Phase 2: Off-screen fights, weighed by each side's strength.
    super::fleet::resolve_battles(&mut sim.ships, &is_active_system, dt);

    // Phase 3: The dead come back after a while: a lone ship by itself, a
    // fleet only once all of it is gone. A fleet that lost two of five
    // stays three strong; that's damage that sticks.
    for ship in sim.ships.iter_mut() {
        if ship.behavior != SimBehavior::Dead {
            continue;
        }
        // The countdown (fuel doubles as the timer).
        ship.fuel -= 0.01;
        if fleet_ids.contains(&ship.fleet) || ship.fuel > -0.5 {
            continue;
        }
        // Respawn at home zone
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let dist = rng.gen_range(0.0..1500.0);
        ship.position = ship.home_zone + Vec2::new(angle.cos() * dist, angle.sin() * dist);
        revive(ship);
    }
    for fleet in sim.fleets.iter_mut() {
        let mut members = sim.ships.iter().filter(|s| s.fleet == fleet.id).peekable();
        if members.peek().is_none() || !members.all(|s| s.behavior == SimBehavior::Dead && s.fuel <= -0.5) {
            continue;
        }
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        fleet.position = fleet.home_zone + Vec2::from_angle(angle) * rng.gen_range(0.0..1500.0);
        fleet.velocity = Vec2::ZERO;
        fleet.destination = fleet.position;
        for ship in sim.ships.iter_mut().filter(|s| s.fleet == fleet.id) {
            ship.position = fleet.slot_position(ship.slot);
            revive(ship);
        }
    }
}

/// A fresh hull where a dead one was: full health and fuel, no wounds.
fn revive(ship: &mut SimulatedShip) {
    ship.health = 1.0;
    ship.fuel = 1.0;
    ship.behavior = SimBehavior::Patrolling;
    ship.spawned = false;
    ship.wounds = None;
    ship.velocity = Vec2::ZERO;
    // This is a fresh respawn, not the same hull a completed bounty was
    // hunting — clear the old tag so the faction's ship pool is eligible
    // for new bounties again. Without this, a single-ship faction (the
    // bosses) could only ever be offered as a bounty once, permanently.
    ship.bounty_id = None;
}

/// Periodically send a small hostile wing after the player. They seed just
/// outside render distance, headed inward — the existing per-faction AI takes
/// over once they materialize. First wave holds off long enough for the
/// player to learn the controls.
pub fn spawn_raider_waves(
    time: Res<Time>,
    mut sim: ResMut<WorldSimulation>,
    ship_query: Query<&Transform, With<Ship>>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
    mut next_wave_at: Local<f32>,
    mut elapsed: Local<f32>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
) {
    let Ok(player_transform) = ship_query.single() else { return };
    *elapsed += time.delta_secs();

    if *next_wave_at == 0.0 {
        *next_wave_at = 180.0; // first raid: 3 minutes in
    }
    if *elapsed < *next_wave_at {
        return;
    }
    *next_wave_at = *elapsed + 150.0 + rand::random::<f32>() * 90.0;

    let player_pos = player_transform.translation.truncate();
    let mut rng = rand::thread_rng();

    let faction = match rng.gen_range(0..3) {
        0 => AiShipType::RecursiveKingdom,   // swarm of junk ships
        1 => AiShipType::GildedThrone,  // tactical mercs
        _ => AiShipType::BrokenChoir,     // erratic ghost ships
    };
    // A raid is a raider fleet: it comes in together, on one bearing, and
    // goes back into the simulation together if you outrun it.
    let mut held = super::fleet::Held::new();
    let Some(classes) = super::fleet::compose(faction, super::fleet::FleetKind::Raiders, &mut rng, &mut held) else {
        return;
    };
    let approach_angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let at = player_pos + Vec2::from_angle(approach_angle) * rng.gen_range(2_800.0..3_600.0);
    let (mut fleet, members) = super::fleet::new_fleet(
        faction,
        super::fleet::FleetKind::Raiders,
        &classes,
        streaming.loaded_system.unwrap_or(0),
        at,
        player_pos,
        5_000.0,
        &mut rng,
    );
    // A raid is the one fleet that already knows where it is going.
    fleet.heading = (player_pos - at).to_angle();
    fleet.destination = player_pos;
    fleet.velocity = (player_pos - at).normalize_or_zero() * 60.0;
    sim.add_fleet(fleet, members);

    notifications.write(crate::events::ShowNotification {
        message: "Hostile contacts inbound on radar!".into(),
        notification_type: crate::events::NotificationType::Danger,
        duration: 4.0,
    });
}

/// A destroyed AI ship whose simulated entry has been marked dead. The
/// bookkeeping is meant to happen once per kill; without a marker it ran
/// every frame for every wreck, and each pass found a *living* spawned ship
/// of the same faction and marked that one dead too.
#[derive(Component)]
pub struct SimSettled;

/// Spawn/despawn real entities based on player proximity.
///
/// A fleet comes and goes whole: it materialises when any member is in range
/// and all of it fits the block budget, and goes back into the simulation
/// only once every member has left. Half a battle group popping in, or an
/// escort left behind as its capital vanished, is what this replaced.
#[allow(clippy::too_many_arguments)]
pub fn sync_simulation_entities(
    mut commands: Commands,
    mut sim: ResMut<WorldSimulation>,
    registry: Res<ModuleRegistry>,
    asset_server: Res<AssetServer>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    ship_query: Query<&Transform, With<Ship>>,
    // Faction stations live outside the simulation (diplomacy::stations):
    // they neither use the block budget nor despawn with distance here.
    ai_ships: Query<
        (
            Entity,
            &Transform,
            &AiShipType,
            &AiShipState,
            Option<&BountyTarget>,
            Has<SimSettled>,
            Option<&SimShipId>,
            (Option<&AiShipClass>, Has<MirrorShip>, Option<&Children>, Option<&super::fleet::FleetMember>),
        ),
        (With<AiShip>, Without<crate::diplomacy::FactionStation>, Without<LeftByLoad>),
    >,
    blocks: WoundBlocks,
) {
    let Ok(player_transform) = ship_query.single() else { return };
    let player_pos = player_transform.translation.truncate();
    let block_count = |children: Option<&Children>| {
        children.map_or(0, |c| c.iter().filter(|e| blocks.contains(*e)).count())
    };
    let sim: &mut WorldSimulation = &mut sim;
    let mut rng = rand::thread_rng();

    // Spawn what entered render distance, nearest first, while the block
    // budget allows. Living hulls only: a wreck's blocks are a pile of
    // salvage, not a ship the frame has to fly and fight.
    let mut live_blocks: usize = ai_ships
        .iter()
        .filter(|(_, _, _, state, ..)| !state.is_destroyed)
        .map(|(.., (_, _, children, _))| block_count(children))
        .sum();
    let any_live = ai_ships.iter().any(|(_, _, _, state, ..)| !state.is_destroyed);

    // A fleet is one candidate; a ship that flies alone is its own.
    struct Group {
        fleet: u32,
        members: Vec<usize>,
        dist: f32,
        cost: usize,
    }
    let live_fleets: std::collections::HashSet<u32> =
        sim.ships.iter().filter(|s| s.spawned && s.fleet != 0).map(|s| s.fleet).collect();
    let mut groups: Vec<Group> = Vec::new();
    let mut fleet_group: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (i, s) in sim.ships.iter().enumerate() {
        if s.spawned || s.behavior == SimBehavior::Dead {
            continue;
        }
        let lost = s.wounds.as_ref().map_or(0, |w| w.hull.len() + w.modules.len() + w.turret_parts.len());
        let cost = super::spawner::design_blocks(s.faction, s.class).saturating_sub(lost);
        let fleet = if sim.fleet(s.fleet).is_some() { s.fleet } else { 0 };
        let g = if fleet != 0 {
            *fleet_group.entry(fleet).or_insert_with(|| {
                groups.push(Group { fleet, members: Vec::new(), dist: f32::MAX, cost: 0 });
                groups.len() - 1
            })
        } else {
            groups.push(Group { fleet, members: Vec::new(), dist: f32::MAX, cost: 0 });
            groups.len() - 1
        };
        let group = &mut groups[g];
        group.members.push(i);
        group.cost += cost;
        // A fleet that's already partly on screen brings the rest at once.
        let d = if live_fleets.contains(&fleet) { 0.0 } else { s.position.distance(player_pos) };
        group.dist = group.dist.min(d);
    }
    groups.retain(|g| g.dist < render_distance());
    groups.sort_by(|a, b| a.dist.total_cmp(&b.dist));

    let mut spawned_any = any_live;
    let mut this_frame = 0usize;
    'groups: for group in groups {
        // Nearest first; one that doesn't fit waits rather than letting a
        // farther, smaller one jump the queue. The first always fits, or a
        // fleet bigger than the budget could never appear.
        // A fleet already partly on screen (arriving over several frames,
        // SPAWN_BLOCKS_PER_FRAME) always finishes: held back by the budget,
        // the rest of it never came and the fleet stayed split for good.
        if spawned_any && !live_fleets.contains(&group.fleet) && live_blocks + group.cost > max_live_ai_blocks() {
            break;
        }
        let fleet = sim.fleet(group.fleet).cloned();
        // The leader: its first living member (the capital, the lead hauler),
        // unless one is already on screen leading it.
        let leader = (!live_fleets.contains(&group.fleet)).then(|| group.members[0]);
        for &index in &group.members {
            if this_frame >= SPAWN_BLOCKS_PER_FRAME {
                break 'groups;
            }
            let sim_ship = &mut sim.ships[index];
            let lost = sim_ship.wounds.as_ref().map_or(0, |w| w.hull.len() + w.modules.len() + w.turret_parts.len());
            let cost = super::spawner::design_blocks(sim_ship.faction, sim_ship.class).saturating_sub(lost);
            this_frame += cost;
            live_blocks += cost;
            let root = spawner::spawn_ai_ship_wounded(
                sim_ship.faction,
                sim_ship.class,
                sim_ship.wounds.as_ref(),
                sim_ship.position,
                &mut commands,
                &registry,
                &asset_server,
            );
            if let Some(id) = sim_ship.bounty_id {
                commands.entity(root).insert(BountyTarget(id));
            }
            if sim_ship.id != 0 {
                commands.entity(root).insert(SimShipId(sim_ship.id));
            }
            // Off-screen fighting wears a ship down too, not only lost blocks.
            let integrity = sim_ship.wounds.as_ref().map_or(1.0, |w| w.integrity).min(sim_ship.health);
            if integrity < 0.999 {
                commands.entity(root).insert(SpawnIntegrity(integrity));
            }
            if let Some(fleet) = &fleet {
                // An escort forms up on you, not on a leader of its own.
                let escort = fleet.mission == super::fleet::Mission::Escort;
                let is_leader = leader == Some(index) && !escort;
                commands.entity(root).insert(super::fleet::FleetMember {
                    fleet: fleet.id,
                    slot: sim_ship.slot,
                    leader: is_leader,
                });
                if escort {
                    commands.entity(root).insert(super::standing_spawns::PlayerEscort);
                }
                if matches!(fleet.mission, super::fleet::Mission::Hunt { .. }) {
                    commands.entity(root).insert(super::standing_spawns::Hunter);
                }
                if is_leader {
                    let waypoints = leader_waypoints(fleet, &mut rng);
                    commands.entity(root).insert(AiShipNav {
                        destination: waypoints.first().copied(),
                        waypoints,
                        ..default()
                    });
                }
            }
            sim_ship.spawned = true;
            spawned_any = true;
        }
    }

    // A fleet stays on screen while any of it is within range.
    let near_fleets: std::collections::HashSet<u32> = ai_ships
        .iter()
        .filter(|(_, tf, _, state, ..)| {
            !state.is_destroyed && tf.translation.truncate().distance(player_pos) <= despawn_distance()
        })
        .filter_map(|(.., (_, _, _, member))| member.map(|m| m.fleet))
        .collect();
    // Where each fleet's members were when they left, to set it down there.
    let mut regroup: std::collections::HashMap<u32, Vec<Vec2>> = std::collections::HashMap::new();

    // Live ships that left render distance go back into the simulation;
    // destroyed ones settle their entry once and stay as real wrecks.
    for (entity, transform, ship_type, state, bounty, settled, sim_id, (class, mirror, children, member)) in ai_ships.iter() {
        // Which entry is this ship? By its id, then its bounty tag. A ship
        // with neither (a debug spawn, the tutorial's raider) has no entry,
        // and must not claim a stranger's: the old "first of its faction"
        // guess swapped ships' records whenever two were on screen.
        let entry = |sim: &mut WorldSimulation| -> Option<usize> {
            if let Some(id) = sim_id {
                sim.ships.iter().position(|s| s.id == id.0)
            } else if let Some(bounty) = bounty {
                sim.ships.iter().position(|s| s.bounty_id == Some(bounty.0))
            } else {
                None
            }
        };

        if state.is_destroyed {
            if settled {
                continue;
            }
            if let Some(i) = entry(sim) {
                let ship = &mut sim.ships[i];
                ship.behavior = SimBehavior::Dead;
                ship.health = 0.0;
                ship.spawned = false;
                ship.wounds = None;
            }
            commands.entity(entity).try_insert(SimSettled);
            continue;
        }

        let pos = transform.translation.truncate();
        if pos.distance(player_pos) <= despawn_distance() {
            continue;
        }
        if member.is_some_and(|m| near_fleets.contains(&m.fleet)) {
            continue;
        }
        // The doppelganger wears the player's design; it is not a ship of its
        // faction to file away, and it does not come back.
        // try_: a load clears every live AI ship the same frame, and a second
        // despawn of the same entity is a crash.
        if mirror {
            commands.entity(entity).try_despawn();
            continue;
        }
        let class_key = class.map(|c| c.0);
        let wounds = wounds_of(*ship_type, class_key, state.hull_integrity, children, &blocks);
        commands.entity(entity).try_despawn();

        match entry(sim) {
            Some(i) => {
                let ship = &mut sim.ships[i];
                ship.position = pos;
                ship.health = state.hull_integrity;
                ship.spawned = false;
                ship.behavior = SimBehavior::Patrolling;
                ship.wounds = wounds;
                if ship.fleet != 0 {
                    regroup.entry(ship.fleet).or_default().push(pos);
                }
            }
            None => {
                // Adopt it: a stray ship becomes an ordinary member of the
                // system it is in, damage and all, and is there when you
                // come back.
                let system_id = streaming.loaded_system.unwrap_or(0);
                let mut ship = SimulatedShip::patrolling(system_id, *ship_type, pos, pos, 6_000.0, 80.0);
                ship.class = class_key;
                ship.health = state.hull_integrity;
                ship.wounds = wounds;
                sim.add(ship);
            }
        }
    }

    // A fleet that left goes back to being one point where its ships were,
    // and they take up their slots round it again.
    for (fleet_id, positions) in regroup {
        let centre = positions.iter().copied().sum::<Vec2>() / positions.len() as f32;
        let Some(fleet) = sim.fleets.iter_mut().find(|f| f.id == fleet_id) else { continue };
        fleet.position = centre;
        let fleet = fleet.clone();
        for ship in sim.ships.iter_mut().filter(|s| s.fleet == fleet_id && !s.spawned && s.behavior != SimBehavior::Dead) {
            ship.position = fleet.slot_position(ship.slot);
        }
    }
}

/// The course a fleet's leader flies on screen: a convoy's run from its next
/// stop, a guard's circuit round its post, anyone else's next leg and a few
/// more round home.
fn leader_waypoints(fleet: &super::fleet::SimFleet, rng: &mut impl Rng) -> Vec<Vec2> {
    use super::fleet::FleetKind;
    match fleet.kind {
        FleetKind::Convoy if !fleet.route.is_empty() => {
            let n = fleet.route.len();
            (0..n).map(|k| fleet.route[(fleet.leg + k) % n]).collect()
        }
        FleetKind::StationGuard => (0..4)
            .map(|i| {
                let angle = fleet.heading + std::f32::consts::TAU * i as f32 / 4.0;
                fleet.home_zone + Vec2::from_angle(angle) * (fleet.patrol_radius * 0.7)
            })
            .collect(),
        _ => std::iter::once(fleet.destination)
            .chain((0..3).map(|_| patrol_waypoint(rng, fleet.home_zone, fleet.patrol_radius)))
            .collect(),
    }
}

/// A system changed hands. The loser's fleets there that are off-screen
/// fall back to its nearest remaining system; the winner sends a garrison (a
/// patrol, and a guard for the station it will put up). Fleets on screen
/// stay and fight it out. A system nobody has populated yet needs nothing:
/// it is populated for its new owner when it first loads.
pub fn fleets_change_hands(
    mut flips: MessageReader<crate::celestial::territory::SystemChangedHands>,
    mut sim: ResMut<WorldSimulation>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
) {
    use super::fleet::FleetKind;
    let sim: &mut WorldSimulation = &mut sim;
    let mut rng = rand::thread_rng();
    for flip in flips.read() {
        if !sim.ships.iter().any(|s| s.system_id == flip.system_id) {
            continue;
        }
        let Some(lost) = galaxy.systems.iter().find(|s| s.id == flip.system_id) else { continue };
        if let Some(loser) = flip.from {
            let refuge = galaxy
                .systems
                .iter()
                .filter(|s| s.faction == Some(loser) && s.id != flip.system_id)
                .min_by(|a, b| a.galaxy_pos.distance(lost.galaxy_pos).total_cmp(&b.galaxy_pos.distance(lost.galaxy_pos)));
            if let Some(refuge) = refuge {
                let live: std::collections::HashSet<u32> =
                    sim.ships.iter().filter(|s| s.spawned && s.fleet != 0).map(|s| s.fleet).collect();
                let home = crate::celestial::galaxy::faction_cluster_center(refuge);
                for fleet in sim.fleets.iter_mut() {
                    if fleet.system_id != flip.system_id || fleet.faction != loser || live.contains(&fleet.id) {
                        continue;
                    }
                    let at = home + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(3_000.0..12_000.0);
                    fleet.system_id = refuge.id;
                    fleet.position = at;
                    fleet.home_zone = home;
                    fleet.destination = at;
                    fleet.route.clear();
                    if fleet.kind == FleetKind::Convoy {
                        fleet.kind = FleetKind::Patrol;
                    }
                    let fleet = fleet.clone();
                    for ship in sim.ships.iter_mut().filter(|s| s.fleet == fleet.id) {
                        ship.system_id = refuge.id;
                        ship.home_zone = home;
                        ship.position = fleet.slot_position(ship.slot);
                    }
                }
            }
        }
        let Some(template) = faction_territories().into_iter().find(|t| t.faction == flip.to) else { continue };
        let home = crate::celestial::galaxy::faction_cluster_center(lost);
        let mut held = super::fleet::Held::new();
        let guard = crate::diplomacy::stations::station_plan(flip.to).is_some();
        for kind in [FleetKind::Patrol, FleetKind::StationGuard] {
            if kind == FleetKind::StationGuard && !guard {
                continue;
            }
            let Some(hulls) = super::fleet::compose(flip.to, kind, &mut rng, &mut held) else { continue };
            let at = home + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(3_000.0..12_000.0);
            let radius = if kind == FleetKind::StationGuard { GUARD_RADIUS } else { template.radius };
            let (mut fleet, members) = super::fleet::new_fleet(flip.to, kind, &hulls, flip.system_id, at, home, radius, &mut rng);
            fleet.destination = patrol_waypoint(&mut rng, home, radius);
            sim.add_fleet(fleet, members);
        }
    }
}

/// Keep the records of on-screen ships current: where they are, how hurt,
/// what they've lost, and where their fleet is. Once a second.
///
/// A ship's record used to hold where it was when it materialised until it
/// left again, so a save taken mid-encounter put every on-screen ship back
/// where it had appeared (a battle group reloaded five kilometres behind
/// itself), and a bounty marker pointed at a ship's arrival point.
pub fn track_live_ships(
    time: Res<Time>,
    mut next: Local<f32>,
    mut sim: ResMut<WorldSimulation>,
    ships: Query<
        (
            &Transform,
            &AiShipType,
            &AiShipState,
            &SimShipId,
            Option<&AiShipClass>,
            Option<&Children>,
            Option<&super::fleet::FleetMember>,
        ),
        (With<AiShip>, Without<LeftByLoad>, Without<crate::diplomacy::FactionStation>),
    >,
    blocks: WoundBlocks,
) {
    *next -= time.delta_secs();
    if *next > 0.0 {
        return;
    }
    *next = 1.0;
    let sim: &mut WorldSimulation = &mut sim;
    let index: std::collections::HashMap<u32, usize> =
        sim.ships.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
    for (tf, ship_type, state, id, class, children, member) in ships.iter() {
        if state.is_destroyed {
            continue;
        }
        let Some(&i) = index.get(&id.0) else { continue };
        let pos = tf.translation.truncate();
        let ship = &mut sim.ships[i];
        if !ship.spawned {
            continue;
        }
        ship.position = pos;
        ship.health = state.hull_integrity;
        ship.wounds = wounds_of(*ship_type, class.map(|c| c.0), state.hull_integrity, children, &blocks);
        if let Some(m) = member.filter(|m| m.leader) {
            if let Some(fleet) = sim.fleets.iter_mut().find(|f| f.id == m.fleet) {
                fleet.position = pos;
                fleet.heading = tf.rotation.to_euler(EulerRot::ZYX).0;
            }
        }
    }
}

/// What a live ship has lost against the design it was built from: the hull
/// cells and module anchors no longer there. None for an unhurt ship.
/// A ship's live blocks, for `wounds_of`: hull, modules, and the turrets
/// standing on its rings with the gun parts on them.
type WoundBlocks<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static crate::components::HullSegment>,
        Option<&'static crate::components::Module>,
        Option<&'static crate::combat::turret_deck::OnTurret>,
        Option<&'static Children>,
    ),
    (
        Or<(
            With<crate::components::HullSegment>,
            With<crate::components::Module>,
            With<crate::combat::turret_deck::TurretBody>,
        )>,
        Without<crate::components::HullDestroyed>,
        Without<crate::components::DestroyedModule>,
    ),
>;

fn wounds_of(
    faction: AiShipType,
    class: Option<&'static str>,
    integrity: f32,
    children: Option<&Children>,
    blocks: &WoundBlocks,
) -> Option<Wounds> {
    let mut hull = std::collections::HashSet::new();
    let mut modules = std::collections::HashSet::new();
    // A turret's gun parts are children of its body, not of the ship, and
    // their cells are the turret's own: kept by ring as well as cell, or
    // every turret gun read as shot away and came back missing from a load.
    let mut parts = std::collections::HashSet::new();
    for child in children.into_iter().flat_map(|c| c.iter()) {
        let Ok((h, m, _, deck)) = blocks.get(child) else { continue };
        if let Some(h) = h {
            hull.insert(h.grid_position);
        }
        if let Some(m) = m {
            modules.insert(m.grid_position);
        }
        if h.is_none() && m.is_none() {
            for part in deck.into_iter().flat_map(|d| d.iter()) {
                // The roof over a turret's guns is laid again when the ship
                // comes back (combat::gun_house); only its guns are recorded.
                if let Ok((_, Some(m), Some(on), _)) = blocks.get(part) {
                    if !crate::combat::gun_house::is_roof_block(m.module_type) {
                        parts.insert((on.ring_cell, m.grid_position));
                    }
                }
            }
        }
    }
    let built = super::spawner::built_design(faction, class);
    let on_turret = |m: &crate::building::blueprint::BlueprintModule| m.extras.as_ref().and_then(|e| e.turret);
    let wounds = Wounds {
        hull: built.hull_cells.iter().map(|c| c.grid_pos).filter(|p| !hull.contains(p)).collect(),
        modules: built
            .modules
            .iter()
            .filter(|m| on_turret(m).is_none())
            .map(|m| m.grid_pos)
            .filter(|p| !modules.contains(p))
            .collect(),
        turret_parts: built
            .modules
            .iter()
            .filter_map(|m| on_turret(m).map(|ring| (ring, m.grid_pos)))
            .filter(|p| !parts.contains(p))
            .collect(),
        integrity,
    };
    (!wounds.is_empty() || integrity < 0.999).then_some(wounds)
}

/// A wounded ship's integrity, set on its first frame back.
pub fn apply_spawn_integrity(
    mut commands: Commands,
    mut ships: Query<(Entity, &mut AiShipState, &SpawnIntegrity)>,
) {
    for (entity, mut state, integrity) in ships.iter_mut() {
        state.hull_integrity = integrity.0;
        commands.entity(entity).remove::<SpawnIntegrity>();
    }
}


#[cfg(test)]
mod population_tests {
    use super::*;

    /// A system's population must not be one clump.
    ///
    /// The old spawn put every ship in one disc around a single point, which
    /// is what "way too close to each other" was describing. This asserts the
    /// shape the replacement is supposed to have: several separate elements,
    /// at genuinely different distances from the centre, with pack-sized
    /// groups among them.
    #[test]
    fn a_system_is_populated_in_separated_groups() {
        let mut sim = WorldSimulation::default();
        let system = crate::celestial::resources::StarSystemDef {
            id: 7,
            name: "test".into(),
            galaxy_pos: Vec2::ZERO,
            local_center: Vec2::ZERO,
            seed: 1,
            faction: Some(AiShipType::RecursiveKingdom),
            capital: false,
            haunt: None,
            danger_tier: 60.0,
            discovery: crate::celestial::resources::SystemDiscovery::Visited,
            last_updated: 0.0,
            resource_fraction_remaining: 1.0,
            dark: 0.0,
        };
        spawn_system_faction_population(&mut sim, &system);
        let ships = &sim.ships;
        assert!(ships.len() >= 20, "only {} ships spawned", ships.len());

        let centre = crate::celestial::galaxy::faction_cluster_center(&system);
        let mut bands: Vec<f32> = ships.iter().map(|s| s.position.distance(centre)).collect();
        bands.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let spread = bands[bands.len() - 1] - bands[0];
        assert!(
            spread > 4_000.0,
            "every ship sits at much the same range from the centre ({spread:.0} units between \
             nearest and furthest) — that is the single clump this replaced"
        );

        // Clustering, between fleets: how far is each fleet from its nearest
        // neighbour? A mix means some move in company and some are alone.
        // (Inside a fleet the spacing is the formation's, by design.)
        let centres: Vec<Vec2> = sim.fleets.iter().map(|f| f.position).collect();
        assert!(centres.len() >= 5, "only {} fleets", centres.len());
        let mut nearest: Vec<f32> = Vec::new();
        for (i, a) in centres.iter().enumerate() {
            let d = centres
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, b)| a.distance(*b))
                .fold(f32::MAX, f32::min);
            nearest.push(d);
        }
        nearest.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let tightest = nearest[0];
        let loneliest = nearest[nearest.len() - 1];
        println!(
            "fleet nearest-neighbour spread: {:.0} .. {:.0} (median {:.0}) over {} fleets",
            tightest, loneliest, nearest[nearest.len() / 2], centres.len()
        );
        // 1.25: fleets of ships drawn twice as big need more room round
        // them, which evens the spacing out (it was 1.5 at half the size).
        assert!(
            loneliest > tightest * 1.25,
            "spacing is uniform — tightest neighbour {tightest:.0}, loneliest {loneliest:.0}"
        );
        // Packs and smaller groups: fleet sizes vary, and some are three or
        // more. (A lone ship is about one fleet in twelve, too rare to demand
        // in every roll.)
        let sizes: Vec<usize> = sim
            .fleets
            .iter()
            .map(|f| sim.ships.iter().filter(|s| s.fleet == f.id).count())
            .collect();
        assert!(sizes.iter().min() < sizes.iter().max(), "every fleet is the same size: {sizes:?}");
        assert!(sizes.iter().any(|&n| n >= 3), "no pack of three or more: {sizes:?}");

        // And no fleet is set down through another's formation.
        for (i, a) in sim.ships.iter().enumerate() {
            for b in &sim.ships[i + 1..] {
                if a.fleet == b.fleet {
                    continue;
                }
                let clear = a.position.distance(b.position)
                    - crate::ai_ship::spawner::design_extent(a.faction, a.class)
                    - crate::ai_ship::spawner::design_extent(b.faction, b.class);
                assert!(clear > 0.0, "ships of two fleets overlap by {:.0}", -clear);
            }
        }
    }

    /// A capital keeps a home fleet: a battle group (a capital ship and its
    /// escorts) posted at its starport, besides the ordinary station guard.
    #[test]
    fn a_capital_keeps_a_home_fleet() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let capital = galaxy
            .systems
            .iter()
            .find(|s| s.capital && s.faction == Some(AiShipType::TerranHegemony))
            .expect("the Hegemony has a capital");
        let mut sim = WorldSimulation::default();
        spawn_system_faction_population(&mut sim, capital);
        let guards: Vec<_> = sim.fleets.iter().filter(|f| f.kind == super::super::fleet::FleetKind::StationGuard).collect();
        let led_by_a_capital = guards.iter().any(|f| {
            sim.ships.iter().any(|s| {
                s.fleet == f.id && super::super::fleet::role_of(s.faction, s.class) == super::super::roster::ClassRole::Capital
            })
        });
        assert!(led_by_a_capital, "no home fleet with a capital ship at the Hegemony's capital");

        // An ordinary system of the same faction has none.
        let outpost = galaxy.systems.iter().find(|s| !s.capital && s.faction == Some(AiShipType::TerranHegemony)).unwrap();
        let mut sim = WorldSimulation::default();
        spawn_system_faction_population(&mut sim, outpost);
        assert!(sim.fleets.iter().filter(|f| f.kind == super::super::fleet::FleetKind::StationGuard).count() <= 1);
    }

    /// Spread over the system, not one territory: several holdings occupied,
    /// far apart, none holding most of the fleet, and none parked on a
    /// station's berth.
    #[test]
    fn a_system_is_populated_across_several_holdings() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        for system in galaxy.systems.iter().filter(|s| s.faction.is_some()) {
            let mut sim = WorldSimulation::default();
            spawn_system_faction_population(&mut sim, system);
            let mut homes: Vec<(Vec2, usize)> = Vec::new();
            for ship in &sim.ships {
                // A convoy's home is the stop its run starts from; its
                // home_zone is the middle of the run.
                let home = sim
                    .fleet(ship.fleet)
                    .and_then(|f| f.route.first().copied())
                    .unwrap_or(ship.home_zone);
                match homes.iter_mut().find(|(h, _)| h.distance(home) < 1.0) {
                    Some((_, n)) => *n += 1,
                    None => homes.push((home, 1)),
                }
            }
            assert!(homes.len() >= 3, "{}: ships in only {} holdings", system.name, homes.len());
            let most = homes.iter().map(|(_, n)| *n).max().unwrap();
            assert!(
                most * 10 <= sim.ships.len() * 7,
                "{}: {most} of {} ships in one holding",
                system.name,
                sim.ships.len()
            );
            for site in crate::world::home_base::station_sites(system) {
                for (home, _) in &homes {
                    assert!(
                        home.distance(site.pos) > 25_000.0,
                        "{}: a holding {:.0} from {}",
                        system.name,
                        home.distance(site.pos),
                        site.name
                    );
                }
            }
        }
    }

    /// Patrols have to actually go somewhere. The old random walk left a ship
    /// roughly where it started however long you watched it.
    #[test]
    fn a_patrol_covers_ground() {
        // Seeded. This used `rand::thread_rng()`, so waypoints differed every
        // run and the test failed about two runs in five on identical code
        // (measured: 6 of 15) -- a coin flip that reads as a regression.
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(11);
        let home = Vec2::ZERO;
        let mut ship = SimulatedShip::patrolling(1, AiShipType::GildedThrone, home, home, 12_000.0, 90.0);
        ship.destination = Vec2::new(6_000.0, 0.0);

        // Distance TRAVELLED, summed step by step. This measured straight-line
        // distance from the start instead, which cannot tell a patrol from
        // loitering: a ship that sweeps its territory properly and happens to
        // curve back toward where it began reads as having gone nowhere. That
        // was the other half of the flake -- the random part only decided how
        // often the loop closed.
        let mut travelled = 0.0;
        for _ in 0..60 {
            let to_dest = ship.destination - ship.position;
            let remaining = to_dest.length();
            if remaining <= ARRIVE_RADIUS {
                ship.destination = next_waypoint(&mut rng, ship.home_zone, ship.patrol_radius, &[]);
            } else {
                let want = to_dest / remaining * ship.cruise;
                ship.velocity = (ship.velocity * 0.75 + want * 0.25).clamp_length_max(ship.cruise);
            }
            let step = ship.velocity * 2.0;
            travelled += step.length();
            ship.position += step;
        }

        assert!(
            travelled > 3_000.0,
            "two minutes of patrol moved the ship {travelled:.0} units — it is loitering, not patrolling"
        );
        assert!(
            ship.position.distance(home) <= 12_000.0,
            "patrol left its territory"
        );
    }
}

#[cfg(test)]
mod wreck_bookkeeping_tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.insert_resource(crate::building::registry::build_registry());
        app.init_resource::<WorldSimulation>();
        app.init_resource::<crate::celestial::resources::SystemStreamingManager>();
        app.add_systems(Update, sync_simulation_entities);
        app.world_mut().spawn((Ship, Transform::default()));
        app
    }

    fn state(destroyed: bool) -> AiShipState {
        let mut s = AiShipState::default();
        s.is_destroyed = destroyed;
        s
    }

    fn add_spawned(app: &mut App, x: f32) -> u32 {
        let mut sim = app.world_mut().resource_mut::<WorldSimulation>();
        let mut ship = SimulatedShip::patrolling(0, AiShipType::RecursiveKingdom, Vec2::new(x, 0.0), Vec2::ZERO, 500.0, 100.0);
        ship.spawned = true;
        sim.add(ship);
        sim.ships.last().unwrap().id
    }

    /// One wreck marks its own simulated ship dead, once -- not a living ship
    /// of the same faction every frame after.
    #[test]
    fn a_wreck_settles_one_entry_once() {
        let mut app = app();
        let wrecked = add_spawned(&mut app, 100.0);
        let living = add_spawned(&mut app, 3_000.0);
        app.world_mut().spawn((AiShip, AiShipType::RecursiveKingdom, state(true), SimShipId(wrecked), Transform::from_xyz(100.0, 0.0, 0.0)));
        app.world_mut().spawn((AiShip, AiShipType::RecursiveKingdom, state(false), SimShipId(living), Transform::from_xyz(3_000.0, 0.0, 0.0)));
        for _ in 0..5 {
            app.update();
        }
        let sim = app.world().resource::<WorldSimulation>();
        let dead = sim.ships.iter().filter(|s| s.behavior == SimBehavior::Dead).count();
        assert_eq!(dead, 1, "the wreck took a living ship's entry with it");
        assert!(sim.ships[1].spawned, "the living ship lost its entry");
    }

    /// A wreck with no entry (a debug spawn, the tutorial's raider) settles
    /// nobody's: the old nearest-of-its-faction guess killed a stranger.
    #[test]
    fn an_untracked_wreck_touches_no_entry() {
        let mut app = app();
        add_spawned(&mut app, 100.0);
        app.world_mut().spawn((AiShip, AiShipType::RecursiveKingdom, state(true), Transform::from_xyz(120.0, 0.0, 0.0)));
        for _ in 0..3 {
            app.update();
        }
        let sim = app.world().resource::<WorldSimulation>();
        assert!(sim.ships.iter().all(|s| s.behavior != SimBehavior::Dead));
    }

    /// An untracked ship that leaves range joins the simulation instead of
    /// overwriting another ship's record.
    #[test]
    fn an_untracked_ship_is_adopted_when_it_leaves() {
        let mut app = app();
        let tracked = add_spawned(&mut app, 100.0);
        app.world_mut().spawn((AiShip, AiShipType::RecursiveKingdom, state(false), Transform::from_xyz(90_000.0, 0.0, 0.0)));
        app.update();
        let sim = app.world().resource::<WorldSimulation>();
        assert_eq!(sim.ships.len(), 2, "the stray was adopted");
        let first = sim.ships.iter().find(|s| s.id == tracked).unwrap();
        assert!(first.spawned && first.position.x == 100.0, "the tracked ship's record was left alone");
        assert!(sim.ships.iter().any(|s| s.position.x == 90_000.0 && !s.spawned));
    }

    /// The whole round trip: a ship spawns from its entry, loses blocks,
    /// leaves range (its holes are written down), and comes back with them.
    #[test]
    fn damage_survives_going_off_screen() {
        let mut app = app();
        // Real ships load their sprites.
        app.init_asset::<Image>();
        let class = crate::ai_ship::roster::intern("lancer");
        {
            let mut sim = app.world_mut().resource_mut::<WorldSimulation>();
            let mut ship = SimulatedShip::patrolling(0, AiShipType::TerranHegemony, Vec2::new(2_000.0, 0.0), Vec2::new(2_000.0, 0.0), 500.0, 0.0);
            ship.class = class;
            sim.add(ship);
        }
        app.update();
        let mut roots = app.world_mut().query_filtered::<(Entity, &Children), With<AiShip>>();
        let (_, children) = roots.single(app.world()).expect("the ship spawned from its entry");
        let built: Vec<Entity> = children.iter().collect();
        // Shoot three hull cells clean off.
        let mut hull = app.world_mut().query::<&crate::components::HullSegment>();
        let victims: Vec<Entity> = built.iter().copied().filter(|e| hull.get(app.world(), *e).is_ok()).take(3).collect();
        let lost: Vec<IVec2> = victims.iter().map(|e| hull.get(app.world(), *e).unwrap().grid_position).collect();
        for v in &victims {
            app.world_mut().despawn(*v);
        }
        // The player leaves: the ship goes back into the simulation.
        let player = app.world_mut().query_filtered::<Entity, With<Ship>>().single(app.world()).unwrap();
        app.world_mut().entity_mut(player).insert(Transform::from_xyz(-100_000.0, 0.0, 0.0));
        app.update();
        let sim = app.world().resource::<WorldSimulation>();
        let wounds = sim.ships[0].wounds.clone().expect("its damage was written down");
        assert!(!sim.ships[0].spawned);
        for p in &lost {
            assert!(wounds.hull.contains(p), "lost cell {p:?} was not recorded");
        }
        // And comes back: the same holes, nothing else missing.
        app.world_mut().entity_mut(player).insert(Transform::default());
        app.update();
        let mut roots = app.world_mut().query_filtered::<&Children, With<AiShip>>();
        let children: Vec<Entity> = roots.single(app.world()).unwrap().iter().collect();
        let mut hull = app.world_mut().query::<&crate::components::HullSegment>();
        let cells: Vec<IVec2> = children.iter().filter_map(|e| hull.get(app.world(), *e).ok()).map(|h| h.grid_position).collect();
        for p in &lost {
            assert!(!cells.contains(p), "lost cell {p:?} grew back");
        }
        let design = crate::ai_ship::spawner::built_design(AiShipType::TerranHegemony, class);
        assert_eq!(cells.len(), design.hull_cells.len() - lost.len());
    }

    /// A fleet materialises whole when any of it is in range, and goes back
    /// into the simulation only once all of it has left.
    #[test]
    fn a_fleet_comes_and_goes_whole() {
        let mut app = app();
        app.init_asset::<Image>();
        let lancer = crate::ai_ship::roster::intern("lancer");
        {
            let mut sim = app.world_mut().resource_mut::<WorldSimulation>();
            let fleet = super::super::fleet::SimFleet::new(
                super::super::fleet::FleetKind::Patrol, AiShipType::TerranHegemony, 0,
                Vec2::new(8_000.0, 0.0), Vec2::ZERO, 50_000.0, 80.0,
            );
            let members = super::super::fleet::crew(&fleet, &super::super::fleet::hulls_of(AiShipType::TerranHegemony, &[lancer, lancer, lancer]));
            sim.add_fleet(fleet, members);
            // Strung out: only the first is inside render distance.
            sim.ships[0].position = Vec2::new(24_000.0, 0.0);
            sim.ships[1].position = Vec2::new(36_000.0, 0.0);
            sim.ships[2].position = Vec2::new(90_000.0, 0.0);
        }
        app.update();
        let mut members = app.world_mut().query::<&super::super::fleet::FleetMember>();
        let live: Vec<_> = members.iter(app.world()).copied().collect();
        assert_eq!(live.len(), 3, "the whole fleet came in, not just the ship in range");
        assert_eq!(live.iter().filter(|m| m.leader).count(), 1, "one leader");

        // The player moves off; one member is still near.
        let player = app.world_mut().query_filtered::<Entity, With<Ship>>().single(app.world()).unwrap();
        app.world_mut().entity_mut(player).insert(Transform::from_xyz(120_000.0, 0.0, 0.0));
        app.update();
        assert_eq!(members.iter(app.world()).count(), 3, "a fleet member left while another was in range");
        assert!(app.world().resource::<WorldSimulation>().ships.iter().all(|s| s.spawned));

        // And then all of it is far: it goes back as one, in formation.
        app.world_mut().entity_mut(player).insert(Transform::from_xyz(-180_000.0, 0.0, 0.0));
        app.update();
        app.update();
        assert_eq!(members.iter(app.world()).count(), 0);
        let sim = app.world().resource::<WorldSimulation>();
        assert!(sim.ships.iter().all(|s| !s.spawned));
        let fleet = sim.fleets[0].clone();
        for ship in &sim.ships {
            assert!(ship.position.distance(fleet.slot_position(ship.slot)) < 1.0, "a member is out of its slot");
        }
    }

    /// A whole populated system, the player parked in the middle of one of
    /// its holdings: whatever comes on screen comes as whole fleets, and the
    /// block budget holds (bar the one group that always fits).
    #[test]
    fn a_populated_system_brings_only_whole_fleets() {
        let mut app = app();
        app.init_asset::<Image>();
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let system = galaxy
            .systems
            .iter()
            .find(|s| s.faction == Some(AiShipType::TerranHegemony))
            .expect("the Hegemony holds a system")
            .clone();
        let centre = {
            let mut sim = app.world_mut().resource_mut::<WorldSimulation>();
            spawn_system_faction_population(&mut sim, &system);
            assert!(!sim.fleets.is_empty());
            // The holding with the most fleets.
            let mut best = (Vec2::ZERO, 0usize);
            for f in &sim.fleets {
                let n = sim.fleets.iter().filter(|g| g.position.distance(f.position) < RENDER_DISTANCE).count();
                if n > best.1 {
                    best = (f.position, n);
                }
            }
            best.0
        };
        let player = app.world_mut().query_filtered::<Entity, With<Ship>>().single(app.world()).unwrap();
        app.world_mut().entity_mut(player).insert(Transform::from_translation(centre.extend(0.0)));
        // A big fleet arrives over a few frames (SPAWN_BLOCKS_PER_FRAME):
        // give it time to finish coming.
        for _ in 0..20 {
            app.update();
        }

        let mut members = app.world_mut().query::<&super::super::fleet::FleetMember>();
        let live: Vec<u32> = members.iter(app.world()).map(|m| m.fleet).collect();
        assert!(!live.is_empty(), "nothing came on screen in the middle of a holding");
        let sim = app.world().resource::<WorldSimulation>();
        let mut fleets: Vec<u32> = live.clone();
        fleets.sort_unstable();
        fleets.dedup();
        for id in &fleets {
            let on_screen = live.iter().filter(|f| *f == id).count();
            let alive = sim.ships.iter().filter(|s| s.fleet == *id && s.behavior != SimBehavior::Dead).count();
            assert_eq!(on_screen, alive, "fleet {id}: {on_screen} of {alive} came on screen");
        }
        let blocks: usize = sim
            .ships
            .iter()
            .filter(|s| s.spawned)
            .map(|s| crate::ai_ship::spawner::design_blocks(s.faction, s.class))
            .sum();
        assert!(fleets.len() == 1 || blocks <= MAX_LIVE_AI_BLOCKS, "{blocks} blocks live over {} fleets", fleets.len());

        // And the player leaves: every fleet goes back whole.
        app.world_mut().entity_mut(player).insert(Transform::from_xyz(centre.x + 200_000.0, centre.y, 0.0));
        app.update();
        app.update();
        assert_eq!(members.iter(app.world()).count(), 0);
        assert!(app.world().resource::<WorldSimulation>().ships.iter().all(|s| !s.spawned));
    }
}
