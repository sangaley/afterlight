use bevy::prelude::*;
use std::f32::consts::PI;

use super::*;
use crate::resources::TargetingBonus;

/// Crew assigned to weapon modules auto-fire projectiles at creatures in range.
/// Checks power connectivity, firing arcs, damage state, staffing, and ammo type.
pub(super) fn crew_weapon_system(
    time: Res<Time>,
    mut weapon_query: Query<(
        Entity, &mut Weapon, &mut WeaponCooldown, &Module,
        Option<&CalculatedStats>, &WeaponMount, &AmmoStorage,
        Option<&CrewStation>,
    )>,
    creature_query: Query<(Entity, &Transform), (With<Creature>, Without<Ship>)>,
    crew_query: Query<&CrewMember>,
    ship_query: Query<(&Transform, &ShipPhysics), With<Ship>>,
    power_graph: Res<PowerGraph>,
    targeting_bonus: Res<TargetingBonus>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok((ship_transform, ship_physics)) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    // Pre-compute closest creature
    let mut closest_creature: Option<(Entity, f32, Vec2)> = None;
    for (_c_entity, c_transform) in creature_query.iter() {
        let c_pos = c_transform.translation.truncate();
        let dist = c_pos.distance(ship_pos);
        if closest_creature.map_or(true, |(_, d, _)| dist < d) {
            closest_creature = Some((_c_entity, dist, c_pos));
        }
    }

    for (_weapon_entity, mut weapon, mut cooldown, module, calculated_stats, weapon_mount, ammo_storage, crew_station) in weapon_query.iter_mut() {
        if !module.is_active || weapon.ammo == 0 {
            continue;
        }

        // Power check — unpowered weapons don't fire
        if !power_graph.powered_tiles.contains(&module.grid_position) {
            continue;
        }

        // Mine layers don't auto-fire
        if ammo_storage.ammo_type == AmmoType::Mine {
            continue;
        }

        // Weapons require a staffed crew member to auto-fire
        let is_staffed = crew_station
            .and_then(|cs| cs.assigned_crew)
            .and_then(|crew_entity| crew_query.get(crew_entity).ok())
            .map_or(false, |crew| crew.health > 0.0);

        if !is_staffed { continue; }

        // Tick cooldown
        cooldown.timer.tick(time.delta());
        if !cooldown.timer.is_finished() {
            continue;
        }

        let fire_rate = get_weapon_fire_rate(calculated_stats, &weapon);
        let range = get_weapon_range(calculated_stats, &weapon);
        let damage = get_weapon_damage(calculated_stats, &weapon);

        // Damage state penalty
        let health_ratio = module.health / module.max_health;
        let efficiency = ModuleDamageState::from_health_ratio(health_ratio).efficiency();

        // Fire rate modified by damage state (no skill modifier — crew are interchangeable)
        let modified_rate = fire_rate * efficiency;
        if modified_rate <= 0.0 { continue; }
        cooldown.timer = Timer::from_seconds(1.0 / modified_rate, TimerMode::Once);

        // Check if closest creature is in range
        if let Some((_target_entity, dist, target_pos)) = closest_creature {
            if dist <= range {
                let dir_to_target = target_pos - ship_pos;

                // Firing arc check
                if !is_in_firing_arc(ship_physics.rotation, &module.rotation, weapon_mount, dir_to_target) {
                    continue;
                }

                weapon.ammo -= 1;

                let accuracy = efficiency;
                let effective_spread = 30.0 * (1.0 - targeting_bonus.accuracy_bonus);
                let adjusted_target = apply_accuracy_spread(ship_pos, target_pos, accuracy, effective_spread);

                projectiles::spawn_projectile(
                    &mut commands,
                    &asset_server,
                    ship_pos,
                    adjusted_target,
                    damage,
                    PROJECTILE_SPEED,
                    range,
                    ProjectileOwner::Player,
                    ammo_storage.ammo_type,
                    None,
                );
            }
        }
    }
}

/// Player manual weapon fire (Space key) — power gated, arc-checked, supports mines.
pub(super) fn manual_weapon_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut weapon_query: Query<(
        &mut Weapon, &mut WeaponCooldown, &Module,
        Option<&CalculatedStats>, &WeaponMount, &AmmoStorage,
    )>,
    ship_query: Query<(&Transform, &ShipPhysics), With<Ship>>,
    creature_query: Query<(&Transform, Entity), (With<Creature>, Without<Ship>)>,
    power_graph: Res<PowerGraph>,
    targeting_bonus: Res<TargetingBonus>,
    mut notifications: MessageWriter<ShowNotification>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::Space) {
        return;
    }

    let Ok((ship_transform, ship_physics)) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    // Fire the first available weapon
    for (mut weapon, mut cooldown, module, calculated_stats, weapon_mount, ammo_storage) in weapon_query.iter_mut() {
        if !module.is_active || weapon.ammo == 0 {
            continue;
        }

        // Cooldown check — skip weapons still reloading
        if !cooldown.timer.is_finished() {
            continue;
        }

        // Power check
        if !power_graph.powered_tiles.contains(&module.grid_position) {
            continue;
        }

        let fire_rate = get_weapon_fire_rate(calculated_stats, &weapon);
        let range = get_weapon_range(calculated_stats, &weapon);
        let damage = get_weapon_damage(calculated_stats, &weapon);

        // Damage state penalty
        let health_ratio = module.health / module.max_health;
        let efficiency = ModuleDamageState::from_health_ratio(health_ratio).efficiency();
        if efficiency <= 0.0 { continue; }

        // Mine deployment — drop behind ship
        if ammo_storage.ammo_type == AmmoType::Mine {
            weapon.ammo -= 1;
            let modified_rate = fire_rate * efficiency;
            cooldown.timer = Timer::from_seconds(0.5 / modified_rate.max(0.01), TimerMode::Once);

            let backward = Vec2::new(
                (ship_physics.rotation + PI).cos(),
                (ship_physics.rotation + PI).sin(),
            );
            let mine_pos = ship_pos + backward * 80.0;
            mines::spawn_mine(&mut commands, &asset_server, mine_pos, damage);
            notifications.write(ShowNotification {
                message: "Mine deployed!".into(),
                notification_type: NotificationType::Info,
                duration: 1.0,
            });
            break;
        }

        weapon.ammo -= 1;
        let modified_rate = fire_rate * efficiency;
        cooldown.timer = Timer::from_seconds(0.5 / modified_rate.max(0.01), TimerMode::Once);

        // Find closest creature in range that is within firing arc
        let mut closest: Option<(Vec2, f32)> = None;
        for (c_transform, _) in creature_query.iter() {
            let c_pos = c_transform.translation.truncate();
            let dist = c_pos.distance(ship_pos);
            if dist <= range {
                let dir_to = c_pos - ship_pos;
                if is_in_firing_arc(ship_physics.rotation, &module.rotation, weapon_mount, dir_to) {
                    if dist < closest.map_or(f32::INFINITY, |(_, d)| d) {
                        closest = Some((c_pos, dist));
                    }
                }
            }
        }

        if let Some((target_pos, _)) = closest {
            let effective_spread = 15.0 * (1.0 - targeting_bonus.accuracy_bonus);
            let adjusted_target = apply_accuracy_spread(ship_pos, target_pos, efficiency, effective_spread);

            projectiles::spawn_projectile(
                &mut commands,
                &asset_server,
                ship_pos,
                adjusted_target,
                damage,
                PROJECTILE_SPEED * 1.2,
                range,
                ProjectileOwner::Player,
                ammo_storage.ammo_type,
                None,
            );
            notifications.write(ShowNotification {
                message: ammo_storage.ammo_type.display_name().into(),
                notification_type: NotificationType::Info,
                duration: 1.0,
            });
        } else {
            // Fire in the ship's facing direction
            let forward_dir = Vec2::new(ship_physics.rotation.cos(), ship_physics.rotation.sin());
            let forward = ship_pos + forward_dir * range;
            projectiles::spawn_projectile(
                &mut commands,
                &asset_server,
                ship_pos,
                forward,
                damage,
                PROJECTILE_SPEED,
                range,
                ProjectileOwner::Player,
                ammo_storage.ammo_type,
                None,
            );
            notifications.write(ShowNotification {
                message: "Fired! No targets in range".into(),
                notification_type: NotificationType::Info,
                duration: 1.5,
            });
        }

        // Only fire one weapon per press
        break;
    }
}
