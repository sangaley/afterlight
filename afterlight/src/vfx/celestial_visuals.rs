use bevy::prelude::*;
use crate::celestial::components::*;
use super::procedural_textures::CelestialTextures;

// ============================================================================
// CELESTIAL BODY VISUAL LAYERS
// Each celestial body gets multiple sprite layers for realistic appearance.
// Stars: core + corona + ambient glow
// Planets: body + shadow + optional atmosphere
// Black holes: center + accretion disk + outer distortion ring
// ============================================================================

/// Marks that an entity already has visual layers attached
#[derive(Component)]
pub struct HasVisualLayers;

/// Star glow layer — pulsing outer corona
#[derive(Component)]
pub struct StarGlow {
    pub base_alpha: f32,
    pub pulse_speed: f32,
    pub pulse_amplitude: f32,
}

/// Star corona layer — larger, dimmer outer ring
#[derive(Component)]
pub struct StarCorona;

/// Star flare visual — brightens during buildup
#[derive(Component)]
pub struct StarFlareGlow;

/// How flat a ring lies. Near enough to edge-on to read as a disc seen at an
/// angle rather than as a halo around the planet.
const RING_TILT: f32 = 0.28;

/// Marker for a planet's ring layer.
#[derive(Component)]
pub struct PlanetRing;

/// Planet atmosphere layer
#[derive(Component)]
pub struct PlanetAtmosphere {
    pub rotation_speed: f32,
}

/// Planet shadow (dark side away from star)
#[derive(Component)]
pub struct PlanetShadow;

/// Black hole accretion disk
#[derive(Component)]
pub struct AccretionDisk {
    pub rotation_speed: f32,
}

/// Black hole event horizon visual
#[derive(Component)]
pub struct EventHorizonVisual;

// ============================================================================
// ATTACH VISUAL LAYERS — runs once per entity when first seen
// ============================================================================

/// Give a body its glow, ring, atmosphere or shadow sprites, and mark it done
/// -- if it still exists when the commands land. A load or a jump clears the
/// system's bodies the same frame a fresh one can first be seen, and adding a
/// child to a despawned body panicked the game (it did, once, mid-load beside
/// a capital world). The orphaned layers go with it.
fn attach_layers(commands: &mut Commands, body: Entity, layers: Vec<Entity>) {
    commands.queue(move |world: &mut World| {
        if let Ok(mut body) = world.get_entity_mut(body) {
            body.insert(HasVisualLayers);
            body.add_children(&layers);
        } else {
            for layer in layers {
                if let Ok(e) = world.get_entity_mut(layer) {
                    e.despawn();
                }
            }
        }
    });
}

/// Attach glow layers to stars that don't have them yet
pub fn attach_star_visuals(
    mut commands: Commands,
    star_query: Query<(Entity, &CelestialBody, &Star), Without<HasVisualLayers>>,
    textures: Res<CelestialTextures>,
) {
    for (entity, body, star) in star_query.iter() {
        let radius = body.radius;

        // Inner glow — bright, tight around the star
        let inner_glow = commands.spawn((
            (Sprite {
                    image: textures.glow.clone(),
                    color: star_glow_color(star.size_class, 0.35),
                    custom_size: Some(Vec2::splat(radius * 2.8)),
                    ..default()
                }, Transform::from_xyz(0.0, 0.0, -0.05)),
            StarGlow {
                base_alpha: 0.35,
                pulse_speed: 0.8 + star.luminosity * 0.3,
                pulse_amplitude: 0.08,
            },
        )).id();

        // Outer corona — wide, dim, atmospheric
        let corona = commands.spawn((
            (Sprite {
                    image: textures.glow.clone(),
                    color: star_corona_color(star.size_class, 0.12),
                    custom_size: Some(Vec2::splat(radius * 4.5)),
                    ..default()
                }, Transform::from_xyz(0.0, 0.0, -0.1)),
            StarCorona,
        )).id();

        // Flare glow — invisible until flare builds up
        let flare_glow = commands.spawn((
            (Sprite {
                    image: textures.glow.clone(),
                    color: Color::srgba(1.0, 0.9, 0.7, 0.0),
                    custom_size: Some(Vec2::splat(radius * 6.0)),
                    ..default()
                }, Transform::from_xyz(0.0, 0.0, -0.15)),
            StarFlareGlow,
        )).id();

        attach_layers(&mut commands, entity, vec![inner_glow, corona, flare_glow]);
    }
}

/// Attach visual layers to planets
pub fn attach_planet_visuals(
    mut commands: Commands,
    planet_query: Query<(Entity, &CelestialBody, &Planet), Without<HasVisualLayers>>,
    textures: Res<CelestialTextures>,
) {
    for (entity, body, planet) in planet_query.iter() {
        let radius = body.radius;

        // Rings sit BEHIND the body, as a wide flat ellipse. One sprite
        // squashed on Y rather than two halves composited around the planet:
        // the far side of a real ring passes behind the globe, but at the
        // sizes these are drawn the seam costs more than the occlusion buys.
        if planet.has_rings {
            let ring = commands.spawn((
                (Sprite {
                        image: textures.ring.clone(),
                        color: match planet.planet_type {
                            PlanetType::IceGiant => Color::srgba(0.80, 0.90, 1.0, 0.40),
                            PlanetType::Shattered => Color::srgba(0.70, 0.66, 0.66, 0.45),
                            _ => Color::srgba(0.90, 0.85, 0.72, 0.38),
                        },
                        custom_size: Some(Vec2::new(radius * 4.4, radius * 4.4 * RING_TILT)),
                        ..default()
                    }, Transform::from_xyz(0.0, 0.0, -0.08)),
                PlanetRing,
            )).id();
            attach_layers(&mut commands, entity, vec![ring]);
        }

        // Atmosphere glow, where the world has one worth drawing
        if planet.has_atmosphere {
            let atmo_color = match planet.planet_type {
                PlanetType::Gas => Color::srgba(0.40, 0.50, 0.70, 0.15),
                PlanetType::IceGiant => Color::srgba(0.45, 0.65, 0.80, 0.16),
                PlanetType::Toxic => Color::srgba(0.55, 0.70, 0.30, 0.17),
                PlanetType::Ocean => Color::srgba(0.40, 0.60, 0.85, 0.13),
                PlanetType::Terran => Color::srgba(0.50, 0.70, 0.80, 0.12),
                PlanetType::Volcanic => Color::srgba(0.70, 0.40, 0.25, 0.12),
                PlanetType::Lava => Color::srgba(0.85, 0.45, 0.25, 0.14),
                PlanetType::Desert => Color::srgba(0.75, 0.65, 0.45, 0.10),
                PlanetType::Rocky => Color::srgba(0.50, 0.60, 0.80, 0.10),
                PlanetType::Ice => Color::srgba(0.65, 0.80, 0.90, 0.10),
                PlanetType::Barren | PlanetType::Shattered => Color::srgba(0.40, 0.40, 0.50, 0.06),
            };

            let atmosphere = commands.spawn((
                (Sprite {
                        image: textures.glow.clone(),
                        color: atmo_color,
                        custom_size: Some(Vec2::splat(radius * 2.3)),
                        ..default()
                    }, Transform::from_xyz(0.0, 0.0, -0.05)),
                PlanetAtmosphere {
                    rotation_speed: 0.02,
                },
            )).id();

            attach_layers(&mut commands, entity, vec![atmosphere]);
        }

        // Night side. Positioned every frame by `spin_and_light_planets` so
        // it faces away from the star; this spawn offset is only where it
        // sits for the one frame before that runs.
        let shadow = commands.spawn((
            (Sprite {
                    image: textures.solid.clone(),
                    color: Color::srgba(0.0, 0.0, 0.02, 0.5),
                    custom_size: Some(Vec2::splat(radius * 2.0)),
                    ..default()
                }, Transform::from_xyz(radius * 0.15, -radius * 0.1, 0.01)),
            PlanetShadow,
        )).id();

        attach_layers(&mut commands, entity, vec![shadow]);
    }
}

/// Attach visual layers to black holes
pub fn attach_black_hole_visuals(
    mut commands: Commands,
    bh_query: Query<(Entity, &CelestialBody, &BlackHole), Without<HasVisualLayers>>,
    textures: Res<CelestialTextures>,
) {
    for (entity, _body, bh) in bh_query.iter() {
        // Event horizon — pitch black center
        let horizon = commands.spawn((
            (Sprite {
                    image: textures.solid.clone(),
                    color: Color::srgba(0.0, 0.0, 0.0, 1.0),
                    custom_size: Some(Vec2::splat(bh.event_horizon_radius * 2.0)),
                    ..default()
                }, Transform::from_xyz(0.0, 0.0, 0.02)),
            EventHorizonVisual,
        )).id();

        // Accretion disk — spinning orange/red ring
        let disk_inner = commands.spawn((
            (Sprite {
                    image: textures.glow.clone(),
                    color: Color::srgba(0.9, 0.4, 0.1, 0.6),
                    custom_size: Some(Vec2::new(bh.accretion_disk_radius * 2.5, bh.accretion_disk_radius * 0.4)),
                    ..default()
                }, Transform::from_xyz(0.0, 0.0, -0.02)),
            AccretionDisk { rotation_speed: 0.5 },
        )).id();

        // Outer accretion glow
        let disk_outer = commands.spawn((
            (Sprite {
                    image: textures.glow.clone(),
                    color: Color::srgba(0.6, 0.15, 0.05, 0.25),
                    custom_size: Some(Vec2::new(bh.accretion_disk_radius * 4.0, bh.accretion_disk_radius * 0.8)),
                    ..default()
                }, Transform::from_xyz(0.0, 0.0, -0.05)),
            AccretionDisk { rotation_speed: 0.3 },
        )).id();

        // Gravitational distortion ring — faint purple/blue outer halo
        let distortion = commands.spawn(
            (Sprite {
                    image: textures.glow.clone(),
                    color: Color::srgba(0.2, 0.1, 0.3, 0.08),
                    custom_size: Some(Vec2::splat(bh.accretion_disk_radius * 6.0)),
                    ..default()
                }, Transform::from_xyz(0.0, 0.0, -0.1)),
        ).id();

        attach_layers(&mut commands, entity, vec![horizon, disk_inner, disk_outer, distortion]);
    }
}

// ============================================================================
// ANIMATION SYSTEMS
// ============================================================================

/// Pulsing star glow
pub fn animate_star_glow(
    time: Res<Time>,
    mut glow_query: Query<(&StarGlow, &mut Sprite)>,
) {
    let t = time.elapsed_secs();
    for (glow, mut sprite) in glow_query.iter_mut() {
        let pulse = glow.base_alpha + (t * glow.pulse_speed).sin() * glow.pulse_amplitude;
        sprite.color.set_alpha(pulse.clamp(0.05, 0.6));
    }
}

/// Star brightens as flare buildup increases
pub fn animate_star_flare_buildup(
    star_query: Query<(&Star, &Children)>,
    mut flare_glow_query: Query<&mut Sprite, With<StarFlareGlow>>,
) {
    for (star, children) in star_query.iter() {
        let flare_alpha = (star.flare_buildup / star.flare_threshold).clamp(0.0, 1.0) * 0.4;
        for child in children.iter() {
            if let Ok(mut sprite) = flare_glow_query.get_mut(child) {
                sprite.color.set_alpha(flare_alpha);
            }
        }
    }
}

/// Spinning accretion disk
pub fn animate_black_hole_disk(
    time: Res<Time>,
    mut disk_query: Query<(&AccretionDisk, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (disk, mut transform) in disk_query.iter_mut() {
        transform.rotation *= Quat::from_rotation_z(disk.rotation_speed * dt);
    }
}

/// Subtle atmosphere shimmer
pub fn animate_planet_atmosphere(
    time: Res<Time>,
    mut atmo_query: Query<(&PlanetAtmosphere, &mut Sprite)>,
) {
    let t = time.elapsed_secs();
    for (_atmo, mut sprite) in atmo_query.iter_mut() {
        let shimmer = 0.10 + (t * 0.5).sin() * 0.03;
        sprite.color.set_alpha(shimmer);
    }
}

// ============================================================================
// COLOR HELPERS
// ============================================================================

fn star_glow_color(class: StarSizeClass, alpha: f32) -> Color {
    match class {
        StarSizeClass::Dwarf => Color::srgba(1.0, 0.6, 0.3, alpha),
        StarSizeClass::Main => Color::srgba(1.0, 0.95, 0.85, alpha),
        StarSizeClass::Giant => Color::srgba(1.0, 0.7, 0.3, alpha),
        StarSizeClass::Supergiant => Color::srgba(0.7, 0.8, 1.0, alpha),
    }
}

fn star_corona_color(class: StarSizeClass, alpha: f32) -> Color {
    match class {
        StarSizeClass::Dwarf => Color::srgba(1.0, 0.4, 0.15, alpha),
        StarSizeClass::Main => Color::srgba(1.0, 0.85, 0.6, alpha),
        StarSizeClass::Giant => Color::srgba(1.0, 0.5, 0.15, alpha),
        StarSizeClass::Supergiant => Color::srgba(0.5, 0.6, 1.0, alpha),
    }
}


/// How far the night-side disc is pushed away from the star, as a fraction of
/// the planet's radius.
///
/// The shadow is a dark disc the same size as the planet, so its offset
/// decides how much of the world is in darkness. It used to sit at a fixed
/// (0.15, -0.1) of the radius, which left the disc covering nearly the whole
/// planet at half opacity -- most of the surface was dimmed, in the same
/// direction for every planet, regardless of where its star was. At 0.55 the
/// overlap is roughly the far half: lit toward the star, dark away from it.
const TERMINATOR_OFFSET: f32 = 0.55;

/// Turns each planet on its axis while its ring stays level and its night
/// side stays facing away from the star.
///
/// Children inherit their parent's rotation, so spinning the planet entity
/// alone would carry the ring and the shadow round with it. Each frame they
/// are given the inverse rotation, and the shadow's offset is set in world
/// space and converted back into the planet's turning frame (local =
/// R⁻¹ · world). The surface features rotate; the light does not.
pub fn spin_and_light_planets(
    time: Res<Time>,
    mut planets: Query<(&PlanetSpin, &mut Transform, &CelestialBody, &OrbitalPath, &Children)>,
    stars: Query<&GlobalTransform>,
    mut layers: Query<
        (&mut Transform, Has<PlanetShadow>),
        (Or<(With<PlanetRing>, With<PlanetShadow>)>, Without<PlanetSpin>),
    >,
) {
    let dt = time.delta_secs();
    for (spin, mut transform, body, orbit, children) in planets.iter_mut() {
        transform.rotate_z(spin.rate * dt);
        let unspin = transform.rotation.inverse();

        // Direction from the star out through the planet, in world space.
        let away = stars
            .get(orbit.parent)
            .map(|star| (transform.translation.truncate() - star.translation().truncate()).normalize_or_zero())
            .unwrap_or(Vec2::X);

        for child in children.iter() {
            let Ok((mut layer, is_shadow)) = layers.get_mut(child) else { continue };
            layer.rotation = unspin;
            if is_shadow {
                let world = (away * body.radius * TERMINATOR_OFFSET).extend(0.0);
                let local = unspin * world;
                layer.translation = Vec3::new(local.x, local.y, layer.translation.z);
            }
        }
    }
}

#[cfg(test)]
mod spin_tests {
    use super::*;

    /// No `MinimalPlugins`: its TimePlugin rewrites `Time` from the wall
    /// clock every update and would discard the delta these tests advance by.
    ///
    /// Running the system at all is the first thing under test. Bevy checks
    /// query conflicts when a system is INITIALISED, not when it compiles, so
    /// two queries overlapping on `&mut Transform` would sail through
    /// `cargo check` and panic with B0001 on the first frame of a real game.
    fn app() -> (App, Entity, Entity, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, spin_and_light_planets);

        // Star at the origin. GlobalTransform is set by hand because without
        // TransformPlugin nothing propagates it.
        let star = app
            .world_mut()
            .spawn((Transform::default(), GlobalTransform::default()))
            .id();

        // Planet due east of the star, so "away from the star" is +X.
        let planet = app
            .world_mut()
            .spawn((
                Transform::from_xyz(100_000.0, 0.0, -0.9),
                CelestialBody {
                    body_type: CelestialBodyType::Planet,
                    mass: 1_000.0,
                    radius: 20_000.0,
                    name: "Test".into(),
                },
                OrbitalPath {
                    parent: star,
                    semi_major_axis: 100_000.0,
                    eccentricity: 0.0,
                    phase: 0.0,
                    period: 1_000.0,
                    clockwise: false,
                },
                PlanetSpin { rate: 1.0 },
            ))
            .id();

        let shadow = app
            .world_mut()
            .spawn((Transform::from_xyz(0.0, 0.0, 0.01), PlanetShadow, ChildOf(planet)))
            .id();
        let ring = app
            .world_mut()
            .spawn((Transform::from_xyz(0.0, 0.0, -0.08), PlanetRing, ChildOf(planet)))
            .id();

        (app, star, planet, shadow, ring)
    }

    fn step(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(seconds));
        app.update();
    }

    #[test]
    fn the_planet_actually_turns() {
        let (mut app, _, planet, _, _) = app();
        step(&mut app, 0.5);
        let turned = app.world().get::<Transform>(planet).unwrap().rotation;
        let (_, angle) = turned.to_axis_angle();
        assert!(angle > 0.4, "planet turned only {angle} rad in half a second at 1 rad/s");
    }

    /// Inheriting the planet's rotation would spin the ring round with it.
    /// Its world orientation must stay level however far the planet turns.
    #[test]
    fn the_ring_stays_level() {
        let (mut app, _, planet, _, ring) = app();
        for _ in 0..20 {
            step(&mut app, 0.25);
        }
        let parent = app.world().get::<Transform>(planet).unwrap().rotation;
        let local = app.world().get::<Transform>(ring).unwrap().rotation;
        let world = parent * local;
        assert!(
            world.angle_between(Quat::IDENTITY) < 1e-3,
            "ring has tilted {} rad in world space -- it is spinning with the planet",
            world.angle_between(Quat::IDENTITY)
        );
    }

    /// The night side points away from the star in WORLD space, no matter
    /// how far the body has turned beneath it.
    #[test]
    fn the_night_side_faces_away_from_the_star() {
        let (mut app, _, planet, shadow, _) = app();
        for _ in 0..13 {
            step(&mut app, 0.37);
        }
        let parent = *app.world().get::<Transform>(planet).unwrap();
        let local = app.world().get::<Transform>(shadow).unwrap().translation;
        let world_offset = (parent.rotation * Vec3::new(local.x, local.y, 0.0)).truncate();

        // Star at origin, planet at +X, so away from the star is +X.
        let dir = world_offset.normalize_or_zero();
        assert!(
            dir.dot(Vec2::X) > 0.999,
            "night side points {dir}, not away from the star (+X)"
        );
        let expected = 20_000.0 * TERMINATOR_OFFSET;
        assert!(
            (world_offset.length() - expected).abs() < 1.0,
            "night side is {} out, expected {expected}",
            world_offset.length()
        );
    }

    /// The shadow keeps its own depth: it must stay drawn above the planet
    /// body, or the night side vanishes behind the surface.
    #[test]
    fn the_shadow_keeps_its_layer() {
        let (mut app, _, _, shadow, _) = app();
        step(&mut app, 1.0);
        let z = app.world().get::<Transform>(shadow).unwrap().translation.z;
        assert_eq!(z, 0.01, "repositioning the shadow moved it to z={z}");
    }
}
