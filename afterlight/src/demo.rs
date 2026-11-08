use bevy::prelude::*;
use bevy::render::view::window::screenshot::{save_to_disk, Screenshot};
use crate::components::{Ship, ShipPhysics};
use crate::resources::{BuildCategory, BuildingState, InputState};
use crate::states::{BuildState, GameState};
use crate::ai_ship::components::{AiShip, WorldSimulation, SimBehavior};
use crate::combat::targeting::selection::{TargetSelection, TargetType};
use crate::combat::targeting::fire_groups::FireGroupState;

// ============================================================================
// DEMO / SELF-PLAYTEST MODE — dev tooling, not a game feature.
// Three independent env-gated modes:
//   AFTERLIGHT_DEMO=1       — full autopilot: skips the menu, flies toward and
//                          fires at the nearest enemy, saves periodic engine
//                          screenshots. Used for unattended log-based testing.
//   AFTERLIGHT_SKIP_MENU=1  — skips the menu/station sequence only. Drops
//                          straight into Exploring with the starter ship,
//                          the normal 37-ship world simulation, and full
//                          manual control (no autopilot).
//   AFTERLIGHT_SHOTS=<n>    — engine-side screenshot every n seconds during
//                          NORMAL play: no autopilot, no menu skip, nothing
//                          else changed. F7 takes one on demand. Captures the
//                          render target, not the display, so it never
//                          photographs whatever app happens to be in front.
//                          It CANNOT see through a window that is fully
//                          covered: macOS stops that window drawing, and the
//                          frames come back black. Use AFTERLIGHT_OFFSCREEN for
//                          anything run in the background.
//                          Dir: AFTERLIGHT_SHOTS_DIR (default /tmp/afterlight_shots).
//   AFTERLIGHT_OFFSCREEN=1  — render into an image instead of the window, so
//                          capture works however buried the window is. The
//                          window itself shows nothing; for unattended runs.
//   AFTERLIGHT_START_SYSTEM=<id|Faction|haunt>[:fleet|:capital|:approach] — jump there a few
//                          seconds in (:approach parks ~12 km out from the capital's
//                          starport, the station between you and its world).
//   AFTERLIGHT_SHOW_SHIPS=1 — every AI ship drawn whole (no hidden interior), with the
//                          zoomed-out roof the player's ship has; faction stations
//                          in their textures' own colours, untinted.
//   AFTERLIGHT_FLY_TO=station — autopilot to the nearest station: turn, burn, brake,
//                          press F. For filming an approach and docking.
//   AFTERLIGHT_ZOOM=<z> — hold the camera at that zoom (1.8 default, 90 furthest out).
//   AFTERLIGHT_ZOOM_RAMP=<from>:<to>:<secs>[:<delay>] — ease the zoom from one value
//                          to another over <secs> of real time, starting
//                          <delay> real seconds after launch. For filming.
//   AFTERLIGHT_HIDE_UI=1 — hide every UI root (HUD, markers, cursor): world only.
//   AFTERLIGHT_NO_BG_PLANETS=1 — hide the small parallax planets behind everything.
//   AFTERLIGHT_KEY_BG=1 — empty space pure green, starfield hidden (for keying footage).
//   AFTERLIGHT_FREEZE_AT=<secs> — stop the game clock then (ships hold their spawn slots).
//   AFTERLIGHT_FX_SCALE=<k> — shots and beams drawn k times bigger (wide fight shots).
//   AFTERLIGHT_NO_DAMAGE_TEXT=1 — no floating damage numbers.
//   AFTERLIGHT_NO_SHIELD_FX=1 — no shield outline glow or facing arcs (shields still work).
//   AFTERLIGHT_STANDOFF=<u> — fighting AI ships hold this far off (before hull sizes).
//   AFTERLIGHT_HOLD_LINES=1 — fighting AI ships hold where they are (placed battle lines).
//   AFTERLIGHT_CAM_AT=<x>,<y> | station[>d] — point the camera there instead of at
//                          the ship ("station": the nearest station core; ">d"
//                          slides d units toward its world).
//   AFTERLIGHT_SHOTS_AFTER=<secs> — with AFTERLIGHT_SHOTS, don't start capturing
//                          until that many real seconds after launch.
//   AFTERLIGHT_KEYS=<script> — tap keys at set times, e.g. "3:KeyM,5:F7,8:Escape".
//                          "6:Minus~3" HOLDS a key for 3s instead -- needed
//                          for anything read with `pressed` rather than
//                          `just_pressed`, like camera zoom.
//                          Seconds of REAL time, so autoplay's time dilation
//                          can't shift it. F7 is the capture key, so a script
//                          can walk the screens and photograph each one.
//                          Goes through the same ButtonInput press the
//                          autoplay director and the gamepad bridge use --
//                          never OS keystrokes, which land in whatever app is
//                          focused rather than the game.
//                          "MouseLeft" / "MouseRight" click the mouse, at
//                          wherever the game thinks it points (see
//                          AFTERLIGHT_BUILD_CELL for build mode).
//   AFTERLIGHT_CASCADE=0.85 — pin the story's progress level, so a late beat can
//                          be looked at without an hour of flying first.
//   AFTERLIGHT_CASCADE_TRACE=1 — print the arc's state once a second.
//   AFTERLIGHT_BUILD_TAB=<name> — stop at the station, open build mode and select
//                          that build tab, so the palette can be looked at
//                          without clicking through to it. Name matches
//                          BuildCategory::name() case-insensitively
//                          ("structural", "life support", "hull").
//   AFTERLIGHT_BUILD_SLOT=<n> — with the above, park on slot n instead of the
//                          first, to check the far end of a scrolling strip.
//   AFTERLIGHT_BUILD_LAUNCH=<s> — with the above, launch after s seconds with the
//                          build menu still open. Build overlays are drawn in
//                          world space and torn down by systems that only run
//                          at the berth, so leaving that way is exactly how
//                          they get stranded there.
//   AFTERLIGHT_CAM_TRACE=1  — once a second, log how far the camera sat from the
//                          ship along its travel, and how much that wobbled
//                          (camera.rs). For judder at speed.
//   AFTERLIGHT_DEBUG=1      — enable the backtick debug menu (debug.rs). A launch
//                          without it has no debug menu at all.
//   AFTERLIGHT_MOVETEST=1   — bare movement sandbox: instant skip (no menu/
//                          station flash), starter ship, manual control,
//                          and NO AI ships spawned — just open space and
//                          stars, for isolating flight feel from everything
//                          else.
//   AFTERLIGHT_SAVE_TRACE=1 — print what a save has to bring back (orbital clock,
//                          planet positions, which ship each bounty points
//                          at) at every save, and for six seconds of flight
//                          after a load, so the two line up in the log.
//   AFTERLIGHT_BOUNTY=1     — accept one bounty on a ship in a neighbouring system
//                          as soon as there is a simulation, for when no board
//                          nearby posts one (autoplay then hunts it down).
// ============================================================================

pub struct DemoPlugin;

/// Seconds to sit in MainMenu / StationDocked before auto-advancing.
/// Movetest uses a near-zero delay so there's no visible flash of either
/// screen; the other modes keep a short delay so their own setup (ship
/// spawn, etc.) has a moment to run.
#[derive(Resource)]
struct DemoAdvanceDelays {
    menu: f32,
    station: f32,
}

/// The build tab `AFTERLIGHT_BUILD_TAB` asks for, if it names a real one.
///
/// Build mode is only reachable while docked and only by keypress, which made
/// the palette the one screen that could not be photographed unattended. Two
/// of the demo's blockers lived there -- a tab whose slots placed blocks other
/// than their labels, and a whole category with no tab at all -- and neither
/// was visible from a log line.
fn requested_build_tab() -> Option<BuildCategory> {
    let want = std::env::var("AFTERLIGHT_BUILD_TAB").ok()?;
    let want = want.trim().to_ascii_lowercase();
    BuildCategory::ALL
        .iter()
        .copied()
        .find(|c| c.name().to_ascii_lowercase() == want)
        .or_else(|| {
            warn!(
                "AFTERLIGHT_BUILD_TAB={want:?} matches no build category; expected one of {:?}",
                BuildCategory::ALL.iter().map(|c| c.name()).collect::<Vec<_>>()
            );
            None
        })
}

/// Hold at the station with the palette open on the requested tab.
fn open_build_tab(
    category: BuildCategory,
    state: Res<State<GameState>>,
    build_state: Res<State<BuildState>>,
    mut next_build: ResMut<NextState<BuildState>>,
    mut building: ResMut<BuildingState>,
    mut done: Local<bool>,
) {
    if *done || *state.get() != GameState::StationDocked {
        return;
    }
    let index = BuildCategory::ALL.iter().position(|c| *c == category);
    let Some(index) = index else { return };
    building.category_index = index;
    // AFTERLIGHT_BUILD_SLOT parks on one slot rather than the first, so a strip
    // long enough to scroll can be checked at its far end.
    building.selected_index = std::env::var("AFTERLIGHT_BUILD_SLOT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if *build_state.get() == BuildState::Inactive {
        next_build.set(BuildState::Placing);
    }
    *done = true;
    info!(
        "BUILD TAB MODE: docked, palette open on {} ({} items), slot {} = {}",
        category.name(),
        category.item_count(),
        building.selected_index,
        building.selection_name()
    );
}

/// Counts what is actually resident, once a second, under AFTERLIGHT_CENSUS=1.
///
/// The world holds dozens of ships but only simulates most of them as numbers;
/// a hull becomes real entities inside RENDER_DISTANCE and is despawned again
/// past DESPAWN_DISTANCE. Any argument about what enemy ships cost has to be
/// about the resident count, not the fleet count, and the two are very
/// different numbers.
fn census(
    time: Res<Time>,
    mut next: Local<f32>,
    mut dumped: Local<bool>,
    mut last_holdings: Local<String>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
    ai: Query<(), With<AiShip>>,
    owned: Query<(), With<crate::ai_ship::components::OwnedByAiShip>>,
    hull: Query<&ChildOf, With<crate::components::HullSegment>>,
    ai_roots: Query<Entity, With<AiShip>>,
    navs: Query<&crate::crew::navigation::NavGrid>,
    dead: Res<crate::crew::burial::DriftingDead>,
    concealed: Query<(), With<crate::ai_ship::interior::Concealed>>,
    fleets: Query<(
        &Transform,
        &crate::ai_ship::fleet::FleetMember,
        &crate::ai_ship::components::AiShipBehavior,
        &crate::ai_ship::components::AiShipState,
        Has<crate::ai_ship::fleet::CivilianHull>,
        Has<crate::ai_ship::standing_spawns::PlayerEscort>,
    )>,
    player: Query<&Transform, With<crate::components::Ship>>,
    previews: Query<(&Transform, &Sprite), With<crate::ai_ship::preview::Preview>>,
) {
    *next -= time.delta_secs();
    if *next > 0.0 {
        return;
    }
    *next = 1.0;
    // Who holds how much, whenever it changes: borders move (territory),
    // and a load must put them back where the save had them.
    if !galaxy.systems.is_empty() {
        let line = crate::celestial::territory::TERRITORIAL
            .iter()
            .map(|f| format!("{f:?}={}", galaxy.systems.iter().filter(|s| s.faction == Some(*f)).count()))
            .collect::<Vec<_>>()
            .join(" ");
        if *last_holdings != line {
            info!("HOLDINGS {line}");
            *last_holdings = line;
        }
    }
    // The territory map, once, when the galaxy exists.
    if !*dumped && !galaxy.systems.is_empty() {
        *dumped = true;
        for sys in &galaxy.systems {
            let borders = crate::celestial::territory::border_factions(&galaxy, sys.id);
            info!(
                "TERRITORY {:>2} {:<9} owner={:?} capital={} haunt={:?} danger={:.1} borders={:?}",
                sys.id, sys.name, sys.faction, sys.capital, sys.haunt, sys.danger_tier, borders
            );
        }
    }
    let roots: std::collections::HashSet<Entity> = ai_roots.iter().collect();
    let ai_hull = hull.iter().filter(|c| roots.contains(&c.parent())).count();
    let nav_cells: usize = navs.iter().map(|n| n.cells.len()).sum();
    let empty_navs = navs.iter().filter(|n| n.cells.is_empty()).count();
    info!(
        "CENSUS ships={} ai_modules={} ai_hull={} nav_grids={} nav_cells={} empty_grids={} concealed={} bodies={}",
        ai.iter().count(),
        owned.iter().count(),
        ai_hull,
        navs.iter().count(),
        nav_cells,
        empty_navs,
        concealed.iter().count(),
        dead.bodies.len()
    );
    // Far ships drawn as pictures: how many, and the nearest.
    if let Ok(you) = player.single() {
        let you = you.translation.truncate();
        let nearest = previews
            .iter()
            .map(|(t, s)| (t.translation.truncate().distance(you), s.color.alpha()))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, a)) = nearest {
            info!("PREVIEWS {} nearest {:.0} alpha {:.2}", previews.iter().count(), d, a);
        }
    }
    // Each live fleet: where its leader is, and how far each escort sits
    // from its slot (turned with the leader), and what it is doing.
    let mut ids: Vec<u32> = fleets.iter().map(|(_, m, ..)| m.fleet).collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        // An escort forms up on you.
        if let (Some((_, _, b, _, _, true)), Ok(you)) = (fleets.iter().find(|(_, m, ..)| m.fleet == id), player.single()) {
            let parts: Vec<String> = fleets
                .iter()
                .filter(|(_, m, ..)| m.fleet == id)
                .map(|(tf, m, b, ..)| {
                    let want = you.translation.truncate() + (you.rotation * m.slot.extend(0.0)).truncate();
                    format!("{:?} off={:.0}", b, tf.translation.truncate().distance(want))
                })
                .collect();
            let _ = b;
            info!("ESCORT {id}: {}", parts.join(" | "));
            continue;
        }
        let members: Vec<_> = fleets.iter().filter(|(_, m, ..)| m.fleet == id).collect();
        let Some((lead_tf, _, lead_b, _, _, _)) = members.iter().find(|(_, m, ..)| m.leader) else {
            info!("FLEET {id}: no live leader, {} members", members.len());
            continue;
        };
        let lead = lead_tf.translation.truncate();
        let parts: Vec<String> = members
            .iter()
            .filter(|(_, m, ..)| !m.leader)
            .map(|(tf, m, b, st, civ, _)| {
                let want = lead + (lead_tf.rotation * m.slot.extend(0.0)).truncate();
                let off = tf.translation.truncate().distance(want);
                format!("{}{:?} off={:.0}{}", if *civ { "civ " } else { "" }, b, off, if st.is_destroyed { " DEAD" } else { "" })
            })
            .collect();
        info!("FLEET {id}: lead {:?} at ({:.0},{:.0}) | {}", lead_b, lead.x, lead.y, parts.join(" | "));
    }
}

/// What a save has to bring back, under AFTERLIGHT_SAVE_TRACE=1: the orbital clock,
/// where the loaded system's planets are, and which simulated ship each
/// accepted bounty points at.
///
/// Printed at every save, then on the first frame of flight after a load and
/// once a second for six more. Flight, because orbits and the simulation only
/// run while exploring: a load that lands docked moves nothing until launch.
fn save_trace(
    time: Res<Time<Real>>,
    mut saved: MessageReader<crate::events::GameSaved>,
    mut loaded: MessageReader<crate::events::GameLoaded>,
    mut since_load: Local<Option<(f32, u32)>>,
    state: Res<State<GameState>>,
    galaxy: Res<crate::celestial::resources::GalaxyState>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    contracts: Res<crate::contracts::ContractState>,
    sim: Res<WorldSimulation>,
    planets: Query<(&crate::celestial::components::CelestialBody, &Transform), With<crate::celestial::components::OrbitalPath>>,
) {
    use crate::celestial::components::CelestialBodyType;
    use crate::contracts::ContractObjective;
    let report = |when: &str| {
        let mut worlds: Vec<String> = planets.iter()
            .filter(|(body, _)| body.body_type == CelestialBodyType::Planet)
            .map(|(body, tf)| format!("{}@({:.0},{:.0})", body.name, tf.translation.x, tf.translation.y))
            .collect();
        worlds.sort();
        let bounties: Vec<String> = contracts.active_contracts.iter()
            .filter_map(|c| match c.objective {
                ContractObjective::DestroyShip { ship_type, target_id, destroyed } => Some((ship_type, target_id, destroyed)),
                _ => None,
            })
            .map(|(wanted, id, destroyed)| match sim.ships.iter().find(|s| s.bounty_id == Some(id)) {
                _ if destroyed => format!("#{id} {wanted:?}: destroyed"),
                Some(s) => format!(
                    "#{id} {wanted:?}: carried by a {:?} in system {}{}",
                    s.faction,
                    s.system_id,
                    if s.behavior == SimBehavior::Dead { " (DEAD)" } else { "" }
                ),
                None => format!("#{id} {wanted:?}: NO SHIP CARRIES IT"),
            })
            .collect();
        info!(
            "SAVETRACE {when}: state={:?} system={:?} clock={:.2} sim_ships={} planets=[{}] bounties=[{}]",
            state.get(),
            streaming.loaded_system,
            galaxy.galaxy_time,
            sim.ships.len(),
            worlds.join(" "),
            bounties.join("; ")
        );
    };

    for save in saved.read().filter(|e| e.success) {
        report(&format!("save slot {}", save.slot));
    }
    if loaded.read().filter(|e| e.success).count() > 0 {
        *since_load = Some((0.0, 0));
    }
    let Some((elapsed, printed)) = since_load.as_mut() else { return };
    if *state.get() != GameState::Exploring {
        return;
    }
    if *printed == 0 || *elapsed >= *printed as f32 {
        report(&format!("load+{elapsed:.1}s"));
        *printed += 1;
    }
    *elapsed += time.delta_secs();
    if *printed > 6 {
        *since_load = None;
    }
}

/// Under AFTERLIGHT_BOUNTY=1: take one bounty, on a ship in a neighbouring system,
/// as soon as there is a simulation to tag in.
///
/// A board only posts bounties on factions near it, at the star ratings the
/// player's standing unlocks, so whether one turns up depends on the galaxy:
/// Haven's neighbours can hold nothing a two-star board will post, and an
/// autoplay run then never takes one. This one is tagged the way a board tags
/// (`tag_bounty_target`) and paid for the way accepting one is, so what
/// follows -- the hunt, the jump, a save, a load -- runs on the real thing.
fn take_a_bounty(
    mut done: Local<bool>,
    state: Res<State<GameState>>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut sim: ResMut<WorldSimulation>,
    mut contracts: ResMut<crate::contracts::ContractState>,
    mut currency: ResMut<crate::resources::Currency>,
) {
    use crate::contracts::{Contract, ContractObjective, ContractStatus, ContractType, Faction};
    if *done || *state.get() != GameState::Exploring || !sim.initialized {
        return;
    }
    *done = true;
    let mut rng = rand::thread_rng();
    // Elsewhere if anywhere, so taking it means a jump.
    let elsewhere = streaming.warm_systems.clone();
    let here: Vec<u32> = streaming.loaded_system.into_iter().collect();
    for systems in [elsewhere, here] {
        let faction = sim.ships.iter()
            .find(|s| systems.contains(&s.system_id) && s.behavior != SimBehavior::Dead && s.bounty_id.is_none())
            .map(|s| s.faction);
        let Some(faction) = faction else { continue };
        let Some((target_id, distance)) = sim.tag_bounty_target(faction, &systems, &mut rng) else { continue };
        let deposit = currency.credits.min(100);
        currency.credits -= deposit;
        let id = contracts.next_id;
        contracts.next_id += 1;
        contracts.active_contracts.push(Contract {
            id,
            faction: Faction::Navy,
            contract_type: ContractType::DestroyShip,
            title: format!("Bounty: {faction:?} vessel"),
            description: "Taken by AFTERLIGHT_BOUNTY.".into(),
            objective: ContractObjective::DestroyShip { ship_type: faction, target_id, destroyed: false },
            reward: 1000,
            deposit,
            star_rating: 3,
            depth_zone: crate::components::ZoneType::NearOrbit,
            status: ContractStatus::Active,
        });
        info!("AFTERLIGHT_BOUNTY: took bounty #{target_id} on a {faction:?} {distance:.0} out, in one of systems {systems:?}");
        return;
    }
    warn!("AFTERLIGHT_BOUNTY: no ship anywhere near to put a bounty on");
}

/// Name every sprite sitting near the ship's origin, under AFTERLIGHT_WHATSTHERE=1.
///
/// Built because three separate guesses at "what is that green thing in the
/// middle of the screen" were all wrong, and counting green pixels in a
/// screenshot was worse than useless -- it matched the main menu's own text
/// and the red power numbers scattered over the hull.
///
/// An exclusive system so it can ask the world what components an entity
/// actually carries. Component names need Bevy's `debug` feature to print;
/// without it the colour, size and distance are still enough to identify a
/// sprite in a codebase you can grep. `ViewVisibility` here is last frame's
/// value, since it is computed in PostUpdate -- treat a hit as "exists and is
/// probably drawn", then crop the frame and look before concluding anything.
fn whats_there(world: &mut World) {
    use bevy::ecs::system::SystemState;
    let mut once: SystemState<(
        Query<&GlobalTransform, With<Ship>>,
        Query<(Entity, &Sprite, &GlobalTransform, &ViewVisibility)>,
    )> = SystemState::new(world);
    let Ok((ships, sprites)) = once.get(world) else { return };
    let Ok(ship_gt) = ships.single() else { return };
    let origin = ship_gt.translation().truncate();

    let mut hits: Vec<(Entity, String, f32)> = Vec::new();
    for (entity, sprite, gt, view) in sprites.iter() {
        // Only what is actually drawn. Listing every sprite regardless caught
        // four hidden footprint tiles parked at the origin and sent me after
        // the wrong thing entirely.
        if !view.get() {
            continue;
        }
        let p = gt.translation().truncate();
        let d = p.distance(origin);
        if d > 60.0 {
            continue;
        }
        let c = sprite.color.to_srgba();
        hits.push((
            entity,
            format!(
                "rgba({:.2},{:.2},{:.2},{:.2}) size={:?}",
                c.red, c.green, c.blue, c.alpha, sprite.custom_size
            ),
            d,
        ));
    }

    for (entity, desc, d) in hits {
        let names: Vec<String> = world
            .inspect_entity(entity)
            .map(|infos| infos.map(|i| i.name().to_string()).collect())
            .unwrap_or_default();
        info!("WHATSTHERE {entity:?} d={d:.0} {desc}\n    components: {}", names.join(", "));
    }
}

pub fn skip_ai_ship_spawn() -> bool {
    std::env::var("AFTERLIGHT_MOVETEST").ok().as_deref() == Some("1")
}

impl Plugin for DemoPlugin {
    fn build(&self, app: &mut App) {
        let full_demo = std::env::var("AFTERLIGHT_DEMO").ok().as_deref() == Some("1");
        let skip_menu = std::env::var("AFTERLIGHT_SKIP_MENU").ok().as_deref() == Some("1");
        let move_test = skip_ai_ship_spawn();
        let build_tab = requested_build_tab();
        if std::env::var("AFTERLIGHT_WHATSTHERE").ok().as_deref() == Some("1") {
            app.add_systems(Update, whats_there.run_if(bevy::time::common_conditions::on_timer(
                std::time::Duration::from_millis(900),
            )));
        }
        if std::env::var("AFTERLIGHT_CENSUS").ok().as_deref() == Some("1") {
            app.add_systems(Update, census);
        }
        // AFTERLIGHT_ZOOM=<z>: hold the camera at this zoom (scripted key holds
        // don't reach the zoom in an offscreen run). 1.8 is the default; 20
        // the furthest out.
        if let Some(z) = std::env::var("AFTERLIGHT_ZOOM").ok().and_then(|v| v.parse::<f32>().ok()) {
            app.add_systems(Update, move |mut cam: ResMut<crate::camera::CameraState>| {
                // filming may go wider than play allows
                cam.max_zoom = cam.max_zoom.max(z);
                cam.zoom = z.clamp(cam.min_zoom, cam.max_zoom);
                cam.user_zoomed = true;
                // A held zoom is a zoom by hand: frame_the_fight leaves it alone.
                cam.fight_framing_off = true;
            });
        }
        // AFTERLIGHT_ZOOM_RAMP=<from>:<to>:<secs>[:<delay>] -- a pull-back or
        // push-in for footage, eased and geometric (equal steps feel equal).
        if let Some((from, to, secs, delay)) = std::env::var("AFTERLIGHT_ZOOM_RAMP").ok().and_then(|v| {
            let n: Vec<f32> = v.split(':').filter_map(|p| p.parse().ok()).collect();
            (n.len() >= 3).then(|| (n[0], n[1], n[2].max(0.01), n.get(3).copied().unwrap_or(0.0)))
        }) {
            app.add_systems(Update, move |time: Res<Time<Real>>, mut cam: ResMut<crate::camera::CameraState>| {
                let t = ((time.elapsed_secs() - delay) / secs).clamp(0.0, 1.0);
                let eased = t * t * (3.0 - 2.0 * t);
                cam.max_zoom = cam.max_zoom.max(from.max(to));
                cam.zoom = (from * (to / from).powf(eased)).clamp(cam.min_zoom, cam.max_zoom);
                cam.user_zoomed = true;
            });
        }
        // AFTERLIGHT_CAM_AT=<x>,<y> | station -- film something other than the ship.
        if let Ok(spec) = std::env::var("AFTERLIGHT_CAM_AT") {
            if let Some(rest) = spec.trim().strip_prefix("station") {
                // The nearest station core: the system's own, or one parked by
                // AFTERLIGHT_STATION_TEST. "station>8000" slides 8 km toward its
                // world, to get the world into the frame behind it.
                let toward_world: f32 = rest.strip_prefix('>').and_then(|v| v.parse().ok()).unwrap_or(0.0);
                app.add_systems(Update, move |cores: Query<&GlobalTransform, With<crate::diplomacy::stations::StationCore>>,
                                              bodies: Query<(&GlobalTransform, &crate::celestial::components::CelestialBody)>,
                                              ship: Query<&Transform, With<crate::components::Ship>>,
                                              mut pin: ResMut<crate::camera::CameraPin>| {
                    let at = ship.single().map(|t| t.translation.truncate()).unwrap_or(Vec2::ZERO);
                    pin.0 = cores.iter()
                        .map(|t| t.translation().truncate())
                        .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
                        .map(|station| {
                            let world = bodies
                                .iter()
                                .map(|(t, b)| (t.translation().truncate(), b.radius))
                                .min_by(|a, b| (a.0.distance(station) - a.1).total_cmp(&(b.0.distance(station) - b.1)));
                            match world {
                                Some((centre, _)) if toward_world != 0.0 => station + (centre - station).normalize_or(Vec2::ZERO) * toward_world,
                                _ => station,
                            }
                        });
                });
            } else if let Some((x, y)) = spec.split_once(',').and_then(|(x, y)| Some((x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?))) {
                app.insert_resource(crate::camera::CameraPin(Some(Vec2::new(x, y))));
            }
        }
        // AFTERLIGHT_NO_BG_PLANETS=1 -- hide the small parallax planets drawn behind
        // everything, for footage with its own backdrop.
        if std::env::var("AFTERLIGHT_NO_BG_PLANETS").ok().as_deref() == Some("1") {
            app.add_systems(
                PostUpdate,
                (|mut planets: Query<&mut Visibility, With<crate::vfx::starfield::BackgroundPlanet>>| {
                    for mut v in &mut planets {
                        *v = Visibility::Hidden;
                    }
                })
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
        }
        // AFTERLIGHT_FREEZE_AT=<secs> -- stop the game clock at that real time, so
        // ships stay exactly where they spawned (F7 still captures).
        if let Some(at) = std::env::var("AFTERLIGHT_FREEZE_AT").ok().and_then(|v| v.parse::<f32>().ok()) {
            app.add_systems(Update, move |real: Res<Time<Real>>, mut virt: ResMut<Time<Virtual>>| {
                if real.elapsed_secs() >= at && virt.relative_speed() != 0.0 {
                    virt.set_relative_speed(0.0);
                }
            });
        }
        // AFTERLIGHT_HIDE_PLAYER=1 -- your own ship and its light cone out of the
        // picture, for footage of the world going on without you.
        if std::env::var("AFTERLIGHT_HIDE_PLAYER").ok().as_deref() == Some("1") {
            app.add_systems(
                PostUpdate,
                (|mut q: Query<&mut Visibility, Or<(With<crate::components::Ship>, With<crate::world::home_base::BaseArrow>, With<crate::contracts::bounty_nav::BountyArrow>)>>| {
                    for mut v in &mut q {
                        *v = Visibility::Hidden;
                    }
                })
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
        }
        // AFTERLIGHT_KEY_BG=1 -- empty space drawn pure green, the starfield, stars
        // and planets hidden, so footage can be keyed cleanly and given its own
        // backdrop.
        if std::env::var("AFTERLIGHT_KEY_BG").ok().as_deref() == Some("1") {
            app.add_systems(
                PostUpdate,
                (|mut clear: ResMut<ClearColor>,
                  mut stars: Query<&mut Visibility, Or<(With<crate::vfx::starfield::StarfieldStar>, With<crate::vfx::starfield::BackgroundPlanet>, With<crate::celestial::components::Star>, With<crate::celestial::components::Planet>)>>| {
                    clear.0 = Color::srgb(0.0, 1.0, 0.0);
                    for mut v in &mut stars {
                        *v = Visibility::Hidden;
                    }
                })
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
        }
        // AFTERLIGHT_HIDE_UI=1 -- the world alone, for footage.
        if std::env::var("AFTERLIGHT_HIDE_UI").ok().as_deref() == Some("1") {
            app.add_systems(
                PostUpdate,
                (|mut roots: Query<&mut Visibility, (With<Node>, Without<ChildOf>)>,
                  // Labels in the world are the HUD too: ship names, damage
                  // read-outs and station plates drawn over the footage.
                  mut labels: Query<
                    &mut Visibility,
                    (
                        Or<(
                            With<crate::vfx::far_view::FarLabel>,
                            With<crate::vfx::far_view::FarLabelShadow>,
                            With<crate::camera::ZoomInvariantText>,
                        )>,
                        Without<Node>,
                    ),
                >| {
                    for mut v in roots.iter_mut().chain(labels.iter_mut()) {
                        *v = Visibility::Hidden;
                    }
                })
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
        }
        // AFTERLIGHT_START_SYSTEM=<where>[:fleet] -- jump there a few seconds in.
        // <where> is a system id, a faction's type name (its capital: the
        // galaxy is rolled fresh each run, so ids don't carry over), or
        // "haunt" (the Shepherd's system). With ":fleet", then park 12 km
        // off one of its fleets (the arrival berth is kept clear of them).
        if let Ok(spec) = std::env::var("AFTERLIGHT_START_SYSTEM") {
            let (place, to_fleet) = match spec.split_once(':') {
                Some((p, rest)) => (p.to_string(), rest.to_string()),
                None => (spec.clone(), String::new()),
            };
            {
                app.add_systems(
                    Update,
                    (move |time: Res<Time<Real>>,
                           mut stage: Local<u8>,
                           mut since: Local<f32>,
                           mut target: Local<Option<u32>>,
                           game_state: Res<State<crate::states::GameState>>,
                           galaxy: Res<crate::celestial::resources::GalaxyMap>,
                           mut jumps: MessageWriter<crate::celestial::events::WarpJumpStarted>,
                           sim: Res<crate::ai_ship::components::WorldSimulation>,
                           streaming: Res<crate::celestial::resources::SystemStreamingManager>,
                           mut ship: Query<(&mut Transform, &mut crate::components::Velocity, Option<&crate::ship::collision::Collider>), With<crate::components::Ship>>,
                           worlds: Query<(&Transform, &crate::celestial::components::CelestialBody), (With<crate::celestial::capital_worlds::CapitalWorld>, Without<crate::components::Ship>)>,
                           faction_stations: Res<crate::diplomacy::stations::FactionStations>,
                           berths: crate::diplomacy::stations::StationBerths| {
                        *since += time.delta_secs();
                        // Only runs in flight, so a changed state means it docked.
                        let away = game_state.is_changed();
                        if target.is_none() && !galaxy.systems.is_empty() {
                            *target = place.parse::<u32>().ok().or_else(|| {
                                if place == "haunt" {
                                    galaxy.systems.iter().find(|s| s.haunt == Some(crate::ai_ship::components::AiShipType::Shepherd)).map(|s| s.id)
                                } else {
                                    crate::celestial::territory::TERRITORIAL
                                        .iter()
                                        .find(|f| format!("{f:?}") == place)
                                        .and_then(|f| crate::celestial::territory::capital_of(&galaxy, *f))
                                }
                            });
                            if target.is_none() {
                                warn!("AFTERLIGHT_START_SYSTEM: no system matches {place:?}");
                                *stage = 2;
                            }
                        }
                        let Some(id) = *target else { return };
                        match *stage {
                            0 if *since >= 3.0 => {
                                info!("AFTERLIGHT_START_SYSTEM: jumping to system {id}");
                                jumps.write(crate::celestial::events::WarpJumpStarted {
                                    target: crate::celestial::resources::GalaxyWarpTarget::System(id),
                                });
                                *stage = match to_fleet.as_str() {
                                    "fleet" => 1,
                                    "capital" => 3,
                                    "approach" => 6,
                                    _ => 2,
                                };
                                *since = 0.0;
                            }
                            1 if *since >= 2.0 && streaming.loaded_system == Some(id) => {
                                *stage = 2;
                                let alive = |f: &crate::ai_ship::fleet::SimFleet| {
                                    sim.ships.iter().any(|s| s.fleet == f.id && s.behavior != crate::ai_ship::components::SimBehavior::Dead)
                                };
                                let fleets: Vec<_> = sim.fleets.iter().filter(|f| f.system_id == id && alive(f)).collect();
                                let Some(fleet) = fleets.get((time.elapsed_secs() * 1000.0) as usize % fleets.len().max(1)) else {
                                    warn!("AFTERLIGHT_START_SYSTEM: system {id} has no fleets");
                                    return;
                                };
                                if let Ok((mut tf, mut v, _)) = ship.single_mut() {
                                    let at = fleet.position + Vec2::new(12_000.0, 0.0);
                                    tf.translation.x = at.x;
                                    tf.translation.y = at.y;
                                    v.0 = Vec2::ZERO;
                                    info!("AFTERLIGHT_START_SYSTEM: parked 12 km off a {:?} {:?} (fleet {})", fleet.faction, fleet.kind, fleet.id);
                                }
                            }
                            // ":capital": the capital world's night side, near its starport.
                            3 if *since >= 3.0 && streaming.loaded_system == Some(id) => {
                                let Some((world, body)) = worlds.iter().next() else {
                                    if *since > 10.0 {
                                        warn!("AFTERLIGHT_START_SYSTEM: system {id} has no capital world");
                                        *stage = 2;
                                    }
                                    return;
                                };
                                *stage = 4;
                                *since = 0.0;
                                let star = galaxy.systems.iter().find(|s| s.id == id).map_or(Vec2::ZERO, |s| s.local_center);
                                let at = world.translation.truncate();
                                let out = (at - star).normalize_or(Vec2::X);
                                // Beside its starport (in docking reach), which
                                // stands off the world's night side.
                                let park = match faction_stations.post.filter(|p| p.0 == id) {
                                    Some((_, port, extent)) => port + out.perp() * (extent + 1_500.0),
                                    None => at + out * (body.radius + 2_500.0) + out.perp() * 4_000.0,
                                };
                                if let Ok((mut tf, mut v, _)) = ship.single_mut() {
                                    tf.translation.x = park.x;
                                    tf.translation.y = park.y;
                                    v.0 = Vec2::ZERO;
                                    info!("AFTERLIGHT_START_SYSTEM: parked on the night side of {}", body.name);
                                }
                            }
                            // ":approach": well out from the starport on the far side
                            // from its world, so flying in has the world behind it.
                            6 if *since >= 3.0 && streaming.loaded_system == Some(id) => {
                                let Some((world, _)) = worlds.iter().next() else {
                                    if *since > 10.0 {
                                        *stage = 2;
                                    }
                                    return;
                                };
                                let Some((_, port, extent)) = faction_stations.post.filter(|p| p.0 == id) else {
                                    if *since > 10.0 {
                                        warn!("AFTERLIGHT_START_SYSTEM: system {id} has no starport");
                                        *stage = 2;
                                    }
                                    return;
                                };
                                *stage = 2;
                                let out = (port - world.translation.truncate()).normalize_or(Vec2::X);
                                let park = port + out * (extent + 12_000.0) + out.perp() * 3_000.0;
                                if let Ok((mut tf, mut v, _)) = ship.single_mut() {
                                    tf.translation.x = park.x;
                                    tf.translation.y = park.y;
                                    v.0 = Vec2::ZERO;
                                    info!("AFTERLIGHT_START_SYSTEM: parked 12 km out from the starport, facing in");
                                }
                            }
                            // Then onto the starport's berth once it has
                            // materialised (the spot above is only a guess at
                            // where its platforms aren't), and hold there the
                            // way a player keeps station, until docking. The
                            // berth trails the station, so a ship left to
                            // itself would be out of docking reach in seconds.
                            4 | 5 => {
                                if away || (*stage == 5 && *since > 60.0) {
                                    *stage = 2;
                                    return;
                                }
                                let Ok((mut tf, mut v, collider)) = ship.single_mut() else { return };
                                if let Some(at) = berths.berth_for(None, &tf, collider) {
                                    tf.translation.x = at.x;
                                    tf.translation.y = at.y;
                                    v.0 = Vec2::ZERO;
                                    if *stage == 4 {
                                        *stage = 5;
                                        *since = 0.0;
                                        info!("AFTERLIGHT_START_SYSTEM: holding at the starport's berth");
                                    }
                                } else if *since > 10.0 {
                                    *stage = 2;
                                }
                            }
                            _ => {}
                        }
                    })
                    .run_if(in_state(crate::states::GameState::Exploring)),
                );
            }
        }
        if std::env::var("AFTERLIGHT_SHOW_SHIPS").ok().as_deref() == Some("1") {
            app.add_systems(Update, (
                show_ships_whole.after(crate::ai_ship::interior::conceal_new_ai_interiors),
                crate::vfx::ship_roof::update_ai_roofs,
            ));
        }
        // AFTERLIGHT_KILL_TRACE=1: log every AI ship destroyed, with when and how.
        if std::env::var("AFTERLIGHT_KILL_TRACE").ok().as_deref() == Some("1") {
            app.add_systems(Last, |time: Res<Time<Real>>,
                                   mut kills: MessageReader<crate::events::AiShipDestroyed>,
                                   mut fired: MessageReader<crate::events::WeaponFired>,
                                   mut hits: MessageReader<crate::events::AiShipDamaged>,
                                   mut tally: Local<(f32, u32, u32, f32)>| {
                for k in kills.read() {
                    info!("KILL_TRACE t={:.1} {:?} at {:?} cause {:?}", time.elapsed_secs(), k.ship_type, k.position, k.cause);
                }
                tally.1 += fired.read().filter(|f| !f.from_player).count() as u32;
                for h in hits.read() {
                    tally.2 += 1;
                    tally.3 += h.amount;
                }
                if time.elapsed_secs() - tally.0 >= 2.0 {
                    info!("KILL_TRACE fire: {} AI shots, {} hits, {:.0} damage in 2 s", tally.1, tally.2, tally.3);
                    *tally = (time.elapsed_secs(), 0, 0, 0.0);
                }
            });
        }
        // AFTERLIGHT_DOOM=<t1>,<t2>,...: at those real seconds the AI ship in shot
        // that's under fire and worst hurt (else the one nearest the camera)
        // is taken apart through the game's own damage path -- a burst
        // of heavy hits on its blocks, then enough to gut it -- so a battle
        // filmed for seconds, not minutes, has ships dying the way they do in play.
        if let Ok(spec) = std::env::var("AFTERLIGHT_DOOM") {
            let times: Vec<f32> = spec.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            app.insert_resource(Doom { times, active: Vec::new() }).add_systems(Update, doom);
        }
        if std::env::var("AFTERLIGHT_ROT_TRACE").ok().as_deref() == Some("1") {
            app.add_systems(Last, rot_trace);
        }
        // AFTERLIGHT_TTK_RANGE=<units>: a firing range. The player ship sits at the
        // origin, nose to +x, and every AI ship is held that far off along +x,
        // so a time-to-kill run measures the guns -- not where the mouse
        // happened to be or how far the enemy wandered.
        if let Some(range) = std::env::var("AFTERLIGHT_TTK_RANGE").ok().and_then(|v| v.parse::<f32>().ok()) {
            app.add_systems(
                PostUpdate,
                (move |mut player: Query<(&mut Transform, &mut ShipPhysics, &mut crate::components::Velocity), (With<Ship>, Without<AiShip>)>,
                       mut enemies: Query<(&mut Transform, Option<&mut crate::components::Velocity>), (With<AiShip>, Without<Ship>)>| {
                    for (mut t, mut p, mut v) in &mut player {
                        t.translation.x = 0.0;
                        t.translation.y = 0.0;
                        t.rotation = Quat::IDENTITY;
                        p.rotation = 0.0;
                        p.angular_velocity = 0.0;
                        v.0 = Vec2::ZERO;
                    }
                    // Held still, velocity and all: a leading shot reads it.
                    for (mut t, v) in &mut enemies {
                        t.translation.x = range;
                        t.translation.y = 0.0;
                        if let Some(mut v) = v {
                            v.0 = Vec2::ZERO;
                        }
                    }
                })
                .before(bevy::transform::TransformSystems::Propagate),
            );
        }
        // AFTERLIGHT_SPEED_TRACE=1: once a second, the player ship's speed, turn
        // rate and mass -- for comparing how ships of different sizes fly.
        if std::env::var("AFTERLIGHT_SPEED_TRACE").ok().as_deref() == Some("1") {
            app.add_systems(Last, |time: Res<Time>,
                                   ship: Query<(&crate::components::Velocity, &crate::components::ShipPhysics), With<crate::components::Ship>>,
                                   mut last: Local<f32>| {
                if time.elapsed_secs() - *last < 1.0 {
                    return;
                }
                *last = time.elapsed_secs();
                if let Ok((v, p)) = ship.single() {
                    info!("SPEED t={:.0} speed={:.0} turn={:.1}deg/s mass={:.0}", time.elapsed_secs(), v.0.length(), p.angular_velocity.to_degrees(), p.mass);
                }
            });
        }
        if std::env::var("AFTERLIGHT_FLY_TO").ok().as_deref() == Some("station") {
            app.add_systems(PreUpdate, fly_to_station.after(bevy::input::InputSystems));
        }
        // AFTERLIGHT_DUMP_GALAXY=<file>: write every system (name, map position,
        // holder, holder's map colour) as JSON once the galaxy exists.
        if let Ok(path) = std::env::var("AFTERLIGHT_DUMP_GALAXY") {
            app.add_systems(Update, move |galaxy: Res<crate::celestial::resources::GalaxyMap>, mut done: Local<bool>| {
                if *done || galaxy.systems.is_empty() {
                    return;
                }
                *done = true;
                let rows: Vec<String> = galaxy.systems.iter().map(|s| {
                    let (holder, rgb) = match s.faction {
                        Some(f) => {
                            let c = crate::ai_ship::components::faction_map_color(f).to_srgba();
                            (format!("{:?}", f), [c.red, c.green, c.blue])
                        }
                        None => ("Haven".to_string(), [1.0, 1.0, 1.0]),
                    };
                    format!(
                        "{{\"id\":{},\"name\":\"{}\",\"x\":{},\"y\":{},\"holder\":\"{}\",\"rgb\":[{},{},{}]}}",
                        s.id, s.name, s.galaxy_pos.x, s.galaxy_pos.y, holder, rgb[0], rgb[1], rgb[2]
                    )
                }).collect();
                let _ = std::fs::write(&path, format!("[{}]", rows.join(",")));
                info!("AFTERLIGHT_DUMP_GALAXY: {} systems to {}", rows.len(), path);
            });
        }
        if std::env::var("AFTERLIGHT_SAVE_TRACE").ok().as_deref() == Some("1") {
            // After the orbits, so a frame's planet positions match its clock.
            app.add_systems(Update, save_trace.after(crate::celestial::CelestialSet::Orbits));
        }
        if std::env::var("AFTERLIGHT_BOUNTY").ok().as_deref() == Some("1") {
            app.add_systems(Update, take_a_bounty);
        }
        if !full_demo && !skip_menu && !move_test && build_tab.is_none() {
            return;
        }

        // Stop at the station rather than launching: the palette only exists
        // while docked, so advancing to Exploring would close the thing we
        // came to look at.
        if let Some(category) = build_tab {
            if let Some(secs) = std::env::var("AFTERLIGHT_BUILD_LAUNCH")
                .ok()
                .and_then(|v| v.trim().parse::<f32>().ok())
            {
                app.add_systems(Update, move |
                    time: Res<Time>,
                    state: Res<State<GameState>>,
                    mut next: ResMut<NextState<GameState>>,
                    mut waited: Local<f32>,
                | {
                    if *state.get() != GameState::StationDocked {
                        return;
                    }
                    *waited += time.delta_secs();
                    if *waited > secs {
                        info!("BUILD TAB MODE: launching with the build menu open");
                        next.set(GameState::Exploring);
                    }
                });
            }
            app.insert_resource(DemoAdvanceDelays { menu: 1.0, station: f32::INFINITY })
                .add_systems(Update, demo_advance_states)
                .add_systems(Update, move |
                    state: Res<State<GameState>>,
                    build_state: Res<State<BuildState>>,
                    next_build: ResMut<NextState<BuildState>>,
                    building: ResMut<BuildingState>,
                    done: Local<bool>,
                | open_build_tab(category, state, build_state, next_build, building, done));
            return;
        }

        let delays = if move_test {
            DemoAdvanceDelays { menu: 0.05, station: 0.05 }
        } else {
            DemoAdvanceDelays { menu: 2.0, station: 4.0 }
        };
        app.insert_resource(delays)
            .add_systems(Update, demo_advance_states);

        // AFTERLIGHT_MOVETEST_ENEMY_AUTOPILOT drives the player ship at the
        // single movetest dummy automatically, for unattended faction-
        // behavior verification (same demo_autopilot used by full
        // AFTERLIGHT_DEMO, just also enabled in the single-dummy sandbox).
        // Combine with AFTERLIGHT_MOVETEST_ENEMY_FACTION to test a specific
        // faction's decision tree in isolation.
        let movetest_autopilot = move_test
            && std::env::var("AFTERLIGHT_MOVETEST_ENEMY_AUTOPILOT").ok().as_deref() == Some("1");

        if full_demo || movetest_autopilot {
            info!("DEMO MODE: autopilot + periodic engine screenshots active");
            app.add_systems(Update, demo_screenshots)
                .add_systems(
                    Update,
                    demo_autopilot
                        .in_set(crate::states::ShipSet::Movement)
                        .before(crate::ship::ship_movement)
                        .after(crate::combat::targeting::fire_groups::fire_group_input)
                        .run_if(in_state(GameState::Exploring)),
                );
        } else if move_test {
            info!("MOVETEST MODE: empty space, starter ship, no AI ships, manual control");
        } else {
            info!("SKIP MENU MODE: jumping straight into Exploring, manual control");
        }
    }
}

/// Menu → station → launch, on a timer instead of keypresses.
fn demo_advance_states(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    delays: Res<DemoAdvanceDelays>,
    mut t: Local<f32>,
) {
    *t += time.delta_secs();
    match state.get() {
        GameState::MainMenu if *t > delays.menu => {
            next.set(GameState::StationDocked);
            *t = 0.0;
        }
        GameState::StationDocked if *t > delays.station => {
            next.set(GameState::Exploring);
            *t = 0.0;
        }
        _ => {}
    }
}

/// Fly toward the nearest enemy (spawned entity first, simulated ship as a
/// distant waypoint otherwise), keep a fighting distance, select it as the
/// target, and hold fire when in range.
fn demo_autopilot(
    mut input_state: ResMut<InputState>,
    mut selection: ResMut<TargetSelection>,
    mut fire_state: ResMut<FireGroupState>,
    sim: Res<WorldSimulation>,
    ai_ships: Query<(Entity, &Transform), With<AiShip>>,
    mut ship_query: Query<(&mut Transform, &mut ShipPhysics), (With<Ship>, Without<AiShip>)>,
) {
    let Ok((mut transform, mut physics)) = ship_query.single_mut() else { return };
    let pos = transform.translation.truncate();

    let mut best_dist = f32::MAX;
    let mut target_pos: Option<Vec2> = None;
    let mut target_entity: Option<Entity> = None;

    for (entity, t) in ai_ships.iter() {
        let p = t.translation.truncate();
        let d = pos.distance(p);
        if d < best_dist {
            best_dist = d;
            target_pos = Some(p);
            target_entity = Some(entity);
        }
    }
    if target_entity.is_none() {
        for s in sim.ships.iter().filter(|s| !s.spawned && s.behavior != SimBehavior::Dead) {
            let d = pos.distance(s.position);
            if d < best_dist {
                best_dist = d;
                target_pos = Some(s.position);
            }
        }
    }
    let Some(tp) = target_pos else { return };

    // Steer the nose straight at the target (authoritative — the cursor
    // isn't over the window during unattended runs, so nothing fights this)
    let dir = tp - pos;
    let angle = dir.y.atan2(dir.x);
    physics.rotation = angle;
    physics.angular_velocity = 0.0;

    // Approach, then hold a fighting distance
    input_state.movement.y = if best_dist > 700.0 {
        1.0
    } else if best_dist < 350.0 {
        -0.4
    } else {
        0.2
    };
    input_state.movement.x = 0.0;

    if let Some(entity) = target_entity {
        selection.target = Some(entity);
        selection.target_type = TargetType::Ship;
        let in_range = best_dist < 900.0;
        fire_state.firing = [in_range; 4];
    } else {
        selection.target = None;
        fire_state.firing = [false; 4];
    }
}

/// Engine-rendered screenshot every few seconds into AFTERLIGHT_DEMO_DIR.
fn demo_screenshots(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut index: Local<u32>,
    offscreen: Option<Res<OffscreenTarget>>,
) {
    *since_last += time.delta_secs();
    if *since_last < 4.0 {
        return;
    }
    *since_last = 0.0;

    let dir = std::env::var("AFTERLIGHT_DEMO_DIR").unwrap_or_else(|_| "/tmp/afterlight_frames".to_string());
    let _ = std::fs::create_dir_all(&dir);
    let path = format!("{}/frame_{:03}.png", dir, *index);
    *index += 1;

    commands.spawn(screenshot_of(offscreen.as_deref())).observe(save_to_disk(path));
}

// ============================================================================
// ENGINE-SIDE CAPTURE — usable during ordinary play.
//
// The existing AFTERLIGHT_DEMO capture is welded to the autopilot, which is no use
// for photographing a session somebody is actually playing. This is the same
// mechanism with nothing else attached.
//
// It matters that this is an engine capture rather than an OS screen grab.
// `Screenshot::primary_window()` reads the render target, so it never catches
// another app the way an OS grab of the display does — verified the hard way.
// But a render target only holds what was drawn, and macOS stops drawing a
// window that is fully covered: a background playtest got one real frame,
// the instant the window opened, then 63 blank. AFTERLIGHT_OFFSCREEN renders into
// an image instead, which keeps drawing regardless; see `OffscreenTarget`.
// ============================================================================

/// The image the game renders into when `AFTERLIGHT_OFFSCREEN=1`, instead of the
/// window.
///
/// Engine-side capture photographs a render target, so it never catches
/// another app -- but it can only photograph what the game actually DRAWS,
/// and macOS stops a window drawing once another window fully covers it.
/// Playtests run with this app in front, so a background run produced one
/// real frame (the instant the window opened) and then nothing: the director
/// itself reported 63 of 64 frames blank. Rendering into an image takes the
/// window out of the capture path entirely.
///
/// The window shows nothing while this is on. It is for unattended runs.
#[derive(Resource, Clone)]
pub struct OffscreenTarget(pub Handle<Image>);

/// Marks the camera once it has been pointed at the offscreen image, so the
/// redirect happens exactly once however late the camera spawns.
#[derive(Component)]
struct RendersOffscreen;

/// The screenshot to take: of the offscreen image when the game is rendering
/// into one, of the window otherwise. Every capture goes through this, or one
/// of them would quietly keep photographing an empty window.
pub fn screenshot_of(offscreen: Option<&OffscreenTarget>) -> Screenshot {
    match offscreen {
        Some(target) => Screenshot::image(target.0.clone()),
        None => Screenshot::primary_window(),
    }
}

/// The window's LOGICAL size (see main.rs), so the offscreen view frames the
/// same patch of space and lays the HUD out identically at scale 1.0.
const OFFSCREEN_SIZE: (u32, u32) = (1280, 720);

fn create_offscreen_target(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    use bevy::render::render_resource::{TextureFormat, TextureUsages};
    let mut image = Image::new_target_texture(
        OFFSCREEN_SIZE.0,
        OFFSCREEN_SIZE.1,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    // `new_target_texture` sets the flags for drawing INTO the image. A
    // screenshot copies it back OUT, which needs COPY_SRC as well -- without
    // it this captures black for a reason that has nothing to do with macOS,
    // and would be misread as "offscreen rendering doesn't work".
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    commands.insert_resource(OffscreenTarget(images.add(image)));
    info!("capture: rendering offscreen at {}x{}", OFFSCREEN_SIZE.0, OFFSCREEN_SIZE.1);
}

fn aim_camera_offscreen(
    mut commands: Commands,
    target: Res<OffscreenTarget>,
    cameras: Query<Entity, (With<crate::camera::MainCamera>, Without<RendersOffscreen>)>,
) {
    for camera in &cameras {
        commands.entity(camera).insert((
            bevy::camera::RenderTarget::Image(bevy::camera::ImageRenderTarget {
                handle: target.0.clone(),
                scale_factor: 1.0,
            }),
            // Without this the HUD vanishes from every frame. Bevy draws UI on
            // the camera marked IsDefaultUiCamera, or failing that the highest
            // camera targeting the PRIMARY WINDOW -- and an image target is
            // neither, so the whole interface was drawing to nowhere. No UI
            // root in this game sets UiTargetCamera, so this one marker moves
            // all of it.
            bevy::ui::IsDefaultUiCamera,
            RendersOffscreen,
        ));
    }
}

/// A key press script read from `AFTERLIGHT_KEYS`. See the header for the format.
#[derive(Resource, Default)]
struct ScriptedKeys {
    /// (seconds, key, hold seconds), sorted by time. Hold 0 is a one-frame tap.
    events: Vec<(f32, ScriptButton, f32)>,
    next: usize,
    /// Keys currently down and when to let them go. A tap releases the next
    /// frame -- one frame down is what `just_pressed` reads.
    held: Vec<(ScriptButton, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ScriptButton {
    Key(KeyCode),
    Mouse(MouseButton),
}

fn parse_button(name: &str) -> Option<ScriptButton> {
    match name.trim() {
        "MouseLeft" => Some(ScriptButton::Mouse(MouseButton::Left)),
        "MouseRight" => Some(ScriptButton::Mouse(MouseButton::Right)),
        other => parse_key(other).map(ScriptButton::Key),
    }
}

/// Key names as Bevy spells them. Only what a script plausibly needs: letters,
/// digits, function keys and the few named keys the game binds.
fn parse_key(name: &str) -> Option<KeyCode> {
    let name = name.trim();
    if let Some(c) = name.strip_prefix("Key").filter(|c| c.len() == 1) {
        let i = c.chars().next()? as u8;
        if (b'A'..=b'Z').contains(&i) {
            const L: [KeyCode; 26] = [
                KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE,
                KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI, KeyCode::KeyJ,
                KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO,
                KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR, KeyCode::KeyS, KeyCode::KeyT,
                KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY,
                KeyCode::KeyZ,
            ];
            return Some(L[(i - b'A') as usize]);
        }
    }
    if let Some(d) = name.strip_prefix("Digit").filter(|d| d.len() == 1) {
        const D: [KeyCode; 10] = [
            KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4,
            KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
        ];
        return d.parse::<usize>().ok().map(|n| D[n]);
    }
    Some(match name {
        "F1" => KeyCode::F1, "F2" => KeyCode::F2, "F3" => KeyCode::F3, "F4" => KeyCode::F4,
        "F5" => KeyCode::F5, "F6" => KeyCode::F6, "F7" => KeyCode::F7, "F8" => KeyCode::F8,
        "F9" => KeyCode::F9, "F10" => KeyCode::F10, "F11" => KeyCode::F11, "F12" => KeyCode::F12,
        "Tab" => KeyCode::Tab,
        "Escape" => KeyCode::Escape,
        "Enter" => KeyCode::Enter,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "Space" => KeyCode::Space,
        "Period" => KeyCode::Period,
        "Semicolon" => KeyCode::Semicolon,
        "BracketLeft" => KeyCode::BracketLeft,
        "BracketRight" => KeyCode::BracketRight,
        "ShiftLeft" => KeyCode::ShiftLeft,
        "Minus" => KeyCode::Minus,
        "Equal" => KeyCode::Equal,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "ControlLeft" => KeyCode::ControlLeft,
        "Backslash" => KeyCode::Backslash,
        _ => return None,
    })
}

/// "3:KeyM,5:F7" -> [(3.0, M), (5.0, F7)]. A malformed entry is reported and
/// skipped rather than aborting the run: a typo in one step should not cost
/// the whole capture session.
fn parse_script(script: &str) -> Vec<(f32, ScriptButton, f32)> {
    let mut events: Vec<(f32, ScriptButton, f32)> = script
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .filter_map(|step| {
            let parsed = (|| {
                let (t, rest) = step.split_once(':')?;
                let (k, hold) = match rest.split_once('~') {
                    Some((k, h)) => (k, h.trim().parse::<f32>().ok()?),
                    None => (rest, 0.0),
                };
                Some((t.trim().parse::<f32>().ok()?, parse_button(k)?, hold))
            })();
            if parsed.is_none() {
                warn!("AFTERLIGHT_KEYS: ignoring '{}'", step.trim());
            }
            parsed
        })
        .collect();
    events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    events
}

fn play_scripted_keys(
    time: Res<Time<Real>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut script: ResMut<ScriptedKeys>,
) {
    let now = time.elapsed_secs();
    let mut set = |b: ScriptButton, down: bool| match (b, down) {
        (ScriptButton::Key(k), true) => keyboard.press(k),
        (ScriptButton::Key(k), false) => keyboard.release(k),
        (ScriptButton::Mouse(m), true) => mouse.press(m),
        (ScriptButton::Mouse(m), false) => mouse.release(m),
    };
    // Release what is due; keep pressing what is still being held, since
    // ButtonInput forgets nothing on its own but other systems may release.
    script.held.retain(|&(key, until)| {
        if now >= until {
            set(key, false);
            false
        } else {
            set(key, true);
            true
        }
    });
    while let Some(&(at, key, hold)) = script.events.get(script.next) {
        if now < at {
            break;
        }
        set(key, true);
        // A tap's release time is "now", so it lets go on the next frame.
        script.held.push((key, now + hold));
        script.next += 1;
    }
}

#[derive(Resource)]
struct Doom {
    times: Vec<f32>,
    /// (victim, attacker, started at)
    active: Vec<(Entity, Option<Entity>, f32)>,
}

fn doom(
    time: Res<Time<Real>>,
    mut doom: ResMut<Doom>,
    pin: Option<Res<crate::camera::CameraPin>>,
    camera: Query<&Projection, With<crate::camera::MainCamera>>,
    ships: Query<(Entity, &GlobalTransform, &crate::ai_ship::components::AiShipType, &crate::ai_ship::components::AiShipState, &Children), (With<AiShip>, Without<crate::diplomacy::FactionStation>)>,
    blocks: Query<&GlobalTransform, Or<(With<crate::components::Module>, With<crate::components::HullSegment>)>>,
    mut hits: MessageWriter<crate::events::AiShipDamaged>,
) {
    let now = time.elapsed_secs();
    let focus = pin.and_then(|p| p.0).unwrap_or(Vec2::ZERO);
    while doom.times.first().is_some_and(|t| now >= *t) {
        doom.times.remove(0);
        let taken: Vec<Entity> = doom.active.iter().map(|a| a.0).collect();
        let alive = || ships.iter().filter(|(e, _, _, st, _)| !st.is_destroyed && !taken.contains(e));
        // The ship in shot that's under fire and worst hurt, so the kill
        // lands where the rounds have been landing; else the nearest one.
        // In shot: inside a 1280x720 frame at this zoom, with a margin.
        let zoom = match camera.single() {
            Ok(Projection::Orthographic(o)) => o.scale,
            _ => 10.0,
        };
        let in_shot = |at: Vec2| {
            let off = (at - focus).abs();
            off.x < 540.0 * zoom && off.y < 290.0 * zoom
        };
        let victim = alive()
            .filter(|(_, gt, _, st, _)| st.last_hit_timer < 2.0 && in_shot(gt.translation().truncate()))
            .min_by(|a, b| a.3.hull_integrity.total_cmp(&b.3.hull_integrity))
            .or_else(|| alive().min_by(|a, b| a.1.translation().truncate().distance(focus).total_cmp(&b.1.translation().truncate().distance(focus))));
        if let Some((v, vt, vty, vst, _)) = victim {
            let at = vt.translation().truncate();
            let attacker = vst.last_attacker.or_else(|| ships
                .iter()
                .filter(|(_, _, t, st, _)| !st.is_destroyed && crate::ai_ship::components::factions_hostile(**t, *vty))
                .min_by(|a, b| a.1.translation().truncate().distance(at).total_cmp(&b.1.translation().truncate().distance(at)))
                .map(|(e, ..)| e));
            info!("DOOM t={now:.1}: taking apart {:?} {:?}", v, vty);
            doom.active.push((v, attacker, now));
        }
    }
    let mut rng = rand::thread_rng();
    use rand::Rng;
    doom.active.retain(|(v, attacker, started)| {
        let Ok((_, gt, _, st, children)) = ships.get(*v) else { return false };
        if st.is_destroyed {
            return false;
        }
        let since = now - started;
        let kids: Vec<Vec2> = children.iter().filter_map(|c| blocks.get(c).ok()).map(|g| g.translation().truncate()).collect();
        if kids.is_empty() {
            return false;
        }
        // a heavy hit every so often for a second and a half, then the end
        if rng.gen_range(0.0..1.0) < 0.22 && since < 1.5 {
            let p = kids[rng.gen_range(0..kids.len())];
            hits.write(crate::events::AiShipDamaged {
                target: *v, source: crate::events::DamageSource::Explosion, amount: 900.0,
                position: Some(p), direction: None, attacker: *attacker,
            });
        }
        if since >= 1.5 {
            hits.write(crate::events::AiShipDamaged {
                target: *v, source: crate::events::DamageSource::Explosion, amount: 1.0e7,
                position: Some(gt.translation().truncate()), direction: None, attacker: *attacker,
            });
            return false;
        }
        true
    });
}

/// AFTERLIGHT_ROT_TRACE=1: every 2 s, log each AI ship whose nose reversed its
/// turning direction (a nod) and what it was doing at the time.
#[derive(Default)]
struct RotTrack {
    last: f32,
    last_w: f32,
    flips: u32,
    max_w: f32,
    /// Heading at the last reversal, and the biggest swing between two.
    at_flip: f32,
    swing: f32,
    dest: Option<Vec2>,
    dest_moved: f32,
    face: Option<f32>,
    /// Total turn this window, either way (a ship spinning on the spot).
    turned: f32,
}

fn rot_trace(
    time: Res<Time>,
    mut acc: Local<f32>,
    mut track: Local<bevy::platform::collections::HashMap<Entity, RotTrack>>,
    ships: Query<(Entity, &Transform, &crate::components::Velocity, &crate::ai_ship::components::AiShipBehavior,
                  &crate::ai_ship::components::AiShipNav, &crate::ai_ship::components::AiShipType,
                  Option<&crate::ai_ship::fleet::FleetMember>), With<AiShip>>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (e, tf, _, _, nav, ..) in &ships {
        let a = tf.rotation.to_euler(EulerRot::ZYX).0;
        let t = track.entry(e).or_insert(RotTrack { last: a, at_flip: a, ..default() });
        let d = (a - t.last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let w = d / dt;
        if w.abs() > 0.05 && t.last_w.abs() > 0.05 && w.signum() != t.last_w.signum() {
            let sw = ((a - t.at_flip + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs();
            // only swings you could see count as a nod
            if sw > 0.08 {
                t.flips += 1;
                t.swing = t.swing.max(sw);
            }
            t.at_flip = a;
        }
        if let (Some(p), Some(n)) = (t.dest, nav.destination) {
            t.dest_moved += p.distance(n);
        }
        t.dest = nav.destination;
        t.face = nav.face;
        if w.abs() > 0.05 {
            t.last_w = w;
        }
        t.max_w = t.max_w.max(w.abs());
        t.turned += d.abs();
        t.last = a;
    }
    *acc += dt;
    if *acc < 2.0 {
        return;
    }
    *acc = 0.0;
    let mut nodders = 0;
    for (e, tf, vel, behavior, nav, ty, member) in &ships {
        let Some(t) = track.get_mut(&e) else { continue };
        if t.flips >= 2 {
            nodders += 1;
            let to_dest = nav.destination.map(|d| d.distance(tf.translation.truncate()));
            info!(
                "ROT_TRACE nod {:?} {:?} {:?} role={} flips={} swing={:.2} max_w={:.2} speed={:.0} to_dest={:?} dest_moved={:.0} face={}",
                e, ty, behavior,
                member.map_or("solo", |m| if m.leader { "leader" } else { "escort" }),
                t.flips, t.swing, t.max_w, vel.0.length(), to_dest.map(|d| d as i32), t.dest_moved, t.face.is_some()
            );
        }
        if t.turned > 1.5 {
            let to_dest = nav.destination.map(|d| d.distance(tf.translation.truncate()));
            info!(
                "ROT_TRACE spin {:?} {:?} {:?} role={} turned={:.1} speed={:.0} to_dest={:?} waypoints={} at={:?}",
                e, ty, behavior,
                member.map_or("solo", |m| if m.leader { "leader" } else { "escort" }),
                t.turned, vel.0.length(), to_dest.map(|d| d as i32), nav.waypoints.len(), tf.translation.truncate().as_ivec2()
            );
        }
        t.flips = 0;
        t.max_w = 0.0;
        t.swing = 0.0;
        t.dest_moved = 0.0;
        t.turned = 0.0;
    }
    info!("ROT_TRACE {} ships, {} nodding", ships.iter().count(), nodders);
}

/// On a station part already shown whole.
#[derive(Component)]
struct SkinShown;

/// AFTERLIGHT_SHOW_SHIPS=1: every AI ship shown whole. An AI hull hides its
/// insides until breached; in the prologue everything is out in the open.
/// Faction stations also drop their material tint, so a reskin shows true.
fn show_ships_whole(
    mut commands: Commands,
    stations: Query<(), With<crate::diplomacy::FactionStation>>,
    mut parts: Query<
        (Entity, &crate::ai_ship::components::OwnedByAiShip, &mut Sprite, Has<crate::components::HullSegment>, Option<&crate::ai_ship::interior::Concealed>),
        Without<SkinShown>,
    >,
) {
    for (entity, owner, mut sprite, hull, concealed) in &mut parts {
        let station = stations.contains(owner.root);
        if !station && concealed.is_none() {
            commands.entity(entity).insert(SkinShown);
            continue;
        }
        let color = if hull && station { Color::WHITE } else { concealed.map_or(sprite.color, |c| c.0) };
        sprite.color = color;
        commands
            .entity(entity)
            .insert((crate::components::BaseSpriteColor(color), SkinShown))
            .remove::<crate::ai_ship::interior::Concealed>();
    }
}

/// AFTERLIGHT_FLY_TO=station: fly to the nearest station the way a player would --
/// nose on it, burn, ease off as the gap closes, brake, then F to dock.
fn fly_to_station(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut input: ResMut<crate::resources::InputState>,
    ship: Query<(&Transform, &crate::components::Velocity), With<crate::components::Ship>>,
    cores: Query<(&GlobalTransform, &crate::diplomacy::stations::StationAnchor), With<crate::diplomacy::stations::StationCore>>,
    mut last_f: Local<f32>,
    mut last_log: Local<f32>,
) {
    // What to hold this frame; everything else of ours is let go, so a held
    // key stays one press rather than a fresh one every frame.
    let mut want_keys: Vec<KeyCode> = Vec::new();
    let mut apply = |keyboard: &mut ButtonInput<KeyCode>, want_keys: &[KeyCode]| {
        for key in [KeyCode::KeyW, KeyCode::ShiftLeft, KeyCode::KeyF] {
            if want_keys.contains(&key) {
                keyboard.press(key);
            } else if keyboard.pressed(key) {
                keyboard.release(key);
            }
        }
    };
    if *state.get() != GameState::Exploring {
        apply(&mut keyboard, &want_keys);
        return;
    }
    let Ok((tf, vel)) = ship.single() else { return };
    let pos = tf.translation.truncate();
    let Some((at, extent)) = cores
        .iter()
        .map(|(t, a)| (t.translation().truncate(), a.extent))
        .min_by(|a, b| a.0.distance(pos).total_cmp(&b.0.distance(pos)))
    else {
        apply(&mut keyboard, &want_keys);
        return;
    };
    let to = at - pos;
    let gap = (to.length() - extent).max(0.0);
    input.gamepad_aim = Some(to.normalize_or(Vec2::X));
    // Closing speed wanted for this gap: brisk far out, a crawl alongside.
    let want = (gap * 0.45).clamp(60.0, 1_300.0);
    let closing = vel.0.dot(to.normalize_or(Vec2::X));
    if time.elapsed_secs() - *last_log > 2.0 {
        *last_log = time.elapsed_secs();
        info!("FLY_TO: gap {gap:.0} (centre {:.0}, reach {extent:.0}) closing {closing:.0} want {want:.0} speed {:.0}", to.length(), vel.0.length());
    }
    if gap < 1_800.0 && vel.0.length() < 140.0 {
        if time.elapsed_secs() - *last_f > 1.5 {
            *last_f = time.elapsed_secs();
            want_keys.push(KeyCode::KeyF);
        }
    } else if closing < want - 60.0 {
        want_keys.push(KeyCode::KeyW);
    } else if closing > want + 120.0 || (gap < 1_800.0 && vel.0.length() >= 140.0) {
        want_keys.push(KeyCode::ShiftLeft);
    }
    apply(&mut keyboard, &want_keys);
}

/// Marker so the capture plugin can be added unconditionally and cost nothing
/// when neither the env var nor the key is used.
pub struct CapturePlugin;

#[derive(Resource)]
struct CaptureState {
    every: Option<f32>,
    /// AFTERLIGHT_SHOTS_AFTER: real seconds before periodic capture starts.
    after: f32,
    since: f32,
    index: u32,
    dir: String,
}

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        let every = std::env::var("AFTERLIGHT_SHOTS")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v > 0.0);
        let dir = std::env::var("AFTERLIGHT_SHOTS_DIR")
            .unwrap_or_else(|_| "/tmp/afterlight_shots".to_string());
        if every.is_some() {
            info!("capture: every {}s into {}  (F7 for one now)", every.unwrap(), dir);
        }
        let after = std::env::var("AFTERLIGHT_SHOTS_AFTER").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        app.insert_resource(CaptureState { every, after, since: 0.0, index: 0, dir })
            .add_systems(Update, capture_frames);
        if let Ok(script) = std::env::var("AFTERLIGHT_KEYS") {
            let events = parse_script(&script);
            info!("AFTERLIGHT_KEYS: {} scripted presses", events.len());
            app.insert_resource(ScriptedKeys { events, ..default() }).add_systems(
                PreUpdate,
                // Before the F token is armed from the keyboard, or a
                // scripted F lands a frame the docking handlers never see.
                play_scripted_keys
                    .after(bevy::input::InputSystems)
                    .before(crate::resources::refresh_interact_press),
            );
        }
        if std::env::var("AFTERLIGHT_OFFSCREEN").ok().as_deref() == Some("1") {
            app.add_systems(Startup, create_offscreen_target)
                .add_systems(
                    Update,
                    aim_camera_offscreen.run_if(resource_exists::<OffscreenTarget>),
                );
        }
    }
}

fn capture_frames(
    mut commands: Commands,
    time: Res<Time>,
    real: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut st: ResMut<CaptureState>,
    offscreen: Option<Res<OffscreenTarget>>,
) {
    let on_demand = keys.just_pressed(KeyCode::F7);

    let periodic = match st.every {
        Some(_) if real.elapsed_secs() < st.after => false,
        Some(every) => {
            st.since += time.delta_secs();
            if st.since >= every {
                st.since = 0.0;
                true
            } else {
                false
            }
        }
        None => false,
    };

    if !on_demand && !periodic {
        return;
    }

    if std::fs::create_dir_all(&st.dir).is_err() {
        warn!("capture: cannot create {}", st.dir);
        return;
    }
    let path = format!("{}/shot_{:04}.png", st.dir, st.index);
    st.index += 1;
    if on_demand {
        info!("capture: {}", path);
    }
    commands
        .spawn(screenshot_of(offscreen.as_deref()))
        .observe(save_to_disk(path));
}

#[cfg(test)]
mod offscreen_tests {
    use super::*;
    use bevy::render::render_resource::TextureUsages;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.add_systems(Startup, create_offscreen_target);
        app.add_systems(
            Update,
            aim_camera_offscreen.run_if(resource_exists::<OffscreenTarget>),
        );
        app
    }

    /// The image has to be readable BACK out. `new_target_texture` only sets
    /// the flags for drawing into it; without COPY_SRC every capture comes
    /// back black -- the same symptom as a covered window, for an unrelated
    /// reason, and exactly the kind that gets misdiagnosed.
    #[test]
    fn the_target_can_be_drawn_into_and_read_back_out() {
        let mut app = app();
        app.update();
        let handle = app.world().resource::<OffscreenTarget>().0.clone();
        let usage = app.world().resource::<Assets<Image>>().get(&handle).unwrap().texture_descriptor.usage;
        assert!(usage.contains(TextureUsages::RENDER_ATTACHMENT), "cannot be rendered into");
        assert!(usage.contains(TextureUsages::COPY_SRC), "cannot be read back -- every capture would be black");
    }

    /// Matches the window's logical size, so the frame shows the same patch
    /// of space and the HUD lays out as it does on screen.
    #[test]
    fn the_target_matches_the_window() {
        let mut app = app();
        app.update();
        let handle = app.world().resource::<OffscreenTarget>().0.clone();
        let image = app.world().resource::<Assets<Image>>().get(&handle).unwrap();
        assert_eq!((image.width(), image.height()), OFFSCREEN_SIZE);
    }

    /// The camera is redirected AND made the UI camera. The second half is the
    /// one that bit: Bevy draws UI on the IsDefaultUiCamera, or failing that
    /// the highest camera targeting the primary window. An image target is
    /// neither, so without the marker the whole HUD drew to nowhere and every
    /// frame showed the world with no interface at all.
    #[test]
    fn the_camera_renders_offscreen_with_its_hud() {
        let mut app = app();
        let camera = app.world_mut().spawn(crate::camera::MainCamera).id();
        app.update();
        app.update();
        let world = app.world();
        assert!(
            matches!(world.get::<bevy::camera::RenderTarget>(camera), Some(bevy::camera::RenderTarget::Image(_))),
            "camera still renders to the window"
        );
        assert!(
            world.get::<bevy::ui::IsDefaultUiCamera>(camera).is_some(),
            "camera renders offscreen but is not the UI camera -- the HUD would be missing"
        );
    }

    /// Redirected once, not every frame. A second insert each frame would
    /// churn the camera's render target for nothing.
    #[test]
    fn the_camera_is_redirected_once() {
        let mut app = app();
        let camera = app.world_mut().spawn(crate::camera::MainCamera).id();
        for _ in 0..4 {
            app.update();
        }
        assert!(app.world().get::<RendersOffscreen>(camera).is_some());
    }
}
