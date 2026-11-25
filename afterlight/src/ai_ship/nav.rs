//! Where AI ships are trying to go, and whether they are getting there.
//!
//! `AFTERLIGHT_NAV_TRACE=1` prints a `NAV` line every ten seconds: how many
//! live ships there are, how many sit inside something solid, and how many
//! have a point to reach and are not closing on it. Those two numbers are
//! what "ships circling at a station" and "ships grinding into a hull" look
//! like from the log, so a change to steering can be judged without watching.

use bevy::prelude::*;

use super::components::*;
use crate::celestial::resources::{GalaxyMap, StarSystemDef, SystemStreamingManager};
use crate::ship::collision::{world_point, Collider, ColliderGrid, TerrainFilter};

/// AFTERLIGHT_NAV_TRACE=1: print the `NAV` lines.
pub fn trace() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_NAV_TRACE").ok().as_deref() == Some("1"))
}

/// Seconds between `NAV` lines.
const CENSUS_S: f32 = 10.0;
/// A ship this far from its point still has somewhere to go.
const STILL_GOING: f32 = 3_000.0;
/// ...and one that closed less than this in a census has stalled.
const CLOSED_ENOUGH: f32 = 800.0;
/// The point is the same one as last time if it moved less than this.
const SAME_POINT: f32 = 2_000.0;

/// Whether a ship with `before` and `now` to run to its point, and its point
/// `moved` since, has stalled: it has a way to go and isn't making it.
pub fn stalled(before: f32, now: f32, moved: f32) -> bool {
    moved < SAME_POINT && now > STILL_GOING && before > STILL_GOING && before - now < CLOSED_ENOUGH
}

/// Whether `at` is inside one of `collider`'s circles.
pub fn inside_collider(at: Vec2, transform: &Transform, collider: &Collider) -> bool {
    if at.distance_squared(world_point(transform, collider.bound_center)) >= collider.bound_radius.powi(2) {
        return false;
    }
    collider.circles.iter().any(|(c, r)| at.distance_squared(world_point(transform, *c)) < r * r)
}

#[derive(Default)]
pub struct Census {
    next: f32,
    /// Each ship's point and how far it was from it, last time.
    last: bevy::platform::collections::HashMap<Entity, (Vec2, f32)>,
}

/// The `NAV` summary line.
#[allow(clippy::type_complexity)]
pub fn nav_census(
    time: Res<Time>,
    mut census: Local<Census>,
    ships: Query<
        (Entity, &Transform, &AiShipBehavior, &AiShipNav, &AiShipState, &AiShipType, Option<&Collider>),
        (With<AiShip>, Without<crate::diplomacy::FactionStation>),
    >,
    grid: Res<ColliderGrid>,
    terrain: Query<(&Transform, &Collider), TerrainFilter>,
) {
    let now = time.elapsed_secs();
    if now < census.next {
        return;
    }
    census.next = now + CENSUS_S;

    let (mut live, mut inside, mut stalled_n, mut detouring, mut stuck) = (0, 0, 0, 0, 0);
    let mut seen: bevy::platform::collections::HashMap<Entity, (Vec2, f32)> = Default::default();
    let mut examples: Vec<String> = Vec::new();
    let mut going: Vec<String> = Vec::new();
    for (entity, transform, behavior, nav, state, faction, collider) in ships.iter() {
        if *behavior == AiShipBehavior::Dead || state.is_destroyed {
            continue;
        }
        live += 1;
        if nav.route_side != 0.0 {
            detouring += 1;
        }
        if nav.progress.strikes > 0 {
            stuck += 1;
        }
        let at = collider.map_or(transform.translation.truncate(), |c| world_point(transform, c.bound_center));
        let in_something = grid
            .candidates(at, 1.0)
            .filter_map(|e| terrain.get(e).ok())
            .any(|(tf, c)| inside_collider(at, tf, c));
        if in_something {
            inside += 1;
            if examples.len() < 3 {
                examples.push(format!("{faction:?} inside something at {:.0},{:.0}", at.x, at.y));
            }
        }
        // Going somewhere on its own business: a fight has its own ideas
        // about distance, and a ship running away has no point to reach.
        let travelling = matches!(
            behavior,
            AiShipBehavior::Patrolling | AiShipBehavior::FollowingTradeRoute | AiShipBehavior::Salvaging
        );
        let Some(point) = nav.destination.filter(|_| travelling) else { continue };
        let to_go = point.distance(at);
        // A few of them in full, to follow one across a run of lines.
        if going.len() < 4 && to_go > STILL_GOING {
            let round = match nav.route_side {
                s if s > 0.0 => ", going round to the left",
                s if s < 0.0 => ", going round to the right",
                _ => "",
            };
            going.push(format!("{entity:?} {faction:?} at {:.0},{:.0}, {:.1} km to go to {:.0},{:.0}{round}", at.x, at.y, to_go / 1000.0, point.x, point.y));
        }
        if let Some((was_point, was_to_go)) = census.last.get(&entity) {
            if stalled(*was_to_go, to_go, was_point.distance(point)) {
                stalled_n += 1;
                if examples.len() < 3 {
                    examples.push(format!("{faction:?} stalled at {:.0},{:.0}, {:.1} km from its point", at.x, at.y, to_go / 1000.0));
                }
            }
        }
        seen.insert(entity, (point, to_go));
    }
    census.last = seen;
    info!("NAV live={live} detouring={detouring} stuck={stuck} inside={inside} stalled={stalled_n} {examples:?}");
    if !going.is_empty() {
        info!("NAV going {going:?}");
    }
}

// ============================================================================
// THE ROUTE ROUND BIG THINGS
// ============================================================================

/// Something a route goes round: a star, a planet, a station.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavBody {
    pub center: Vec2,
    pub radius: f32,
    /// How it is moving: a planet goes round its star, and a starport with it.
    pub vel: Vec2,
    /// A star or a planet: nothing is ever inside it, and a route goes
    /// round it. A station is not. Ships have business in the space between
    /// its arms, and the circle round the whole of one is no use to plan by:
    /// Haven's reaches most of the way to its own star, so a way round both
    /// had no gap to go through and sent traders off round the far side of
    /// the sun. A ship meets a station's parts as it comes to them
    /// (ship::collision::terrain_blockers). Stations are kept here for where
    /// they are and how they are moving.
    pub solid: bool,
}

/// The big things in the systems that are running, kept up to date a few
/// times a second for whoever is planning a way through.
#[derive(Resource, Default)]
pub struct NavBodies {
    /// The system you are in, and what is in it.
    pub system: Option<u32>,
    pub loaded: Vec<NavBody>,
    /// The neighbouring systems still being simulated.
    pub warm: bevy::platform::collections::HashMap<u32, Vec<NavBody>>,
    next: f32,
    /// Where the starport was last time, for how fast it is moving.
    port_was: Option<(Vec2, f32)>,
}

impl NavBodies {
    /// What is in `system`. Nothing, for one that isn't running.
    pub fn of(&self, system: u32) -> &[NavBody] {
        if self.system == Some(system) {
            &self.loaded
        } else {
            self.warm.get(&system).map_or(&[], |b| b.as_slice())
        }
    }
}

/// The star, planets and stations of a system at galaxy time `t`, worked out
/// from its seed alone: the same answer whether or not it is loaded.
/// `station_reach` says how far the station with a given index reaches.
pub fn system_bodies(def: &StarSystemDef, t: f32, station_reach: impl Fn(usize) -> f32) -> Vec<NavBody> {
    let roll = crate::celestial::galaxy::system_roll(def);
    let mut out = vec![NavBody { center: def.local_center, radius: roll.star_radius(), vel: Vec2::ZERO, solid: true }];
    for planet in &roll.planets {
        let at = planet.offset_at(t);
        out.push(NavBody { center: def.local_center + at, radius: planet.radius, vel: planet.offset_at(t + 1.0) - at, solid: true });
    }
    for site in crate::world::home_base::station_sites(def) {
        out.push(NavBody { center: site.pos, radius: station_reach(site.index), vel: Vec2::ZERO, solid: false });
    }
    out
}

/// Seconds between looks at where everything is.
const BODIES_S: f32 = 0.25;

/// Keep `NavBodies` current.
pub fn refresh_nav_bodies(
    time: Res<Time>,
    clock: Res<crate::celestial::resources::GalaxyState>,
    map: Res<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
    ports: Res<crate::diplomacy::stations::FactionStations>,
    hulls: Query<(&crate::world::home_base::Station, &Collider)>,
    mut bodies: ResMut<NavBodies>,
) {
    let now = time.elapsed_secs();
    if now < bodies.next && bodies.system == streaming.loaded_system {
        return;
    }
    bodies.next = now + BODIES_S;
    let t = clock.galaxy_time as f32;
    let extent = crate::world::home_base::station_extent;
    let def = |id: u32| map.systems.iter().find(|s| s.id == id);

    bodies.system = streaming.loaded_system;
    bodies.loaded = match streaming.loaded_system.and_then(def) {
        // A station as it was built, where it has been: its bound is a good
        // deal less than the most its kind could reach.
        Some(system) => system_bodies(system, t, |index| {
            hulls.iter().find(|(s, _)| s.index == index).map_or_else(|| extent(index), |(_, c)| c.bound_radius)
        }),
        None => Vec::new(),
    };
    let port = streaming.loaded_system.and_then(|id| ports.harbour(id));
    if let Some((at, ring)) = port {
        let vel = match bodies.port_was {
            Some((was, then)) if now > then => (at - was) / (now - then),
            _ => Vec2::ZERO,
        };
        // A jump, or a different system's port: not a speed.
        let vel = if vel.length() > 2_000.0 { Vec2::ZERO } else { vel };
        bodies.loaded.push(NavBody { center: at, radius: ring, vel, solid: false });
    }
    bodies.port_was = port.map(|(at, _)| (at, now));

    bodies.warm.clear();
    for id in streaming.warm_systems.iter().copied() {
        if let Some(system) = def(id) {
            bodies.warm.insert(id, system_bodies(system, t, extent));
        }
    }
}

/// How much wider than a body's padded edge a route passes it.
const ROUTE_WIDE: f32 = 1.04;
/// A route's next point is never nearer than this: close to the body the
/// tangent point is a few units away, and a point that near swings about.
const AIM_MIN: f32 = 1_500.0;

/// Where to make for next on the way from `from` to `to`, keeping `pad`
/// clear of every body: `to` itself when the way is open, else a point
/// that takes it round the first body in the way. Also the side it goes:
/// +1 left of the straight line, -1 right, 0 for straight there.
///
/// Nothing is kept from one call to the next but `side_hint` (the side it
/// went last time): the answer is worked out afresh from where things are,
/// so a planet moving on, or the ship being pushed off its line, can't
/// leave it following a path that is no longer there.
pub fn route_aim(from: Vec2, to: Vec2, pad: f32, bodies: &[NavBody], side_hint: f32) -> (Vec2, f32) {
    route_leg(from, to, pad, bodies, side_hint, 2)
}

fn route_leg(from: Vec2, to: Vec2, pad: f32, bodies: &[NavBody], side_hint: f32, depth: u32) -> (Vec2, f32) {
    // Somewhere inside a star or a planet is taken at its edge.
    let to = settle_goal(from, to, pad, bodies);
    let leg = to - from;
    let len = leg.length();
    if len < 1.0 {
        return (to, 0.0);
    }
    let dir = leg / len;

    let mut first: Option<(f32, NavBody)> = None;
    for body in bodies {
        let room = body.radius + pad;
        let from_in = from.distance_squared(body.center) < room * room;
        if from_in && body.solid {
            // Too close to a star already: straight out before anything else.
            let out = (from - body.center).normalize_or(-dir);
            return (body.center + out * (room * ROUTE_WIDE + AIM_MIN), 0.0);
        }
        if !body.solid {
            continue;
        }
        let Some(t) = crate::ship::collision::segment_meets_circle(from, dir, len, body.center, room) else { continue };
        if first.map_or(true, |(best, _)| t < best) {
            first = Some((t, *body));
        }
    }
    let Some((_, body)) = first else { return (to, 0.0) };

    // The two ways round: along each line from here that just touches it.
    let room = (body.radius + pad) * ROUTE_WIDE;
    let to_body = body.center - from;
    let d = to_body.length().max(1.0);
    let toward = to_body / d;
    let spread = (room / d).min(1.0).asin();
    let run = (d * d - room * room).max(0.0).sqrt();
    let mut best: Option<(f32, Vec2, f32)> = None; // (cost, heading, side)
    for turn in [spread, -spread] {
        let heading = Vec2::from_angle(turn).rotate(toward);
        let touch = from + heading * run;
        let side = if dir.perp_dot(heading) >= 0.0 { 1.0 } else { -1.0 };
        let mut cost = run + touch.distance(to);
        // Not between two bodies with no room between them: this way to
        // the touching point runs into another one.
        // ...or the other one stands against this one on that side, so the
        // way on round it is shut a little further along.
        let shut = bodies.iter().any(|other| {
            if !other.solid || *other == body {
                return false;
            }
            let across = dir.perp_dot(other.center - body.center) * side;
            let against = across > 0.0 && other.center.distance(body.center) < other.radius + pad + room;
            against || crate::ship::collision::segment_meets_circle(from, heading, run.max(1.0), other.center, other.radius + pad).is_some()
        });
        if shut {
            cost *= 3.0;
        }
        // Behind something that is moving, not across its bows.
        if body.vel.length() > 20.0 && (touch - body.center).dot(body.vel) < 0.0 {
            cost *= 0.85;
        }
        // The side it is already going, unless the other is a good deal shorter.
        if side_hint != 0.0 && side == side_hint.signum() {
            cost *= 0.8;
        }
        if best.map_or(true, |(c, ..)| cost < c) {
            best = Some((cost, heading, side));
        }
    }
    let (_, heading, side) = best.unwrap();
    let aim = from + heading * run.max(AIM_MIN);
    // Something else between here and there: round that first.
    if depth > 0 {
        let others: Vec<NavBody> = bodies.iter().copied().filter(|b| *b != body).collect();
        // The side stays this body's: it is what the hint is kept for.
        let (sooner, sooner_side) = route_leg(from, aim, pad, &others, side_hint, depth - 1);
        if sooner_side != 0.0 {
            return (sooner, side);
        }
    }
    (aim, side)
}

/// `to`, or the nearest point to it outside every star and planet, on the
/// side `from` is coming from where it has the choice.
fn settle_goal(from: Vec2, to: Vec2, pad: f32, bodies: &[NavBody]) -> Vec2 {
    let mut goal = to;
    for body in bodies.iter().filter(|b| b.solid) {
        let room = body.radius + pad;
        if goal.distance_squared(body.center) < room * room {
            let out = (goal - body.center).try_normalize().unwrap_or((from - body.center).normalize_or(Vec2::X));
            goal = body.center + out * (room + 1.0);
        }
    }
    goal
}

/// Whether `at` is inside a star or a planet, or within `pad` of one.
pub fn in_a_body(at: Vec2, pad: f32, bodies: &[NavBody]) -> bool {
    bodies.iter().any(|b| b.solid && at.distance_squared(b.center) < (b.radius + pad).powi(2))
}

/// `at`, moved out of every star and planet with `pad` to spare.
pub fn push_clear(at: Vec2, pad: f32, bodies: &[NavBody]) -> Vec2 {
    let mut point = at;
    // A few passes settle a point pushed out of one body into another.
    for _ in 0..4 {
        let mut moved = false;
        for body in bodies.iter().filter(|b| b.solid) {
            let room = body.radius + pad;
            let off = point - body.center;
            if off.length_squared() < room * room {
                // Out of a planet's way to the side, not ahead of it: pushed
                // straight out from its middle, something in front of it was
                // carried along for as long as the planet kept coming.
                let out = if body.vel.length() > 20.0 {
                    let across = body.vel.perp().normalize();
                    if off.dot(across) < 0.0 { -across } else { across }
                } else {
                    off.normalize_or(Vec2::X)
                };
                point = body.center + out * (room + 1.0);
                moved = true;
            }
        }
        if !moved {
            return point;
        }
    }
    // Still inside something: two bodies whose keep-out rooms overlap pass
    // a point back and forth between them. Out past both, from between.
    let over: Vec<&NavBody> = bodies.iter().filter(|b| b.solid && point.distance(b.center) < b.radius + pad).collect();
    if over.is_empty() {
        return point;
    }
    let middle = over.iter().map(|b| b.center).sum::<Vec2>() / over.len() as f32;
    let out = (at - middle).normalize_or(Vec2::Y);
    let reach = bodies
        .iter()
        .filter(|b| b.solid)
        .map(|b| b.center.distance(middle) + b.radius + pad)
        .fold(0.0, f32::max);
    // Straight out from between them, to the first place clear of everything.
    let mut d = 0.0;
    while d < reach {
        let candidate = middle + out * d;
        if !in_a_body(candidate, pad, bodies) {
            return candidate;
        }
        d += 500.0;
    }
    middle + out * (reach + 1.0)
}

/// One tick of a ship or fleet nobody is watching: turn toward the next
/// point on its way to `dest`, move, and get out from under anything that
/// has rolled over it.
///
/// It was a straight line to `dest`, through whatever star or planet lay
/// between. And a planet does 150 u/s where most fleets cruise at half
/// that, so going round is not enough by itself: it is moved clear as well.
pub fn sim_step(pos: &mut Vec2, vel: &mut Vec2, dest: Vec2, cruise: f32, pad: f32, bodies: &[NavBody], dt: f32) {
    let (aim, _) = route_aim(*pos, dest, pad, bodies, 0.0);
    let want = (aim - *pos).normalize_or_zero() * cruise;
    *vel = (*vel * 0.75 + want * 0.25).clamp_length_max(cruise);
    *pos = push_clear(*pos + *vel * dt, pad, bodies);
}

// ============================================================================
// STUCK
// ============================================================================

/// How a ship is getting on toward the point it has been given.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavProgress {
    /// The point, and the nearest it has been to it.
    pub goal: Vec2,
    pub best: f32,
    /// Where it was when it last made headway, and how long ago.
    pub anchor: Vec2,
    pub since: f32,
    /// How many times running it has stalled on this point.
    pub strikes: u8,
    /// How far it had to go at the last stall.
    pub struck_at: f32,
    /// Seconds left of backing straight out of whatever it is against.
    pub backing: f32,
    /// Seconds left of working along whatever it is against instead of
    /// making straight for its point; the sign is the side, + for left.
    pub sidestep: f32,
    /// A station it stalled against on its way somewhere else: it goes
    /// round the whole of it, as it would a planet, until it is past.
    pub around: Option<Entity>,
    /// An escort that couldn't get to its slot waits where it is until the
    /// slot has moved from here: where the slot was when it gave up.
    pub hold: Option<Vec2>,
}

/// Seconds without headway before a ship counts as stuck.
pub const STUCK_S: f32 = 6.0;
/// Getting this much nearer its point is headway.
const HEADWAY: f32 = 150.0;
/// ...and so, on a planned way round something, is moving this far: the
/// first of the way round a star goes across the line to the point, not
/// toward it.
const DETOUR_HEADWAY: f32 = 600.0;
/// This much nearer than where it last stalled, and the stalls are forgotten.
const RECOVERED: f32 = 1_500.0;
/// Seconds a stuck ship backs away from what it is against.
pub const BACK_OUT_S: f32 = 3.0;

/// How long, and to which side, a ship that has stalled `strikes` times
/// running works along whatever is in its way: longer each time, and the
/// other side each time, so a pocket it backed out of is tried round one
/// end and then the other. Seconds, signed: + for left.
pub fn sidestep_for(strikes: u8) -> f32 {
    if strikes < 2 {
        return 0.0;
    }
    let side = if strikes % 2 == 0 { 1.0 } else { -1.0 };
    side * (strikes as f32 * 5.0).min(20.0)
}

/// Keep track of a ship's headway; true on the frame it has gone `STUCK_S`
/// without any. `to_go` is how far it is from `goal`, `on_detour` whether
/// it is on a planned way round something, `trying` whether it is meant to
/// be going there at all (not holding a slot, not fighting, not nearly there).
///
/// Stuck is not the same as slow or still: it is told to go somewhere and
/// is not getting there. Pinned against a hull, or circling a point inside
/// one, or shoved back as fast as it comes on.
pub fn stuck_update(p: &mut NavProgress, at: Vec2, goal: Vec2, to_go: f32, on_detour: bool, trying: bool, dt: f32) -> bool {
    if !trying || goal.distance(p.goal) > SAME_POINT {
        // Somewhere new to go, or nowhere: start again.
        let strikes = if trying && goal.distance(p.goal) <= SAME_POINT { p.strikes } else { 0 };
        *p = NavProgress { goal, best: to_go, anchor: at, since: 0.0, strikes, struck_at: to_go, backing: p.backing, sidestep: p.sidestep, around: p.around, hold: p.hold };
        return false;
    }
    p.goal = goal;
    if to_go < p.best - HEADWAY {
        p.best = to_go;
        p.anchor = at;
        p.since = 0.0;
        if p.struck_at - to_go > RECOVERED {
            p.strikes = 0;
        }
    } else if on_detour && at.distance(p.anchor) > DETOUR_HEADWAY {
        p.anchor = at;
        p.since = 0.0;
    } else {
        p.since += dt;
    }
    if p.since < STUCK_S {
        return false;
    }
    p.since = 0.0;
    p.best = to_go;
    p.anchor = at;
    p.struck_at = to_go;
    p.strikes = p.strikes.saturating_add(1);
    true
}

#[cfg(test)]
mod nav_tests {
    use super::*;

    #[test]
    fn a_ship_going_nowhere_is_stalled_and_one_closing_is_not() {
        // 10 km off, 10 km off ten seconds later.
        assert!(stalled(10_000.0, 9_900.0, 0.0));
        // Closing at a few hundred a second.
        assert!(!stalled(10_000.0, 7_000.0, 0.0));
        // Arrived, near enough.
        assert!(!stalled(2_000.0, 1_900.0, 0.0));
        // Given somewhere new to go: not the same journey.
        assert!(!stalled(10_000.0, 9_900.0, 30_000.0));
    }

    #[test]
    fn inside_means_inside_a_part_not_the_bound() {
        // A hub and an arm: the bound is wide, and mostly empty.
        let c = Collider::circles(vec![(Vec2::ZERO, 100.0), (Vec2::new(1_000.0, 0.0), 100.0)], f32::INFINITY);
        let tf = Transform::default();
        assert!(inside_collider(Vec2::new(1_020.0, 10.0), &tf, &c));
        assert!(!inside_collider(Vec2::new(500.0, 400.0), &tf, &c), "open space between the parts");
        assert!(!inside_collider(Vec2::new(5_000.0, 0.0), &tf, &c));
    }

    fn star(x: f32, y: f32, radius: f32) -> NavBody {
        NavBody { center: Vec2::new(x, y), radius, vel: Vec2::ZERO, solid: true }
    }

    /// Fly `route_aim` from `from` to `to` in 500 u steps; the track.
    fn fly(from: Vec2, to: Vec2, pad: f32, bodies: &[NavBody]) -> Vec<Vec2> {
        let (mut pos, mut hint, mut track) = (from, 0.0, vec![from]);
        for _ in 0..4_000 {
            if pos.distance(to) < 600.0 {
                break;
            }
            let (aim, side) = route_aim(pos, to, pad, bodies, hint);
            hint = side;
            pos += (aim - pos).normalize_or_zero() * 500.0;
            track.push(pos);
        }
        track
    }

    #[test]
    fn a_clear_line_is_flown_straight() {
        let bodies = [star(0.0, 50_000.0, 10_000.0)];
        let (aim, side) = route_aim(Vec2::new(-80_000.0, 0.0), Vec2::new(80_000.0, 0.0), 1_000.0, &bodies, 0.0);
        assert_eq!(aim, Vec2::new(80_000.0, 0.0));
        assert_eq!(side, 0.0);
    }

    #[test]
    fn a_route_round_a_star_never_enters_it() {
        let bodies = [star(0.0, 0.0, 60_000.0)];
        let (from, to) = (Vec2::new(-150_000.0, 4_000.0), Vec2::new(150_000.0, -9_000.0));
        let track = fly(from, to, 2_000.0, &bodies);
        assert!(track.last().unwrap().distance(to) < 600.0, "never arrived: at {}", track.last().unwrap());
        let nearest = track.iter().map(|p| p.length()).fold(f32::MAX, f32::min);
        assert!(nearest >= 62_000.0 - 1.0, "came {nearest:.0} from the middle of a star 60,000 across with 2,000 to keep");
        // And not by a mile: it rounds it close.
        let widest = track.iter().map(|p| p.length()).filter(|d| *d < 100_000.0).fold(0.0, f32::max);
        assert!(nearest < 70_000.0 && widest < 100_000.0);
        // Not much longer than the way round has to be.
        let flown: f32 = track.windows(2).map(|w| w[0].distance(w[1])).sum();
        assert!(flown < from.distance(to) * 1.25, "{flown:.0} for a {:.0} crossing", from.distance(to));
    }

    #[test]
    fn it_keeps_to_the_side_it_started_round() {
        let bodies = [star(0.0, 0.0, 60_000.0)];
        // Dead in line with the middle: either side is as good.
        let (mut pos, mut hint, mut first) = (Vec2::new(-150_000.0, 0.0), 0.0, 0.0);
        let to = Vec2::new(150_000.0, 0.0);
        for step in 0..2_000 {
            if pos.distance(to) < 600.0 {
                break;
            }
            let (aim, side) = route_aim(pos, to, 2_000.0, &bodies, hint);
            if side != 0.0 {
                if first == 0.0 {
                    first = side;
                }
                assert_eq!(side, first, "changed sides at step {step}, at {pos}");
            }
            hint = side;
            pos += (aim - pos).normalize_or_zero() * 500.0;
        }
        assert!(first != 0.0 && pos.distance(to) < 600.0);
    }

    #[test]
    fn two_bodies_in_a_row_are_both_gone_round() {
        let bodies = [star(-60_000.0, 0.0, 20_000.0), star(40_000.0, 15_000.0, 30_000.0)];
        let (from, to) = (Vec2::new(-150_000.0, 0.0), Vec2::new(150_000.0, 0.0));
        let track = fly(from, to, 1_500.0, &bodies);
        assert!(track.last().unwrap().distance(to) < 600.0);
        for b in &bodies {
            let nearest = track.iter().map(|p| p.distance(b.center)).fold(f32::MAX, f32::min);
            assert!(nearest >= b.radius + 1_500.0 - 1.0, "{nearest:.0} from a body of {:.0}", b.radius);
        }
    }

    #[test]
    fn a_goal_inside_a_planet_is_taken_at_its_surface() {
        let bodies = [star(0.0, 0.0, 30_000.0)];
        let track = fly(Vec2::new(-100_000.0, 0.0), Vec2::new(5_000.0, 0.0), 1_000.0, &bodies);
        let end = *track.last().unwrap();
        assert!((end.length() - 31_000.0).abs() < 700.0, "stopped {:.0} from its middle", end.length());
        // At the edge nearest the point, wherever it came from: the goal
        // must not move about as the ship does.
        assert!(end.x > 0.0, "ended at {end}");
        assert_eq!(
            settle_goal(Vec2::new(-100_000.0, 0.0), Vec2::new(5_000.0, 0.0), 1_000.0, &bodies),
            settle_goal(Vec2::new(40_000.0, 70_000.0), Vec2::new(5_000.0, 0.0), 1_000.0, &bodies)
        );
        assert!(track.iter().all(|p| p.length() >= 31_000.0 - 501.0));
    }

    /// A station's circle is never something to plan round: bound for a
    /// berth inside it, leaving one, or passing by.
    #[test]
    fn nobody_is_sent_round_the_whole_of_a_station() {
        let station = NavBody { center: Vec2::ZERO, radius: 30_000.0, vel: Vec2::ZERO, solid: false };
        let berth = Vec2::new(-12_000.0, 3_000.0);
        let outside = Vec2::new(-90_000.0, 0.0);
        let far = Vec2::new(90_000.0, 0.0);
        assert_eq!(route_aim(outside, berth, 1_000.0, &[station], 0.0), (berth, 0.0));
        assert_eq!(route_aim(berth, outside, 1_000.0, &[station], 0.0), (outside, 0.0));
        assert_eq!(route_aim(outside, far, 1_000.0, &[station], 0.0), (far, 0.0));
    }

    /// Haven is 75 km from a star 40 km across, and reaches tens of km
    /// itself. From beside it, round the star to a point on the far side is
    /// the short way round the star, whatever the station is doing there.
    #[test]
    fn a_station_by_a_star_does_not_send_ships_the_long_way_round() {
        let sun = star(31_000.0, -68_400.0, 40_000.0);
        let haven = NavBody { center: Vec2::new(-20_450.0, -50.0), radius: 45_000.0, vel: Vec2::ZERO, solid: false };
        // From north of the star to somewhere south-west of it: the west
        // side is the short one.
        let (from, to) = (Vec2::new(33_000.0, -13_000.0), Vec2::new(-20_000.0, -150_000.0));
        let track = fly(from, to, 6_000.0, &[sun, haven]);
        assert!(track.last().unwrap().distance(to) < 600.0);
        let furthest_east = track.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        assert!(furthest_east < 40_000.0, "went east to x = {furthest_east:.0}, the long way round");
        let flown: f32 = track.windows(2).map(|w| w[0].distance(w[1])).sum();
        assert!(flown < from.distance(to) * 1.3, "{flown:.0} for a {:.0} crossing", from.distance(to));
    }

    /// Haven and its star have no room between them. Round Haven, then,
    /// on the side the star isn't.
    #[test]
    fn between_two_bodies_with_no_gap_it_goes_round_the_outside() {
        let sun = star(31_000.0, -68_400.0, 40_000.0);
        // Haven as something to go round: a ship that has stalled against it.
        let haven = star(-20_450.0, -50.0, 45_000.0);
        let (from, to) = (Vec2::new(-110_000.0, -20_000.0), Vec2::new(60_000.0, 10_000.0));
        // The straight line passes south of Haven's middle, so the south
        // side is the nearer one -- and the star shuts it.
        let (aim, side) = route_aim(from, to, 2_500.0, &[sun, haven], 0.0);
        assert!(side != 0.0 && aim.y > from.y, "made for {aim}: between Haven and its star");
        let track = fly(from, to, 2_500.0, &[sun, haven]);
        assert!(track.last().unwrap().distance(to) < 600.0, "never arrived: at {}", track.last().unwrap());
        for body in [sun, haven] {
            let nearest = track.iter().map(|p| p.distance(body.center)).fold(f32::MAX, f32::min);
            assert!(nearest >= body.radius + 2_500.0 - 1.0, "{nearest:.0} from the middle of a body of {:.0}", body.radius);
        }
    }

    #[test]
    fn it_passes_astern_of_a_planet_crossing_its_path() {
        // The planet is dead ahead and moving up the screen: go below it.
        let planet = NavBody { center: Vec2::ZERO, radius: 20_000.0, vel: Vec2::new(0.0, 150.0), solid: true };
        let (aim, side) = route_aim(Vec2::new(-80_000.0, 0.0), Vec2::new(80_000.0, 0.0), 1_000.0, &[planet], 0.0);
        assert!(aim.y < 0.0 && side == -1.0, "crossed its bows: aim {aim}, side {side}");
    }

    #[test]
    fn push_clear_leaves_no_overlap() {
        // Two that overlap each other, and a station (which nothing is pushed out of).
        let bodies = [star(0.0, 0.0, 10_000.0), star(14_000.0, 0.0, 8_000.0), NavBody { center: Vec2::new(0.0, 40_000.0), radius: 9_000.0, vel: Vec2::ZERO, solid: false }];
        for at in [Vec2::new(1_000.0, 0.0), Vec2::new(9_000.0, 500.0), Vec2::new(13_000.0, -2_000.0), Vec2::ZERO] {
            let out = push_clear(at, 500.0, &bodies);
            assert!(!in_a_body(out, 499.0, &bodies), "{at} went to {out}, still inside");
            assert!(out.distance(at) < 40_000.0, "{at} sent {:.0} away", out.distance(at));
        }
        let by_the_station = Vec2::new(0.0, 38_000.0);
        assert_eq!(push_clear(by_the_station, 500.0, &bodies), by_the_station);
        let open = Vec2::new(-60_000.0, 0.0);
        assert_eq!(push_clear(open, 500.0, &bodies), open);
    }

    #[test]
    fn a_sim_fleet_crossing_a_star_goes_round() {
        let bodies = [star(0.0, 0.0, 60_000.0)];
        let (mut pos, mut vel) = (Vec2::new(-120_000.0, 1_000.0), Vec2::ZERO);
        let dest = Vec2::new(120_000.0, 0.0);
        let mut nearest = f32::MAX;
        for _ in 0..3_000 {
            sim_step(&mut pos, &mut vel, dest, 100.0, 2_500.0, &bodies, 2.0);
            nearest = nearest.min(pos.length());
            if pos.distance(dest) < 450.0 {
                break;
            }
        }
        assert!(pos.distance(dest) < 450.0, "never arrived: at {pos}");
        assert!(nearest >= 62_500.0, "went {nearest:.0} from the middle of the star");
    }

    /// A planet does 150 u/s and this fleet 60: it can't get out of the way,
    /// so it is moved out of the way.
    #[test]
    fn a_slow_fleet_is_not_run_over_by_a_planet() {
        let (mut pos, mut vel) = (Vec2::new(0.0, 0.0), Vec2::ZERO);
        let dest = Vec2::new(0.0, 0.0);
        let mut planet = NavBody { center: Vec2::new(-40_000.0, 3_000.0), radius: 15_000.0, vel: Vec2::new(150.0, 0.0), solid: true };
        for _ in 0..600 {
            planet.center += planet.vel * 2.0;
            sim_step(&mut pos, &mut vel, dest, 60.0, 2_000.0, &[planet], 2.0);
            assert!(pos.distance(planet.center) >= 17_000.0, "under the planet: {:.0} from its middle", pos.distance(planet.center));
        }
        // And once it has gone by, the fleet is back where it was going.
        assert!(pos.distance(dest) < 2_500.0, "left {:.0} from its point", pos.distance(dest));
    }

    /// The planner's planets are where the game draws them: the same sum.
    #[test]
    fn a_systems_bodies_are_its_star_planets_and_stations() {
        let galaxy = crate::celestial::galaxy::generate_galaxy_map(42);
        let system = &galaxy.systems[3];
        let roll = crate::celestial::galaxy::system_roll(system);
        let bodies = system_bodies(system, 1_234.0, crate::world::home_base::station_extent);
        assert_eq!(bodies.len(), 1 + roll.planets.len() + crate::world::home_base::STATIONS_PER_SYSTEM);
        assert!(bodies[0].solid && bodies[0].center == system.local_center && bodies[0].radius == roll.star_radius());
        for (body, planet) in bodies[1..].iter().zip(&roll.planets) {
            let from_star = body.center.distance(system.local_center);
            let (near, far) = (planet.orbit_distance * (1.0 - planet.eccentricity), planet.orbit_distance * (1.0 + planet.eccentricity));
            assert!(from_star >= near - 1.0 && from_star <= far + 1.0, "{from_star} outside its orbit {near}..{far}");
            assert!(body.solid && body.vel.length() > 0.0);
        }
        assert!(bodies[1 + roll.planets.len()..].iter().all(|b| !b.solid));
        // A minute on, the planets have moved and nothing else has.
        let later = system_bodies(system, 1_294.0, crate::world::home_base::station_extent);
        assert_eq!(later[0], bodies[0]);
        if !roll.planets.is_empty() {
            assert!(later[1].center.distance(bodies[1].center) > 100.0);
        }
    }

    /// Run `stuck_update` at 60 frames a second over a track; the frames it fired on.
    fn stalls(track: impl Iterator<Item = (Vec2, bool)>, goal: Vec2, trying: bool) -> (Vec<usize>, NavProgress) {
        let mut p = NavProgress::default();
        let mut fired = Vec::new();
        for (frame, (at, on_detour)) in track.enumerate() {
            if stuck_update(&mut p, at, goal, at.distance(goal), on_detour, trying, 1.0 / 60.0) {
                fired.push(frame);
            }
        }
        (fired, p)
    }

    #[test]
    fn a_ship_pinned_for_six_seconds_is_stuck() {
        let goal = Vec2::new(20_000.0, 0.0);
        // Jiggling against a hull, going nowhere.
        let (fired, p) = stalls((0..800).map(|f| (Vec2::new((f % 7) as f32 * 3.0, 0.0), false)), goal, true);
        assert_eq!(fired.len(), 2, "{fired:?}");
        assert!(fired[0] >= 355 && fired[0] <= 365, "first at frame {}", fired[0]);
        assert_eq!(p.strikes, 2);
    }

    #[test]
    fn a_ship_circling_its_point_is_stuck_however_fast_it_goes() {
        let goal = Vec2::ZERO;
        // Round and round 5 km out at 700 u/s: the baseline's trader.
        let (fired, _) = stalls((0..800).map(|f| (Vec2::from_angle(f as f32 * 700.0 / 60.0 / 5_000.0) * 5_000.0, false)), goal, true);
        assert!(!fired.is_empty());
    }

    #[test]
    fn a_ship_getting_there_is_not_stuck_and_neither_is_one_not_trying() {
        let goal = Vec2::new(60_000.0, 0.0);
        // Closing at 100 u/s: slow, but headway.
        assert!(stalls((0..2_000).map(|f| (Vec2::new(f as f32 * 100.0 / 60.0, 0.0), false)), goal, true).0.is_empty());
        // Sitting on its formation slot.
        assert!(stalls((0..2_000).map(|_| (Vec2::ZERO, false)), goal, false).0.is_empty());
    }

    /// The first of the way round a star goes across the line to the point:
    /// no nearer for a long while, and not stuck.
    #[test]
    fn a_ship_on_its_way_round_a_star_is_not_stuck() {
        let goal = Vec2::new(160_000.0, 0.0);
        let arc = |f: usize| Vec2::from_angle(std::f32::consts::PI - f as f32 * 300.0 / 60.0 / 150_000.0) * 150_000.0;
        assert!(stalls((0..3_000).map(|f| (arc(f), true)), goal, true).0.is_empty());
        // The same track with no plan behind it would be a ship going round
        // in circles.
        assert!(!stalls((0..3_000).map(|f| (arc(f), false)), goal, true).0.is_empty());
    }

    #[test]
    fn a_ship_that_keeps_stalling_tries_one_end_then_the_other_for_longer() {
        assert_eq!(sidestep_for(0), 0.0);
        assert_eq!(sidestep_for(1), 0.0, "the first time it only backs out");
        assert!(sidestep_for(2) > 0.0 && sidestep_for(3) < 0.0 && sidestep_for(4) > 0.0);
        assert!(sidestep_for(3).abs() > sidestep_for(2).abs());
        assert_eq!(sidestep_for(200).abs(), 20.0);
    }

    #[test]
    fn a_new_point_or_real_headway_forgets_the_stalls() {
        let mut p = NavProgress::default();
        let goal = Vec2::new(30_000.0, 0.0);
        for _ in 0..400 {
            stuck_update(&mut p, Vec2::ZERO, goal, 30_000.0, false, true, 1.0 / 60.0);
        }
        assert_eq!(p.strikes, 1);
        // Free again and 2 km nearer: forgotten.
        stuck_update(&mut p, Vec2::new(2_000.0, 0.0), goal, 28_000.0, false, true, 1.0 / 60.0);
        assert_eq!(p.strikes, 0);
        for _ in 0..400 {
            stuck_update(&mut p, Vec2::new(2_000.0, 0.0), goal, 28_000.0, false, true, 1.0 / 60.0);
        }
        assert_eq!(p.strikes, 1);
        // Sent somewhere else.
        stuck_update(&mut p, Vec2::new(2_000.0, 0.0), Vec2::new(0.0, 90_000.0), 90_000.0, false, true, 1.0 / 60.0);
        assert_eq!(p.strikes, 0);
    }
}
