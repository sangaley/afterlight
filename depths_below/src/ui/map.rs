//! The map: one view of everything you know, from the whole galaxy down to a
//! planet, on one scroll wheel.
//!
//! There used to be two maps, a local one and a galaxy one, in different
//! coordinates, flipped with Tab. Now there is one camera over galaxy
//! coordinates. A system's insides are drawn around its place in the galaxy,
//! squeezed into a disc small enough never to touch a neighbour (SYSTEM_DISC),
//! and fade in as the disc grows on screen, so zooming goes straight through.
//!
//! What's known is gathered once into a `Scene` (when the map opens, when you
//! pick something, and a few times a second on Live) and everything on screen
//! is painted from that. Moving the camera only repaints: no queries, no
//! rolling of systems. A system you aren't in comes from its seed, the same
//! rolls that place it when it loads, so what you see is what you'd arrive at.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::theme::{ThemeColors as C, ThemeFonts as F};
use super::{
    format_range_km, warp_dash_arrival_buffer, warp_dash_charge_time, warp_dash_fuel_cost,
    warp_key_pressed, MapOverlay, PendingWarpTarget,
};
use crate::ai_ship::components::{
    faction_display_name, faction_map_color, AiShip, AiShipType, BountyTarget, SimBehavior, WorldSimulation,
};
use crate::ai_ship::fleet::{CivilianHull, FleetKind, FleetMember, SimFleet};
use crate::celestial::galaxy::{roman, system_roll, GALAXY_RADIUS, SNAP_TOLERANCE};
use crate::celestial::resources::{
    GalaxyMap, GalaxyWarpTarget, PendingGalaxyWarpTarget, StarSystemDef, SystemDiscovery, SystemStreamingManager,
};
use crate::celestial::territory::{border_factions, BorderWar};
use crate::celestial::warp::{
    interstellar_charge_time, interstellar_fuel_cost, INTERSTELLAR_BASE_FUEL, INTERSTELLAR_MAX_EXTRA_FUEL,
};
use crate::components::Ship;
use crate::diplomacy::FactionStanding;
use crate::resources::{FuelState, HullState, Inventory};
use crate::states::{BuildState, GameState};

const MAP_HOSTILE: Color = Color::srgb(1.0, 0.15, 0.15);
const MAP_NEUTRAL: Color = Color::srgb(0.62, 0.66, 0.72);
/// Words about a hostile system: MAP_HOSTILE is too dark to read as text.
pub(super) const HOSTILE_TEXT: Color = Color::srgb(1.0, 0.38, 0.35);
const MAP_TRADER: Color = Color::srgb(0.35, 0.85, 0.75);
const MAP_YOU: Color = Color::srgb(0.3, 0.9, 1.0);
const MAP_TARGET: Color = Color::srgb(1.0, 0.85, 0.4);
const MAP_BOUNTY: Color = Color::srgb(1.0, 0.2, 0.85);
const MAP_ORBIT: Color = Color::srgba(0.45, 0.53, 0.72, 0.45);
const MAP_GROUND: Color = Color::srgb(0.008, 0.012, 0.04);
const REACH_ONE_WAY: Color = Color::srgb(0.95, 0.55, 0.2);
const REACH_RETURN: Color = Color::srgb(0.9, 0.75, 0.25);

/// How far from its place in the galaxy a system's insides are drawn, in
/// galaxy units. Under half MIN_SYSTEM_SEPARATION, so two discs never touch.
const SYSTEM_DISC: f32 = SNAP_TOLERANCE;
/// A system's own space, fitted into the disc: never closer than this...
const MIN_SYSTEM_RANGE: f32 = 600_000.0;
/// ...and with this much room outside its furthest orbit.
const SYSTEM_FIT_MARGIN: f32 = 1.08;
/// The disc's radius on screen, in pixels, where a system starts to open, and
/// where it's fully open.
const OPENS_AT_PX: (f32, f32) = (70.0, 190.0);

const SIDE_WIDTH: f32 = 300.0;
const ORBIT_SEGMENTS: usize = 64;
const REACH_SEGMENTS: usize = 96;

/// How much one wheel notch zooms.
const ZOOM_PER_NOTCH: f32 = 1.3;
/// How fast the camera catches up with where you scrolled it, per second.
const CAMERA_EASE: f32 = 14.0;
/// How far in past a system filling the view you can go.
const MAX_ZOOM_PAST_FILL: f32 = 4.0;
/// Pixels of trackpad scroll that make one wheel notch.
const PIXELS_PER_NOTCH: f32 = 40.0;
/// Scrolled this far out with the flight camera already at its limit opens the
/// map, so reaching the limit in a fight doesn't pause the game.
const NOTCHES_FROM_FLIGHT: f32 = 3.0;
/// And this far in past the limit, over your own system, closes it again.
const NOTCHES_TO_FLIGHT: f32 = 2.0;
/// A swipe keeps sending scroll after it opens the map. Ignored for this long,
/// in real seconds, so it doesn't carry straight on out to the galaxy.
const OPENING_HOLD: f64 = 0.35;
/// A press that moves further than this is a drag, not a click.
const DRAG_PX: f32 = 5.0;
/// On Live, how often the scene is gathered again, in real seconds.
const LIVE_REFRESH: f32 = 0.2;

// ---------------------------------------------------------------------------
// The camera
// ---------------------------------------------------------------------------

/// Where the map looks, and how close. `scale` is pixels per galaxy unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapCam {
    pub center: Vec2,
    pub scale: f32,
}

impl Default for MapCam {
    fn default() -> Self {
        Self { center: Vec2::ZERO, scale: 1e-4 }
    }
}

fn whole_galaxy_scale(stage: Vec2) -> f32 {
    stage.min_element() / 2.0 / (GALAXY_RADIUS * 1.05)
}

/// The scale at which one system fills the view.
fn system_fill_scale(stage: Vec2) -> f32 {
    stage.min_element() / 2.0 * 0.86 / SYSTEM_DISC
}

/// 0 at the galaxy, 1 once a system is open.
fn openness(scale: f32) -> f32 {
    let t = ((SYSTEM_DISC * scale - OPENS_AT_PX.0) / (OPENS_AT_PX.1 - OPENS_AT_PX.0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl MapCam {
    /// A galaxy position to a pixel on the stage. North is up.
    fn to_px(self, pos: Vec2, stage: Vec2) -> Vec2 {
        let rel = (pos - self.center) * self.scale;
        stage / 2.0 + Vec2::new(rel.x, -rel.y)
    }

    /// The inverse of `to_px`, so a click lands where a thing was drawn.
    fn from_px(self, px: Vec2, stage: Vec2) -> Vec2 {
        let rel = (px - stage / 2.0) / self.scale;
        self.center + Vec2::new(rel.x, -rel.y)
    }

    /// Never past the zoom limits, nor looking off the edge of the galaxy, so
    /// zooming out always ends on all of it.
    fn framed(self, stage: Vec2) -> Self {
        let scale = self.scale.clamp(whole_galaxy_scale(stage), system_fill_scale(stage) * MAX_ZOOM_PAST_FILL);
        let half_view = stage / 2.0 / scale;
        let room = (Vec2::splat(GALAXY_RADIUS * 1.05) - half_view).max(Vec2::ZERO);
        Self { center: self.center.clamp(-room, room), scale }
    }

    /// Zoom by wheel notches, keeping what's under the cursor under the cursor.
    fn scrolled(self, notches: f32, cursor: Vec2, stage: Vec2) -> Self {
        let anchor = self.from_px(cursor, stage);
        let scale = self.scale * ZOOM_PER_NOTCH.powf(notches);
        let next = Self { center: Vec2::ZERO, scale };
        let drift = next.from_px(cursor, stage);
        Self { center: anchor - drift, scale }.framed(stage)
    }

    fn at_max_zoom(self, stage: Vec2) -> bool {
        self.scale >= system_fill_scale(stage) * MAX_ZOOM_PAST_FILL * 0.999
    }
}

/// The chips along the top: three places to look from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapLevel {
    Galaxy,
    Known,
    Here,
}

fn level_cam(level: MapLevel, scene: &Scene, stage: Vec2) -> MapCam {
    match level {
        MapLevel::Galaxy => MapCam { center: Vec2::ZERO, scale: whole_galaxy_scale(stage) },
        MapLevel::Here => MapCam { center: scene.here.galaxy, scale: system_fill_scale(stage) },
        MapLevel::Known => {
            let mut lo = scene.you;
            let mut hi = scene.you;
            for sys in &scene.systems {
                lo = lo.min(sys.galaxy);
                hi = hi.max(sys.galaxy);
            }
            // Room for the top bar and the chips, and for names to the right.
            let span = (hi - lo).max(Vec2::splat(1.0));
            let room = (stage - Vec2::new(260.0, 180.0)).max(Vec2::splat(100.0));
            let scale = (room.x / span.x).min(room.y / span.y);
            // A lone system still reads as a place on a chart, not its insides.
            let closest = OPENS_AT_PX.0 * 0.9 / SYSTEM_DISC;
            MapCam { center: (lo + hi) / 2.0, scale: scale.min(closest) }.framed(stage)
        }
    }
}

// ---------------------------------------------------------------------------
// The scene: what's known, gathered once
// ---------------------------------------------------------------------------

/// How a system's own coordinates sit on the galaxy map.
#[derive(Clone, Copy, Debug)]
struct SystemFrame {
    galaxy: Vec2,
    local: Vec2,
    squeeze: f32,
}

impl SystemFrame {
    fn to_galaxy(self, world: Vec2) -> Vec2 {
        self.galaxy + (world - self.local) * self.squeeze
    }

    fn to_world(self, galaxy: Vec2) -> Vec2 {
        self.local + (galaxy - self.galaxy) / self.squeeze
    }
}

/// One line of a system's description. `lead` is the one to read first.
#[derive(Clone, Debug)]
struct BriefLine {
    text: String,
    color: Color,
    lead: bool,
}

struct ScenePlanet {
    name: String,
    kind: String,
    color: Color,
    world: Vec2,
    galaxy: Vec2,
}

struct SceneStation {
    name: String,
    kind: &'static str,
    color: Color,
    world: Vec2,
    galaxy: Vec2,
}

struct SceneShips {
    name: String,
    /// Under the name: what it's carrying and where, or what it's doing.
    doing: String,
    faction: AiShipType,
    color: Color,
    count: usize,
    heading: f32,
    bounty: bool,
    world: Vec2,
    galaxy: Vec2,
}

struct SceneSystem {
    id: u32,
    name: String,
    galaxy: Vec2,
    owner: Option<AiShipType>,
    color: Color,
    visited: bool,
    capital: bool,
    haunted: bool,
    contested: bool,
    hostile: bool,
    here: bool,
    /// How far the Cascade has it (narrative::front): 1.0 taken.
    dark: f32,
    brief: Vec<BriefLine>,
    star: String,
    frame: SystemFrame,
    /// Semi-major axis and eccentricity, in the system's own units.
    orbits: Vec<(f32, f32)>,
    planets: Vec<ScenePlanet>,
    stations: Vec<SceneStation>,
    ships: Vec<SceneShips>,
}

#[derive(Default)]
struct Scene {
    systems: Vec<SceneSystem>,
    here: HereFrame,
    here_id: Option<u32>,
    here_name: String,
    you: Vec2,
    you_world: Vec2,
    you_heading: f32,
    target: Option<Vec2>,
    fuel: f32,
    max_fuel: f32,
    hull: f32,
    hold: (f32, f32),
    cargo: Vec<(String, u32)>,
    /// The tear where the summit station was (narrative::summit): galaxy
    /// position and radius, once the black box has been found.
    scar: Option<(Vec2, f32)>,
}

/// Where you are on the map: your system, or open space after a blind jump.
#[derive(Clone, Copy, Debug)]
struct HereFrame {
    galaxy: Vec2,
    frame: SystemFrame,
}

impl Default for HereFrame {
    fn default() -> Self {
        Self { galaxy: Vec2::ZERO, frame: SystemFrame { galaxy: Vec2::ZERO, local: Vec2::ZERO, squeeze: 1.0 } }
    }
}

impl Scene {
    fn system(&self, id: u32) -> Option<&SceneSystem> {
        self.systems.iter().find(|s| s.id == id)
    }
}

/// A ship's colour on the map: what it is to you.
fn ship_color(standing: &FactionStanding, faction: AiShipType, civilian: bool) -> Color {
    if civilian {
        MAP_TRADER
    } else if standing.attacks_player(faction) {
        MAP_HOSTILE
    } else {
        MAP_NEUTRAL
    }
}

fn planet_color(kind: crate::celestial::components::PlanetType) -> Color {
    use crate::celestial::components::PlanetType::*;
    match kind {
        Lava | Volcanic => Color::srgb(0.85, 0.42, 0.25),
        Desert => Color::srgb(0.8, 0.66, 0.42),
        Ocean => Color::srgb(0.3, 0.55, 0.9),
        Terran => Color::srgb(0.38, 0.72, 0.5),
        Toxic => Color::srgb(0.62, 0.72, 0.3),
        Ice => Color::srgb(0.7, 0.85, 0.95),
        _ => Color::srgb(0.58, 0.6, 0.68),
    }
}

/// Whose a system is, what they think of you, and what haunts it.
fn system_brief(sys: &StarSystemDef, galaxy_map: &GalaxyMap, standing: &FactionStanding, war: &BorderWar) -> Vec<BriefLine> {
    // The Cascade first: nothing else about a taken system matters now.
    if crate::narrative::front::is_taken(sys) {
        return vec![
            BriefLine { text: "Taken by the Cascade".into(), color: C::TEXT_MUTED, lead: true },
            BriefLine { text: "Nothing there moves. Nothing answers.".into(), color: C::TEXT_MUTED, lead: false },
        ];
    }
    let mut lines = system_brief_living(sys, galaxy_map, standing, war);
    if sys.dark > 0.0 {
        lines.insert(0, BriefLine { text: format!("The dark is reaching it ({:.0}%)", sys.dark * 100.0), color: C::ACCENT_ORANGE, lead: true });
    }
    lines
}

fn system_brief_living(sys: &StarSystemDef, galaxy_map: &GalaxyMap, standing: &FactionStanding, war: &BorderWar) -> Vec<BriefLine> {
    let line = |text: String, color: Color| BriefLine { text, color, lead: false };
    let mut lines = vec![if standing.system_hostile(sys) {
        BriefLine { text: "HOSTILE - they attack on sight".to_string(), color: HOSTILE_TEXT, lead: true }
    } else {
        BriefLine { text: "Not hostile".to_string(), color: C::ACCENT_GREEN, lead: true }
    }];
    match sys.faction {
        None => lines.push(line("Nobody's space".to_string(), C::TEXT_SECONDARY)),
        Some(owner) => {
            let what = if sys.capital { "capital" } else { "space" };
            lines.push(line(format!("{} {what}", faction_display_name(owner)), faction_map_color(owner)));
            lines.push(line(format!("They count you {}", standing.tier(owner).label()), C::TEXT_SECONDARY));
        }
    }
    // A capital is a world: its name, its people, what it makes.
    if let (true, Some(owner)) = (sys.capital, sys.faction) {
        let (people, makes) = crate::celestial::capital_worlds::capital_blurb(owner);
        lines.push(line(
            format!("{}: {people}; makes {makes}", crate::celestial::capital_worlds::capital_world_name(&sys.name)),
            C::TEXT_SECONDARY,
        ));
    }
    let borders = border_factions(galaxy_map, sys.id);
    if !borders.is_empty() {
        let names: Vec<&str> = borders.iter().map(|f| faction_display_name(*f)).collect();
        lines.push(line(format!("Borders: {}", names.join(", ")), C::TEXT_MUTED));
    }
    if let Some(boss) = sys.haunt {
        lines.push(line(format!("Haunted by {}", faction_display_name(boss)), Color::srgb(0.95, 0.3, 0.3)));
    }
    if let Some(contest) = war.contest(sys.id) {
        for (claimant, share) in contest.claims.iter().filter(|(_, share)| *share >= 1.0) {
            lines.push(line(
                format!("Contested: {} holds {:.0}%", faction_display_name(*claimant), share),
                Color::srgb(0.95, 0.75, 0.2),
            ));
        }
    }
    lines
}

/// A system as its seed and the politics have it, with no ships yet.
fn chart_system(sys: &StarSystemDef, galaxy_map: &GalaxyMap, standing: &FactionStanding, war: &BorderWar, here: bool) -> SceneSystem {
    let roll = system_roll(sys);
    let furthest = roll.planets.iter()
        .map(|p| p.orbit_distance * (1.0 + p.eccentricity) + p.radius)
        .fold(0.0f32, f32::max);
    let range = (furthest * SYSTEM_FIT_MARGIN).max(MIN_SYSTEM_RANGE);
    let frame = SystemFrame { galaxy: sys.galaxy_pos, local: sys.local_center, squeeze: SYSTEM_DISC / range };
    let planets = roll.planets.iter().enumerate()
        .map(|(i, p)| {
            let r = crate::celestial::orbits::orbit_radius(p.orbit_distance, p.eccentricity, p.phase);
            let world = sys.local_center + Vec2::new(p.phase.cos(), p.phase.sin()) * r;
            ScenePlanet {
                name: format!("{} {}", sys.name, roman(i as u32 + 1)),
                kind: format!("{:?}", p.planet_type),
                color: planet_color(p.planet_type),
                world,
                galaxy: frame.to_galaxy(world),
            }
        })
        .collect();
    let stations = crate::world::home_base::station_sites(sys).iter()
        .map(|s| SceneStation {
            name: s.name.clone(),
            kind: crate::world::station_types::station_type_name(s.kind),
            color: crate::world::home_base::station_accent(s.kind),
            world: s.pos,
            galaxy: frame.to_galaxy(s.pos),
        })
        .collect();
    SceneSystem {
        id: sys.id,
        name: sys.name.clone(),
        galaxy: sys.galaxy_pos,
        owner: sys.faction,
        // A taken system has no colour left.
        color: if crate::narrative::front::is_taken(sys) { Color::srgb(0.32, 0.32, 0.36) } else { sys.faction.map_or(MAP_YOU, faction_map_color) },
        visited: sys.discovery == SystemDiscovery::Visited,
        capital: sys.capital,
        haunted: sys.haunt.is_some(),
        // A rival holds a real share: a quarter or more.
        contested: war.contest(sys.id).is_some_and(|c| c.claims.iter().any(|(_, share)| *share >= 25.0)),
        // A taken system threatens nobody any more; it's simply gone.
        hostile: !crate::narrative::front::is_taken(sys) && standing.system_hostile(sys),
        here,
        dark: sys.dark,
        brief: system_brief(sys, galaxy_map, standing, war),
        star: format!("{:?}", roll.star_class),
        frame,
        orbits: roll.planets.iter().map(|p| (p.orbit_distance, p.eccentricity)).collect(),
        planets,
        stations,
        ships: Vec::new(),
    }
}

fn fleet_words(fleet: &SimFleet) -> (String, String) {
    if let Some(run) = fleet.trade.as_ref() {
        let name = crate::ai_ship::traffic::trader_name(fleet.id, fleet.faction);
        let cargo = crate::ai_ship::traffic::trader_cargo(fleet.id);
        let doing = if run.dwell > 0.0 {
            format!("{cargo}, in port")
        } else if run.bound_for.is_empty() {
            cargo.to_string()
        } else {
            format!("{cargo}, bound for {}", run.bound_for)
        };
        return (name, doing);
    }
    let kind = match fleet.kind {
        FleetKind::Patrol => "patrol",
        FleetKind::BattleGroup => "battle group",
        FleetKind::Convoy => "convoy",
        FleetKind::StationGuard => "station guard",
        FleetKind::Raiders => "raiders",
    };
    (format!("{} {kind}", faction_display_name(fleet.faction)), "On station".to_string())
}

/// Everything the map reads from the world, in one place: Bevy caps how many
/// parameters a system can take.
#[derive(bevy::ecs::system::SystemParam)]
pub struct MapWorld<'w, 's> {
    ai_ships: Query<'w, 's, (&'static Transform, &'static AiShipType, Has<CivilianHull>, Option<&'static FleetMember>), With<AiShip>>,
    bounty_ships: Query<'w, 's, (&'static Transform, &'static BountyTarget), With<AiShip>>,
    planets: Query<'w, 's, (&'static Transform, &'static crate::celestial::components::CelestialBody), With<crate::celestial::components::Planet>>,
    player: Query<'w, 's, &'static Transform, With<Ship>>,
    standing: Res<'w, FactionStanding>,
    sim: Res<'w, WorldSimulation>,
    contracts: Res<'w, crate::contracts::ContractState>,
    streaming: Res<'w, SystemStreamingManager>,
    galaxy_map: Res<'w, GalaxyMap>,
    stations: Res<'w, crate::world::home_base::SystemStations>,
    war: Res<'w, BorderWar>,
    hull: Res<'w, HullState>,
    fuel: Res<'w, FuelState>,
    inventory: Res<'w, Inventory>,
    /// The logs read so far, for the notes they leave on the map.
    stats: Res<'w, crate::resources::Statistics>,
    /// How far the Summit Record has come (the scar).
    contacts: Res<'w, crate::diplomacy::contacts::Contacts>,
}

/// Off-screen ships of one system as fleets on the map.
fn sim_fleets(sys: &mut SceneSystem, sim: &WorldSimulation, standing: &FactionStanding) {
    let mut seen: Vec<u32> = sim.ships.iter()
        .filter(|s| s.system_id == sys.id && !s.spawned && s.behavior != SimBehavior::Dead)
        .map(|s| s.fleet)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    for id in seen {
        let members: Vec<_> = sim.ships.iter()
            .filter(|s| s.system_id == sys.id && !s.spawned && s.behavior != SimBehavior::Dead && s.fleet == id)
            .collect();
        let fleet = sim.fleets.iter().find(|f| f.id == id);
        let faction = members[0].faction;
        let world = members.iter().map(|s| s.position).sum::<Vec2>() / members.len() as f32;
        let (name, doing) = fleet.map_or_else(
            || (faction_display_name(faction).to_string(), "Under way".to_string()),
            fleet_words,
        );
        sys.ships.push(SceneShips {
            name,
            doing,
            faction,
            color: ship_color(standing, faction, fleet.is_some_and(|f| f.trade.is_some())),
            count: members.len(),
            heading: fleet.map_or(0.0, |f| f.heading),
            bounty: false,
            world,
            galaxy: sys.frame.to_galaxy(world),
        });
    }
}

fn gather_scene(world: &MapWorld, dash: Option<Vec2>, jump: Option<GalaxyWarpTarget>) -> Scene {
    let standing = &*world.standing;
    let here_id = world.streaming.loaded_system;
    let player = world.player.single().ok();
    let you_world = player.map_or(Vec2::ZERO, |t| t.translation.truncate());
    let you_heading = player.map_or(0.0, |t| t.rotation.to_euler(EulerRot::ZYX).0);

    let mut systems: Vec<SceneSystem> = world.galaxy_map.systems.iter()
        .filter(|s| s.discovery != SystemDiscovery::Unknown)
        .map(|s| chart_system(s, &world.galaxy_map, standing, &world.war, Some(s.id) == here_id))
        .collect();
    // What the logs said about a system (narrative::leads), under its brief.
    for (id, line, warning) in crate::narrative::leads::notes(&world.galaxy_map, &world.stats.logs_found) {
        if let Some(sys) = systems.iter_mut().find(|s| s.id == id) {
            sys.brief.push(BriefLine {
                text: format!("From a log: {line}"),
                color: if warning { C::ACCENT_ORANGE } else { C::ACCENT_YELLOW },
                lead: false,
            });
        }
    }

    let mut here = HereFrame {
        galaxy: world.streaming.current_galaxy_pos,
        frame: SystemFrame {
            galaxy: world.streaming.current_galaxy_pos,
            local: you_world,
            squeeze: SYSTEM_DISC / MIN_SYSTEM_RANGE,
        },
    };

    for sys in &mut systems {
        sim_fleets(sys, &world.sim, standing);
        if !sys.here {
            continue;
        }
        here = HereFrame { galaxy: sys.galaxy, frame: sys.frame };
        // Your own system is the real thing: where the planets are right now,
        // the stations as they stand, the ships actually on screen.
        for planet in &mut sys.planets {
            if let Some((t, _)) = world.planets.iter().find(|(_, body)| body.name == planet.name) {
                planet.world = t.translation.truncate();
                planet.galaxy = sys.frame.to_galaxy(planet.world);
            }
        }
        sys.stations = world.stations.sites.iter()
            .map(|s| SceneStation {
                name: s.name.clone(),
                kind: crate::world::station_types::station_type_name(s.kind),
                color: crate::world::home_base::station_accent(s.kind),
                world: s.pos,
                galaxy: sys.frame.to_galaxy(s.pos),
            })
            .collect();
        // Live ships by fleet, each counted where it actually is.
        let mut live: Vec<(Option<u32>, AiShipType, bool, Vec2, usize)> = Vec::new();
        for (t, faction, civilian, member) in world.ai_ships.iter() {
            let pos = t.translation.truncate();
            let fleet = member.map(|m| m.fleet);
            match live.iter_mut().find(|g| fleet.is_some() && g.0 == fleet) {
                Some(group) => {
                    group.3 += pos;
                    group.4 += 1;
                }
                None => live.push((fleet, *faction, civilian, pos, 1)),
            }
        }
        for (fleet_id, faction, civilian, sum, count) in live {
            let fleet = fleet_id.and_then(|id| world.sim.fleets.iter().find(|f| f.id == id));
            let world_pos = sum / count as f32;
            let (name, doing) = fleet.map_or_else(
                || (faction_display_name(faction).to_string(), "In sensor range".to_string()),
                fleet_words,
            );
            sys.ships.push(SceneShips {
                name,
                doing,
                faction,
                color: ship_color(standing, faction, civilian),
                count,
                heading: fleet.map_or(0.0, |f| f.heading),
                bounty: false,
                world: world_pos,
                galaxy: sys.frame.to_galaxy(world_pos),
            });
        }
        // Your hunts: the nearest group to each active bounty carries the mark.
        for (pos, _) in crate::contracts::bounty_nav::active_bounty_positions_with_id(&world.contracts, &world.sim, &world.bounty_ships) {
            if let Some(group) = sys.ships.iter_mut()
                .filter(|g| g.world.distance(pos) < 40_000.0)
                .min_by(|a, b| a.world.distance(pos).total_cmp(&b.world.distance(pos)))
            {
                group.bounty = true;
            }
        }
    }

    let target = match (jump, dash) {
        (Some(t), _) => crate::celestial::warp::target_galaxy_pos(&world.galaxy_map, t),
        (None, Some(p)) => Some(here.frame.to_galaxy(p)),
        _ => None,
    };
    Scene {
        here_name: here_id
            .and_then(|id| systems.iter().find(|s| s.id == id))
            .map_or_else(|| "Uncharted space".to_string(), |s| s.name.clone()),
        systems,
        you: here.frame.to_galaxy(you_world),
        here,
        here_id,
        you_world,
        you_heading,
        target,
        fuel: world.fuel.current_fuel,
        max_fuel: world.fuel.max_fuel,
        hull: world.hull.hull_integrity,
        hold: (world.inventory.current_weight, world.inventory.max_capacity),
        cargo: world.inventory.items.iter().map(|(item, count)| (item.name().to_string(), *count)).collect(),
        scar: crate::narrative::summit::scar_radius(world.contacts.summit.done)
            .map(|r| (crate::narrative::summit::summit_site(&world.galaxy_map), r)),
    }
}

// ---------------------------------------------------------------------------
// Picking things
// ---------------------------------------------------------------------------

/// What's selected on the map.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MapPick {
    #[default]
    Nothing,
    You,
    System(u32),
    Planet(u32, usize),
    Station(u32, usize),
    Ships(u32, usize),
    /// Open space in the system you're in, in its own coordinates.
    Point(Vec2),
    /// Open space between the stars, in galaxy coordinates.
    Blind(Vec2),
}

impl MapPick {
    fn at_galaxy(self, scene: &Scene) -> Option<Vec2> {
        match self {
            MapPick::Nothing => None,
            MapPick::You => Some(scene.you),
            MapPick::System(id) => scene.system(id).map(|s| s.galaxy),
            MapPick::Planet(id, i) => scene.system(id)?.planets.get(i).map(|p| p.galaxy),
            MapPick::Station(id, i) => scene.system(id)?.stations.get(i).map(|p| p.galaxy),
            MapPick::Ships(id, i) => scene.system(id)?.ships.get(i).map(|p| p.galaxy),
            MapPick::Point(world) => Some(scene.here.frame.to_galaxy(world)),
            MapPick::Blind(pos) => Some(pos),
        }
    }

    /// Where the warp key goes with this picked: a dash in the system you're
    /// in, a jump to any other. The latest pick is the target, as it always
    /// has been: one warp key, and it goes where you last pointed.
    fn warp_target(self, scene: &Scene) -> (Option<Vec2>, Option<GalaxyWarpTarget>) {
        let inside = |id: u32, world: Option<Vec2>| {
            if Some(id) == scene.here_id {
                (world, None)
            } else {
                (None, Some(GalaxyWarpTarget::System(id)))
            }
        };
        match self {
            MapPick::Nothing | MapPick::You => (None, None),
            MapPick::System(id) => inside(id, None),
            MapPick::Planet(id, i) => inside(id, scene.system(id).and_then(|s| s.planets.get(i)).map(|p| p.world)),
            MapPick::Station(id, i) => inside(id, scene.system(id).and_then(|s| s.stations.get(i)).map(|p| p.world)),
            MapPick::Ships(id, i) => inside(id, scene.system(id).and_then(|s| s.ships.get(i)).map(|p| p.world)),
            MapPick::Point(world) => (Some(world), None),
            MapPick::Blind(pos) => (None, Some(GalaxyWarpTarget::BlindPoint(pos))),
        }
    }
}

/// Something clickable, where it was last painted.
#[derive(Clone, Debug)]
struct Hotspot {
    px: Vec2,
    radius: f32,
    pick: MapPick,
    tip: Vec<String>,
}

fn hotspot_at(spots: &[Hotspot], px: Vec2) -> Option<&Hotspot> {
    spots.iter()
        .filter(|s| s.px.distance(px) <= s.radius)
        .min_by(|a, b| a.px.distance(px).total_cmp(&b.px.distance(px)))
}

/// A click with nothing under it: open space in whichever system is around it,
/// or between the stars. A click close to a star you know means that star.
fn pick_in_space(scene: &Scene, cam: MapCam, stage: Vec2, px: Vec2) -> MapPick {
    let pos = cam.from_px(px, stage);
    let open = openness(cam.scale) > 0.35;
    if open && scene.here.galaxy.distance(pos) <= SYSTEM_DISC {
        return MapPick::Point(scene.here.frame.to_world(pos));
    }
    let near = scene.systems.iter()
        .filter(|s| s.galaxy.distance(pos) <= SYSTEM_DISC)
        .min_by(|a, b| a.galaxy.distance(pos).total_cmp(&b.galaxy.distance(pos)));
    match near {
        Some(sys) => MapPick::System(sys.id),
        None => MapPick::Blind(pos),
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapLayers {
    pub territory: bool,
    pub reach: bool,
    pub ships: bool,
    pub orbits: bool,
}

impl Default for MapLayers {
    fn default() -> Self {
        Self { territory: true, reach: true, ships: true, orbits: true }
    }
}

#[derive(Resource, Default)]
pub struct MapState {
    cam: MapCam,
    goal: MapCam,
    stage: Vec2,
    level: Option<MapLevel>,
    layers: MapLayers,
    live: bool,
    live_clock: f32,
    pick: MapPick,
    scene: Scene,
    spots: Vec<Hotspot>,
    /// Scroll toward leaving the map, in notches, signed: in is positive.
    pushed: f32,
    last_scroll: f64,
    hold_until: f64,
    press: Option<Vec2>,
    dragging: bool,
    paint_dirty: bool,
    side_dirty: bool,
}

impl MapState {
    fn opened(layers: MapLayers, scene: Scene, stage: Vec2, level: MapLevel, pick: MapPick) -> Self {
        let cam = level_cam(level, &scene, stage);
        Self {
            cam,
            goal: cam,
            stage,
            level: Some(level),
            layers,
            pick,
            scene,
            paint_dirty: true,
            side_dirty: true,
            ..default()
        }
    }
}

#[derive(Component)]
struct MapStage;
#[derive(Component)]
struct MapContent;
#[derive(Component)]
struct MapChrome;
#[derive(Component)]
struct MapSide;
#[derive(Component)]
struct MapTip;

/// Everything you can press on the map that isn't the map.
#[derive(Component, Clone, Copy, Debug)]
enum MapButton {
    Level(MapLevel),
    Territory,
    Reach,
    Ships,
    Orbits,
    Live,
    Pick(MapPick),
    Into(u32),
    Clear,
}

fn stage_size(window: Option<&Window>, ui_scale: f32) -> Vec2 {
    let (w, h) = window.map_or((1280.0, 720.0), |w| (w.width(), w.height()));
    (Vec2::new(w, h) / ui_scale.max(0.01) - Vec2::new(SIDE_WIDTH, 0.0)).max(Vec2::splat(200.0))
}

/// The cursor on the stage, if it's on it.
fn cursor_on_stage(window: Option<&Window>, ui_scale: f32, stage: Vec2) -> Option<Vec2> {
    let px = window?.cursor_position()? / ui_scale.max(0.01);
    (px.x <= stage.x && px.y <= stage.y).then_some(px)
}

fn pick_from_targets(dash: Option<Vec2>, jump: Option<GalaxyWarpTarget>) -> MapPick {
    match (jump, dash) {
        (Some(GalaxyWarpTarget::System(id)), _) => MapPick::System(id),
        (Some(GalaxyWarpTarget::BlindPoint(pos)), _) => MapPick::Blind(pos),
        (None, Some(world)) => MapPick::Point(world),
        _ => MapPick::Nothing,
    }
}

fn text_bundle(text: impl Into<String>, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    (
        Text::new(text.into()),
        TextFont { font_size: FontSize::Px(size), ..default() },
        TextColor(color),
    )
}

// ---------------------------------------------------------------------------
// Painting the stage
// ---------------------------------------------------------------------------

fn faded(color: Color, alpha: f32) -> Color {
    color.with_alpha(color.alpha() * alpha.clamp(0.0, 1.0))
}

struct Painter<'a, 'b> {
    p: &'a mut ChildSpawnerCommands<'b>,
    stage: Vec2,
}

impl Painter<'_, '_> {
    fn on_stage(&self, px: Vec2, margin: f32) -> bool {
        px.x >= -margin && px.y >= -margin && px.x <= self.stage.x + margin && px.y <= self.stage.y + margin
    }

    fn square(&mut self, at: Vec2, size: f32, color: Color) {
        self.p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(at.x - size / 2.0),
                top: Val::Px(at.y - size / 2.0),
                width: Val::Px(size),
                height: Val::Px(size),
                ..default()
            },
            BackgroundColor(color),
        ));
    }

    fn disc(&mut self, at: Vec2, radius: f32, color: Color) {
        self.p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(at.x - radius),
                top: Val::Px(at.y - radius),
                width: Val::Px(radius * 2.0),
                height: Val::Px(radius * 2.0),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(color),
        ));
    }

    fn ring(&mut self, at: Vec2, radius: f32, width: f32, color: Color) {
        self.p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(at.x - radius),
                top: Val::Px(at.y - radius),
                width: Val::Px(radius * 2.0),
                height: Val::Px(radius * 2.0),
                border: UiRect::all(Val::Px(width)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(color),
        ));
    }

    /// A square turned on its point: a capital's mark.
    fn diamond(&mut self, at: Vec2, radius: f32, width: f32, color: Color) {
        self.p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(at.x - radius),
                top: Val::Px(at.y - radius),
                width: Val::Px(radius * 2.0),
                height: Val::Px(radius * 2.0),
                border: UiRect::all(Val::Px(width)),
                ..default()
            },
            BorderColor::all(color),
            UiTransform::from_rotation(Rot2::radians(std::f32::consts::FRAC_PI_4)),
        ));
    }

    fn glow(&mut self, at: Vec2, radius: f32, color: Color, alpha: f32) {
        self.p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(at.x - radius),
                top: Val::Px(at.y - radius),
                width: Val::Px(radius * 2.0),
                height: Val::Px(radius * 2.0),
                ..default()
            },
            BackgroundGradient::from(RadialGradient::new(
                UiPosition::CENTER,
                RadialGradientShape::ClosestSide,
                vec![
                    ColorStop::new(color.with_alpha(alpha), Val::Percent(0.0)),
                    ColorStop::new(color.with_alpha(alpha * 0.55), Val::Percent(65.0)),
                    ColorStop::new(color.with_alpha(0.0), Val::Percent(100.0)),
                ],
            )),
        ));
    }

    fn line(&mut self, a: Vec2, b: Vec2, width: f32, color: Color) {
        let Some((a, b)) = clip_to_stage(a, b, self.stage) else { return };
        let delta = b - a;
        let length = delta.length();
        if length < 0.5 {
            return;
        }
        let mid = (a + b) / 2.0;
        self.p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(mid.x - length / 2.0),
                top: Val::Px(mid.y - width / 2.0),
                width: Val::Px(length),
                height: Val::Px(width),
                ..default()
            },
            BackgroundColor(color),
            UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x))),
        ));
    }

    /// An arrowhead: which way a ship is going. `angle` is on screen.
    fn chevron(&mut self, at: Vec2, angle: f32, size: f32, color: Color) {
        let dir = Vec2::from_angle(angle);
        let nose = at + dir * size;
        for side in [2.5f32, -2.5] {
            self.line(nose, at + Vec2::from_angle(angle + side) * size, 1.6, color);
        }
        self.line(at + Vec2::from_angle(angle + 2.5) * size, at + Vec2::from_angle(angle - 2.5) * size, 1.2, color);
    }

    fn label_width(text: &str, size: f32) -> f32 {
        text.chars().count() as f32 * size * 0.62 + 8.0
    }

    /// A name to the right of `at`, or failing that to the left of `pip`: a
    /// system's name matters more than where it sits.
    fn name_beside(&mut self, text: &str, pip: Vec2, gap: f32, size: f32, color: Color, placed: &mut Vec<Rect>) {
        let right = pip + Vec2::new(gap, -8.0);
        if !self.label(text, right, size, color, placed) {
            let left = pip + Vec2::new(-gap - Self::label_width(text, size), -8.0);
            self.label(text, left, size, color, placed);
        }
    }

    /// False if something's already there, and nothing was drawn.
    fn label(&mut self, text: &str, at: Vec2, size: f32, color: Color, placed: &mut Vec<Rect>) -> bool {
        let width = Self::label_width(text, size);
        let bounds = Rect::from_corners(at, at + Vec2::new(width, size + 4.0));
        if placed.iter().any(|q| {
            let overlap = q.intersect(bounds);
            overlap.width() > 0.0 && overlap.height() > 0.0
        }) {
            return false;
        }
        placed.push(bounds);
        self.p.spawn((
            text_bundle(text, size, color),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(at.x),
                top: Val::Px(at.y),
                padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(faded(Color::srgba(0.008, 0.012, 0.04, 0.72), color.alpha())),
        ));
        true
    }

    /// Dashes round a circle in galaxy units; only what's on the stage.
    fn dashed_circle(&mut self, cam: MapCam, center: Vec2, radius: f32, color: Color) {
        for i in (0..REACH_SEGMENTS).step_by(2) {
            let at = |n: usize| {
                let theta = n as f32 / REACH_SEGMENTS as f32 * std::f32::consts::TAU;
                cam.to_px(center + Vec2::from_angle(theta) * radius, self.stage)
            };
            let (a, b) = (at(i), at(i + 1));
            if self.on_stage(a, 2.0) || self.on_stage(b, 2.0) {
                self.line(a, b, 1.0, color);
            }
        }
    }
}

/// A line cut down to what's on the stage (Liang-Barsky). Zoomed in, a route
/// can be a million pixels long, and a node that size does the UI no favours.
fn clip_to_stage(a: Vec2, b: Vec2, stage: Vec2) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for (p, q) in [(-d.x, a.x), (d.x, stage.x - a.x), (-d.y, a.y), (d.y, stage.y - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                lo = lo.max(r);
            } else {
                hi = hi.min(r);
            }
        }
    }
    (lo <= hi).then(|| (a + d * lo, a + d * hi))
}

/// How far a full tank's worth of `fuel` jumps, in galaxy units: the inverse of
/// the drive's own price. None if it doesn't get you out of the system, or
/// reaches everywhere.
fn jump_reach(fuel: f32) -> Option<f32> {
    if fuel <= INTERSTELLAR_BASE_FUEL {
        return None;
    }
    let t = ((fuel - INTERSTELLAR_BASE_FUEL) / INTERSTELLAR_MAX_EXTRA_FUEL).sqrt();
    (t < 1.0).then_some(t * GALAXY_RADIUS)
}

fn jump_price(from: Vec2, to: Vec2) -> (f32, f32) {
    let t = from.distance(to) / GALAXY_RADIUS;
    (interstellar_fuel_cost(t), interstellar_charge_time(t))
}

fn system_tip(scene: &Scene, sys: &SceneSystem) -> Vec<String> {
    let owner = sys.owner.map_or("Nobody's space", faction_display_name);
    let mut tip = vec![sys.name.clone(), owner.to_string()];
    if sys.hostile {
        tip.push("HOSTILE".to_string());
    }
    if sys.here {
        tip.push("You are here".to_string());
    } else {
        let (fuel, charge) = jump_price(scene.here.galaxy, sys.galaxy);
        tip.push(format!("Jump: {fuel:.0} fuel, {charge:.0} s"));
    }
    tip
}

/// Paint the map. Returns what can be clicked and where.
fn paint(p: &mut ChildSpawnerCommands, scene: &Scene, cam: MapCam, stage: Vec2, layers: MapLayers, pick: MapPick) -> Vec<Hotspot> {
    let mut spots = Vec::new();
    let mut placed: Vec<Rect> = Vec::new();
    let mut paint = Painter { p, stage };
    let open = openness(cam.scale);
    let disc_px = SYSTEM_DISC * cam.scale;

    // Territory, under everything: a soft wash of each holder's colour.
    if layers.territory && open < 0.9 {
        for sys in scene.systems.iter().filter(|s| s.dark < 1.0) {
            let Some(owner) = sys.owner else { continue };
            let at = cam.to_px(sys.galaxy, stage);
            let radius = (560_000.0 * cam.scale).max(30.0);
            if !paint.on_stage(at, radius) {
                continue;
            }
            let alpha = (if sys.visited { 0.11 } else { 0.07 }) * (1.0 - open);
            paint.glow(at, radius, faction_map_color(owner), alpha);
        }
    }

    // The tear where the summit was: a cluster of dark lobes, no outline,
    // light simply gone (the prologue's Cascade look).
    if let Some((site, radius)) = scene.scar {
        const LOBES: [(f32, f32, f32); 7] = [
            (0.0, 0.0, 1.0), (0.6, 0.45, 0.7), (2.1, 0.55, 0.6), (3.3, 0.4, 0.75),
            (4.4, 0.6, 0.5), (5.4, 0.35, 0.65), (1.3, 0.8, 0.35),
        ];
        for (angle, out, size) in LOBES {
            let at = cam.to_px(site + Vec2::from_angle(angle) * radius * out, stage);
            let r = (radius * size * cam.scale).max(6.0);
            if paint.on_stage(at, r) {
                paint.glow(at, r * 1.25, Color::srgb(0.35, 0.18, 0.45), 0.10);
                paint.disc(at, r, Color::srgba(0.01, 0.0, 0.02, 0.55));
            }
        }
    }

    // The Cascade: taken systems are a hole in the light, systems it's
    // reaching are dimming. No outline, as in the prologue.
    for sys in scene.systems.iter().filter(|s| s.dark > 0.0) {
        let at = cam.to_px(sys.galaxy, stage);
        let r = (420_000.0 * cam.scale).max(10.0) * (0.6 + 0.4 * sys.dark.min(1.0));
        if paint.on_stage(at, r) {
            paint.glow(at, r * 1.3, Color::srgb(0.30, 0.15, 0.40), 0.08 * sys.dark.min(1.0));
            paint.disc(at, r, Color::srgba(0.0, 0.0, 0.01, 0.75 * sys.dark.min(1.0)));
        }
    }

    // How far the tank gets you, there and back.
    if layers.reach && open < 0.6 {
        let fade = 1.0 - open;
        for (fuel, color, words) in [
            (scene.fuel, REACH_ONE_WAY, "One way, on this tank"),
            (scene.fuel / 2.0, REACH_RETURN, "There and back"),
        ] {
            let Some(radius) = jump_reach(fuel) else { continue };
            paint.dashed_circle(cam, scene.here.galaxy, radius, faded(color, 0.5 * fade));
            let top = cam.to_px(scene.here.galaxy + Vec2::Y * radius, stage);
            if paint.on_stage(top, -30.0) {
                paint.label(words, top + Vec2::new(-(words.len() as f32) * 3.2, -8.0), F::TINY, faded(color, 0.9 * fade), &mut placed);
            }
        }
    }

    // The route the warp key will fly.
    if let Some(target) = scene.target {
        paint.line(cam.to_px(scene.you, stage), cam.to_px(target, stage), 1.4, faded(MAP_TARGET, 0.8));
    }

    // Who you can't afford to miss gets a name first when they crowd together.
    let mut order: Vec<&SceneSystem> = scene.systems.iter().collect();
    order.sort_by_key(|s| (!s.here, !s.hostile));
    // Keep the room for the map's own bars: names don't go under them.
    placed.push(Rect::from_corners(Vec2::ZERO, Vec2::new(stage.x, 40.0)));
    placed.push(Rect::from_corners(Vec2::new(0.0, stage.y - 40.0), stage));

    for sys in order {
        let at = cam.to_px(sys.galaxy, stage);
        if !paint.on_stage(at, disc_px + 40.0) {
            continue;
        }
        let pip_alpha = 1.0 - open * if sys.visited { 0.92 } else { 0.4 };
        let size = if sys.visited { 5.5 } else { 4.2 } + if sys.capital { 1.0 } else { 0.0 };

        // Whether it shoots at you is a red glow. Who holds it is the pip.
        if sys.hostile && open < 0.95 {
            paint.glow(at, size + 13.0, MAP_HOSTILE, 0.75 * (1.0 - open));
        }
        if sys.capital {
            paint.diamond(at, size + 2.0, 1.5, faded(sys.color, pip_alpha * 0.9));
        }
        if sys.haunted {
            paint.ring(at, size + 9.0, 1.5, faded(Color::srgb(0.95, 0.2, 0.2), pip_alpha));
        }
        if sys.contested {
            paint.ring(at, size + 5.5, 1.0, faded(Color::srgb(0.9, 0.75, 0.25), pip_alpha * 0.9));
        }
        if sys.visited {
            paint.disc(at, size, faded(sys.color, pip_alpha));
        } else {
            paint.ring(at, size, 1.4, faded(sys.color, pip_alpha * 0.85));
        }
        if pip_alpha > 0.15 {
            spots.push(Hotspot { px: at, radius: 13.0, pick: MapPick::System(sys.id), tip: system_tip(scene, sys) });
        }
        if open < 0.65 {
            let color = if sys.hostile {
                HOSTILE_TEXT
            } else if sys.visited {
                C::TEXT_PRIMARY
            } else {
                C::TEXT_SECONDARY
            };
            let font = if sys.here { F::BODY_SMALL } else { F::CAPTION };
            paint.name_beside(&sys.name, at, size + 7.0, font, faded(color, 1.0 - open), &mut placed);
        }

        // Inside it.
        if open <= 0.02 {
            continue;
        }
        if layers.orbits {
            let color = faded(MAP_ORBIT, open);
            let center = sys.frame.galaxy;
            for &(a, e) in &sys.orbits {
                let on_orbit = |n: usize| {
                    let theta = n as f32 / ORBIT_SEGMENTS as f32 * std::f32::consts::TAU;
                    let r = crate::celestial::orbits::orbit_radius(a, e, theta) * sys.frame.squeeze;
                    cam.to_px(center + Vec2::from_angle(theta) * r, stage)
                };
                for i in 0..ORBIT_SEGMENTS {
                    let (from, to) = (on_orbit(i), on_orbit(i + 1));
                    if paint.on_stage(from, 2.0) || paint.on_stage(to, 2.0) {
                        paint.line(from, to, 1.0, color);
                    }
                }
            }
        }
        // The star.
        paint.glow(at, 26.0, Color::srgb(1.0, 0.82, 0.3), 0.55 * open);
        paint.disc(at, 6.0, faded(Color::srgb(1.0, 0.95, 0.77), open));
        for (i, station) in sys.stations.iter().enumerate() {
            let px = cam.to_px(station.galaxy, stage);
            if !paint.on_stage(px, 60.0) {
                continue;
            }
            paint.square(px, 11.0, faded(Color::srgba(0.9, 1.0, 0.95, 0.85), open));
            paint.square(px, 8.0, faded(station.color, open));
            paint.label(&station.name, px + Vec2::new(9.0, -8.0), F::CAPTION, faded(Color::srgb(0.75, 0.95, 0.8), open), &mut placed);
            spots.push(Hotspot {
                px,
                radius: 11.0,
                pick: MapPick::Station(sys.id, i),
                tip: vec![station.name.clone(), station.kind.to_string()],
            });
        }
        for (i, planet) in sys.planets.iter().enumerate() {
            let px = cam.to_px(planet.galaxy, stage);
            if !paint.on_stage(px, 60.0) {
                continue;
            }
            paint.disc(px, 3.8, faded(planet.color, open));
            paint.label(&planet.name, px + Vec2::new(7.0, -7.0), F::TINY, faded(C::TEXT_SECONDARY, open), &mut placed);
            spots.push(Hotspot {
                px,
                radius: 10.0,
                pick: MapPick::Planet(sys.id, i),
                tip: vec![planet.name.clone(), format!("{} planet", planet.kind)],
            });
        }
        if layers.ships {
            for (i, ships) in sys.ships.iter().enumerate() {
                let px = cam.to_px(ships.galaxy, stage);
                if !paint.on_stage(px, 20.0) {
                    continue;
                }
                paint.chevron(px, -ships.heading, 5.5, faded(ships.color, open));
                if ships.bounty {
                    paint.ring(px, 10.0, 1.5, faded(MAP_BOUNTY, open));
                }
                if ships.count > 1 {
                    paint.label(&format!("x{}", ships.count), px + Vec2::new(8.0, -13.0), F::TINY, faded(ships.color, open), &mut placed);
                }
                let mut tip = vec![ships.name.clone(), ships.doing.clone()];
                tip.push(format!("{} ship{}", ships.count, if ships.count == 1 { "" } else { "s" }));
                spots.push(Hotspot { px, radius: 10.0, pick: MapPick::Ships(sys.id, i), tip });
            }
        }
        // Over its star, if that's still free: the crumb names it anyway.
        if open > 0.6 {
            let width = sys.name.len() as f32 * 3.4 + 4.0;
            let words = sys.name.to_uppercase();
            paint.label(&words, at + Vec2::new(-width, -34.0), F::CAPTION, faded(Color::srgb(1.0, 0.91, 0.66), open), &mut placed);
        }
    }

    // You, on top.
    let you = cam.to_px(scene.you, stage);
    paint.ring(you, 11.0, 1.2, faded(MAP_YOU, 0.6));
    paint.chevron(you, -scene.you_heading, 6.5, MAP_YOU);
    spots.push(Hotspot { px: you, radius: 10.0, pick: MapPick::You, tip: vec!["Your ship".to_string(), scene.here_name.clone()] });

    // The selection: gold corners round whatever's picked.
    if let Some(pos) = pick.at_galaxy(scene) {
        let px = cam.to_px(pos, stage);
        let (near, far) = (6.0, 11.0);
        for (u, v) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let corner = px + Vec2::new(u, v) * far;
            paint.line(corner, px + Vec2::new(u * near, v * far), 1.4, MAP_TARGET);
            paint.line(corner, px + Vec2::new(u * far, v * near), 1.4, MAP_TARGET);
        }
    }
    spots
}

// ---------------------------------------------------------------------------
// The bars over the stage
// ---------------------------------------------------------------------------

fn chip(p: &mut ChildSpawnerCommands, words: &str, on: Option<bool>, button: MapButton) {
    let lit = on.unwrap_or(false);
    p.spawn((
        Button,
        button,
        Node {
            padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(if lit { C::BG_ELEVATED } else { C::BG_CARD }),
        BorderColor::all(if lit { C::BORDER_BRIGHT } else { C::BORDER_DEFAULT }),
    )).with_children(|c| {
        c.spawn(text_bundle(words, F::CAPTION, if on == Some(false) { C::TEXT_MUTED } else { C::TEXT_PRIMARY }));
    });
}

/// The fuel to spend on the target, if there is one: the white end of the gauge.
fn target_fuel_cost(scene: &Scene, pick: MapPick) -> f32 {
    match pick.warp_target(scene) {
        (_, Some(GalaxyWarpTarget::System(id))) => {
            scene.system(id).map_or(0.0, |s| jump_price(scene.here.galaxy, s.galaxy).0)
        }
        (_, Some(GalaxyWarpTarget::BlindPoint(pos))) => jump_price(scene.here.galaxy, pos).0,
        (Some(world), None) => {
            let dist = scene.you_world.distance(world);
            warp_dash_fuel_cost((dist - warp_dash_arrival_buffer(dist)).max(0.0))
        }
        _ => 0.0,
    }
}

fn crumb_words(scene: &Scene, cam: MapCam, stage: Vec2) -> String {
    if openness(cam.scale) > 0.5 {
        let nearest = scene.systems.iter()
            .min_by(|a, b| a.galaxy.distance(cam.center).total_cmp(&b.galaxy.distance(cam.center)));
        if let Some(sys) = nearest {
            return format!("Galaxy > {}", sys.name);
        }
    }
    if cam.scale < whole_galaxy_scale(stage) * 1.5 { "Galaxy".to_string() } else { "Known space".to_string() }
}

#[derive(Component)]
struct MapCrumb;

fn paint_chrome(p: &mut ChildSpawnerCommands, state: &MapState) {
    let scene = &state.scene;
    p.spawn(Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        right: Val::Px(0.0),
        top: Val::Px(0.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(14.0),
        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
        ..default()
    }).with_children(|bar| {
        bar.spawn((text_bundle(crumb_words(scene, state.cam, state.stage), F::BODY, C::TEXT_TITLE), MapCrumb));
        bar.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(2.0), ..default() }).with_children(|row| {
            chip(row, "Galaxy", None, MapButton::Level(MapLevel::Galaxy));
            chip(row, "Known space", None, MapButton::Level(MapLevel::Known));
            chip(row, "This system", None, MapButton::Level(MapLevel::Here));
        });
        // The gauge, with what the target would burn marked on its end.
        let cost = target_fuel_cost(scene, state.pick);
        let max = scene.max_fuel.max(1.0);
        let filled = (scene.fuel / max).clamp(0.0, 1.0);
        let spend = (cost / max).clamp(0.0, filled);
        bar.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            margin: UiRect::left(Val::Auto),
            ..default()
        }).with_children(|fuel| {
            fuel.spawn(text_bundle("Fuel", F::CAPTION, C::TEXT_SECONDARY));
            fuel.spawn((Node { width: Val::Px(120.0), height: Val::Px(6.0), ..default() }, BackgroundColor(C::BG_INPUT))).with_children(|gauge| {
                gauge.spawn((
                    Node { width: Val::Percent((filled - spend) * 100.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(C::ACCENT_ORANGE),
                ));
                gauge.spawn((
                    Node { width: Val::Percent(spend * 100.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
                ));
            });
            let words = if cost > 0.0 {
                format!("{:.0} after", (scene.fuel - cost).max(0.0))
            } else {
                format!("{:.0} / {:.0}", scene.fuel, scene.max_fuel)
            };
            let color = if cost > scene.fuel { C::ACCENT_RED } else { C::TEXT_SECONDARY };
            fuel.spawn(text_bundle(words, F::CAPTION, color));
        });
    });
    p.spawn(Node {
        position_type: PositionType::Absolute,
        left: Val::Px(12.0),
        bottom: Val::Px(10.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(4.0),
        ..default()
    }).with_children(|row| {
        let layers = state.layers;
        chip(row, "Territory", Some(layers.territory), MapButton::Territory);
        chip(row, "Fuel reach", Some(layers.reach), MapButton::Reach);
        chip(row, "Ships", Some(layers.ships), MapButton::Ships);
        chip(row, "Orbits", Some(layers.orbits), MapButton::Orbits);
        chip(row, if state.live { "Live" } else { "Paused" }, Some(state.live), MapButton::Live);
    });
    p.spawn((
        text_bundle("Scroll: zoom   Drag: move   Click: target   G: go   M: close", F::TINY, C::TEXT_MUTED),
        Node { position_type: PositionType::Absolute, right: Val::Px(12.0), bottom: Val::Px(14.0), ..default() },
    ));
}

// ---------------------------------------------------------------------------
// The side panel
// ---------------------------------------------------------------------------

fn heading(p: &mut ChildSpawnerCommands, over: &str, title: &str) {
    p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).with_children(|c| {
        c.spawn(text_bundle(over.to_uppercase(), F::TINY, C::TEXT_MUTED));
        c.spawn(text_bundle(title, F::H2, C::TEXT_TITLE));
    });
}

fn section(p: &mut ChildSpawnerCommands, words: &str) {
    p.spawn((
        text_bundle(words.to_uppercase(), F::TINY, C::TEXT_MUTED),
        Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
    ));
}

fn fact(p: &mut ChildSpawnerCommands, label: &str, value: impl Into<String>, color: Color) {
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        column_gap: Val::Px(12.0),
        ..default()
    }).with_children(|row| {
        row.spawn(text_bundle(label, F::BODY_SMALL, C::TEXT_SECONDARY));
        row.spawn(text_bundle(value, F::BODY_SMALL, color));
    });
}

fn panel_button(p: &mut ChildSpawnerCommands, words: &str, button: MapButton) {
    p.spawn((
        Button,
        button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(C::BG_ELEVATED),
        BorderColor::all(C::BORDER_DEFAULT),
    )).with_children(|c| {
        c.spawn(text_bundle(words, F::BODY_SMALL, C::TEXT_PRIMARY));
    });
}

/// A row in a list: a swatch, a name, and something quiet on the right.
fn list_row(p: &mut ChildSpawnerCommands, swatch: Color, name: &str, right: &str, name_color: Color, button: MapButton) {
    p.spawn((
        Button,
        button,
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(7.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::NONE),
    )).with_children(|row| {
        row.spawn((Node { width: Val::Px(8.0), height: Val::Px(8.0), flex_shrink: 0.0, ..default() }, BackgroundColor(swatch)));
        row.spawn(text_bundle(name, F::BODY_SMALL, name_color));
        row.spawn((text_bundle(right, F::CAPTION, C::TEXT_MUTED), Node { margin: UiRect::left(Val::Auto), ..default() }));
    });
}

fn jump_block(p: &mut ChildSpawnerCommands, scene: &Scene, to: Vec2) {
    let (fuel, charge) = jump_price(scene.here.galaxy, to);
    let afford = scene.fuel >= fuel;
    section(p, "Jump");
    fact(p, "Fuel", format!("{:.0} of {:.0}", fuel, scene.fuel), if afford { C::TEXT_PRIMARY } else { C::ACCENT_RED });
    fact(p, "Charge", format!("{charge:.0} s"), C::TEXT_PRIMARY);
    fact(p, "Left after", format!("{:.0}", (scene.fuel - fuel).max(0.0)), C::TEXT_PRIMARY);
    if fuel * 2.0 <= scene.fuel {
        fact(p, "Can get back", "Yes", C::TEXT_PRIMARY);
    } else {
        fact(p, "Can get back", "No", C::ACCENT_ORANGE);
    }
    p.spawn(text_bundle(
        if afford { "Target set. Press G to jump." } else { "Not enough fuel for this jump." },
        F::BODY_SMALL,
        if afford { C::ACCENT_GREEN } else { C::ACCENT_RED },
    ));
}

fn dash_block(p: &mut ChildSpawnerCommands, scene: &Scene, to: Vec2) {
    let dist = scene.you_world.distance(to);
    // Same arithmetic the dash itself uses, so the preview can't quote a price
    // the drive won't charge.
    let travel = (dist - warp_dash_arrival_buffer(dist)).max(0.0);
    let fuel = warp_dash_fuel_cost(travel);
    let afford = scene.fuel >= fuel;
    section(p, "Warp dash");
    fact(p, "Distance", format_range_km(dist), C::TEXT_PRIMARY);
    fact(p, "Fuel", format!("{:.0} of {:.0}", fuel, scene.fuel), if afford { C::TEXT_PRIMARY } else { C::ACCENT_RED });
    fact(p, "Charge", format!("{:.0} s", warp_dash_charge_time(travel)), C::TEXT_PRIMARY);
    p.spawn(text_bundle(
        if afford { "Target set. Press G to warp." } else { "Not enough fuel for this dash." },
        F::BODY_SMALL,
        if afford { C::ACCENT_GREEN } else { C::ACCENT_RED },
    ));
}

fn go_block(p: &mut ChildSpawnerCommands, scene: &Scene, id: u32, world: Vec2) {
    match scene.system(id) {
        Some(sys) if sys.here => dash_block(p, scene, world),
        Some(sys) => jump_block(p, scene, sys.galaxy),
        None => {}
    }
}

fn paint_brief(p: &mut ChildSpawnerCommands, sys: &SceneSystem) {
    for line in &sys.brief {
        let size = if line.lead { F::BODY } else { F::BODY_SMALL };
        p.spawn(text_bundle(line.text.clone(), size, line.color));
    }
}

fn panel_home(p: &mut ChildSpawnerCommands, scene: &Scene) {
    heading(p, "You", &scene.here_name);
    let here = scene.here_id.and_then(|id| scene.system(id));
    if let Some(sys) = here.filter(|s| s.hostile) {
        paint_brief(p, sys);
    }
    fact(p, "Hull", format!("{:.0}%", scene.hull * 100.0), C::TEXT_PRIMARY);
    fact(p, "Fuel", format!("{:.0} / {:.0}", scene.fuel, scene.max_fuel), C::TEXT_PRIMARY);
    fact(p, "Hold", format!("{:.0} / {:.0}", scene.hold.0, scene.hold.1), C::TEXT_PRIMARY);
    for (name, count) in scene.cargo.iter().take(6) {
        fact(p, name, format!("{count}"), C::TEXT_SECONDARY);
    }
    if let Some(sys) = here {
        if !sys.stations.is_empty() {
            section(p, "In this system");
        }
        for (i, station) in sys.stations.iter().enumerate() {
            let dist = (scene.you_world.distance(station.world) / 1000.0).round();
            list_row(p, station.color, &station.name, &format!("{dist:.0} km"), C::TEXT_PRIMARY, MapButton::Pick(MapPick::Station(sys.id, i)));
        }
    }
    let mut known: Vec<&SceneSystem> = scene.systems.iter().filter(|s| !s.here).collect();
    known.sort_by(|a, b| a.galaxy.distance(scene.here.galaxy).total_cmp(&b.galaxy.distance(scene.here.galaxy)));
    if !known.is_empty() {
        section(p, "Nearest known systems");
    }
    for sys in known.iter().take(7) {
        let fuel = jump_price(scene.here.galaxy, sys.galaxy).0;
        let color = if sys.hostile { HOSTILE_TEXT } else { C::TEXT_PRIMARY };
        list_row(p, sys.color, &sys.name, &format!("{fuel:.0} fuel"), color, MapButton::Pick(MapPick::System(sys.id)));
    }
    p.spawn((
        text_bundle("Click empty space to plot a blind jump there.", F::CAPTION, C::TEXT_MUTED),
        Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
    ));
}

fn panel_system(p: &mut ChildSpawnerCommands, scene: &Scene, sys: &SceneSystem) {
    let over = match (sys.capital, sys.visited) {
        (true, true) => "Capital system",
        (true, false) => "Capital system - not visited",
        (false, true) => "System",
        (false, false) => "System - not visited",
    };
    heading(p, over, &sys.name);
    paint_brief(p, sys);
    section(p, "In it");
    fact(p, "Star", sys.star.clone(), C::TEXT_PRIMARY);
    fact(p, "Planets", format!("{}", sys.planets.len()), C::TEXT_PRIMARY);
    fact(p, "Stations", format!("{}", sys.stations.len()), C::TEXT_PRIMARY);
    let ships: usize = sys.ships.iter().map(|s| s.count).sum();
    if ships == 0 {
        fact(p, "Ships", "No reports", C::TEXT_MUTED);
    } else {
        fact(p, "Ships", format!("{ships}"), C::TEXT_PRIMARY);
    }
    if sys.here {
        p.spawn(text_bundle("You are here.", F::BODY_SMALL, MAP_YOU));
    } else {
        jump_block(p, scene, sys.galaxy);
    }
    panel_button(p, &format!("Zoom into {}", sys.name), MapButton::Into(sys.id));
}

fn paint_side(p: &mut ChildSpawnerCommands, state: &MapState) {
    let scene = &state.scene;
    match state.pick {
        MapPick::Nothing => panel_home(p, scene),
        MapPick::You => {
            heading(p, "Your ship", &scene.here_name);
            fact(p, "Hull", format!("{:.0}%", scene.hull * 100.0), C::TEXT_PRIMARY);
            fact(p, "Fuel", format!("{:.0} / {:.0}", scene.fuel, scene.max_fuel), C::TEXT_PRIMARY);
            fact(p, "Hold", format!("{:.0} / {:.0}", scene.hold.0, scene.hold.1), C::TEXT_PRIMARY);
        }
        MapPick::System(id) => {
            if let Some(sys) = scene.system(id) {
                panel_system(p, scene, sys);
            }
        }
        MapPick::Planet(id, i) => {
            if let Some((sys, planet)) = scene.system(id).and_then(|s| Some((s, s.planets.get(i)?))) {
                heading(p, "Planet", &planet.name);
                fact(p, "Type", planet.kind.clone(), C::TEXT_PRIMARY);
                fact(p, "Orbits", sys.name.clone(), C::TEXT_PRIMARY);
                go_block(p, scene, id, planet.world);
            }
        }
        MapPick::Station(id, i) => {
            if let Some((sys, station)) = scene.system(id).and_then(|s| Some((s, s.stations.get(i)?))) {
                heading(p, &format!("Station - {}", station.kind), &station.name);
                match sys.owner {
                    Some(owner) => fact(p, "Owner", faction_display_name(owner), faction_map_color(owner)),
                    None => fact(p, "Owner", "Free port", C::TEXT_PRIMARY),
                }
                if sys.hostile {
                    fact(p, "Docking", "Refused", C::ACCENT_RED);
                } else {
                    fact(p, "Docking", "Allowed", C::ACCENT_GREEN);
                }
                go_block(p, scene, id, station.world);
            }
        }
        MapPick::Ships(id, i) => {
            if let Some((sys, ships)) = scene.system(id).and_then(|s| Some((s, s.ships.get(i)?))) {
                heading(p, if ships.bounty { "Fleet - your bounty" } else { "Fleet" }, &ships.name);
                fact(p, "Faction", faction_display_name(ships.faction), faction_map_color(ships.faction));
                fact(p, "Ships", format!("{}", ships.count), C::TEXT_PRIMARY);
                fact(p, "In", sys.name.clone(), C::TEXT_PRIMARY);
                p.spawn(text_bundle(ships.doing.clone(), F::BODY_SMALL, C::TEXT_SECONDARY));
                go_block(p, scene, id, ships.world);
            }
        }
        MapPick::Point(world) => {
            heading(p, &format!("Point in {}", scene.here_name), "Open space");
            dash_block(p, scene, world);
        }
        MapPick::Blind(pos) => {
            heading(p, "Blind jump", "Uncharted space");
            p.spawn(text_bundle(
                "Nothing is confirmed out here. You may find a system when you land, or nothing.",
                F::BODY_SMALL,
                C::TEXT_SECONDARY,
            ));
            jump_block(p, scene, pos);
        }
    }
    if state.pick != MapPick::Nothing {
        panel_button(p, "Clear", MapButton::Clear);
    }
}

// ---------------------------------------------------------------------------
// Opening and closing
// ---------------------------------------------------------------------------

/// A few fixed stars, so the void reads as space and not as a blank panel.
fn paint_sky(p: &mut ChildSpawnerCommands, stage: Vec2) {
    let mut seed = 0x5EED_u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    for _ in 0..110 {
        let (x, y, size, alpha) = (next(), next(), next(), next());
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x * stage.x),
                top: Val::Px(y * stage.y),
                width: Val::Px(1.0 + size.round()),
                height: Val::Px(1.0 + size.round()),
                ..default()
            },
            BackgroundColor(Color::srgba(0.78, 0.83, 0.93, 0.08 + alpha * 0.32)),
        ));
    }
}

fn spawn_map(commands: &mut Commands, stage: Vec2) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        },
        BackgroundColor(MAP_GROUND),
        ZIndex(50),
        MapOverlay,
    )).with_children(|root| {
        root.spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            Interaction::None,
            MapStage,
        )).with_children(|s| {
            paint_sky(s, stage);
            s.spawn((
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                MapContent,
            ));
            s.spawn((
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                MapChrome,
            ));
            s.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(C::BG_PANEL),
                BorderColor::all(C::BORDER_BRIGHT),
                ZIndex(5),
                MapTip,
            ));
        });
        root.spawn((
            Node {
                width: Val::Px(SIDE_WIDTH),
                flex_shrink: 0.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::left(Val::Px(1.0)),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(C::BG_PANEL),
            BorderColor::all(C::BORDER_DEFAULT),
            MapSide,
        ));
    });
}

/// M opens the map, framed on what you know, and closes it.
#[allow(clippy::too_many_arguments)]
fn toggle_map(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    existing: Query<Entity, With<MapOverlay>>,
    world: MapWorld,
    targets: (Res<PendingWarpTarget>, Res<PendingGalaxyWarpTarget>),
    windows: Query<&Window>,
    ui_scale: Res<bevy::ui::UiScale>,
    mut state: ResMut<MapState>,
    mut virtual_time: ResMut<Time<Virtual>>,
    build_state: Res<State<BuildState>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyM) {
        return;
    }
    if let Ok(entity) = existing.single() {
        commands.entity(entity).despawn();
        virtual_time.unpause();
        return;
    }
    // In the yard M cycles hull material. It also opened the full-screen map
    // over the builder and paused time, every press.
    if *build_state.get() != BuildState::Inactive {
        return;
    }
    open_map(&mut commands, &world, targets.0.0, targets.1.0, windows.single().ok(), ui_scale.0, &mut state, MapLevel::Known);
    // The full-screen map pauses the simulation: ships, timers, damage and
    // fuel burn all read Time<Virtual>. Live (a chip on the map) lets it run.
    virtual_time.pause();
}

#[allow(clippy::too_many_arguments)]
fn open_map(
    commands: &mut Commands,
    world: &MapWorld,
    dash: Option<Vec2>,
    jump: Option<GalaxyWarpTarget>,
    window: Option<&Window>,
    ui_scale: f32,
    state: &mut MapState,
    level: MapLevel,
) {
    let stage = stage_size(window, ui_scale);
    let scene = gather_scene(world, dash, jump);
    *state = MapState::opened(state.layers, scene, stage, level, pick_from_targets(dash, jump));
    spawn_map(commands, stage);
}

/// With the map open and somewhere picked, the warp key is "go": close the map
/// and let the drive charge. Time stands still behind the map, so otherwise the
/// charge would sit there until you closed it yourself.
fn warp_key_closes_map(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    existing: Query<Entity, With<MapOverlay>>,
    targets: (Res<PendingWarpTarget>, Res<PendingGalaxyWarpTarget>),
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    if !warp_key_pressed(&keyboard) || (targets.0.0.is_none() && targets.1.0.is_none()) {
        return;
    }
    if let Ok(entity) = existing.single() {
        commands.entity(entity).despawn();
        virtual_time.unpause();
    }
}

// ---------------------------------------------------------------------------
// Using it
// ---------------------------------------------------------------------------

fn set_pick(state: &mut MapState, pick: MapPick, dash: &mut PendingWarpTarget, jump: &mut PendingGalaxyWarpTarget) {
    let (to_dash, to_jump) = pick.warp_target(&state.scene);
    dash.0 = to_dash;
    jump.0 = to_jump;
    state.scene.target = to_jump
        .and_then(|t| match t {
            GalaxyWarpTarget::System(id) => state.scene.system(id).map(|s| s.galaxy),
            GalaxyWarpTarget::BlindPoint(pos) => Some(pos),
        })
        .or_else(|| to_dash.map(|p| state.scene.here.frame.to_galaxy(p)));
    state.pick = pick;
    state.paint_dirty = true;
    state.side_dirty = true;
}

/// The chips and the panel's buttons.
fn map_buttons(
    mut buttons: Query<(&Interaction, &MapButton, &mut BackgroundColor), Changed<Interaction>>,
    mut state: ResMut<MapState>,
    mut dash: ResMut<PendingWarpTarget>,
    mut jump: ResMut<PendingGalaxyWarpTarget>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    for (interaction, button, mut background) in &mut buttons {
        let is_list_row = matches!(button, MapButton::Pick(_));
        background.0 = match interaction {
            Interaction::Pressed => C::BG_PRESSED,
            Interaction::Hovered => C::BG_HOVER,
            Interaction::None if is_list_row => Color::NONE,
            Interaction::None => C::BG_ELEVATED,
        };
        if *interaction != Interaction::Pressed {
            continue;
        }
        let stage = state.stage;
        match *button {
            MapButton::Level(level) => {
                state.goal = level_cam(level, &state.scene, stage);
                state.level = Some(level);
            }
            MapButton::Territory => state.layers.territory = !state.layers.territory,
            MapButton::Reach => state.layers.reach = !state.layers.reach,
            MapButton::Ships => state.layers.ships = !state.layers.ships,
            MapButton::Orbits => state.layers.orbits = !state.layers.orbits,
            MapButton::Live => {
                state.live = !state.live;
                if state.live {
                    virtual_time.unpause();
                } else {
                    virtual_time.pause();
                }
            }
            MapButton::Pick(pick) => set_pick(&mut state, pick, &mut dash, &mut jump),
            MapButton::Into(id) => {
                if let Some(sys) = state.scene.system(id) {
                    state.goal = MapCam { center: sys.galaxy, scale: system_fill_scale(stage) }.framed(stage);
                }
            }
            MapButton::Clear => set_pick(&mut state, MapPick::Nothing, &mut dash, &mut jump),
        }
        state.paint_dirty = true;
        state.side_dirty = true;
    }
}

/// Tab steps between the three views, as it used to flip between two maps.
fn map_tab(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<MapState>) {
    if !keyboard.just_pressed(KeyCode::Tab) {
        return;
    }
    let next = match state.level {
        Some(MapLevel::Here) => MapLevel::Galaxy,
        Some(MapLevel::Galaxy) => MapLevel::Known,
        _ => MapLevel::Here,
    };
    state.goal = level_cam(next, &state.scene, state.stage);
    state.level = Some(next);
}

/// Click picks, drag moves.
fn map_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    stage_node: Query<&Interaction, With<MapStage>>,
    windows: Query<&Window>,
    ui_scale: Res<bevy::ui::UiScale>,
    mut state: ResMut<MapState>,
    mut dash: ResMut<PendingWarpTarget>,
    mut jump: ResMut<PendingGalaxyWarpTarget>,
) {
    let window = windows.single().ok();
    let cursor = cursor_on_stage(window, ui_scale.0, state.stage);
    if mouse.just_pressed(MouseButton::Left) && stage_node.single().is_ok_and(|i| *i == Interaction::Pressed) {
        state.press = cursor;
        state.dragging = false;
    }
    let Some(from) = state.press else { return };
    if mouse.pressed(MouseButton::Left) {
        let Some(px) = cursor else { return };
        if state.dragging || px.distance(from) > DRAG_PX {
            state.dragging = true;
            let delta = px - from;
            let scale = state.cam.scale;
            let stage = state.stage;
            let moved = MapCam { center: state.cam.center + Vec2::new(-delta.x, delta.y) / scale, scale };
            state.goal = moved.framed(stage);
            state.cam = state.goal;
            state.press = Some(px);
            state.level = None;
            state.paint_dirty = true;
        }
        return;
    }
    state.press = None;
    if state.dragging {
        state.dragging = false;
        return;
    }
    let (cam, stage) = (state.cam, state.stage);
    let pick = hotspot_at(&state.spots, from).map(|s| s.pick).unwrap_or_else(|| pick_in_space(&state.scene, cam, stage, from));
    set_pick(&mut state, pick, &mut dash, &mut jump);
}

/// The scroll wheel, or plus and minus: in and out, all the way from your ship
/// to the whole galaxy.
#[allow(clippy::too_many_arguments)]
fn map_scroll(
    mut commands: Commands,
    mut scroll: MessageReader<MouseWheel>,
    keyboard: Res<ButtonInput<KeyCode>>,
    real: Res<Time<Real>>,
    existing: Query<Entity, With<MapOverlay>>,
    world: MapWorld,
    targets: (Res<PendingWarpTarget>, Res<PendingGalaxyWarpTarget>),
    windows: Query<&Window>,
    ui_scale: Res<bevy::ui::UiScale>,
    flight: (Res<crate::camera::CameraState>, Res<State<BuildState>>, Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<crate::ui::windows::framework::ScrollsOnWheel>>),
    mut state: ResMut<MapState>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    let (camera, build_state, scroll_lists) = flight;
    let mut notches: f32 = scroll.read().map(|e| match e.unit {
        MouseScrollUnit::Line => e.y,
        MouseScrollUnit::Pixel => e.y / PIXELS_PER_NOTCH,
    }).sum();
    let open = existing.single().ok();
    if open.is_some() {
        let dt = real.delta_secs();
        if keyboard.any_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
            notches += dt * 7.0;
        }
        if keyboard.any_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
            notches -= dt * 7.0;
        }
    }
    let now = real.elapsed_secs_f64();
    // Let go of the wheel, or turn back, and the push toward leaving is off.
    if now - state.last_scroll > 0.5 || notches * state.pushed < 0.0 {
        state.pushed = 0.0;
    }
    if notches == 0.0 {
        return;
    }
    state.last_scroll = now;
    if now < state.hold_until {
        return;
    }
    let window = windows.single().ok();

    // In flight: past the camera's limit, the map of the system you're in.
    let Some(overlay) = open else {
        let cursor = window.and_then(|w| w.cursor_position().map(|p| p * w.scale_factor()));
        let over_list = crate::ui::windows::framework::cursor_over_scroll_list(&scroll_lists, cursor);
        if notches > 0.0 || over_list || camera.zoom < camera.max_zoom || *build_state.get() != BuildState::Inactive {
            state.pushed = 0.0;
            return;
        }
        state.pushed += notches;
        if state.pushed > -NOTCHES_FROM_FLIGHT {
            return;
        }
        open_map(&mut commands, &world, targets.0.0, targets.1.0, window, ui_scale.0, &mut state, MapLevel::Here);
        state.hold_until = now + OPENING_HOLD;
        state.last_scroll = now;
        virtual_time.pause();
        return;
    };

    let stage = state.stage;
    let cursor = cursor_on_stage(window, ui_scale.0, stage).unwrap_or(stage / 2.0);
    // All the way in on your own system: the rest of the scroll is back to
    // your ship.
    if notches > 0.0 && state.goal.at_max_zoom(stage) {
        let on_here = state.goal.center.distance(state.scene.here.galaxy) <= SYSTEM_DISC;
        state.pushed += notches;
        if on_here && state.pushed >= NOTCHES_TO_FLIGHT {
            commands.entity(overlay).despawn();
            virtual_time.unpause();
        }
        return;
    }
    state.goal = state.goal.scrolled(notches, cursor, stage);
    state.level = None;
}

/// The camera catches up, and the crumb says where it's got to.
fn map_camera(real: Res<Time<Real>>, mut state: ResMut<MapState>, mut crumb: Query<&mut Text, With<MapCrumb>>) {
    let (cam, goal) = (state.cam, state.goal);
    if cam == goal {
        return;
    }
    let k = 1.0 - (-real.delta_secs() * CAMERA_EASE).exp();
    // Evenly in zoom, not in scale, or it would rush the far end.
    let scale = (cam.scale.ln() + (goal.scale.ln() - cam.scale.ln()) * k).exp();
    let center = cam.center.lerp(goal.center, k);
    let near = (scale / goal.scale - 1.0).abs() < 0.002 && center.distance(goal.center) * goal.scale < 0.5;
    state.cam = if near { goal } else { MapCam { center, scale } };
    state.paint_dirty = true;
    if let Ok(mut text) = crumb.single_mut() {
        let words = crumb_words(&state.scene, state.cam, state.stage);
        if text.0 != words {
            text.0 = words;
        }
    }
}

/// On Live the world keeps moving, so the picture has to as well.
fn map_live(
    real: Res<Time<Real>>,
    world: MapWorld,
    targets: (Res<PendingWarpTarget>, Res<PendingGalaxyWarpTarget>),
    mut state: ResMut<MapState>,
) {
    if !state.live {
        return;
    }
    state.live_clock += real.delta_secs();
    if state.live_clock < LIVE_REFRESH {
        return;
    }
    state.live_clock = 0.0;
    state.scene = gather_scene(&world, targets.0.0, targets.1.0);
    state.paint_dirty = true;
}

/// The window changed size: the stage did too.
fn map_resized(windows: Query<&Window>, ui_scale: Res<bevy::ui::UiScale>, mut state: ResMut<MapState>) {
    let stage = stage_size(windows.single().ok(), ui_scale.0);
    if stage != state.stage {
        state.stage = stage;
        state.goal = state.goal.framed(stage);
        state.cam = state.cam.framed(stage);
        state.paint_dirty = true;
    }
}

fn map_repaint(
    mut commands: Commands,
    mut state: ResMut<MapState>,
    content: Query<Entity, With<MapContent>>,
    chrome: Query<Entity, With<MapChrome>>,
    side: Query<Entity, With<MapSide>>,
) {
    if state.paint_dirty {
        let (Ok(content), Ok(chrome)) = (content.single(), chrome.single()) else { return };
        let mut spots = Vec::new();
        commands.entity(content).despawn_related::<Children>().with_children(|p| {
            spots = paint(p, &state.scene, state.cam, state.stage, state.layers, state.pick);
        });
        state.spots = spots;
        if state.side_dirty {
            commands.entity(chrome).despawn_related::<Children>().with_children(|p| paint_chrome(p, &state));
        }
        state.paint_dirty = false;
    }
    if state.side_dirty {
        let Ok(side) = side.single() else { return };
        commands.entity(side).despawn_related::<Children>().with_children(|p| paint_side(p, &state));
        state.side_dirty = false;
    }
}

/// Hover names whatever's under the cursor.
fn map_tip(
    mut commands: Commands,
    windows: Query<&Window>,
    ui_scale: Res<bevy::ui::UiScale>,
    state: Res<MapState>,
    mut tip: Query<(Entity, &mut Node), With<MapTip>>,
    mut shown: Local<Option<Vec<String>>>,
) {
    let Ok((entity, mut node)) = tip.single_mut() else { return };
    let cursor = cursor_on_stage(windows.single().ok(), ui_scale.0, state.stage);
    let spot = cursor.and_then(|px| hotspot_at(&state.spots, px));
    let Some(spot) = spot else {
        if node.display != Display::None {
            node.display = Display::None;
        }
        *shown = None;
        return;
    };
    node.display = Display::Flex;
    node.left = Val::Px(spot.px.x + 16.0);
    node.top = Val::Px(spot.px.y + 12.0);
    if shown.as_ref() != Some(&spot.tip) {
        *shown = Some(spot.tip.clone());
        commands.entity(entity).despawn_related::<Children>().with_children(|p| {
            for (i, line) in spot.tip.iter().enumerate() {
                let color = if line == "HOSTILE" {
                    HOSTILE_TEXT
                } else if i == 0 {
                    C::TEXT_PRIMARY
                } else {
                    C::TEXT_SECONDARY
                };
                p.spawn(text_bundle(line.clone(), if i == 0 { F::BODY_SMALL } else { F::CAPTION }, color));
            }
        });
    }
}

/// DEPTHS_MAP_PICK=<system name, or "hostile">: pick that system when the map
/// opens. DEPTHS_MAP_LIVE=1: open on Live. Automated runs have keys but no
/// mouse, so this is how they reach the panel and the chip.
fn map_pick_hook(
    mut state: ResMut<MapState>,
    existing: Query<(), Added<MapOverlay>>,
    mut dash: ResMut<PendingWarpTarget>,
    mut jump: ResMut<PendingGalaxyWarpTarget>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    if existing.is_empty() {
        return;
    }
    if std::env::var("DEPTHS_MAP_LIVE").is_ok() {
        state.live = true;
        state.side_dirty = true;
        state.paint_dirty = true;
        virtual_time.unpause();
    }
    let Ok(name) = std::env::var("DEPTHS_MAP_PICK") else { return };
    let found = if name == "hostile" {
        state.scene.systems.iter().find(|s| s.hostile && !s.here)
    } else {
        state.scene.systems.iter().find(|s| s.name == name)
    };
    if let Some(id) = found.map(|s| s.id) {
        set_pick(&mut state, MapPick::System(id), &mut dash, &mut jump);
    }
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let is_open = |q: Query<(), With<MapOverlay>>| !q.is_empty();
        app.init_resource::<MapState>().add_systems(
            Update,
            (
                toggle_map,
                map_scroll,
                map_pick_hook,
                (map_resized, map_tab, map_buttons, map_mouse, map_camera, map_live, map_repaint, map_tip)
                    .chain()
                    .run_if(is_open),
            )
                .chain()
                .run_if(in_state(GameState::Exploring).or_else(in_state(GameState::StationDocked))),
        )
        .add_systems(Update, warp_key_closes_map.run_if(in_state(GameState::Exploring)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAGE: Vec2 = Vec2::new(980.0, 720.0);

    fn known_map(seed: u64) -> GalaxyMap {
        let mut map = crate::celestial::galaxy::generate_galaxy_map(seed);
        for s in &mut map.systems {
            s.discovery = SystemDiscovery::Located;
        }
        map
    }

    fn scene_of(map: &GalaxyMap, standing: &FactionStanding) -> Scene {
        let war = BorderWar::default();
        let systems: Vec<SceneSystem> = map.systems.iter().map(|s| chart_system(s, map, standing, &war, s.id == 0)).collect();
        let home = systems.iter().find(|s| s.here).unwrap();
        let here = HereFrame { galaxy: home.galaxy, frame: home.frame };
        let you_world = home.frame.local;
        Scene {
            here,
            here_id: Some(0),
            here_name: home.name.clone(),
            you: here.frame.to_galaxy(you_world),
            you_world,
            fuel: 1000.0,
            max_fuel: 1500.0,
            systems,
            ..default()
        }
    }

    /// Everything painted, as (text, colour).
    fn painted(scene: &Scene, cam: MapCam) -> Vec<(String, Color)> {
        let mut world = World::new();
        let mut commands_queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut commands_queue, &world);
            commands.spawn(Node::default()).with_children(|p| {
                paint(p, scene, cam, STAGE, MapLayers::default(), MapPick::Nothing);
            });
        }
        commands_queue.apply(&mut world);
        let mut texts = world.query::<(&Text, &TextColor)>();
        texts.iter(&world).map(|(t, c)| (t.0.clone(), c.0)).collect()
    }

    #[test]
    fn a_click_lands_where_the_thing_was_drawn() {
        for scale in [whole_galaxy_scale(STAGE), 3e-4, system_fill_scale(STAGE)] {
            let cam = MapCam { center: Vec2::new(1_200_000.0, -800_000.0), scale };
            let pos = cam.center + Vec2::new(150_000.0, 90_000.0);
            assert!(cam.from_px(cam.to_px(pos, STAGE), STAGE).distance(pos) < 1.0);
        }
    }

    #[test]
    fn north_is_up() {
        let cam = MapCam { center: Vec2::ZERO, scale: 1e-4 };
        assert!(cam.to_px(Vec2::Y * 1000.0, STAGE).y < cam.to_px(Vec2::ZERO, STAGE).y);
    }

    /// Scrolling keeps what's under the cursor under the cursor, and scrolling
    /// back out always ends on the whole galaxy, wherever you went in.
    #[test]
    fn it_zooms_about_the_cursor_and_all_the_way_back_out() {
        let whole = MapCam { center: Vec2::ZERO, scale: whole_galaxy_scale(STAGE) };
        let cursor = Vec2::new(700.0, 200.0);
        let target = whole.from_px(cursor, STAGE);
        let close = whole.scrolled(3.0, cursor, STAGE);
        assert!(close.scale > whole.scale * 2.0);
        assert!(close.to_px(target, STAGE).distance(cursor) < 1.0, "{:?}", close.to_px(target, STAGE));

        let mut cam = close.scrolled(30.0, Vec2::new(40.0, 500.0), STAGE);
        assert!(cam.at_max_zoom(STAGE));
        for _ in 0..80 {
            cam = cam.scrolled(-1.0, Vec2::new(900.0, 40.0), STAGE);
        }
        assert_eq!(cam, whole);
    }

    /// A system opens up on the way in and is shut at the galaxy, however
    /// short the window.
    #[test]
    fn systems_open_only_as_you_get_close() {
        for stage in [STAGE, Vec2::new(1620.0, 1080.0), Vec2::new(500.0, 480.0)] {
            assert_eq!(openness(whole_galaxy_scale(stage)), 0.0);
            assert_eq!(openness(system_fill_scale(stage)), 1.0);
        }
    }

    /// Two systems never draw into each other, whatever the seed.
    #[test]
    fn no_systems_insides_reach_a_neighbour() {
        for seed in [1u64, 7, 42, 99, 12_345] {
            let map = known_map(seed);
            for a in &map.systems {
                for b in &map.systems {
                    if a.id != b.id {
                        assert!(a.galaxy_pos.distance(b.galaxy_pos) >= SYSTEM_DISC * 2.0);
                    }
                }
            }
        }
    }

    /// Every orbit fits its system's disc, big star or small.
    #[test]
    fn a_system_fits_inside_its_disc() {
        let map = known_map(42);
        let standing = FactionStanding::default();
        let war = BorderWar::default();
        for def in &map.systems {
            let sys = chart_system(def, &map, &standing, &war, false);
            for &(a, e) in &sys.orbits {
                assert!(a * (1.0 + e) * sys.frame.squeeze <= SYSTEM_DISC, "{}", sys.name);
            }
            let world = sys.frame.local + Vec2::new(1234.0, -987.0);
            // Far out on the map f32 only holds a few units; a click isn't that fine.
            assert!(sys.frame.to_world(sys.frame.to_galaxy(world)).distance(world) < 20.0);
        }
    }

    /// The ring is the distance the tank actually buys.
    #[test]
    fn the_reach_ring_is_what_the_drive_charges() {
        for fuel in [200.0f32, 700.0, 1400.0] {
            let reach = jump_reach(fuel).unwrap();
            let cost = interstellar_fuel_cost(reach / GALAXY_RADIUS);
            assert!((cost - fuel).abs() < 0.5, "{fuel} buys {reach} which costs {cost}");
        }
        assert_eq!(jump_reach(INTERSTELLAR_BASE_FUEL), None);
        assert_eq!(jump_reach(INTERSTELLAR_BASE_FUEL + INTERSTELLAR_MAX_EXTRA_FUEL + 1.0), None);
    }

    /// Hostile systems are named in red, friends aren't, and it follows your
    /// standing rather than a fixed list.
    #[test]
    fn hostile_systems_are_named_in_red() {
        let map = known_map(42);
        let mut standing = FactionStanding::default();
        let scene = scene_of(&map, &standing);
        let enemy = scene.systems.iter().find(|s| s.hostile && !s.haunted).unwrap();
        let cam = MapCam { center: enemy.galaxy, scale: 2.5e-4 };
        let all = painted(&scene, cam);
        let label = all.iter().find(|(t, _)| *t == enemy.name).expect("the hostile system is named");
        assert_eq!(label.1, faded(HOSTILE_TEXT, 1.0));

        standing.change(enemy.owner.unwrap(), 200.0, "peace");
        let at_peace = scene_of(&map, &standing);
        let same = at_peace.system(enemy.id).unwrap();
        assert!(!same.hostile);
        assert!(same.brief[0].text == "Not hostile");
        assert!(enemy.brief[0].text.starts_with("HOSTILE"));
        let painted_again = painted(&at_peace, cam);
        let label = painted_again.iter().find(|(t, _)| *t == enemy.name).expect("still named");
        assert_ne!(label.1, faded(HOSTILE_TEXT, 1.0));
    }

    /// Zoomed in, a system's planets and stations are there to read. Zoomed
    /// out they aren't, and what you've never found is never drawn.
    #[test]
    fn zooming_in_shows_whats_inside_and_never_the_unknown() {
        let mut map = known_map(42);
        let hidden = map.systems[5].id;
        let hidden_name = map.systems[5].name.clone();
        map.systems[5].discovery = SystemDiscovery::Unknown;
        let standing = FactionStanding::default();
        let war = BorderWar::default();
        let systems: Vec<SceneSystem> = map.systems.iter()
            .filter(|s| s.id != hidden)
            .map(|s| chart_system(s, &map, &standing, &war, s.id == 0))
            .collect();
        let target = systems.iter().find(|s| s.id != 0 && !s.stations.is_empty()).unwrap();
        let (planet, station) = (target.planets[0].name.clone(), target.stations[0].name.clone());
        let center = target.galaxy;
        let scene = Scene { systems, ..default() };

        let far = painted(&scene, MapCam { center: Vec2::ZERO, scale: whole_galaxy_scale(STAGE) });
        assert!(!far.iter().any(|(t, _)| *t == station), "{station} shown from the whole galaxy");
        assert!(!far.iter().any(|(t, _)| *t == hidden_name), "the unknown {hidden_name} was named");

        let near = painted(&scene, MapCam { center, scale: system_fill_scale(STAGE) });
        assert!(near.iter().any(|(t, _)| *t == station), "{station} not shown: {near:?}");
        assert!(near.iter().any(|(t, _)| *t == planet), "{planet} not shown: {near:?}");
    }

    /// In your own system a pick is a dash; in any other it's a jump to that
    /// system. Between the stars it's a blind jump. Picking it never does both.
    #[test]
    fn a_pick_is_a_dash_here_and_a_jump_elsewhere() {
        let map = known_map(42);
        let scene = scene_of(&map, &FactionStanding::default());
        let other = scene.systems.iter().find(|s| !s.here).unwrap();
        let home = scene.system(0).unwrap();

        let (dash, jump) = MapPick::Station(0, 0).warp_target(&scene);
        assert_eq!(dash, Some(home.stations[0].world));
        assert_eq!(jump, None);

        let (dash, jump) = MapPick::Planet(other.id, 0).warp_target(&scene);
        assert_eq!(dash, None);
        assert_eq!(jump, Some(GalaxyWarpTarget::System(other.id)));

        assert_eq!(MapPick::System(0).warp_target(&scene), (None, None));
        let empty = Vec2::new(4_000_000.0, 4_000_000.0);
        assert_eq!(MapPick::Blind(empty).warp_target(&scene), (None, Some(GalaxyWarpTarget::BlindPoint(empty))));
    }

    /// Open space means the system around the click, if the map is close
    /// enough to see it, and the void otherwise.
    #[test]
    fn empty_space_picks_what_it_is() {
        let map = known_map(42);
        let scene = scene_of(&map, &FactionStanding::default());
        let cam = MapCam { center: scene.here.galaxy, scale: system_fill_scale(STAGE) };
        let px = cam.to_px(scene.here.galaxy + Vec2::new(60_000.0, 0.0), STAGE);
        let MapPick::Point(world) = pick_in_space(&scene, cam, STAGE, px) else { panic!("not a point") };
        let expected = scene.here.frame.to_world(scene.here.galaxy + Vec2::new(60_000.0, 0.0));
        assert!(world.distance(expected) < 2.0);

        let other = scene.systems.iter().find(|s| !s.here).unwrap();
        let cam = MapCam { center: other.galaxy, scale: system_fill_scale(STAGE) };
        let px = cam.to_px(other.galaxy + Vec2::new(60_000.0, 0.0), STAGE);
        assert_eq!(pick_in_space(&scene, cam, STAGE, px), MapPick::System(other.id));
    }

    #[test]
    fn opening_the_map_reselects_where_you_were_going() {
        assert_eq!(pick_from_targets(None, None), MapPick::Nothing);
        assert_eq!(pick_from_targets(None, Some(GalaxyWarpTarget::System(3))), MapPick::System(3));
        let at = Vec2::new(10.0, 20.0);
        assert_eq!(pick_from_targets(Some(at), None), MapPick::Point(at));
        assert_eq!(pick_from_targets(Some(at), Some(GalaxyWarpTarget::BlindPoint(at))), MapPick::Blind(at));
    }

    #[test]
    fn a_route_is_cut_to_the_stage() {
        let (a, b) = clip_to_stage(Vec2::new(-5000.0, 100.0), Vec2::new(5000.0, 100.0), STAGE).unwrap();
        assert_eq!((a.x, b.x), (0.0, STAGE.x));
        assert!(clip_to_stage(Vec2::new(-5000.0, -100.0), Vec2::new(5000.0, -100.0), STAGE).is_none());
    }
}
