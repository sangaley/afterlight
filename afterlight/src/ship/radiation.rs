use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::celestial::components::{Star, CelestialBody};

/// Checks for radiation damage based on proximity to stars.
/// Hull segments with insufficient radiation shielding take damage over time.
/// Radiation intensity comes from nearby stars, not arbitrary depth.
pub fn check_radiation_damage(
    time: Res<Time>,
    config: Res<GameConfig>,
    ship_query: Query<&Transform, With<Ship>>,
    star_query: Query<(&Transform, &Star, &CelestialBody)>,
    mut hull_query: Query<(Entity, &mut HullSegment)>,
    mut death_cause: ResMut<DeathCause>,
    mut breach_events: MessageWriter<HullBreached>,
    mut damage_events: MessageWriter<ShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
    mut warned_50: Local<bool>,
    mut warned_30: Local<bool>,
) {
    let Ok(ship_transform) = ship_query.single() else {
        return;
    };
    let ship_pos = ship_transform.translation.truncate();

    // Calculate total radiation from all nearby stars
    let mut current_radiation = 0.0_f32;
    for (star_transform, star, body) in star_query.iter() {
        let star_pos = star_transform.translation.truncate();
        let dist = ship_pos.distance(star_pos);

        // Radiation falls off with distance squared
        let safe_dist = body.radius * 3.0; // Beyond 3x radius, radiation is minimal
        if dist < safe_dist {
            let proximity = 1.0 - (dist / safe_dist).clamp(0.0, 1.0);
            current_radiation += star.radiation_output * proximity * proximity
                * star.size_class.radiation_multiplier()
                * config.radiation_per_unit;
        }
    }

    // Also add baseline void radiation based on distance from home (radial,
    // matching the zone/danger progression).
    let depth = ship_transform.translation.truncate().length();
    current_radiation += depth * config.radiation_per_unit * 0.1;

    if current_radiation < 0.01 {
        *warned_50 = false;
        *warned_30 = false;
        return;
    }

    let mut took_damage = false;
    for (entity, mut hull) in hull_query.iter_mut() {
        if current_radiation > hull.radiation_shielding {
            took_damage = true;
            let excess = current_radiation - hull.radiation_shielding;
            let radiation_factor = (current_radiation / 100.0).max(1.0);
            let damage = excess * config.radiation_damage_multiplier * radiation_factor * time.delta_secs();

            hull.health -= damage;

            let health_pct = hull.health / hull.max_health;

            if health_pct <= 0.5 && !*warned_50 {
                *warned_50 = true;
                notifications.write(ShowNotification {
                    message: "Hull segment at 50%! Stellar radiation is intense!".into(),
                    notification_type: NotificationType::Warning,
                    duration: 3.0,
                });
            }

            if health_pct <= 0.3 && !*warned_30 {
                *warned_30 = true;
                notifications.write(ShowNotification {
                    message: "Hull segment critical at 30%! Move away from the star!".into(),
                    notification_type: NotificationType::Danger,
                    duration: 3.0,
                });
            }

            if hull.health <= hull.max_health * 0.3 && !hull.is_depressurized {
                hull.is_depressurized = true;
                breach_events.write(HullBreached {
                    segment: entity,
                    severity: 1.0 - (hull.health / hull.max_health),
                });
                damage_events.write(ShipDamaged {
                    source: DamageSource::Radiation,
                    amount: damage,
                    position: None,
                    direction: None,
                    applied: false,
                });
            }

            if hull.health <= 0.0 {
                hull.health = 0.0;
                damage_events.write(ShipDamaged {
                    source: DamageSource::Radiation,
                    amount: 10.0,
                    position: None,
                    direction: None,
                    applied: false,
                });
            }
        } else {
            *warned_50 = false;
            *warned_30 = false;
        }
    }

    if took_damage {
        death_cause.last_damage = Some((
            "radiation (hull shielding exceeded - too far from Haven, or too close to a star)".to_string(),
            time.elapsed_secs_f64(),
        ));
    }
}
