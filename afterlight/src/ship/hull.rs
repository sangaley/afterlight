use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::building::rooms::RoomMap;

/// Updates hull integrity tracking.
/// Player ship only — HullSegment is shared with AI ships (same pattern as
/// everywhere else this got missed), so this was summing every AI ship's
/// hull in the world into the player's own HULL% stat.
pub fn update_hull_integrity(
    hull_query: Query<(&HullSegment, &ChildOf)>,
    ship_query: Query<Entity, With<Ship>>,
    mut hull_state: ResMut<HullState>,
) {
    let Ok(player_ship) = ship_query.single() else { return };
    let mut total_health = 0.0;
    let mut max_health = 0.0;
    let mut min_radiation_shielding = f32::MAX;

    for (hull, parent) in hull_query.iter() {
        if parent.parent() != player_ship { continue; }
        total_health += hull.health;
        max_health += hull.max_health;

        if hull.radiation_shielding < min_radiation_shielding {
            min_radiation_shielding = hull.radiation_shielding;
        }
    }

    if max_health > 0.0 {
        hull_state.hull_integrity = total_health / max_health;
    }
    hull_state.max_radiation_shielding = if min_radiation_shielding == f32::MAX {
        200.0
    } else {
        min_radiation_shielding
    };
}

/// Linear per-channel blend from `base` toward `target` by `t` (0 = base, 1 = target).
pub fn mix_color(base: Color, target: Color, t: f32) -> Color {
    let b = base.to_srgba();
    let d = target.to_srgba();
    let t = t.clamp(0.0, 1.0);
    // Carry alpha through. Returning srgb() dropped it and handed back an
    // OPAQUE colour, so anything with a transparent base got repainted solid
    // black on its first tint — which is what turned the angled plates' empty
    // half into a black square instead of showing the sky behind it.
    Color::srgba(
        b.red + (d.red - b.red) * t,
        b.green + (d.green - b.green) * t,
        b.blue + (d.blue - b.blue) * t,
        b.alpha + (d.alpha - b.alpha) * t,
    )
}

/// The color hull/module damage tints blend toward as health drops — matches
/// the destroyed-tint color exactly, so a block darkens continuously into
/// its own "destroyed" look rather than jumping there at the last hit.
pub const DAMAGE_TINT_TARGET: Color = Color::srgb(0.15, 0.15, 0.15);

/// Gradual damage tint for hull (player or AI): darkens continuously as
/// health drops from max toward 0, using the spawn-time BaseSpriteColor as a
/// stable reference — blending from the *live* sprite.color would compound
/// every frame (the exact class of bug that decayed weapon range to zero
/// earlier this session). Without this, a block that had taken real damage
/// looked identical to a full-health one right up until the killing hit.
pub fn tint_damaged_hull(
    mut hull_query: Query<(&HullSegment, &BaseSpriteColor, &mut Sprite), Without<HullDestroyed>>,
) {
    for (hull, base, mut sprite) in hull_query.iter_mut() {
        if hull.max_health <= 0.0 { continue; }
        let damage_frac = 1.0 - (hull.health / hull.max_health).clamp(0.0, 1.0);
        // Only write a colour that differs: every write marks the sprite
        // changed for the renderer, and this runs over every block in the
        // world every frame.
        let tint = mix_color(base.0, DAMAGE_TINT_TARGET, damage_frac);
        if sprite.color != tint {
            sprite.color = tint;
        }
    }
}

/// Tints any hull segment (player or AI) dark and marks it destroyed once its
/// health hits 0. This was the only piece of hull destruction with no visual
/// feedback at all — HullDestroyed was purely an internal marker (gating the
/// severance/cascade systems), never tied to a sprite change, so a block you
/// reduced to 0 HP looked identical to a full-health one. Modules already got
/// this treatment (process_module_destruction); hull tiles didn't.
pub fn tint_destroyed_hull(
    mut commands: Commands,
    mut hull_query: Query<(Entity, &mut HullSegment, &mut Sprite, &ChildOf), Without<HullDestroyed>>,
    ship: Query<Entity, With<Ship>>,
    debug_tuning: Option<Res<crate::debug::DebugTuning>>,
) {
    let player = ship.single().ok();
    for (entity, mut hull, mut sprite, parent) in hull_query.iter_mut() {
        if hull.health <= 0.0 && debug_tuning.as_ref().is_some_and(|t| t.god_mode) && Some(parent.parent()) == player {
            // God mode: your plates never go.
            hull.health = hull.max_health;
            continue;
        }
        if hull.health <= 0.0 {
            sprite.color = DAMAGE_TINT_TARGET;
            // try_insert: this runs for every ship including AI ships, and an
            // AI ship whose reactor also died this same frame gets recursively
            // despawned by ai_ship_death_system. Plain insert() panics if that
            // despawn command flushes before this one for the same hull tile.
            commands.entity(entity).try_insert(HullDestroyed);
        }
    }
}

/// Queues a freshly-destroyed hull segment for removal. Doesn't despawn
/// directly here — see `PendingRemoval`'s doc comment for why.
pub fn queue_hull_removal(
    mut commands: Commands,
    fresh: Query<Entity, Added<HullDestroyed>>,
) {
    for entity in fresh.iter() {
        commands.entity(entity).try_insert(PendingRemoval {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
        });
    }
}

/// When a hull segment is destroyed (0 HP), apply 15% max_health stress damage to adjacent hull.
/// This can cascade through already-weakened segments but won't one-shot healthy hull.
/// Player ship only. Cells are ship-LOCAL and the room map and breach events
/// are the player's: matched on grid_position alone, a plate lost on one ship
/// stressed every other ship's plate at the neighbouring local cell, wherever
/// in the world it was.
pub fn process_hull_cascade(
    mut commands: Commands,
    mut hull_destroy_events: MessageReader<HullSegmentDestroyed>,
    mut hull_query: Query<(Entity, &mut HullSegment, &ChildOf), Without<HullDestroyed>>,
    parents: Query<&ChildOf>,
    ship_query: Query<Entity, With<Ship>>,
    mut breach_events: MessageWriter<HullBreached>,
    room_map: Res<RoomMap>,
    mut room_depressurize_events: MessageWriter<RoomDepressurized>,
    mut notifications: MessageWriter<ShowNotification>,
    debug_tuning: Option<Res<crate::debug::DebugTuning>>,
) {
    let player_ship = ship_query.single().ok();
    let mut destroyed_positions: Vec<IVec2> = Vec::new();

    for event in hull_destroy_events.read() {
        let on_player = parents.get(event.segment).ok().map(|p| p.parent()) == player_ship;
        if on_player && player_ship.is_some() && debug_tuning.as_ref().is_some_and(|t| t.god_mode) {
            if let Ok((_, mut seg, _)) = hull_query.get_mut(event.segment) {
                seg.health = seg.max_health;
            }
            continue;
        }
        // Mark the destroyed hull segment
        commands.entity(event.segment).try_insert(HullDestroyed);
        if on_player && player_ship.is_some() {
            destroyed_positions.push(event.grid_position);
        }
    }

    // Apply stress damage to adjacent hull segments
    for destroyed_pos in &destroyed_positions {
        for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let adj_pos = *destroyed_pos + offset;

            for (hull_entity, mut hull, parent) in hull_query.iter_mut() {
                if Some(parent.parent()) != player_ship || hull.grid_position != adj_pos {
                    continue;
                }

                // 15% of max_health as stress damage
                let stress_damage = hull.max_health * 0.15;
                hull.health = (hull.health - stress_damage).max(0.0);

                let health_pct = if hull.max_health > 0.0 {
                    hull.health / hull.max_health
                } else {
                    0.0
                };

                // Breach if newly below 30%
                if health_pct < 0.3 && !hull.is_depressurized {
                    hull.is_depressurized = true;
                    breach_events.write(HullBreached {
                        segment: hull_entity,
                        severity: 1.0 - health_pct,
                    });
                    if let Some(&room_id) = room_map.tile_to_room.get(&adj_pos) {
                        room_depressurize_events.write(RoomDepressurized {
                            room_id,
                            severity: 1.0 - health_pct,
                        });
                    }
                    notifications.write(ShowNotification {
                        message: "Hull cascade! Adjacent segment weakened!".into(),
                        notification_type: NotificationType::Warning,
                        duration: 3.0,
                    });
                }
            }
        }
    }
}
