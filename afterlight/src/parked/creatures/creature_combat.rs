use bevy::prelude::*;

use crate::components::{
    AttackCooldown, Creature, CreatureAI, CreatureAIState, EcoTarget, CreatureType,
};
use crate::events::CreatureAteCreature;

/// Handles creature-vs-creature combat when a creature is Attacking with EcoTarget::Creature.
/// Collects attack intents first, then applies damage to avoid query conflicts.
pub fn creature_vs_creature_combat(
    mut creatures: Query<(
        Entity,
        &Transform,
        &mut Creature,
        &CreatureAI,
        &mut AttackCooldown,
    )>,
    mut ate_events: MessageWriter<CreatureAteCreature>,
) {
    // Phase 1: Collect attack intents (immutable borrow)
    let intents: Vec<(Entity, Entity, Vec2, f32, CreatureType)> = creatures
        .iter()
        .filter_map(|(entity, transform, creature, ai, cooldown)| {
            if ai.state != CreatureAIState::Attacking {
                return None;
            }
            if let Some(EcoTarget::Creature(target_entity)) = ai.target {
                if cooldown.timer.is_finished() {
                    return Some((
                        entity,
                        target_entity,
                        transform.translation.truncate(),
                        creature.damage,
                        creature.creature_type,
                    ));
                }
            }
            None
        })
        .collect();

    // Phase 2: For each intent, check distance using immutable access
    let mut damage_to_apply: Vec<(Entity, Entity, f32, CreatureType, Vec2, CreatureType)> = Vec::new();

    for (attacker_entity, target_entity, attacker_pos, damage, attacker_type) in &intents {
        if let Ok((_, target_transform, target_creature, _, _)) = creatures.get(*target_entity) {
            let target_pos = target_transform.translation.truncate();
            let dist = attacker_pos.distance(target_pos);
            if dist < 80.0 {
                damage_to_apply.push((
                    *attacker_entity,
                    *target_entity,
                    *damage,
                    *attacker_type,
                    target_pos,
                    target_creature.creature_type,
                ));
            }
        }
    }

    // Phase 3: Apply damage (mutable borrow, one entity at a time)
    for (attacker_entity, target_entity, damage, attacker_type, target_pos, target_type) in damage_to_apply {
        // Reset attacker cooldown
        if let Ok((_, _, _, _, mut cooldown)) = creatures.get_mut(attacker_entity) {
            cooldown.timer.reset();
        }

        // Apply damage to target
        if let Ok((_, _, mut target_creature, _, _)) = creatures.get_mut(target_entity) {
            target_creature.health -= damage;

            if target_creature.health <= 0.0 {
                ate_events.write(CreatureAteCreature {
                    predator: attacker_entity,
                    predator_type: attacker_type,
                    prey_type: target_type,
                    position: target_pos,
                });
            }
        }
    }
}
