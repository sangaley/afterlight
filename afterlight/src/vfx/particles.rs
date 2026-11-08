use bevy::prelude::*;
use rand::Rng;
use crate::components::*;

// ============================================================================
// PARTICLE SYSTEM
// Lightweight sprite-based particles for engine exhaust, weapon fire,
// hull breaches, and explosions. Each particle is a small sprite entity
// with velocity, lifetime, and fade behavior.
// ============================================================================

/// A single particle entity
#[derive(Component)]
pub struct Particle {
    pub velocity: Vec2,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub fade: bool,
    pub shrink: bool,
    /// Opacity at birth.
    ///
    /// Fading multiplies THIS by the remaining life ratio. Reading the
    /// sprite's current alpha instead compounded the fade every frame — the
    /// product of sixty ratios a second collapses almost immediately, so a
    /// particle was invisible long before its lifetime ran out and anything
    /// meant to linger simply could not.
    pub base_alpha: f32,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            velocity: Vec2::ZERO,
            lifetime: 0.5,
            max_lifetime: 0.5,
            fade: true,
            shrink: true,
            base_alpha: 1.0,
        }
    }
}

impl Particle {
    /// A fully opaque particle that fades and shrinks over `life` seconds.
    pub fn new(velocity: Vec2, life: f32) -> Self {
        Self { velocity, lifetime: life, max_lifetime: life, ..default() }
    }

    /// As `new`, but starting at `alpha` and optionally holding its size.
    pub fn wisp(velocity: Vec2, life: f32, alpha: f32, shrink: bool) -> Self {
        Self { velocity, lifetime: life, max_lifetime: life, base_alpha: alpha, shrink, ..default() }
    }
}

/// Spawn engine exhaust particles behind active engines when thrusting
pub fn spawn_engine_particles(
    time: Res<Time>,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,

    // ChildOf: the throttle and heading below are the PLAYER's, so only the
    // player's engines may use them. Unscoped, every engine on every loaded
    // AI ship puffed whenever the player held W, pointed along the player's
    // heading, and never when the AI itself was manoeuvring.
    engine_query: Query<(&Engine, &Module, &GlobalTransform, &ChildOf), Without<DestroyedModule>>,
    ship_physics: Query<(Entity, &ShipPhysics), With<Ship>>,
    mut commands: Commands,
    mut spawn_timer: Local<f32>,
) {
    *spawn_timer += time.delta_secs();
    if *spawn_timer < 0.05 { return; } // 20 particles/sec max per engine
    *spawn_timer = 0.0;

    let Ok((player, physics)) = ship_physics.single() else { return };
    if physics.throttle.abs() < 0.1 { return; } // Not thrusting

    let mut rng = rand::thread_rng();

    for (_engine, module, global_transform, parent) in engine_query.iter() {
        if !module.is_active || parent.parent() != player { continue; }

        let pos = global_transform.translation().truncate();
        let facing = Vec2::new(physics.rotation.cos(), physics.rotation.sin());
        // Exhaust goes opposite to facing direction
        let exhaust_dir = -facing;

        let intensity = physics.throttle.abs();
        let particle_count = (intensity * 3.0) as u32;

        for _ in 0..particle_count {
            let spread = Vec2::new(
                rng.gen_range(-0.3..0.3),
                rng.gen_range(-0.3..0.3),
            );
            let vel = (exhaust_dir + spread).normalize_or_zero() * rng.gen_range(80.0..200.0);
            let lifetime = rng.gen_range(0.2..0.5);
            // Up from 3-8: at 3 world units a particle is under two screen
            // pixels, which is why the old exhaust read as a sparse dribble of
            // dots rather than a plume.
            let size = rng.gen_range(9.0..20.0);

            // Color: blue-white core fading to orange
            let heat = rng.gen_range(0.5..1.0);
            let color = Color::srgba(
                0.5 + heat * 0.5,
                0.3 + heat * 0.4,
                0.8 * (1.0 - heat * 0.5),
                0.8,
            );

            commands.spawn((
                (Sprite {
                        // Hot near the nozzle, smoke as it cools -- the same
                        // two-part read the missile plume has.
                        image: if heat > 0.72 { fx.flame() } else { fx.puff() },
                        color,
                        custom_size: Some(Vec2::splat(size)),
                        ..default()
                    }, Transform::from_xyz(pos.x, pos.y, 0.5)),
                Particle::wisp(vel, lifetime, 0.55, true),
            ));
        }
    }
}

/// Spawn air particles escaping from hull breaches
pub fn spawn_breach_particles(
    time: Res<Time>,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    hull_query: Query<(&HullSegment, &GlobalTransform)>,
    mut commands: Commands,
    mut spawn_timer: Local<f32>,
) {
    *spawn_timer += time.delta_secs();
    if *spawn_timer < 0.15 { return; }
    *spawn_timer = 0.0;

    let mut rng = rand::thread_rng();

    for (hull, global_transform) in hull_query.iter() {
        if !hull.is_depressurized || hull.depressurization_level < 0.1 { continue; }

        let pos = global_transform.translation().truncate();
        let intensity = hull.depressurization_level;
        let particle_count = (intensity * 4.0) as u32;

        // A breach vents OUTWARD, and which way it points is the information:
        // it tells you which side of the hull is holed. This used to pick a
        // random angle over the full circle, which reads as a puff sitting on
        // the ship rather than as air leaving it.
        //
        // The segment's grid position is ship-local, so its direction from the
        // origin is roughly "away from the middle"; rotating that through the
        // segment's own transform puts it in world space and keeps the jet
        // pointing correctly as the ship turns.
        let local_out = Vec2::new(
            hull.grid_position.x as f32,
            hull.grid_position.y as f32,
        );
        let out = global_transform
            .affine()
            .transform_vector3(local_out.extend(0.0))
            .truncate()
            .normalize_or_zero();

        for _ in 0..particle_count.min(3) {
            // Fall back to a full-circle puff for a block sitting on the ship's
            // own centreline, where "outward" genuinely has no answer.
            let dir = if out == Vec2::ZERO {
                let a = rng.gen_range(0.0..std::f32::consts::TAU);
                Vec2::new(a.cos(), a.sin())
            } else {
                Vec2::from_angle(rng.gen_range(-0.45..0.45)).rotate(out)
            };
            let speed = rng.gen_range(70.0..150.0) * intensity;
            let lifetime = rng.gen_range(0.5..1.1);
            let size = rng.gen_range(8.0..16.0);

            // Real decompression reads as a condensation fog, not the ice
            // glitter the films use -- so: pale, soft, and thinning fast.
            commands.spawn((
                (Sprite {
                        image: fx.puff(),
                        color: Color::srgba(0.78, 0.85, 1.0, 0.42 * intensity),
                        custom_size: Some(Vec2::splat(size)),
                        ..default()
                    }, Transform::from_xyz(pos.x, pos.y, 0.6)),
                Particle::wisp(dir * speed, lifetime, 0.42 * intensity, false),
            ));
        }
    }
}

/// An expanding, fading blast sphere.
///
/// `HitEffect` is a fixed-size square that sits still for 0.2s, which is
/// adequate for a bullet strike and reads as nothing at all for a warhead.
/// This grows and cools instead, so the size of a detonation is something you
/// can actually see.
#[derive(Component)]
pub struct Blast {
    pub elapsed: f32,
    pub duration: f32,
    pub from: f32,
    pub to: f32,
    pub hot: Color,
    pub cool: Color,
}

/// Grow, cool and fade every blast, then despawn it.
pub fn update_blasts(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Blast, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();

    for (entity, mut blast, mut transform, mut sprite) in query.iter_mut() {
        blast.elapsed += dt;
        let t = (blast.elapsed / blast.duration).clamp(0.0, 1.0);

        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }

        // Ease out: a detonation is fastest at the instant it goes off and
        // coasts to its full width, rather than growing at a constant rate.
        let eased = 1.0 - (1.0 - t).powi(3);
        let size = blast.from + (blast.to - blast.from) * eased;
        transform.scale = Vec3::splat(size);

        let hot = blast.hot.to_srgba();
        let cool = blast.cool.to_srgba();
        sprite.color = Color::srgba(
            hot.red + (cool.red - hot.red) * t,
            hot.green + (cool.green - hot.green) * t,
            hot.blue + (cool.blue - hot.blue) * t,
            // Hold opacity through the first half, then fall away — a
            // fireball does not start fading the moment it appears.
            (1.0 - t).powi(2) * (hot.alpha + (cool.alpha - hot.alpha) * t),
        );
    }
}

/// Update all particles: move, age, fade, shrink, despawn
pub fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particle_query: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();

    for (entity, mut particle, mut transform, mut sprite) in particle_query.iter_mut() {
        // Move
        transform.translation.x += particle.velocity.x * dt;
        transform.translation.y += particle.velocity.y * dt;

        // Age
        particle.lifetime -= dt;

        if particle.lifetime <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        let life_ratio = particle.lifetime / particle.max_lifetime;

        // Fade
        if particle.fade {
            sprite.color.set_alpha(particle.base_alpha * life_ratio.clamp(0.0, 1.0));
        }

        // Shrink
        if particle.shrink {
            let scale = life_ratio.clamp(0.1, 1.0);
            transform.scale = Vec3::splat(scale);
        }

        // Slow down over time (drag-like for particles only — visual, not physics)
        particle.velocity *= 0.98;
    }
}

// ============================================================================
// TRANSIT FLASHES
//
// Warp and docking were the two loudest events in the game with no visual at
// all. A warp jump set the ship's translation and zeroed its velocity; a dock
// changed GameState. Both reported themselves entirely through notification
// text, so the most dramatic thing you can do -- crossing a star system --
// looked like nothing happening.
// ============================================================================

/// An expanding ring with a bright core: something arrived, or left.
///
/// Deliberately quieter than a detonation. This is a drive event, not damage,
/// so it is one clean ring rather than a fireball plus spray plus smoke, and
/// the alpha is low enough to read as light rather than as an explosion.
pub fn spawn_warp_flash(
    commands: &mut Commands,
    fx: &crate::vfx::effect_textures::EffectTextures,
    position: Vec2,
    scale: f32,
    color: Color,
) {
    let c = color.to_srgba();
    let pale = Color::srgba(
        c.red + (1.0 - c.red) * 0.65,
        c.green + (1.0 - c.green) * 0.65,
        c.blue + (1.0 - c.blue) * 0.65,
        0.85,
    );

    // The ring: fast, wide, gone quickly.
    commands.spawn((
        Sprite {
            image: fx.ring.clone(),
            color: pale,
            custom_size: Some(Vec2::ONE),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.7),
        Blast {
            elapsed: 0.0,
            duration: 0.55,
            from: scale * 0.15,
            to: scale * 2.6,
            hot: pale,
            cool: Color::srgba(c.red, c.green, c.blue, 0.0),
        },
    ));

    // The core: a soft glow that collapses rather than expands, so the eye is
    // pulled to the ship's position instead of away from it.
    commands.spawn((
        Sprite {
            image: fx.fireball.clone(),
            color: pale,
            custom_size: Some(Vec2::ONE),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.71),
        Blast {
            elapsed: 0.0,
            duration: 0.40,
            from: scale * 1.1,
            to: scale * 0.25,
            hot: pale,
            cool: Color::srgba(c.red, c.green, c.blue, 0.0),
        },
    ));

    // A few streaks flung outward, so the ring reads as displacing something
    // rather than as a decal painted on the void.
    for i in 0..10 {
        let a = (i as f32 / 10.0) * std::f32::consts::TAU + rand::random::<f32>() * 0.4;
        let heading = Vec2::from_angle(a);
        commands.spawn((
            Sprite {
                image: fx.spark.clone(),
                color: pale,
                custom_size: Some(Vec2::new(scale * 0.5, scale * 0.12)),
                ..default()
            },
            Transform {
                translation: position.extend(0.72),
                rotation: Quat::from_rotation_z(a),
                ..default()
            },
            Particle::new(heading * scale * (2.4 + rand::random::<f32>() * 2.0), 0.3 + rand::random::<f32>() * 0.25),
        ));
    }
}

/// Coming alongside: a slow soft pulse, and cold-gas puffs off the hull.
///
/// Much gentler than a warp flash -- docking is a manoeuvre, not an event, and
/// a bright burst here would read as a collision.
pub fn spawn_dock_pulse(
    commands: &mut Commands,
    fx: &crate::vfx::effect_textures::EffectTextures,
    position: Vec2,
    scale: f32,
) {
    let pale = Color::srgba(0.72, 0.85, 1.0, 0.42);
    commands.spawn((
        Sprite {
            image: fx.ring.clone(),
            color: pale,
            custom_size: Some(Vec2::ONE),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.7),
        Blast {
            elapsed: 0.0,
            duration: 1.1,
            from: scale * 0.5,
            to: scale * 1.7,
            hot: pale,
            cool: Color::srgba(0.6, 0.8, 1.0, 0.0),
        },
    ));

    // Manoeuvring thrusters: pale, slow, short-lived.
    for _ in 0..8 {
        let a = rand::random::<f32>() * std::f32::consts::TAU;
        let heading = Vec2::from_angle(a);
        commands.spawn((
            Sprite {
                image: fx.puff(),
                color: Color::srgba(0.80, 0.86, 0.95, 0.34),
                custom_size: Some(Vec2::splat(scale * (0.10 + rand::random::<f32>() * 0.10))),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, 0.6),
            Particle::wisp(heading * scale * 0.5, 0.7 + rand::random::<f32>() * 0.5, 0.34, false),
        ));
    }
}
