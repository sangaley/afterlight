use bevy::prelude::*;

use crate::ai_ship::components::{AiShip, AiShipType};
use crate::camera::CameraState;
use crate::celestial::components::{CelestialBody, CelestialBodyType};
use crate::celestial::poi::{SpacePoi, SpacePoiType};
use crate::combat::energy_weapons::{EmpWarhead, IonPulse};
use crate::combat::new_projectiles::{MissileProjectile, Projectile as NewProjectile};
use crate::combat::severance::DetachedSection;
use crate::components::{HullSegment, Module, Projectile as LegacyProjectile, Ship, ShipPhysics, Velocity};
use crate::events::{AiShipDamaged, DamageSource, ShipDamaged};
use crate::spatial::SpatialGrid;
use crate::world::home_base::Station;

/// "Terrain" for weapons fire and AI steering: solid scenery, not ships and
/// not battle debris (shots pass through drifting chunks; only real obstacles
/// stop them).
pub type TerrainFilter = (Without<Ship>, Without<AiShip>, Without<DetachedSection>);

// ============================================================================
// PHYSICAL COLLISION — ships, stations, asteroids, planets, stars.
// Everything solid carries a Collider (one or more circles in local space).
// Ships and asteroids exchange real momentum (mass-weighted impulses);
// stations, planets and stars are immovable. Hard crashes damage the hulls
// on both sides. Projectiles/missiles keep their own hit systems.
// ============================================================================

/// Bounce energy kept on impact (0 = dead stop, 1 = perfect bounce).
const RESTITUTION: f32 = 0.30;
/// Fraction of remaining overlap corrected per frame — softens de-penetration
/// so contacts settle instead of snapping.
const CORRECTION: f32 = 0.45;
/// Per-frame cap on positional correction. A warp that drops the ship inside
/// a planet slides it to the surface over a few frames instead of teleporting.
const MAX_PUSH: f32 = 350.0;
/// Relative approach speed above which an impact sparks and kicks the camera.
const IMPACT_FX_SPEED: f32 = 180.0;
/// Approach speed above which a crash starts damaging hulls. Close-quarters
/// ram speeds are ~200-500; the original 320 was above most real impacts.
const DAMAGE_MIN_SPEED: f32 = 200.0;
/// ...damage per unit of speed beyond that (scaled by reduced mass below).
const DAMAGE_SCALE: f32 = 0.15;
/// Cap per contact so an extreme-speed clip can't one-shot a hull.
const DAMAGE_CAP: f32 = 130.0;
/// Covers a block's corners from its center (33 * sqrt2) plus a little skin.
const BLOCK_SLACK: f32 = 48.0;
/// AI ships have no ShipPhysics; estimate mass from block count. Tuned so a
/// starter-sized hull (~150 blocks) lands near the player's 1200.
const BLOCK_MASS: f32 = 8.0;
/// Colliders bigger than this (planets, stars) skip the grid and are checked
/// against every ship directly — there are only a handful per system, and a
/// star-sized query radius would defeat the grid entirely.
const HUGE_RADIUS: f32 = 1500.0;
/// Below this speed (squared) a body counts as asleep — sleeping asteroids
/// don't scan for collisions; ships always do.
const WAKE_SPEED_SQ: f32 = 0.25;

/// Physical collision body: one or more solid circles in entity-local space.
/// Static obstacles are a single circle with infinite mass; ships get a few
/// circles fitted along their long axis so a wedge hull doesn't collide like
/// one giant ball around its nose.
#[derive(Component)]
pub struct Collider {
    /// (local center, radius) — rotated/translated by the entity's Transform.
    pub circles: Vec<(Vec2, f32)>,
    /// Bounding circle enclosing all sub-circles, for the broad phase.
    pub bound_center: Vec2,
    pub bound_radius: f32,
    /// f32::INFINITY = immovable.
    pub mass: f32,
    /// Always scans for collisions (ships). Non-awake movers (asteroids)
    /// only scan while actually moving; sleeping rocks cost nothing.
    pub awake: bool,
}

impl Collider {
    pub(crate) fn circle(radius: f32, mass: f32) -> Self {
        Self {
            circles: vec![(Vec2::ZERO, radius)],
            bound_center: Vec2::ZERO,
            bound_radius: radius,
            mass,
            awake: false,
        }
    }

    /// A collider made of several circles, with its bounds worked out.
    pub(crate) fn circles(circles: Vec<(Vec2, f32)>, mass: f32) -> Self {
        let bound_radius = circles
            .iter()
            .map(|(center, radius)| center.length() + radius)
            .fold(0.0, f32::max);
        Self { circles, bound_center: Vec2::ZERO, bound_radius, mass, awake: false }
    }

    fn inv_mass(&self) -> f32 {
        if self.mass.is_finite() { 1.0 / self.mass } else { 0.0 }
    }
}

/// Collision shove for AI ships, kept separate from their steering Velocity —
/// the AI movement system rewrites Velocity every frame (lerp toward its nav
/// target), which would silently eat any impulse landed there. Knock is
/// integrated and decayed independently, so a ram physically displaces an AI
/// ship (or a dead wreck) while its own steering stays intact.
#[derive(Component, Default)]
pub struct KnockVelocity(pub Vec2);

pub(crate) fn world_point(transform: &Transform, local: Vec2) -> Vec2 {
    transform.translation.truncate() + (transform.rotation * local.extend(0.0)).truncate()
}

/// Broad-phase index over all colliders, rebuilt each frame. Small colliders
/// go in the grid; planet/star-sized ones are kept in a flat list.
#[derive(Resource)]
pub struct ColliderGrid {
    grid: SpatialGrid,
    max_small: f32,
    huge: Vec<Entity>,
}

impl Default for ColliderGrid {
    fn default() -> Self {
        Self { grid: SpatialGrid::new(512.0), max_small: 0.0, huge: Vec::new() }
    }
}

impl ColliderGrid {
    /// Collider entities whose bounds could touch a circle around `pos` —
    /// grid candidates plus every huge body. Callers do the exact test.
    pub fn candidates(&self, pos: Vec2, radius: f32) -> impl Iterator<Item = Entity> + '_ {
        self.grid
            .nearby(pos, radius + self.max_small)
            .map(|(entity, _)| entity)
            .chain(self.huge.iter().copied())
    }
}

/// Nearest terrain collider hit by the segment `from -> to` (padded by
/// `pad`), as (entity, impact point). Uses bounding circles — plenty for
/// bullets vs rocks.
pub fn terrain_segment_hit(
    grid: &ColliderGrid,
    terrain: &Query<(&Transform, &Collider), TerrainFilter>,
    from: Vec2,
    to: Vec2,
    pad: f32,
) -> Option<(Entity, Vec2)> {
    let travel = to - from;
    let len = travel.length();
    let dir = if len > 1e-4 { travel / len } else { Vec2::X };
    let mid = from + travel * 0.5;

    let mut best: Option<(f32, Entity)> = None;
    for entity in grid.candidates(mid, len * 0.5 + pad) {
        let Ok((transform, collider)) = terrain.get(entity) else { continue };
        let center = world_point(transform, collider.bound_center);
        // The bounding circle is only the broad phase: a station with arms
        // has a bound tens of kilometres across that is mostly open space,
        // and stopping at it cut every beam fired inside it to nothing.
        if segment_meets_circle(from, dir, len, center, collider.bound_radius + pad).is_none() {
            continue;
        }
        for (c, r) in &collider.circles {
            let Some(t) = segment_meets_circle(from, dir, len, world_point(transform, *c), r + pad) else { continue };
            if best.map(|(bt, _)| t < bt).unwrap_or(true) {
                best = Some((t, entity));
            }
        }
    }
    best.map(|(t, entity)| (entity, from + dir * t))
}

/// How far along a segment (`from`, unit `dir`, length `len`) it first meets
/// a circle; 0 if it starts inside.
pub(crate) fn segment_meets_circle(from: Vec2, dir: Vec2, len: f32, center: Vec2, radius: f32) -> Option<f32> {
    let m = from - center;
    let b = m.dot(dir);
    let c = m.length_squared() - radius * radius;
    if c > 0.0 && b > 0.0 {
        return None; // starts outside, pointing away
    }
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()).max(0.0);
    (t <= len).then_some(t)
}

/// How far a shape made of circles (centres measured from its middle)
/// reaches along `dir` from that middle, counting only what lies within
/// `half_width` of the line. None when nothing of it lies that way.
///
/// A station's bounding circle says how far its longest arm reaches; this
/// says how far it reaches the way you are coming from, which between two
/// arms is a great deal less.
pub fn hull_reach(circles: &[(Vec2, f32)], dir: Vec2, half_width: f32) -> Option<f32> {
    circles
        .iter()
        .filter_map(|&(c, r)| {
            let along = c.dot(dir);
            let lateral = (c - dir * along).length();
            (lateral < r + half_width && along + r > 0.0).then_some(along + r)
        })
        .reduce(f32::max)
}

/// Something round in a ship's way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blocker {
    pub center: Vec2,
    pub radius: f32,
}

/// Room kept between a hull and whatever it is steering round.
const STEER_SKIN: f32 = 60.0;
/// How much of a hull's reach it keeps clear to either side as it steers.
///
/// A hull's reach is half its length, nose to tail, and it flies nose
/// first: it is nothing like that wide. Steering as if it were a ball of
/// that radius, a freighter 3.7 km long counted every part of a station
/// within 3.7 km as already too close, was pushed away from the ones on
/// both sides of it at once, and hung where it was between two arms it had
/// kilometres of room to leave by.
const HULL_SHARE: f32 = 0.6;

/// The room a hull of reach `own_radius` keeps from the middle of `b`.
fn room_for(own_radius: f32, b: &Blocker) -> f32 {
    b.radius + own_radius * HULL_SHARE + STEER_SKIN
}

/// How far ahead a ship doing `speed` looks for things in its way: about two
/// and a half seconds of flying, and never past where it's going.
///
/// It was a flat 2,200 u whatever the speed. At harbour speed that is six
/// seconds of warning, which swung traders wide of things they were nowhere
/// near; at 1,100 u/s it is two, which is less than a big hull needs to turn.
pub fn look_ahead(speed: f32, own_radius: f32, travel: f32) -> f32 {
    ((speed * 2.5).clamp(900.0, 3_500.0) + own_radius).min(travel)
}

/// The solid things near the line from `pos` along `desired` for `look`,
/// added to `out`.
///
/// Something big made of several parts -- a station with arms -- gives its
/// parts, not its bounding circle: the circle round a Raft is tens of
/// kilometres of mostly open space, and steering round that kept ships out
/// of the places between its arms that they had been told to go to.
pub fn terrain_blockers(
    grid: &ColliderGrid,
    terrain: &Query<(&Transform, &Collider), TerrainFilter>,
    pos: Vec2,
    desired: Vec2,
    look: f32,
    own_radius: f32,
    out: &mut Vec<Blocker>,
) {
    let mid = pos + desired * (look * 0.5);
    let reach = look * 0.5 + own_radius + STEER_SKIN;
    for entity in grid.candidates(mid, look * 0.5) {
        let Ok((transform, collider)) = terrain.get(entity) else { continue };
        collider_blockers(transform, collider, mid, reach, out);
    }
}

/// One collider's share of `terrain_blockers`: its bounding circle, or its
/// parts when it is big and has more than one. Only what comes within
/// `reach` of `mid`.
pub fn collider_blockers(transform: &Transform, collider: &Collider, mid: Vec2, reach: f32, out: &mut Vec<Blocker>) {
    let center = world_point(transform, collider.bound_center);
    if center.distance_squared(mid) >= (reach + collider.bound_radius).powi(2) {
        return;
    }
    if collider.bound_radius <= HUGE_RADIUS || collider.circles.len() < 2 {
        out.push(Blocker { center, radius: collider.bound_radius });
        return;
    }
    for (c, r) in &collider.circles {
        let center = world_point(transform, *c);
        if center.distance_squared(mid) < (reach + r).powi(2) {
            out.push(Blocker { center, radius: *r });
        }
    }
}

/// A blocker with the room `own_radius` needs round it, as seen from `pos`
/// flying along `desired`: (how far along, how far to the side and which
/// way, the room to keep).
fn sight(pos: Vec2, desired: Vec2, own_radius: f32, b: &Blocker) -> (f32, Vec2, f32) {
    let rel = b.center - pos;
    let along = rel.dot(desired);
    (along, rel - desired * along, room_for(own_radius, b))
}

/// Whether a ship at `pos` flying along `desired` for `look` would come
/// closer to `b` than it should, or already has.
fn threatens(pos: Vec2, desired: Vec2, look: f32, own_radius: f32, b: &Blocker) -> bool {
    let (along, lateral, clearance) = sight(pos, desired, own_radius, b);
    let inside = (b.center - pos).length_squared() < clearance * clearance;
    inside || (along >= 0.0 && along <= look + clearance && lateral.length() < clearance)
}

/// `desired`, bent to slide round whatever of `blockers` is in the way --
/// the same "swing around, don't clip through" trick the EVA crew use,
/// scaled up. Returns the (unit) direction to fly and the side it went:
/// +1 left of `desired`, -1 right, 0 for straight on.
///
/// Things too close together to fly between are rounded as one. Taken one
/// at a time, two rocks side by side each sent the ship toward the other.
///
/// `side_hint` is the side it went last time (0 for none). Something dead
/// ahead, or nearly, is passed on that side again: the nearest side changes
/// as the ship swings, and a ship that follows it wags its nose instead of
/// going round.
pub fn bend_around(pos: Vec2, desired: Vec2, look: f32, own_radius: f32, blockers: &[Blocker], side_hint: f32) -> (Vec2, f32) {
    if look < 1.0 {
        return (desired, 0.0);
    }
    let threats: Vec<Blocker> = blockers.iter().copied().filter(|b| threatens(pos, desired, look, own_radius, b)).collect();
    // The first one it would come to.
    let Some(first) = threats.iter().copied().min_by(|a, b| {
        sight(pos, desired, own_radius, a).0.total_cmp(&sight(pos, desired, own_radius, b).0)
    }) else {
        return (desired, 0.0);
    };
    // ...and everything in the way that it couldn't get between and that.
    // A hull goes through a gap nose first: `own_radius` is how far it
    // reaches end to end, and it is about half as wide as that. Measured by
    // its full reach, the rocks of an asteroid pack (1,300 u apart) were one
    // solid wall to a freighter, which then had nowhere to go.
    let gap = own_radius + 2.0 * STEER_SKIN;
    let mut group = vec![first];
    let mut rest: Vec<Blocker> = threats.into_iter().filter(|b| *b != first).collect();
    loop {
        let before = group.len();
        rest.retain(|b| {
            let joins = group.iter().any(|g| g.center.distance(b.center) - g.radius - b.radius < gap);
            if joins {
                group.push(*b);
            }
            !joins
        });
        if group.len() == before {
            break;
        }
    }
    let whole = enclosing(&group);
    // In among them already: the circle round the lot is all round it, and
    // steering out of that from the middle of a pack goes nowhere. By the
    // nearest one, then, as a ship picking its way through.
    let whole = if group.len() > 1 && whole.center.distance(pos) < whole.radius { first } else { whole };

    let (along, lateral, clearance) = sight(pos, desired, own_radius, &whole);
    let left = desired.perp();
    // Away from its middle. Dead ahead, or close to it, the side it took
    // before; with no side taken yet, the left.
    let natural = if lateral.dot(left) > 0.0 { -1.0 } else { 1.0 };
    let side = if side_hint != 0.0 && lateral.length() < clearance * 0.3 {
        side_hint.signum()
    } else if lateral.length() < 1.0 {
        1.0
    } else {
        natural
    };
    let away = left * side;
    let need = ((clearance - lateral.length()) / clearance).clamp(0.0, 1.0);
    let urgency = (1.0 - along / look).clamp(0.2, 1.0);
    let mut out = desired + away * (need * (0.6 + 1.4 * urgency));
    // Already closer than it should be to something: straight out from it.
    for b in &group {
        let rel = b.center - pos;
        let room = room_for(own_radius, b);
        if rel.length_squared() < room * room {
            out += (-rel).normalize_or(away);
        }
    }
    (out.try_normalize().unwrap_or(desired), side)
}

/// One circle round all of `group`.
fn enclosing(group: &[Blocker]) -> Blocker {
    if group.len() == 1 {
        return group[0];
    }
    let weight: f32 = group.iter().map(|b| b.radius).sum();
    let center = group.iter().map(|b| b.center * b.radius).sum::<Vec2>() / weight.max(1.0);
    let radius = group.iter().map(|b| b.center.distance(center) + b.radius).fold(0.0, f32::max);
    Blocker { center, radius }
}

/// Bends a desired travel direction to slide around blocking terrain.
/// Returns the (unit) direction to actually fly. For a ship that keeps no
/// memory of which side it went; `bend_around` is the whole of it.
pub fn steer_around(
    grid: &ColliderGrid,
    terrain: &Query<(&Transform, &Collider), TerrainFilter>,
    pos: Vec2,
    desired: Vec2,
    travel: f32,
    own_radius: f32,
) -> Vec2 {
    let look = travel.min(2200.0);
    let mut blockers = Vec::new();
    terrain_blockers(grid, terrain, pos, desired, look, own_radius, &mut blockers);
    bend_around(pos, desired, look, own_radius, &blockers, 0.0).0
}

/// Weapons fire smashes into terrain: bullets, missiles, ion pulses and EMP
/// warheads fizzle against asteroids/stations/planets instead of passing
/// through — rocks are cover now. Lasers are clamped separately at fire time
/// (instant rays), and debris chunks deliberately don't block shots.
pub fn shots_hit_terrain(
    grid: Res<ColliderGrid>,
    shots: Query<
        (Entity, &Transform),
        Or<(
            With<NewProjectile>,
            With<LegacyProjectile>,
            With<MissileProjectile>,
            With<IonPulse>,
            With<EmpWarhead>,
        )>,
    >,
    terrain: Query<(&Transform, &Collider), TerrainFilter>,
    mut commands: Commands,
) {
    for (entity, transform) in shots.iter() {
        let pos = transform.translation.truncate();
        for candidate in grid.candidates(pos, 8.0) {
            let Ok((terrain_transform, collider)) = terrain.get(candidate) else { continue };
            let center = world_point(terrain_transform, collider.bound_center);
            if shot_meets(collider, terrain_transform, center, pos) {
                crate::combat::spawn_hit_effect(
                    &mut commands,
                    pos,
                    Color::srgba(0.9, 0.8, 0.6, 0.8),
                    10.0,
                );
                commands.entity(entity).try_despawn();
                break;
            }
        }
    }
}

/// Does a shot at `pos` meet this body? Its bounding circle is only the broad
/// phase: a station with arms, or a ring, has a bound tens of kilometres
/// across that is mostly open space, and testing against it alone swallowed
/// every shot anywhere inside it -- a whole fight near Haven fired rounds that
/// vanished at the muzzle.
fn shot_meets(collider: &Collider, tf: &Transform, center: Vec2, pos: Vec2) -> bool {
    const PAD: f32 = 6.0;
    if pos.distance_squared(center) >= (collider.bound_radius + PAD).powi(2) {
        return false;
    }
    collider.circles.iter().any(|(c, r)| pos.distance_squared(world_point(tf, *c)) < (r + PAD).powi(2))
}

/// Gives freshly spawned static world objects their collision circle.
/// Everything is matched by Added<> here so no spawn site needs to know
/// about collision. Asteroids also get a Velocity: they have finite,
/// area-scaled mass and can be shunted around, billiards-style.
pub fn attach_static_colliders(
    mut commands: Commands,
    celestial: Query<(Entity, &CelestialBody), Added<CelestialBody>>,
    stations: Query<(Entity, &Station, Option<&crate::world::home_base::StationCircles>), Added<Station>>,
    pois: Query<(Entity, &SpacePoi), (Added<SpacePoi>, Without<CelestialBody>)>,
) {
    for (entity, body) in celestial.iter() {
        match body.body_type {
            CelestialBodyType::Asteroid => {
                // Rock sprites don't fill their square; shrink so the contact
                // matches what the eye sees. Mass scales with area: small
                // rocks shove aside easily, big ones barely notice you.
                let radius = body.radius * 0.85;
                let mass = (radius / 100.0).powi(2) * 1200.0;
                commands
                    .entity(entity)
                    .try_insert((Collider::circle(radius, mass), Velocity(Vec2::ZERO)));
            }
            CelestialBodyType::Planet | CelestialBodyType::Star => {
                commands
                    .entity(entity)
                    .try_insert(Collider::circle(body.radius, f32::INFINITY));
            }
            // Black holes consume ships (BeingConsumed spiral) — a solid rim
            // would break that. Debris is not worth bouncing off.
            CelestialBodyType::BlackHole | CelestialBodyType::Debris => {}
        }
    }
    // Stations: the hub-and-ring disc plus each docking arm, built from the
    // same layout the station is drawn from (home_base::station_collider_circles).
    for (entity, station, own) in stations.iter() {
        let radius = crate::world::home_base::station_radius(station.index);
        // The Raft's own shape when it has one (world::raft).
        let circles = own.map(|c| c.0.clone()).unwrap_or_else(|| crate::world::home_base::station_collider_circles(radius));
        commands.entity(entity).try_insert(Collider::circles(circles, f32::INFINITY));
    }
    for (entity, poi) in pois.iter() {
        let radius = match poi.poi_type {
            SpacePoiType::SpaceStation => 75.0,
            SpacePoiType::DerelictShip => 85.0,
            // Anomalies are readings, not matter; asteroid nodes are covered
            // by their CelestialBody above.
            _ => continue,
        };
        commands.entity(entity).try_insert(Collider::circle(radius, f32::INFINITY));
    }
}

/// Every AI ship gets a knock channel (see KnockVelocity).
pub fn attach_knock(
    mut commands: Commands,
    ships: Query<Entity, (With<AiShip>, Without<KnockVelocity>)>,
) {
    for entity in ships.iter() {
        commands.entity(entity).try_insert(KnockVelocity::default());
    }
}

/// Fits circles over the hull via a 2D bucket grid (~2.5 blocks per cell,
/// one circle per occupied cell). Fine enough that shooting a pocket of
/// blocks off a ship actually opens a hole in its collision — the earlier
/// long-axis-only bucketing kept "phantom hull" where destroyed blocks were.
/// Cell size scales up on huge hulls to cap the circle count.
pub(crate) fn fit_ship_collider(points: &[Vec2], mass: f32) -> Collider {
    if points.is_empty() {
        let mut collider = Collider::circle(120.0, mass);
        collider.awake = true;
        return collider;
    }
    let mut min = points[0];
    let mut max = points[0];
    let mut sum = Vec2::ZERO;
    for p in points {
        min = min.min(*p);
        max = max.max(*p);
        sum += *p;
    }
    let centroid = sum / points.len() as f32;
    let span = max - min;
    let cell = (span.x.max(span.y) / 8.0).max(170.0);

    let mut buckets: std::collections::HashMap<IVec2, Vec<Vec2>> =
        std::collections::HashMap::new();
    for p in points {
        let key = IVec2::new(
            ((p.x - min.x) / cell) as i32,
            ((p.y - min.y) / cell) as i32,
        );
        buckets.entry(key).or_default().push(*p);
    }

    let mut circles = Vec::new();
    for bucket in buckets.values() {
        let center = bucket.iter().sum::<Vec2>() / bucket.len() as f32;
        let radius = bucket.iter().map(|p| p.distance(center)).fold(0.0, f32::max) + BLOCK_SLACK;
        circles.push((center, radius.max(50.0)));
    }
    let bound_radius = circles
        .iter()
        .map(|(c, r)| c.distance(centroid) + r)
        .fold(0.0, f32::max);

    Collider { circles, bound_center: centroid, bound_radius, mass, awake: true }
}

/// (Re)builds ship colliders from their child blocks — on spawn and whenever
/// the block set changes (build edits, battle damage, severance). Covers the
/// player, AI ships, and wrecks (a dead AI ship keeps its entity and blocks).
pub fn refresh_ship_colliders(
    mut commands: Commands,
    // Refit on child add/remove (build edits, block despawn, severance)...
    changed: Query<
        Entity,
        (Or<(With<Ship>, With<AiShip>)>, Or<(Changed<Children>, Without<Collider>)>),
    >,
    // ...and the moment a block is MARKED destroyed — its despawn is 0.5s
    // out (PendingRemoval) and the hole should open now, not then.
    fresh_destroyed: Query<
        &ChildOf,
        Or<(Added<crate::components::HullDestroyed>, Added<crate::components::DestroyedModule>)>,
    >,
    ships: Query<
        (Entity, Option<&ShipPhysics>, Option<&Children>, Has<crate::diplomacy::FactionStation>),
        Or<(With<Ship>, With<AiShip>)>,
    >,
    // Destroyed-but-not-yet-despawned blocks (0.5s PendingRemoval window)
    // don't count toward the shape — a hole is a hole the moment it's made.
    blocks: Query<
        &Transform,
        (
            Or<(With<Module>, With<HullSegment>)>,
            Without<crate::components::DestroyedModule>,
            Without<crate::components::HullDestroyed>,
        ),
    >,
) {
    let mut targets: std::collections::HashSet<Entity> = changed.iter().collect();
    for parent in fresh_destroyed.iter() {
        targets.insert(parent.parent());
    }
    for target in targets {
        let Ok((entity, physics, children, anchored)) = ships.get(target) else { continue };
        let points: Vec<Vec2> = children
            .map(|children| {
                children
                    .iter()
                    .filter_map(|child| blocks.get(child).ok())
                    .map(|t| t.translation.truncate())
                    .collect()
            })
            .unwrap_or_default();
        // A faction station is anchored to its planet: nothing shoves it.
        let mass = if anchored {
            f32::INFINITY
        } else {
            physics.map(|p| p.mass).unwrap_or((points.len() as f32 * BLOCK_MASS).max(100.0))
        };
        commands.entity(entity).try_insert(fit_ship_collider(&points, mass));
    }
}

/// Integrates the motion the regular movement systems don't own: shunted
/// asteroids drift (with a long half-life so a shove sends them coasting,
/// not skidding to a halt), and AI-ship knocks play out and decay.
pub fn integrate_drift(
    time: Res<Time>,
    mut rocks: Query<
        (&mut Transform, &mut Velocity),
        (With<CelestialBody>, Without<KnockVelocity>),
    >,
    mut knocked: Query<(&mut Transform, &mut KnockVelocity), Without<CelestialBody>>,
) {
    let dt = time.delta_secs();
    for (mut transform, mut velocity) in rocks.iter_mut() {
        if velocity.0.length_squared() < 1.0 {
            continue;
        }
        transform.translation += (velocity.0 * dt).extend(0.0);
        velocity.0 *= (0.5_f32).powf(dt / 22.0);
        if velocity.0.length_squared() < 1.0 {
            velocity.0 = Vec2::ZERO;
        }
    }
    for (mut transform, mut knock) in knocked.iter_mut() {
        if knock.0.length_squared() < 4.0 {
            continue;
        }
        transform.translation += (knock.0 * dt).extend(0.0);
        knock.0 *= (0.5_f32).powf(dt / 1.0);
        if knock.0.length_squared() < 4.0 {
            knock.0 = Vec2::ZERO;
        }
    }
}

pub fn rebuild_collider_grid(
    mut grid: ResMut<ColliderGrid>,
    colliders: Query<(Entity, &Transform, &Collider)>,
) {
    grid.grid.clear();
    grid.huge.clear();
    grid.max_small = 0.0;
    for (entity, transform, collider) in colliders.iter() {
        if collider.bound_radius > HUGE_RADIUS {
            grid.huge.push(entity);
        } else {
            grid.grid.insert(entity, world_point(transform, collider.bound_center));
            grid.max_small = grid.max_small.max(collider.bound_radius);
        }
    }
}

fn effective_velocity(vel: Option<&Velocity>, knock: Option<&KnockVelocity>) -> Vec2 {
    vel.map(|v| v.0).unwrap_or(Vec2::ZERO) + knock.map(|k| k.0).unwrap_or(Vec2::ZERO)
}

/// Whether this body scans for collisions this frame (see Collider::awake).
fn initiates(collider: &Collider, vel: Option<&Velocity>, knock: Option<&KnockVelocity>) -> bool {
    collider.awake || effective_velocity(vel, knock).length_squared() > WAKE_SPEED_SQ
}

/// Finds overlapping pairs (broad phase via the grid) and resolves each with
/// a positional push plus a momentum exchange along the contact normal, split
/// by mass. AI ships take their impulse on the knock channel so their
/// steering can't overwrite it. Crashes above DAMAGE_MIN_SPEED hurt both
/// hulls through the same damage events weapons use.
pub fn resolve_collisions(
    grid: Res<ColliderGrid>,
    mut bodies: Query<(
        Entity,
        &mut Transform,
        Option<&mut Velocity>,
        Option<&mut KnockVelocity>,
        &Collider,
    )>,
    player: Query<(), With<Ship>>,
    ai: Query<(), With<AiShip>>,
    ship_types: Query<&AiShipType>,
    mut camera: ResMut<CameraState>,
    mut ship_damage: MessageWriter<ShipDamaged>,
    mut ai_damage: MessageWriter<AiShipDamaged>,
    mut commands: Commands,
) {
    // Pass 1 (read-only): collect candidate pairs. Ships always scan;
    // asteroids only while moving; statics never. Pairs where both sides
    // scan are deduped by entity order so each is resolved once.
    let mut contacts: Vec<(Entity, Entity)> = Vec::new();
    for (entity, transform, velocity, knock, collider) in bodies.iter() {
        if !initiates(collider, velocity, knock) {
            continue;
        }
        let center = world_point(transform, collider.bound_center);
        let candidates = grid
            .grid
            .nearby(center, collider.bound_radius + grid.max_small)
            .map(|(other, _)| other)
            .chain(grid.huge.iter().copied());
        for other in candidates {
            if other == entity {
                continue;
            }
            let Ok((_, other_transform, other_vel, other_knock, other_collider)) =
                bodies.get(other)
            else {
                continue;
            };
            if initiates(other_collider, other_vel, other_knock) && other < entity {
                continue;
            }
            let other_center = world_point(other_transform, other_collider.bound_center);
            if center.distance(other_center) < collider.bound_radius + other_collider.bound_radius {
                contacts.push((entity, other));
            }
        }
    }

    // Pass 2: resolve. Overlaps are recomputed from current transforms so a
    // chain of contacts in one frame doesn't act on stale positions.
    for (a, b) in contacts {
        let Ok([(_, mut ta, mut va, mut ka, ca), (_, mut tb, mut vb, mut kb, cb)]) =
            bodies.get_many_mut([a, b])
        else {
            continue;
        };

        // Deepest overlapping circle pair decides the contact.
        let mut best: Option<(f32, Vec2, Vec2, f32)> = None; // (pen, world_a, world_b, radius_a)
        for (offset_a, radius_a) in &ca.circles {
            let wa = world_point(&ta, *offset_a);
            for (offset_b, radius_b) in &cb.circles {
                let wb = world_point(&tb, *offset_b);
                let pen = radius_a + radius_b - wa.distance(wb);
                if pen > 0.0 && best.map(|(bp, ..)| pen > bp).unwrap_or(true) {
                    best = Some((pen, wa, wb, *radius_a));
                }
            }
        }
        let Some((pen, wa, wb, radius_a)) = best else { continue };

        let delta = wb - wa;
        let normal = if delta.length_squared() > 1e-6 { delta.normalize() } else { Vec2::X };
        let (inv_a, inv_b) = (ca.inv_mass(), cb.inv_mass());
        let total_inv = inv_a + inv_b;
        if total_inv <= 0.0 {
            continue;
        }

        // Momentum exchange: kill approach speed along the normal, keep some
        // bounce. AI ships take the change on their knock channel.
        let vel_a = effective_velocity(va.as_deref(), ka.as_deref());
        let vel_b = effective_velocity(vb.as_deref(), kb.as_deref());
        let approach = (vel_a - vel_b).dot(normal);
        if approach > 0.0 {
            let impulse = (1.0 + RESTITUTION) * approach / total_inv;
            let (dv_a, dv_b) = (-normal * impulse * inv_a, normal * impulse * inv_b);
            match (&mut ka, &mut va) {
                (Some(k), _) => k.0 += dv_a,
                (None, Some(v)) => v.0 += dv_a,
                _ => {}
            }
            match (&mut kb, &mut vb) {
                (Some(k), _) => k.0 += dv_b,
                (None, Some(v)) => v.0 += dv_b,
                _ => {}
            }
        }

        // Positional correction, split by mass.
        let push = (pen * CORRECTION).min(MAX_PUSH);
        ta.translation -= (normal * push * (inv_a / total_inv)).extend(0.0);
        tb.translation += (normal * push * (inv_b / total_inv)).extend(0.0);

        let contact = wa + normal * (radius_a - pen * 0.5);

        // Crash damage: speed beyond the threshold hurts, split by mass —
        // the lighter body takes the bigger share, an immovable wall deals
        // all of it to you. Both hulls go through the regular damage events
        // (block damage, breaches, death attribution, HUD arrows).
        if approach > DAMAGE_MIN_SPEED {
            // Severity scales with the pair's reduced mass: an immovable
            // wall (reduced mass = your own mass) or a heavy ship hits far
            // harder than a light chunk at the same speed — debris pings
            // off you for ~nothing, a planet crumples your bow.
            let reduced_mass = 1.0 / total_inv;
            let mass_factor = (reduced_mass / 500.0).clamp(0.2, 2.0);
            let energy =
                ((approach - DAMAGE_MIN_SPEED) * DAMAGE_SCALE * mass_factor).min(DAMAGE_CAP);
            // Ram identity: (damage dealt mult, damage taken mult) per
            // faction. Recursive Kingdom hulls ARE battering rams — they hit way
            // harder and shrug most of it off; Corpse Stars ram heavy too.
            let ram_profile = |entity: Entity| match ship_types.get(entity) {
                Ok(AiShipType::RecursiveKingdom) => (2.5, 0.4),
                Ok(AiShipType::CorpseStars) => (1.8, 0.7),
                _ => (1.0, 1.0),
            };
            // Split by sqrt-softened inverse mass: the lighter body still
            // takes the bigger share, but the raw inv-mass split let a heavy
            // ship shrug rams to near-zero (1200 vs a 240-mass raider gave
            // the player 17% of already-small damage — invisible in play).
            let (weight_a, weight_b) = (inv_a.sqrt(), inv_b.sqrt());
            let total_weight = weight_a + weight_b;
            // ShipDamaged/AiShipDamaged `direction` points from the ship
            // TOWARD the attacker (the convention every other writer uses,
            // and what process_ship_damage walks inward from). `normal`
            // runs a -> b, so for a the attacker lies along +normal.
            // Who rammed whom: the one driving harder into the other along
            // the contact line. A ship blamed for a ram is the attacker; a
            // ship that was rammed is not, so a neutral escort drifting into
            // your hull doesn't count as you attacking it.
            let (closing_a, closing_b) = (vel_a.dot(normal), -vel_b.dot(normal));
            for (entity, other, share, into, other_rammed) in [
                (a, b, weight_a / total_weight, normal, closing_b >= closing_a),
                (b, a, weight_b / total_weight, -normal, closing_a >= closing_b),
            ] {
                let amount =
                    (energy * share * ram_profile(other).0 * ram_profile(entity).1).min(150.0);
                if amount < 1.0 {
                    continue;
                }
                if player.contains(entity) {
                    ship_damage.write(ShipDamaged {
                        source: DamageSource::Collision,
                        amount,
                        position: Some(contact),
                        direction: Some(into),
                        applied: false,
                    });
                } else if ai.contains(entity) {
                    ai_damage.write(AiShipDamaged {
                        target: entity,
                        source: DamageSource::Collision,
                        amount,
                        position: Some(contact),
                        direction: Some(into),
                        // Rams aggro: the other ship is the attacker if it
                        // IS a ship (player or AI root), not scenery.
                        attacker: (other_rammed && (player.contains(other) || ai.contains(other)))
                            .then_some(other),
                    });
                }
            }
        }

        // Hard hits get a scrape flash; the player also feels a kick.
        if approach > IMPACT_FX_SPEED {
            crate::combat::spawn_hit_effect(
                &mut commands,
                contact,
                Color::srgba(0.75, 0.82, 0.95, 0.85),
                (approach / 70.0).clamp(8.0, 18.0),
            );
            if player.contains(a) || player.contains(b) {
                let kick = ((approach - IMPACT_FX_SPEED) / 80.0).clamp(1.5, 8.0);
                camera.shake_intensity = (camera.shake_intensity + kick).min(10.0);
            }
        }
    }
}

#[cfg(test)]
mod steering_tests {
    use super::*;

    #[test]
    fn a_shape_reaches_as_far_as_the_part_that_lies_your_way() {
        // A hub and one arm out to the east.
        let mut shape = vec![(Vec2::ZERO, 500.0)];
        shape.extend((1..=10).map(|i| (Vec2::X * (i as f32 * 400.0), 200.0)));
        let east = hull_reach(&shape, Vec2::X, 100.0).unwrap();
        assert!((east - 4_200.0).abs() < 1.0, "{east}");
        // Any other way, only the hub.
        for dir in [Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y] {
            assert!((hull_reach(&shape, dir, 100.0).unwrap() - 500.0).abs() < 1.0);
        }
        // A wide enough strip catches the arm's first parts from the north-east.
        let ne = Vec2::new(1.0, 1.0).normalize();
        assert!(hull_reach(&shape, ne, 100.0).unwrap() < hull_reach(&shape, ne, 1_500.0).unwrap());
        assert!(hull_reach(&[], Vec2::X, 100.0).is_none());
    }

    fn rock(x: f32, y: f32, radius: f32) -> Blocker {
        Blocker { center: Vec2::new(x, y), radius }
    }

    #[test]
    fn nothing_in_the_way_is_flown_straight_at() {
        // One behind, one well off to the side, one past where it looks.
        let rocks = [rock(-800.0, 0.0, 200.0), rock(1_000.0, 900.0, 200.0), rock(6_000.0, 0.0, 200.0)];
        let (dir, side) = bend_around(Vec2::ZERO, Vec2::X, 2_000.0, 150.0, &rocks, 0.0);
        assert_eq!(dir, Vec2::X);
        assert_eq!(side, 0.0);
    }

    #[test]
    fn a_rock_ahead_is_passed_on_the_side_it_is_not() {
        // A little to the left of the line: go right.
        let (dir, side) = bend_around(Vec2::ZERO, Vec2::X, 2_000.0, 150.0, &[rock(1_000.0, 250.0, 300.0)], 0.0);
        assert!(dir.y < -0.1, "{dir}");
        assert_eq!(side, -1.0);
        let (dir, side) = bend_around(Vec2::ZERO, Vec2::X, 2_000.0, 150.0, &[rock(1_000.0, -250.0, 300.0)], 0.0);
        assert!(dir.y > 0.1, "{dir}");
        assert_eq!(side, 1.0);
    }

    /// The station's bounding circle is mostly open space: a ship between
    /// two of its arms, with nothing ahead, is not steered at all.
    #[test]
    fn a_ship_inside_a_stations_bound_but_clear_of_its_parts_flies_straight() {
        let mut circles = vec![(Vec2::ZERO, 2_000.0)];
        for dir in [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y] {
            circles.extend((1..=12).map(|i| (dir * (2_000.0 + i as f32 * 1_000.0), 600.0)));
        }
        let station = Collider::circles(circles, f32::INFINITY);
        assert!(station.bound_radius > 14_000.0);
        // North-east of the hub, between the east and north arms, flying
        // further out between them.
        let pos = Vec2::new(6_000.0, 6_000.0);
        let desired = Vec2::new(1.0, 1.0).normalize();
        let mut blockers = Vec::new();
        collider_blockers(&Transform::default(), &station, pos + desired * 1_000.0, 1_000.0 + 300.0 + STEER_SKIN, &mut blockers);
        let (dir, _) = bend_around(pos, desired, 2_000.0, 300.0, &blockers, 0.0);
        assert!(dir.dot(desired) > 0.999, "bent to {dir} with nothing in the way");
        // The old way, round the bound: it is inside it, so always pushed.
        let whole = [Blocker { center: Vec2::ZERO, radius: station.bound_radius }];
        assert!(bend_around(pos, desired, 2_000.0, 300.0, &whole, 0.0).0.dot(desired) < 0.99);
    }

    #[test]
    fn two_rocks_too_close_to_thread_are_rounded_as_one() {
        // 300 u of gap between them, dead ahead; the hull is 600 long and
        // some 300 wide, and wants its 60 either side.
        let rocks = [rock(1_500.0, 350.0, 200.0), rock(1_500.0, -350.0, 200.0)];
        let (dir, side) = bend_around(Vec2::ZERO, Vec2::X, 2_500.0, 300.0, &rocks, 0.0);
        assert!(side != 0.0);
        // Round the outside of whichever it chose, not at the gap.
        assert!(dir.y.abs() > 0.3, "{dir}: making for the gap");
        // In among a pack already, it steers by the one in front of it, not
        // out of the whole pack: here the nearest is to its right, so left.
        let pack = [rock(400.0, -300.0, 200.0), rock(-100.0, -500.0, 200.0), rock(-100.0, 500.0, 200.0)];
        let (dir, _) = bend_around(Vec2::ZERO, Vec2::X, 2_500.0, 300.0, &pack, 0.0);
        assert!(dir.y > 0.0 && dir.x > 0.0, "{dir}: turned back out of the pack");
        // Far enough apart, it goes between: each is off the line.
        let wide = [rock(1_500.0, 1_200.0, 200.0), rock(1_500.0, -1_200.0, 200.0)];
        assert_eq!(bend_around(Vec2::ZERO, Vec2::X, 2_500.0, 300.0, &wide, 0.0).0, Vec2::X);
    }

    /// Flown for four seconds at something dead ahead, a ship goes round one
    /// side of it and stays on that side.
    #[test]
    fn dead_ahead_keeps_the_side_it_picked() {
        let rocks = [rock(3_000.0, 0.0, 500.0)];
        let goal = Vec2::new(8_000.0, 0.0);
        let (mut pos, mut hint, mut first) = (Vec2::ZERO, 0.0, 0.0);
        for frame in 0..600 {
            let to = goal - pos;
            if to.length() < 300.0 {
                break;
            }
            let (dir, side) = bend_around(pos, to.normalize(), look_ahead(400.0, 200.0, to.length()), 200.0, &rocks, hint);
            if side != 0.0 {
                if first == 0.0 {
                    first = side;
                }
                assert_eq!(side, first, "changed sides at frame {frame}, at {pos}");
                hint = side;
            }
            pos += dir * 400.0 / 60.0;
            assert!(pos.distance(rocks[0].center) > 500.0 + 200.0 * HULL_SHARE - 1.0, "flew into it at frame {frame}");
        }
        assert!(pos.x > 3_500.0, "never got past: at {pos}");
    }

    #[test]
    fn a_faster_ship_looks_further_but_never_past_its_point() {
        assert!(look_ahead(1_100.0, 300.0, 1.0e6) > look_ahead(350.0, 300.0, 1.0e6));
        assert!(look_ahead(350.0, 300.0, 1.0e6) < 2_200.0, "harbour speed looks no further than it used to");
        assert_eq!(look_ahead(1_100.0, 300.0, 400.0), 400.0);
    }
}

#[cfg(test)]
mod shot_terrain_tests {
    use super::*;

    #[test]
    fn a_shot_in_the_open_space_of_a_big_body_flies_on() {
        // Two far-apart parts: a hub at the origin and an arm 20 km out.
        let c = Collider {
            circles: vec![(Vec2::ZERO, 1_000.0), (Vec2::new(20_000.0, 0.0), 1_000.0)],
            bound_center: Vec2::new(10_000.0, 0.0),
            bound_radius: 11_000.0,
            mass: f32::INFINITY,
            awake: false,
        };
        let tf = Transform::default();
        let centre = world_point(&tf, c.bound_center);
        // Between them: inside the bound, outside both parts.
        assert!(!shot_meets(&c, &tf, centre, Vec2::new(10_000.0, 0.0)));
        // On a part: it stops.
        assert!(shot_meets(&c, &tf, centre, Vec2::new(19_500.0, 300.0)));
        assert!(shot_meets(&c, &tf, centre, Vec2::new(200.0, 0.0)));
        // Outside the bound altogether.
        assert!(!shot_meets(&c, &tf, centre, Vec2::new(40_000.0, 0.0)));
    }

    #[test]
    fn a_beam_inside_a_big_body_reaches_its_parts_not_its_bound() {
        let parts = [(Vec2::ZERO, 1_000.0), (Vec2::new(20_000.0, 0.0), 1_000.0)];
        let bound = (Vec2::new(10_000.0, 0.0), 11_000.0);
        let first = |from: Vec2, to: Vec2| -> Option<f32> {
            let (d, len) = ((to - from).normalize(), from.distance(to));
            segment_meets_circle(from, d, len, bound.0, bound.1)?;
            parts.iter().filter_map(|(c, r)| segment_meets_circle(from, d, len, *c, *r)).min_by(f32::total_cmp)
        };
        // Fired across open space inside the bound, away from both parts: the
        // whole beam, not a stub at the muzzle.
        assert_eq!(first(Vec2::new(10_000.0, 0.0), Vec2::new(10_000.0, 8_000.0)), None);
        // Fired at the arm: stops on its face, 1 km short of its centre.
        let t = first(Vec2::new(10_000.0, 0.0), Vec2::new(25_000.0, 0.0)).unwrap();
        assert!((t - 9_000.0).abs() < 1.0, "{t}");
    }
}

