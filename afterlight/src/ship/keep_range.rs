//! Right-click a ship and your own ship holds its fighting range on it.
//!
//! The AI has always done this (ai_ship::movement's standoff): it parks where
//! its main battery bears and stays there. The player had to do it by hand
//! with WASD while also aiming, so most fights drifted into point blank or out
//! past the guns. Now a right-click lock on a ship also takes the helm for
//! distance only: the nose still follows the cursor and the turrets still aim
//! themselves; the ship slides in or out along the line to the target until
//! it sits where half its gun firepower reaches (`Hull::fighting`), never
//! closer than the two hulls allow.
//!
//! Any WASD or brake gives the helm straight back. Right-click the ship again
//! to hand it over again.

use bevy::prelude::*;

use crate::ai_ship::awareness::Battlefield;
use crate::combat::targeting::AimLock;
use crate::components::*;
use crate::events::*;
use crate::resources::InputState;

/// Holding range on this ship.
#[derive(Resource, Default)]
pub struct KeepRange {
    pub ship: Option<Entity>,
}

/// How far into its fighting reach the ship sits: inside it, so a target
/// edging away or turning doesn't take it straight out of range.
const INTO_REACH: f32 = 0.8;
/// Clear water kept between the two hulls whatever the guns say.
const HULL_GAP: f32 = 600.0;
/// No correction within this fraction of the hold distance: it parks instead
/// of hunting back and forth.
const DEADBAND: f32 = 0.06;
/// Closing speed it asks for per unit of distance off, and the most it asks.
const SPEED_PER_UNIT: f32 = 0.4;
const MAX_CLOSING: f32 = 900.0;
/// Speed error that takes full thrust.
const FULL_THRUST_AT: f32 = 350.0;

/// Where to hold, centre to centre: inside its fighting reach, measured to the
/// target's near side, and never into the other hull.
pub fn hold_distance(fighting_reach: f32, own_radius: f32, target_radius: f32) -> f32 {
    (fighting_reach * INTO_REACH + target_radius).max(own_radius + target_radius + HULL_GAP)
}

/// Thrust along the line to the target, -1 (back off) to 1 (close), for a
/// ship `distance` off that wants `hold`, closing at `closing` u/s.
pub fn range_thrust(distance: f32, hold: f32, closing: f32) -> f32 {
    let off = distance - hold;
    let want = if off.abs() < hold * DEADBAND {
        0.0
    } else {
        (off * SPEED_PER_UNIT).clamp(-MAX_CLOSING, MAX_CLOSING)
    };
    ((want - closing) / FULL_THRUST_AT).clamp(-1.0, 1.0)
}

/// Takes and gives back the helm, and works out the thrust for this frame.
#[allow(clippy::too_many_arguments)]
pub fn keep_range(
    lock: Res<AimLock>,
    field: Res<Battlefield>,
    mut keep: ResMut<KeepRange>,
    mut input: ResMut<InputState>,
    me: Query<(Entity, &GlobalTransform, &Velocity), With<Ship>>,
    targets: Query<&GlobalTransform>,
    mut notifications: MessageWriter<ShowNotification>,
    mut was_steering: Local<bool>,
    time: Res<Time>,
    power: Res<crate::resources::PowerChannels>,
) {
    input.autopilot = Vec2::ZERO;
    let mut say = |message: String| {
        notifications.write(ShowNotification { message, notification_type: NotificationType::Info, duration: 2.5 });
    };
    let steering = input.movement != Vec2::ZERO || input.brake;
    // A fresh press, not a held key: a right-click made with W still down
    // takes the helm as soon as W comes up, instead of being cancelled by it
    // on the same frame.
    let took_helm = steering && !*was_steering;
    *was_steering = steering;

    // A right-click that locked a ship hands over the helm.
    if lock.click_consumed && lock.is_locked() && keep.ship != lock.ship {
        keep.ship = lock.ship;
        say("Holding gun range - WASD takes the helm back".into());
    } else if keep.ship.is_some() && took_helm {
        keep.ship = None;
        say("Manual helm - right-click the ship to hold range again".into());
    }
    // AFTERLIGHT_KEEP_RANGE=1 (testing): hold on the nearest ship, no click.
    let trial = std::env::var("AFTERLIGHT_KEEP_RANGE").is_ok();
    if trial && keep.ship.is_none() {
        if let Some(player) = field.player.and_then(|p| field.hulls.get(&p)) {
            keep.ship = field
                .hulls
                .iter()
                .filter(|(e, _)| Some(**e) != field.player)
                .min_by(|a, b| a.1.pos.distance(player.pos).total_cmp(&b.1.pos.distance(player.pos)))
                .map(|(e, _)| *e);
        }
    }
    // Lock dropped or moved off a ship: nothing to hold on.
    if !trial && keep.ship.is_some() && lock.ship != keep.ship {
        keep.ship = None;
    }
    let Some(target) = keep.ship else { return };
    if steering {
        return;
    }

    let Ok((player, gt, vel)) = me.single() else { return };
    let Ok(target_gt) = targets.get(target) else { return };
    let (Some(mine), Some(theirs)) = (field.hulls.get(&player), field.hulls.get(&target)) else { return };
    let reach = mine.fighting();
    if reach <= 0.0 {
        return;
    }

    let to = target_gt.translation().truncate() - gt.translation().truncate();
    let distance = to.length();
    if distance < 1.0 {
        return;
    }
    let dir = to / distance;
    let closing = (vel.0 - theirs.vel).dot(dir);
    let hold = hold_distance(reach, mine.radius, theirs.radius);
    input.autopilot = dir * range_thrust(distance, hold, closing);
    if crate::combat::gun_assembly::trace() && (time.elapsed_secs() % 2.0) < time.delta_secs() {
        info!("KEEP RANGE {:.0} u off, holding {:.0} (reach {:.0}), closing {:.0} u/s, own speed {:.0}, thrust {:.2}, engine power {:.2}", distance, hold, reach, closing, vel.0.length(), input.autopilot.length(), power.engines_mult);
    }
}

/// A faint ring round the target at the distance being held.
pub fn draw_keep_range(
    mut gizmos: Gizmos,
    keep: Res<KeepRange>,
    field: Res<Battlefield>,
    targets: Query<&GlobalTransform>,
) {
    let Some(target) = keep.ship else { return };
    let Some(player) = field.player else { return };
    let (Some(mine), Some(theirs)) = (field.hulls.get(&player), field.hulls.get(&target)) else { return };
    let Ok(gt) = targets.get(target) else { return };
    let hold = hold_distance(mine.fighting(), mine.radius, theirs.radius);
    gizmos
        .circle_2d(gt.translation().truncate(), hold, Color::srgba(0.55, 0.85, 1.0, 0.18))
        .resolution(96);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_holds_inside_its_reach_and_off_the_other_hull() {
        // 20 km guns on two 1 km ships: 16 km to the target's near side.
        assert_eq!(hold_distance(20_000.0, 1_000.0, 1_000.0), 17_000.0);
        // Guns shorter than the hulls: it still doesn't ram.
        assert_eq!(hold_distance(500.0, 1_000.0, 1_000.0), 2_600.0);
    }

    #[test]
    fn it_closes_when_far_backs_off_when_near_and_parks_between() {
        let hold = 10_000.0;
        assert_eq!(range_thrust(30_000.0, hold, 0.0), 1.0, "far and still: full ahead");
        assert_eq!(range_thrust(3_000.0, hold, 0.0), -1.0, "too close: full astern");
        assert_eq!(range_thrust(10_300.0, hold, 0.0), 0.0, "inside the deadband: hold");
    }

    #[test]
    fn it_brakes_before_it_gets_there() {
        // Close and already closing fast: thrust comes off before arriving.
        let hold = 10_000.0;
        assert!(range_thrust(11_000.0, hold, 900.0) < 0.0);
        // Parked but drifting in: it pushes back out.
        assert!(range_thrust(10_000.0, hold, 300.0) < 0.0);
    }
}
