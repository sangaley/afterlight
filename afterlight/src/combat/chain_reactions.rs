use bevy::prelude::*;
use crate::components::*;
use crate::events::*;
use crate::building::GridOccupancy;

// ============================================================================
// CHAIN REACTION SYSTEM
// Destroyed modules trigger type-specific chain reactions.
// Ammo cook-off, reactor meltdown, fuel fire, capacitor discharge.
// ============================================================================

/// Detect newly destroyed modules and trigger chain reactions.
/// Player ship only: grid positions are ship-local, and the adjacency lookups
/// below resolve against the player's GridOccupancy — an AI ship's ammo bay
/// cooking off must not start fires on the player's modules that happen to
/// share the same local coordinates.
pub fn trigger_chain_reactions(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut commands: Commands,
    destroyed_modules: Query<(Entity, &Module, &GlobalTransform, &ChildOf), (Added<DestroyedModule>, With<DestroyedModule>)>,
    ship_query: Query<Entity, With<Ship>>,
    occupancy: Res<GridOccupancy>,
    mut notifications: MessageWriter<ShowNotification>,
    mut fire_events: MessageWriter<FireStarted>,
) {
    let player_ship = ship_query.single().ok();

    for (_entity, module, global_transform, parent) in destroyed_modules.iter() {
        if Some(parent.parent()) != player_ship {
            continue;
        }
        let pos = global_transform.translation().truncate();
        let grid_pos = module.grid_position;

        match module.module_type {
            // === AMMO COOK-OFF ===
            // Ammo storage destroyed → all ammo detonates → area damage
            ModuleType::AmmoFeedUnit | ModuleType::WarheadBay | ModuleType::AmmoBay => {
                notifications.write(ShowNotification {
                    message: "AMMO COOK-OFF! Ammunition detonating!".into(),
                    notification_type: NotificationType::Danger,
                    duration: 4.0,
                });

                // Damage all adjacent modules
                let _blast_damage = match module.module_type {
                    ModuleType::WarheadBay => 60.0, // Warheads hit harder
                    _ => 35.0,
                };

                for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y,
                    IVec2::new(1, 1), IVec2::new(1, -1), IVec2::new(-1, 1), IVec2::new(-1, -1)]
                {
                    let adj_pos = grid_pos + offset;
                    if let Some(&adj_entity) = occupancy.cells.get(&adj_pos) {
                        // We can't directly query mutable here due to borrow rules
                        // Fire event will handle the damage
                        fire_events.write(FireStarted {
                            module: adj_entity,
                            grid_position: adj_pos,
                            intensity: 0.8,
                        });
                    }
                }

                // Big explosion visual
                super::spawn_explosion(&mut commands, &fx, pos, 44.0, Color::srgb(1.0, 0.5, 0.1));
            }

            // === FUEL FIRE ===
            // Fuel tank destroyed → fire starts → spreads
            ModuleType::FuelTank | ModuleType::FuelProcessor => {
                notifications.write(ShowNotification {
                    message: "Fuel breach! Fire spreading!".into(),
                    notification_type: NotificationType::Danger,
                    duration: 3.0,
                });

                // Start fires on all adjacent modules
                for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                    let adj_pos = grid_pos + offset;
                    if let Some(&adj_entity) = occupancy.cells.get(&adj_pos) {
                        fire_events.write(FireStarted {
                            module: adj_entity,
                            grid_position: adj_pos,
                            intensity: 0.6,
                        });
                    }
                }

                super::spawn_explosion(&mut commands, &fx, pos, 27.0, Color::srgb(0.9, 0.4, 0.05));
            }

            // === CAPACITOR DISCHARGE ===
            // Battery/capacitor destroyed → electrical surge disables adjacent electronics
            ModuleType::BatteryBank | ModuleType::Capacitor | ModuleType::OverchargeCapacitor => {
                notifications.write(ShowNotification {
                    message: "Capacitor discharge! Electrical surge!".into(),
                    notification_type: NotificationType::Warning,
                    duration: 3.0,
                });

                // The comment always said "blue-white flash"; before the
                // colour parameter was wired up this could not have been one.
                super::spawn_explosion(&mut commands, &fx, pos, 22.0, Color::srgb(0.5, 0.7, 1.0));
            }

            // === REACTOR (handled by existing explosive system + emergency shutdown) ===
            // Reactors already have Explosive component and PendingDetonation
            // Emergency Shutdown module prevents meltdown

            _ => {}
        }
    }
}
