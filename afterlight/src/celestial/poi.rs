use bevy::prelude::*;
use rand::Rng;
use crate::components::*;
use crate::events::*;
use crate::resources::*;
use super::components::*;

// ============================================================================
// SPACE POINTS OF INTEREST
// Derelicts, anomalies, resource nodes, space stations.
// Spawned per star system during generation.
// ============================================================================

/// Types of space POIs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpacePoiType {
    DerelictShip,       // Lootable wreck
    AsteroidNode,       // Mineable resource deposit
    Anomaly,            // Strange readings — story trigger
    SpaceStation,       // Trading outpost
    DebrisField,        // Scattered salvage
    SignalSource,       // Distress signal or trap
}

impl SpacePoiType {
    /// The chunk-layer equivalent, for anything that speaks in `PoiType`.
    ///
    /// The two point-of-interest systems grew up separately and never met:
    /// contracts, discovery and the map all speak `PoiType`, which only ever
    /// existed on the chunk layer near the world origin, while `SpacePoi` is
    /// what actually exists in all thirty-one systems. So Explore contracts
    /// could only be completed within a few thousand units of Haven, and the
    /// expedition trail sends people the other way.
    pub fn as_poi_type(self) -> Option<PoiType> {
        match self {
            SpacePoiType::DerelictShip | SpacePoiType::DebrisField => Some(PoiType::Wreck),
            SpacePoiType::Anomaly | SpacePoiType::SignalSource => Some(PoiType::Ruins),
            SpacePoiType::SpaceStation => Some(PoiType::Settlement),
            // A rock is not a place.
            SpacePoiType::AsteroidNode => None,
        }
    }
}

/// Component marking a space POI
#[derive(Component)]
pub struct SpacePoi {
    pub poi_type: SpacePoiType,
    pub looted: bool,
    /// Whether the ship has been close enough to log it.
    ///
    /// Separate from `looted` on purpose: that one gates whether there is
    /// anything left to take, and reusing it to mean "seen" would have made
    /// every derelict in the galaxy unlootable the moment you flew past it.
    pub discovered: bool,
    pub name: String,
    pub loot_value: u32,
}

/// Which lootable this is in its system's spawn order: derelicts first, then
/// the anomaly. The spawn order is fixed by the system's seed, so the same
/// hulk gets the same slot on every visit -- which is what lets
/// `LootedPois` remember it was stripped once its entity is long gone.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoiSlot(pub u32);

/// Component for mineable asteroids
#[derive(Component)]
pub struct MineableResource {
    pub resource_remaining: f32,
    pub resource_type: ResourceNodeType,
    pub extraction_rate: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum ResourceNodeType {
    MetalOre,
    RareCrystal,
    FuelDeposit,
    ExoticMatter,
}

/// How far from the spawn berth Haven's first record waits: past the asteroid
/// rocks and the station's neighbourhood, a short flight once training ends.
const FIRST_RECORD_FROM_SPAWN: f32 = 18_000.0;

/// How far from a system's station its records wait. A jump drops you at the
/// station, so the record is a short hop from where you turn up -- the
/// tracker's "next record: 24 km" is the whole errand, not a trek across a
/// system half a million units wide on a tank that has to get you home.
const RECORD_FROM_STATION: std::ops::Range<f32> = 15_000.0..35_000.0;

/// Bearings tried round the station before a record settles for the nearest
/// clear point outward. Stepped by the golden angle so no two repeat.
const RECORD_BEARINGS: usize = 64;
const GOLDEN_ANGLE: f32 = 2.399_963;

/// How close to a star's centre a point of interest may sit, in star radii.
/// At one and a half radii the pull is under half the surface's 110-145
/// u/s² -- at most ~64, a third of the starter's thrust: enough to warn a
/// pilot, never enough to trap one. Not two radii: some stations sit closer
/// in than that (Ostra's is at 1.6), and a record by the station has to be
/// allowed where the station itself is.
const STAR_KEEP_OUT_RADII: f32 = 1.5;

/// How far `KeepClear::settle` moves a blocked point outward per try.
const SETTLE_STEP: f32 = 2_000.0;

/// Where in a system nothing solid ever reaches.
///
/// Points of interest used to be dropped 30-100 km from the star's centre,
/// which was open space when stars were small. Stars are solid now and 40-150
/// km in radius, and in the seed-42 galaxy 12 of the 24 placed expedition
/// records lay inside their sun, 11 more in the crushing pull just above it,
/// and every waystation in the galaxy was inside its star. A playtest jumped
/// to Vesper for its record and spent the whole tank dashing at it: each dash
/// landed in the star and the star pushed the ship back out.
///
/// Planets are solid too and sweep their whole path every few minutes, so a
/// point also keeps a planet's radius clear of every planet's path.
pub struct KeepClear<'a> {
    pub center: Vec2,
    pub star_radius: f32,
    pub planets: &'a [super::orbits::PlanetPath],
}

impl KeepClear<'_> {
    fn floor(&self) -> f32 {
        self.star_radius * STAR_KEEP_OUT_RADII
    }

    /// Whether nothing solid ever comes near `pos`.
    pub fn is_clear(&self, pos: Vec2) -> bool {
        self.is_clear_by(pos, 0.0)
    }

    /// Whether nothing solid ever comes near anything within `margin` of
    /// `pos`: the same rule for something with a size of its own, like a
    /// station and the docking range round it.
    pub fn is_clear_by(&self, pos: Vec2, margin: f32) -> bool {
        let offset = pos - self.center;
        offset.length() >= self.floor() + margin
            && self.planets.iter().all(|p| p.clears(offset, p.radius + margin))
    }

    /// `pos` if it's clear, else the first clear point further out from the
    /// star on the same bearing. Past every planet's furthest reach is
    /// always clear, so this always lands somewhere.
    pub fn settle(&self, pos: Vec2) -> Vec2 {
        let offset = pos - self.center;
        let dir = offset.normalize_or(Vec2::X);
        let beyond = self
            .planets
            .iter()
            .map(|p| p.outer_reach() + p.radius)
            .fold(self.floor(), f32::max);
        let mut dist = offset.length().max(self.floor());
        while dist < beyond {
            let at = self.center + dir * dist;
            if self.is_clear(at) {
                return at;
            }
            dist += SETTLE_STEP;
        }
        self.center + dir * beyond
    }

    /// The clear point nearest `pos` on its own bearing from the star,
    /// keeping everything within `margin` of it clear as well.
    ///
    /// Unlike `settle` this looks inward as well as out and takes whichever
    /// is closer. A station that starts a few km inside a planet's path
    /// should step off it, not be sent out past every world in the system.
    pub fn nudge(&self, pos: Vec2, margin: f32) -> Vec2 {
        let offset = pos - self.center;
        let dir = offset.normalize_or(Vec2::X);
        let from_star = offset.length();
        let inner = self.floor() + margin;
        // A radius and the margin past every planet's furthest reach is
        // always clear, so the outward search has an end.
        let beyond = self
            .planets
            .iter()
            .map(|p| p.outer_reach() + p.radius + margin)
            .fold(inner, f32::max);
        let mut step = 0.0;
        while from_star + step < beyond {
            // Outward first, so a tie goes to the side the star pulls less.
            for dist in [from_star + step, from_star - step] {
                let at = self.center + dir * dist;
                if dist >= inner && self.is_clear_by(at, margin) {
                    return at;
                }
            }
            step += SETTLE_STEP;
        }
        self.center + dir * beyond.max(from_star)
    }

    /// Where a record waits near the station at `station`: `reach` out on
    /// `bearing` if that's clear, else the first clear bearing round it,
    /// widening a little each lap.
    fn near_station(&self, station: Vec2, bearing: f32, reach: f32) -> Vec2 {
        for k in 0..RECORD_BEARINGS {
            let lap = (k / 16) as f32;
            let at = station
                + Vec2::from_angle(bearing + k as f32 * GOLDEN_ANGLE) * (reach + lap * 10_000.0);
            if self.is_clear(at) {
                return at;
            }
        }
        self.settle(station + Vec2::from_angle(bearing) * reach)
    }
}

/// Spawn POIs when a new star system is generated
/// How much richer a wreck is in a system the dark has taken.
pub const DARK_HAUL: u32 = 4;

pub fn spawn_system_pois(
    commands: &mut Commands,
    keep_clear: &KeepClear,
    system_id: u32,
    // The system's primary station, where a jump arrives
    // (`world::home_base::station_sites`).
    station: Option<Vec2>,
    planet_positions: &[Vec2],
    rng: &mut impl Rng,
    // The expedition records this system holds, in reading order (see
    // narrative::trail). Each goes on a derelict of its own.
    records: &[&'static crate::narrative::logs::LogEntryDef],
    // Slots already stripped on an earlier visit (LootedPois::for_system).
    looted: &[u32],
    // Taken by the Cascade (narrative::front): everything here stopped where
    // it was, and nobody has stripped it since. Rich, if you can stand it.
    in_the_dark: bool,
) {
    let haul = if in_the_dark { DARK_HAUL } else { 1 };
    let system_center = keep_clear.center;
    // Derelict ships (1-3 per system, and at least one per record held)
    let derelict_count = rng.gen_range(1..=3).max(records.len());
    for i in 0..derelict_count {
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let dist = rng.gen_range(30_000.0..80_000.0);
        let pos = if system_id == 0 && i == 0 && !records.is_empty() {
            // The trail starts at home: Haven's first record lies a short
            // flight out from the berth, on the side away from the star.
            let spawn = crate::world::home_base::SPAWN_BERTH;
            spawn + (spawn - system_center).normalize_or(Vec2::Y) * FIRST_RECORD_FROM_SPAWN
        } else if let (Some(station), true) = (station, i < records.len()) {
            // A record: a short hop from the station a jump arrives at.
            let t = (dist - 30_000.0) / 50_000.0;
            let reach = RECORD_FROM_STATION.start
                + t * (RECORD_FROM_STATION.end - RECORD_FROM_STATION.start);
            keep_clear.near_station(station, angle, reach)
        } else {
            // Anything else: the same 30-80 km, counted from where the
            // star's pull eases off rather than from its centre.
            keep_clear.settle(system_center + Vec2::from_angle(angle) * (keep_clear.floor() + dist))
        };

        let poi = commands.spawn((
            (Sprite {
                    color: Color::srgb(0.35, 0.30, 0.28),
                    custom_size: Some(Vec2::new(200.0, 80.0)),
                    ..default()
                }, Transform::from_xyz(pos.x, pos.y, -0.3)),
            SpacePoi {
                poi_type: SpacePoiType::DerelictShip,
                looted: looted.contains(&(i as u32)),
                discovered: false,
                name: format!("{} derelict", super::galaxy::system_name(system_id)),
                loot_value: rng.gen_range(50..200) * haul,
            },
            PoiSlot(i as u32),
            StarSystemMember { system_id },
        )).id();
        // The record this derelict holds, if the trail put one here. (The
        // roll that used to decide whether a hulk had something to read is
        // still drawn, so the rest of the system lays out as it always has.)
        let _ = rng.gen::<f32>();
        if let Some(entry) = records.get(i) {
            commands.entity(poi).insert(LogEntry {
                title: entry.title.to_string(),
                text: entry.text.to_string(),
                depth_hint: 0.0,
            });
        }
    }

    // Asteroid resource nodes used to be spawned separately here, clustered
    // near planets — now every decorative asteroid (spawning::spawn_asteroid_field,
    // called for every system including warp jumps) carries its own
    // MineableResource directly, so this duplicate/invisible-to-the-player
    // node type is gone. planet_positions is still used below (space station).

    // Anomaly (0-1 per system, rare)
    if rng.gen::<f32>() < 0.4 {
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let dist = rng.gen_range(50_000.0..100_000.0);
        let pos = keep_clear.settle(system_center + Vec2::from_angle(angle) * (keep_clear.floor() + dist));

        // After every derelict's slot, so it never shares one.
        let slot = derelict_count as u32;
        let poi = commands.spawn((
            (Sprite {
                    color: Color::srgba(0.5, 0.3, 0.8, 0.6),
                    custom_size: Some(Vec2::splat(300.0)),
                    ..default()
                }, Transform::from_xyz(pos.x, pos.y, -0.3)),
            PoiSlot(slot),
            SpacePoi {
                poi_type: SpacePoiType::Anomaly,
                looted: looted.contains(&slot),
                discovered: false,
                name: format!("{} anomaly", super::galaxy::system_name(system_id)),
                loot_value: rng.gen_range(100..500) * haul,
            },
            StarSystemMember { system_id },
        )).id();
        // Anomalies no longer carry records: every record has one planned
        // home on a derelict (narrative::trail), and a second copy here
        // would only ever be a repeat.
        let _ = poi;
    }

    // Space station (1 per system, near a planet)
    if let Some(planet_pos) = planet_positions.first() {
        let station_offset = Vec2::new(
            rng.gen_range(-8_000.0..8_000.0),
            rng.gen_range(-8_000.0..8_000.0),
        );
        // `planet_positions` are rolled points near the star, not planets;
        // keep their bearing, counted out from the star's keep-out.
        let offset = *planet_pos + station_offset - system_center;
        let pos = keep_clear.settle(
            system_center + offset.normalize_or(Vec2::X) * (keep_clear.floor() + offset.length()),
        );

        commands.spawn((
            (Sprite {
                    color: Color::srgb(0.45, 0.50, 0.55),
                    custom_size: Some(Vec2::splat(150.0)),
                    ..default()
                }, Transform::from_xyz(pos.x, pos.y, -0.2)),
            SpacePoi {
                poi_type: SpacePoiType::SpaceStation,
                looted: false,
                discovered: false,
                name: format!("{} waystation", super::galaxy::system_name(system_id)),
                loot_value: 0,
            },
            StarSystemMember { system_id },
        ));
    }
}

/// Mining system: when ship is near a MineableResource and has a Mining Drill, extract resources
pub fn mining_system(
    time: Res<Time>,
    ship_query: Query<&Transform, With<Ship>>,
    drill_query: Query<&Module, Without<DestroyedModule>>,
    mut resource_query: Query<(&Transform, &mut MineableResource, &mut SpacePoi, Option<&CelestialBody>), Without<Ship>>,
    mut inventory: ResMut<Inventory>,
    mut notifications: MessageWriter<ShowNotification>,
    mut last_notify: Local<f32>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();
    let dt = time.delta_secs();

    // Check if ship has active mining drill
    let has_drill = drill_query.iter()
        .any(|m| m.module_type == ModuleType::MiningDrill && m.is_active);
    if !has_drill { return; }

    *last_notify += dt;

    for (res_transform, mut resource, mut poi, body) in resource_query.iter_mut() {
        let dist = ship_pos.distance(res_transform.translation.truncate());

        // Mining range, measured ship root to asteroid center. Rocks are
        // solid now, so the reachable minimum is the rock's radius plus most
        // of the ship's own length — scale the range with the rock so big
        // asteroids stay mineable.
        let range = 1200.0 + body.map(|b| b.radius).unwrap_or(0.0);
        if dist > range || resource.resource_remaining <= 0.0 { continue; }

        let extracted = resource.extraction_rate * dt;
        resource.resource_remaining -= extracted;

        // Convert to inventory items
        let item = match resource.resource_type {
            ResourceNodeType::MetalOre => ItemType::ScrapMetal,
            ResourceNodeType::RareCrystal => ItemType::Crystal,
            ResourceNodeType::FuelDeposit => ItemType::FuelCell,
            ResourceNodeType::ExoticMatter => ItemType::RareAlloy,
        };

        // Add to inventory every ~2 seconds worth of extraction
        if resource.resource_remaining % 10.0 < extracted {
            inventory.add_item(item, 1);
        }

        // Notify periodically
        if *last_notify > 3.0 {
            *last_notify = 0.0;
            notifications.write(ShowNotification {
                message: format!("Mining {:?}... {:.0} remaining", resource.resource_type, resource.resource_remaining),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
        }

        if resource.resource_remaining <= 0.0 {
            poi.looted = true;
            notifications.write(ShowNotification {
                message: format!("{} depleted", poi.name),
                notification_type: NotificationType::Info,
                duration: 3.0,
            });
        }
    }
}

/// How close the ship's root must be to a derelict or anomaly to loot it.
/// Root-to-center; derelicts are solid now, so leave room for the ship's own
/// hull between root and contact point.
const LOOT_RANGE: f32 = 1400.0;

fn lootable(poi: &SpacePoi) -> bool {
    !poi.looted && matches!(poi.poi_type, SpacePoiType::DerelictShip | SpacePoiType::Anomaly)
}

/// Says so when something lootable comes within reach. E strips a derelict
/// or anomaly outright, and nothing anywhere on screen mentioned it -- no
/// prompt, no toolbar entry, no hint -- so the only players who ever got paid
/// for one pressed E by accident. Once per object, like the station's
/// "press F to dock".
pub fn loot_prompt_system(
    ship_query: Query<&Transform, With<Ship>>,
    poi_query: Query<(Entity, &Transform, &SpacePoi), Without<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
    mut prompted: Local<Option<Entity>>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();
    let in_reach = poi_query
        .iter()
        .filter(|(_, t, poi)| lootable(poi) && ship_pos.distance(t.translation.truncate()) <= LOOT_RANGE)
        .min_by(|(_, a, _), (_, b, _)| {
            ship_pos
                .distance_squared(a.translation.truncate())
                .total_cmp(&ship_pos.distance_squared(b.translation.truncate()))
        });
    let Some((entity, _, poi)) = in_reach else {
        *prompted = None;
        return;
    };
    if *prompted != Some(entity) {
        *prompted = Some(entity);
        notifications.write(ShowNotification {
            message: format!("{} in reach - press E to strip it", poi.name),
            notification_type: NotificationType::Info,
            duration: 4.0,
        });
    }
}

/// Loot derelict ships when close
pub fn loot_derelict_system(
    ship_query: Query<&Transform, With<Ship>>,
    mut poi_query: Query<(&Transform, &mut SpacePoi, Option<&PoiSlot>, Option<&StarSystemMember>), Without<Ship>>,
    mut currency: ResMut<Currency>,
    mut looted: ResMut<super::resources::LootedPois>,
    mut notifications: MessageWriter<ShowNotification>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) { return; }

    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    for (poi_transform, mut poi, slot, member) in poi_query.iter_mut() {
        if !lootable(&poi) { continue; }
        if ship_pos.distance(poi_transform.translation.truncate()) > LOOT_RANGE { continue; }

        poi.looted = true;
        // The flag dies with the entity on the next jump; this outlives it.
        if let (Some(slot), Some(member)) = (slot, member) {
            looted.mark(member.system_id, slot.0);
        }
        currency.credits += poi.loot_value;
        notifications.write(ShowNotification {
            message: format!("Looted {}! +{}c", poi.name, poi.loot_value),
            notification_type: NotificationType::Success,
            duration: 3.0,
        });
        return;
    }
}

#[cfg(test)]
mod loot_prompt_tests {
    use super::*;

    fn poi(kind: SpacePoiType, looted: bool) -> SpacePoi {
        SpacePoi { poi_type: kind, looted, discovered: true, name: "Haven derelict".into(), loot_value: 100 }
    }

    fn prompts(app: &App) -> Vec<String> {
        app.world().resource::<Messages<ShowNotification>>().iter_current_update_messages().map(|n| n.message.clone()).collect()
    }

    #[test]
    fn a_derelict_in_reach_is_announced_once() {
        let mut app = App::new();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, loot_prompt_system);
        app.world_mut().spawn((Ship, Transform::default()));
        app.world_mut().spawn((Transform::from_xyz(800.0, 0.0, 0.0), poi(SpacePoiType::DerelictShip, false)));
        app.update();
        assert_eq!(prompts(&app), vec!["Haven derelict in reach - press E to strip it".to_string()]);
        app.update();
        assert!(prompts(&app).is_empty(), "prompted again while still in reach");
    }

    /// Rocks, waystations and already-stripped hulks have nothing for E.
    #[test]
    fn nothing_to_loot_says_nothing() {
        let mut app = App::new();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, loot_prompt_system);
        app.world_mut().spawn((Ship, Transform::default()));
        app.world_mut().spawn((Transform::from_xyz(500.0, 0.0, 0.0), poi(SpacePoiType::AsteroidNode, false)));
        app.world_mut().spawn((Transform::from_xyz(600.0, 0.0, 0.0), poi(SpacePoiType::DerelictShip, true)));
        app.world_mut().spawn((Transform::from_xyz(5000.0, 0.0, 0.0), poi(SpacePoiType::DerelictShip, false)));
        app.update();
        assert!(prompts(&app).is_empty());
    }
}

#[cfg(test)]
mod loot_memory_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use rand::SeedableRng;
    use crate::celestial::resources::LootedPois;

    const SYSTEM: u32 = 3;

    /// One system's points of interest as `spawn_system_pois` lays them out
    /// from a fixed seed, as (slot, looted).
    fn spawn(looted: Vec<u32>) -> Vec<(u32, bool)> {
        let mut world = World::new();
        world
            .run_system_once(move |mut commands: Commands| {
                let keep_clear = KeepClear { center: Vec2::ZERO, star_radius: 40_000.0, planets: &[] };
                let mut rng = rand::rngs::StdRng::seed_from_u64(7);
                spawn_system_pois(&mut commands, &keep_clear, SYSTEM, None, &[], &mut rng, &[], &looted, false);
            })
            .unwrap();
        let mut q = world.query::<(&PoiSlot, &SpacePoi)>();
        let mut out: Vec<(u32, bool)> = q.iter(&world).map(|(slot, poi)| (slot.0, poi.looted)).collect();
        out.sort();
        out
    }

    /// Strip a derelict, leave the system (its entities go), come back (they
    /// respawn from the seed): it is still stripped, and nothing else is.
    #[test]
    fn a_stripped_derelict_stays_stripped_on_the_next_visit() {
        let first = spawn(Vec::new());
        assert!(first.iter().all(|(_, looted)| !looted), "a fresh system came up already stripped");
        let (slot, _) = first[0];

        let mut app = App::new();
        app.add_message::<ShowNotification>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Currency>()
            .init_resource::<LootedPois>()
            .add_systems(Update, loot_derelict_system);
        app.world_mut().spawn((Ship, Transform::default()));
        app.world_mut().spawn((
            Transform::from_xyz(500.0, 0.0, 0.0),
            SpacePoi { poi_type: SpacePoiType::DerelictShip, looted: false, discovered: true, name: "x".into(), loot_value: 100 },
            PoiSlot(slot),
            StarSystemMember { system_id: SYSTEM },
        ));
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyE);
        app.update();

        let looted = app.world().resource::<LootedPois>().for_system(SYSTEM).to_vec();
        assert_eq!(looted, vec![slot]);
        assert!(app.world().resource::<LootedPois>().for_system(SYSTEM + 1).is_empty());

        let again = spawn(looted);
        assert_eq!(again.len(), first.len(), "the respawn laid the system out differently");
        for (s, is_looted) in again {
            assert_eq!(is_looted, s == slot, "slot {s} came back looted={is_looted}");
        }
    }
}
