//! Enemy tractor beams pull.
//!
//! The Stellar Preserve hangs tractor booms off every ark, the Shepherd lines
//! its hook with them, and the Gilded Throne's Debt Collectors carry them to
//! repossess. In an enemy's hands they used to fire rounds that did nothing.
//! Now a hostile ship with live beams, in a fight, drags its target toward a
//! point just off its own hull, and holds it there: what it catches it keeps,
//! carried wherever the catcher goes.
//!
//! The pull is measured against the target's own drive, never quite matching
//! a ship at full thrust: run with every engine lit and you claw free slowly.
//! So the counter is the doctrine's counter: keep your engines and power up
//! (the Preserve's EMP and the Shepherd's ion fire are there to stop that),
//! or shoot the beams out.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use super::awareness::Battlefield;
use super::components::*;
use crate::components::*;

/// How far a tractor reaches, from its own hull to the target's.
pub const TRACTOR_REACH: f32 = 7_000.0;
/// Each live beam's share of the target's best acceleration.
const PULL_PER_BEAM: f32 = 0.15;
/// Never quite all of it: a ship at full thrust always claws free, slowly.
const MAX_PULL: f32 = 0.85;
/// Where a catch is held: this far off the catcher's hull.
const HOLD_GAP: f32 = 900.0;
/// Pull on another AI ship, units/s² per beam, up to a cap. They steer by
/// blending toward where they want to be, so a steady pull still tells.
const AI_PULL_PER_BEAM: f32 = 8.0;
const AI_PULL_MAX: f32 = 80.0;

/// The best acceleration the player's drive gives this frame (ship_movement
/// writes it), so a tractor can be measured against it.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct PlayerDrive {
    pub accel: f32,
}

/// The beam drawn between a catcher and its catch.
#[derive(Component)]
pub struct TractorFx;

/// How hard `beams` live tractors pull on a ship whose own drive gives
/// `drive_accel` at full thrust.
pub fn pull_strength(beams: u32, drive_accel: f32) -> f32 {
    (beams as f32 * PULL_PER_BEAM).min(MAX_PULL) * drive_accel.max(0.0)
}

/// The pull on a catch at `catch` from a catcher wanting it at `hold`: full
/// strength from afar, easing off as it arrives, so it settles rather than
/// bouncing through the hold point.
/// A beam only pulls: a catch already closer than the hold point is held
/// where it is, never pushed out.
pub fn pull_toward(catch: Vec2, hold: Vec2, catcher: Vec2, strength: f32) -> Vec2 {
    let to_hold = hold - catch;
    let d = to_hold.length();
    if d < 1.0 || catch.distance(catcher) <= hold.distance(catcher) {
        return Vec2::ZERO;
    }
    to_hold / d * strength * (d / 1_500.0).min(1.0)
}

#[allow(clippy::type_complexity)]
pub fn ai_tractor_system(
    time: Res<Time>,
    mut commands: Commands,
    field: Res<Battlefield>,
    drive: Res<PlayerDrive>,
    ships: Query<(Entity, &Transform, &AiShipBehavior, &AiShipTarget, &AiShipState, &Children), (With<AiShip>, Without<TractorFx>)>,
    modules: Query<&Module, Without<DestroyedModule>>,
    mut targets: Query<(&Transform, &mut Velocity, Has<Ship>), Without<TractorFx>>,
    mut fx: Query<(&mut Transform, &mut Sprite), (With<TractorFx>, Without<AiShip>, Without<Ship>)>,
    mut beams_drawn: Local<HashMap<Entity, Entity>>,
    // Each hull's live beams, counted twice a second rather than every frame.
    mut counted: Local<(f32, HashMap<Entity, u32>)>,
) {
    let dt = time.delta_secs();
    counted.0 -= dt;
    if counted.0 <= 0.0 {
        counted.0 = 0.5;
        counted.1.clear();
    }
    let mut pulling: HashMap<Entity, (Vec2, Vec2)> = HashMap::default();

    for (ship, tf, behavior, target, state, children) in ships.iter() {
        if state.is_destroyed || !matches!(behavior, AiShipBehavior::Engaging | AiShipBehavior::Fleeing) {
            continue;
        }
        let Some(catch) = target.entity else { continue };
        let beams = *counted.1.entry(ship).or_insert_with(|| {
            children
                .iter()
                .filter_map(|c| modules.get(c).ok())
                .filter(|m| m.module_type == ModuleType::TractorBeam && m.is_active)
                .count() as u32
        });
        if beams == 0 {
            continue;
        }
        let Ok((catch_tf, mut vel, is_player)) = targets.get_mut(catch) else { continue };
        let me = tf.translation.truncate();
        let them = catch_tf.translation.truncate();
        let my_r = field.hulls.get(&ship).map_or(0.0, |h| h.radius);
        let their_r = field.hulls.get(&catch).map_or(0.0, |h| h.radius);
        let gap = me.distance(them) - my_r - their_r;
        if gap > TRACTOR_REACH {
            continue;
        }
        let dir = (them - me).normalize_or(Vec2::X);
        let hold = me + dir * (my_r + their_r + HOLD_GAP);
        let strength = if is_player {
            pull_strength(beams, drive.accel)
        } else {
            (beams as f32 * AI_PULL_PER_BEAM).min(AI_PULL_MAX)
        };
        vel.0 += pull_toward(them, hold, me, strength) * dt;
        if crate::combat::gun_assembly::trace() && (time.elapsed_secs() % 1.0) < dt {
            info!("TRACTOR {ship:?} -> {catch:?} (player {is_player}): {beams} beams, gap {gap:.0}, pull {strength:.1} vs drive {:.1}", drive.accel);
        }
        // Held, it goes where the catcher goes.
        if them.distance(me) < hold.distance(me) + 1_200.0 {
            let carry = field.hulls.get(&ship).map_or(Vec2::ZERO, |h| h.vel);
            vel.0 = vel.0.lerp(carry, (0.6 * dt * (strength / 50.0).min(1.0)).min(1.0));
        }
        pulling.insert(ship, (me + dir * my_r, them - dir * their_r));
    }

    // The beams: one pale band per catcher, from its hull to the catch's.
    for (ship, (from, to)) in pulling.iter() {
        let mid = (*from + *to) * 0.5;
        let len = from.distance(*to).max(1.0);
        let angle = (*to - *from).to_angle();
        let entity = *beams_drawn.entry(*ship).or_insert_with(|| {
            commands
                .spawn((
                    TractorFx,
                    Sprite { color: Color::srgba(0.45, 0.95, 0.95, 0.22), custom_size: Some(Vec2::new(len, 40.0)), ..default() },
                    Transform::from_translation(mid.extend(0.4)).with_rotation(Quat::from_rotation_z(angle)),
                ))
                .id()
        });
        if let Ok((mut t, mut s)) = fx.get_mut(entity) {
            t.translation = mid.extend(0.4);
            t.rotation = Quat::from_rotation_z(angle);
            // A slow shimmer, so it reads as a field rather than a bar.
            let shimmer = 0.18 + 0.08 * (time.elapsed_secs() * 3.0).sin().abs();
            s.color = Color::srgba(0.45, 0.95, 0.95, shimmer);
            s.custom_size = Some(Vec2::new(len, 40.0));
        }
    }
    beams_drawn.retain(|ship, beam| {
        let keep = pulling.contains_key(ship);
        if !keep {
            commands.entity(*beam).try_despawn();
        }
        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ship_at_full_thrust_always_claws_free() {
        let drive = 40.0;
        for beams in 0..30 {
            assert!(pull_strength(beams, drive) < drive, "{beams} beams");
        }
        // More beams pull harder, up to the cap.
        assert!(pull_strength(4, drive) > pull_strength(2, drive));
        assert_eq!(pull_strength(10, drive), pull_strength(20, drive));
        assert_eq!(pull_strength(0, drive), 0.0);
    }

    #[test]
    fn a_crippled_ship_is_caught() {
        // Engines knocked out to a third: three beams already out-pull what's left.
        let healthy = 40.0;
        let crippled = healthy / 3.0;
        assert!(pull_strength(3, healthy) > crippled);
    }

    #[test]
    fn the_pull_eases_as_the_catch_arrives() {
        // The catcher sits at the origin and holds its catch 1,000 out.
        let (catcher, hold) = (Vec2::ZERO, Vec2::new(1_000.0, 0.0));
        let far = pull_toward(Vec2::new(6_000.0, 0.0), hold, catcher, 30.0);
        let near = pull_toward(Vec2::new(1_300.0, 0.0), hold, catcher, 30.0);
        assert!(far.x < 0.0 && near.x < 0.0, "toward the hold point");
        assert!((far.length() - 30.0).abs() < 0.01);
        assert!(near.length() < far.length() * 0.5);
        // Closer in than the hold point: held, never pushed back out.
        assert_eq!(pull_toward(Vec2::new(400.0, 0.0), hold, catcher, 30.0), Vec2::ZERO);
    }
}
