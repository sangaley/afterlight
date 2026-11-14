use bevy::prelude::*;
use crate::states::GameState;
use crate::resources::*;
use crate::events::*;
use crate::components::*;

mod generation;
mod chunks;
mod biomes;
pub mod home_base;
pub mod raft;
pub mod docking;
pub mod raft_art;
pub mod raft_life;
pub mod sections;
pub mod station_style;
pub mod starport_raft;
pub mod station_types;

#[allow(unused_imports)]
pub use generation::*;
pub use chunks::*;
// Biome selection is parked for the demo — see `update_biome` below. The
// table stays compiled (and tested) so turning it back on is a one-line
// change rather than an archaeology exercise.
#[allow(unused_imports)]
pub use biomes::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<WorldState>()
            .init_resource::<ChunkManager>()
            .init_resource::<DiscoveredLocations>()
            .init_resource::<MarketEvents>()
            .init_resource::<home_base::SystemStations>()
            .init_resource::<raft_art::RaftBakes>()
            .init_resource::<raft_life::HavenRoster>()
            .add_systems(Update, raft_life::forget_roster_on_new_run)
            .add_plugins(docking::DockingPlugin)
            .add_systems(Startup, raft_art::make_raft_art)
            // Stations follow whichever system is streamed in, and have to
            // exist while docked too — the game opens docked at Haven.
            .add_systems(OnEnter(GameState::StationDocked), home_base::spawn_base_arrow)
            .add_systems(
                Update,
                (
                    home_base::refresh_system_stations,
                    home_base::sync_station_entities,
                    home_base::pulse_station_beacons,
                    raft_art::apply_raft_bakes,
                    raft_life::raft_roster,
                    raft_life::raft_motion,
                    raft_life::raft_districts,
                    home_base::park_at_station,
                )
                    .chain()
                    .run_if(in_state(GameState::Exploring).or_else(in_state(GameState::StationDocked))),
            )
            .add_systems(
                Update,
                (
                    update_chunks,
                    check_depth_zone_change,
                    // update_biome is OFF for the demo — see the function.
                    tick_market_events,
                    // Both claim the shared F press (resources::InteractPress),
                    // so they must sit behind the salvage handler: crew on the
                    // hull, and a wreck nearer than the station, outrank
                    // docking. Without the ordering the winner would be
                    // whichever Bevy happened to schedule first.
                    check_docking_proximity
                        .after(crate::crew::eva_salvage::order_salvage_detail),
                    home_base::station_docking
                        .after(crate::crew::eva_salvage::order_salvage_detail),
                    home_base::update_base_arrow,
                    apply_hazard_damage,
                )
                    .run_if(in_state(GameState::Exploring)),
            )
            // Must run AFTER transform propagation. In Update, a point of
            // interest spawned this frame still has the default GlobalTransform
            // — the world origin — so every log-bearing wreck in the galaxy
            // read as sitting 50 units from the ship and was "discovered" on
            // the frame it spawned. That is why the player was handed a log
            // before touching a control, and it had nothing to do with where
            // the wreck actually was.
            .add_systems(
                PostUpdate,
                // Both of these compare GlobalTransform, so both must run
                // after propagation. check_poi_discovery was left behind in
                // Update when discover_log_entries was moved, and had exactly
                // the same bug: a point of interest spawned this frame still
                // carries the default GlobalTransform — the world origin —
                // and the ship starts 50 units from it, so every streamed POI
                // read as adjacent and was instantly "discovered". That meant
                // toast spam and contract objectives completing on their own.
                (discover_log_entries, check_poi_discovery)
                    .after(bevy::transform::TransformSystems::Propagate)
                    .run_if(in_state(GameState::Exploring)),
            );
    }
}


/// How far the ship is from the nearest berth in the system it is standing in.
///
/// The honest answer to "how far out am I", and deliberately separate from
/// `DepthState.current_depth`, which measures distance from the world origin
/// and is what the depth vignette, the HUD readout and the camera are all
/// calibrated against. Redefining that field broke rendering outright.
///
/// The origin measure stopped meaning anything when the galaxy arrived: every
/// system except Haven centres hundreds of thousands of units away in a shared
/// local space, so anything keyed on it saturates the moment the player warps
/// anywhere. That is what auto-completed ReachDepth contracts, pinned zone
/// selection, and put creature density and spawn rate at maximum everywhere
/// outside Haven.
pub fn distance_from_safety(ship_pos: Vec2, stations: &home_base::SystemStations) -> f32 {
    let nearest = stations
        .sites
        .iter()
        .map(|s| (s.pos.distance(ship_pos) - s.radius()).max(0.0))
        .fold(f32::INFINITY, f32::min);
    if nearest.is_finite() {
        nearest
    } else {
        // Blind-warped into empty space: nowhere to dock, so treat it as a
        // long way from anywhere rather than as zero.
        f32::MAX
    }
}

/// Checks if player entered a new depth zone
/// Seconds a new zone must hold before it's announced. Hovering on a ring
/// boundary -- which is exactly where a ship parked near a station sits --
/// announced every crossing: 35 "Entering Asteroid Belt" in one 21-minute run.
const ZONE_SETTLE_SECONDS: f32 = 2.5;

/// The name a range band is shown under: HUD, zone notices, contract titles.
///
/// These describe distance from the nearest station, honestly. The old names
/// were depth zones from the submarine days and promised things that weren't
/// there: "Asteroid Belt" three km from a station whose rocks are kept twelve
/// km clear, "Nebula" with no nebula, and "Black Hole Proximity" anywhere
/// over 30 km out -- which, chasing a bounty, was most of a system. The
/// ZoneType variants keep their old identifiers; gameplay reads those.
pub fn zone_display_name(zone: ZoneType) -> &'static str {
    match zone {
        ZoneType::NearOrbit => "Near Space",
        ZoneType::AsteroidBelt => "Outer Approach",
        ZoneType::DeepSpace => "Deep Space",
        ZoneType::Nebula => "Far Reaches",
        ZoneType::BlackHole => "The Outer Dark",
    }
}

/// The zone event stays immediate and Haven-relative (spawn tables and
/// contracts read it). The *announcement* is measured from the local station
/// (see SystemStations::local_range) and waits for the zone to settle.
fn check_depth_zone_change(
    ship_state: Res<DepthState>,
    time: Res<Time>,
    stations: Res<home_base::SystemStations>,
    ship: Query<&Transform, With<Ship>>,
    mut last_zone: Local<Option<crate::components::ZoneType>>,
    mut announced: Local<Option<crate::components::ZoneType>>,
    mut settling: Local<(Option<crate::components::ZoneType>, f32)>,
    mut zone_events: MessageWriter<DepthZoneChanged>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let current_zone = depth_to_zone(ship_state.current_depth);
    if Some(current_zone) != *last_zone {
        *last_zone = Some(current_zone);
        zone_events.write(DepthZoneChanged {
            new_depth: ship_state.current_depth,
            new_zone: current_zone,
        });
    }

    let local = ship
        .single()
        .ok()
        .and_then(|t| stations.local_range(t.translation.truncate()))
        .unwrap_or(ship_state.current_depth);
    let candidate = zone_with_margin(local, *announced);
    if let Some(zone) = settled_zone(candidate, time.delta_secs(), &mut settling, &mut announced) {
        notifications.write(ShowNotification {
            message: format!("Entering {}", zone_display_name(zone)),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
    }
}

/// The zone at `range`, unless the ship is within 12% of a boundary -- then
/// whichever zone was last announced. Time alone wasn't enough: a ship holding
/// station near a ring sat more than the settle time on each side of it in
/// turn and still announced every crossing.
fn zone_with_margin(range: f32, announced: Option<ZoneType>) -> ZoneType {
    let (inner, outer) = (depth_to_zone(range * 0.88), depth_to_zone(range * 1.12));
    if inner == outer {
        inner
    } else {
        announced.unwrap_or_else(|| depth_to_zone(range))
    }
}

/// Feeds one frame's zone in; returns a zone to announce once it has held for
/// ZONE_SETTLE_SECONDS and differs from the last one announced. The very
/// first zone of a run is recorded silently -- launching into Near Orbit is
/// not news.
fn settled_zone(
    zone: ZoneType,
    dt: f32,
    settling: &mut (Option<ZoneType>, f32),
    announced: &mut Option<ZoneType>,
) -> Option<ZoneType> {
    if announced.is_none() {
        *announced = Some(zone);
        *settling = (Some(zone), 0.0);
        return None;
    }
    if settling.0 != Some(zone) {
        *settling = (Some(zone), 0.0);
    } else {
        settling.1 += dt;
    }
    if *announced != Some(zone) && settling.1 >= ZONE_SETTLE_SECONDS {
        *announced = Some(zone);
        return Some(zone);
    }
    None
}

/// The zone a given range corresponds to. One function, on purpose.
///
/// `contracts::tracking` carried a private copy of this that still used the
/// old 200/500/1000/2000 thresholds, so the contract board and the HUD
/// disagreed about where the player was standing: at 2,100 units the HUD said
/// Near Space and a Survey contract said Black Hole Proximity, and a five-star
/// survey completed a few seconds after undocking.
pub fn depth_to_zone(depth: f32) -> crate::components::ZoneType {
    use crate::components::ZoneType;
    // Radial distance from Haven Station (origin). Thresholds sized for the
    // current cruise speeds — the old 200/500/1000/2000 were submarine depths
    // that a ship at full burn now crosses in a couple of seconds.
    match depth {
        d if d < 3000.0 => ZoneType::NearOrbit,
        d if d < 8000.0 => ZoneType::AsteroidBelt,
        d if d < 16000.0 => ZoneType::DeepSpace,
        d if d < 30000.0 => ZoneType::Nebula,
        _ => ZoneType::BlackHole,
    }
}

/// Updates current biome based on ship position. PARKED for the demo.
///
/// Two things were wrong with it and the second is the fatal one.
///
/// The bands are 200, 500, 1000 and 2000 units from the nearest berth, and a
/// ship crosses all five in a few seconds of ordinary flight — so it fired
/// "Entered DeadZone biome" toasts continuously while you were still in sight
/// of the station.
///
/// And nothing reads the result. `WorldState.current_biome` is written here
/// and the only other mention of it in the whole tree is its default. The one
/// consumer, `biome_creature_weights`, is called from `src/parked/creatures`,
/// which is not compiled — there is no `mod parked` anywhere. So the system
/// cost a notification every few seconds and changed nothing about the game.
///
/// To bring it back: give the bands a scale that matches how far a ship
/// actually travels (the system's own `danger_tier` is the authored
/// near-weak/far-strong curve and is the natural input), and wire a consumer
/// that does something with the biome before the toast is worth showing.
#[allow(dead_code)]
fn update_biome(
    ship_query: Query<&Transform, With<Ship>>,
    stations: Res<home_base::SystemStations>,
    mut world_state: ResMut<WorldState>,
    mut notifications: MessageWriter<ShowNotification>,
    mut last_biome: Local<Option<BiomeType>>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };

    let x = ship_transform.translation.x;
    // Distance from the nearest berth, not from the world origin — otherwise
    // this reads >180,000 in every system but Haven and pins the biome to the
    // most hostile band everywhere.
    let depth = distance_from_safety(ship_transform.translation.truncate(), &stations);

    // Determine biome from position and depth
    let biome = match depth {
        d if d < 200.0 => {
            if x.abs() > 2000.0 { BiomeType::AsteroidField } else { BiomeType::OpenVoid }
        }
        d if d < 500.0 => {
            if x > 1500.0 { BiomeType::CrystalFormation } else { BiomeType::OpenVoid }
        }
        d if d < 1000.0 => {
            if x < -1500.0 { BiomeType::IceShells } else { BiomeType::ThermalVents }
        }
        d if d < 2000.0 => BiomeType::DeadZone,
        _ => BiomeType::VoidRift,
    };

    if world_state.current_biome != biome {
        world_state.current_biome = biome;

        if last_biome.is_some() {
            notifications.write(ShowNotification {
                message: format!("Entered {:?} biome", biome),
                notification_type: NotificationType::Info,
                duration: 3.0,
            });
        }
        *last_biome = Some(biome);
    }
}

/// Discovers POIs when ship gets close
/// Chunks stream out and back in, and each time their POIs are rebuilt from
/// the chunk's seed as fresh entities with `discovered: false` -- so every lap
/// past the same wreck announced it again, and logged it again. The seed puts
/// a rebuilt POI exactly where it was, which is how one already logged is
/// recognised.
fn same_spot(a: Vec2, b: Vec2) -> bool {
    a.distance_squared(b) < 1.0
}

/// At most one discovery toast in this many seconds; whatever else turns up
/// meanwhile goes into one "N more sites" line when it's up.
///
/// The band of chunk sites south of Haven is dense and discovery fires within
/// 700 of a ship over 1,500 long, so crossing it at speed logged fourteen
/// sites in four seconds -- fourteen toasts, mostly for things still under
/// the hull, and the story's own notices pushed off the stack.
const DISCOVERY_TOAST_EVERY: f32 = 3.0;

/// Discoveries waiting for the toast window to close.
#[derive(Default)]
struct HeldDiscoveries {
    names: Vec<String>,
    window_until: f32,
}

impl HeldDiscoveries {
    /// One toast now if the window is shut, else hold `name` for the batch.
    fn announce(&mut self, now: f32, message: String, name: String) -> Option<String> {
        if now >= self.window_until {
            self.window_until = now + DISCOVERY_TOAST_EVERY;
            Some(message)
        } else {
            self.names.push(name);
            None
        }
    }

    /// The batched line, once the window has closed on anything held.
    fn flush(&mut self, now: f32) -> Option<String> {
        if self.names.is_empty() || now < self.window_until {
            return None;
        }
        self.window_until = now + DISCOVERY_TOAST_EVERY;
        let names = std::mem::take(&mut self.names);
        Some(match names.as_slice() {
            [one] => format!("Discovered {one}!"),
            [a, b] => format!("Discovered 2 more sites: {a}, {b}"),
            [a, b, c] => format!("Discovered 3 more sites: {a}, {b}, {c}"),
            [a, b, c, rest @ ..] => format!(
                "Discovered {} more sites: {a}, {b}, {c} and {} others (M: map)",
                names.len(),
                rest.len()
            ),
            [] => unreachable!(),
        })
    }
}

fn check_poi_discovery(
    time: Res<Time>,
    mut held: Local<HeldDiscoveries>,
    ship_query: Query<&GlobalTransform, With<Ship>>,
    mut poi_query: Query<(&GlobalTransform, &mut PointOfInterest)>,
    // The celestial layer, which is what exists outside Haven. Deliberately
    // fires the same PoiDiscovered event rather than being given a
    // PointOfInterest component: that component is also what radar draws, and
    // making every derelict in the galaxy show up on the sweep is a separate
    // decision that has not been taken.
    mut space_query: Query<(&GlobalTransform, &mut crate::celestial::poi::SpacePoi)>,
    mut discovered: ResMut<DiscoveredLocations>,
    mut poi_events: MessageWriter<PoiDiscovered>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let now = time.elapsed_secs();
    if let Some(message) = held.flush(now) {
        notifications.write(ShowNotification {
            message,
            notification_type: NotificationType::Success,
            duration: 4.0,
        });
    }
    let Ok(ship_gt) = ship_query.single() else { return };
    let ship_pos = ship_gt.translation().truncate();

    for (gt, mut sp) in space_query.iter_mut() {
        if sp.discovered {
            continue;
        }
        let Some(kind) = sp.poi_type.as_poi_type() else { continue };
        let pos = gt.translation().truncate();
        if ship_pos.distance(pos) >= 700.0 {
            continue;
        }
        sp.discovered = true;
        if discovered.special.iter().any(|(p, _)| same_spot(*p, pos)) {
            continue;
        }
        discovered.special.push((pos, sp.name.clone()));
        poi_events.write(PoiDiscovered { poi_type: kind, position: pos });
        if let Some(message) = held.announce(now, format!("Contact logged: {}", sp.name), sp.name.clone()) {
            notifications.write(ShowNotification {
                message,
                notification_type: NotificationType::Info,
                duration: 4.0,
            });
        }
    }

    for (poi_gt, mut poi) in poi_query.iter_mut() {
        if poi.discovered {
            continue;
        }

        let poi_pos = poi_gt.translation().truncate();
        let dist = ship_pos.distance(poi_pos);

        // Root-to-center; the ship's own hull spans several hundred units,
        // so discovery triggers as the hull gets near, not once the root
        // is parked on top of the POI.
        if dist < 700.0 {
            poi.discovered = true;

            let logged = match poi.poi_type {
                PoiType::Wreck => discovered.wrecks.iter().any(|p| same_spot(*p, poi_pos)),
                PoiType::Cave => discovered.caves.iter().any(|p| same_spot(*p, poi_pos)),
                PoiType::Settlement => discovered.settlements.iter().any(|p| same_spot(*p, poi_pos)),
                _ => discovered.special.iter().any(|(p, _)| same_spot(*p, poi_pos)),
            };
            if logged {
                continue;
            }

            match poi.poi_type {
                PoiType::Wreck => discovered.wrecks.push(poi_pos),
                PoiType::Cave => discovered.caves.push(poi_pos),
                PoiType::Settlement => discovered.settlements.push(poi_pos),
                _ => discovered.special.push((poi_pos, format!("{:?}", poi.poi_type))),
            }

            poi_events.write(PoiDiscovered {
                poi_type: poi.poi_type,
                position: poi_pos,
            });

            let name = poi.poi_type.display_name();
            if let Some(message) = held.announce(now, format!("Discovered {name}!"), name.to_string()) {
                notifications.write(ShowNotification {
                    message,
                    notification_type: NotificationType::Success,
                    duration: 3.0,
                });
            }
        }
    }
}

/// Check for docking proximity to settlements
fn check_docking_proximity(
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut press: ResMut<crate::resources::InteractPress>,
    ship_query: Query<&GlobalTransform, With<Ship>>,
    poi_query: Query<(Entity, &GlobalTransform, &PointOfInterest)>,
    mut docking_events: MessageWriter<DockingStarted>,
    mut notifications: MessageWriter<ShowNotification>,
    mut next_state: ResMut<NextState<GameState>>,
    stations: Res<home_base::SystemStations>,
) {
    if !press.pending() {
        return;
    }

    let Ok(ship_gt) = ship_query.single() else { return };
    let ship_pos = ship_gt.translation().truncate();

    // A station in range has already said "press F to dock" and this has
    // said nothing, so the press is the station's. Without this the two
    // raced for it and a settlement beside Haven kept winning: F docked at
    // an outpost colony, and again on every try.
    if stations.nearest_in_range(ship_pos).is_some() {
        return;
    }

    for (entity, poi_gt, poi) in poi_query.iter() {
        if poi.poi_type != PoiType::Settlement {
            continue;
        }

        let dist = ship_pos.distance(poi_gt.translation().truncate());
        if dist < 900.0 {
            // Claim only now that we know we're actually docking here.
            if !press.claim() {
                return;
            }
            crate::vfx::particles::spawn_dock_pulse(&mut commands, &fx, ship_pos, 180.0);
            docking_events.write(DockingStarted { target: entity });
            notifications.write(ShowNotification {
                message: "Docking at settlement...".into(),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
            next_state.set(GameState::Docked);
            return;
        }
    }
}

// NOTE: wreck looting moved to crew::eva_salvage — F now dispatches a
// crew salvage detail instead of teleporting loot into the hold.

/// MARKET EVENTS — every few minutes an outpost may develop a shortage
/// and pay well over the odds for one good for a while. Prices resolve
/// through resources::live_item_price, so the docking menu reflects the
/// premium automatically; expiry is silent (the notification names the
/// duration up front).
fn tick_market_events(
    time: Res<Time>,
    mut events: ResMut<MarketEvents>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    use rand::Rng;
    let dt = time.delta_secs();
    events.active.retain_mut(|e| {
        e.remaining -= dt;
        e.remaining > 0.0
    });

    events.next_roll -= dt;
    if events.next_roll > 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    events.next_roll = rng.gen_range(180.0..300.0);
    if events.active.len() >= 2 {
        return;
    }

    let station_idx = rng.gen_range(1..=12usize);
    let goods = [
        ItemType::ScrapMetal,
        ItemType::Crystal,
        ItemType::BioSample,
        ItemType::FuelCell,
        ItemType::RareAlloy,
        ItemType::AncientArtifact,
        ItemType::AmmoCrate,
    ];
    let item = goods[rng.gen_range(0..goods.len())];
    let sell_mult = rng.gen_range(1.6..2.0_f32);
    let remaining = rng.gen_range(300.0..480.0_f32);

    // By name -- this printed "Outpost 8 (Refuel Depot)", the station's raw
    // index. station_display_name already includes the station type.
    notifications.write(ShowNotification {
        message: format!(
            "MARKET: {} short on {} - paying {:.0}% for ~{:.0} min!",
            home_base::station_display_name(station_idx),
            item.name(),
            sell_mult * 100.0,
            remaining / 60.0
        ),
        notification_type: NotificationType::Info,
        duration: 6.0,
    });
    events.active.push(MarketEvent { station_idx, item, sell_mult, remaining });
}

/// Discover log entries when near POIs that have them
/// How close the ship's ROOT must get to a record to read it. Was 500, which
/// suited the chunk layer's flat, non-solid wrecks. Records live on system
/// derelicts now, which are solid, and a big hull's root sits ~1,100 units
/// behind its nose -- at 500 no starter-sized ship could ever read one. Same
/// reach as stripping a derelict with E (celestial::poi).
const LOG_PICKUP_RANGE: f32 = 1_400.0;

fn discover_log_entries(
    ship_query: Query<&GlobalTransform, With<Ship>>,
    // Deliberately does NOT require PointOfInterest. That component only
    // exists on the chunk layer, which generates in a narrow band of world Y
    // around the origin — so requiring it meant a log could only ever be read
    // near Haven, and celestial derelicts out in the galaxy were invisible to
    // this system no matter what they carried.
    log_query: Query<(&GlobalTransform, &LogEntry), Without<Ship>>,
    mut statistics: ResMut<Statistics>,
    mut log_queue: ResMut<crate::narrative::reader::LogQueue>,
    mut finale: ResMut<crate::narrative::FinaleFound>,
    mut read: MessageWriter<crate::narrative::leads::LogRead>,
) {
    let Ok(ship_gt) = ship_query.single() else { return };
    let ship_pos = ship_gt.translation().truncate();

    for (poi_gt, log) in log_query.iter() {
        let poi_pos = poi_gt.translation().truncate();
        let dist = ship_pos.distance(poi_pos);

        // Statistics.logs_found is the only record of what has been read.
        //
        // This used to dedupe against a system `Local` as well, which was a
        // quiet disaster: a Local outlives the run, and reset_for_new_game has
        // no way to clear one. So after a single New Expedition every log read
        // in the previous run was permanently unreadable — the finale
        // included, which made the ending unreachable on any second run until
        // the process was restarted. logs_found IS reset, so it is the right
        // and only source of truth.
        if dist < LOG_PICKUP_RANGE && !statistics.logs_found.contains(&log.title) {
            statistics.logs_found.push(log.title.clone());

            // The ending keys on *finding* the finale, not on holding it, so
            // that loading a save from after the ending does not replay it.
            if log.title == crate::narrative::logs::FINALE_TITLE {
                finale.0 = true;
            }
            // Goes to the reader, not a toast. Several entries run past two
            // hundred characters and the toast is 340px wide with an eight
            // second life — they were unreadable by construction.
            log_queue.push(log.title.clone(), log.text.clone());
            // What it does (narrative::leads): its closing line, and a mark,
            // a cache, a patch or a glitch.
            read.write(crate::narrative::leads::LogRead(log.title.clone()));
        }
    }
}

/// Applies damage and forces from environmental hazard zones
fn apply_hazard_damage(
    time: Res<Time>,
    ship_query: Query<&GlobalTransform, With<Ship>>,
    hazard_query: Query<(&GlobalTransform, &HazardZone)>,
    mut damage_events: MessageWriter<ShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
    mut warned_thermal: Local<bool>,
    mut warned_current: Local<bool>,
) {
    let Ok(ship_gt) = ship_query.single() else { return };
    let ship_pos = ship_gt.translation().truncate();

    for (hazard_gt, hazard) in hazard_query.iter() {
        let hazard_pos = hazard_gt.translation().truncate();
        let dist = ship_pos.distance(hazard_pos);

        if dist > hazard.radius {
            continue;
        }

        match &hazard.hazard_type {
            HazardType::ThermalVent => {
                let damage = hazard.damage_per_second * time.delta_secs();
                if damage > 0.01 {
                    damage_events.write(ShipDamaged {
                        source: DamageSource::Fire,
                        amount: damage,
                        position: Some(hazard_pos),
                        direction: Some((hazard_pos - ship_pos).normalize_or_zero()),
                        applied: false,
                    });
                }

                if !*warned_thermal {
                    *warned_thermal = true;
                    notifications.write(ShowNotification {
                        message: "Thermal vent! Hull taking heat damage!".into(),
                        notification_type: NotificationType::Danger,
                        duration: 3.0,
                    });
                }
            }
            HazardType::StrongCurrent(_direction) => {
                // Strong currents don't damage, they apply force
                // (Movement would be affected externally; for now just warn)
                if !*warned_current {
                    *warned_current = true;
                    notifications.write(ShowNotification {
                        message: "Strong current! Navigation affected!".into(),
                        notification_type: NotificationType::Warning,
                        duration: 3.0,
                    });
                }
            }
        }
    }

    // Reset warnings when ship moves away from all hazards
    let near_any = hazard_query.iter().any(|(gt, hz)| {
        ship_pos.distance(gt.translation().truncate()) < hz.radius
    });
    if !near_any {
        *warned_thermal = false;
        *warned_current = false;
    }
}

#[cfg(test)]
mod rediscovery_tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<DiscoveredLocations>();
        app.add_message::<PoiDiscovered>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, check_poi_discovery);
        app.world_mut().spawn((Ship, GlobalTransform::from_xyz(0.0, 0.0, 0.0)));
        app
    }

    fn wreck_at(app: &mut App, pos: Vec2) -> Entity {
        app.world_mut()
            .spawn((
                GlobalTransform::from_xyz(pos.x, pos.y, 0.0),
                PointOfInterest { poi_type: PoiType::Wreck, discovered: false },
            ))
            .id()
    }

    fn announcements(app: &App) -> usize {
        app.world().resource::<Messages<ShowNotification>>().iter_current_update_messages().count()
    }

    /// A chunk that streams back in rebuilds its wreck as a new entity on
    /// the same spot. It is the same wreck: no second announcement, no
    /// second entry in the log.
    #[test]
    fn a_wreck_rebuilt_on_the_same_spot_is_not_rediscovered() {
        let mut app = app();
        let first = wreck_at(&mut app, Vec2::new(300.0, 0.0));
        app.update();
        assert_eq!(announcements(&app), 1);

        app.world_mut().despawn(first);
        let again = wreck_at(&mut app, Vec2::new(300.0, 0.0));
        app.update();
        assert_eq!(announcements(&app), 0, "the same wreck was announced twice");
        assert!(app.world().get::<PointOfInterest>(again).unwrap().discovered);
        assert_eq!(app.world().resource::<DiscoveredLocations>().wrecks.len(), 1);
    }

    fn wait(app: &mut App, secs: f32) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(secs));
    }

    fn messages(app: &App) -> Vec<String> {
        app.world()
            .resource::<Messages<ShowNotification>>()
            .iter_current_update_messages()
            .map(|n| n.message.clone())
            .collect()
    }

    /// Logged at once; announced once the toast window from the first one
    /// has closed.
    #[test]
    fn a_different_wreck_nearby_still_is() {
        let mut app = app();
        wreck_at(&mut app, Vec2::new(300.0, 0.0));
        app.update();
        wreck_at(&mut app, Vec2::new(-250.0, 100.0));
        app.update();
        assert_eq!(app.world().resource::<DiscoveredLocations>().wrecks.len(), 2);
        wait(&mut app, DISCOVERY_TOAST_EVERY + 0.1);
        app.update();
        assert_eq!(messages(&app), vec!["Discovered Drifting Wreck!".to_string()]);
    }

    /// Crossing a dense band: the first find gets its toast, the rest one
    /// line between them, not a toast each.
    #[test]
    fn a_burst_of_finds_is_two_toasts_not_six() {
        let mut app = app();
        for k in 0..6 {
            wreck_at(&mut app, Vec2::new(100.0 * k as f32 - 250.0, 50.0));
        }
        app.update();
        assert_eq!(announcements(&app), 1);
        assert_eq!(app.world().resource::<DiscoveredLocations>().wrecks.len(), 6);
        wait(&mut app, 1.0);
        app.update();
        assert_eq!(announcements(&app), 0, "a toast inside the window");
        wait(&mut app, DISCOVERY_TOAST_EVERY);
        app.update();
        let batch = messages(&app);
        assert_eq!(batch.len(), 1);
        assert!(batch[0].starts_with("Discovered 5 more sites"), "{}", batch[0]);
    }
}

#[cfg(test)]
mod settlement_dock_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use crate::resources::InteractPress;
    use home_base::{StationSite, SystemStations};

    fn app(station_beside: bool) -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.insert_state(GameState::Exploring);
        app.insert_resource(crate::vfx::effect_textures::EffectTextures::empty());
        app.init_resource::<InteractPress>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_message::<DockingStarted>();
        app.add_message::<ShowNotification>();
        let mut stations = SystemStations::default();
        if station_beside {
            stations.sites.push(StationSite {
                index: 0,
                system_id: 0,
                pos: Vec2::new(-600.0, 0.0),
                name: "Haven Station".into(),
                kind: crate::world::station_types::station_type(0),
            });
        }
        app.insert_resource(stations);
        app.add_systems(Update, (crate::resources::refresh_interact_press, check_docking_proximity).chain());
        app.world_mut().spawn((Ship, GlobalTransform::default()));
        app.world_mut().spawn((
            GlobalTransform::from_xyz(400.0, 0.0, 0.0),
            PointOfInterest { poi_type: PoiType::Settlement, discovered: true },
        ));
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyF);
        app
    }

    fn docked_at_settlement(app: &mut App) -> bool {
        app.update();
        matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Docked)
        )
    }

    #[test]
    fn f_docks_at_a_settlement_on_its_own() {
        assert!(docked_at_settlement(&mut app(false)));
    }

    /// The station's prompt is the one on screen; the press is its.
    #[test]
    fn a_station_in_range_keeps_the_press() {
        let mut app = app(true);
        assert!(!docked_at_settlement(&mut app));
        assert!(app.world().resource::<InteractPress>().pending(), "the press was swallowed");
    }
}

#[cfg(test)]
mod zone_notice_tests {
    use super::*;

    fn run(frames: &[(ZoneType, f32)]) -> Vec<ZoneType> {
        let (mut settling, mut announced) = ((None, 0.0), None);
        frames.iter().filter_map(|(z, dt)| settled_zone(*z, *dt, &mut settling, &mut announced)).collect()
    }

    /// Wobbling across a ring for ten seconds announces nothing.
    #[test]
    fn hovering_on_a_boundary_is_quiet() {
        let mut frames = vec![(ZoneType::NearOrbit, 0.1)];
        for i in 0..100 {
            let z = if (i / 10) % 2 == 0 { ZoneType::AsteroidBelt } else { ZoneType::NearOrbit };
            frames.push((z, 0.1));
        }
        assert!(run(&frames).is_empty());
    }

    /// Within 12% of the 3,000 ring the last announced zone stands.
    #[test]
    fn near_a_ring_the_announced_zone_stands() {
        assert_eq!(zone_with_margin(3_100.0, Some(ZoneType::NearOrbit)), ZoneType::NearOrbit);
        assert_eq!(zone_with_margin(2_900.0, Some(ZoneType::AsteroidBelt)), ZoneType::AsteroidBelt);
        assert_eq!(zone_with_margin(3_600.0, Some(ZoneType::NearOrbit)), ZoneType::AsteroidBelt);
    }

    /// Crossing and staying is announced, once.
    #[test]
    fn a_real_crossing_is_announced_once() {
        let mut frames = vec![(ZoneType::NearOrbit, 0.1)];
        frames.extend(std::iter::repeat((ZoneType::AsteroidBelt, 0.1)).take(100));
        assert_eq!(run(&frames), vec![ZoneType::AsteroidBelt]);
    }
}
