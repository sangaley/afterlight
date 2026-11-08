use bevy::prelude::*;
use rand::Rng;
use crate::components::*;
use crate::events::*;
use crate::building::rooms::RoomMap;

/// Reads FireStarted events and ignites modules that aren't already on fire.
pub fn apply_fire_ignition(
    mut commands: Commands,
    mut fire_events: MessageReader<FireStarted>,
    module_query: Query<(Entity, &Module, &ChildOf), (Without<DestroyedModule>, Without<OnFire>)>,
    player: Query<Entity, With<Ship>>,
    room_map: Res<RoomMap>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let player = player.single().ok();
    for event in fire_events.read() {
        // Check if module exists, is not destroyed, and not already on fire
        let Ok((entity, module, parent)) = module_query.get(event.module) else {
            continue;
        };
        // The room map and the alarm are the player's. An enemy module's cell
        // numbers mean nothing in our room map, and "Fire in Reactor!" for a
        // reactor burning on someone else's ship is a false alarm.
        let ours = Some(parent.parent()) == player;

        // Can't ignite in heavily depressurized rooms (no oxygen to sustain fire)
        if ours {
            if let Some(&room_id) = room_map.tile_to_room.get(&event.grid_position) {
                if let Some(room) = room_map.rooms.get(room_id) {
                    if room.air_level < 0.3 {
                        continue;
                    }
                }
            }
        }

        let intensity = event.intensity.clamp(0.05, 1.0);
        commands.entity(entity).insert(OnFire {
            intensity,
            damage_per_second: 8.0 * intensity,
            spread_timer: Timer::from_seconds(3.0, TimerMode::Repeating),
            duration: Timer::from_seconds(15.0, TimerMode::Once),
        });

        if ours {
            notifications.write(ShowNotification {
                message: format!("Fire in {}!", module.module_type.name()),
                notification_type: NotificationType::Danger,
                duration: 3.0,
            });
        }
    }
}

/// Updates all burning modules: vacuum suppression, burnout, DoT, visual tint, and spread.
pub fn update_fire(
    mut commands: Commands,
    time: Res<Time>,
    mut fire_query: Query<(Entity, &mut OnFire, &mut Module, &mut Sprite, &ChildOf), Without<DestroyedModule>>,
    room_map: Res<RoomMap>,
    player: Query<Entity, With<Ship>>,
    ship_grids: Query<&crate::building::ShipGrid>,
    alive_modules: Query<Entity, (With<Module>, Without<DestroyedModule>, Without<OnFire>)>,
    sealed_query: Query<(&HullSegment, &Transform, &ChildOf), With<BulkheadSealed>>,
    firebreak_query: Query<(&Module, &ChildOf), (With<FirebreakMarker>, Without<DestroyedModule>, Without<OnFire>)>,
    mut fire_events: MessageWriter<FireStarted>,
    mut extinguish_events: MessageWriter<FireExtinguished>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    let player = player.single().ok();

    // Build set of sealed bulkhead positions to block fire spread. Ours only:
    // positions are ship-local, so these only mean anything on our grid.
    let mut sealed_positions: std::collections::HashSet<IVec2> = sealed_query
        .iter()
        .filter(|(_, _, parent)| Some(parent.parent()) == player)
        .map(|(_, transform, _)| crate::building::rooms::transform_to_grid(transform))
        .collect();

    // FirebreakWall always blocks fire spread (no seal needed)
    for (fb_module, parent) in firebreak_query.iter() {
        if Some(parent.parent()) == player {
            sealed_positions.insert(fb_module.grid_position);
        }
    }

    for (entity, mut fire, mut module, mut sprite, parent) in fire_query.iter_mut() {
        let ship = parent.parent();
        let ours = Some(ship) == player;

        // Vacuum/decompression suppression — fire can't burn without air.
        // Only our rooms are mapped.
        let room = if ours { room_map.tile_to_room.get(&module.grid_position) } else { None };
        if let Some(&room_id) = room {
            if let Some(room) = room_map.rooms.get(room_id) {
                if room.air_level < 0.2 {
                    // Fully extinguish — not enough oxygen
                    commands.entity(entity).remove::<OnFire>();
                    sprite.color = Color::srgb(0.2, 0.2, 0.2); // Burnt/destroyed tint
                    extinguish_events.write(FireExtinguished {
                        module: entity,
                        cause: FireExtinguishCause::Decompression,
                    });
                    continue;
                } else if room.air_level < 0.5 {
                    // Reduce intensity based on low air
                    fire.intensity = (fire.intensity - (1.0 - room.air_level) * 0.5 * dt).max(0.0);
                    fire.damage_per_second = 8.0 * fire.intensity;
                }
            }
        }

        // Tick timers
        fire.duration.tick(time.delta());
        fire.spread_timer.tick(time.delta());

        // Burnout check
        if fire.duration.is_finished() || fire.intensity < 0.05 {
            commands.entity(entity).remove::<OnFire>();
            extinguish_events.write(FireExtinguished {
                module: entity,
                cause: FireExtinguishCause::BurnedOut,
            });
            continue;
        }

        // DoT
        module.health -= fire.damage_per_second * dt;
        if module.health < 0.0 {
            module.health = 0.0;
        }

        // Scorch the module, and let the flame overlay do the rest.
        //
        // This used to drive the tint to full orange, because it was the only
        // signal a module was burning. Now that a real flame sits on top, a
        // fully orange block underneath reads as neon rather than as damage --
        // so this backs off to "hot and charred" and the flame carries it.
        let r = 0.35 + fire.intensity * 0.45;
        let g = 0.25 + fire.intensity * 0.18;
        let b = 0.22 * (1.0 - fire.intensity);
        sprite.color = Color::srgb(r, g, b);

        // Spread on timer tick, through the burning block's OWN ship's grid.
        // This read the player's GridOccupancy with the block's local cell,
        // so a fire on any enemy or wreck -- an overheated gun, a burning
        // reactor -- jumped to whatever of ours sat at the same cell numbers,
        // anywhere in the world. Angled armour plates are modules and burn.
        if fire.spread_timer.just_finished() {
            let spread_chance = 0.3 * fire.intensity;
            let Ok(grid) = ship_grids.get(ship) else { continue };
            for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let adj_pos = module.grid_position + offset;
                // Block fire spread across sealed bulkheads
                if ours && sealed_positions.contains(&adj_pos) {
                    continue;
                }
                if rng.gen::<f32>() < spread_chance {
                    if let Some(adj_entity) = grid.get(adj_pos) {
                        // Only spread to alive, non-burning modules
                        if alive_modules.get(adj_entity).is_ok() {
                            fire_events.write(FireStarted {
                                module: adj_entity,
                                grid_position: adj_pos,
                                intensity: fire.intensity * 0.7,
                            });
                        }
                    }
                }
            }
        }
    }
}


// ============================================================================
// BURNING OVERLAY
//
// A module on fire used to signal it by tinting its own sprite orange, which
// on a dark hull reads as "this block is a slightly different colour" rather
// than as anything burning. ART_BRIEF asks for a looping flame that can sit
// on top of any module tile; this is that, done without an animation system.
//
// The flicker is a swap between three flame textures on a short timer plus a
// scale and alpha jitter, rather than a texture atlas. That is deliberate:
// three handles and a timer need no TextureAtlasLayout, no frame-index
// bookkeeping, and no new component on the module itself, and at the size a
// burning module actually occupies on screen nobody can tell the difference.
// ============================================================================

/// A flame sprite parented to a burning module.
#[derive(Component)]
pub struct FireOverlay {
    /// Steps the texture swap.
    pub tick: Timer,
    /// Which flame variant is showing.
    pub frame: usize,
    /// Per-overlay phase so two adjacent fires don't flicker in lockstep,
    /// which reads as one animation playing twice rather than two fires.
    pub phase: f32,
}

/// Attach a flame to anything newly `OnFire`, and remove it once the fire is
/// out. Runs off `Added`/orphan checks rather than events so it cannot drift
/// out of sync with the component that actually decides whether it burns.
pub fn sync_fire_overlays(
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    newly_lit: Query<Entity, (With<OnFire>, Without<DestroyedModule>)>,
    overlays: Query<(Entity, &ChildOf), With<FireOverlay>>,
    has_overlay: Query<&Children>,
    still_burning: Query<(), (With<OnFire>, Without<DestroyedModule>)>,
) {
    // Drop overlays whose module stopped burning (or was destroyed).
    for (overlay, parent) in overlays.iter() {
        if still_burning.get(parent.parent()).is_err() {
            commands.entity(overlay).despawn();
        }
    }

    // Add one to anything burning that hasn't got one.
    for module in newly_lit.iter() {
        let already = has_overlay
            .get(module)
            .map(|kids| kids.iter().any(|c| overlays.get(c).is_ok()))
            .unwrap_or(false);
        if already {
            continue;
        }
        commands.entity(module).with_children(|p| {
            p.spawn((
                Sprite {
                    image: fx.flame_at(0),
                    color: Color::srgba(1.0, 0.62, 0.22, 0.0),
                    custom_size: Some(Vec2::splat(46.0)),
                    ..default()
                },
                // Above the module's own sprite and its turret barrel (local
                // 0.3), below projectiles (0.5). Local to the parent -- the
                // 0.25 this was set to sat UNDER the barrel it describes.
                Transform::from_xyz(0.0, 0.0, 0.32),
                FireOverlay {
                    tick: Timer::from_seconds(0.11, TimerMode::Repeating),
                    frame: 0,
                    phase: rand::random::<f32>() * std::f32::consts::TAU,
                },
            ));
        });
    }
}

/// Flicker the flames: swap texture on the timer, and breathe scale/alpha
/// between swaps so the motion is continuous rather than a three-frame loop.
pub fn animate_fire_overlays(
    time: Res<Time>,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    parents: Query<&OnFire>,
    mut overlays: Query<(&mut FireOverlay, &mut Sprite, &mut Transform, &ChildOf)>,
) {
    let t = time.elapsed_secs();
    for (mut ov, mut sprite, mut transform, parent) in overlays.iter_mut() {
        // Intensity drives everything, so a dying fire visibly dies.
        let intensity = parents.get(parent.parent()).map(|f| f.intensity).unwrap_or(0.0);

        ov.tick.tick(time.delta());
        if ov.tick.just_finished() {
            ov.frame = ov.frame.wrapping_add(1);
            sprite.image = fx.flame_at(ov.frame);
        }

        // Two out-of-phase sines: a single one reads as a pulse.
        let flick = 0.78
            + 0.16 * (t * 11.0 + ov.phase).sin()
            + 0.06 * (t * 23.0 + ov.phase * 1.7).sin();

        // Alpha stays low on purpose. This sits on top of a module the player
        // still needs to read -- it should say "this is burning", not hide
        // what is burning.
        sprite.color.set_alpha((0.34 * intensity * flick).clamp(0.0, 0.5));
        transform.scale = Vec3::splat((0.72 + 0.5 * intensity) * flick);
    }
}
