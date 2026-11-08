use bevy::prelude::*;
use super::new_projectiles::Projectile;
use super::new_projectiles::MissileProjectile;

// ============================================================================
// ENTITY LIMITS
// Prevents runaway entity spawning from breaking performance.
// ============================================================================

/// Max projectiles alive at once (kinetic + missiles combined)
pub const MAX_PROJECTILES: usize = 1000;

/// Max blocks on the player ship. Was 250, then 500; ships are meant to run
/// to capitals of ~4,000 blocks now (the faction capitals are already 1,400
/// to 2,300), and the per-ship rebuilds in building::layout were made so a
/// hull that size doesn't cost every frame. Still a bound on entity count.
pub const MAX_SHIP_BLOCKS: usize = 5_000;

/// System: despawn oldest projectiles if over the limit
pub fn enforce_projectile_limit(
    mut commands: Commands,
    projectile_query: Query<(Entity, &Projectile)>,
    missile_query: Query<(Entity, &MissileProjectile)>,
) {
    let proj_count = projectile_query.iter().count();
    let missile_count = missile_query.iter().count();
    let total = proj_count + missile_count;

    if total <= MAX_PROJECTILES { return; }

    // Despawn excess projectiles (oldest first — lowest lifetime remaining)
    let excess = total - MAX_PROJECTILES;
    let mut to_remove: Vec<(Entity, f32)> = projectile_query.iter()
        .map(|(e, p)| (e, p.lifetime))
        .collect();
    to_remove.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (entity, _) in to_remove.iter().take(excess) {
        commands.entity(*entity).try_despawn();
    }
}

/// Max effect particles alive at once.
///
/// Nothing bounded these before, which was survivable while an explosion was
/// one flat square. Routing the game's detonations through `spawn_explosion`
/// turned each of ~18 of those call sites into a ~34-entity composite, and a
/// ship's death rattle fires up to a dozen of them a second apart — so a
/// three-ship pile-up can now ask for well over a thousand short-lived
/// sprites at once. They all expire inside ~2s on their own; this only stops
/// a pathological frame from turning into a stall.
pub const MAX_PARTICLES: usize = 1600;

/// System: despawn the nearest-to-death particles if over the limit.
///
/// Culling by *lowest remaining lifetime* means the ones about to vanish go
/// first, so a cull is much harder to see than dropping the newest — which
/// would delete the explosion that just happened.
pub fn enforce_particle_limit(
    mut commands: Commands,
    particle_query: Query<(Entity, &crate::vfx::particles::Particle)>,
) {
    let total = particle_query.iter().count();
    if total <= MAX_PARTICLES { return; }

    let excess = total - MAX_PARTICLES;
    let mut to_remove: Vec<(Entity, f32)> = particle_query
        .iter()
        .map(|(e, p)| (e, p.lifetime))
        .collect();
    to_remove.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (entity, _) in to_remove.iter().take(excess) {
        commands.entity(*entity).try_despawn();
    }
}
