use bevy::prelude::*;

use super::*;

/// Some creatures can fire projectiles back at the ship
pub(super) fn creature_ranged_attack(
    time: Res<Time>,
    mut creature_query: Query<(&Transform, &mut Creature, &CreatureAI), Without<Ship>>,
    ship_query: Query<&Transform, With<Ship>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    for (c_transform, mut creature, ai) in creature_query.iter_mut() {
        // Only fire when actively attacking and targeting the ship
        if !matches!(ai.state, CreatureAIState::Attacking) {
            continue;
        }
        if !matches!(ai.target, Some(EcoTarget::Ship(_))) {
            continue;
        }

        // Only some creature types shoot back
        let (shoot_range, shoot_damage, shoot_cooldown) = match creature.creature_type {
            CreatureType::Stalker => (300.0, 12.0, 4.0),
            _ => continue,
        };

        let c_pos = c_transform.translation.truncate();
        let dist = c_pos.distance(ship_pos);

        if dist > shoot_range {
            continue;
        }

        // Use attack_cooldown for ranged timing
        creature.attack_cooldown -= time.delta_secs();
        if creature.attack_cooldown > 0.0 {
            continue;
        }
        creature.attack_cooldown = shoot_cooldown;

        // Fire projectile at ship
        projectiles::spawn_projectile(
            &mut commands,
            &asset_server,
            c_pos,
            ship_pos,
            shoot_damage,
            PROJECTILE_SPEED * 0.6,
            shoot_range,
            ProjectileOwner::Creature,
            AmmoType::Charge,
            None,
        );
    }
}

/// Animates floating damage numbers: move upward, fade out, despawn
pub(super) fn animate_floating_damage(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FloatingDamage, &mut Transform, &mut TextColor)>,
) {
    for (entity, mut dmg, mut transform, mut text_color) in query.iter_mut() {
        dmg.timer.tick(time.delta());
        transform.translation.y += dmg.velocity * time.delta_secs();

        // Fade out alpha
        let alpha = 1.0 - dmg.timer.fraction();
        text_color.0.set_alpha(alpha);

        if dmg.timer.is_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}

