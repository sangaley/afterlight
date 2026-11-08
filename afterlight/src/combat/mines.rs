use bevy::prelude::*;

use super::*;

/// Spawn a mine entity at the given position
pub(super) fn spawn_mine(commands: &mut Commands, asset_server: &AssetServer, position: Vec2, damage: f32) {
    commands.spawn((
        (Sprite {
                image: asset_server.load(crate::sprite_map::effect_sprite_path("explosion")),
                color: Color::srgb(0.6, 0.6, 0.6),
                custom_size: Some(Vec2::splat(14.0)),
                ..default()
            }, Transform::from_xyz(position.x, position.y, 0.4)),
        Mine {
            damage,
            detection_radius: 80.0,
            arm_timer: Timer::from_seconds(2.0, TimerMode::Once),
            lifetime: Timer::from_seconds(60.0, TimerMode::Once),
        },
    ));
}

/// Ticks mine timers, despawns expired mines, triggers detonation on creature proximity.
pub(super) fn mine_system(
    time: Res<Time>,
    mut commands: Commands,
    mut mine_query: Query<(Entity, &mut Mine, &Transform)>,
    creature_query: Query<(Entity, &Transform), With<Creature>>,
    asset_server: Res<AssetServer>,
) {
    for (mine_entity, mut mine, mine_transform) in mine_query.iter_mut() {
        mine.arm_timer.tick(time.delta());
        mine.lifetime.tick(time.delta());

        // Despawn expired mines
        if mine.lifetime.is_finished() {
            commands.entity(mine_entity).try_despawn();
            continue;
        }

        // Only check proximity once armed
        if !mine.arm_timer.is_finished() {
            continue;
        }

        let mine_pos = mine_transform.translation.truncate();

        for (_c_entity, c_transform) in creature_query.iter() {
            let c_pos = c_transform.translation.truncate();
            if mine_pos.distance(c_pos) < mine.detection_radius {
                // Detonate: despawn mine, spawn explosion entity
                let damage = mine.damage;
                commands.entity(mine_entity).try_despawn();
                commands.spawn((
                    (Sprite {
                            image: asset_server.load(crate::sprite_map::effect_sprite_path("explosion")),
                            color: Color::srgb(1.0, 0.6, 0.2),
                            custom_size: Some(Vec2::splat(40.0)),
                            ..default()
                        }, Transform::from_xyz(mine_pos.x, mine_pos.y, 0.6)),
                    MineExplosion {
                        damage,
                        blast_radius: 120.0,
                        applied: false,
                        timer: Timer::from_seconds(0.5, TimerMode::Once),
                    },
                ));
                break;
            }
        }
    }
}

/// Applies mine explosion damage to all creatures in blast radius, then despawns.
pub(super) fn mine_explosion_system(
    time: Res<Time>,
    mut commands: Commands,
    mut explosion_query: Query<(Entity, &mut MineExplosion, &Transform)>,
    mut creature_query: Query<(Entity, &Transform, &mut Creature)>,
) {
    for (exp_entity, mut explosion, exp_transform) in explosion_query.iter_mut() {
        explosion.timer.tick(time.delta());
        let exp_pos = exp_transform.translation.truncate();

        // Apply damage on first frame
        if !explosion.applied {
            explosion.applied = true;

            for (_c_entity, c_transform, mut creature) in creature_query.iter_mut() {
                let c_pos = c_transform.translation.truncate();
                if exp_pos.distance(c_pos) < explosion.blast_radius {
                    creature.health -= explosion.damage;

                    spawn_floating_damage(&mut commands, c_pos, explosion.damage, Color::srgb(1.0, 0.5, 0.2));
                    spawn_hit_effect(&mut commands, c_pos, Color::srgb(1.0, 0.6, 0.2), 24.0);
                }
            }
        }

        // Despawn after timer
        if explosion.timer.is_finished() {
            commands.entity(exp_entity).try_despawn();
        }
    }
}
