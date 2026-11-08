use bevy::prelude::*;

use super::*;
use crate::ai_ship::components::AiShip;
use crate::events::AiShipDamaged;

/// Spawn a projectile entity, differentiated by ammo type.
///
/// `range` sets how far the shot can actually travel before it expires —
/// this used to be a fixed per-ammo-type timer (1.5-4s) completely
/// disconnected from the weapon's stated range, so a "6000-range" weapon's
/// bullets (600u/s * 1.5 speed_mult = 900u/s, 1.5s lifetime) physically
/// expired after ~1350 units. Every ship "in range" per that stat was
/// wasting ammo shooting at something its own shots could never reach,
/// which meant nothing could ever actually fight at the ranges the AI
/// standoff distances and weapon stats implied. Lifetime is now derived
/// from range so a shot fired at max range takes exactly as long to arrive
/// as the geometry implies.
pub(crate) fn spawn_projectile(
    commands: &mut Commands,
    asset_server: &AssetServer,
    origin: Vec2,
    target: Vec2,
    damage: f32,
    speed: f32,
    range: f32,
    owner: ProjectileOwner,
    ammo_type: AmmoType,
    kinetic: Option<crate::combat::ammo_types::KineticAmmoType>,
) {
    let direction = (target - origin).normalize_or_zero();
    let angle = direction.y.atan2(direction.x);

    let texture_path = if owner.is_player() {
        crate::sprite_map::effect_sprite_path("torpedo")
    } else {
        crate::sprite_map::effect_sprite_path("enemy_projectile")
    };

    // Enemy projectiles keep red tint regardless of ammo type
    let final_color = if owner.is_player() { ammo_type.projectile_color() } else { Color::srgb(1.0, 0.2, 0.2) };

    let final_speed = speed * ammo_type.speed_mult();
    let lifetime_secs = (range / final_speed.max(1.0)).max(0.1);

    commands.spawn((
        (Sprite {
                image: asset_server.load(texture_path),
                color: final_color,
                custom_size: Some(ammo_type.projectile_size() * super::fx_scale()),
                ..default()
            }, Transform {
                translation: Vec3::new(origin.x, origin.y, 0.5),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            }),
        Projectile {
            damage,
            speed: final_speed,
            direction,
            lifetime: Timer::from_seconds(lifetime_secs, TimerMode::Once),
            owner,
            ammo_type,
            prev_pos: origin,
            bounces: 0,
            kinetic,
        },
    ));
}

/// Move projectiles and despawn expired ones
pub(super) fn projectile_movement(
    time: Res<Time>,
    mut commands: Commands,
    mut projectile_query: Query<(Entity, &mut Projectile, &mut Transform)>,
) {
    for (entity, mut projectile, mut transform) in projectile_query.iter_mut() {
        // Move
        let delta = projectile.direction * projectile.speed * time.delta_secs();
        projectile.prev_pos = transform.translation.truncate();
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;

        // Tick lifetime
        projectile.lifetime.tick(time.delta());
        if projectile.lifetime.is_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Check projectile collisions — ammo-type aware.
/// Torpedo/Bullet: single target. Charge: AoE hits all creatures in radius.
// Only creatures fire this legacy round now. Ship guns, the AI's included,
// fire new_projectiles::Projectile and are resolved by check_projectile_hits;
// the AiShip-owner arms below no longer see a round.
pub(super) fn projectile_collision(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut commands: Commands,
    // Transform and Sprite are MUTABLE here now (a deflected round is moved and
    // recoloured), which puts them up against the &Transform in the creature
    // and ship queries below. Bevy can't infer that a projectile is never a
    // creature or a ship, so the disjointness has to be spelled out — same
    // missing-canceling-pair issue already documented on ship_query.
    mut projectile_query: Query<
        (Entity, &mut Projectile, &mut Transform, &mut Sprite),
        (Without<Creature>, Without<Ship>, Without<AiShip>),
    >,
    // Enemy rounds resolve their own deflection, same maths the player's guns
    // use — see the ricochet arm below.
    block_query: Query<&crate::building::Block>,
    mut gunnery_query: Query<&mut crate::ai_ship::components::AiGunneryLog>,
    mut creature_query: Query<(Entity, &Transform, &mut Creature), Without<Ship>>,
    // Without<AiShip>: this system also reads AI ships' ShipShield
    // (immutably) via ai_ship_query below. Bevy's conflict checker can't
    // infer From With<Ship>/With<AiShip> alone that these two queries are
    // disjoint — same missing-canceling-pair issue documented on the
    // laser/ion systems in energy_weapons.rs.
    mut ship_query: Query<(
        Entity,
        &Transform,
        &GlobalTransform,
        Option<&mut crate::combat::shields::ShipShield>,
        Option<&crate::building::ShipGrid>,
    ), (With<Ship>, Without<AiShip>)>,
    // Every AI ship gets a ShipShield on spawn (attach_ai_shields) sized to
    // its actual hull extent — SUBMARINE_RADIUS below is only the fallback
    // for the brief window before that attaches. Without this, hit
    // detection used a flat 60-unit circle around the ship ROOT regardless
    // of actual size, so a shot aimed dead-center at a large ship (most of
    // the roster — Terran Hegemony, Eternal Hegemony, The Shepherd...) could sail
    // straight through its visible hull without ever registering as a hit.
    mut ai_ship_query: Query<(
        Entity,
        &Transform,
        Option<&mut crate::combat::shields::ShipShield>,
        // For the AI-vs-AI arm's block walk — see stage 2 below.
        &GlobalTransform,
        Option<&crate::building::ShipGrid>,
    ), With<AiShip>>,
    mut damage_events: MessageWriter<ShipDamaged>,
    mut ai_damage_events: MessageWriter<AiShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for (proj_entity, mut projectile, mut proj_transform, mut proj_sprite) in projectile_query.iter_mut() {
        // Same race the new projectile path has: projectile_movement despawns
        // a round the frame its timer runs out, and both systems queue commands
        // before the flush. Resolving a hit on a spent round queues a second
        // despawn for the same entity and the flush logs "Entity despawned".
        if projectile.lifetime.is_finished() {
            continue;
        }
        let proj_pos = proj_transform.translation.truncate();

        // Stage 2 of the ownership rework: an AI shot that whiffs past the
        // player now still resolves against every OTHER ai ship it's near
        // (never its own — see the `owner_ai_root` filter below). Player
        // shots and creature shots keep their stage-1 behavior; brains
        // still only aim at the player (that's slice 5), so this mostly
        // catches stray fire in crowded fights — but a shot flying through
        // a ship no longer does structurally nothing.
        let owner_ai_root = match projectile.owner {
            ProjectileOwner::AiShip(root) => Some(root),
            _ => None,
        };

        if projectile.owner.is_player() {
            let effective_radius = PROJECTILE_RADIUS * projectile.ammo_type.hit_radius_mult() + CREATURE_RADIUS;
            let is_aoe = projectile.ammo_type.is_aoe();
            let mut hit_any = false;

            let hit_color = if is_aoe { Color::srgb(0.5, 0.7, 1.0) } else { Color::srgb(1.0, 1.0, 0.5) };
            let hit_size = if is_aoe { 28.0 } else { 16.0 };

            for (_c_entity, c_transform, mut creature) in creature_query.iter_mut() {
                let c_pos = c_transform.translation.truncate();
                let dist = proj_pos.distance(c_pos);

                if dist < effective_radius {
                    creature.health -= projectile.damage;
                    hit_any = true;

                    spawn_hit_effect(&mut commands, c_pos, hit_color, hit_size);
                    spawn_floating_damage(&mut commands, c_pos, projectile.damage, Color::srgb(1.0, 1.0, 0.3));

                    if !is_aoe {
                        break;
                    }
                }
            }

            // Check AI ships if no creature was hit (single-target) or always for AoE
            if !hit_any || is_aoe {
                for (ai_entity, ai_transform, mut shield, _, _) in ai_ship_query.iter_mut() {
                    let ai_pos = shield.as_ref().map(|s| s.world_center(ai_transform))
                        .unwrap_or_else(|| ai_transform.translation.truncate());
                    let hit_radius = shield.as_ref().map(|s| s.radius).unwrap_or(SUBMARINE_RADIUS);
                    let dist = proj_pos.distance(ai_pos);

                    if dist < PROJECTILE_RADIUS + hit_radius {
                        // Shield absorbs first — this used to skip straight
                        // to hull/module damage regardless of shield state.
                        if let Some(shield) = shield.as_deref_mut() {
                            // Directional: only the facing arc blocks; a shot to
                            // the flank/rear slips past to the hull below.
                            if shield.is_up() && shield.covers_arc(proj_pos - ai_pos) {
                                shield.absorb(projectile.damage);
                                hit_any = true;
                                spawn_hit_effect(&mut commands, proj_pos, Color::srgb(0.5, 0.8, 1.0), 16.0);
                                if !is_aoe {
                                    break;
                                }
                                continue;
                            }
                        }

                        ai_damage_events.write(AiShipDamaged {
                            target: ai_entity,
                            source: DamageSource::Explosion,
                            amount: projectile.damage,
                            position: Some(proj_pos),
                            direction: Some(projectile.direction),
                            attacker: ship_query.single().ok().map(|(e, ..)| e),
                        });
                        hit_any = true;

                        spawn_hit_effect(&mut commands, ai_pos, Color::srgb(1.0, 0.5, 0.2), hit_size);
                        spawn_floating_damage(&mut commands, ai_pos, projectile.damage, Color::srgb(1.0, 0.8, 0.3));

                        if !is_aoe {
                            break;
                        }
                    }
                }
            }

            if hit_any {
                commands.entity(proj_entity).try_despawn();
            }
        } else {
            // Non-player projectile (AI ship or creature) -> player shield
            // first, then the hull. Tracks whether the player was actually
            // hit, so an AI-owned shot that whiffs can fall through to the
            // stage-2 AI-vs-AI check below instead of flying on forever.
            let mut hit_player = false;

            if let Ok((_, ship_transform, ship_gt, shield, grid)) = ship_query.single_mut() {
                let ship_pos = ship_transform.translation.truncate();
                let mut dist = proj_pos.distance(ship_pos);

                if let Some(mut shield) = shield {
                    // Bubble is centered on the blocks' centroid, not the root
                    let center = shield.world_center(ship_transform);
                    dist = proj_pos.distance(center);
                    if shield.is_up() && dist < shield.radius {
                        // Directional: only the lit segment blocks. A shot from
                        // an angle the segment isn't covering slips past to the
                        // hull (the segment tracks the most dangerous shot).
                        let hit_dir = proj_pos - center;
                        if shield.covers_arc(hit_dir) {
                            shield.absorb(projectile.damage);
                            spawn_hit_effect(&mut commands, proj_pos, Color::srgb(0.5, 0.8, 1.0), 16.0);
                            commands.entity(proj_entity).try_despawn();
                            continue;
                        }
                    }
                }

                // Shield down, or the arc didn't cover this angle: the round
                // carries on to the hull. It counts as a hit only where it
                // actually crosses a LIVE BLOCK.
                //
                // This used to be `dist < hull_hit_radius`, with
                // hull_hit_radius raised to the SHIELD radius — a bubble that
                // stands 70+ units clear of the hull at its narrowest and is
                // sized from the ship's full extent. So enemy rounds burst in
                // open space well short of the ship and damaged it anyway,
                // and because the radius was taken regardless of `is_up()`,
                // a dead shield went on doing it. The shield looked like it
                // was still working; it was just the bubble acting as a
                // hitbox.
                let inv = ship_gt.affine().inverse();
                let to_cell = |world: Vec2| {
                    let p = inv.transform_point3(world.extend(0.0)).truncate();
                    Vec2::new(p.x / 66.0, (p.y + 33.0) / 66.0)
                };
                let cell_from = to_cell(projectile.prev_pos);
                let dir_local = (to_cell(proj_pos) - cell_from).normalize_or_zero();
                let block_hit = grid.and_then(|grid| {
                    grid.walk(cell_from, to_cell(proj_pos)).into_iter().next()
                });

                if let Some(step) = block_hit {
                    // RICOCHET — decided HERE, at the round, not downstream in
                    // process_ship_damage. By the time a ShipDamaged event is
                    // read the projectile is already gone, so a deflection off
                    // the player's plating could never be seen. Enemy fire
                    // skips off your armour exactly the way yours skips off
                    // theirs.
                    let block = block_query
                        .get(step.entity)
                        .copied()
                        .unwrap_or(crate::building::Block::module(step.cell));
                    let entry = cell_from + dir_local * step.t_enter;
                    let exit = cell_from + dir_local * (step.t_enter + step.span);
                    let surface = crate::combat::impact::clip_to_shape(
                        &block, step.entry_face, step.cell, entry, exit,
                    );
                    let obl = surface
                        .map(|s| crate::combat::impact::obliquity(s.normal, dir_local, &block, projectile.kinetic, 1.0))
                        .unwrap_or(crate::combat::impact::Obliquity::HEAD_ON);

                    if obl.ricochet && projectile.bounces < 2 {
                        let local = Vec2::new(step.cell.x as f32 * 66.0, step.cell.y as f32 * 66.0 - 33.0);
                        let at = ship_gt.affine().transform_point3(local.extend(0.0)).truncate();
                        let n_local = surface.map(|s| s.normal).unwrap_or(Vec2::ZERO);
                        let n = ship_gt.affine()
                            .transform_vector3(n_local.extend(0.0))
                            .truncate()
                            .normalize_or_zero();
                        let v = projectile.direction.normalize_or_zero();
                        if n != Vec2::ZERO && v != Vec2::ZERO {
                            let mirror = v - 2.0 * v.dot(n) * n;
                            let tangent = (v - v.dot(n) * n).normalize_or_zero();
                            let skid = 0.35 + 0.45 * (1.0 - obl.cos_impact);
                            let scatter = (rand::random::<f32>() - 0.5) * 0.17;
                            let out = Vec2::from_angle(scatter)
                                .rotate(mirror.lerp(tangent, skid).normalize_or_zero());
                            projectile.direction = out;
                            projectile.speed = (projectile.speed * (0.45 + 0.40 * (1.0 - obl.cos_impact)))
                                .min(super::new_projectiles::RICOCHET_MAX_SPEED);
                            projectile.damage *= 0.10 + 0.30 * (1.0 - obl.cos_impact);
                            projectile.bounces += 1;
                            proj_sprite.color = Color::srgb(1.0, 0.85, 0.55);
                            // Step it clear so it doesn't restart inside the
                            // hull and spend its bounces on the spot.
                            proj_transform.translation.x += out.x * 70.0;
                            proj_transform.translation.y += out.y * 70.0;
                            projectile.prev_pos = proj_transform.translation.truncate();
                            let graze = 1.0 - obl.cos_impact;
                            super::spawn_impact_sparks(&mut commands, &fx, at, out, graze, 9 + (graze * 7.0) as usize);
                            spawn_hit_effect(&mut commands, at, Color::srgb(0.95, 0.95, 0.85), 10.0);
                        }
                        // Tell the shooter. A ship that can't see its own
                        // rounds skipping can't learn anything from them.
                        if let crate::components::ProjectileOwner::AiShip(shooter) = projectile.owner {
                            if let Ok(mut log) = gunnery_query.get_mut(shooter) {
                                log.ricochets += 1;
                            }
                        }
                        hit_player = true;
                        continue;
                    }

                    // Report the impact at the block the round actually
                    // reached, so process_ship_damage's own walk starts from
                    // the right place instead of from the bubble's edge.
                    let local = Vec2::new(step.cell.x as f32 * 66.0, step.cell.y as f32 * 66.0 - 33.0);
                    let block_pos = ship_gt.affine()
                        .transform_point3(local.extend(0.0))
                        .truncate();
                    // Incoming fire sparks off your own plating too. Without
                    // this, hits on the player registered only as a hull-bar
                    // twitch and a notification line.
                    spawn_hit_effect(&mut commands, block_pos, Color::srgb(1.0, 0.55, 0.2), 14.0);
                    super::spawn_impact_sparks(&mut commands, &fx, block_pos, -projectile.direction, 0.25, 6);
                    damage_events.write(ShipDamaged {
                        source: DamageSource::Creature(Entity::PLACEHOLDER),
                        amount: projectile.damage,
                        position: Some(block_pos),
                        // process_ship_damage's outermost-first penetration
                        // sort assumes `direction` points from the ship
                        // TOWARD the attacker (every other ShipDamaged
                        // writer uses (attacker_pos - ship_pos)). This
                        // passed the projectile's own direction of travel —
                        // attacker THROUGH the ship, the opposite sign — so
                        // damage was applied outermost-first along the
                        // wrong axis: a shot into the bow could destroy
                        // blocks at the stern first instead of the bow
                        // blocks it actually hit.
                        direction: Some(-projectile.direction),
                        applied: false,
                    });

                    notifications.write(ShowNotification {
                        message: format!("Hull hit! -{:.0} damage", projectile.damage),
                        notification_type: NotificationType::Danger,
                        duration: 2.0,
                    });

                    commands.entity(proj_entity).try_despawn();
                    hit_player = true;
                }
            }

            // Stage 2: an AI-owned shot that missed the player is still
            // live — check it against every OTHER ai ship (never its own;
            // firing-arc/adjacency already keeps a ship from hitting
            // itself, this is belt-and-suspenders). Creature-owned shots
            // don't get this arm — creatures don't fight AI ships here.
            if !hit_player {
                if let Some(owner_root) = owner_ai_root {
                    for (ai_entity, ai_transform, mut shield, ai_gt, grid) in ai_ship_query.iter_mut() {
                        if ai_entity == owner_root { continue; }
                        let ai_pos = shield.as_ref().map(|s| s.world_center(ai_transform))
                            .unwrap_or_else(|| ai_transform.translation.truncate());
                        let reach = shield.as_ref().map(|s| s.radius).unwrap_or(SUBMARINE_RADIUS);
                        // Broad phase over the whole step, not its end point.
                        let (dist, _) = super::new_projectiles::segment_closest(
                            projectile.prev_pos, proj_pos, ai_pos,
                        );
                        if dist > PROJECTILE_RADIUS + reach { continue; }

                        // Same two tests the player's hull gets above. The
                        // shield stops a round only while it is UP, inside the
                        // bubble, and on the lit arc; this arm used to absorb
                        // on is_up() alone, ignoring the arc.
                        if let Some(shield) = shield.as_deref_mut() {
                            if shield.is_up()
                                && proj_pos.distance(ai_pos) < shield.radius
                                && shield.covers_arc(proj_pos - ai_pos)
                            {
                                shield.absorb(projectile.damage);
                                spawn_hit_effect(&mut commands, proj_pos, Color::srgb(0.5, 0.8, 1.0), 16.0);
                                commands.entity(proj_entity).try_despawn();
                                break;
                            }
                        }

                        // ...and the hull is hit only where the round crosses
                        // a LIVE BLOCK. The bubble radius used to be the
                        // hitbox whether the shield was up or not, so stray
                        // fire burst in open space a long way short of the
                        // hull and damaged the ship anyway.
                        let Some(grid) = grid else { continue };
                        let inv = ai_gt.affine().inverse();
                        let to_cell = |world: Vec2| {
                            let p = inv.transform_point3(world.extend(0.0)).truncate();
                            Vec2::new(p.x / 66.0, (p.y + 33.0) / 66.0)
                        };
                        let Some(step) = grid
                            .walk(to_cell(projectile.prev_pos), to_cell(proj_pos))
                            .into_iter()
                            .next()
                        else { continue };
                        let local = Vec2::new(step.cell.x as f32 * 66.0, step.cell.y as f32 * 66.0 - 33.0);
                        let block_pos = ai_gt.affine().transform_point3(local.extend(0.0)).truncate();

                        ai_damage_events.write(AiShipDamaged {
                            target: ai_entity,
                            source: DamageSource::Explosion,
                            amount: projectile.damage,
                            position: Some(block_pos),
                            // From the ship TOWARD the attacker, the
                            // convention every other damage writer uses.
                            direction: Some(-projectile.direction),
                            attacker: Some(owner_root),
                        });

                        spawn_hit_effect(&mut commands, block_pos, Color::srgb(1.0, 0.5, 0.2), 16.0);

                        commands.entity(proj_entity).try_despawn();
                        break;
                    }
                }
            }
        }
    }
}
