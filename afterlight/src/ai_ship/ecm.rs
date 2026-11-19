//! Enemy jammers and stealth.
//!
//! The Broken Choir carries signal jammers by the hundred and the Silence
//! sets stealth coating into its walls; on an enemy ship both used to do
//! nothing. Now:
//!
//!  - A ship with live jammers, in a fight, throws a jamming field round
//!    itself, wider the more jammers it has. Inside one, your radar scope
//!    fills with static and your guided missiles' seekers can't hold a lock:
//!    radar and heat heads go blind, command-link ones wander. Guns don't
//!    care, which is the counter: shoot, don't launch, and the jammers
//!    themselves are big targets.
//!  - A ship with stealth coating stays off your radar scope until it is
//!    close. You see it when you look out of the window, not before.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use super::components::*;
use crate::components::*;
use crate::events::{NotificationType, ShowNotification};

/// A jamming field's reach: a base, a little more per jammer, up to a cap.
const JAM_BASE: f32 = 6_000.0;
const JAM_PER_JAMMER: f32 = 400.0;
const JAM_MAX: f32 = 14_000.0;
/// How close a stealthy hull has to be before it shows on your scope.
pub const STEALTH_REVEAL: f32 = 4_000.0;

/// This frame's jamming fields and hidden hulls.
#[derive(Resource, Default, Debug)]
pub struct Ecm {
    /// (centre, radius, the ship throwing it)
    pub fields: Vec<(Vec2, f32, Entity)>,
    pub hidden: HashSet<Entity>,
}

impl Ecm {
    /// Is a point inside anyone's jamming field?
    pub fn jams(&self, at: Vec2) -> bool {
        self.fields.iter().any(|(c, r, _)| c.distance(at) < *r)
    }
}

/// How far `jammers` live jammers throw their field.
pub fn jam_radius(jammers: u32) -> f32 {
    if jammers == 0 { 0.0 } else { (JAM_BASE + JAM_PER_JAMMER * jammers as f32).min(JAM_MAX) }
}

#[allow(clippy::type_complexity)]
pub fn ai_ecm_system(
    mut ecm: ResMut<Ecm>,
    ships: Query<(Entity, &Transform, &AiShipBehavior, &AiShipState, &Children), With<AiShip>>,
    modules: Query<&Module, Without<DestroyedModule>>,
    player: Query<&Transform, (With<Ship>, Without<AiShip>)>,
    mut was_jammed: Local<bool>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    ecm.fields.clear();
    ecm.hidden.clear();
    for (ship, tf, behavior, state, children) in ships.iter() {
        if state.is_destroyed {
            continue;
        }
        let (mut jammers, mut coated) = (0u32, false);
        for m in children.iter().filter_map(|c| modules.get(c).ok()) {
            match m.module_type {
                // Enemy utility blocks aren't switched on the way the
                // player's are: a jammer works while it's whole.
                ModuleType::SignalJammer if m.health > 0.0 => jammers += 1,
                ModuleType::StealthCoating => coated = true,
                _ => {}
            }
        }
        if coated {
            ecm.hidden.insert(ship);
        }

        // Jamming is a fighting measure: a ship at peace keeps quiet.
        if jammers > 0 && matches!(behavior, AiShipBehavior::Engaging | AiShipBehavior::Fleeing) {
            ecm.fields.push((tf.translation.truncate(), jam_radius(jammers), ship));
        }
    }
    let jammed = player.single().is_ok_and(|p| ecm.jams(p.translation.truncate()));
    if jammed != *was_jammed && crate::combat::gun_assembly::trace() {
        info!("ECM: player {} ({} fields, {} hidden hulls)", if jammed { "jammed" } else { "clear" }, ecm.fields.len(), ecm.hidden.len());
    }
    if jammed && !*was_jammed {
        notifications.write(ShowNotification {
            message: "Jammed - radar and missile seekers scrambled".into(),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
    }
    *was_jammed = jammed;
}

/// Where a hull shows on a jammed scope: shaken by static, `seed` keeping
/// each contact's shake its own.
pub fn static_offset(seed: u64, now: f32, amount: f32) -> Vec2 {
    let a = ((seed % 97) as f32 * 0.37 + now * 7.3).sin();
    let b = ((seed % 89) as f32 * 0.53 + now * 5.1).cos();
    Vec2::new(a, b) * amount
}

/// Should a hull this far off show on the scope at all?
pub fn on_scope(hidden: bool, dist: f32) -> bool {
    !hidden || dist <= STEALTH_REVEAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_jammers_throw_a_wider_field_up_to_a_cap() {
        assert_eq!(jam_radius(0), 0.0);
        assert!(jam_radius(10) > jam_radius(2));
        assert_eq!(jam_radius(500), JAM_MAX);
    }

    #[test]
    fn a_point_inside_any_field_is_jammed() {
        let ecm = Ecm { fields: vec![(Vec2::ZERO, 8_000.0, Entity::PLACEHOLDER)], hidden: HashSet::default() };
        assert!(ecm.jams(Vec2::new(7_000.0, 0.0)));
        assert!(!ecm.jams(Vec2::new(9_000.0, 0.0)));
        assert!(!Ecm::default().jams(Vec2::ZERO));
    }

    #[test]
    fn a_stealthy_hull_shows_only_up_close() {
        assert!(on_scope(false, 30_000.0));
        assert!(!on_scope(true, 10_000.0));
        assert!(on_scope(true, STEALTH_REVEAL - 1.0));
    }

    #[test]
    fn static_stays_within_its_amount() {
        for i in 0..50 {
            let o = static_offset(i, i as f32 * 0.31, 20.0);
            assert!(o.x.abs() <= 20.0 && o.y.abs() <= 20.0);
        }
    }
}
