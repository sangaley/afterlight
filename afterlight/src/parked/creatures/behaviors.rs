use bevy::prelude::*;
use crate::components::*;
use crate::celestial::components::{CelestialBody, Planet, Star};

/// Gravity-aware creature behaviors.
/// VoidDrifters gravitate toward planets for warmth.
/// Stalkers position themselves behind celestial bodies relative to the ship.
/// Leviathans patrol between planets.
pub fn gravity_aware_wandering(
    _time: Res<Time>,
    ship_query: Query<&Transform, With<Ship>>,
    planet_query: Query<(&Transform, &CelestialBody), (With<Planet>, Without<Creature>, Without<Ship>)>,
    star_query: Query<(&Transform, &CelestialBody), (With<Star>, Without<Creature>, Without<Ship>)>,
    mut creature_query: Query<(&Transform, &Creature, &mut CreatureAI), Without<Ship>>,
) {
    let ship_pos = ship_query.single()
        .map(|t| t.translation.truncate())
        .unwrap_or(Vec2::ZERO);

    // Collect planet positions
    let planets: Vec<(Vec2, f32)> = planet_query.iter()
        .map(|(t, body)| (t.translation.truncate(), body.radius))
        .collect();

    let _stars: Vec<(Vec2, f32)> = star_query.iter()
        .map(|(t, body)| (t.translation.truncate(), body.radius))
        .collect();

    if planets.is_empty() {
        return;
    }

    for (transform, creature, mut ai) in creature_query.iter_mut() {
        // Only modify wandering/idle creatures (don't override combat decisions)
        if !matches!(ai.state, CreatureAIState::Wandering | CreatureAIState::Idle) {
            continue;
        }

        let pos = transform.translation.truncate();

        match creature.creature_type {
            CreatureType::VoidDrifter => {
                // Drift toward nearest planet (warmth seeking)
                if let Some((planet_pos, planet_radius)) = planets.iter()
                    .min_by(|(a, _), (b, _)| {
                        a.distance(pos).partial_cmp(&b.distance(pos)).unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    let dist = pos.distance(*planet_pos);
                    // Cluster at 1.5-3x planet radius
                    if dist > *planet_radius * 3.0 {
                        ai.home_position = *planet_pos + (pos - *planet_pos).normalize_or_zero() * *planet_radius * 2.0;
                    }
                }
            }
            CreatureType::Stalker => {
                // Position behind a planet/asteroid relative to the ship — ambush position
                if let Some((planet_pos, planet_radius)) = planets.iter()
                    .filter(|(p, _)| p.distance(pos) < 50_000.0 && p.distance(ship_pos) < 80_000.0)
                    .min_by(|(a, _), (b, _)| {
                        a.distance(pos).partial_cmp(&b.distance(pos)).unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    // "Behind" = opposite side of planet from ship
                    let planet_to_ship = (ship_pos - *planet_pos).normalize_or_zero();
                    let ambush_pos = *planet_pos - planet_to_ship * (*planet_radius * 1.5);
                    ai.home_position = ambush_pos;
                }
            }
            CreatureType::Leviathan => {
                // Patrol between the two nearest planets
                if planets.len() >= 2 {
                    let mut sorted: Vec<(Vec2, f32)> = planets.clone();
                    sorted.sort_by(|(a, _), (b, _)| {
                        a.distance(pos).partial_cmp(&b.distance(pos)).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let (p1, _) = sorted[0];
                    let (p2, _) = sorted[1];
                    // Alternate home between the two planets
                    let midpoint = (p1 + p2) / 2.0;
                    let to_p1 = p1 - pos;
                    let to_p2 = p2 - pos;
                    // Drift toward whichever planet is farther (creates patrol loop)
                    if to_p1.length() > to_p2.length() {
                        ai.home_position = p1;
                    } else {
                        ai.home_position = p2;
                    }
                    ai.wander_radius = midpoint.distance(p1).min(2000.0);
                }
            }
            CreatureType::ParasiteSwarm => {
                // Attracted to ship energy — drift toward ship when wandering
                let dist_to_ship = pos.distance(ship_pos);
                if dist_to_ship < 1500.0 {
                    // Slowly drift toward ship
                    let toward_ship = ship_pos + (pos - ship_pos).normalize_or_zero() * 500.0;
                    ai.home_position = toward_ship;
                }
            }
        }
    }
}
