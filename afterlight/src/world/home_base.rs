use bevy::prelude::*;
use crate::components::{Ship, ShipPhysics, Velocity, Weapon};
use crate::events::{ShowNotification, NotificationType};
use crate::resources::{OxygenState, FuelState};
use crate::states::GameState;
use super::station_types::{StationType, station_type, station_type_name};
use crate::celestial::poi::KeepClear;
use crate::celestial::resources::StarSystemDef;

// ============================================================================
// STATIONS
// Every star system carries STATIONS_PER_SYSTEM full stations, spread wide
// around that system's own space. They are all "full" stations: dock with F
// and you get the shipyard (build mode), the shop, the bounty board and the
// hiring hall, exactly like Haven — the old model had thirteen stations
// crammed into Haven's system alone, twelve of them resupply-only blobs you
// could do nothing with but sell cargo, and every other system in the galaxy
// had none at all.
//
// Haven (system 0, slot 0) keeps its fixed position: it's where the ship
// spawns and where the berth is.
// ============================================================================

/// Marker for Haven specifically — the one station with a name of its own,
/// still used for its bigger collision hull and its "home" framing.
#[derive(Component)]
pub struct HomeStation;

/// Any station structure in the loaded system. `index` is the global station
/// index used for contract boards and pricing (see station_index).
#[derive(Component)]
pub struct Station {
    pub index: usize,
    pub system_id: u32,
}

/// Stations per star system. Two is deliberately sparse: a station is a
/// destination worth flying to, not scenery.
pub const STATIONS_PER_SYSTEM: usize = 2;

/// Upper bound on system ids the contract-board index space reserves room
/// for (galaxy::SYSTEM_COUNT is 30 today; boards are generated lazily, so
/// over-reserving costs an empty Vec entry each).
pub const MAX_SYSTEMS: usize = 64;

/// Size of the global station index space — see contracts::STATION_COUNT.
pub const TOTAL_STATION_SLOTS: usize = MAX_SYSTEMS * STATIONS_PER_SYSTEM;

/// Radius of Haven's structure: centre to the tip of a docking arm. Every
/// other station is built at OTHER_STATION_SCALE of it.
///
/// Was about 200. Next to the starter pincer (~1,800 long) a station read as
/// a drone parked beside the ship rather than a place you dock at; at this
/// size the station is about three times the length of the ship.
pub const HAVEN_RADIUS: f32 = 2_600.0;
const OTHER_STATION_SCALE: f32 = 0.8;

/// Structure radius of the station with this global index.
pub fn station_radius(index: usize) -> f32 {
    if index == 0 { HAVEN_RADIUS } else { HAVEN_RADIUS * OTHER_STATION_SCALE }
}

/// How far past a station's structure the ship's ROOT can be and still dock.
/// Stations and hulls are solid (see ship::collision), and the root of a big
/// ship can sit ~800 units behind its own nose, so the range is measured from
/// the station's edge, with room for the hull in between.
///
/// 7 km, at the playtester's call. At 1.8 km a station kilometres across
/// filled the screen well before F would take, and "close enough" by eye
/// was not close enough by the rules. The map's "Nearest station" reads
/// from the same edge, so under 7 km there means you can dock.
pub const DOCK_MARGIN: f32 = 7_000.0;

/// The largest docking range of any station (Haven's), from its centre. For
/// code that needs one bound; per-station checks use StationSite::dock_range.
pub const DOCK_RANGE: f32 = HAVEN_RADIUS + DOCK_MARGIN;

/// Docked ships berth out past the tip of the east docking arm.
const BERTH_DIR: Vec2 = Vec2::X;
/// Clearance between a station's structure and a berthed ship's hull.
const BERTH_GAP: f32 = 250.0;
/// Radius of the starter hull, for the berth it spawns at before it has a
/// collider of its own. A little over the pincer's own ~1,100.
const STARTER_BERTH_RADIUS: f32 = 1_200.0;

/// Where a ship of `ship_radius` (root to farthest hull) berths at a station.
pub fn berth_position(station_pos: Vec2, station_radius: f32, ship_radius: f32) -> Vec2 {
    station_pos + BERTH_DIR * (station_radius + ship_radius + BERTH_GAP)
}

/// The berth the game opens at. Kept at (0, -50) as it always has been --
/// everything near Haven (the star's distance, the asteroid field, the chunk
/// layer's keep-clear) is placed relative to it -- so Haven itself sits to
/// the west of it.
pub const SPAWN_BERTH: Vec2 = Vec2::new(0.0, -50.0);

/// Haven Station's fixed world position: due west of the spawn berth by its
/// own radius, the starter hull, and the gap.
pub const STATION_POS: Vec2 = Vec2::new(
    SPAWN_BERTH.x - (HAVEN_RADIUS + STARTER_BERTH_RADIUS + BERTH_GAP),
    SPAWN_BERTH.y,
);

/// One station's identity and placement. Derived deterministically from the
/// system it belongs to, so it's identical every time that system loads and
/// needs no save data of its own.
#[derive(Clone, Debug)]
pub struct StationSite {
    /// Global index — contract board, prices, faction reputation.
    pub index: usize,
    pub system_id: u32,
    pub pos: Vec2,
    pub name: String,
    pub kind: StationType,
}

impl StationSite {
    /// Centre to the tip of a docking arm.
    pub fn radius(&self) -> f32 {
        station_radius(self.index)
    }

    /// Ship-root distance from the centre inside which this station docks.
    pub fn dock_range(&self) -> f32 {
        self.radius() + DOCK_MARGIN
    }
}

/// Global station index for a (system, slot) pair. Haven is 0.
pub fn station_index(system_id: u32, slot: usize) -> usize {
    system_id as usize * STATIONS_PER_SYSTEM + slot
}

/// Mirrors the names galaxy::generate_galaxy_map gives its systems, so a
/// station name can be derived from its index alone (no galaxy lookup).
pub fn system_display_name(system_id: u32) -> String {
    crate::celestial::galaxy::system_name(system_id)
}

/// How far past the dock radius the ship must get before a station will
/// prompt again.
const PROMPT_REARM: f32 = 1.5;

/// "Haven Station (Shipyard) in range". Most station names already end in
/// their type ("Vesper Trade Hub"), and appending it again read
/// "Vesper Trade Hub (Trade Hub)".
fn dock_prompt(name: &str, kind: &str) -> String {
    if name.ends_with(kind) {
        format!("{name} in range - press F to dock")
    } else {
        format!("{name} ({kind}) in range - press F to dock")
    }
}

/// Display name for a global station index.
pub fn station_display_name(index: usize) -> String {
    if index == 0 {
        return "Haven Station".to_string();
    }
    let system_id = (index / STATIONS_PER_SYSTEM) as u32;
    format!("{} {}", system_display_name(system_id), station_type_name(station_type(index)))
}

/// Deterministic station layout for one system. Stations are pushed far out
/// from the system center and away from each other — a golden-angle spread
/// keyed by (system, slot) means no two stations in a system share a
/// direction, and no two systems put theirs in the same relative spot.
///
/// Each spot is then nudged along its bearing until the station and its
/// docking range keep clear of the star and of every planet's path (see
/// `poi::KeepClear`). Planets are solid and sweep their whole ellipse every
/// few minutes, and placed without them 16 of the 31 systems in the seed-42
/// galaxy had a station a planet ran through -- and with it the point a jump
/// arrives at. The planets come from the system's seed (`galaxy::system_roll`,
/// the same rolls its spawn makes), so this needs nothing spawned and gives
/// the same answer whether the system is loaded or not.
pub fn station_sites(def: &StarSystemDef) -> Vec<StationSite> {
    let roll = crate::celestial::galaxy::system_roll(def);
    let planets = roll.planet_paths();
    let keep_clear = KeepClear {
        center: def.local_center,
        star_radius: roll.star_radius(),
        planets: &planets,
    };
    (0..STATIONS_PER_SYSTEM)
        .map(|slot| {
            let index = station_index(def.id, slot);
            let pos = if index == 0 {
                // Haven keeps its fixed spot: the ship spawns beside it.
                STATION_POS
            } else {
                let n = index as f32;
                let angle = n * 2.399963; // golden angle, radians
                // 180k-420k out to begin with, far enough apart that two
                // stations never share a screen.
                let radius = 180_000.0 + ((n * 0.6180339).fract()) * 240_000.0;
                keep_clear.nudge(
                    def.local_center + Vec2::new(angle.cos(), angle.sin()) * radius,
                    station_radius(index) + DOCK_MARGIN,
                )
            };
            StationSite {
                index,
                system_id: def.id,
                pos,
                name: station_display_name(index),
                kind: station_type(index),
            }
        })
        .collect()
}

/// The loaded system's stations. Rebuilt whenever the streamed system
/// changes (see refresh_system_stations) so every consumer — docking, the
/// map, radar, the contract board — reads one list instead of each deriving
/// its own.
#[derive(Resource, Default)]
pub struct SystemStations {
    pub system_id: Option<u32>,
    pub sites: Vec<StationSite>,
}

impl SystemStations {
    /// Nearest station to `pos` within docking range, if any.
    /// Distance to the nearest station in the loaded system, if it has any.
    ///
    /// The zone names (Station Orbit, Near Space, Asteroid Belt...) were
    /// measured from Haven at the origin, so in any other system the ship is
    /// a thousand km out and everything read "Black Hole Proximity" -- the HUD
    /// said it permanently and every arrival announced "Entering Black Hole
    /// Proximity". Measured from the local station they mean the same thing
    /// in every system.
    ///
    /// Measured from the structure's edge, not its centre, so a big station
    /// doesn't put a ship berthed at it a few kilometres "out".
    pub fn local_range(&self, pos: Vec2) -> Option<f32> {
        self.sites
            .iter()
            .map(|s| (pos.distance(s.pos) - s.radius()).max(0.0))
            .reduce(f32::min)
    }

    pub fn nearest_in_range(&self, pos: Vec2) -> Option<&StationSite> {
        self.sites
            .iter()
            .filter(|s| pos.distance(s.pos) < s.dock_range())
            .min_by(|a, b| {
                pos.distance_squared(a.pos)
                    .partial_cmp(&pos.distance_squared(b.pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Contract-board index of the station in docking range, if any. Used to
    /// pick which board the mission board shows and to gate claiming rewards.
    pub fn nearest_index(&self, pos: Vec2) -> Option<usize> {
        self.nearest_in_range(pos).map(|s| s.index)
    }

    /// Closest station regardless of range — what the HUD arrow points at.
    pub fn closest(&self, pos: Vec2) -> Option<&StationSite> {
        self.sites.iter().min_by(|a, b| {
            pos.distance_squared(a.pos)
                .partial_cmp(&pos.distance_squared(b.pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn positions(&self) -> impl Iterator<Item = Vec2> + '_ {
        self.sites.iter().map(|s| s.pos)
    }
}

/// Keeps SystemStations pointed at whichever system is currently streamed in.
/// Falls back to Haven before the galaxy has been generated (the game opens
/// docked at Haven, which is a frame or two before celestial's OnEnter
/// (Exploring) generation runs) so Haven Station is never missing.
pub fn refresh_system_stations(
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    galaxy_map: Res<crate::celestial::resources::GalaxyMap>,
    mut stations: ResMut<SystemStations>,
) {
    let current = streaming.loaded_system.or(if galaxy_map.systems.is_empty() { Some(0) } else { None });

    if stations.system_id == current && !(current.is_some() && stations.sites.is_empty()) {
        return;
    }

    stations.system_id = current;
    stations.sites = match current {
        Some(id) => match galaxy_map.systems.iter().find(|s| s.id == id) {
            Some(def) => station_sites(def),
            // Docked at Haven before the galaxy is rolled. Haven's layout is
            // fixed, so its stations don't have to wait for the rest.
            None if id == 0 => station_sites(&crate::celestial::galaxy::haven_def()),
            // A system with no record has no planets to place around; stay
            // empty, which re-runs this once the galaxy catches up.
            None => Vec::new(),
        },
        // Blind-warped into empty space: no system, no stations.
        None => Vec::new(),
    };
}

/// Spawns/despawns station structures so the world always shows exactly the
/// loaded system's stations. Reconciling every frame (rather than hooking
/// system load/unload) keeps warp, game start and save-load on one path.
pub fn sync_station_entities(
    mut commands: Commands,
    stations: Res<SystemStations>,
    existing: Query<(Entity, &Station)>,
) {
    let mut present: Vec<usize> = Vec::new();
    for (entity, station) in existing.iter() {
        let still_here = stations.system_id == Some(station.system_id)
            && stations.sites.iter().any(|s| s.index == station.index);
        if still_here {
            present.push(station.index);
        } else {
            commands.entity(entity).despawn();
        }
    }

    for site in stations.sites.iter() {
        if !present.contains(&site.index) {
            spawn_station(&mut commands, site);
        }
    }
}

/// A station's layout, in units of its radius (1.0 = tip of a docking arm).
/// The drawing and the collision shape are both built from these, so they
/// can't drift apart.
mod shape {
    /// Half-width of the central hub.
    pub const CORE: f32 = 0.22;
    /// Centreline radius and thickness of the habitat ring.
    pub const RING_R: f32 = 0.52;
    pub const RING_W: f32 = 0.09;
    /// Docking arms: width, and where they start (inside the hub).
    pub const ARM_W: f32 = 0.075;
    pub const ARM_FROM: f32 = 0.18;
    /// Docking heads at the arm tips: depth along the arm, and width across.
    pub const HEAD_LEN: f32 = 0.10;
    pub const HEAD_W: f32 = 0.20;
    /// Where the arm stops and the head begins.
    pub const ARM_TO: f32 = 1.0 - HEAD_LEN;
    /// Ring segments. Enough that the ring reads round at any zoom.
    pub const RING_SEGMENTS: usize = 32;
}

/// The four docking-arm directions.
const ARMS: [Vec2; 4] = [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y];

/// Solar panel size, and where the panels sit: two per side on the north
/// and south arms.
const SOLAR_PANEL: Vec2 = Vec2::new(0.22, 0.10);
fn solar_panel_centers() -> impl Iterator<Item = Vec2> {
    [Vec2::Y, Vec2::NEG_Y].into_iter().flat_map(|dir| {
        [0.68, 0.80].into_iter().flat_map(move |along| {
            [-1.0f32, 1.0].into_iter().map(move |side| {
                dir * along + Vec2::X * side * (shape::ARM_W * 0.5 + 0.11)
            })
        })
    })
}

/// Radiator fin size, and where the fins sit along the west arm.
const RADIATOR: Vec2 = Vec2::new(0.022, 0.12);
fn radiator_centers() -> impl Iterator<Item = Vec2> {
    [0.66, 0.74, 0.82].into_iter().flat_map(|along| {
        [-1.0f32, 1.0].into_iter().map(move |side| {
            Vec2::NEG_X * along + Vec2::Y * side * (shape::ARM_W * 0.5 + 0.06)
        })
    })
}

/// Collision shape for a station of `radius`: one disc over the hub and ring,
/// then each docking arm as a row of circles ending in its head. A single
/// circle either walled off the empty space between the arms or let ships
/// through the arms.
pub fn station_collider_circles(radius: f32) -> Vec<(Vec2, f32)> {
    use shape::*;
    let mut circles = vec![(Vec2::ZERO, (RING_R + RING_W * 0.5) * radius)];
    for dir in ARMS {
        for along in [0.64, 0.74, 0.84] {
            circles.push((dir * along * radius, ARM_W * 0.7 * radius));
        }
        circles.push((dir * (ARM_TO + HEAD_LEN * 0.5) * radius, HEAD_W * 0.5 * radius));
    }
    // The solar wings and radiator fins are hull too. Drawn but not solid,
    // a ship could slide straight over them -- one autoplay run sat parked on
    // top of Haven's solar array for minutes.
    for at in solar_panel_centers() {
        for across in [-0.3, 0.0, 0.3] {
            circles.push(((at + Vec2::X * SOLAR_PANEL.x * across) * radius, SOLAR_PANEL.y * 0.75 * radius));
        }
    }
    for at in radiator_centers() {
        circles.push((at * radius, RADIATOR.y * 0.5 * radius));
    }
    circles
}

/// Every sprite a station is drawn from.
#[derive(Component)]
pub struct StationPart;

/// Running lights on the docking heads and the ring: a slow fade in and out,
/// each on its own beat so the station never blinks in unison.
#[derive(Component)]
pub struct StationBeacon {
    phase: f32,
    color: Color,
}

pub fn pulse_station_beacons(time: Res<Time>, mut beacons: Query<(&StationBeacon, &mut Sprite)>) {
    let t = time.elapsed_secs();
    for (beacon, mut sprite) in &mut beacons {
        // Mostly dim with a soft swell, not an on/off strobe.
        let swell = 0.5 + 0.5 * (t * 2.6 + beacon.phase).sin();
        sprite.color = beacon.color.with_alpha(0.25 + 0.75 * swell * swell);
    }
}

/// Builds one station structure: a hub, a habitat ring on four spokes, four
/// docking arms with lit heads, solar wings, radiators and running lights.
/// Haven is the largest; the accent colour codes the station type.
fn spawn_station(commands: &mut Commands, site: &StationSite) {
    use shape::*;
    let is_haven = site.index == 0;
    let r = site.radius();
    let accent = station_accent(site.kind);

    let root = commands
        .spawn((
            Transform::from_xyz(site.pos.x, site.pos.y, 0.05),
            Visibility::default(),
            Station { index: site.index, system_id: site.system_id },
        ))
        .id();
    if is_haven {
        commands.entity(root).insert(HomeStation);
    }

    // Everything below is in station units (radius 1.0); `part` scales it.
    let part = |commands: &mut Commands, size: Vec2, color: Color, at: Vec2, angle: f32, z: f32| {
        let child = commands
            .spawn((
                Sprite { color, custom_size: Some(size * r), ..default() },
                Transform::from_translation((at * r).extend(z)).with_rotation(Quat::from_rotation_z(angle)),
                StationPart,
            ))
            .id();
        commands.entity(root).add_child(child);
        child
    };
    let beacon = |commands: &mut Commands, at: Vec2, size: f32, phase: f32| {
        let child = commands
            .spawn((
                Sprite { color: accent, custom_size: Some(Vec2::splat(size * r)), ..default() },
                Transform::from_translation((at * r).extend(0.012)),
                StationBeacon { phase, color: accent },
                StationPart,
            ))
            .id();
        commands.entity(root).add_child(child);
    };

    let hull_dark = Color::srgb(0.15, 0.17, 0.24);
    let hull = Color::srgb(0.20, 0.22, 0.30);
    let hull_light = Color::srgb(0.25, 0.28, 0.38);
    let window = Color::srgb(0.95, 0.85, 0.45);
    let tau = std::f32::consts::TAU;
    let quarter = std::f32::consts::FRAC_PI_2;

    // Habitat ring: segments laid tangentially, a lighter panel every fourth.
    let seg_len = tau * RING_R / RING_SEGMENTS as f32 * 1.08;
    for i in 0..RING_SEGMENTS {
        let a = i as f32 / RING_SEGMENTS as f32 * tau;
        let at = Vec2::from_angle(a) * RING_R;
        let color = if i % 4 == 0 { hull_light } else { hull };
        part(commands, Vec2::new(seg_len, RING_W), color, at, a + quarter, 0.0);
        // Inner edge trim, and a pair of lit windows on alternate segments.
        part(commands, Vec2::new(seg_len, RING_W * 0.18), hull_dark, Vec2::from_angle(a) * (RING_R - RING_W * 0.38), a + quarter, 0.001);
        if i % 2 == 1 {
            for off in [-0.3, 0.3] {
                let w = at + Vec2::from_angle(a + quarter) * seg_len * off;
                part(commands, Vec2::new(0.012, 0.018), window, w, a + quarter, 0.002);
            }
        }
    }

    // Spokes on the diagonals, clear of the docking arms.
    for k in 0..4 {
        let a = std::f32::consts::FRAC_PI_4 + k as f32 * quarter;
        let mid = (CORE + RING_R - RING_W * 0.5) * 0.5;
        part(commands, Vec2::new(RING_R - RING_W * 0.5 - CORE, 0.035), hull_dark, Vec2::from_angle(a) * mid, a, 0.003);
    }

    // Hub: two squares at 45deg to each other read as an octagon, with a
    // lighter deck on top and a grid of lit windows.
    part(commands, Vec2::splat(CORE * 2.0), hull_dark, Vec2::ZERO, 0.0, 0.004);
    part(commands, Vec2::splat(CORE * 2.0), hull_dark, Vec2::ZERO, std::f32::consts::FRAC_PI_4, 0.004);
    part(commands, Vec2::splat(CORE * 1.55), hull_light, Vec2::ZERO, std::f32::consts::FRAC_PI_4, 0.005);
    part(commands, Vec2::splat(CORE * 1.1), hull, Vec2::ZERO, 0.0, 0.006);
    for gx in -2..=2 {
        for gy in [-1, 1] {
            part(commands, Vec2::new(0.016, 0.012), window, Vec2::new(gx as f32 * 0.04, gy as f32 * 0.05), 0.0, 0.007);
        }
    }

    // Docking arms and their heads.
    for (k, dir) in ARMS.into_iter().enumerate() {
        let angle = dir.to_angle();
        let len = ARM_TO - ARM_FROM;
        let mid = dir * (ARM_FROM + len * 0.5);
        part(commands, Vec2::new(len, ARM_W), hull, mid, angle, 0.008);
        part(commands, Vec2::new(len, ARM_W * 0.3), hull_dark, mid, angle, 0.009);
        // Head: a T across the arm's end, pads lit in the station's colour.
        let head = dir * (ARM_TO + HEAD_LEN * 0.5);
        part(commands, Vec2::new(HEAD_LEN, HEAD_W), hull_light, head, angle, 0.010);
        part(commands, Vec2::new(HEAD_LEN * 0.4, HEAD_W * 0.8), hull_dark, head + dir * HEAD_LEN * 0.2, angle, 0.011);
        let across = Vec2::new(-dir.y, dir.x);
        for side in [-1.0, 1.0] {
            beacon(commands, head + across * HEAD_W * 0.38 * side + dir * HEAD_LEN * 0.4, 0.022, k as f32 * 1.7 + side);
        }
    }

    // Solar wings on the north and south arms.
    let panel = Color::srgb(0.10, 0.16, 0.30);
    let panel_frame = Color::srgb(0.30, 0.36, 0.50);
    for at in solar_panel_centers() {
        part(commands, SOLAR_PANEL, panel_frame, at, 0.0, 0.007);
        part(commands, SOLAR_PANEL * Vec2::new(0.91, 0.85), panel, at, 0.0, 0.0075);
    }

    // Radiator fins along the west arm.
    let radiator = Color::srgb(0.42, 0.47, 0.56);
    for at in radiator_centers() {
        part(commands, RADIATOR, radiator, at, 0.0, 0.007);
    }

    // A few running lights around the ring.
    for k in 0..8 {
        let a = k as f32 / 8.0 * tau + 0.2;
        beacon(commands, Vec2::from_angle(a) * (RING_R + RING_W * 0.5), 0.016, k as f32 * 0.9);
    }

    // Name plate above the station. Most names already end in their type
    // ("Vesper Trade Hub"); Haven's doesn't, so its type rides underneath.
    let kind = station_type_name(site.kind);
    let label = commands
        .spawn((
            Text2d::new(site.name.to_uppercase()),
            TextFont { font_size: FontSize::Px(if is_haven { 28.0 } else { 24.0 }), ..default() },
            TextColor(Color::srgba(0.7, 0.8, 1.0, 0.8)),
            Transform::from_xyz(0.0, r * 1.12, 0.03),
            crate::camera::ZoomInvariantText,
        ))
        .id();
    if !site.name.ends_with(kind) {
        let span = commands
            .spawn((
                TextSpan::new(format!("\n{}", kind.to_uppercase())),
                TextFont { font_size: FontSize::Px(16.0), ..default() },
                TextColor(Color::srgba(0.6, 0.7, 0.9, 0.6)),
            ))
            .id();
        commands.entity(label).add_child(span);
    }
    commands.entity(root).add_child(label);
}

/// Accent color per station type — the same coding the map legend uses.
pub fn station_accent(kind: StationType) -> Color {
    match kind {
        StationType::Shipyard => Color::srgb(0.85, 0.70, 0.25),
        StationType::MiningColony => Color::srgb(0.80, 0.45, 0.25),
        StationType::TradeHub => Color::srgb(0.35, 0.85, 0.45),
        StationType::MilitaryOutpost => Color::srgb(0.85, 0.30, 0.30),
        StationType::ResearchOutpost => Color::srgb(0.40, 0.65, 1.00),
        StationType::RefuelDepot => Color::srgb(0.55, 0.80, 0.85),
    }
}

/// Marker for the HUD arrow that points to the nearest station.
#[derive(Component)]
pub struct BaseArrow;

/// Spawns the nearest-station arrow once. (Stations themselves are spawned by
/// sync_station_entities, which follows whichever system is loaded.)
pub fn spawn_base_arrow(mut commands: Commands, existing: Query<(), With<BaseArrow>>) {
    if !existing.is_empty() {
        return;
    }

    let arrow_root = commands
        // ZoomInvariantText scales it with the camera: at a fixed 34 world
        // units it was a ~6px glyph at the framing zoom.
        .spawn((Transform::from_xyz(0.0, 0.0, 5.0), Visibility::Hidden, BaseArrow, crate::camera::ZoomInvariantText))
        .id();
    let shaft = commands
        .spawn((
            Sprite { color: Color::srgba(0.5, 0.8, 1.0, 0.8), custom_size: Some(Vec2::new(34.0, 6.0)), ..default() },
            Transform::from_xyz(-8.0, 0.0, 0.0),
        ))
        .id();
    let head = commands
        .spawn((
            Sprite { color: Color::srgba(0.6, 0.9, 1.0, 0.9), custom_size: Some(Vec2::new(14.0, 14.0)), ..default() },
            Transform {
                translation: Vec3::new(14.0, 0.0, 0.0),
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
                ..default()
            },
        ))
        .id();
    commands.entity(arrow_root).add_children(&[shaft, head]);
}

/// Where a nav arrow pointing along `dir` sits: just outside the hull.
///
/// It used to orbit 150 units from the ship's root, and the pincer reaches
/// ~1,000 units from its root -- the arrow was drawn on top of the ship's own
/// blocks, where nobody could pick it out of the hull art.
pub fn nav_arrow_orbit(ship: &Transform, collider: Option<&crate::ship::collision::Collider>, dir: Vec2) -> Vec2 {
    let (centre, radius) = match collider {
        Some(c) => (ship.transform_point(c.bound_center.extend(0.0)).truncate(), c.bound_radius),
        None => (ship.translation.truncate(), 150.0),
    };
    centre + dir * (radius + 140.0)
}

/// Point the arrow from the ship toward the nearest station; hidden when
/// already close to one (or when there's no station at all — blind space).
pub fn update_base_arrow(
    stations: Res<SystemStations>,
    ship_query: Query<(&Transform, Option<&crate::ship::collision::Collider>), (With<Ship>, Without<BaseArrow>)>,
    mut arrow_query: Query<(&mut Transform, &mut Visibility), With<BaseArrow>>,
) {
    let Ok((ship_transform, collider)) = ship_query.single() else { return };
    let Ok((mut arrow_transform, mut vis)) = arrow_query.single_mut() else { return };
    let ship_pos = ship_transform.translation.truncate();

    let Some(nearest) = stations.closest(ship_pos).map(|s| s.pos) else {
        *vis = Visibility::Hidden;
        return;
    };

    // Hidden inside docking range -- the HUD already says "press F to dock"
    // there, and at the berth itself the arrow pointed at the station you
    // were moored to. (It was hidden under 600 units, which the berth, a
    // few km off a 5 km station, never is.)
    if stations.nearest_in_range(ship_pos).is_some() {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    let dir = (nearest - ship_pos).normalize_or_zero();
    let orbit = nav_arrow_orbit(ship_transform, collider, dir);
    arrow_transform.translation.x = orbit.x;
    arrow_transform.translation.y = orbit.y;
    arrow_transform.rotation = Quat::from_rotation_z(dir.y.atan2(dir.x));
}

/// What docking puts back in a magazine for free: a quarter of it, so a
/// broke captain can still fight their way to the next station. Power-fed
/// guns (no real magazine) are simply full.
pub fn dock_ammo_reserve(max_ammo: u32) -> u32 {
    if max_ammo >= crate::combat::ammo_types::POWER_FED_AMMO {
        max_ammo
    } else {
        max_ammo.div_ceil(4)
    }
}

/// Fly within range of any station and press F to dock: the ship parks at
/// that station's berth, momentum dies, supplies top up and the game enters
/// StationDocked — build mode, shop, bounty board, hiring, everywhere.
/// (Before, only Haven did this; the twelve outposts opened a sell-only trade
/// menu instead.)
pub fn station_docking(
    mut press: ResMut<crate::resources::InteractPress>,
    stations: Res<SystemStations>,
    mut ship_query: Query<(Entity, &mut Transform, &mut Velocity, &mut ShipPhysics, Option<&crate::ship::collision::Collider>), With<Ship>>,
    mut weapon_query: Query<(&mut Weapon, &ChildOf)>,
    mut oxygen_state: ResMut<OxygenState>,
    mut fuel_state: ResMut<FuelState>,
    mut notifications: MessageWriter<ShowNotification>,
    mut next_state: ResMut<NextState<GameState>>,
    mut prompted_for: Local<Option<usize>>,
) {
    let Ok((ship_entity, mut transform, mut velocity, mut physics, collider)) = ship_query.single_mut() else { return };
    let ship_pos = transform.translation.truncate();

    let Some(site) = stations.nearest_in_range(ship_pos) else {
        // Re-arm the prompt only once the ship is properly clear of the
        // station it named. Re-arming at the dock radius itself repeated
        // "press F to dock" every time a ship hovering nearby dipped across
        // it -- 23 times in ten minutes of one run.
        let clear = prompted_for.is_none_or(|index| {
            stations
                .sites
                .iter()
                .find(|s| s.index == index)
                .is_none_or(|s| ship_pos.distance(s.pos) > DOCK_RANGE * PROMPT_REARM)
        });
        if clear {
            *prompted_for = None;
        }
        return;
    };

    if *prompted_for != Some(site.index) {
        *prompted_for = Some(site.index);
        notifications.write(ShowNotification {
            message: dock_prompt(&site.name, station_type_name(site.kind)),
            notification_type: NotificationType::Info,
            duration: 4.0,
        });
    }

    // Claims the shared interact press (see resources::InteractPress). A
    // salvage detail out on the hull, or a wreck closer than this station,
    // takes it first - docking would otherwise strand them.
    if press.claim() {
        // Clear of the structure whatever the ship's size: the collider's
        // far edge from the root, or the starter's if it has none yet.
        let ship_radius = collider
            .map(|c| c.bound_center.length() + c.bound_radius)
            .unwrap_or(STARTER_BERTH_RADIUS);
        let berth = berth_position(site.pos, site.radius(), ship_radius);
        transform.translation.x = berth.x;
        transform.translation.y = berth.y;
        // Square the ship up with the build grid — modules are placed in
        // unrotated grid space, so a tilted ship would misalign the ghost.
        transform.rotation = Quat::IDENTITY;
        physics.rotation = 0.0;
        velocity.0 = Vec2::ZERO;
        physics.angular_velocity = 0.0;
        physics.throttle = 0.0;

        // Docking is safety, not a free tank.
        //
        // This used to fill fuel and oxygen outright, which made the Refuel
        // and Refill services in the docking menu permanently unavailable --
        // they were priced, displayed, and could never be bought, because
        // arriving had already done it for nothing. It also meant distance
        // cost nothing: with two stations in every system you could cross the
        // galaxy topping up as you went.
        //
        // Air stays free: suffocating at a berth is not a decision, it is a
        // bug. Fuel tops up to a reserve that guarantees a short jump, so a
        // broke player is never stranded, and the rest is bought.
        oxygen_state.current_oxygen = oxygen_state.max_oxygen;
        const DOCK_FUEL_RESERVE: f32 = 260.0;
        let topped_up = fuel_state.current_fuel < DOCK_FUEL_RESERVE;
        if topped_up {
            fuel_state.current_fuel = DOCK_FUEL_RESERVE.min(fuel_state.max_fuel);
        }
        // Ammo the same way. Magazines used to fill to the brim on every
        // arrival, which with ammo burning made the Rearm service something
        // nobody ever needed. Every gun gets back a reserve -- enough to fight
        // your way to the next station -- and the rest is bought.
        let mut rearmed = false;
        for (mut weapon, parent) in weapon_query.iter_mut() {
            if parent.parent() != ship_entity {
                continue;
            }
            let reserve = dock_ammo_reserve(weapon.max_ammo);
            if weapon.ammo < reserve {
                weapon.ammo = reserve;
                rearmed = true;
            }
        }

        // Say what docking actually gave: "fuel resupplied" on every arrival
        // read as a full tank, then the shop asked for credits to fill it.
        let resupplied = match (topped_up, rearmed) {
            (true, true) => "O2, reserve fuel and reserve ammo",
            (true, false) => "O2 and reserve fuel",
            (false, true) => "O2 and reserve ammo",
            (false, false) => "O2",
        };
        notifications.write(ShowNotification {
            message: format!(
                "Docked at {} - {resupplied} resupplied. B: build | U: shop | J: jobs | Enter: launch",
                site.name
            ),
            notification_type: NotificationType::Success,
            duration: 5.0,
        });
        next_state.set(GameState::StationDocked);
    }
}

#[cfg(test)]
mod dock_prompt_tests {
    use super::*;

    #[test]
    fn the_type_is_said_once() {
        assert_eq!(dock_prompt("Vesper Trade Hub", "Trade Hub"), "Vesper Trade Hub in range - press F to dock");
        assert_eq!(dock_prompt("Haven Station", "Shipyard"), "Haven Station (Shipyard) in range - press F to dock");
    }
}

#[cfg(test)]
mod station_size_tests {
    use super::*;

    /// Gap between a ship disc and the nearest part of a station's collision.
    fn clearance(station_pos: Vec2, radius: f32, ship_pos: Vec2, ship_radius: f32) -> f32 {
        station_collider_circles(radius)
            .into_iter()
            .map(|(c, r)| (station_pos + c).distance(ship_pos) - r - ship_radius)
            .fold(f32::INFINITY, f32::min)
    }

    /// A docked ship sits clear of the structure, for small hulls and big
    /// ones, at Haven and at the smaller stations.
    #[test]
    fn a_berthed_ship_clears_the_structure() {
        for index in [0usize, 5] {
            let radius = station_radius(index);
            for ship_radius in [400.0, 1_100.0, 2_500.0] {
                let berth = berth_position(Vec2::ZERO, radius, ship_radius);
                let gap = clearance(Vec2::ZERO, radius, berth, ship_radius);
                assert!(gap > 0.0, "station {index}, ship r={ship_radius}: overlaps by {}", -gap);
            }
        }
    }

    /// The game opens docked at the spawn berth: outside Haven, and close
    /// enough that F docks again without flying anywhere.
    #[test]
    fn the_spawn_berth_is_outside_haven_and_in_reach() {
        assert!(clearance(STATION_POS, HAVEN_RADIUS, SPAWN_BERTH, 1_100.0) > 0.0);
        let haven = StationSite {
            index: 0,
            system_id: 0,
            pos: STATION_POS,
            name: "Haven Station".into(),
            kind: station_type(0),
        };
        assert!(SPAWN_BERTH.distance(STATION_POS) < haven.dock_range());
    }

    /// Every drawn part of the station is solid: points across each solar
    /// panel and radiator fin sit inside the collision shape.
    #[test]
    fn wings_and_radiators_are_solid() {
        let circles = station_collider_circles(1.0);
        let inside = |p: Vec2| circles.iter().any(|(c, r)| c.distance(p) <= *r);
        for at in solar_panel_centers() {
            for dx in [-0.35, 0.0, 0.35] {
                let p = at + Vec2::new(dx * SOLAR_PANEL.x, 0.0);
                assert!(inside(p), "solar panel point {p} is not solid");
            }
        }
        for at in radiator_centers() {
            assert!(inside(at), "radiator {at} is not solid");
        }
    }

    /// The point of the change: a station is several times the starter's
    /// length (~1,800) rather than a fraction of it.
    #[test]
    fn stations_dwarf_the_starter() {
        assert!(station_radius(0) * 2.0 > 1_800.0 * 2.5);
        assert!(station_radius(7) * 2.0 > 1_800.0 * 2.0);
    }

    /// Zones and contract ranges measure from the edge, so berthing at a big
    /// station isn't kilometres "out".
    #[test]
    fn local_range_is_from_the_edge() {
        let stations = SystemStations {
            system_id: Some(0),
            sites: vec![StationSite {
                index: 0,
                system_id: 0,
                pos: Vec2::ZERO,
                name: "Haven Station".into(),
                kind: station_type(0),
            }],
        };
        assert_eq!(stations.local_range(Vec2::new(HAVEN_RADIUS + 500.0, 0.0)), Some(500.0));
        assert_eq!(stations.local_range(Vec2::new(100.0, 0.0)), Some(0.0));
    }
}
