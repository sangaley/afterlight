use bevy::prelude::*;

// ============================================================================
// CELESTIAL BODY COMPONENTS
// ============================================================================

/// Core component for any celestial body (star, planet, asteroid, black hole)
#[derive(Component)]
pub struct CelestialBody {
    pub body_type: CelestialBodyType,
    pub mass: f32,
    pub radius: f32,
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CelestialBodyType {
    Star,
    Planet,
    Asteroid,
    BlackHole,
    Debris,
}

// ============================================================================
// GRAVITY
// ============================================================================

/// Anything with this component exerts gravitational pull
#[derive(Component)]
pub struct GravityWell {
    /// Pre-computed GM constant (tuned for gameplay, not physics)
    pub strength: f32,
    /// Beyond this distance, gravity is zero (performance optimization)
    pub influence_radius: f32,
    pub falloff: GravityFalloff,
}

#[derive(Clone, Copy, Debug)]
pub enum GravityFalloff {
    /// F = strength / r^2 — realistic, used for stars and planets
    InverseSquare,
    /// F = strength / r — gentler, used for smaller bodies
    InverseLinear,
    /// Custom dramatic ramp near event horizon
    BlackHole,
}

/// Anything with this component is AFFECTED by gravity wells
#[derive(Component)]
pub struct GravityAffected {
    pub mass: f32,
}

/// Accumulated gravity force this frame — written by gravity system, read by movement
#[derive(Component, Default)]
pub struct GravityForce(pub Vec2);

// ============================================================================
// ORBITS
// ============================================================================

/// Stable Keplerian orbit around a parent body. Position evaluated analytically.
#[derive(Component)]
pub struct OrbitalPath {
    pub parent: Entity,
    pub semi_major_axis: f32,
    pub eccentricity: f32,
    pub phase: f32,
    pub period: f32,
    pub clockwise: bool,
}

/// Replaces OrbitalPath when a star dies — body flies off on a tangent
#[derive(Component)]
pub struct FreeFlight {
    pub velocity: Vec2,
}

// ============================================================================
// STARS
// ============================================================================

#[derive(Component)]
pub struct Star {
    pub luminosity: f32,
    pub radiation_output: f32,
    pub size_class: StarSizeClass,
    /// Builds up randomly over time. When it crosses flare_threshold, a flare fires.
    pub flare_buildup: f32,
    /// Randomized per-star (0.7 to 0.95) — unpredictable flare timing
    pub flare_threshold: f32,
    pub is_dying: bool,
    pub death_timer: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StarSizeClass {
    Dwarf,
    Main,
    Giant,
    Supergiant,
}

impl StarSizeClass {
    pub fn radiation_multiplier(&self) -> f32 {
        match self {
            Self::Dwarf => 0.5,
            Self::Main => 1.0,
            Self::Giant => 2.5,
            Self::Supergiant => 5.0,
        }
    }

    pub fn flare_intensity_multiplier(&self) -> f32 {
        match self {
            Self::Dwarf => 0.3,
            Self::Main => 1.0,
            Self::Giant => 3.0,
            Self::Supergiant => 8.0,
        }
    }

    pub fn radius(&self) -> f32 {
        match self {
            Self::Dwarf => 40_000.0,
            Self::Main => 80_000.0,
            Self::Giant => 120_000.0,
            Self::Supergiant => 150_000.0,
        }
    }

    pub fn mass(&self) -> f32 {
        match self {
            Self::Dwarf => 5_000.0,
            Self::Main => 20_000.0,
            Self::Giant => 80_000.0,
            Self::Supergiant => 200_000.0,
        }
    }

    /// Acceleration at the star's surface, in world units per second².
    ///
    /// The well used to be `mass * 500` with inverse-square falloff, which
    /// put a dwarf's pull AT ITS OWN SURFACE at about 0.0016 u/s² — the same
    /// fault planets had, never fixed here. You could skim a sun and not
    /// notice it.
    ///
    /// Every class out-pulls the strongest planet (a gas giant, 95) because
    /// a star should be the deepest well in its system. And every class stays
    /// under the starter ship's ~180 u/s² of thrust, so a star is a fight to
    /// climb out of and never a trap. A heavier ship accelerates less, so
    /// near a supergiant that margin is genuinely thin.
    pub fn surface_gravity(&self) -> f32 {
        match self {
            Self::Dwarf => 110.0,
            Self::Main => 125.0,
            Self::Giant => 135.0,
            Self::Supergiant => 145.0,
        }
    }
}

// ============================================================================
// BLACK HOLES
// ============================================================================

#[derive(Component)]
pub struct BlackHole {
    pub event_horizon_radius: f32,
    pub accretion_disk_radius: f32,
    /// Grows as it consumes mass — makes it progressively more dangerous
    pub consumed_mass: f32,
    pub tidal_force_multiplier: f32,
}

/// Marks an entity being consumed — visual spiral-in before despawn
#[derive(Component)]
pub struct BeingConsumed {
    pub by_black_hole: Entity,
    pub progress: f32,
}

// ============================================================================
// PLANETS
// ============================================================================

#[derive(Component)]
pub struct Planet {
    pub planet_type: PlanetType,
    pub has_atmosphere: bool,
    /// Drawn as a flat ellipse around the body. Giants mostly, and shattered
    /// worlds, where the debris is what is left of whatever broke it.
    pub has_rings: bool,
    pub resource_richness: f32,
}

/// How far from its star a planet sits, which is what decides what kind of
/// world it can be. Assigned by orbit index rather than rolled freely, so a
/// system reads as a system: scorched things close in, ice and giants out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrbitBand {
    Hot,
    Temperate,
    Cold,
}

/// A planet turning on its own axis, in radians per second.
///
/// Only the body turns. Its ring stays level and its night side stays turned
/// away from the star -- see `celestial_visuals::spin_and_light_planets` --
/// because a spinning disc whose shadow spins with it reads as a rotating
/// sticker, not a world.
#[derive(Component)]
pub struct PlanetSpin {
    pub rate: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlanetType {
    // --- Hot ---
    Lava,
    Volcanic,
    Desert,
    // --- Temperate ---
    Rocky,
    Barren,
    Ocean,
    Terran,
    Toxic,
    // --- Cold ---
    Ice,
    Gas,
    IceGiant,
    Shattered,
}

impl PlanetType {
    /// Which of the ten Kenney sprites can stand for this world.
    ///
    /// Matched to what the art actually depicts, which the previous mapping
    /// was not: it could render a Lava planet as `planet07` (blue-white ice)
    /// and an Ice planet as `planet03` (a green-blue earth-like world), and
    /// put the orange volcanic `planet02` forward as a gas giant. Planets
    /// routinely showed the wrong face for their type.
    ///
    /// 00 cyan banded · 01 purple/green mottled · 02 orange volcanic
    /// 03 green-blue earthlike · 04 grey cratered · 05 tan desert
    /// 06 dark maroon · 07 blue-white icy · 08 bright red molten
    /// 09 violet
    pub fn sprites(&self) -> &'static [u8] {
        match self {
            Self::Lava => &[8, 2],
            Self::Volcanic => &[2, 6],
            Self::Desert => &[5],
            Self::Rocky => &[4, 5],
            Self::Barren => &[4],
            Self::Ocean => &[3, 0],
            Self::Terran => &[3],
            Self::Toxic => &[9, 1],
            Self::Ice => &[7, 0],
            Self::Gas => &[1, 9],
            Self::IceGiant => &[0, 7],
            Self::Shattered => &[6, 4],
        }
    }

    /// Doubled from the original ranges. A planet reads as scenery at the
    /// old sizes -- against a star of 40,000-150,000 radius, a 10,000 rock
    /// was a pebble -- and the orbits are derived from these now, so growing
    /// them pushes the system apart rather than making planets collide.
    pub fn radius_range(&self) -> (f32, f32) {
        match self {
            Self::Lava => (18_000.0, 26_000.0),
            Self::Volcanic => (20_000.0, 28_000.0),
            Self::Desert => (20_000.0, 30_000.0),
            Self::Rocky => (20_000.0, 30_000.0),
            Self::Barren => (12_000.0, 22_000.0),
            Self::Ocean => (22_000.0, 32_000.0),
            Self::Terran => (20_000.0, 28_000.0),
            Self::Toxic => (20_000.0, 30_000.0),
            Self::Ice => (16_000.0, 24_000.0),
            Self::Gas => (40_000.0, 60_000.0),
            Self::IceGiant => (32_000.0, 48_000.0),
            Self::Shattered => (10_000.0, 20_000.0),
        }
    }

    /// Acceleration at the surface, in world units per second squared.
    ///
    /// The ship's engines manage about 180 u/s² (`THRUST_SCALE` over the
    /// starter's thrust-to-mass), so these are a real tug you can still climb
    /// out of. The old model derived the well from mass alone --
    /// `strength = mass * 100` with inverse-square falloff -- which put a
    /// rocky world's pull at its OWN SURFACE at about 0.0009 u/s². Five
    /// orders of magnitude under the thrusters: gravity existed in the data
    /// and did nothing whatsoever.
    pub fn surface_gravity(&self) -> f32 {
        match self {
            Self::Gas => 95.0,
            Self::IceGiant => 70.0,
            Self::Toxic => 34.0,
            Self::Ocean => 32.0,
            Self::Volcanic => 32.0,
            Self::Lava => 30.0,
            Self::Rocky => 30.0,
            Self::Terran => 30.0,
            Self::Desert => 28.0,
            Self::Ice => 18.0,
            Self::Barren => 12.0,
            Self::Shattered => 8.0,
        }
    }

    pub fn mass_range(&self) -> (f32, f32) {
        match self {
            Self::Lava => (900.0, 2_200.0),
            Self::Volcanic => (1_000.0, 2_500.0),
            Self::Desert => (800.0, 2_000.0),
            Self::Rocky => (800.0, 2_000.0),
            Self::Barren => (400.0, 1_200.0),
            Self::Ocean => (1_000.0, 2_600.0),
            Self::Terran => (900.0, 2_200.0),
            Self::Toxic => (1_100.0, 2_800.0),
            Self::Ice => (600.0, 1_500.0),
            Self::Gas => (5_000.0, 15_000.0),
            Self::IceGiant => (3_000.0, 9_000.0),
            Self::Shattered => (200.0, 800.0),
        }
    }

    /// How often this kind of world has an atmosphere worth drawing.
    pub fn atmosphere_chance(&self) -> f64 {
        match self {
            Self::Gas | Self::IceGiant | Self::Toxic => 1.0,
            Self::Ocean | Self::Terran => 0.9,
            Self::Volcanic => 0.6,
            Self::Rocky | Self::Desert => 0.4,
            Self::Lava | Self::Ice => 0.25,
            Self::Barren | Self::Shattered => 0.0,
        }
    }

    /// Rings. Giants mostly, and shattered worlds -- whatever broke one left
    /// its debris in orbit.
    pub fn ring_chance(&self) -> f64 {
        match self {
            Self::Gas => 0.6,
            Self::IceGiant => 0.45,
            Self::Shattered => 0.5,
            _ => 0.0,
        }
    }

    /// What a planet at this distance from its star can be.
    ///
    /// Barren appears in every band because a dead rock is the one world that
    /// can happen anywhere, and Shattered likewise -- something broke it, and
    /// that is not a function of temperature.
    pub fn for_band(band: OrbitBand) -> &'static [PlanetType] {
        match band {
            OrbitBand::Hot => &[
                Self::Lava, Self::Lava, Self::Volcanic, Self::Desert, Self::Barren, Self::Shattered,
            ],
            OrbitBand::Temperate => &[
                Self::Rocky, Self::Desert, Self::Ocean, Self::Terran,
                Self::Toxic, Self::Barren, Self::Volcanic, Self::Shattered,
            ],
            OrbitBand::Cold => &[
                Self::Ice, Self::Ice, Self::Gas, Self::IceGiant,
                Self::Barren, Self::Toxic, Self::Shattered,
            ],
        }
    }

    /// The colour the sprite is multiplied by. Deliberately close to white:
    /// the art already carries its own palette, and a strong tint turns
    /// detailed pixel work into a flat wash. This is for separating two
    /// worlds of the same type, not for recolouring them.
    pub fn tint(&self) -> Color {
        match self {
            Self::Lava => Color::srgb(1.0, 0.92, 0.88),
            Self::Volcanic => Color::srgb(0.95, 0.88, 0.85),
            Self::Desert => Color::srgb(1.0, 0.97, 0.88),
            // Rocky and Barren share sprite 04, and Ocean and Terran share
            // 03 -- ten images cannot cover twelve types. The tint is what
            // separates each pair, so these four are pushed further from
            // white than the rest: Rocky warm against Barren's cold grey,
            // Ocean blue against Terran's green. Still a tint and not a
            // recolour; the art keeps its own palette underneath.
            Self::Rocky => Color::srgb(1.0, 0.93, 0.84),
            Self::Barren => Color::srgb(0.78, 0.82, 0.92),
            Self::Ocean => Color::srgb(0.80, 0.91, 1.0),
            Self::Terran => Color::srgb(0.90, 1.0, 0.84),
            Self::Toxic => Color::srgb(0.97, 1.0, 0.90),
            Self::Ice => Color::srgb(0.95, 0.98, 1.0),
            Self::Gas => Color::srgb(1.0, 0.96, 0.92),
            Self::IceGiant => Color::srgb(0.92, 0.97, 1.0),
            Self::Shattered => Color::srgb(0.88, 0.86, 0.88),
        }
    }
}

// ============================================================================
// STAR SYSTEM
// ============================================================================

/// Tags an entity as belonging to a specific star system
#[derive(Component)]
pub struct StarSystemMember {
    pub system_id: u32,
}

// ============================================================================
// WARP / JUMP
// ============================================================================

/// Marks the ship as currently charging a warp jump
#[derive(Component)]
pub struct WarpCharging {
    pub target: super::resources::GalaxyWarpTarget,
    pub charge_timer: Timer,
    /// Computed once at charge-start from galaxy-map distance (see
    /// celestial::warp::interstellar_fuel_cost) and deducted on completion.
    pub fuel_cost: f32,
}
