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
/// either a thin swarm or a frame-rate cliff.
///
/// 9,000 kept the capitals away: a battle group is 7-12k blocks, so with a
/// few small ships already about it never fitted, and you only ever saw the
/// small ones. 14,000 fits one with room to spare (measured 2026-11-16 on an
/// M2: about 1.1 ms of simulation a frame per 1,000 live AI blocks), and a
/// nearer fleet now makes room by sending far ships back (EVICT_CLEAR).
///
/// 20,000 since 2026-11-23: the reworked fleet's civilians and line ships run
/// 2-4k blocks, and six of them round Haven filled 14,000, so no battle group
/// (fleet::BATTLE_GROUP_BLOCKS, 12,000) came on screen in four 40-minute
/// autoplay runs. The perf work cut the cost to about 0.3 ms a frame per
/// 1,000 live AI blocks (16.7k in a fleet fight: 8.1 ms of game work in all).
const MAX_LIVE_AI_BLOCKS: usize = 20_000;

/// A ship at least this far from you, and farther than a fleet waiting to
/// come on screen, goes back into the simulation to make room for it: past
/// the fighting band and a missile's reach, so nothing you're fighting.
const EVICT_CLEAR: f32 = 20_000.0;

/// And farther than the waiting fleet by this much. Without it two fleets
/// about as far off as each other, with room for only one, traded places
/// whenever they crossed: each vanished and was built again every few
/// seconds (seen 2026-11-17 beside the Terran fleet, every 4 to 30 s).
const EVICT_MARGIN: f32 = 10_000.0;

/// Most AI blocks brought into being in one frame. Spawning is a whole ship's
/// worth of entities at once, about a millisecond per hundred blocks: since
/// the ships were drawn twice as big a fleet arriving in one frame froze the
/// game for a tenth of a second. Past this it arrives over a few frames, the
/// rest of the fleet first in line (it's already partly on screen).
const SPAWN_BLOCKS_PER_FRAME: usize = 2_500;

/// RENDER_DISTANCE, or AFTERLIGHT_AI_RANGE: a filming switch to make whole fleets
/// real far from the player, for wide shots of a war.
pub fn render_distance() -> f32 {
    static V: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_AI_RANGE").ok().and_then(|v| v.parse().ok()).unwrap_or(RENDER_DISTANCE))
}

/// DESPAWN_DISTANCE, kept the same distance past render_distance().
fn despawn_distance() -> f32 {
    DESPAWN_DISTANCE + (render_distance() - RENDER_DISTANCE)
}

/// MAX_LIVE_AI_BLOCKS, or AFTERLIGHT_AI_BLOCKS (filming).
fn max_live_ai_blocks() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_AI_BLOCKS").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_LIVE_AI_BLOCKS))
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

    // AFTERLIGHT_MOVETEST=1: bare movement sandbox. Normally zero AI ships, but
    // AFTERLIGHT_MOVETEST_ENEMY=1 adds exactly one non-shooting dummy (see
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
        if std::env::var("AFTERLIGHT_MOVETEST_ENEMY").ok().as_deref() == Some("1") {
            // AFTERLIGHT_MOVETEST_ENEMY_FACTION overrides the dummy's faction for
            // behavior-tree testing (e.g. "The Silence", "Terran Hegemony") — defaults
            // to Broken Choir for the original damage-model testing use case.
            let faction = std::env::var("AFTERLIGHT_MOVETEST_ENEMY_FACTION")
                .ok()
                .and_then(|f| faction_named(&f))
                .unwrap_or(AiShipType::BrokenChoir);
            let mut dummy = SimulatedShip::patrolling(
                0, faction, Vec2::new(500.0, 0.0), Vec2::new(500.0, 0.0), 2000.0, 0.0,
            );
            // AFTERLIGHT_MOVETEST_ENEMY_CLASS=<key> picks a roster class (ai_ship::roster),
            // e.g. "bulwark"; unset flies the faction's original design.
            if let Ok(key) = std::env::var("AFTERLIGHT_MOVETEST_ENEMY_CLASS") {
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
            // A design file (AFTERLIGHT_MOVETEST_ENEMY_DESIGN, ai_ship::spawner)
            // can be a whole warship: at 500 units it would be born inside
            // the player's hull, the same as a class.
            if std::env::var("AFTERLIGHT_MOVETEST_ENEMY_DESIGN").is_ok() {
                dummy.position = Vec2::new(3200.0, 0.0);
                dummy.home_zone = dummy.position;
                dummy.destination = dummy.position;
            }
            let shoots = std::env::var("AFTERLIGHT_MOVETEST_ENEMY_SHOOTS").ok().as_deref() == Some("1");
            info!(
                "MOVETEST: one {} {:?} {:?} dummy spawned at {:?}",
                if shoots { "shooting" } else { "non-shooting" },
                faction,
                dummy.class,
                dummy.position
            );
            sim.add(dummy);
        }

        // AFTERLIGHT_MOVETEST_FLEET=<Faction>:<Kind> (Patrol, BattleGroup, Convoy,
        // StationGuard, Raiders): one whole fleet of that kind, 9 km off and
        // crossing ahead of you, for watching formations and fleet fights.
        // Several, separated by ';', alternate sides and meet head-on in the
        // middle -- two enemies make a battle to film.
        if let Ok(specs) = std::env::var("AFTERLIGHT_MOVETEST_FLEET") {
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
                // AFTERLIGHT_FLEET_SEED=<n>: the same ships every run, so two test
                // fights can be compared at all.
                use rand::SeedableRng;
                let mut rng = match std::env::var("AFTERLIGHT_FLEET_SEED").ok().and_then(|v| v.parse::<u64>().ok()) {
                    Some(seed) => rand::rngs::StdRng::seed_from_u64(seed + i as u64),
                    None => rand::rngs::StdRng::from_entropy(),
                };
                let mut held = super::fleet::Held::new();
                // Even: from the east heading west (the original single fleet);
                // odd: from the west heading east, a little offset.
                // AFTERLIGHT_MOVETEST_FLEET_AT=<x>,<y>: meet there instead, starting
                // close enough to be at it within seconds -- for filming.
                let meet = std::env::var("AFTERLIGHT_MOVETEST_FLEET_AT").ok()
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
        if let Ok(count) = std::env::var("AFTERLIGHT_AI_VS_AI_TEST").unwrap_or_default().parse::<usize>() {
            let factions = [AiShipType::TerranHegemony, AiShipType::RecursiveKingdom, AiShipType::BrokenChoir, AiShipType::GildedThrone];
            // AFTERLIGHT_AI_VS_AI_AT=<x>,<y>: hold the fight away from you, to film it.
            let centre = std::env::var("AFTERLIGHT_AI_VS_AI_AT").ok()
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
    if sim.populated.contains(&system_id) {
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
        sim.populated.insert(system_id);
    }
}

/// Whether a system is one nothing hostile comes to: Haven. Traders still
/// call; patrols and raiders from the neighbours don't wander in, and
/// whoever is hunting you waits for you to leave it.
pub fn safe_harbour(system: &crate::celestial::resources::StarSystemDef) -> bool {
    crate::narrative::front::is_spared(system)
}

/// Room a fleet's home keeps from a star or the path of a planet.
const HOLDING_CLEAR: f32 = 8_000.0;

/// Nothing new is put into the world nearer you than this: past where ships
/// come on screen (48 km), inside where their far pictures show (105 km).
/// What arrives, arrives: a flash at a distance, then a silhouette, then hulls.
pub const ARRIVAL_RANGE: f32 = 54_000.0;
/// How fast a fleet that has just jumped in, or is on its way out, travels
/// while nobody is flying it. Patrol pace (45-140 u/s) across a system
/// hundreds of km wide is an hour's journey; this is minutes.
pub const TRANSIT_CRUISE: f32 = 600.0;

/// Where something that wants to be at `want` is actually put: there, if
/// that is out of your sight; otherwise out along the line from you through
/// it until it is `ARRIVAL_RANGE` off. Clear of the star and the planets
/// either way.
///
/// Scavengers used to appear 2 km from a wreck you were sitting on, an
/// escort 12 km from you, a garrison wherever the new owner's rally point
/// was, you or no you.
pub fn arrival_point(system: &crate::celestial::resources::StarSystemDef, you: Option<Vec2>, want: Vec2, reach: f32) -> Vec2 {
    let clear = |at: Vec2| super::traffic::clear_point(system, at, reach);
    let Some(you) = you else { return clear(want) };
    let at = clear(want);
    if at.distance(you) >= ARRIVAL_RANGE {
        return at;
    }
    // On the side it was wanted, if there is clear space there at that
    // range; else the nearest bearing to it that has some. Near a star the
    // space between it and the first planet's path is a narrow band, and
    // straight out can be a long way before anything is clear.
    let out = (want - you).normalize_or(Vec2::X);
    for ring in 0..6 {
        let range = ARRIVAL_RANGE + 500.0 + ring as f32 * 8_000.0;
        let best = (0..12)
            .map(|k| {
                // 0, +30, -30, +60, -60 ... degrees off the side it was wanted.
                let turn = ((k + 1) / 2) as f32 * 30.0_f32.to_radians() * if k % 2 == 0 { -1.0 } else { 1.0 };
                clear(you + Vec2::from_angle(turn).rotate(out) * range)
            })
            .filter(|at| at.distance(you) >= ARRIVAL_RANGE && at.distance(you) <= range + 30_000.0)
            .min_by(|a, b| a.distance(want).total_cmp(&b.distance(want)));
        if let Some(at) = best {
            return at;
        }
    }
    clear(you + out * (ARRIVAL_RANGE * 2.0))
}

/// The jump point a fleet of `from` comes into `system` by: the one toward
/// the nearest other system `from` holds, or toward any neighbour.
pub fn jump_in_point(
    galaxy: &crate::celestial::resources::GalaxyMap,
    system: &crate::celestial::resources::StarSystemDef,
    from: Option<AiShipType>,
    rng: &mut impl Rng,
) -> Vec2 {
    let theirs = galaxy
        .systems
        .iter()
        .filter(|s| s.id != system.id && from.is_some() && s.faction == from)
        .min_by(|a, b| a.galaxy_pos.distance(system.galaxy_pos).total_cmp(&b.galaxy_pos.distance(system.galaxy_pos)));
    let other = theirs.or_else(|| {
        let near = crate::celestial::galaxy::nearest_neighbors(galaxy, system.id, 3);
        (!near.is_empty()).then(|| near[rng.gen_range(0..near.len())]).and_then(|id| galaxy.systems.iter().find(|s| s.id == id))
    });
    match other {
        Some(other) => super::traffic::jump_point(system, other, 1.0),
        None => super::traffic::clear_point(system, system.local_center + Vec2::X * 90_000.0, 6_000.0),
    }
}

/// The jump point nearest `from` that leads out of `system`.
pub fn depart_point(galaxy: &crate::celestial::resources::GalaxyMap, system: &crate::celestial::resources::StarSystemDef, from: Vec2) -> Vec2 {
    crate::celestial::galaxy::nearest_neighbors(galaxy, system.id, 3)
        .into_iter()
        .filter_map(|id| galaxy.systems.iter().find(|s| s.id == id))
        .map(|other| super::traffic::jump_point(system, other, 1.0))
        .min_by(|a, b| a.distance(from).total_cmp(&b.distance(from)))
        .unwrap_or_else(|| super::traffic::clear_point(system, system.local_center + (from - system.local_center).normalize_or(Vec2::X) * 90_000.0, 6_000.0))
}

/// Show the jumps: the player's own drive flash, one ring to a fleet, for
/// those in the system you are in and near enough to see.
pub fn show_jump_flashes(
    mut commands: Commands,
    mut sim: ResMut<WorldSimulation>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    player: Query<&Transform, With<Ship>>,
    fx: Option<Res<crate::vfx::effect_textures::EffectTextures>>,
) {
    if sim.jumps.is_empty() {
        return;
    }
    let jumps = std::mem::take(&mut sim.jumps);
    let you = player.single().ok().map(|t| t.translation.truncate());
    for jump in jumps {
        let off = you.map(|p| p.distance(jump.at));
        let seen = streaming.loaded_system == Some(jump.system) && off.is_some_and(|d| d <= super::preview::PREVIEW_RANGE);
        if !seen {
            continue;
        }
        info!("JUMP {} {:?}, {:.0} km from you", if jump.arriving { "in" } else { "out" }, jump.faction, off.unwrap_or(0.0) / 1000.0);
        if let Some(fx) = fx.as_deref() {
            crate::vfx::particles::spawn_warp_flash(&mut commands, fx, jump.at, jump.reach, Color::srgb(0.45, 0.70, 1.0));
        }
    }
}

/// On screen, a fleet that is leaving flies for its jump point, and goes
/// when it gets there.
pub fn fly_departures(
    time: Res<Time>,
    mut next: Local<f32>,
    mut sim: ResMut<WorldSimulation>,
    mut leaders: Query<(&Transform, &mut AiShipNav, &super::fleet::FleetMember)>,
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 1.0;
    let mut gone: Vec<u32> = Vec::new();
    for (tf, mut nav, member) in leaders.iter_mut().filter(|(.., m)| m.leader) {
        let Some(point) = sim.fleet(member.fleet).and_then(|f| f.depart) else { continue };
        if nav.waypoints.len() != 1 || nav.waypoints[0] != point {
            nav.waypoints = vec![point];
            nav.current_waypoint = 0;
            nav.destination = Some(point);
        }
        if tf.translation.truncate().distance(point) < DEPART_AT {
            gone.push(member.fleet);
        }
    }
    sim.fold_back.extend(gone);
}

/// A fleet this near its jump point has reached it.
const DEPART_AT: f32 = 3_000.0;

/// How many hulls `system`'s owner keeps there: what it is populated to,
/// and what it is topped back up toward.
pub fn owner_target(system: &crate::celestial::resources::StarSystemDef) -> usize {
    let Some(faction) = system.faction else { return 0 };
    let Some(template) = faction_territories().into_iter().find(|t| t.faction == faction) else { return 0 };
    let count = system_ship_count(template.ship_count, super::components::faction_power(faction));
    // A capital is where a faction keeps its strength.
    if system.capital { (count as f32 * CAPITAL_SHARE).round() as usize } else { count }
}

/// Seconds between looks at whether a system needs anyone sent to it.
pub const REFILL_EVERY: f32 = 300.0;
/// A system with this share of its strength left is left alone. Not nearer
/// full than this: its own traders come and go between systems, and a
/// convoy away on a run is not a loss to be made good.
const REFILL_BELOW: f32 = 0.75;
/// A boss killed in its haunt is gone this long before another comes.
const BOSS_RETURNS: f32 = 1_800.0;
/// A dead ship's `fuel` counts down by 0.01 a tick (2 s) from wherever it
/// was. At this it is struck off (a fleet lost with all hands, a ship
/// flying alone)...
const STRUCK_OFF_AT: f32 = -0.5;
/// ...and at this a fleet that still has ships is sent a replacement for
/// one it lost: about ten minutes.
const MENDED_AT: f32 = -2.0;

/// Who a system's fighting ships belong to: its owner, or the owner's ally
/// when the owner is a trading house with no navy of its own.
fn flier_of(owner: AiShipType) -> AiShipType {
    owner.ally().filter(|_| owner.is_trading()).unwrap_or(owner)
}

/// Send one fleet to a system that has lost too many: the owner's first,
/// else one for a neighbour that has nobody left there. It comes in by the
/// jump from its own nearest system and makes for a holding. Nothing for a
/// harbour, the dark, or a point you would see it appear at.
///
/// This is what there is instead of the dead getting up again where they
/// fell, five minutes on, whether you were looking or not.
pub fn refill_system(
    sim: &mut WorldSimulation,
    galaxy: &crate::celestial::resources::GalaxyMap,
    system: &crate::celestial::resources::StarSystemDef,
    you: Option<Vec2>,
    rng: &mut impl Rng,
) -> Option<String> {
    use super::fleet::{FleetKind, Mission};
    if safe_harbour(system) || crate::narrative::front::is_taken(system) {
        return None;
    }
    // Who counts as being here: everyone but fleets sent for or with you,
    // and fleets on their way out. Traders count: a system's convoys are
    // part of what it was populated with, and the freight lanes keep their
    // own numbers up (ai_ship::traffic), so none are sent from here.
    let ordinary: std::collections::HashSet<u32> =
        sim.fleets.iter().filter(|f| f.system_id == system.id && f.mission == Mission::None && f.depart.is_none()).map(|f| f.id).collect();
    // The hulls of `faction` here and alive, as (faction, class).
    let living = |sim: &WorldSimulation, faction: AiShipType| -> Vec<super::fleet::Hull> {
        sim.ships
            .iter()
            .filter(|s| s.system_id == system.id && s.faction == faction && s.behavior != SimBehavior::Dead)
            .filter(|s| s.fleet == 0 || ordinary.contains(&s.fleet))
            .map(|s| (s.faction, s.class))
            .collect()
    };
    // Not where you would see it arrive out of nothing.
    let unseen = |at: Vec2| you.is_none_or(|p| p.distance(at) >= ARRIVAL_RANGE);
    let stations: Vec<Vec2> = crate::world::home_base::station_sites(system).iter().map(|s| s.pos).collect();

    if let Some(owner) = system.faction {
        let flier = flier_of(owner);
        let here = living(sim, owner).len() + if flier != owner { living(sim, flier).len() } else { 0 };
        let target = owner_target(system);
        if (here as f32) < target as f32 * REFILL_BELOW {
            let template = faction_territories().into_iter().find(|t| t.faction == owner)?;
            // The caps count what is already here.
            let mut held = super::fleet::Held::new();
            for (faction, class) in living(sim, owner).into_iter().chain(living(sim, flier)) {
                super::fleet::hold(&mut held, super::fleet::role_of(faction, class));
            }
            let has_guard = sim.fleets.iter().any(|f| ordinary.contains(&f.id) && f.kind == FleetKind::StationGuard);
            let wants_guard = crate::diplomacy::stations::station_plan(owner).is_some() && !has_guard;
            let holdings = faction_holdings(system, rng);
            // Its station's guard first, if that is what it has lost.
            let guard = wants_guard
                .then(|| super::fleet::compose(owner, FleetKind::StationGuard, rng, &mut held))
                .flatten()
                .map(|hulls| (FleetKind::StationGuard, hulls));
            let (kind, hulls) = match guard {
                Some(guard) => guard,
                None => super::fleet::roll_fleet(owner, rng, &mut held, |k| !matches!(k, FleetKind::StationGuard | FleetKind::Convoy))?,
            };
            let at = jump_in_point(galaxy, system, Some(owner), rng);
            if !unseen(at) {
                return None;
            }
            let h = if kind == FleetKind::StationGuard { 0 } else { rng.gen_range(0..holdings.len()) };
            let home = holdings[h];
            let radius = if kind == FleetKind::StationGuard { GUARD_RADIUS } else { template.radius };
            let (mut fleet, members) = super::fleet::new_fleet(owner, kind, &hulls, system.id, at, home, radius, rng);
            fleet.destination = home;
            fleet.cruise = TRANSIT_CRUISE;
            let n = members.len();
            sim.add_fleet(fleet, members);
            sim.flash(system.id, at, true, owner);
            return Some(format!("{:?} sends a {:?} of {n} to {} ({here} of {target} left)", owner, kind, system.name));
        }
    }
    // A neighbour with nobody left here sends a patrol or raiders back.
    for neighbour in crate::celestial::territory::border_factions(galaxy, system.id) {
        if !living(sim, neighbour).is_empty() {
            continue;
        }
        let Some(template) = faction_territories().into_iter().find(|t| t.faction == neighbour) else { continue };
        let mut held = super::fleet::Held::new();
        let Some((kind, hulls)) = super::fleet::roll_fleet(neighbour, rng, &mut held, |k| matches!(k, FleetKind::Patrol | FleetKind::Raiders)) else { continue };
        let at = jump_in_point(galaxy, system, Some(neighbour), rng);
        if !unseen(at) {
            continue;
        }
        let edge = system.local_center + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(OPEN_HOLDING_RANGE);
        let home = clear_in_system(system, edge, HOLDING_CLEAR, &stations);
        let (mut fleet, members) = super::fleet::new_fleet(neighbour, kind, &hulls, system.id, at, home, template.radius * 1.5, rng);
        fleet.destination = home;
        fleet.cruise = TRANSIT_CRUISE;
        let n = members.len();
        sim.add_fleet(fleet, members);
        sim.flash(system.id, at, true, neighbour);
        return Some(format!("{:?} sends {:?} of {n} across the border into {}", neighbour, kind, system.name));
    }
    None
}

/// The boss that haunts a system, and its patrol: come in at the edge.
fn spawn_haunt(sim: &mut WorldSimulation, system: &crate::celestial::resources::StarSystemDef, you: Option<Vec2>, rng: &mut impl Rng) -> usize {
    use super::fleet::FleetKind;
    let Some(boss) = system.haunt else { return 0 };
    let stations: Vec<Vec2> = crate::world::home_base::station_sites(system).iter().map(|s| s.pos).collect();
    let mut sent = 0;
    for kind in [FleetKind::BattleGroup, FleetKind::Patrol] {
        let mut held = super::fleet::Held::new();
        let Some(hulls) = super::fleet::compose(boss, kind, rng, &mut held) else { continue };
        let edge = system.local_center + Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU)) * rng.gen_range(OPEN_HOLDING_RANGE);
        let home = arrival_point(system, you, clear_in_system(system, edge, HOLDING_CLEAR, &stations), HOLDING_CLEAR);
        let (mut fleet, members) =
            super::fleet::new_fleet(boss, kind, &hulls, system.id, home, system.local_center, OPEN_HOLDING_RANGE.end, rng);
        fleet.destination = patrol_waypoint(rng, system.local_center, fleet.patrol_radius);
        sent += members.len();
        sim.add_fleet(fleet, members);
        sim.flash(system.id, home, true, boss);
    }
    sent
}

/// Strike off what is lost for good and has waited out its time: fleets
/// with no ship left alive, and dead ships that flew alone. Not what is on
/// screen as a wreck with its record still wanted (`spawned`). Returns how
/// many ships went.
pub fn prune_the_lost(sim: &mut WorldSimulation) -> usize {
    let gone: std::collections::HashSet<u32> = sim
        .fleets
        .iter()
        .filter(|f| sim.ships.iter().filter(|s| s.fleet == f.id).all(|s| s.behavior == SimBehavior::Dead && s.fuel <= STRUCK_OFF_AT))
        .map(|f| f.id)
        .collect();
    let fleets: std::collections::HashSet<u32> = sim.fleets.iter().map(|f| f.id).collect();
    let before = sim.ships.len();
    sim.ships.retain(|s| {
        let lost_alone = !fleets.contains(&s.fleet) && s.behavior == SimBehavior::Dead && s.fuel <= STRUCK_OFF_AT;
        !(gone.contains(&s.fleet) || lost_alone)
    });
    sim.fleets.retain(|f| !gone.contains(&f.id));
    for id in gone {
        sim.out_of_sight.remove(&id);
    }
    before - sim.ships.len()
}

/// `pos` in `system`, moved off its star, out of every planet's path and
/// away from its stations: somewhere ships can be set down and left.
///
/// Fleets were placed clear of stations and of each other, and of nothing
/// else. A star is 40 to 150 km across and a planet sweeps its whole orbit
/// every few minutes.
fn clear_in_system(system: &crate::celestial::resources::StarSystemDef, pos: Vec2, margin: f32, stations: &[Vec2]) -> Vec2 {
    let clear = |at: Vec2| {
        let at = super::traffic::clear_point(system, at, margin);
        (at, clear_of_stations(at, stations) == at)
    };
    // Off a planet's path can be back toward a station, and off the station
    // back onto the path: a few turns of each, then further out from the
    // star until both are satisfied.
    let mut at = pos;
    for _ in 0..4 {
        let (settled, ok) = clear(at);
        if ok {
            return settled;
        }
        at = clear_of_stations(settled, stations);
    }
    let out = (pos - system.local_center).normalize_or(Vec2::X);
    for step in 1..200 {
        let (settled, ok) = clear(at + out * (step as f32 * 5_000.0));
        if ok {
            return settled;
        }
    }
    at
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
    let count = owner_target(system);
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
            // Out of tries: carried on outward until it's clear, never set
            // down through a formation.
            let anchor = settle_fleet(system, anchor, Vec2::from_angle(angle), reach, &placed_fleets);
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
            // Clear of the fleets already about, like any other: the reworked
            // capitals are big enough that 6 km out often sat in one.
            let reach = hulls.iter().map(|&(f, c)| super::spawner::design_extent(f, c)).fold(0.0_f32, f32::max)
                * (hulls.len() as f32 + 1.0)
                + 1_000.0;
            let at = settle_fleet(system, holdings[0] + Vec2::new(0.0, 6_000.0), Vec2::Y, reach, &placed_fleets);
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
    if safe_harbour(system) {
        return;
    }
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
            let home = clear_in_system(system, edge(&mut rng), HOLDING_CLEAR, &stations);
            let (mut fleet, members) =
                super::fleet::new_fleet(neighbour, kind, &hulls, system.id, home, home, template.radius * 1.5, &mut rng);
            if kind == FleetKind::Convoy {
                // A convoy crossing the border: through the system and out.
                let far = clear_in_system(system, edge(&mut rng), HOLDING_CLEAR, &stations);
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
    spawn_haunt(sim, system, None, &mut rng);
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
    holdings.into_iter().map(|h| clear_in_system(system, h, HOLDING_CLEAR, &stations)).collect()
}

/// `pos`, pushed straight out from any station it's within
/// `STATION_PICKET.start` of.
/// `at`, or the first spot past it along `dir` where a fleet of `reach` sits
/// clear of every fleet already placed.
fn clear_spot(mut at: Vec2, dir: Vec2, reach: f32, placed: &[(Vec2, f32)]) -> Vec2 {
    let dir = dir.normalize_or(Vec2::Y);
    for _ in 0..400 {
        if placed.iter().all(|&(c, r)| c.distance(at) > r + reach) {
            break;
        }
        at += dir * 1_000.0;
    }
    at
}

/// Where a fleet of `reach` wanted at `at` is set down: clear of every
/// fleet already placed, and of the star and every planet's path. Moving it
/// off a planet's path can put it onto another fleet, and off the fleet
/// back onto the path, so it is carried outward until both are satisfied.
fn settle_fleet(system: &crate::celestial::resources::StarSystemDef, at: Vec2, dir: Vec2, reach: f32, placed: &[(Vec2, f32)]) -> Vec2 {
    let roll = crate::celestial::galaxy::system_roll(system);
    let planets = roll.planet_paths();
    let keep = crate::celestial::poi::KeepClear { center: system.local_center, star_radius: roll.star_radius(), planets: &planets };
    let ok = |p: Vec2| keep.is_clear_by(p, reach) && placed.iter().all(|&(c, r)| c.distance(p) > r + reach);
    // Out from the star: past the last planet everything is clear, and the
    // fleets already placed come to an end.
    let out = (at - system.local_center).try_normalize().unwrap_or(dir.normalize_or(Vec2::Y));
    let mut p = at;
    for _ in 0..900 {
        if ok(p) {
            return p;
        }
        p += out * 1_500.0;
    }
    p
}

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
/// Room a ship flying alone off screen keeps from a star or a planet.
const LONE_PAD: f32 = 1_500.0;
/// The same for a fleet, past the reach of its formation.
const FLEET_PAD: f32 = 1_500.0;

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
/// its post, anyone else's next patrol leg. `harbour` is the station a guard
/// keeps (its centre and how far all of it reaches), where there is one.
fn next_fleet_waypoint(
    rng: &mut impl Rng,
    fleet: &mut super::fleet::SimFleet,
    anchors: &[(Vec2, f32)],
    harbour: Option<(Vec2, f32)>,
) -> Vec2 {
    use super::fleet::FleetKind;
    match fleet.kind {
        FleetKind::Convoy if !fleet.route.is_empty() => {
            fleet.leg = (fleet.leg + 1) % fleet.route.len();
            fleet.route[fleet.leg]
        }
        FleetKind::StationGuard => guard_waypoint(rng, fleet, harbour),
        _ => next_waypoint(rng, fleet.home_zone, fleet.patrol_radius, anchors),
    }
}

/// A guard's next leg: round the outside of the station it keeps, or
/// anywhere about its post where there is no station to go round.
fn guard_waypoint(rng: &mut impl Rng, fleet: &super::fleet::SimFleet, harbour: Option<(Vec2, f32)>) -> Vec2 {
    match harbour {
        Some((at, ring)) => {
            let (inner, outer) = guard_ring(ring, fleet.patrol_radius);
            ring_waypoint(rng, at, inner, outer)
        }
        None => patrol_waypoint(rng, fleet.home_zone, fleet.patrol_radius),
    }
}

/// Room a guard keeps between itself and the outermost part of its station.
const GUARD_CLEAR: f32 = 1_500.0;

/// The band a guard patrols round a station that reaches `ring` from its
/// centre: from just outside all of it, out to the guard's own range.
pub fn guard_ring(ring: f32, patrol_radius: f32) -> (f32, f32) {
    let inner = ring + GUARD_CLEAR;
    (inner, patrol_radius.max(inner + 0.5 * GUARD_RADIUS))
}

/// A point between `inner` and `outer` of `centre`.
///
/// A guard's legs were drawn from the whole disc round its station, hull
/// included: most of them were inside it.
fn ring_waypoint(rng: &mut impl Rng, centre: Vec2, inner: f32, outer: f32) -> Vec2 {
    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let dist = if outer > inner { rng.gen_range(inner..outer) } else { inner };
    centre + Vec2::from_angle(angle) * dist
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
    nav_bodies: Option<Res<super::nav::NavBodies>>,
    galaxy: Option<Res<crate::celestial::resources::GalaxyMap>>,
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
    // A Raft's `radius` is only its east arm; the rest of it reaches up to
    // twice as far, and a point taken off `radius` landed among its arms.
    let anchors: Vec<(Vec2, f32)> =
        stations.sites.iter().map(|s| (s.pos, crate::world::home_base::station_extent(s.index))).collect();

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

    // The star, planets and stations of each running system, to go round.
    let bodies_of = |system: u32| nav_bodies.as_deref().map_or(&[][..], |b| b.of(system));
    // How far each fleet's formation reaches from its middle.
    let mut fleet_reach: std::collections::HashMap<u32, f32> = Default::default();
    for ship in sim.ships.iter().filter(|s| s.fleet != 0) {
        let reach = fleet_reach.entry(ship.fleet).or_insert(0.0);
        *reach = reach.max(ship.slot.length());
    }

    // Fleets leaving the system make for their jump point, quickly, and go
    // when they reach it -- or at once, if nobody can see them.
    {
        let you_here = |system: u32, at: Vec2| {
            streaming.loaded_system == Some(system) && player_pos.is_some_and(|p| p.distance(at) <= super::preview::PREVIEW_RANGE)
        };
        let mut gone: Vec<(u32, u32, Vec2, AiShipType, bool)> = Vec::new();
        for fleet in sim.fleets.iter_mut() {
            let Some(point) = fleet.depart else { continue };
            fleet.destination = point;
            fleet.home_zone = point;
            fleet.patrol_radius = f32::MAX;
            fleet.cruise = fleet.cruise.max(TRANSIT_CRUISE);
            fleet.route.clear();
            let seen = you_here(fleet.system_id, fleet.position);
            if !live_fleets.contains(&fleet.id) && (!seen || fleet.position.distance(point) < DEPART_AT) {
                gone.push((fleet.id, fleet.system_id, fleet.position, fleet.faction, seen));
            }
        }
        for (id, system, at, faction, seen) in gone {
            if seen {
                sim.flash(system, at, false, faction);
            }
            sim.remove_fleet(id);
        }
        // Nothing stays hidden that isn't on its way out or through a jump.
        let leaving: std::collections::HashSet<u32> = sim
            .fleets
            .iter()
            .filter(|f| f.depart.is_some() || f.trade.as_ref().is_some_and(|t| t.leaving.is_some()))
            .map(|f| f.id)
            .collect();
        sim.out_of_sight.retain(|id| leaving.contains(id));
    }

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
        let bodies = bodies_of(ship.system_id);
        let remaining = ship.destination.distance(ship.position);
        // Arrived -- or the point is somewhere a planet has got to.
        if remaining <= ARRIVE_RADIUS || super::nav::in_a_body(ship.destination, LONE_PAD, bodies) {
            let next = next_waypoint(&mut rng, ship.home_zone, ship.patrol_radius, &anchors);
            ship.destination = super::nav::push_clear(next, LONE_PAD, bodies);
        }
        // Turn toward the way there rather than snapping to it, so a course
        // change reads as a turn; round a star or a planet, not through it.
        super::nav::sim_step(&mut ship.position, &mut ship.velocity, ship.destination, ship.cruise, LONE_PAD, bodies, dt);
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
    for fleet in sim.fleets.iter_mut() {
        if !is_active_system(fleet.system_id) || live_fleets.contains(&fleet.id) || !manned_fleets.contains(&fleet.id) {
            continue;
        }
        use super::fleet::FleetKind;
        // The station here, out to the last of its gun platforms.
        let harbour = faction_stations.harbour(fleet.system_id);
        if let Some((at, ring)) = harbour {
            match fleet.kind {
                FleetKind::StationGuard => {
                    fleet.home_zone = at;
                    // The larger of the two: a capital's home fleet ranges wider.
                    fleet.patrol_radius = fleet.patrol_radius.max(ring + GUARD_RADIUS);
                    // Posted before anyone knew where the station would be:
                    // take station there, out of sight, rather than crossing
                    // half a system to get to it.
                    let unseen = |p: Vec2| player_pos.map_or(true, |pp| pp.distance(p) > render_distance() * 1.5);
                    let post_at = at + Vec2::from_angle(fleet.heading) * (ring + 3_000.0);
                    if fleet.position.distance(at) > 30_000.0 && unseen(fleet.position) && unseen(post_at) {
                        fleet.position = post_at;
                        fleet.destination = guard_waypoint(&mut rng, fleet, harbour);
                    }
                    // The station rides its planet: a leg drawn round where
                    // it was is, a while later, a leg through its hull.
                    if fleet.destination.distance(at) < ring + GUARD_CLEAR * 0.5 {
                        fleet.destination = guard_waypoint(&mut rng, fleet, harbour);
                    }
                }
                FleetKind::Convoy if fleet.route.len() >= 2 => {
                    // The near end of the run is the station's berth, off its hull.
                    let berth = at + (fleet.route[1] - at).normalize_or(Vec2::X) * (ring + 4_000.0);
                    // The station sits by its planet: not a berth inside it.
                    let berth = super::nav::push_clear(berth, FLEET_PAD, bodies_of(fleet.system_id));
                    if fleet.leg == 0 {
                        fleet.destination = berth;
                    }
                    fleet.route[0] = berth;
                }
                _ => {}
            }
        }

        let bodies = bodies_of(fleet.system_id);
        let pad = fleet_reach.get(&fleet.id).copied().unwrap_or(0.0) + FLEET_PAD;
        let remaining = fleet.destination.distance(fleet.position);
        let trading = fleet.trade.is_some();
        if trading && remaining <= ARRIVE_RADIUS {
            // ai_ship::traffic decides where a trader goes from here.
            fleet.velocity = Vec2::ZERO;
            fleet.position = super::nav::push_clear(fleet.position, pad, bodies);
        } else {
            // Arrived -- or its point is somewhere a planet has got to.
            if !trading && (remaining <= ARRIVE_RADIUS || super::nav::in_a_body(fleet.destination, pad, bodies)) {
                // Come in by the jump and got where it was sent: from here
                // on it goes about at its own pace.
                if fleet.cruise == TRANSIT_CRUISE && fleet.mission == super::fleet::Mission::None && fleet.depart.is_none() {
                    fleet.cruise = super::fleet::cruise_for(fleet.kind, &mut rng);
                }
                let next = next_fleet_waypoint(&mut rng, fleet, &anchors, harbour);
                fleet.destination = super::nav::push_clear(next, pad, bodies);
            }
            super::nav::sim_step(&mut fleet.position, &mut fleet.velocity, fleet.destination, fleet.cruise, pad, bodies, dt);
        }
        if fleet.velocity.length() > 1.0 {
            fleet.heading = fleet.velocity.to_angle();
        }
        if !trading && fleet.position.distance(fleet.home_zone) > fleet.patrol_radius {
            fleet.destination = match fleet.kind {
                // A convoy strayed off its run heads for its next stop.
                FleetKind::Convoy if !fleet.route.is_empty() => fleet.route[fleet.leg],
                FleetKind::StationGuard if harbour.is_some() => guard_waypoint(&mut rng, fleet, harbour),
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

    // A harbour is kept clear of everyone but traders: the patrols and
    // raiders that wandered in before it was one, the strays filed under it.
    // Out of sight only; one you can see is left to go by itself.
    let system_of = |id: u32| galaxy.as_deref().and_then(|g| g.systems.iter().find(|s| s.id == id));
    if !crate::demo::skip_ai_ship_spawn() {
        let unseen = |system: u32, at: Vec2| {
            streaming.loaded_system != Some(system) || player_pos.is_none_or(|p| p.distance(at) > super::preview::PREVIEW_RANGE)
        };
        let leaving = clear_harbours(sim, |id| system_of(id).is_some_and(safe_harbour), unseen);
        if leaving > 0 {
            info!("SIM harbour: {leaving} ships that had no business there are gone");
        }
        // ...and one you can see is told to go, and flies out.
        if let Some(g) = galaxy.as_deref() {
            for fleet in sim.fleets.iter_mut() {
                let Some(system) = system_of(fleet.system_id).filter(|s| safe_harbour(s)) else { continue };
                if fleet.trade.is_none() && fleet.mission == super::fleet::Mission::None && fleet.depart.is_none() {
                    fleet.depart = Some(depart_point(g, system, fleet.position));
                }
            }
        }
    }

    // Phase 3: the lost. A fleet lost with all hands, and a ship that flew
    // alone, are struck off once their time is up; nothing gets up again
    // where it fell. A fleet that still has ships is sent a replacement for
    // a lost one after a good while, and only where nobody is looking. What
    // a system has lost comes back through the jump, a fleet at a time
    // (`refill_system`) -- and not at all in the dark, where what was taken
    // stays where it stopped.
    let dark = |id: u32| system_of(id).is_some_and(crate::narrative::front::is_taken);
    for ship in sim.ships.iter_mut().filter(|s| s.behavior == SimBehavior::Dead) {
        // The countdown (fuel doubles as the timer).
        ship.fuel -= 0.01;
    }
    prune_the_lost(sim);
    let unseen_fleet: std::collections::HashSet<u32> = sim
        .fleets
        .iter()
        .filter(|f| !live_fleets.contains(&f.id))
        .filter(|f| streaming.loaded_system != Some(f.system_id) || player_pos.is_none_or(|p| p.distance(f.position) > super::preview::PREVIEW_RANGE))
        .map(|f| f.id)
        .collect();
    let slots: std::collections::HashMap<u32, super::fleet::SimFleet> =
        sim.fleets.iter().filter(|f| unseen_fleet.contains(&f.id) && f.trade.is_none()).map(|f| (f.id, f.clone())).collect();
    for ship in sim.ships.iter_mut() {
        if ship.behavior != SimBehavior::Dead || ship.fuel > MENDED_AT || dark(ship.system_id) {
            continue;
        }
        let Some(fleet) = slots.get(&ship.fleet) else { continue };
        ship.position = fleet.slot_position(ship.slot);
        revive(ship);
    }

    // Phase 4: who needs sending for.
    if let Some(galaxy) = galaxy.as_deref() {
        let running: Vec<u32> = streaming.loaded_system.into_iter().chain(streaming.warm_systems.iter().copied()).collect();
        for id in running {
            let Some(system) = galaxy.systems.iter().find(|s| s.id == id) else { continue };
            if !sim.populated.contains(&id) || crate::demo::skip_ai_ship_spawn() {
                continue;
            }
            let you = player_pos.filter(|_| streaming.loaded_system == Some(id));
            // A boss killed in its haunt stays gone a long while.
            if let Some(boss) = system.haunt.filter(|_| !crate::narrative::front::is_taken(system)) {
                let about = sim.ships.iter().any(|s| s.system_id == id && s.faction == boss && s.behavior != SimBehavior::Dead);
                let gone = sim.boss_gone.entry(id).or_insert(0.0);
                *gone = if about { 0.0 } else { *gone + dt };
                if *gone >= BOSS_RETURNS {
                    *gone = 0.0;
                    let n = spawn_haunt(sim, system, you, &mut rng);
                    info!("REFILL system {id}: {boss:?} returns to its haunt, {n} ships");
                }
            }
            let clock = sim.refill_clock.entry(id).or_insert(0.0);
            *clock += dt;
            if *clock < REFILL_EVERY {
                continue;
            }
            *clock = 0.0;
            if let Some(what) = refill_system(sim, galaxy, system, you, &mut rng) {
                info!("REFILL system {id}: {what}");
            }
        }
    }
}

/// Take out of every harbour (`is_harbour`) whatever has no business in
/// one and can't be seen (`unseen`, by system and position): fleets that
/// are neither trading nor on a mission for or against you, and ships
/// flying alone. A ship with a bounty on it stays: someone has been sent
/// for it. Returns how many ships went.
pub fn clear_harbours(sim: &mut WorldSimulation, is_harbour: impl Fn(u32) -> bool, unseen: impl Fn(u32, Vec2) -> bool) -> usize {
    let wanted: std::collections::HashSet<u32> = sim.ships.iter().filter(|s| s.bounty_id.is_some()).map(|s| s.fleet).collect();
    let live: std::collections::HashSet<u32> = sim.ships.iter().filter(|s| s.spawned).map(|s| s.fleet).collect();
    let gone: std::collections::HashSet<u32> = sim
        .fleets
        .iter()
        .filter(|f| is_harbour(f.system_id) && f.trade.is_none() && f.mission == super::fleet::Mission::None)
        .filter(|f| !wanted.contains(&f.id) && !live.contains(&f.id) && unseen(f.system_id, f.position))
        .map(|f| f.id)
        .collect();
    let before = sim.ships.len();
    sim.ships.retain(|s| {
        let in_a_fleet_going = s.fleet != 0 && gone.contains(&s.fleet);
        let stray = s.fleet == 0 && is_harbour(s.system_id) && !s.spawned && s.bounty_id.is_none() && unseen(s.system_id, s.position);
        !(in_a_fleet_going || stray)
    });
    sim.fleets.retain(|f| !gone.contains(&f.id));
    before - sim.ships.len()
}

/// AFTERLIGHT_CENSUS=1: every ten seconds, what the simulation holds in each
/// system that is running -- ships, how many of them dead, fleets, and how
/// many of those are traders. The number to watch for a population that
/// creeps.
pub fn sim_census(
    time: Res<Time>,
    mut next: Local<f32>,
    sim: Res<WorldSimulation>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
) {
    if std::env::var("AFTERLIGHT_CENSUS").ok().as_deref() != Some("1") {
        return;
    }
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 10.0;
    for id in streaming.loaded_system.into_iter().chain(streaming.warm_systems.iter().copied()) {
        let ships = sim.ships.iter().filter(|s| s.system_id == id);
        let (all, dead, live) = ships.fold((0, 0, 0), |(a, d, l), s| {
            (a + 1, d + usize::from(s.behavior == SimBehavior::Dead), l + usize::from(s.spawned))
        });
        let fleets = sim.fleets.iter().filter(|f| f.system_id == id).count();
        let traders = sim.fleets.iter().filter(|f| f.system_id == id && f.trade.is_some()).count();
        let here = if streaming.loaded_system == Some(id) { " (here)" } else { "" };
        info!("SIM system={id}{here} ships={all} dead={dead} on_screen={live} fleets={fleets} traders={traders}");
    }
    info!("SIM total ships={} fleets={} systems populated={}", sim.ships.len(), sim.fleets.len(), sim.populated.len());
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
/// What leaves the screen together: a fleet whole, or a ship flying alone.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Unit {
    Fleet(u32),
    Ship(Entity),
}

/// Which of the ships on screen (`by_far`: unit, nearest member's distance,
/// blocks; farthest first) to send back to make `need` blocks of room for a
/// fleet waiting `waiting` away: only ones farther than it by EVICT_MARGIN
/// and at least EVICT_CLEAR from you. What they free, and which.
fn make_room(by_far: &[(Unit, f32, usize)], taken: &std::collections::HashSet<Unit>, waiting: f32, need: usize) -> (usize, Vec<Unit>) {
    let mut freed = 0;
    let mut chosen = Vec::new();
    for &(unit, dist, cost) in by_far {
        if freed >= need || dist <= waiting + EVICT_MARGIN || dist < EVICT_CLEAR {
            break;
        }
        if taken.contains(&unit) {
            continue;
        }
        chosen.push(unit);
        freed += cost;
    }
    (freed, chosen)
}

pub fn sync_simulation_entities(
    mut commands: Commands,
    mut sim: ResMut<WorldSimulation>,
    registry: Res<ModuleRegistry>,
    asset_server: Res<AssetServer>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    ship_query: Query<&Transform, With<Ship>>,
    // Where a charging warp comes out: ships there are brought in ahead, so
    // they're already flying when you arrive (celestial::warp's preload).
    preload: Option<Res<crate::celestial::resources::WarpPreload>>,
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
    let ahead = preload.as_ref().and_then(|p| p.arrival);
    // Distance to you, or to where you're about to be.
    let near = |p: Vec2| ahead.map_or(p.distance(player_pos), |a| p.distance(player_pos).min(p.distance(a)));
    // The same for a ship the simulation holds, which is only near you if it
    // is in the system you are in (or the one you are about to come out in).
    // Every system shares one set of coordinates, their middles as little as
    // 200 km apart and their fleets ranging 450 km out: a neighbour's patrol
    // could come on screen in the wrong system, with no far picture first.
    let here = streaming.loaded_system;
    let ahead_in = preload.as_ref().and_then(|p| p.system).or(here);
    let near_in = |system: u32, p: Vec2| -> f32 {
        // In uncharted space there is no system to be in: by distance alone.
        let Some(here) = here else { return near(p) };
        let to_you = if system == here { p.distance(player_pos) } else { f32::MAX };
        match ahead {
            Some(a) if ahead_in == Some(system) => to_you.min(p.distance(a)),
            _ => to_you,
        }
    };
    let block_count = |children: Option<&Children>| {
        children.map_or(0, |c| c.iter().filter(|e| blocks.contains(*e)).count())
    };
    let sim: &mut WorldSimulation = &mut sim;
    let mut rng = rand::thread_rng();
    // Fleets going off screen this frame on purpose (see `fold_back`).
    let folding: std::collections::HashSet<u32> = sim.fold_back.drain(..).collect();
    sim.out_of_sight.extend(folding.iter().copied());

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
        if s.spawned || s.behavior == SimBehavior::Dead || (s.fleet != 0 && sim.out_of_sight.contains(&s.fleet)) {
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
        let d = if live_fleets.contains(&fleet) { 0.0 } else { near_in(s.system_id, s.position) };
        group.dist = group.dist.min(d);
    }
    groups.retain(|g| g.dist < render_distance());
    groups.sort_by(|a, b| a.dist.total_cmp(&b.dist));

    // What's on screen, as the units that leave it: a fleet whole, or a ship
    // that flies alone. Nearest member, and blocks.
    let mut units: std::collections::HashMap<Unit, (f32, usize)> = std::collections::HashMap::new();
    for (entity, tf, _, state, _, _, _, (_, mirror, children, member)) in ai_ships.iter() {
        if state.is_destroyed || mirror {
            continue;
        }
        let unit = member.map_or(Unit::Ship(entity), |m| Unit::Fleet(m.fleet));
        let e = units.entry(unit).or_insert((f32::MAX, 0));
        e.0 = e.0.min(near(tf.translation.truncate()));
        e.1 += block_count(children);
    }
    let mut by_far: Vec<(Unit, f32, usize)> = units.into_iter().map(|(u, (d, c))| (u, d, c)).collect();
    by_far.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut evicted: std::collections::HashSet<Unit> = std::collections::HashSet::new();

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
            // Room for it: the farthest ships on screen, well clear of you
            // and farther than it, go back into the simulation. A capital's
            // battle group a few kilometres off used to wait for good behind
            // a scatter of small ships at the edge of sight.
            let need = live_blocks + group.cost - max_live_ai_blocks();
            let (freed, chosen) = make_room(&by_far, &evicted, group.dist, need);
            if freed < need {
                break;
            }
            evicted.extend(chosen);
            live_blocks -= freed;
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
            !state.is_destroyed && near(tf.translation.truncate()) <= despawn_distance()
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
        // Sent back to make room for a nearer fleet, or gone out of range.
        let made_room = evicted.contains(&member.map_or(Unit::Ship(entity), |m| Unit::Fleet(m.fleet)))
            || member.is_some_and(|m| folding.contains(&m.fleet));
        if !made_room && near(pos) <= despawn_distance() {
            continue;
        }
        if !made_room && member.is_some_and(|m| near_fleets.contains(&m.fleet)) {
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
        // Gone in a flash, where you were watching it.
        if folding.contains(&fleet_id) {
            sim.flash(fleet.system_id, centre, false, fleet.faction);
        }
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

/// How many legs a guard's circuit round its station has. Enough that the
/// straight line between two of them stays outside the station: with four,
/// each leg cut in to 0.7 of the circuit's radius, through the hull.
const GUARD_LEGS: usize = 8;
/// A station that has moved this far has left its guard's circuit behind.
const GUARD_DRIFT: f32 = 400.0;

/// A guard's circuit on screen: `GUARD_LEGS` points round a station at `at`
/// that reaches `ring` from its centre, every leg between them clear of it.
/// Turned by the fleet's id, so two guards don't fly the same corners.
pub fn guard_circuit(at: Vec2, ring: f32, patrol_radius: f32, id: u32) -> Vec<Vec2> {
    use std::f32::consts::{PI, TAU};
    let (inner, outer) = guard_ring(ring, patrol_radius);
    // The middle of a leg is this much nearer the centre than its ends.
    let sag = (PI / GUARD_LEGS as f32).cos();
    let radius = (inner / sag).max(inner + 0.4 * (outer - inner));
    let turn = (id as f32 * 0.618_034).fract() * TAU;
    (0..GUARD_LEGS).map(|i| at + Vec2::from_angle(turn + TAU * i as f32 / GUARD_LEGS as f32) * radius).collect()
}

/// Keep a guard that is on screen flying round its station where the
/// station is now.
///
/// Its circuit was laid out once, when it came on screen, round the point
/// the station was at -- and the station rides its planet at 150 u/s. A few
/// minutes later the guard was patrolling a patch of empty space, or the
/// station had moved onto its circuit.
pub fn hold_guard_posts(
    time: Res<Time>,
    mut next: Local<f32>,
    sim: Res<WorldSimulation>,
    stations: Res<crate::diplomacy::stations::FactionStations>,
    mut leaders: Query<(&mut AiShipNav, &super::fleet::FleetMember)>,
    nav_bodies: Option<Res<super::nav::NavBodies>>,
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 1.0;
    let bodies: &[super::nav::NavBody] = nav_bodies.as_deref().map_or(&[], |b| b.loaded.as_slice());
    for (mut nav, member) in leaders.iter_mut().filter(|(_, m)| m.leader) {
        let Some(fleet) = sim.fleet(member.fleet) else { continue };
        if fleet.kind != super::fleet::FleetKind::StationGuard || fleet.trade.is_some() {
            continue;
        }
        let Some((at, ring)) = stations.harbour(fleet.system_id) else { continue };
        // The station sits by its planet, and some of the way round it is
        // planet: those corners are taken at its surface.
        let want: Vec<Vec2> = guard_circuit(at, ring, fleet.patrol_radius, fleet.id)
            .into_iter()
            .map(|corner| super::nav::push_clear(corner, FLEET_PAD + 1_000.0, bodies))
            .collect();
        let stale = nav.waypoints.len() != want.len() || nav.waypoints.iter().zip(&want).any(|(a, b)| a.distance(*b) > GUARD_DRIFT);
        if !stale {
            continue;
        }
        // On its way to a corner of the circuit: to where that corner is now.
        let on_a_leg = nav.waypoints.get(nav.current_waypoint).zip(nav.destination).is_some_and(|(w, d)| d.distance(*w) < 1.0);
        nav.current_waypoint %= want.len();
        if on_a_leg || nav.destination.is_none() {
            nav.destination = Some(want[nav.current_waypoint]);
        }
        nav.waypoints = want;
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
    streaming: Option<Res<crate::celestial::resources::SystemStreamingManager>>,
    player: Query<&Transform, With<Ship>>,
) {
    use super::fleet::FleetKind;
    let sim: &mut WorldSimulation = &mut sim;
    let mut rng = rand::thread_rng();
    let loaded = streaming.as_deref().and_then(|s| s.loaded_system);
    let you = player.single().ok().map(|t| t.translation.truncate());
    for flip in flips.read() {
        if !sim.populated.contains(&flip.system_id) {
            continue;
        }
        let Some(lost) = galaxy.systems.iter().find(|s| s.id == flip.system_id) else { continue };
        // Nobody garrisons the dark.
        if crate::narrative::front::is_taken(lost) {
            continue;
        }
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
            // Not out of nothing in front of you: in sight, they come in.
            let here = loaded == Some(flip.system_id);
            let at = arrival_point(lost, you.filter(|_| here), at, 2_000.0);
            if here {
                sim.flash(flip.system_id, at, true, flip.to);
            }
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
            heard: Default::default(),
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

    /// A guard's legs, off screen and on, keep outside its station: every
    /// point drawn, and every straight run between two corners on screen.
    #[test]
    fn a_guard_goes_round_its_station_not_through_it() {
        let mut rng = rand::thread_rng();
        let at = Vec2::new(40_000.0, -12_000.0);
        // A small post, a capital's city, and a home fleet's wider range.
        for (ring, patrol) in [(4_000.0, 12_000.0), (26_000.0, 34_000.0), (9_000.0, 14_000.0), (30_000.0, 14_000.0)] {
            let (inner, outer) = guard_ring(ring, patrol);
            assert!(inner > ring && outer > inner);
            for _ in 0..200 {
                let d = ring_waypoint(&mut rng, at, inner, outer).distance(at);
                assert!(d >= inner - 1.0 && d <= outer + 1.0, "{d} outside {inner}..{outer}");
            }
            for id in [1, 2, 77] {
                let circuit = guard_circuit(at, ring, patrol, id);
                assert_eq!(circuit.len(), GUARD_LEGS);
                for (i, a) in circuit.iter().enumerate() {
                    let b = circuit[(i + 1) % circuit.len()];
                    let nearest = ((*a + b) * 0.5).distance(at);
                    assert!(nearest >= ring + GUARD_CLEAR - 1.0, "ring {ring}: a leg passes {nearest:.0} from the middle");
                }
            }
        }
        // Two guards on one station don't fly the same corners.
        assert!(guard_circuit(at, 4_000.0, 12_000.0, 1)[0].distance(guard_circuit(at, 4_000.0, 12_000.0, 2)[0]) > 1_000.0);
    }

    fn a_fleet(sim: &mut WorldSimulation, faction: AiShipType, system: u32, at: Vec2, n: usize) -> u32 {
        let fleet = super::super::fleet::SimFleet::new(super::super::fleet::FleetKind::Patrol, faction, system, at, at, 9_000.0, 80.0);
        let members = (0..n).map(|_| SimulatedShip::patrolling(system, faction, at, at, 9_000.0, 80.0)).collect();
        sim.add_fleet(fleet, members)
    }

    /// What is lost with all hands is struck off once its time is up, and
    /// nothing gets up again where it fell: a fleet whole, a ship that flew
    /// alone. A fleet with a ship left stays, and so does one lost a moment ago.
    #[test]
    fn the_lost_are_struck_off_not_brought_back() {
        let mut sim = WorldSimulation::default();
        let lost = a_fleet(&mut sim, AiShipType::MerchantPrinces, 3, Vec2::ZERO, 3);
        let hurt = a_fleet(&mut sim, AiShipType::TerranHegemony, 3, Vec2::ZERO, 3);
        let fresh = a_fleet(&mut sim, AiShipType::MerchantPrinces, 3, Vec2::ZERO, 2);
        let navy = a_fleet(&mut sim, AiShipType::TerranHegemony, 3, Vec2::ZERO, 2);
        sim.fleets.iter_mut().find(|f| f.id == lost).unwrap().trade = Some(Default::default());
        for s in sim.ships.iter_mut() {
            s.behavior = SimBehavior::Dead;
            // Long dead, except the convoy lost a moment ago.
            s.fuel = if s.fleet == fresh { 0.4 } else { STRUCK_OFF_AT - 0.1 };
        }
        // One of the hurt fleet is alive.
        sim.ships.iter_mut().find(|s| s.fleet == hurt).unwrap().behavior = SimBehavior::Patrolling;
        // Two that flew alone: one long dead, one alive.
        let mut gone = SimulatedShip::patrolling(3, AiShipType::RecursiveKingdom, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0);
        gone.behavior = SimBehavior::Dead;
        gone.fuel = STRUCK_OFF_AT - 0.1;
        sim.add(gone);
        sim.add(SimulatedShip::patrolling(3, AiShipType::RecursiveKingdom, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0));
        sim.out_of_sight.insert(lost);
        assert_eq!(prune_the_lost(&mut sim), 3 + 2 + 1, "the convoy, the navy's fleet and the lone ship");
        assert!(sim.fleet(lost).is_none() && sim.fleet(navy).is_none());
        assert!(sim.fleet(hurt).is_some() && sim.ships.iter().filter(|s| s.fleet == hurt).count() == 3);
        assert!(sim.fleet(fresh).is_some());
        assert_eq!(sim.ships.iter().filter(|s| s.fleet == 0).count(), 1);
        assert!(sim.out_of_sight.is_empty());
        assert_eq!(prune_the_lost(&mut sim), 0);
    }

    /// Wherever something is wanted, it is not put in front of you: within
    /// sight it comes in `ARRIVAL_RANGE` off, on that side, and never in a
    /// star or on a planet's path.
    #[test]
    fn nothing_arrives_in_front_of_you() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        for system in galaxy.systems.iter().take(8) {
            let roll = crate::celestial::galaxy::system_roll(system);
            let planets = roll.planet_paths();
            let keep = crate::celestial::poi::KeepClear { center: system.local_center, star_radius: roll.star_radius(), planets: &planets };
            let you = crate::world::home_base::station_sites(system)[0].pos + Vec2::new(4_000.0, 0.0);
            for i in 0..12 {
                let side = Vec2::from_angle(std::f32::consts::TAU * i as f32 / 12.0);
                // By a wreck 2 km from you; 12 km off; and well out of sight.
                for off in [2_000.0, 12_000.0, 160_000.0] {
                    let at = arrival_point(system, Some(you), you + side * off, 2_000.0);
                    assert!(at.distance(you) >= ARRIVAL_RANGE - 1.0, "{}: {:.0} km from you", system.name, at.distance(you) / 1000.0);
                    assert!(keep.is_clear(at), "{}: arrived in something's way", system.name);
                    if off < ARRIVAL_RANGE {
                        assert!(at.distance(you) < ARRIVAL_RANGE * 3.0, "{}: sent {:.0} km off", system.name, at.distance(you) / 1000.0);
                    }
                }
            }
            // With nobody to see, where it was wanted (or the nearest clear spot).
            let want = system.local_center + Vec2::new(300_000.0, 20_000.0);
            assert!(keep.is_clear(arrival_point(system, None, want, 2_000.0)));
        }
    }

    /// A system that has lost ships is sent a fleet, through the jump, a
    /// fleet at a time. A full one is sent nothing; nor is the dark, nor a
    /// harbour, nor a jump point you are sitting on.
    #[test]
    fn a_system_that_has_lost_ships_is_sent_a_fleet_through_the_jump() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let mut rng = rand::thread_rng();
        let system = galaxy.systems.iter().find(|s| s.faction == Some(AiShipType::TerranHegemony) && !s.capital && s.haunt.is_none()).unwrap();
        let mut sim = WorldSimulation::default();
        ensure_system_population(&mut sim, &galaxy, system.id);
        assert_eq!(refill_system(&mut sim, &galaxy, system, None, &mut rng), None, "a full system was sent more");

        // All but two of the navy's fleets are lost.
        let theirs: Vec<u32> = sim.fleets.iter().filter(|f| f.faction == AiShipType::TerranHegemony).map(|f| f.id).collect();
        for id in theirs.iter().skip(2) {
            sim.remove_fleet(*id);
        }
        let left = sim.ships.iter().filter(|s| s.faction == AiShipType::TerranHegemony).count();
        assert!((left as f32) < owner_target(system) as f32 * REFILL_BELOW, "{left} of {} left: not a loss to test with", owner_target(system));
        let (fleets, ships) = (sim.fleets.len(), sim.ships.len());
        // Sitting on every way in: nothing comes.
        let on_the_jump = jump_in_point(&galaxy, system, Some(AiShipType::TerranHegemony), &mut rng);
        assert_eq!(refill_system(&mut sim, &galaxy, system, Some(on_the_jump), &mut rng), None);
        assert_eq!(sim.fleets.len(), fleets);

        let what = refill_system(&mut sim, &galaxy, system, None, &mut rng);
        assert!(what.is_some(), "nothing was sent to a system with {left} of {} left", owner_target(system));
        assert_eq!(sim.fleets.len(), fleets + 1, "one fleet at a time");
        assert!(sim.ships.len() > ships);
        let new = sim.fleets.last().unwrap();
        assert_eq!(new.faction, AiShipType::TerranHegemony);
        assert_eq!(new.cruise, TRANSIT_CRUISE);
        assert!(new.trade.is_none() && new.kind != super::super::fleet::FleetKind::Convoy);
        let roll = crate::celestial::galaxy::system_roll(system);
        assert!(new.position.distance(system.local_center) > roll.star_radius(), "it came in inside the star");
        assert!(new.position.distance(new.destination) > 10_000.0, "it has somewhere to go");
        assert_eq!(sim.jumps.len(), 1);
        assert!(sim.jumps[0].arriving && sim.jumps[0].at == new.position);

        // The dark gets nobody, and neither does Haven.
        let mut dark = galaxy.systems.clone();
        let taken = dark.iter_mut().find(|s| s.id == system.id).unwrap();
        taken.dark = crate::narrative::front::TAKEN;
        let taken = taken.clone();
        assert_eq!(refill_system(&mut sim, &galaxy, &taken, None, &mut rng), None);
        assert_eq!(refill_system(&mut sim, &galaxy, &galaxy.systems[0], None, &mut rng), None);
    }

    /// Haven is a harbour: what has no business there and can't be seen
    /// goes. Traders stay, so does anything sent for or with you, anything
    /// with a bounty on it, and anything you can see.
    #[test]
    fn a_harbour_is_cleared_of_everyone_but_traders() {
        let mut sim = WorldSimulation::default();
        let raiders = a_fleet(&mut sim, AiShipType::RecursiveKingdom, 0, Vec2::new(300_000.0, 0.0), 3);
        let in_sight = a_fleet(&mut sim, AiShipType::RecursiveKingdom, 0, Vec2::new(20_000.0, 0.0), 2);
        let trader = a_fleet(&mut sim, AiShipType::MerchantPrinces, 0, Vec2::new(300_000.0, 0.0), 2);
        let hunters = a_fleet(&mut sim, AiShipType::CorpseStars, 0, Vec2::new(300_000.0, 0.0), 2);
        let wanted = a_fleet(&mut sim, AiShipType::BrokenChoir, 0, Vec2::new(300_000.0, 0.0), 1);
        let elsewhere = a_fleet(&mut sim, AiShipType::RecursiveKingdom, 5, Vec2::new(300_000.0, 0.0), 2);
        sim.fleets.iter_mut().find(|f| f.id == trader).unwrap().trade = Some(Default::default());
        sim.fleets.iter_mut().find(|f| f.id == hunters).unwrap().mission = super::super::fleet::Mission::Hunt { left: 100.0 };
        sim.ships.iter_mut().find(|s| s.fleet == wanted).unwrap().bounty_id = Some(7);
        // A stray flying alone, far off.
        sim.add(SimulatedShip::patrolling(0, AiShipType::RecursiveKingdom, Vec2::new(250_000.0, 0.0), Vec2::ZERO, 6_000.0, 80.0));
        let you = Vec2::ZERO;
        let gone = clear_harbours(&mut sim, |id| id == 0, |_, at| at.distance(you) > 105_000.0);
        assert_eq!(gone, 4, "the raiders and the stray");
        assert!(sim.fleet(raiders).is_none());
        for kept in [in_sight, trader, hunters, wanted, elsewhere] {
            assert!(sim.fleet(kept).is_some(), "fleet {kept} went");
        }
        assert!(!sim.ships.iter().any(|s| s.fleet == 0));
    }

    /// Haven gets no visitors; any other system the same neighbours border does.
    #[test]
    fn a_harbour_gets_no_visitors() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let neighbours = [AiShipType::RecursiveKingdom, AiShipType::BrokenChoir];
        let mut sim = WorldSimulation::default();
        spawn_visitors(&mut sim, &galaxy.systems[0], &neighbours);
        assert!(sim.ships.is_empty() && sim.fleets.is_empty(), "{} ships came to Haven", sim.ships.len());
        spawn_visitors(&mut sim, &galaxy.systems[4], &neighbours);
        assert!(!sim.fleets.is_empty());
    }

    /// A system is populated once, by the list and not by who happens to be
    /// in it: fleets that fell back into it don't count as its population,
    /// and losing its last ship doesn't bring a new population.
    #[test]
    fn a_system_is_populated_once() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let system = galaxy.systems.iter().find(|s| s.faction == Some(AiShipType::TerranHegemony)).unwrap();
        let mut sim = WorldSimulation::default();
        // Someone else's fleet has fallen back into it before anyone came.
        a_fleet(&mut sim, AiShipType::BrokenChoir, system.id, system.local_center, 2);
        ensure_system_population(&mut sim, &galaxy, system.id);
        let first = sim.ships.len();
        assert!(first > 10, "the refugees were taken for its population: {first} ships");
        // Everything in it is lost; coming back does not refill it at a stroke.
        sim.ships.clear();
        sim.fleets.clear();
        ensure_system_population(&mut sim, &galaxy, system.id);
        assert!(sim.ships.is_empty());
        // A save keeps the list; an old save, with none, has it worked out.
        let saved = sim.snapshot();
        assert_eq!(saved.populated, vec![system.id]);
        let mut old = SavedWorld::default();
        old.ships.push(SimulatedShip::patrolling(9, AiShipType::TerranHegemony, Vec2::ZERO, Vec2::ZERO, 1.0, 1.0));
        let mut loaded = WorldSimulation::default();
        loaded.restore(&old);
        assert!(loaded.populated.contains(&9) && loaded.populated.len() == 1);
    }

    /// Fleets and holdings are set down clear of the star and of every
    /// planet's path, across the galaxy.
    #[test]
    fn nothing_is_set_down_in_a_star_or_on_a_planets_path() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let mut rng = rand::thread_rng();
        for system in galaxy.systems.iter().filter(|s| s.faction.is_some()) {
            let roll = crate::celestial::galaxy::system_roll(system);
            let planets = roll.planet_paths();
            let keep = crate::celestial::poi::KeepClear { center: system.local_center, star_radius: roll.star_radius(), planets: &planets };
            for h in faction_holdings(system, &mut rng) {
                assert!(keep.is_clear_by(h, HOLDING_CLEAR - 1.0), "{}: a holding at {h} is in something's way", system.name);
            }
            let mut sim = WorldSimulation::default();
            spawn_system_faction_population(&mut sim, system);
            for f in &sim.fleets {
                assert!(keep.is_clear(f.position), "{}: a fleet set down at {} is in something's way", system.name, f.position);
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

    /// Every system shares one set of coordinates. A ship of a neighbouring
    /// system that happens to be where you are is not here; one of the
    /// system you are about to come out in, at the point you'll come out, is.
    #[test]
    fn only_the_ships_of_the_system_you_are_in_come_on_screen() {
        let mut app = app();
        app.init_asset::<Image>();
        app.world_mut().resource_mut::<crate::celestial::resources::SystemStreamingManager>().loaded_system = Some(4);
        let near = Vec2::new(6_000.0, 0.0);
        let arrival = Vec2::new(900_000.0, 0.0);
        {
            let mut sim = app.world_mut().resource_mut::<WorldSimulation>();
            sim.add(SimulatedShip::patrolling(7, AiShipType::RecursiveKingdom, near, near, 500.0, 0.0));
            sim.add(SimulatedShip::patrolling(7, AiShipType::RecursiveKingdom, arrival, arrival, 500.0, 0.0));
        }
        for _ in 0..5 {
            app.update();
        }
        let live = |app: &mut App| app.world_mut().query::<&AiShipType>().iter(app.world()).count();
        assert_eq!(live(&mut app), 0, "a neighbouring system's ship came on screen in this one");
        // A ship of this system at the same spot does.
        app.world_mut().resource_mut::<WorldSimulation>().add(SimulatedShip::patrolling(4, AiShipType::RecursiveKingdom, near, near, 500.0, 0.0));
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(live(&mut app), 1);
        // Charging a jump to system 7: its ship at the arrival point is
        // brought in ahead; its other one, near where you are now, still isn't.
        app.insert_resource(crate::celestial::resources::WarpPreload { system: Some(7), arrival: Some(arrival), ..default() });
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(live(&mut app), 2, "the ship where you are about to come out was not brought in");
    }

    #[test]
    fn room_is_made_only_from_far_ships_farther_than_the_fleet_waiting() {
        let far = |d: f32, c: usize, n: u32| (Unit::Fleet(n), d, c);
        // Farthest first, as the spawner sorts them.
        let on_screen = [far(45_000.0, 600, 1), far(30_000.0, 900, 2), far(25_000.0, 800, 3), far(12_000.0, 3_000, 4)];
        let none = std::collections::HashSet::new();
        // Enough from the two farthest.
        let (freed, chosen) = make_room(&on_screen, &none, 8_000.0, 1_200);
        assert_eq!(freed, 1_500);
        assert_eq!(chosen, vec![Unit::Fleet(1), Unit::Fleet(2)]);
        // Never one nearer than the fleet waiting, nor one close to you.
        let (freed, _) = make_room(&on_screen, &none, 32_000.0, 5_000);
        assert_eq!(freed, 600, "only what's farther than the fleet waiting");
        let (freed, chosen) = make_room(&on_screen, &none, 1_000.0, 10_000);
        assert!(!chosen.contains(&Unit::Fleet(4)), "the ship 12 km off is in reach of a fight");
        assert_eq!(freed, 2_300);
    }

    /// Two fleets about as far off as each other don't trade places: the one
    /// waiting has to be well nearer than the one it would send back.
    #[test]
    fn fleets_about_as_far_off_do_not_trade_places() {
        let none = std::collections::HashSet::new();
        let on_screen = [(Unit::Fleet(1), 31_000.0, 9_000)];
        let (freed, _) = make_room(&on_screen, &none, 26_000.0, 5_000);
        assert_eq!(freed, 0, "5 km nearer is not reason enough");
        let (freed, chosen) = make_room(&on_screen, &none, 15_000.0, 5_000);
        assert_eq!((freed, chosen), (9_000, vec![Unit::Fleet(1)]), "16 km nearer is");
    }

    /// A capital's battle group a few km off comes on screen even with small
    /// ships scattered at the edge of sight: they make room for it.
    #[test]
    fn a_capital_near_you_comes_on_screen_past_a_scatter_of_small_ships() {
        let mut app = app();
        app.init_asset::<Image>();
        let small = super::super::spawner::design_blocks(AiShipType::RecursiveKingdom, None);
        let capital = super::super::spawner::design_blocks(AiShipType::TerranHegemony, Some("sovereign"));
        // Fill most of the budget with small ships 35 km out.
        let n = (MAX_LIVE_AI_BLOCKS - capital / 2) / small;
        {
            let mut sim = app.world_mut().resource_mut::<WorldSimulation>();
            for i in 0..n {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                let at = Vec2::from_angle(a) * 35_000.0;
                sim.add(SimulatedShip::patrolling(0, AiShipType::RecursiveKingdom, at, at, 500.0, 0.0));
            }
        }
        for _ in 0..20 {
            app.update();
        }
        let live = |app: &mut App, f: AiShipType| app.world_mut().query::<&AiShipType>().iter(app.world()).filter(|t| **t == f).count();
        assert_eq!(live(&mut app, AiShipType::RecursiveKingdom), n, "the small ships came first");
        // Now a capital 8 km off.
        {
            let mut sim = app.world_mut().resource_mut::<WorldSimulation>();
            let mut ship = SimulatedShip::patrolling(0, AiShipType::TerranHegemony, Vec2::new(8_000.0, 0.0), Vec2::new(8_000.0, 0.0), 500.0, 0.0);
            ship.class = Some("sovereign");
            sim.add(ship);
        }
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(live(&mut app, AiShipType::TerranHegemony), 1, "the capital never came on screen");
        let left = live(&mut app, AiShipType::RecursiveKingdom);
        assert!(left < n, "nothing made room");
        let blocks = left * small + capital;
        assert!(blocks <= MAX_LIVE_AI_BLOCKS, "over the budget: {blocks}");
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
