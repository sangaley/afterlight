use bevy::prelude::*;
use rand::Rng;
use crate::components::{Module, DestroyedModule};
use crate::events::{ShowNotification, NotificationType};
use super::components::*;

// ============================================================================
// CHAIN DAMAGE AND DISCONNECTION SYSTEM
// When a block is destroyed:
// 1. Everything past it in the chain (toward tip) is disconnected
// 2. Cascade explosion chance based on barrel stress
// 3. Core destruction = full machine detonation
// ============================================================================

/// When a MachineBlock is destroyed, disconnect everything past it in the chain.
/// Also roll for cascade explosion.
/// Notifications only fire for the player's own ship (via ChildOf) — AI ships
/// losing a weapon core is not something the player needs to be alarmed about.
pub fn process_block_destruction(
    mut commands: Commands,
    destroyed_blocks: Query<(Entity, &MachineBlock, &Module, Option<&BarrelStress>, Option<&CascadeRisk>, &ChildOf), (Added<DestroyedModule>, With<DestroyedModule>)>,
    mut chain_blocks: Query<(Entity, &mut MachineBlock, &mut Module), Without<DestroyedModule>>,
    ship_query: Query<Entity, With<crate::components::Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let mut rng = rand::thread_rng();
    let player_ship = ship_query.single().ok();

    for (_destroyed_entity, destroyed_block, _destroyed_module, stress, cascade, parent) in destroyed_blocks.iter() {
        let is_player_ship = Some(parent.parent()) == player_ship;

        // Skip if not part of a machine
        if destroyed_block.connected_core.is_none() {
            continue;
        }

        let Some(core_entity) = destroyed_block.connected_core else { continue; };

        // === CORE DESTRUCTION = CATASTROPHIC ===
        if destroyed_block.role == BlockRole::Core {
            if is_player_ship {
                notifications.write(ShowNotification {
                    message: "MACHINE CORE DESTROYED! Full system failure!".into(),
                    notification_type: NotificationType::Danger,
                    duration: 4.0,
                });
            }

            // Disconnect ALL blocks connected to this core
            for (entity, mut block, mut module) in chain_blocks.iter_mut() {
                if block.connected_core == Some(core_entity) {
                    block.connected_core = None;
                    block.chain_distance = 0;
                    block.next_in_chain = None;
                    block.prev_in_chain = None;
                    module.is_active = false;
                    // try_insert: unscoped to all ships (AI included). The
                    // destroyed core may belong to a ship whose reactor also
                    // died this frame and got recursively despawned already.
                    commands.entity(entity).try_insert(Disconnected);
                }
            }
            continue;
        }

        // === NON-CORE BLOCK DESTRUCTION ===

        // Find all blocks downstream (higher chain_distance, connected through this block)
        let _destroyed_distance = destroyed_block.chain_distance;
        let mut to_disconnect: Vec<Entity> = Vec::new();

        // Walk the chain forward from this block
        let mut current = destroyed_block.next_in_chain;
        while let Some(next_entity) = current {
            to_disconnect.push(next_entity);
            if let Ok((_, block, _)) = chain_blocks.get(next_entity) {
                current = block.next_in_chain;
            } else {
                break;
            }
        }

        // Disconnect all downstream blocks
        let disconnected_count = to_disconnect.len();
        for entity in &to_disconnect {
            if let Ok((_, mut block, mut module)) = chain_blocks.get_mut(*entity) {
                block.connected_core = None;
                block.chain_distance = 0;
                block.next_in_chain = None;
                block.prev_in_chain = None;
                module.is_active = false;
                commands.entity(*entity).try_insert(Disconnected);
            }
        }

        if disconnected_count > 0 && is_player_ship {
            notifications.write(ShowNotification {
                message: format!("Block destroyed! {} connected blocks lost!", disconnected_count + 1),
                notification_type: NotificationType::Danger,
                duration: 3.0,
            });
        }

        // === CASCADE EXPLOSION CHECK ===
        let cascade_chance = stress
            .map(|s| s.effective_cascade_chance)
            .or_else(|| cascade.map(|c| c.cascade_chance))
            .unwrap_or(0.15);

        let cascade_dmg = cascade.map(|c| c.cascade_damage).unwrap_or(30.0);

        if rng.gen::<f32>() < cascade_chance {
            // Cascade toward core (prev_in_chain)
            if let Some(prev_entity) = destroyed_block.prev_in_chain {
                if let Ok((_, _, mut prev_module)) = chain_blocks.get_mut(prev_entity) {
                    prev_module.health -= cascade_dmg;
                    if prev_module.health < 0.0 {
                        prev_module.health = 0.0;
                    }

                    if is_player_ship {
                        notifications.write(ShowNotification {
                            message: "CASCADE! Explosion spreading toward core!".into(),
                            notification_type: NotificationType::Danger,
                            duration: 3.0,
                        });
                    }
                }
            }
        }

        // Clear the prev block's next_in_chain reference
        if let Some(prev_entity) = destroyed_block.prev_in_chain {
            if let Ok((_, mut prev_block, _)) = chain_blocks.get_mut(prev_entity) {
                prev_block.next_in_chain = None;
            }
        }
    }
}

/// Crew repair priority for machine blocks:
/// Core (highest) → AmmoFeed → Barrel → Cooling (lowest)
pub fn machine_repair_priority(role: &BlockRole) -> u32 {
    match role {
        BlockRole::Core => 100,
        BlockRole::AmmoFeed => 80,
        BlockRole::FuelRod => 80,
        BlockRole::Barrel => 60,
        BlockRole::Nozzle => 60,
        BlockRole::Cooling => 40,
        BlockRole::ShieldEmitter => 70,
    }
}
