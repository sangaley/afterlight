use bevy::prelude::*;

use crate::components::{Corpse, Creature, CreatureType};
use crate::resources::EcosystemConfig;
use crate::sprite_map;

use super::food_chain;

/// Base display size for corpse sprites (matches creature spawn sizes)
fn corpse_display_size(creature_type: CreatureType) -> Vec2 {
    match creature_type {
        CreatureType::VoidDrifter =>     Vec2::new(45.0, 22.0),
        CreatureType::Stalker =>         Vec2::new(150.0, 54.0),
        CreatureType::Leviathan =>       Vec2::new(660.0, 180.0),
        CreatureType::ParasiteSwarm =>   Vec2::new(30.0, 20.0),
    }
}

/// When a creature's health hits 0, spawn a Corpse entity at its position
/// using the creature's actual sprite with a dark, desaturated tint.
pub fn spawn_corpse_on_death(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    creatures: Query<(Entity, &Transform, &Creature), Changed<Creature>>,
    existing_corpses: Query<(Entity, &Corpse)>,
    eco_config: Res<EcosystemConfig>,
) {
    for (_entity, transform, creature) in creatures.iter() {
        if creature.health > 0.0 {
            continue;
        }

        let eco_stats = food_chain::creature_ecosystem_stats(creature.creature_type);

        // Enforce corpse cap — remove oldest if at limit
        let corpse_count = existing_corpses.iter().count();
        if corpse_count >= eco_config.max_corpses {
            if let Some((oldest_entity, _)) = existing_corpses
                .iter()
                .min_by(|a, b| a.1.decay_timer.partial_cmp(&b.1.decay_timer).unwrap_or(std::cmp::Ordering::Equal))
            {
                commands.entity(oldest_entity).despawn();
            }
        }

        let pos = transform.translation;
        let size = corpse_display_size(creature.creature_type);

        // Use the creature's actual sprite, tinted dark brownish-red
        commands.spawn((
            (Sprite {
                    image: asset_server.load(sprite_map::creature_sprite_path(creature.creature_type)),
                    color: Color::srgba(0.3, 0.2, 0.15, 0.7),
                    custom_size: Some(size),
                    ..default()
                }, Transform {
                    translation: Vec3::new(pos.x, pos.y, -0.1), 
                    
                    rotation: Quat::from_rotation_z(0.3),
                    ..default()
                }),
            Corpse {
                creature_type: creature.creature_type,
                food_remaining: eco_stats.food_value,
                decay_timer: eco_config.corpse_decay_time,
            },
        ));
    }
}

/// Corpses decay over time and despawn when timer runs out or food is gone
pub fn decay_corpses(
    time: Res<Time>,
    mut commands: Commands,
    mut corpses: Query<(Entity, &mut Corpse, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (entity, mut corpse, mut sprite) in corpses.iter_mut() {
        corpse.decay_timer -= dt;

        // Fade out as corpse decays
        let alpha = (corpse.decay_timer / 120.0).clamp(0.0, 0.7);
        sprite.color.set_alpha(alpha);

        if corpse.decay_timer <= 0.0 || corpse.food_remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
