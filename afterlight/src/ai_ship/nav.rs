//! Where AI ships are trying to go, and whether they are getting there.
//!
//! `AFTERLIGHT_NAV_TRACE=1` prints a `NAV` line every ten seconds: how many
//! live ships there are, how many sit inside something solid, and how many
//! have a point to reach and are not closing on it. Those two numbers are
//! what "ships circling at a station" and "ships grinding into a hull" look
//! like from the log, so a change to steering can be judged without watching.

use bevy::prelude::*;

use super::components::*;
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

    let (mut live, mut inside, mut stalled_n) = (0, 0, 0);
    let mut seen: bevy::platform::collections::HashMap<Entity, (Vec2, f32)> = Default::default();
    let mut examples: Vec<String> = Vec::new();
    for (entity, transform, behavior, nav, state, faction, collider) in ships.iter() {
        if *behavior == AiShipBehavior::Dead || state.is_destroyed {
            continue;
        }
        live += 1;
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
    info!("NAV live={live} inside={inside} stalled={stalled_n} {examples:?}");
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
}
