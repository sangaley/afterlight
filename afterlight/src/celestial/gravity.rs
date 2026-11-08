use bevy::prelude::*;
use crate::components::{Ship, Velocity, ShipPhysics};
use crate::events::ShowNotification;
use super::components::*;
use super::resources::*;
use super::events::GravityWarning;

/// Pull, in u/s², at which flying near a body is worth a warning: about a
/// fifth of the starter's thrust, enough to bend a course you aren't steering.
pub const GRAVITY_NOTICE: f32 = 40.0;
/// Pull at which escaping takes most of your engines: two-thirds of thrust.
pub const GRAVITY_SEVERE: f32 = 120.0;

/// A well's pull at `distance` from its centre, before the global clamp.
///
/// Inside the body it pulls at its surface value. A planet or star well is
/// `surface_gravity * r²`, so `strength / d²` is tuned AT the surface and
/// only grows past it: a warp dash that dropped the ship well inside a planet
/// met many times surface pull, up to the 50,000 u/s² clamp -- hundreds of
/// times the engines -- instead of the strong-but-survivable value the body
/// was designed around. (Real gravity falls off toward the centre; holding
/// the surface value is simpler, and never more than the body was tuned for.)
///
/// `body_radius` is the CelestialBody's, 0 for a well without one. Black
/// holes keep their own ramp: crossing the horizon is meant to end you.
pub fn well_pull(well: &GravityWell, distance: f32, body_radius: f32) -> f32 {
    let r = distance.max(body_radius);
    match well.falloff {
        GravityFalloff::InverseSquare => well.strength / (r * r),
        GravityFalloff::InverseLinear => well.strength / r,
        GravityFalloff::BlackHole => {
            // Dramatic ramp: gentle at distance, extreme near event horizon
            let normalized = (distance / well.influence_radius).clamp(0.01, 1.0);
            well.strength / (normalized * normalized * distance)
        }
    }
}

/// Accumulate gravity forces on all GravityAffected entities from all GravityWells.
/// Uses influence_radius to skip distant wells.
pub fn accumulate_gravity(
    config: Res<CelestialConfig>,
    well_query: Query<(&Transform, &GravityWell, Option<&CelestialBody>), Without<BeingConsumed>>,
    mut affected_query: Query<(&Transform, &GravityAffected, &mut GravityForce)>,
) {
    for (affected_transform, _affected, mut gravity_force) in affected_query.iter_mut() {
        let pos = affected_transform.translation.truncate();
        let mut total_force = Vec2::ZERO;

        for (well_transform, well, body) in well_query.iter() {
            let well_pos = well_transform.translation.truncate();
            let delta = well_pos - pos;
            let distance = delta.length();

            // Skip if outside influence radius
            if distance > well.influence_radius || distance < 1.0 {
                continue;
            }

            let direction = delta / distance;
            let force_magnitude = well_pull(well, distance, body.map_or(0.0, |b| b.radius));

            let clamped = force_magnitude.min(config.max_gravity_force);
            total_force += direction * clamped;
        }

        gravity_force.0 = total_force;
    }
}

/// Apply accumulated gravity to velocity for all gravity-affected entities (creatures, debris)
pub fn apply_gravity_to_velocity(
    time: Res<Time>,
    mut query: Query<(&GravityForce, &GravityAffected, &mut Velocity), Without<Ship>>,
) {
    let dt = time.delta_secs();
    // The field IS the acceleration. Every well is built as
    // `surface_gravity * r²`, so `strength / d²` lands directly in u/s², and
    // in real gravity a pebble and a hulk fall at the same rate. This used to
    // divide by `GravityAffected::mass`, which the bodies' masses were tuned
    // as a "how much does gravity touch this" dial -- harmless while the
    // field was tiny, but once wells were given real strength it meant a
    // 0.5-mass projectile bent at TWICE the field while the ship barely felt
    // it. One rule for everything now.
    for (gravity_force, _affected, mut velocity) in query.iter_mut() {
        velocity.0 += gravity_force.0 * dt;
    }
}

/// Apply gravity to the ship — integrates with existing movement system.
/// Also fires GravityWarning events when pull is significant.
pub fn apply_gravity_to_ship(
    time: Res<Time>,
    mut ship_query: Query<(&GravityForce, &mut Velocity, &ShipPhysics), With<Ship>>,
    mut warnings: MessageWriter<GravityWarning>,
    mut notifications: MessageWriter<ShowNotification>,
    well_query: Query<(Entity, &Transform, &GravityWell)>,
    ship_transform_query: Query<&Transform, With<Ship>>,
    mut warned_light: Local<bool>,
    mut warned_heavy: Local<bool>,
) {
    let dt = time.delta_secs();

    let Ok((gravity_force, mut velocity, physics)) = ship_query.single_mut() else {
        return;
    };

    let force_magnitude = gravity_force.0.length();

    // The field is already an acceleration -- see apply_gravity_to_velocity.
    // Dividing by `physics.mass` (1200 for the starter) made the pull about
    // 1/1200th of what every well was tuned for: 0.026 u/s² at Haven's spawn
    // against 180 u/s² of thrust, which is why flying past a star felt like
    // nothing. The mass still matters for thrust, where it should.
    let _ = physics;
    velocity.0 += gravity_force.0 * dt;

    // Warn player when gravity is pulling them. Thresholds are in u/s² against
    // the starter's ~180 of thrust: a noticeable tug, then most of your engines.
    if force_magnitude > GRAVITY_NOTICE && !*warned_light {
        *warned_light = true;
        notifications.write(ShowNotification {
            message: "Gravitational pull detected - watch your trajectory!".into(),
            notification_type: crate::events::NotificationType::Warning,
            duration: 3.0,
        });
    }
    if force_magnitude > GRAVITY_SEVERE && !*warned_heavy {
        *warned_heavy = true;
        notifications.write(ShowNotification {
            message: "EXTREME GRAVITY! Full thrust required to escape!".into(),
            notification_type: crate::events::NotificationType::Danger,
            duration: 4.0,
        });
    }

    // Reset warnings when the pull drops well below the notice line
    if force_magnitude < GRAVITY_NOTICE * 0.6 {
        *warned_light = false;
        *warned_heavy = false;
    }

    // Fire warning events for UI
    if force_magnitude > GRAVITY_NOTICE {
        if let Ok(ship_transform) = ship_transform_query.single() {
            let ship_pos = ship_transform.translation.truncate();
            // Find the strongest gravity source
            if let Some((entity, _, _)) = well_query.iter()
                .min_by(|(_, ta, _), (_, tb, _)| {
                    let da = ta.translation.truncate().distance(ship_pos);
                    let db = tb.translation.truncate().distance(ship_pos);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                warnings.write(GravityWarning {
                    source: entity,
                    pull_strength: force_magnitude,
                });
            }
        }
    }
}

#[cfg(test)]
mod gravity_tests {
    use super::*;

    /// No MinimalPlugins: its TimePlugin rewrites `Time` from the wall clock
    /// and would discard the delta advanced here.
    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_message::<GravityWarning>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, (apply_gravity_to_ship, apply_gravity_to_velocity));
        app
    }

    fn step(app: &mut App, secs: f32) {
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(secs));
        app.update();
    }

    /// The test the last gravity change did not have. That change checked the
    /// FIELD (`strength / d²`) and assumed it was what the ship felt; the
    /// system then divided by the ship's mass, so a pull tuned at 31 u/s²
    /// arrived as 0.026. Here the real system runs on a real-mass ship and the
    /// velocity it produces is what gets measured.
    #[test]
    fn a_ship_accelerates_at_the_full_field_whatever_its_mass() {
        let mut app = app();
        let field = Vec2::new(31.0, 0.0); // Haven's spawn
        let ship = app
            .world_mut()
            .spawn((
                crate::components::Ship,
                GravityForce(field),
                Velocity(Vec2::ZERO),
                ShipPhysics::default(), // mass 1200
                Transform::default(),
            ))
            .id();
        step(&mut app, 1.0);
        let v = app.world().get::<Velocity>(ship).unwrap().0;
        assert!(
            (v - field).length() < 0.01,
            "after 1s in a 31 u/s² field the ship moves at {v}; dividing by its mass of \
             1200 would give about 0.026"
        );
    }

    /// Real gravity: a pebble and a hulk fall together. Before, each body's
    /// `mass` acted as a dial, so a 0.5-mass projectile bent at twice the
    /// field and a 50-mass severed section at a fiftieth of it.
    #[test]
    fn every_body_falls_at_the_same_rate() {
        let mut app = app();
        let field = Vec2::new(0.0, -20.0);
        let bodies: Vec<Entity> = [0.2f32, 0.5, 3.0, 50.0]
            .iter()
            .map(|&m| {
                app.world_mut()
                    .spawn((GravityForce(field), GravityAffected { mass: m }, Velocity(Vec2::ZERO)))
                    .id()
            })
            .collect();
        step(&mut app, 2.0);
        for (i, e) in bodies.iter().enumerate() {
            let v = app.world().get::<Velocity>(*e).unwrap().0;
            assert!(
                (v - field * 2.0).length() < 0.01,
                "body {i} fell at {v}, not the field's {}", field * 2.0
            );
        }
    }

    /// Inside a planet the pull is its surface value, not `strength / d²`
    /// running away toward the clamp. Outside, nothing changes.
    #[test]
    fn inside_a_body_the_pull_is_the_surface_pull() {
        let radius = 30_000.0;
        let surface = 60.0;
        let well = GravityWell {
            strength: surface * radius * radius,
            influence_radius: radius * 5.0,
            falloff: GravityFalloff::InverseSquare,
        };
        for depth in [0.99, 0.5, 0.1, 0.001] {
            let pull = well_pull(&well, radius * depth, radius);
            assert!((pull - surface).abs() < 1e-3, "{:.0}% of the way in: {pull} u/s²", (1.0 - depth) * 100.0);
        }
        let outside = well_pull(&well, radius * 2.0, radius);
        assert!((outside - surface / 4.0).abs() < 1e-3, "two radii out: {outside}");
    }

    /// The warning and HUD lines are in u/s², and sit below the starter's
    /// ~180 of thrust so they mean something: a notice well inside what the
    /// engines can beat, a severe warning before the pull overpowers them.
    #[test]
    fn warnings_sit_inside_what_the_engines_can_beat() {
        const THRUST: f32 = 180.0;
        assert!(GRAVITY_NOTICE < GRAVITY_SEVERE);
        assert!(GRAVITY_SEVERE < THRUST, "a 'severe' pull should still be escapable");
        assert!(GRAVITY_NOTICE > 10.0, "a notice should be a real tug, not noise");
    }
}
