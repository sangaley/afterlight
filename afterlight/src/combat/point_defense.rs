use bevy::prelude::*;
use super::targeting::fire_groups::InterceptMode;
use super::new_projectiles::{MissileProjectile, Projectile};
use super::*;

// ============================================================================
// POINT DEFENSE INTERCEPT MODE
// Any weapon flagged with InterceptMode auto-shoots incoming missiles.
// Player assigns intercept mode with I key + click on weapon.
// ============================================================================

/// Toggle intercept mode on a weapon during build mode
pub fn toggle_intercept_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    occupancy: Res<crate::building::GridOccupancy>,
    mut commands: Commands,
    weapon_query: Query<(Entity, &Module, Option<&InterceptMode>), With<Weapon>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !keyboard.pressed(KeyCode::KeyI) || !mouse.just_pressed(MouseButton::Left) { return; }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, cam_transform)) = camera_query.single() else { return };
    let Some(cursor) = window.cursor_position()
        .and_then(|p| camera.viewport_to_world_2d(cam_transform, p).ok())
    else { return };

    let grid_pos = IVec2::new(
        (cursor.x / 66.0).round() as i32,
        ((cursor.y + 33.0) / 66.0).round() as i32,
    );

    if let Some(&entity) = occupancy.cells.get(&grid_pos) {
        if let Ok((_, module, intercept)) = weapon_query.get(entity) {
            if intercept.is_some() {
                commands.entity(entity).remove::<InterceptMode>();
                notifications.write(ShowNotification {
                    message: format!("{}: Intercept mode OFF", module.module_type.name()),
                    notification_type: NotificationType::Info,
                    duration: 2.0,
                });
            } else {
                commands.entity(entity).insert(InterceptMode);
                notifications.write(ShowNotification {
                    message: format!("{}: Intercept mode ON - will target incoming missiles", module.module_type.name()),
                    notification_type: NotificationType::Warning,
                    duration: 2.0,
                });
            }
        }
    }
}

/// A round fired BY point defence, carrying the ship that fired it.
///
/// Without this every kinetic round in the game counted as an interceptor:
/// pd_missile_collision took a bare `Query<&Projectile>` and killed any
/// missile within 20 units of any shot. The ship is here so a battery cannot
/// swat its own outbound salvo, which is the only salvo there currently is —
/// AI ships do not launch missiles yet, so every missile in the world is the
/// player's.
#[derive(Component)]
pub struct PointDefenseRound {
    pub ship: Entity,
}

/// Intercept system: weapons in intercept mode auto-fire at incoming missiles
pub fn intercept_missiles(
    time: Res<Time>,
    ship_query: Query<(Entity, &Transform), With<Ship>>,
    mut intercept_weapons: Query<(
        &Module, &mut Weapon, &mut WeaponCooldown, &GlobalTransform,
    ), (With<InterceptMode>, Without<DestroyedModule>)>,
    missile_query: Query<(Entity, &Transform, &Velocity, &MissileProjectile)>,
    mut commands: Commands,
    sides: super::missile_defence::Sides,
) {
    let Ok((player_ship, ship_transform)) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    // Find incoming missiles (heading toward the ship)
    let mut threats: Vec<(Entity, Vec2, f32)> = Vec::new();
    for (entity, transform, velocity, missile) in missile_query.iter() {
        // Our own warhead is not a threat, however it happens to be flying.
        // The heading filter below catches an outbound salvo, but not one
        // that has looped back around on a target behind us.
        if !sides.threat(player_ship, missile.owner_ship) { continue; }
        let missile_pos = transform.translation.truncate();
        let to_ship = ship_pos - missile_pos;
        let dist = to_ship.length();

        // Only track missiles heading roughly toward us within 800 units
        if dist > 800.0 { continue; }
        let heading_toward = to_ship.normalize_or_zero().dot(velocity.0.normalize_or_zero());
        if heading_toward < 0.3 { continue; } // Must be heading somewhat toward us

        threats.push((entity, missile_pos, dist));
    }

    if threats.is_empty() { return; }

    // Sort by distance (closest first)
    threats.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    // Each intercept weapon targets the closest threat
    for (module, mut weapon, mut cooldown, global_transform) in intercept_weapons.iter_mut() {
        if !module.is_active { continue; }
        // A built gun on point defence fires its own shells from its own
        // turret (fire_weapons_system), not these.
        if module.module_type == ModuleType::FiringPiece { continue; }
        cooldown.timer.tick(time.delta());
        if !cooldown.timer.is_finished() { continue; }
        if weapon.ammo <= 0 { continue; }

        let weapon_pos = global_transform.translation().truncate();

        // Find closest threat in range
        let Some((_threat_entity, threat_pos, _)) = threats.iter()
            .find(|(_, pos, _)| weapon_pos.distance(*pos) < weapon.range)
        else { continue; };

        // Fire at the missile
        let direction = (*threat_pos - weapon_pos).normalize_or_zero();
        let proj_speed = 600.0;

        cooldown.timer.reset();
        weapon.ammo = weapon.ammo.saturating_sub(1);

        let angle = direction.y.atan2(direction.x);

        commands.spawn((
            (Sprite {
                    color: Color::srgb(0.3, 1.0, 0.5), 
                    custom_size: Some(Vec2::new(13.0, 5.0)),
                    ..default()
                }, Transform {
                    translation: Vec3::new(weapon_pos.x, weapon_pos.y, 0.5),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                }),
            Projectile {
                damage: weapon.damage * 0.5, // PD rounds do less damage but only need to hit missiles
                speed: proj_speed,
                lifetime: 1.5,
                max_lifetime: 1.5,
                owner: Entity::PLACEHOLDER,
                damage_type: super::new_projectiles::ProjectileDamageType::Kinetic,
                penetration: 5.0,
                has_penetrated: false,
                ammo: None, // PD rounds have no special on-hit behavior
                effect: None,
                caliber: 0.3,
                last_hit: None,
                prev_pos: weapon_pos,
                bounces: 0,
                flak: None,
            },
            Velocity(direction * proj_speed),
            PointDefenseRound { ship: player_ship },
        ));

        // Muzzle flash
        spawn_hit_effect(&mut commands, weapon_pos + direction * 20.0, Color::srgb(0.3, 0.9, 0.4), 6.0);
    }
}

/// What one point defence hit takes off a designed missile's toughness.
pub const PD_HIT: f32 = 25.0;

/// Point defence rounds against missiles. A flak round (a shell with a
/// proximity fuse) bursts when an enemy missile comes inside its fuse, and
/// every enemy missile within the burst takes its flak damage; any other
/// round has to hit one. A designed missile soaks hits up to its toughness
/// (a bigger one is easier to hit); a fixed-list missile goes down to one.
#[allow(clippy::type_complexity)]
pub fn pd_missile_collision(
    mut commands: Commands,
    proj_query: Query<(Entity, &PointDefenseRound, &Projectile, &Transform)>,
    mut missile_query: Query<(&Transform, &MissileProjectile, Option<&mut super::missiles::BuiltMissile>)>,
    missile_grid: Res<crate::spatial::MissileGrid>,
    sides: super::missile_defence::Sides,
) {
    for (proj_entity, pd, proj, proj_transform) in proj_query.iter() {
        let proj_pos = proj_transform.translation.truncate();
        let enemy = |m: &MissileProjectile| sides.threat(pd.ship, m.owner_ship);

        if let Some(flak) = proj.flak {
            let fused = missile_grid.0.nearby(proj_pos, flak.fuse).any(|(e, _)| {
                missile_query.get(e).is_ok_and(|(t, m, _)| enemy(m) && t.translation.truncate().distance(proj_pos) < flak.fuse)
            });
            if !fused { continue; }
            commands.entity(proj_entity).try_despawn();
            spawn_hit_effect(&mut commands, proj_pos, Color::srgb(1.0, 0.8, 0.45), flak.burst * 0.35);
            let caught: Vec<Entity> = missile_grid.0.nearby(proj_pos, flak.burst).map(|(e, _)| e).collect();
            for e in caught {
                let Ok((t, m, built)) = missile_query.get_mut(e) else { continue };
                if !enemy(m) || t.translation.truncate().distance(proj_pos) > flak.burst { continue; }
                let at = t.translation.truncate();
                if hurt_missile(built, flak.damage) {
                    commands.entity(e).try_despawn();
                    spawn_hit_effect(&mut commands, at, Color::srgb(1.0, 0.6, 0.2), 15.0);
                    if super::gun_assembly::trace() {
                        info!("FLAK downed a missile");
                    }
                }
            }
            continue;
        }

        for (missile_entity, _) in missile_grid.0.nearby(proj_pos, 32.0) {
            let Ok((missile_transform, missile, built)) = missile_query.get_mut(missile_entity) else { continue };
            // A battery does not shoot down its own salvo on the way out.
            if !enemy(missile) { continue; }
            let missile_pos = missile_transform.translation.truncate();
            let radius = built.as_ref().map_or(20.0, |b| 14.0 + 10.0 * b.stats.k.sqrt());
            if proj_pos.distance(missile_pos) >= radius { continue; }
            commands.entity(proj_entity).try_despawn();
            // A built gun's round does its own damage; the old PD rounds 25.
            let damage = if proj.effect.is_some() { proj.damage } else { PD_HIT };
            if hurt_missile(built, damage) {
                commands.entity(missile_entity).try_despawn();
                spawn_hit_effect(&mut commands, missile_pos, Color::srgb(1.0, 0.6, 0.2), 15.0);
            } else {
                spawn_hit_effect(&mut commands, missile_pos, Color::srgb(0.9, 0.9, 0.7), 8.0);
            }
            break;
        }
    }
}

/// Takes `damage` off a missile; true when it's down. A fixed-list missile
/// has no toughness and goes down to anything.
fn hurt_missile(built: Option<Mut<super::missiles::BuiltMissile>>, damage: f32) -> bool {
    match built {
        Some(mut b) => {
            b.hp -= damage;
            b.hp <= 0.0
        }
        None => true,
    }
}
