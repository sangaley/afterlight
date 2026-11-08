use bevy::prelude::*;
use rand::Rng;
use crate::components::*;

// ============================================================================
// BLOCK DEBRIS
// Destroyed blocks eject a few physical chunks — tinted shards with velocity
// and spin that tumble away and fade. Works for player and AI ships alike
// (both mark death with DestroyedModule / HullDestroyed).
// ============================================================================

/// A chunk of a destroyed block, tumbling away and fading out.
#[derive(Component)]
pub struct Debris {
    pub velocity: Vec2,
    pub angular_vel: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

pub fn spawn_chunks(
    commands: &mut Commands,
    fx: &crate::vfx::effect_textures::EffectTextures,
    rng: &mut impl Rng,
    origin: Vec2,
    base_color: Color,
    inherited_vel: Vec2,
) {
    let count = rng.gen_range(2..=4);
    for _ in 0..count {
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = rng.gen_range(30.0..130.0);
        let velocity = Vec2::new(angle.cos(), angle.sin()) * speed + inherited_vel;
        // Larger than the solid quad this replaced: a shard's texture only
        // fills a third to a half of its footprint, so a matched size would
        // read as smaller debris rather than better-shaped debris.
        let size = rng.gen_range(10.0..24.0);
        let max_lifetime = rng.gen_range(1.2..2.6);

        // Charred shade of the block's own color so debris reads as "a piece
        // of that thing" rather than generic dust.
        let c = base_color.to_srgba();
        let color = Color::srgba(c.red * 0.55, c.green * 0.55, c.blue * 0.55, 1.0);

        commands.spawn((
            (Sprite {
                    image: fx.chunk(),
                    color,
                    // Nearly square now. The old 0.5..1.0 squash existed to
                    // make identical rectangles look varied; the shards carry
                    // their own irregular outlines, and squashing them on top
                    // just reads as a stretched sprite.
                    custom_size: Some(Vec2::new(size, size * rng.gen_range(0.82..1.0))),
                    ..default()
                }, Transform {
                    translation: origin.extend(0.6),
                    rotation: Quat::from_rotation_z(rng.gen_range(0.0..std::f32::consts::TAU)),
                    ..default()
                }),
            Debris {
                velocity,
                angular_vel: rng.gen_range(-6.0..6.0),
                lifetime: max_lifetime,
                max_lifetime,
            },
        ));
    }
}

/// Eject debris when a block dies — module or hull, any ship.
pub fn spawn_block_debris(
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    dead_modules: Query<
        (&GlobalTransform, &Sprite, Option<&BaseSpriteColor>, &ChildOf),
        Added<DestroyedModule>,
    >,
    dead_hull: Query<
        (&GlobalTransform, &Sprite, &ChildOf),
        (Added<HullDestroyed>, Without<DestroyedModule>),
    >,
    ship_vel_query: Query<&Velocity>,
    mut rng_seed: Local<u32>,
) {
    let mut rng = rand::thread_rng();
    *rng_seed = rng_seed.wrapping_add(1);

    for (gt, sprite, base, parent) in dead_modules.iter() {
        let origin = gt.translation().truncate();
        // BaseSpriteColor is the pre-damage-tint color — at death the live
        // sprite is already charred dark, which flattens every module to
        // the same grey chunk.
        let color = base.map(|b| b.0).unwrap_or(sprite.color);
        let inherited = ship_vel_query.get(parent.parent()).map(|v| v.0 * 0.6).unwrap_or(Vec2::ZERO);
        spawn_chunks(&mut commands, &fx, &mut rng, origin, color, inherited);
    }

    for (gt, sprite, parent) in dead_hull.iter() {
        let origin = gt.translation().truncate();
        let inherited = ship_vel_query.get(parent.parent()).map(|v| v.0 * 0.6).unwrap_or(Vec2::ZERO);
        spawn_chunks(&mut commands, &fx, &mut rng, origin, sprite.color, inherited);
    }
}

/// Tumble, fade, despawn.
pub fn update_debris(
    time: Res<Time>,
    mut commands: Commands,
    mut debris_query: Query<(Entity, &mut Debris, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (entity, mut debris, mut transform, mut sprite) in debris_query.iter_mut() {
        debris.lifetime -= dt;
        if debris.lifetime <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation.x += debris.velocity.x * dt;
        transform.translation.y += debris.velocity.y * dt;
        transform.rotate_z(debris.angular_vel * dt);

        let frac = (debris.lifetime / debris.max_lifetime).clamp(0.0, 1.0);
        sprite.color = sprite.color.with_alpha(frac);
    }
}
