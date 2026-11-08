use bevy::prelude::*;
use rand::Rng;
use crate::components::Ship;
use crate::events::{ShipDamaged, DamageSource, ShowNotification, NotificationType};
use super::components::*;
use super::resources::*;
use super::events::*;

/// Random flare buildup. NOT timed — uses random accumulation rate per star.
/// When buildup crosses the star's random threshold, a flare fires.
pub fn star_flare_buildup(
    time: Res<Time>,
    config: Res<CelestialConfig>,
    mut star_query: Query<(Entity, &Transform, &mut Star, &CelestialBody)>,
    mut flare_events: MessageWriter<RadiationFlare>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (entity, transform, mut star, body) in star_query.iter_mut() {
        if star.is_dying {
            continue;
        }

        // Random buildup rate each frame (creates unpredictable timing)
        let rate = rng.gen_range(config.flare_buildup_rate_min..config.flare_buildup_rate_max);
        star.flare_buildup += rate * dt * star.size_class.radiation_multiplier();

        // Fire flare when buildup crosses threshold
        if star.flare_buildup >= star.flare_threshold {
            star.flare_buildup = 0.0;
            // Randomize next threshold for unpredictability
            star.flare_threshold = rng.gen_range(0.7..0.95);

            let intensity = star.size_class.flare_intensity_multiplier();
            let flare_radius = body.radius * 3.0 + intensity * 10_000.0;

            flare_events.write(RadiationFlare {
                star: entity,
                intensity,
                position: transform.translation.truncate(),
                radius: flare_radius,
            });

            notifications.write(ShowNotification {
                message: format!("SOLAR FLARE from {}! Radiation spike!", body.name),
                notification_type: NotificationType::Danger,
                duration: 4.0,
            });
        }
    }
}

/// Apply radiation flare damage to ship if in range.
/// Uses existing radiation damage pipeline via ShipDamaged events.
pub fn apply_flare_radiation(
    time: Res<Time>,
    config: Res<CelestialConfig>,
    mut flare_events: MessageReader<RadiationFlare>,
    ship_query: Query<&Transform, With<Ship>>,
    mut damage_events: MessageWriter<ShipDamaged>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    for flare in flare_events.read() {
        let dist = ship_pos.distance(flare.position);
        if dist > flare.radius {
            continue;
        }

        // Damage falls off with distance from star
        let proximity = 1.0 - (dist / flare.radius).clamp(0.0, 1.0);
        let damage = flare.intensity * config.flare_radiation_multiplier * proximity * time.delta_secs();

        if damage > 0.1 {
            damage_events.write(ShipDamaged {
                source: DamageSource::Radiation,
                amount: damage,
                position: None,
                direction: None,
                applied: false,
            });
        }
    }
}

/// Star death sequence: story-triggered via health reduction.
/// When star health reaches 0, countdown begins. When timer expires → supernova.
pub fn star_death_check(
    time: Res<Time>,
    mut commands: Commands,
    mut star_query: Query<(Entity, &Transform, &mut Star, &mut CelestialBody)>,
    mut destroyed_events: MessageWriter<StarDestroyed>,
    orbit_query: Query<(Entity, &OrbitalPath)>,
    mut galaxy: ResMut<GalaxyState>,
    config: Res<CelestialConfig>,
    mut notifications: MessageWriter<ShowNotification>,
    mut shockwave_events: MessageWriter<SupernovaShockwave>,
) {
    let dt = time.delta_secs();

    for (entity, transform, mut star, mut body) in star_query.iter_mut() {
        if !star.is_dying {
            continue;
        }

        star.death_timer -= dt;

        // Countdown warnings
        if star.death_timer > 0.0 && star.death_timer < 3.0 {
            // Visual: star could pulse/grow here in a future visual system
        }

        if star.death_timer <= 0.0 {
            let pos = transform.translation.truncate();

            // Collect freed planets
            let freed: Vec<Entity> = orbit_query.iter()
                .filter(|(_, orbit)| orbit.parent == entity)
                .map(|(e, _)| e)
                .collect();

            destroyed_events.write(StarDestroyed {
                star: entity,
                position: pos,
                supernova_radius: config.star_death_supernova_radius,
                freed_planets: freed,
            });

            shockwave_events.write(SupernovaShockwave {
                origin: pos,
                damage: config.supernova_damage,
                radius: config.star_death_supernova_radius,
            });

            // Mark system as dead
            for system in galaxy.systems.iter_mut() {
                if system.star_entity == Some(entity) {
                    system.is_alive = false;
                }
            }

            // Transform star into a black hole
            body.body_type = CelestialBodyType::BlackHole;
            body.radius = 2_000.0; // Visual collapse
            star.is_dying = false; // Stop ticking death

            commands.entity(entity)
                .remove::<Star>()
                .insert(BlackHole {
                    event_horizon_radius: 3_000.0,
                    accretion_disk_radius: 8_000.0,
                    consumed_mass: 0.0,
                    tidal_force_multiplier: 2.0,
                })
                .insert(GravityWell {
                    strength: body.mass * 3.0, // Black hole gravity is much stronger
                    influence_radius: 60_000.0,
                    falloff: GravityFalloff::BlackHole,
                });

            notifications.write(ShowNotification {
                message: "SUPERNOVA! A black hole has formed!".into(),
                notification_type: NotificationType::Danger,
                duration: 6.0,
            });
        }
    }
}

/// Apply supernova shockwave damage to ship
pub fn apply_supernova_damage(
    mut shockwave_events: MessageReader<SupernovaShockwave>,
    ship_query: Query<&Transform, With<Ship>>,
    mut damage_events: MessageWriter<ShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    for shockwave in shockwave_events.read() {
        let dist = ship_pos.distance(shockwave.origin);
        if dist > shockwave.radius {
            continue;
        }

        let proximity = 1.0 - (dist / shockwave.radius).clamp(0.0, 1.0);
        let damage = shockwave.damage * proximity;

        damage_events.write(ShipDamaged {
            source: DamageSource::Radiation,
            amount: damage,
            position: Some(shockwave.origin),
            direction: Some((ship_pos - shockwave.origin).normalize_or_zero()),
            applied: false,
        });

        notifications.write(ShowNotification {
            message: format!("Supernova shockwave! {:.0} damage!", damage),
            notification_type: NotificationType::Danger,
            duration: 4.0,
        });
    }
}
