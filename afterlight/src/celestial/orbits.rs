use bevy::prelude::*;
use crate::events::{ShowNotification, NotificationType};
use super::components::*;
use super::resources::*;
use super::events::StarDestroyed;

/// Evaluates orbital positions at current galaxy_time using Keplerian math.
/// Cheap: just trig per orbiting body per frame.
pub fn update_orbital_positions(
    time: Res<Time>,
    mut galaxy: ResMut<GalaxyState>,
    parent_query: Query<&Transform, (With<CelestialBody>, Without<OrbitalPath>)>,
    mut orbit_query: Query<(&OrbitalPath, &mut Transform), Without<FreeFlight>>,
) {
    galaxy.galaxy_time += time.delta_secs() as f64;
    let t = galaxy.galaxy_time as f32;

    for (orbit, mut transform) in orbit_query.iter_mut() {
        let Ok(parent_transform) = parent_query.get(orbit.parent) else {
            continue;
        };

        let parent_pos = parent_transform.translation.truncate();

        // Simplified Keplerian: treat as elliptical motion
        let angular_speed = std::f32::consts::TAU / orbit.period;
        let direction = if orbit.clockwise { -1.0 } else { 1.0 };
        let angle = orbit.phase + angular_speed * t * direction;

        let r = orbit_radius(orbit.semi_major_axis, orbit.eccentricity, angle);

        let x = parent_pos.x + r * angle.cos();
        let y = parent_pos.y + r * angle.sin();

        transform.translation.x = x;
        transform.translation.y = y;
    }
}

/// How far from its star an orbiting body is at polar angle `theta`: the
/// ellipse `r = a(1 - e²) / (1 + e·cos θ)`, star at the focus.
pub fn orbit_radius(semi_major_axis: f32, eccentricity: f32, theta: f32) -> f32 {
    semi_major_axis * (1.0 - eccentricity * eccentricity) / (1.0 + eccentricity * theta.cos())
}

/// Points sampled round a planet's path when measuring clearance from it.
/// A path 1,000 km out is checked every ~6 km, so a point can read at most
/// ~3 km further from the path than it is, well inside the margin anything
/// placed against a path keeps (a whole planet radius).
const PATH_SAMPLES: usize = 1024;

/// The tube a planet sweeps: every point of its elliptical orbit, `radius`
/// either side. Planets are solid and go all the way round every few
/// minutes, so anything placed in the tube is eventually run over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetPath {
    pub semi_major_axis: f32,
    pub eccentricity: f32,
    pub radius: f32,
}

impl PlanetPath {
    /// How far `offset` (measured from the star) is from the planet's
    /// surface at its closest pass. Negative when the planet runs over it.
    pub fn clearance(&self, offset: Vec2) -> f32 {
        let step = std::f32::consts::TAU / PATH_SAMPLES as f32;
        let nearest = (0..PATH_SAMPLES)
            .map(|i| {
                let theta = i as f32 * step;
                Vec2::from_angle(theta) * orbit_radius(self.semi_major_axis, self.eccentricity, theta)
            })
            .map(|on_path| on_path.distance(offset))
            .fold(f32::INFINITY, f32::min);
        nearest - self.radius
    }

    /// Whether the planet's surface always passes at least `margin` from
    /// `offset` (measured from the star).
    ///
    /// Same answer as `clearance(offset) >= margin`, but most points a placer
    /// tries are nowhere near a given path, and those are settled without
    /// walking it: the path never leaves the ring between perihelion and
    /// aphelion, so a point well outside that ring is clear outright.
    pub fn clears(&self, offset: Vec2, margin: f32) -> bool {
        let from_star = offset.length();
        let perihelion = self.semi_major_axis * (1.0 - self.eccentricity);
        let aphelion = self.semi_major_axis * (1.0 + self.eccentricity);
        let nearest_possible = (perihelion - from_star).max(from_star - aphelion);
        nearest_possible - self.radius >= margin || self.clearance(offset) >= margin
    }

    /// The furthest the planet's surface ever gets from the star.
    pub fn outer_reach(&self) -> f32 {
        self.semi_major_axis * (1.0 + self.eccentricity) + self.radius
    }
}

/// When a star dies, convert OrbitalPath → FreeFlight with tangential velocity.
/// Planets go flying!
pub fn destabilize_orbits(
    mut commands: Commands,
    mut star_destroyed_events: MessageReader<StarDestroyed>,
    orbit_query: Query<(Entity, &OrbitalPath, &Transform)>,
    _galaxy: Res<GalaxyState>,
    config: Res<CelestialConfig>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for event in star_destroyed_events.read() {
        let star_pos = event.position;

        for (entity, orbit, transform) in orbit_query.iter() {
            if orbit.parent != event.star {
                continue;
            }

            // Calculate tangential velocity at current position
            let pos = transform.translation.truncate();
            let radial = (pos - star_pos).normalize_or_zero();
            // Tangent is perpendicular to radial
            let tangent = if orbit.clockwise {
                Vec2::new(radial.y, -radial.x)
            } else {
                Vec2::new(-radial.y, radial.x)
            };

            // Speed from orbital velocity + explosion kick
            let orbital_speed = std::f32::consts::TAU * orbit.semi_major_axis / orbit.period;
            let kick_speed = config.freed_planet_speed_multiplier;
            let velocity = tangent * orbital_speed + radial * kick_speed;

            // Remove orbit, add free flight
            commands.entity(entity)
                .remove::<OrbitalPath>()
                .insert(FreeFlight { velocity });
        }

        notifications.write(ShowNotification {
            message: "STAR DESTROYED! Orbital bodies destabilized!".into(),
            notification_type: NotificationType::Danger,
            duration: 5.0,
        });
    }
}

/// Move free-flight bodies (freed planets, debris) — simple velocity integration
pub fn update_free_flight(
    time: Res<Time>,
    mut query: Query<(&FreeFlight, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (flight, mut transform) in query.iter_mut() {
        transform.translation.x += flight.velocity.x * dt;
        transform.translation.y += flight.velocity.y * dt;
    }
}

/// How many segments one full orbit is sampled into. The path is an ellipse
/// (`r = a(1-e²)/(1+e·cos θ)`), not a circle, so it has to be sampled rather
/// than drawn as a ring.
const ORBIT_SAMPLES: usize = 360;
/// Samples drawn, then samples skipped — this is what makes it dotted.
const ORBIT_DASH: usize = 2;
const ORBIT_GAP: usize = 4;

/// Faint. A navigation aid behind the scene, not part of it: at any moment
/// several of these cross the screen at once, and a bright one would draw the
/// eye away from whatever is actually happening.
const ORBIT_LINE_COLOR: Color = Color::srgba(0.45, 0.62, 0.85, 0.22);

/// Draws each orbiting body's path as a dotted ellipse.
///
/// Sampled from the same `r = a(1-e²)/(1+e·cos θ)` that
/// `update_orbital_positions` integrates, so the line is where the planet
/// will actually be rather than an idealised circle through its current
/// position — with eccentricity up to 0.3 those differ visibly.
pub fn draw_orbit_paths(
    mut gizmos: Gizmos,
    parent_query: Query<&Transform, (With<CelestialBody>, Without<OrbitalPath>)>,
    orbit_query: Query<&OrbitalPath>,
) {
    for orbit in orbit_query.iter() {
        let Ok(parent_transform) = parent_query.get(orbit.parent) else { continue };
        let center = parent_transform.translation.truncate();

        let at = |theta: f32| -> Vec2 {
            let r = orbit_radius(orbit.semi_major_axis, orbit.eccentricity, theta);
            center + Vec2::new(r * theta.cos(), r * theta.sin())
        };

        let step = std::f32::consts::TAU / ORBIT_SAMPLES as f32;
        for i in 0..ORBIT_SAMPLES {
            // Draw ORBIT_DASH samples out of every DASH+GAP.
            if i % (ORBIT_DASH + ORBIT_GAP) >= ORBIT_DASH {
                continue;
            }
            gizmos.line_2d(at(i as f32 * step), at((i + 1) as f32 * step), ORBIT_LINE_COLOR);
        }
    }
}

#[cfg(test)]
mod orbit_clock_tests {
    use super::*;
    use crate::resources::SaveData;

    /// Where one planet sits with the orbital clock at `galaxy_time`. The
    /// clock is set, not ticked -- Time's delta stays zero -- so the planet
    /// lands exactly where that reading puts it.
    fn planet_at(galaxy_time: f64) -> Vec2 {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.insert_resource(GalaxyState { galaxy_time, ..default() });
        app.add_systems(Update, update_orbital_positions);
        let star = app.world_mut().spawn((
            CelestialBody {
                body_type: CelestialBodyType::Star,
                mass: 1.0,
                radius: 40_000.0,
                name: "Star".into(),
            },
            Transform::from_xyz(200_000.0, -50_000.0, 0.0),
        )).id();
        let planet = app.world_mut().spawn((
            OrbitalPath {
                parent: star,
                semi_major_axis: 300_000.0,
                eccentricity: 0.2,
                phase: 0.7,
                period: 2_400.0,
                clockwise: false,
            },
            Transform::default(),
        )).id();
        app.update();
        app.world().get::<Transform>(planet).unwrap().translation.truncate()
    }

    /// A load from the menu puts the planets back where the save left them.
    /// The way to the menu resets GalaxyState, clock and all, so the clock the
    /// save carries is the only record of where they were.
    #[test]
    fn a_saved_clock_puts_planets_back_where_they_were() {
        let mut save = SaveData::default();
        save.galaxy_time = 1_234.5;
        let where_saved = planet_at(save.galaxy_time);

        let json = serde_json::to_string(&save).unwrap();
        let back: SaveData = serde_json::from_str(&json).unwrap();
        assert_eq!(planet_at(back.galaxy_time), where_saved);

        // And it matters: the reset clock put the planet half an orbit away.
        assert!(
            planet_at(0.0).distance(where_saved) > 100_000.0,
            "a clock at zero puts the planet near where it was saved anyway, so this proves nothing"
        );
    }
}
