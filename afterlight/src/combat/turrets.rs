use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use crate::components::{Ship, Turret, TurretBarrel};

/// Ease `cur` toward `target` (radians) by at most `max_step`, the short way round.
fn approach_angle(cur: f32, target: f32, max_step: f32) -> f32 {
    let mut diff = (target - cur).rem_euclid(TAU);
    if diff > PI { diff -= TAU; }
    if diff.abs() <= max_step { cur + diff } else { cur + diff.signum() * max_step }
}

/// Traverse each gun turret's barrel toward its aim target — the mouse cursor for
/// the player ship, the player ship for AI ships — capped at the turret's turn
/// speed so heavy guns swing slowly and light ones snap around. The barrel is a
/// pivot-centred child sprite; we set its LOCAL rotation from the desired WORLD
/// heading minus the module's world rotation, so it tracks correctly however the
/// ship (and the module) is oriented.
pub fn aim_turrets(
    time: Res<Time>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    ships: Query<(), With<Ship>>,
    player_ship: Query<&GlobalTransform, With<Ship>>,
    mut turrets: Query<(&GlobalTransform, &mut Turret, &ChildOf, Option<&crate::building::customization::tuning::WeaponTuning>)>,
    mut barrels: Query<(&ChildOf, &mut Transform), With<TurretBarrel>>,
    aim_lock: Res<crate::combat::targeting::AimLock>,
) {
    let dt = time.delta_secs();

    let cursor_world: Option<Vec2> = windows.single().ok()
        .and_then(|w| w.cursor_position())
        .and_then(|c| camera.single().ok().and_then(|(cam, gt)| cam.viewport_to_world_2d(gt, c).ok()));
    let player_pos = player_ship.iter().next().map(|g| g.translation().truncate());

    // 1) ease each turret's WORLD heading toward its target
    for (mod_gt, mut turret, ship_parent, tuning) in turrets.iter_mut() {
        let is_player = ships.get(ship_parent.parent()).is_ok();
        // Locked guns look where they shoot — the barrels swinging onto the
        // locked block is most of what sells the lock as real.
        let target = if is_player { aim_lock.aim_point().or(cursor_world) } else { player_pos };
        let Some(target) = target else { continue };
        let mod_pos = mod_gt.translation().truncate();
        let dir = target - mod_pos;
        if dir.length_squared() < 4.0 { continue; }
        // barrel art points +Y (up), so a heading of `atan2 - 90°` aims the barrel
        let desired = dir.y.atan2(dir.x) - FRAC_PI_2;
        // per-weapon traverse customization scales the base turn speed
        let speed = turret.turn_speed * tuning.map(|t| t.traverse).unwrap_or(1.0);
        turret.world_angle = approach_angle(turret.world_angle, desired, speed * dt);
    }

    // 2) apply to each barrel: local = world heading - module world rotation
    for (barrel_parent, mut btf) in barrels.iter_mut() {
        if let Ok((mod_gt, turret, _, _)) = turrets.get(barrel_parent.parent()) {
            let (_, q, _) = mod_gt.to_scale_rotation_translation();
            let mod_world = q.to_euler(EulerRot::ZYX).0;
            btf.rotation = Quat::from_rotation_z(turret.world_angle - mod_world);
        }
    }
}
