//! What the guns shoot at when the player hasn't locked anything by hand.
//!
//! Free aim sent every gun to the same cursor point, so a battery drilled one
//! tile at a time and a hull came apart in a single neat hole. Each gun now
//! picks its OWN block on the engaged ship and holds it for a moment before
//! rolling another, which spreads a salvo across the silhouette the way a
//! broadside should look.
//!
//! A right-click lock still overrides all of this — see [`AimLock`]. This is
//! only what happens in its absence.
//!
//! Marked ships ([`MarkedTargets`]: Shift + right-click one, Shift + `\` for
//! every hostile in range) split the battery: each turret and hull gun takes
//! one of them, spread evenly, among those it can reach.

use std::collections::HashMap;

use bevy::prelude::*;
use rand::Rng;

use crate::ai_ship::components::{AiShip, AiShipWreck};
use crate::building::ShipGrid;
use crate::components::*;

use super::{AimLock, TargetSelection};
use super::fire_groups::InterceptMode;

/// Furthest away an unlocked battery will pick a fight on its own. Matches
/// the ship-target range in `cycle_target`, so what `\` can select and what
/// the guns will engage unprompted are the same set.
const AUTO_ENGAGE_RANGE: f32 = 27_000.0;

/// How long one gun stays on one block before rolling another. Randomised per
/// gun so a battery drifts across the hull instead of every barrel jumping to
/// a new tile on the same frame.
const REAIM_MIN: f32 = 0.4;
const REAIM_MAX: f32 = 1.1;

/// The block one gun is working on while nothing is manually locked.
#[derive(Component)]
pub struct AutoAimPoint {
    /// The block itself, so a gun can tell when the tile it was drilling dies.
    pub block: Entity,
    /// The ship that owns it — lead prediction needs its velocity.
    pub ship: Entity,
    /// Refreshed every frame from the block's own transform, so the aim
    /// tracks a moving target rather than a stale world point.
    pub point: Vec2,
    ttl: f32,
    /// False for a gun with no marked ship it can hit: it holds fire rather
    /// than throw rounds that fall short.
    pub in_reach: bool,
}

/// Ships the player has marked for the battery to split across.
#[derive(Resource, Default)]
pub struct MarkedTargets {
    pub ships: Vec<Entity>,
}

/// Which marked ship a turret or gun takes: of those it can hit, the one the
/// fewest others are on, nearest first. `marks` are in that order.
pub fn pick_mark(marks: &[Entity], can_hit: impl Fn(Entity) -> bool, load: &HashMap<Entity, u32>) -> Option<Entity> {
    let mut best: Option<(Entity, u32)> = None;
    for &ship in marks.iter().filter(|e| can_hit(**e)) {
        let on_it = load.get(&ship).copied().unwrap_or(0);
        if best.is_none_or(|(_, least)| on_it < least) {
            best = Some((ship, on_it));
        }
    }
    best.map(|(ship, _)| ship)
}

/// One thing that aims at marked ships this frame.
struct MarkGun {
    entity: Entity,
    /// What moves it onto a target: its turret, or the gun itself.
    mount: Entity,
    on: Option<Entity>,
    reach: f32,
    /// A hull gun's barrel line and half arc; a turret turns to its target.
    arc: Option<(Vec2, Vec2, f32)>,
}

fn can_hit(gun: &MarkGun, at: Vec2, from: Vec2) -> bool {
    if from.distance(at) > gun.reach {
        return false;
    }
    gun.arc.is_none_or(|(muzzle, dir, cos_half)| dir.dot((at - muzzle).normalize_or_zero()) >= cos_half)
}

/// Assigns each player gun a block on the engaged enemy.
///
/// Runs before the firing systems and writes only a component, so the firing
/// system reads its aim point out of the query it already has. That matters:
/// `fire_weapons_system` sits on Bevy's 16-parameter ceiling and cannot take
/// another query.
pub fn assign_auto_aim(
    time: Res<Time>,
    mut commands: Commands,
    aim_lock: Res<AimLock>,
    selection: Res<TargetSelection>,
    player: Query<(Entity, &Transform), With<Ship>>,
    // Without<AiShipWreck> is what makes the "still a live ship" filter below
    // actually mean that: a wreck is the same entity as the ship that died and
    // keeps its AiShip component, so an unfiltered query has the whole battery
    // drilling derelicts out to AUTO_ENGAGE_RANGE without the player touching
    // a key.
    enemies: Query<(Entity, &Transform, &ShipGrid), (With<AiShip>, Without<AiShipWreck>, Without<Ship>)>,
    block_pos: Query<&GlobalTransform>,
    mut weapons: Query<(Entity, &ChildOf, Option<&mut AutoAimPoint>), (Or<(With<Weapon>, With<crate::combat::lasers::LaserEmitter>)>, Without<DestroyedModule>)>,
    mut marks: ResMut<MarkedTargets>,
    aimers: (
        Query<&ChildOf, With<crate::combat::turret_deck::TurretBody>>,
        Query<(&Module, Option<&Weapon>, Option<&WeaponMount>, Option<&crate::combat::gun_assembly::GunMuzzle>, Has<InterceptMode>)>,
    ),
) {
    let Ok((player_ship, player_tf)) = player.single() else { return };

    if marks.ships.iter().any(|e| !enemies.contains(*e)) {
        marks.ships.retain(|e| enemies.contains(*e));
    }

    // A manual lock owns the whole battery. Leave the guns alone.
    if aim_lock.is_locked() {
        return;
    }

    let player_pos = player_tf.translation.truncate();

    if !marks.ships.is_empty() {
        let (turrets, kinds) = &aimers;
        let at = |ship: Entity| enemies.get(ship).map_or(player_pos, |(_, t, _)| t.translation.truncate());
        let mut order = marks.ships.clone();
        order.sort_by(|a, b| player_pos.distance(at(*a)).total_cmp(&player_pos.distance(at(*b))));

        let mut guns: Vec<MarkGun> = Vec::new();
        for (entity, parent, point) in weapons.iter() {
            let holder = parent.parent();
            let on_turret = turrets.get(holder).is_ok_and(|c| c.parent() == player_ship);
            if holder != player_ship && !on_turret {
                continue;
            }
            let Ok((module, weapon, mount, muzzle, point_defence)) = kinds.get(entity) else { continue };
            let aims = matches!(
                module.module_type,
                ModuleType::Cannon | ModuleType::Railgun | ModuleType::Coilgun | ModuleType::Gatling | ModuleType::FiringPiece
            ) || (weapon.is_none() && on_turret);
            if point_defence || !aims {
                continue;
            }
            guns.push(MarkGun {
                entity,
                mount: if on_turret { holder } else { entity },
                on: point.map(|p| p.ship),
                reach: weapon.map_or(f32::INFINITY, |w| w.range),
                arc: match (on_turret, muzzle, mount) {
                    (false, Some(m), Some(arc)) => Some((m.world, m.dir, (arc.firing_arc / 2.0).to_radians().cos())),
                    _ => None,
                },
            });
        }

        // Each keeps the ship it's on while it can still hit it, unless the
        // marks changed: then they all deal again, so a new mark gets its share.
        let mut mounts: HashMap<Entity, Entity> = HashMap::new();
        let mut load: HashMap<Entity, u32> = HashMap::new();
        if !marks.is_changed() {
            for gun in &guns {
                if let Some(ship) = gun.on.filter(|s| order.contains(s) && can_hit(gun, at(*s), player_pos)) {
                    if !mounts.contains_key(&gun.mount) {
                        mounts.insert(gun.mount, ship);
                        *load.entry(ship).or_insert(0) += 1;
                    }
                }
            }
        }
        for gun in &guns {
            if !mounts.contains_key(&gun.mount) {
                if let Some(ship) = pick_mark(&order, |s| can_hit(gun, at(s), player_pos), &load) {
                    mounts.insert(gun.mount, ship);
                    *load.entry(ship).or_insert(0) += 1;
                }
            }
        }

        let dt = time.delta_secs();
        let mut rng = rand::thread_rng();
        for gun in &guns {
            // Nothing it can hit: it faces the nearest and holds fire.
            let ship = mounts.get(&gun.mount).copied().unwrap_or(order[0]);
            let in_reach = mounts.contains_key(&gun.mount);
            let Ok((_, _, grid)) = enemies.get(ship) else { continue };
            let live: Vec<Entity> = grid.cells.values().copied().collect();
            if live.is_empty() {
                continue;
            }
            let Ok((_, _, point)) = weapons.get_mut(gun.entity) else { continue };
            match point {
                Some(mut p) => {
                    p.ttl -= dt;
                    p.in_reach = in_reach;
                    if p.ttl <= 0.0 || p.ship != ship || !live.contains(&p.block) {
                        p.block = live[rng.gen_range(0..live.len())];
                        p.ship = ship;
                        p.ttl = rng.gen_range(REAIM_MIN..REAIM_MAX);
                    }
                    if let Ok(gt) = block_pos.get(p.block) {
                        p.point = gt.translation().truncate();
                    }
                }
                None => {
                    let block = live[rng.gen_range(0..live.len())];
                    let Ok(gt) = block_pos.get(block) else { continue };
                    commands.entity(gun.entity).try_insert(AutoAimPoint {
                        block,
                        ship,
                        point: gt.translation().truncate(),
                        ttl: rng.gen_range(REAIM_MIN..REAIM_MAX),
                        in_reach,
                    });
                }
            }
        }
        return;
    }

    // Whatever `\` last selected, as long as it is still a live ship;
    // otherwise the nearest one in range.
    let engaged = selection
        .target
        .filter(|e| enemies.contains(*e))
        .or_else(|| {
            enemies
                .iter()
                .map(|(e, t, _)| (e, player_pos.distance(t.translation.truncate())))
                .filter(|(_, d)| *d <= AUTO_ENGAGE_RANGE)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(e, _)| e)
        });

    let Some(engaged) = engaged else {
        // Nothing worth shooting. Drop the stale points so the guns fall back
        // to the cursor instead of holding aim on a ship that is gone.
        for (entity, _, point) in weapons.iter() {
            if point.is_some() {
                commands.entity(entity).remove::<AutoAimPoint>();
            }
        }
        return;
    };

    let Ok((_, _, grid)) = enemies.get(engaged) else { return };
    // ShipGrid holds LIVE blocks only — destroyed ones drop out of it — so a
    // gun can never be assigned a tile that has already been shot away.
    // Multi-cell modules appear once per cell they occupy, which is a bias
    // worth keeping: a bigger block is a bigger thing to hit.
    let live: Vec<Entity> = grid.cells.values().copied().collect();
    if live.is_empty() {
        return;
    }

    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (entity, parent, point) in weapons.iter_mut() {
        if parent.parent() != player_ship {
            // Marks released: a turret goes back to following the mouse.
            if point.is_some() {
                commands.entity(entity).remove::<AutoAimPoint>();
            }
            continue;
        }

        match point {
            Some(mut p) => {
                p.ttl -= dt;
                p.in_reach = true;
                let stale = p.ship != engaged || !live.contains(&p.block);
                if p.ttl <= 0.0 || stale {
                    p.block = live[rng.gen_range(0..live.len())];
                    p.ship = engaged;
                    p.ttl = rng.gen_range(REAIM_MIN..REAIM_MAX);
                }
                if let Ok(gt) = block_pos.get(p.block) {
                    p.point = gt.translation().truncate();
                }
            }
            None => {
                let block = live[rng.gen_range(0..live.len())];
                let Ok(gt) = block_pos.get(block) else { continue };
                commands.entity(entity).try_insert(AutoAimPoint {
                    block,
                    ship: engaged,
                    point: gt.translation().truncate(),
                    ttl: rng.gen_range(REAIM_MIN..REAIM_MAX),
                    in_reach: true,
                });
            }
        }
    }
}

/// Shift + `\`: mark every hostile ship in range, or clear the marks.
#[allow(clippy::type_complexity)]
pub fn mark_all_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut marks: ResMut<MarkedTargets>,
    mut lock: ResMut<AimLock>,
    standing: Res<crate::diplomacy::FactionStanding>,
    player: Query<(Entity, &Transform), With<Ship>>,
    enemies: Query<(Entity, &Transform, &crate::ai_ship::components::AiShipType, &crate::ai_ship::components::AiShipState), (With<AiShip>, Without<AiShipWreck>, Without<Ship>)>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    if !keyboard.just_pressed(KeyCode::Backslash) || !keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        return;
    }
    let Ok((me, tf)) = player.single() else { return };
    let message = if marks.ships.is_empty() {
        marks.ships = enemies
            .iter()
            .filter(|(_, t, ..)| tf.translation.truncate().distance(t.translation.truncate()) <= AUTO_ENGAGE_RANGE)
            .filter(|(_, _, faction, state)| standing.attacks_player(**faction) || state.last_attacker == Some(me))
            .map(|(e, ..)| e)
            .collect();
        if marks.ships.is_empty() {
            "No hostile ships in range".to_string()
        } else {
            lock.clear();
            format!("{} ships marked - guns split across them.", marks.ships.len())
        }
    } else {
        marks.ships.clear();
        "Marks cleared.".to_string()
    };
    notifications.write(crate::events::ShowNotification {
        message,
        notification_type: crate::events::NotificationType::Info,
        duration: 2.0,
    });
}

/// A bracket around each marked ship.
#[derive(Component)]
pub struct MarkBracket;

pub fn draw_marks(
    mut commands: Commands,
    marks: Res<MarkedTargets>,
    existing: Query<Entity, With<MarkBracket>>,
    ships: Query<(&Transform, Option<&crate::combat::shields::ShipShield>)>,
    camera: Query<&Projection, With<crate::camera::MainCamera>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).try_despawn();
    }
    let scale = match camera.single() {
        Ok(Projection::Orthographic(o)) => crate::camera::label_scale(o.scale),
        _ => 1.0,
    };
    let (thick, color) = (2.0 * scale, Color::srgb(1.0, 0.86, 0.35));
    for ship in &marks.ships {
        let Ok((transform, shield)) = ships.get(*ship) else { continue };
        let (centre, size) = match shield {
            Some(s) => (s.world_center(transform), s.radius * 2.0 + 20.0),
            None => (transform.translation.truncate(), 100.0),
        };
        let arm = size * 0.2;
        for (sx, sy) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let corner = centre + Vec2::new(sx * size / 2.0, sy * size / 2.0);
            commands.spawn((
                Sprite { color, custom_size: Some(Vec2::new(arm, thick)), ..default() },
                Transform::from_xyz(corner.x - sx * arm * 0.5, corner.y, 6.0),
                MarkBracket,
            ));
            commands.spawn((
                Sprite { color, custom_size: Some(Vec2::new(thick, arm)), ..default() },
                Transform::from_xyz(corner.x, corner.y - sy * arm * 0.5, 6.0),
                MarkBracket,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ship(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn guns_spread_evenly_over_marks_nearest_first() {
        let marks = [ship(1), ship(2), ship(3)];
        let mut load = HashMap::new();
        let mut order = Vec::new();
        for _ in 0..7 {
            let on = pick_mark(&marks, |_| true, &load).unwrap();
            *load.entry(on).or_insert(0) += 1;
            order.push(on);
        }
        assert_eq!(order[..3], marks);
        assert_eq!((load[&ship(1)], load[&ship(2)], load[&ship(3)]), (3, 2, 2));
    }

    #[test]
    fn a_gun_only_takes_a_mark_it_can_hit() {
        let marks = [ship(1), ship(2)];
        let load = HashMap::from([(ship(2), 5)]);
        assert_eq!(pick_mark(&marks, |s| s == ship(2), &load), Some(ship(2)));
        assert_eq!(pick_mark(&marks, |_| false, &load), None);
    }

    #[test]
    fn a_hull_gun_only_hits_what_its_arc_covers() {
        let gun = MarkGun {
            entity: ship(9),
            mount: ship(9),
            on: None,
            reach: 1000.0,
            arc: Some((Vec2::ZERO, Vec2::X, 0.9)),
        };
        assert!(can_hit(&gun, Vec2::new(500.0, 0.0), Vec2::ZERO));
        assert!(!can_hit(&gun, Vec2::new(-500.0, 0.0), Vec2::ZERO), "behind it");
        assert!(!can_hit(&gun, Vec2::new(5000.0, 0.0), Vec2::ZERO), "out of reach");
    }
}
