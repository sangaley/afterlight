use bevy::prelude::*;
use rand::Rng;
use crate::components::*;
use crate::events::*;
use crate::building::rooms::RoomMap;
use crate::building::GridOccupancy;
use super::hull::{mix_color, DAMAGE_TINT_TARGET};

/// Consumes ShipDamaged events and applies damage using directional kinetic penetration.
///
/// If direction is available: sweep the shot's line through the player's
/// ShipGrid from the reported impact point inward, damaging the blocks it
/// actually passes through, outermost first (armour absorbs by material
/// rating, the remainder continues — see combat::impact::resolve_impact).
/// If no direction (radiation, explosion): fall back to random hull segment.
/// PLAYER SHIP ONLY: hull/module queries are filtered to the player's own
/// children. Unscoped, the "outermost block first" damage walk collected the
/// ATTACKER's blocks too (they sit right on the attack ray) — enemy fire was
/// being absorbed by the enemy's own ship and never reached the player.
pub fn process_ship_damage(
    mut damage_events: MessageReader<ShipDamaged>,
    mut hull_query: Query<(Entity, &mut HullSegment, &GlobalTransform, &ChildOf)>,
    mut module_query: Query<(Entity, &mut Module, &GlobalTransform, &ChildOf), Without<DestroyedModule>>,
    ship_query: Query<(Entity, &GlobalTransform), With<Ship>>,
    room_map: Res<RoomMap>,
    time: Res<Time>,
    mut death_cause: ResMut<crate::resources::DeathCause>,
    mut breach_events: MessageWriter<HullBreached>,
    mut room_depressurize_events: MessageWriter<RoomDepressurized>,
    mut commands: Commands,
    debug_tuning: Res<crate::debug::DebugTuning>,
    grid_query: Query<&crate::building::ShipGrid>,
    block_query: Query<&crate::building::Block>,
) {
    let mut rng = rand::thread_rng();

    let Ok((player_ship, player_gt)) = ship_query.single() else { return };
    let ship_center = player_gt.translation().truncate();

    for event in damage_events.read() {
        // Skip radiation damage — it's handled directly in check_radiation_damage
        if matches!(event.source, DamageSource::Radiation) {
            continue;
        }

        // Debug god mode: the event is consumed (read()'s iterator already
        // advanced past it) but entirely ignored — no hull/module damage,
        // no death-cause bookkeeping, no breach. Ship takes nothing.
        if debug_tuning.god_mode {
            continue;
        }

        // Remember what hit us — the death screen uses this to attribute
        // hull/crew deaths to their actual source.
        let source_desc = match event.source {
            DamageSource::Creature(_) => "creature attack",
            DamageSource::Collision => "collision",
            DamageSource::Explosion => "explosion",
            DamageSource::Fire => "fire",
            DamageSource::Gunfire => "enemy fire",
            DamageSource::Radiation => unreachable!(),
        };
        death_cause.last_damage = Some((source_desc.to_string(), time.elapsed_secs_f64()));

        // Gunfire has already been resolved block by block where it landed.
        if event.applied {
            continue;
        }

        if hull_query.is_empty() {
            continue;
        }

        // Determine attack direction
        let direction = event.direction.or_else(|| {
            event.position.map(|pos| (pos - ship_center).normalize_or_zero())
        });

        if let Some(dir) = direction {
            // === SWEPT GRID WALK ===
            // `dir` points from the ship TOWARD the attacker (every writer
            // agrees on that); the round travels the other way. Start a cell
            // and a half outside the reported impact point and walk inward
            // through the player's ShipGrid, so the blocks damaged are the
            // ones physically on the shot's line. The old code rayed through
            // the ship's CENTROID, ignored event.position entirely, and
            // damaged a 3-cell-wide column — a shot into the bow could hurt
            // the far side of the ship before the plate it actually struck.
            let travel = -dir;
            let impact_pos = event.position.unwrap_or(ship_center + dir * 66.0 * 12.0);
            let start = impact_pos + dir * 66.0 * 1.5;
            let end = start + travel * 66.0 * crate::building::MAX_WALK_STEPS as f32;
            let inv = player_gt.affine().inverse();
            let to_cell = |world: Vec2| {
                let p = inv.transform_point3(world.extend(0.0)).truncate();
                Vec2::new(p.x / 66.0, (p.y + 33.0) / 66.0)
            };
            let mut steps = grid_query
                .get(player_ship)
                .map(|grid| grid.walk(to_cell(start), to_cell(end)))
                .unwrap_or_default();

            // Line missed the grid entirely (impact reported off-hull):
            // the nearest live plate to the impact point takes it.
            if steps.is_empty() {
                let nearest = hull_query.iter()
                    .filter(|(_, _, _, parent)| parent.parent() == player_ship)
                    .map(|(e, hull, gt, _)| (e, hull.grid_position, gt.translation().truncate().distance_squared(impact_pos)))
                    .filter(|(_, _, d)| *d < (2.0 * 66.0_f32).powi(2))
                    .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
                if let Some((entity, cell, _)) = nearest {
                    steps.push(crate::building::GridStep { entity, cell, entry_face: IVec2::ZERO, t_enter: 0.0, span: 1.0 });
                }
            }

            // Direction of travel in the PLAYER's own cell space. The hull's
            // heading is baked into `inv`, so turning the ship off the threat
            // axis angles every plate on that side — armour you earn by
            // manoeuvring rather than by building.
            let dir_local = (to_cell(end) - to_cell(start)).normalize_or_zero();

            let mut remaining_damage = event.amount;
            for step in steps {
                if remaining_damage <= 0.0 {
                    break;
                }
                let block = block_query
                    .get(step.entity)
                    .copied()
                    .unwrap_or(crate::building::Block::module(step.cell));
                // Ask the block's shape what was actually met — a wedge whose
                // hollow corner the round crossed isn't hit at all.
                let cell_from = to_cell(start);
                let entry = cell_from + dir_local * step.t_enter;
                let exit = cell_from + dir_local * (step.t_enter + step.span);
                let Some(surface) = crate::combat::impact::clip_to_shape(
                    &block, step.entry_face, step.cell, entry, exit,
                ) else { continue };
                // No ammo profile on this path: incoming fire arrives as a
                // ShipDamaged event that doesn't carry what fired it, so the
                // round is treated as unspecialised (AP-like thresholds).
                let obl = crate::combat::impact::obliquity(
                    surface.normal, dir_local, &block, None, 1.0,
                );

                if let Ok((_, mut hull, _, parent)) = hull_query.get_mut(step.entity) {
                    if parent.parent() != player_ship { continue; }
                    let impact = crate::combat::impact::resolve_impact(remaining_damage, &block, surface.span, obl, None);
                    hull.health = (hull.health - impact.to_block).max(0.0);
                    remaining_damage = impact.through;

                    let health_pct = if hull.max_health > 0.0 {
                        hull.health / hull.max_health
                    } else {
                        0.0
                    };

                    // Breach if health drops below 30%
                    if health_pct < 0.3 && !hull.is_depressurized {
                        hull.is_depressurized = true;
                        breach_events.write(HullBreached {
                            segment: step.entity,
                            severity: 1.0 - health_pct,
                        });

                        // Send RoomDepressurized if this tile is in a room
                        if let Some(&room_id) = room_map.tile_to_room.get(&hull.grid_position) {
                            room_depressurize_events.write(RoomDepressurized {
                                room_id,
                                severity: 1.0 - health_pct,
                            });
                        }
                        // (Announced from the HullBreached event in the HUD,
                        // which has the severity. Saying it here as well put
                        // two breach notices up for every breach.)
                    }
                } else if let Ok((_, mut module, _, parent)) = module_query.get_mut(step.entity) {
                    if parent.parent() != player_ship { continue; }
                    // Modules take 70% of remaining damage as HP damage
                    let module_damage = remaining_damage * 0.7;
                    module.health = (module.health - module_damage).max(0.0);
                    // Absorb 50% of remaining damage
                    remaining_damage *= 0.5;
                }
            }
        } else {
            // === NON-DIRECTIONAL FALLBACK (radiation, explosion, etc.) ===
            // Player hull only — the random pick used to land on AI ships.
            let player_hulls: Vec<Entity> = hull_query.iter()
                .filter(|(_, _, _, parent)| parent.parent() == player_ship)
                .map(|(e, _, _, _)| e)
                .collect();
            if player_hulls.is_empty() { continue; }
            let target = player_hulls[rng.gen_range(0..player_hulls.len())];

            if let Ok((_, mut hull, _, _)) = hull_query.get_mut(target) {
                hull.health = (hull.health - event.amount).max(0.0);

                let health_pct = if hull.max_health > 0.0 {
                    hull.health / hull.max_health
                } else {
                    0.0
                };

                if health_pct < 0.3 && !hull.is_depressurized {
                    hull.is_depressurized = true;
                    breach_events.write(HullBreached {
                        segment: target,
                        severity: 1.0 - health_pct,
                    });

                    if let Some(&room_id) = room_map.tile_to_room.get(&hull.grid_position) {
                        room_depressurize_events.write(RoomDepressurized {
                            room_id,
                            severity: 1.0 - health_pct,
                        });
                    }
                    // (Announced from the HullBreached event in the HUD.)
                }
            }

            // 30% chance to also damage the nearest module (legacy behavior for non-directional)
            let hit_pos = event.position.unwrap_or(Vec2::ZERO);
            if event.position.is_some() && rng.gen::<f32>() < 0.3 {
                let closest_module = module_query
                    .iter_mut()
                    .filter(|(_, _, t, parent)| {
                        parent.parent() == player_ship
                            && t.translation().truncate().distance(hit_pos) < 80.0
                    })
                    .min_by(|(_, _, ta, _), (_, _, tb, _)| {
                        let da = ta.translation().truncate().distance(hit_pos);
                        let db = tb.translation().truncate().distance(hit_pos);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    });

                if let Some((_, mut module, _, _)) = closest_module {
                    let module_damage = event.amount * 0.5;
                    module.health = (module.health - module_damage).max(0.0);
                }
            }
        }

        // Spawn hit spark at damage position
        if let Some(pos) = event.position {
            commands.spawn((
                (Sprite {
                        color: Color::srgba(1.0, 0.4, 0.1, 0.9),
                        custom_size: Some(Vec2::splat(24.0)),
                        ..default()
                    }, Transform::from_xyz(pos.x, pos.y, 0.7)),
                HitEffect {
                    timer: Timer::from_seconds(0.3, TimerMode::Once),
                },
            ));
        }
    }
}

/// Gradual damage tint for modules (player or AI): darkens continuously as
/// health drops from max toward 0. Same reasoning as tint_damaged_hull —
/// blends from the stable spawn-time BaseSpriteColor, never from the live
/// (already-tinted) sprite.color.
pub fn tint_damaged_modules(
    mut module_query: Query<(&Module, &BaseSpriteColor, &mut Sprite), Without<DestroyedModule>>,
) {
    for (module, base, mut sprite) in module_query.iter_mut() {
        if module.max_health <= 0.0 { continue; }
        // A fully transparent base means the module draws no square of its own
        // and builds its shape out of child sprites instead — the angled
        // plates. There's nothing here to tint, and tinting it anyway fades a
        // solid rectangle in over the block's real silhouette as it takes
        // damage.
        if base.0.alpha() <= 0.0 { continue; }
        let damage_frac = 1.0 - (module.health / module.max_health).clamp(0.0, 1.0);
        // Only write a colour that differs (see tint_damaged_hull).
        let tint = mix_color(base.0, DAMAGE_TINT_TARGET, damage_frac);
        if sprite.color != tint {
            sprite.color = tint;
        }
    }
}

/// Processes module destruction — marks destroyed modules with DestroyedModule component.
/// Applies to every ship (AI blocks visibly "break" — dark grey — too), but
/// events/notifications only fire for the player's own modules.
pub fn process_module_destruction(
    mut commands: Commands,
    mut module_query: Query<(Entity, &mut Module, &mut Sprite, &ChildOf), Without<DestroyedModule>>,
    ship_query: Query<Entity, With<Ship>>,
    turrets: Query<&ChildOf, With<crate::combat::turret_deck::TurretBody>>,
    mut destroy_events: MessageWriter<ModuleDestroyed>,
    mut notifications: MessageWriter<ShowNotification>,
    debug_tuning: Option<Res<crate::debug::DebugTuning>>,
) {
    let player_ship = ship_query.single().ok();
    for (entity, mut module, mut sprite, parent) in module_query.iter_mut() {
        // A part on a turret deck belongs to the ship the turret stands on.
        let owner = turrets.get(parent.parent()).map(|t| t.parent()).unwrap_or(parent.parent());
        // God mode: whatever reached it, nothing of yours is ever destroyed.
        if debug_tuning.as_ref().is_some_and(|t| t.god_mode) && Some(owner) == player_ship {
            if module.health <= 0.0 {
                module.health = module.max_health;
            }
            continue;
        }
        // Health alone decides this. It used to also require is_active, which
        // meant a module that spawns INACTIVE could never be destroyed — its
        // health went to zero and past it and nothing ever marked it. That's
        // every Structural and Utility block: armour plates, bulkheads,
        // corridors, cargo. They stayed in ShipGrid forever (it filters on
        // Without<DestroyedModule>), so a wrecked plate went on armouring the
        // ship and deflecting rounds at 0 HP.
        //
        // The de-dup this was standing in for is the query's own
        // Without<DestroyedModule> filter, which is the correct guard.
        if module.health <= 0.0 {
            module.is_active = false;
            module.health = 0.0;
            // try_insert: applies to every ship including AI ships. If this
            // module's ship also lost its reactor this same frame,
            // ai_ship_death_system recursively despawns the whole ship —
            // plain insert() panics if that despawn flushes first.
            commands.entity(entity).try_insert(DestroyedModule {
                original_type: module.module_type,
            });
            sprite.color = DAMAGE_TINT_TARGET;
            if Some(owner) == player_ship {
                destroy_events.write(ModuleDestroyed { module: entity });
                notifications.write(ShowNotification {
                    message: format!("{} destroyed!", module.module_type.name()),
                    notification_type: NotificationType::Danger,
                    duration: 3.0,
                });
            }
        }
    }
}

/// Queues a freshly-destroyed module for removal. Doesn't despawn directly
/// here — see `PendingRemoval`'s doc comment for why.
pub fn queue_module_removal(
    mut commands: Commands,
    fresh: Query<Entity, Added<DestroyedModule>>,
) {
    for entity in fresh.iter() {
        commands.entity(entity).try_insert(PendingRemoval {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
        });
    }
}

/// Ticks the destruction-to-removal delay and despawns blocks (hull or
/// module, player or AI ship) once it expires — leaving an actual gap
/// instead of an inert dark husk. try_despawn: the block's whole ship may
/// already be gone by now (e.g. a reactor kill recursively despawning
/// everything), in which case this is just a no-op.
pub fn tick_pending_removal(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut PendingRemoval)>,
) {
    for (entity, mut pending) in query.iter_mut() {
        pending.timer.tick(time.delta());
        if pending.timer.is_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Hit effect that auto-despawns after timer expires.
/// Used for both ship damage sparks and creature hit flashes.
#[derive(Component)]
pub struct HitEffect {
    pub timer: Timer,
}

/// Cleanup system for hit effects
pub fn cleanup_hit_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut HitEffect)>,
) {
    for (entity, mut effect) in query.iter_mut() {
        effect.timer.tick(time.delta());
        if effect.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// How hard anything explosive goes up, and how far, of what it's listed with.
/// A magazine took about 21 blocks with it, and a ship with eight lost a tenth of
/// itself to its own shells. Reach is what counts: most blocks die to even a
/// weakened blast, so damage alone at 0.7 saved 4% (measured, with nothing else
/// shooting). A hit magazine should still hurt badly. Both sides alike.
pub(crate) const BLAST_STRENGTH: f32 = 0.7;
pub(crate) const BLAST_REACH: f32 = 0.8;

/// When an explosive module is freshly destroyed, queue a detonation with a short fuse delay.
/// Warning notifications only fire for the player's own ship (via ChildOf) — an
/// AI ship's reactor cooking off is not the player's emergency.
pub fn queue_detonation(
    mut commands: Commands,
    query: Query<(Entity, &Module, &Explosive, &ChildOf), Added<DestroyedModule>>,
    ship_query: Query<Entity, With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let player_ship = ship_query.single().ok();
    for (entity, module, explosive, parent) in query.iter() {
        let is_player_ship = Some(parent.parent()) == player_ship;

        // AI ship modules never queue detonations: their grid_position is in
        // THEIR ship's local coordinates, but detonation AoE resolves against
        // the player's GridOccupancy — an AI reactor at its (1,0) would blow
        // up the player's module at (1,0). AI ships have their own
        // hull-integrity damage model.
        if !is_player_ship {
            continue;
        }
        let fuse_secs = match explosive.explosive_type {
            ExplosiveType::Reactor => 0.15,
            ExplosiveType::Ammo => 0.05,
            ExplosiveType::Fuel => 0.2,
            ExplosiveType::Battery => 0.1,
        };

        commands.entity(entity).insert(PendingDetonation {
            timer: Timer::from_seconds(fuse_secs, TimerMode::Once),
            blast_radius: explosive.blast_radius * BLAST_REACH,
            blast_damage: explosive.blast_damage * BLAST_STRENGTH,
            explosive_type: explosive.explosive_type,
            grid_position: module.grid_position,
        });

        let warning = match explosive.explosive_type {
            ExplosiveType::Reactor => "Reactor critical! Explosion imminent!",
            ExplosiveType::Ammo => "Ammo cooking off!",
            ExplosiveType::Fuel => "Fuel tank rupture! Fire risk!",
            ExplosiveType::Battery => "Battery overload!",
        };
        notifications.write(ShowNotification {
            message: warning.into(),
            notification_type: NotificationType::Danger,
            duration: 3.0,
        });
    }
}

/// Ticks pending detonation timers and applies AoE damage when they finish.
pub fn process_detonations(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut commands: Commands,
    time: Res<Time>,
    // The ship it belongs to and where it is: grid cells are ship-LOCAL, so
    // the blast can only mean anything on the ship that owns the cell.
    mut det_query: Query<(Entity, &mut PendingDetonation, Option<&ChildOf>, Option<&GlobalTransform>)>,
    mut module_query: Query<(Entity, &mut Module), Without<DestroyedModule>>,
    mut hull_query: Query<(Entity, &mut HullSegment, &ChildOf), Without<HullDestroyed>>,
    occupancy: Res<GridOccupancy>,
    mut explosion_events: MessageWriter<ModuleExploded>,
    mut fire_events: MessageWriter<FireStarted>,
    mut breach_events: MessageWriter<HullBreached>,
    mut hull_destroy_events: MessageWriter<HullSegmentDestroyed>,
    room_map: Res<RoomMap>,
    mut room_depressurize_events: MessageWriter<RoomDepressurized>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    // Collect finished detonations first to avoid borrow issues
    let mut finished: Vec<(Entity, PendingDetonation, Option<Entity>, Option<Vec2>)> = Vec::new();

    for (entity, mut det, parent, gt) in det_query.iter_mut() {
        det.timer.tick(time.delta());
        if det.timer.is_finished() {
            finished.push((entity, PendingDetonation {
                timer: det.timer.clone(),
                blast_radius: det.blast_radius,
                blast_damage: det.blast_damage,
                explosive_type: det.explosive_type,
                grid_position: det.grid_position,
            }, parent.map(|p| p.parent()), gt.map(|g| g.translation().truncate())));
        }
    }

    for (det_entity, det, ship, world_pos) in finished {
        // Remove the PendingDetonation component
        commands.entity(det_entity).remove::<PendingDetonation>();

        let radius_cells = det.blast_radius;
        let radius_i = radius_cells.ceil() as i32;

        // Scan grid cells within blast radius
        for dx in -radius_i..=radius_i {
            for dy in -radius_i..=radius_i {
                let target_pos = det.grid_position + IVec2::new(dx, dy);
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist > radius_cells {
                    continue;
                }
                // Skip self (center)
                if dx == 0 && dy == 0 {
                    continue;
                }

                // Damage falloff: full damage at center, 30% at edge
                let falloff = 1.0 - (dist / radius_cells) * 0.7;
                let damage = det.blast_damage * falloff;

                // Try to damage a module at this position
                if let Some(&target_entity) = occupancy.cells.get(&target_pos) {
                    if let Ok((_, mut target_module)) = module_query.get_mut(target_entity) {
                        target_module.health = (target_module.health - damage).max(0.0);
                    }
                }

                // Try to damage hull at this position -- on THIS ship. Matched
                // on grid_position alone, the blast also hit every AI ship's
                // plate that happened to share the local cell, wherever in
                // the world that ship was.
                for (hull_entity, mut hull, parent) in hull_query.iter_mut() {
                    if Some(parent.parent()) != ship { continue; }
                    if hull.grid_position == target_pos {
                        hull.health = (hull.health - damage).max(0.0);

                        let health_pct = if hull.max_health > 0.0 {
                            hull.health / hull.max_health
                        } else {
                            0.0
                        };

                        if hull.health <= 0.0 {
                            hull_destroy_events.write(HullSegmentDestroyed {
                                segment: hull_entity,
                                grid_position: target_pos,
                            });
                        } else if health_pct < 0.3 && !hull.is_depressurized {
                            hull.is_depressurized = true;
                            breach_events.write(HullBreached {
                                segment: hull_entity,
                                severity: 1.0 - health_pct,
                            });
                            if let Some(&room_id) = room_map.tile_to_room.get(&target_pos) {
                                room_depressurize_events.write(RoomDepressurized {
                                    room_id,
                                    severity: 1.0 - health_pct,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Fuel/Battery explosions start fires on 4-adjacent non-destroyed modules
        if matches!(det.explosive_type, ExplosiveType::Fuel | ExplosiveType::Battery | ExplosiveType::Ammo) {
            for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let adj_pos = det.grid_position + offset;
                if let Some(&adj_entity) = occupancy.cells.get(&adj_pos) {
                    if module_query.get(adj_entity).is_ok() {
                        fire_events.write(FireStarted {
                            module: adj_entity,
                            grid_position: adj_pos,
                            intensity: 0.8,
                        });
                    }
                }
            }
        }

        // Send explosion event
        explosion_events.write(ModuleExploded {
            grid_position: det.grid_position,
            blast_damage: det.blast_damage,
            explosive_type: det.explosive_type,
        });

        // Was a flat opaque orange square sitting on your own ship for half a
        // second, sized blast_radius (in CELLS) x 66 -- up to ~264 units of
        // solid rectangle. blast_radius is cells, so x66 converts to world;
        // /2.2 because spawn_explosion's fireball grows to radius * 2.2.
        //
        // At the module's WORLD position. The grid cell converted to units is
        // ship-local, so the blast used to draw near the world origin rather
        // than on the ship that was exploding.
        let world_pos = world_pos.unwrap_or(Vec2::new(
            det.grid_position.x as f32 * 66.0,
            det.grid_position.y as f32 * 66.0 - 33.0,
        ));
        crate::combat::spawn_explosion(
            &mut commands,
            &fx,
            world_pos,
            det.blast_radius * 30.0,
            Color::srgb(1.0, 0.6, 0.1),
        );

        notifications.write(ShowNotification {
            message: format!("EXPLOSION at ({}, {})!", det.grid_position.x, det.grid_position.y),
            notification_type: NotificationType::Danger,
            duration: 3.0,
        });
    }
}

// ============================================================================
// AI SHIP DETONATIONS
// The player-ship path above resolves blasts through GridOccupancy, which
// only knows the PLAYER's grid — that's why queue_detonation skips AI ships.
// This pair resolves against the AI ship's own child blocks in world space
// instead, so shooting out an enemy's cannon cooks off ITS ammo and chains
// into ITS neighbors.
// ============================================================================

/// A queued explosion on an AI ship's module (world-space resolution).
#[derive(Component)]
pub struct AiPendingDetonation {
    pub timer: Timer,
    pub blast_radius_world: f32,
    pub blast_damage: f32,
    pub explosive_type: ExplosiveType,
    pub position: Vec2,
    pub ship: Entity,
}

/// Freshly destroyed explosive module on an AI ship → short fuse.
pub fn queue_ai_detonation(
    mut commands: Commands,
    query: Query<(Entity, &Explosive, &GlobalTransform, &ChildOf), Added<DestroyedModule>>,
    ai_ships: Query<(), With<crate::ai_ship::components::AiShip>>,
) {
    for (entity, explosive, gt, parent) in query.iter() {
        if ai_ships.get(parent.parent()).is_err() {
            continue; // player ship handled by queue_detonation above
        }
        let fuse_secs = match explosive.explosive_type {
            ExplosiveType::Reactor => 0.15,
            ExplosiveType::Ammo => 0.05,
            ExplosiveType::Fuel => 0.2,
            ExplosiveType::Battery => 0.1,
        };
        commands.entity(entity).try_insert(AiPendingDetonation {
            timer: Timer::from_seconds(fuse_secs, TimerMode::Once),
            blast_radius_world: explosive.blast_radius * BLAST_REACH * 66.0,
            blast_damage: explosive.blast_damage * BLAST_STRENGTH,
            explosive_type: explosive.explosive_type,
            position: gt.translation().truncate(),
            ship: parent.parent(),
        });
    }
}

/// Ticks AI detonation fuses; on boom, damages every block of that ship in
/// radius (with falloff) and sets survivors near the center burning.
pub fn process_ai_detonations(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut commands: Commands,
    time: Res<Time>,
    mut det_query: Query<(Entity, &mut AiPendingDetonation)>,
    children_query: Query<&Children>,
    mut module_query: Query<
        (&mut Module, &GlobalTransform),
        (Without<DestroyedModule>, With<crate::ai_ship::components::OwnedByAiShip>),
    >,
    mut hull_query: Query<
        (&mut HullSegment, &GlobalTransform),
        (Without<HullDestroyed>, With<crate::ai_ship::components::OwnedByAiShip>),
    >,
    mut ai_damage_events: MessageWriter<AiShipDamaged>,
    mut boom_events: MessageWriter<crate::events::AiModuleExploded>,
) {
    for (det_entity, mut det) in det_query.iter_mut() {
        det.timer.tick(time.delta());
        if !det.timer.is_finished() { continue; }
        commands.entity(det_entity).remove::<AiPendingDetonation>();

        let Ok(children) = children_query.get(det.ship) else { continue };
        // Fires start on surviving blocks close to the blast center.
        let fire_radius = det.blast_radius_world * 0.6;
        let starts_fires = matches!(det.explosive_type,
            ExplosiveType::Ammo | ExplosiveType::Fuel | ExplosiveType::Battery);

        for child in children.iter() {
            if child == det_entity { continue; }
            let (block_pos, dealt) = if let Ok((mut module, gt)) = module_query.get_mut(child) {
                let pos = gt.translation().truncate();
                let dist = det.position.distance(pos);
                if dist > det.blast_radius_world { continue; }
                let falloff = 1.0 - (dist / det.blast_radius_world) * 0.7;
                let damage = det.blast_damage * falloff;
                module.health = (module.health - damage).max(0.0);
                (pos, damage)
            } else if let Ok((mut hull, gt)) = hull_query.get_mut(child) {
                let pos = gt.translation().truncate();
                let dist = det.position.distance(pos);
                if dist > det.blast_radius_world { continue; }
                let falloff = 1.0 - (dist / det.blast_radius_world) * 0.7;
                let damage = det.blast_damage * falloff;
                hull.health = (hull.health - damage).max(0.0);
                (pos, damage)
            } else {
                continue;
            };

            crate::combat::spawn_floating_damage(
                &mut commands, block_pos, dealt, Color::srgb(1.0, 0.5, 0.15),
            );
            if starts_fires && det.position.distance(block_pos) < fire_radius {
                commands.entity(child).try_insert(
                    crate::combat::new_projectiles::BlockBurning {
                        dps: det.blast_damage * 0.1,
                        remaining: 6.0,
                        ship: det.ship,
                    },
                );
            }
        }

        // Same story as the player-side blast above.
        crate::combat::spawn_explosion(
            &mut commands,
            &fx,
            det.position,
            det.blast_radius_world * 0.9,
            Color::srgb(1.0, 0.55, 0.1),
        );

        ai_damage_events.write(AiShipDamaged {
            target: det.ship,
            source: DamageSource::Explosion,
            amount: 0.0, // damage already applied block-by-block above
            position: Some(det.position),
            direction: None,
            attacker: None, // self-inflicted detonation cascade, no external attacker
        });
        boom_events.write(crate::events::AiModuleExploded {
            position: det.position,
            blast_damage: det.blast_damage,
        });
    }
}

// ============================================================================
// EXPLOSION SHOCKWAVES
// Real detonations (cook-offs, death-rattle pops, final booms) give nearby
// ships a soft radial shove. Deliberately subtle — it should read as feel,
// never wrestle aim away from the player. Set SHOCKWAVE_SCALE to 0.0 to
// disable outright.
// ============================================================================

const SHOCKWAVE_SCALE: f32 = 1.0;
/// Impulse per point of blast damage (world-units/s of Δv per unit mass).
/// A 60-damage ammo cook-off vs the 1200-mass starter ship ≈ 40 u/s bump.
const IMPULSE_PER_DAMAGE: f32 = 800.0;
/// Δv cap per single blast — chains still stack past this, one blast can't
/// punt a fighter at projectile speeds on its own.
const SHOCKWAVE_MAX_KICK: f32 = 300.0;
/// AI ships have no ShipPhysics — derive mass from block count at the same
/// ratio as the starter ship (mass 1200 / ~35 blocks).
const AI_MASS_PER_BLOCK: f32 = 34.0;

/// Impulse ÷ mass shockwaves: every real detonation imparts momentum, so a
/// 100-block freighter shrugs off a corner cook-off while a 25-block raider
/// with a chain of HE going off gets properly yeeted. Off-center blasts also
/// torque the PLAYER ship (AI steering owns its own rotation and would just
/// fight it) — the lurch reads as "hit where it hurts".
pub fn explosion_shockwaves(
    mut ai_booms: MessageReader<crate::events::AiModuleExploded>,
    mut player_booms: MessageReader<ModuleExploded>,
    mut player_query: Query<(&GlobalTransform, &mut Velocity, &mut ShipPhysics), With<Ship>>,
    mut ai_query: Query<
        (Entity, &GlobalTransform, &mut Velocity),
        (With<crate::ai_ship::components::AiShip>, Without<Ship>),
    >,
    children_query: Query<&Children>,
) {
    if SHOCKWAVE_SCALE <= 0.0 {
        ai_booms.clear();
        player_booms.clear();
        return;
    }

    let mut rng = rand::thread_rng();
    let mut blasts: Vec<(Vec2, f32)> = Vec::new();
    for ev in ai_booms.read() {
        blasts.push((ev.position, ev.blast_damage));
    }
    // Player-side ModuleExploded only carries a ship-local grid position —
    // rotate it into world space through the ship's transform.
    if let Ok((player_gt, _, _)) = player_query.single() {
        let player_gt = *player_gt;
        for ev in player_booms.read() {
            let local = Vec3::new(
                ev.grid_position.x as f32 * 66.0,
                ev.grid_position.y as f32 * 66.0 - 33.0,
                0.0,
            );
            blasts.push((player_gt.transform_point(local).truncate(), ev.blast_damage));
        }
    }
    if blasts.is_empty() { return; }

    for (blast_pos, blast_damage) in blasts {
        let shock_radius = 250.0 + blast_damage * 2.0;
        let impulse = blast_damage * IMPULSE_PER_DAMAGE * SHOCKWAVE_SCALE;

        // Player: real mass + torque lurch
        if let Ok((gt, mut velocity, mut physics)) = player_query.single_mut() {
            let ship_pos = gt.translation().truncate();
            let offset = ship_pos - blast_pos;
            let dist = offset.length();
            if dist <= shock_radius {
                if let Some(dir) = offset.try_normalize() {
                    let falloff = 1.0 - dist / shock_radius;
                    let dv = (impulse / physics.mass.max(1.0) * falloff).min(SHOCKWAVE_MAX_KICK);
                    velocity.0 += dir * dv;
                    // Off-center lurch — small, random-signed, damped out by
                    // the steering blend within a second.
                    physics.angular_velocity += rng.gen_range(-1.0..1.0) * (dv / 100.0) * 0.35;
                }
            }
        }

        // AI ships: mass from block count, linear impulse only
        for (entity, gt, mut velocity) in ai_query.iter_mut() {
            let ship_pos = gt.translation().truncate();
            let offset = ship_pos - blast_pos;
            let dist = offset.length();
            if dist > shock_radius { continue; }
            let Some(dir) = offset.try_normalize() else { continue };
            let block_count = children_query.get(entity)
                .map(|c| c.iter().count())
                .unwrap_or(20)
                .max(1);
            let mass = block_count as f32 * AI_MASS_PER_BLOCK;
            let falloff = 1.0 - dist / shock_radius;
            let dv = (impulse / mass * falloff).min(SHOCKWAVE_MAX_KICK);
            velocity.0 += dir * dv;
        }
    }
}

#[cfg(test)]
mod destruction_tests {
    use super::*;

    fn module_at_zero(module_type: ModuleType, is_active: bool) -> Module {
        Module {
            module_type,
            health: 0.0,
            max_health: 100.0,
            power_consumption: 0.0,
            power_generation: 0.0,
            is_active,
            grid_position: IVec2::ZERO,
            size: IVec2::ONE,
            rotation: Rotation::North,
        }
    }

    fn run(module: Module) -> bool {
        run_with(module, false)
    }

    fn run_with(module: Module, god_mode: bool) -> bool {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(crate::debug::DebugTuning { god_mode, ..default() });
        app.add_message::<ModuleDestroyed>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, process_module_destruction);
        let ship = app.world_mut().spawn(Ship).id();
        let block = app.world_mut()
            .spawn((module, Sprite::default(), BaseSpriteColor(Color::WHITE)))
            .insert(ChildOf(ship))
            .id();
        app.update();
        app.world().get::<DestroyedModule>(block).is_some()
    }

    /// God mode: nothing of the player's is destroyed, however it got to
    /// zero (a road the damage guards missed breached the hull in a
    /// god-mode playtest, 2026-11-05).
    #[test]
    fn god_mode_never_loses_a_block() {
        assert!(!run_with(module_at_zero(ModuleType::ArmorPlate, false), true));
        assert!(run_with(module_at_zero(ModuleType::ArmorPlate, false), false));
    }

    /// The same for hull plates.
    #[test]
    fn god_mode_never_loses_a_plate() {
        for god_mode in [true, false] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            app.insert_resource(crate::debug::DebugTuning { god_mode, ..default() });
            app.add_systems(Update, crate::ship::hull::tint_destroyed_hull);
            let ship = app.world_mut().spawn(Ship).id();
            let plate = app.world_mut()
                .spawn((HullSegment { health: 0.0, max_health: 50.0, ..default() }, Sprite::default()))
                .insert(ChildOf(ship))
                .id();
            app.update();
            assert_eq!(app.world().get::<HullDestroyed>(plate).is_some(), !god_mode);
        }
    }

    /// A block at zero health is destroyed, whatever it is.
    ///
    /// This used to also require is_active, and everything Structural,
    /// Utility, Storage, Crew and Control spawns INACTIVE (see spawn_module) —
    /// so armour plates, bulkheads, corridors and cargo holds could take
    /// unlimited damage and never die. Worse for the armour model: ShipGrid
    /// filters on Without<DestroyedModule>, so a wrecked plate stayed in the
    /// grid and went on deflecting rounds at 0 HP.
    #[test]
    fn inactive_blocks_can_still_be_destroyed() {
        for module_type in [
            ModuleType::AngledArmorPlate,
            ModuleType::ArmorPlate,
            ModuleType::StaggeredArmorPlate,
            ModuleType::Bulkhead,
            ModuleType::Corridor,
        ] {
            assert!(run(module_at_zero(module_type, false)),
                "{module_type:?} spawns inactive and must still be destructible");
        }
    }

    /// Active modules keep working the way they always did.
    #[test]
    fn active_blocks_are_unaffected() {
        assert!(run(module_at_zero(ModuleType::SmallReactor, true)));
    }

    /// Every armour block this matters for really does spawn inactive — if
    /// that ever changes, this test should be the thing that notices.
    #[test]
    fn armour_plates_do_spawn_inactive() {
        for module_type in [
            ModuleType::AngledArmorPlate,
            ModuleType::AngledHullPlate,
            ModuleType::ArmorPlate,
        ] {
            assert!(!matches!(module_type.category(),
                ModuleCategory::Power | ModuleCategory::Propulsion
                | ModuleCategory::LifeSupport | ModuleCategory::Weapons
                | ModuleCategory::Detection),
                "{module_type:?} is in an always-active category; revisit the destruction rule");
        }
    }
}
