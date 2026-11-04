use bevy::prelude::*;
use crate::celestial::components::{GravityAffected, GravityForce};
use super::targeting::{TargetSelection, FireGroupState, FireGroup};
use super::combat_features::WeakPoint;
use super::*;

// ============================================================================
// ENERGY WEAPONS — Laser, Ion Disruptor, EMP Missile
// ============================================================================

// ============================================================================
// LASER BEAM — instant hit, continuous, cuts through targets
// ============================================================================

/// Component marking an active laser beam
#[derive(Component)]
pub struct LaserBeam {
    pub origin: Vec2,
    pub direction: Vec2,
    pub damage_per_second: f32,
    pub max_range: f32,
    pub power_drain_per_second: f32,
    pub heat_per_second: f32,
}

/// Visual line for the laser beam
#[derive(Component)]
pub struct LaserBeamVisual;

/// System: fire laser weapons — continuous beam while fire group held
pub fn fire_laser_system(
    time: Res<Time>,
    fire_state: Res<FireGroupState>,
    // Bundled: this system is at Bevy's 16-parameter ceiling.
    targeting: (Res<TargetSelection>, Res<crate::combat::targeting::AimLock>),
    ship_query: Query<(Entity, &Transform, &ShipPhysics), With<Ship>>,
    weapon_query: Query<(
        Entity, &Module, Has<CrewStation>, Option<&ModuleEfficiency>, &Weapon, &FireGroup, &GlobalTransform, &WeaponMount, &ChildOf,
        Option<&ModuleTemperature>,
    ), (Without<DestroyedModule>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    target_query: Query<&Transform, Without<Ship>>,
    power_state: ResMut<PowerState>,
    mut creature_query: Query<(&Transform, &mut Creature, Option<&Velocity>, Option<&WeakPoint>), Without<Ship>>,
    mut ai_ship_query: Query<
        (Entity, &Transform, &Children, &mut crate::combat::shields::ShipShield),
        With<crate::ai_ship::components::AiShip>,
    >,
    // With<OwnedByAiShip>: this function also reads the player's own
    // weapon modules via `weapon_query` above (&Module, immutable). Without
    // a static marker proving these two queries can never match the same
    // entity, Bevy's conflict checker can't tell that ai_module_query's
    // &mut Module is disjoint from weapon_query's &Module — the check_*_hits
    // systems this pattern was copied from don't have this issue because
    // they're separate from firing; the laser resolves hits inline since
    // it's an instant beam, not a projectile entity checked later.
    mut ai_module_query: Query<(&mut Module, &GlobalTransform), (Without<DestroyedModule>, With<crate::ai_ship::components::OwnedByAiShip>)>,
    mut ai_hull_query: Query<(&mut HullSegment, &GlobalTransform), (Without<crate::components::HullDestroyed>, With<crate::ai_ship::components::OwnedByAiShip>)>,
    mut ai_damage_events: MessageWriter<crate::events::AiShipDamaged>,
    // Bevy caps systems at 16 params and this function is at the limit —
    // related params are grouped into tuples (one SystemParam each) to fit.
    (mut fired_events, mut commands): (MessageWriter<crate::events::WeaponFired>, Commands),
    // Terrain occlusion for the beam (rocks are cover) — see ship::collision.
    (collider_grid, terrain): (
        Res<crate::ship::collision::ColliderGrid>,
        Query<
            (&Transform, &crate::ship::collision::Collider),
            crate::ship::collision::TerrainFilter,
        >,
    ),
    existing_beams: Query<Entity, With<LaserBeamVisual>>,
    // Per-weapon "which block am I currently cutting into" — without this,
    // the beam picked the nearest block to its aim point fresh every frame,
    // so the smallest aim drift (or the target ship just moving) scattered
    // damage across whatever was nearest each instant instead of boring
    // through one spot. Cleared once that block is destroyed.
    mut laser_locks: Local<std::collections::HashMap<Entity, Entity>>,
) {
    let (selection, aim_lock) = targeting;
    let dt = time.delta_secs();

    // Despawn old beam visuals
    for entity in existing_beams.iter() {
        commands.entity(entity).try_despawn();
    }

    let Ok((player_ship, _ship_transform, ship_physics)) = ship_query.single() else { return };
    let ship_forward = Vec2::new(ship_physics.rotation.cos(), ship_physics.rotation.sin());

    for (weapon_entity, module, has_station, staffing, weapon, fire_group, global_transform, mount, parent, temp) in weapon_query.iter() {
        // A gun with nobody on it does not fire.
        if !crate::combat::weapon_is_crewed(has_station, staffing) {
            continue;
        }
        // Player ship only — AI ships carry identical laser components and
        // would otherwise fire whenever the player does, beaming toward
        // whatever the player has targeted (including themselves).
        if parent.parent() != player_ship { continue; }
        if module.module_type != ModuleType::Laser { continue; }
        if !module.is_active { continue; }

        // Check fire group
        let group_firing = fire_state.firing[fire_group.group as usize % 4];
        if !group_firing { continue; }

        // Check if overheated
        if let Some(temp) = temp {
            if temp.current >= temp.max_temp * 0.95 { continue; } // Overheated
        }

        // Check power
        let power_cost = 15.0 * dt; // Power drain per second
        if power_state.power_balance < -power_cost {
            continue; // Not enough power
        }

        let weapon_pos = global_transform.translation().truncate();
        let mut beam_range = weapon.range;

        // Aim at the right-click lock first (a named block), then the
        // ship-level selection, then straight ahead.
        let beam_dir = if let Some(point) = aim_lock.aim_point() {
            (point - weapon_pos).normalize_or_zero()
        } else if let Some(target) = selection.target {
            if let Ok(target_transform) = target_query.get(target) {
                (target_transform.translation.truncate() - weapon_pos).normalize_or_zero()
            } else {
                ship_forward
            }
        } else {
            ship_forward
        };

        // Fixed/broadside mounts can't swivel outside their arc — without this
        // check a side-mounted laser could beam straight through the hull to
        // reach a target on the wrong side of the ship.
        if !is_in_firing_arc(ship_physics.rotation, &module.rotation, mount, beam_dir) {
            continue;
        }

        // Beam is live this frame — written every frame while firing; the
        // audio system rate-limits continuous weapons per type.
        fired_events.write(crate::events::WeaponFired {
            weapon_type: module.module_type,
            position: weapon_pos,
            from_player: true,
        });

        // Terrain blocks the beam: clamp range at the first asteroid/station/
        // planet along the ray, so rocks are cover against lasers too.
        let mut terrain_stop: Option<Vec2> = None;
        if let Some((_, point)) = crate::ship::collision::terrain_segment_hit(
            &collider_grid,
            &terrain,
            weapon_pos,
            weapon_pos + beam_dir * beam_range,
            0.0,
        ) {
            beam_range = weapon_pos.distance(point);
            terrain_stop = Some(point);
        }
        // The beam chews at the rock face — small continuous scorch glow.
        if let Some(point) = terrain_stop {
            spawn_hit_effect(&mut commands, point, Color::srgba(1.0, 0.7, 0.4, 0.7), 8.0);
        }

        // Trace beam — check all creatures along the line
        let beam_end = weapon_pos + beam_dir * beam_range;
        let mut total_damage_dealt = 0.0;
        let mut hit_count = 0u32;
        let mut blocked = false;
        // Where the beam actually stopped — drives the visual beam length.
        // Previously the beam always drew to a fixed 70%-of-range guess
        // whenever `blocked` was true, regardless of how far away the
        // actual target was, so it visually overshot anything closer than
        // that — looking like it passed straight through.
        let mut hit_pos: Option<Vec2> = None;

        // === AI SHIPS — the beam previously only ever checked creatures, so
        // it was physically incapable of hitting a ship no matter how well
        // aimed. Shield first (drains continuously while the beam is on
        // it), then nearest hull/module to the beam's closest point once
        // the shield is down — same resolution model check_projectile_hits
        // uses for kinetic rounds, just continuous instead of one-shot.
        // Any ship hit blocks the beam (it's a full hull, not a small mote).
        'ai_ships: for (ai_entity, ai_transform, children, mut shield) in ai_ship_query.iter_mut() {
            if blocked { break; }

            let center = shield.world_center(ai_transform);
            let to_ship = center - weapon_pos;
            let projection = to_ship.dot(beam_dir);
            if projection < 0.0 || projection > beam_range { continue; }
            let closest_point = weapon_pos + beam_dir * projection;
            let perpendicular_dist = center.distance(closest_point);

            if shield.is_up() && perpendicular_dist < shield.radius && shield.covers_arc(closest_point - center) {
                let damage = weapon.damage * dt;
                shield.absorb(damage);
                total_damage_dealt += damage;
                hit_count += 1;
                blocked = true;
                // Approximate impact point: where the beam crosses the
                // shield bubble's edge, not its center.
                let into_shield = (shield.radius - (shield.radius.powi(2) - perpendicular_dist.powi(2)).max(0.0).sqrt()).max(0.0);
                hit_pos = Some(weapon_pos + beam_dir * (projection - into_shield));
                laser_locks.remove(&weapon_entity);
                continue 'ai_ships;
            }

            if perpendicular_dist < shield.radius + 60.0 {
                // Stick to whatever block this weapon was already cutting
                // into, as long as it's still alive, still on THIS ship, and
                // still on the beam — only re-scan once any of that fails.
                // The lock is per weapon, not per ship, so without the
                // ownership test a beam swung onto a second ship kept
                // damaging a block on the first, wherever it now was.
                let locked = laser_locks.get(&weapon_entity).copied()
                    .filter(|e| children.contains(e));
                let still_on_beam = |gt: &GlobalTransform| {
                    on_beam(gt.translation().truncate(), weapon_pos, beam_dir, beam_range)
                };
                let mut target: Option<(Entity, bool)> = None; // (entity, is_module)
                if let Some(locked_entity) = locked {
                    if let Ok((module, gt)) = ai_module_query.get(locked_entity) {
                        if module.health > 0.0 && still_on_beam(gt) { target = Some((locked_entity, true)); }
                    } else if let Ok((hull, gt)) = ai_hull_query.get(locked_entity) {
                        if hull.health > 0.0 && still_on_beam(gt) { target = Some((locked_entity, false)); }
                    }
                }

                if target.is_none() {
                    // Pick the FIRST block the beam actually reaches — the
                    // one with the smallest projection along the beam from
                    // the weapon — not whichever block sits nearest the
                    // ship's overall centroid (the old `closest_point` was
                    // the beam's closest approach to `center`, a point that
                    // can land behind several rows of armor on an off-center
                    // or angled ship). Picking by that instead of true
                    // front-to-back order let the beam skip visible hull and
                    // light up something several blocks back.
                    let mut best_module: Option<(Entity, f32)> = None; // (entity, projection)
                    for child in children.iter() {
                        if let Ok((_, gt)) = ai_module_query.get(child) {
                            let pos = gt.translation().truncate();
                            if !on_beam(pos, weapon_pos, beam_dir, beam_range) { continue; }
                            let proj = (pos - weapon_pos).dot(beam_dir);
                            if best_module.map(|(_, bp)| proj < bp).unwrap_or(true) {
                                best_module = Some((child, proj));
                            }
                        }
                    }
                    let mut best_hull: Option<(Entity, f32)> = None;
                    for child in children.iter() {
                        if let Ok((_, gt)) = ai_hull_query.get(child) {
                            let pos = gt.translation().truncate();
                            if !on_beam(pos, weapon_pos, beam_dir, beam_range) { continue; }
                            let proj = (pos - weapon_pos).dot(beam_dir);
                            if best_hull.map(|(_, bp)| proj < bp).unwrap_or(true) {
                                best_hull = Some((child, proj));
                            }
                        }
                    }

                    let hit_module = beam_meets_module_first(
                        best_module.map(|(_, p)| p),
                        best_hull.map(|(_, p)| p),
                    );

                    target = if hit_module {
                        best_module.map(|(e, _)| (e, true))
                    } else {
                        best_hull.map(|(e, _)| (e, false))
                    };
                }

                let Some((block_entity, is_module)) = target else { continue };
                laser_locks.insert(weapon_entity, block_entity);
                let damage = weapon.damage * dt;

                if is_module {
                    if let Ok((mut module, gt)) = ai_module_query.get_mut(block_entity) {
                        module.health = (module.health - damage).max(0.0);
                        hit_pos = Some(gt.translation().truncate());
                        ai_damage_events.write(crate::events::AiShipDamaged {
                            target: ai_entity,
                            source: crate::events::DamageSource::Explosion,
                            amount: 0.0,
                            position: hit_pos,
                            direction: None,
                            attacker: Some(player_ship),
                        });
                        total_damage_dealt += damage;
                        hit_count += 1;
                        blocked = true;
                        if module.health <= 0.0 { laser_locks.remove(&weapon_entity); }
                    }
                } else if let Ok((hull, _)) = ai_hull_query.get(block_entity) {
                    // ARMOUR EXPOSURE, as check_projectile_hits does it: a
                    // module still under this plate takes the beam's small
                    // pass-through share and the plate spends the rest. The
                    // module is found by footprint, since a plate and the
                    // module it covers share a cell centre.
                    let cell = hull.grid_position;
                    let covered = children.iter().find(|child| {
                        ai_module_query.get(*child).ok().is_some_and(|(module, _)| {
                            let footprint = crate::building::footprints::footprint_override(module.module_type);
                            crate::building::ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint)
                                .contains(&cell)
                        })
                    });
                    let (to_plate, to_module) = beam_split(damage, covered.is_some());
                    if let (Some(module_entity), true) = (covered, to_module > 0.0) {
                        if let Ok((mut module, _)) = ai_module_query.get_mut(module_entity) {
                            module.health = (module.health - to_module).max(0.0);
                        }
                    }
                    let Ok((mut hull, gt)) = ai_hull_query.get_mut(block_entity) else { continue };
                    hull.health = (hull.health - to_plate).max(0.0);
                    hit_pos = Some(gt.translation().truncate());
                    ai_damage_events.write(crate::events::AiShipDamaged {
                        target: ai_entity,
                        source: crate::events::DamageSource::Explosion,
                        amount: 0.0,
                        position: hit_pos,
                        direction: None,
                        attacker: Some(player_ship),
                    });
                    total_damage_dealt += damage;
                    hit_count += 1;
                    blocked = true;
                    if hull.health <= 0.0 { laser_locks.remove(&weapon_entity); }
                }
            }
        }

        for (creature_transform, mut creature, velocity, weak_point) in creature_query.iter_mut() {
            if creature.health <= 0.0 { continue; }
            if blocked { break; }

            let creature_pos = creature_transform.translation.truncate();

            // Point-to-line distance check
            let to_creature = creature_pos - weapon_pos;
            let projection = to_creature.dot(beam_dir);

            if projection < 0.0 || projection > beam_range { continue; }

            let closest_point = weapon_pos + beam_dir * projection;
            let perpendicular_dist = creature_pos.distance(closest_point);

            let hit_radius = match creature.creature_type {
                CreatureType::Leviathan => 80.0,
                CreatureType::Stalker => 25.0,
                CreatureType::ParasiteSwarm => 12.0,
                CreatureType::VoidDrifter => 10.0,
            };

            if perpendicular_dist > hit_radius { continue; }

            // HIT — apply continuous damage
            let mut damage = weapon.damage * dt;

            // Weak point check
            if let Some(wp) = weak_point {
                let vel = velocity.map(|v| v.0).unwrap_or(Vec2::ZERO);
                let mult = super::combat_features::check_weak_point_hit(
                    closest_point, creature_pos, vel, wp,
                );
                damage *= mult;
            }

            creature.health -= damage;
            total_damage_dealt += damage;
            hit_count += 1;

            // Beam continues through small targets, blocked by large ones
            match creature.creature_type {
                CreatureType::Leviathan => {
                    blocked = true;
                    hit_pos = Some(closest_point);
                }
                _ => {} // Beam cuts through
            }
        }

        // Draw beam visual — stops exactly where it hit instead of a fixed
        // 70%-of-range guess (see hit_pos doc comment above).
        let actual_end = hit_pos.unwrap_or(beam_end);

        let midpoint = (weapon_pos + actual_end) / 2.0;
        let length = weapon_pos.distance(actual_end);
        let angle = beam_dir.y.atan2(beam_dir.x);

        // Core beam (bright)
        commands.spawn((
            (Sprite {
                    color: Color::srgba(0.3, 0.9, 0.4, 0.8),
                    custom_size: Some(Vec2::new(length, 3.0 * super::fx_scale())),
                    ..default()
                }, Transform {
                    translation: Vec3::new(midpoint.x, midpoint.y, 0.6),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                }),
            LaserBeamVisual,
        ));

        // Glow around beam
        commands.spawn((
            (Sprite {
                    color: Color::srgba(0.2, 0.7, 0.3, 0.2),
                    custom_size: Some(Vec2::new(length, 10.0 * super::fx_scale())),
                    ..default()
                }, Transform {
                    translation: Vec3::new(midpoint.x, midpoint.y, 0.55),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                }),
            LaserBeamVisual,
        ));

        // Impact flash if hitting something
        if hit_count > 0 {
            spawn_hit_effect(&mut commands, actual_end, Color::srgb(0.4, 1.0, 0.5), 8.0);
        }
    }
}

/// Half-width of the beam's footprint on a block: a block whose centre is
/// within this of the beam line (and ahead of the emitter, inside range) is
/// one the beam can be cutting into.
const BEAM_BLOCK_REACH: f32 = 45.0;

/// Is a block centred at `pos` on the beam this frame?
fn on_beam(pos: Vec2, origin: Vec2, dir: Vec2, range: f32) -> bool {
    let proj = (pos - origin).dot(dir);
    if proj < 0.0 || proj > range {
        return false;
    }
    pos.distance(origin + dir * proj) < BEAM_BLOCK_REACH
}

/// Does the beam reach the nearest module before the nearest plate?
///
/// Strictly before. A plate and the module it covers sit on the same cell
/// centre, so they tie, and `<=` handed every tie to the module: the beam
/// reached straight under the armour every time, the one weapon in the game
/// that ignored plating.
fn beam_meets_module_first(module_proj: Option<f32>, hull_proj: Option<f32>) -> bool {
    match (module_proj, hull_proj) {
        (Some(m), Some(h)) => m < h,
        (Some(_), None) => true,
        _ => false,
    }
}

/// One tick of beam damage on a plate: (to the plate, to the module under
/// it). A beam is unspecialised, so the bleed-through is the ammo table's
/// default -- the same split a kinetic round with no profile gets.
fn beam_split(damage: f32, covers_module: bool) -> (f32, f32) {
    let pass = if covers_module {
        crate::combat::ammo_types::armor_pass_through(None)
    } else {
        0.0
    };
    (damage * (1.0 - pass), damage * pass)
}

#[cfg(test)]
mod beam_tests {
    use super::*;

    /// The report: beams always hit under the plating. A plate and its
    /// module tie on distance, and the tie has to go to the plate.
    #[test]
    fn a_tie_goes_to_the_plate() {
        assert!(!beam_meets_module_first(Some(300.0), Some(300.0)));
        assert!(beam_meets_module_first(Some(234.0), Some(300.0)), "an exposed module in front is still hit");
        assert!(beam_meets_module_first(Some(300.0), None));
        assert!(!beam_meets_module_first(None, Some(300.0)));
    }

    /// Armour stops most of a beam, never all of it, and nothing leaks
    /// through a plate with nothing behind it.
    #[test]
    fn a_plate_soaks_most_of_the_beam() {
        let (plate, module) = beam_split(100.0, true);
        assert!(plate > module && module > 0.0, "plate {plate}, module {module}");
        assert!((plate + module - 100.0).abs() < 1e-4, "damage was created or lost");
        assert_eq!(beam_split(100.0, false), (100.0, 0.0));
    }

    /// A held lock must be re-checked against where the beam is now. A block
    /// the beam has swung off (or one behind the emitter) is not on it.
    #[test]
    fn a_block_off_the_beam_is_not_on_it() {
        let (origin, dir, range) = (Vec2::ZERO, Vec2::X, 1000.0);
        assert!(on_beam(Vec2::new(500.0, 20.0), origin, dir, range));
        assert!(!on_beam(Vec2::new(500.0, 200.0), origin, dir, range), "swung off");
        assert!(!on_beam(Vec2::new(-50.0, 0.0), origin, dir, range), "behind the emitter");
        assert!(!on_beam(Vec2::new(1200.0, 0.0), origin, dir, range), "out of range");
    }
}

// ============================================================================
// ION DISRUPTOR — slow projectile, loses energy over distance, disables
// ============================================================================

/// Ion pulse projectile
#[derive(Component)]
pub struct IonPulse {
    pub initial_energy: f32,
    pub current_energy: f32,
    pub decay_rate: f32,       // Energy lost per second
    pub disable_duration: f32, // How long targets stay disabled
    pub owner: Entity,
}

/// Marks a module as temporarily disabled by ion
#[derive(Component)]
pub struct IonDisabled {
    pub timer: f32,
}

/// System: fire ion disruptor — slow pulse projectile
pub fn fire_ion_system(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    time: Res<Time>,
    fire_state: Res<FireGroupState>,
    selection: Res<TargetSelection>,
    ship_query: Query<(Entity, &ShipPhysics), With<Ship>>,
    mut weapon_query: Query<(
        &Module, Has<CrewStation>, Option<&ModuleEfficiency>, &mut Weapon, &mut WeaponCooldown,
        &GlobalTransform, &FireGroup, &WeaponMount, &ChildOf,
        Option<&crate::building::customization::tuning::WeaponTuning>,
        Option<&ModuleTemperature>,
    ), Without<DestroyedModule>>,
    target_query: Query<&Transform, Without<Ship>>,
    mut fired_events: MessageWriter<crate::events::WeaponFired>,
    mut commands: Commands,
) {
    let Ok((player_ship, ship_physics)) = ship_query.single() else { return };

    for (module, has_station, staffing, mut weapon, mut cooldown, global_transform, fire_group, mount, parent, tuning, temp) in weapon_query.iter_mut() {
        // A gun with nobody on it does not fire.
        if !crate::combat::weapon_is_crewed(has_station, staffing) {
            continue;
        }
        // Player ship only — see fire_weapons_system for why this matters.
        if parent.parent() != player_ship { continue; }
        if module.module_type != ModuleType::IonDisruptor { continue; }
        if !module.is_active { continue; }
        // Tick before the thermal gate — a frozen cooldown reads as
        // "recently fired" in generate_heat and locks the gun hot forever.
        cooldown.timer.tick(time.delta());
        if !cooldown.timer.is_finished() { continue; }

        // Thermal throttle — same gate the laser/kinetics use.
        if let Some(temp) = temp {
            if temp.current >= temp.max_temp * 0.95 { continue; }
        }

        let group_firing = fire_state.firing[fire_group.group as usize % 4];
        if !group_firing { continue; }

        let Some(target_entity) = selection.target else { continue; };
        let Ok(target_transform) = target_query.get(target_entity) else { continue; };

        if weapon.ammo <= 0 { continue; }

        let weapon_pos = global_transform.translation().truncate();
        let target_pos = target_transform.translation.truncate();
        let direction = (target_pos - weapon_pos).normalize_or_zero();

        // Fixed/broadside mounts can't swivel outside their arc.
        if !is_in_firing_arc(ship_physics.rotation, &module.rotation, mount, direction) {
            continue;
        }

        cooldown.timer.reset();
        weapon.ammo = weapon.ammo.saturating_sub(1);
        fired_events.write(crate::events::WeaponFired {
            weapon_type: module.module_type,
            position: weapon_pos,
            from_player: true,
        });

        let speed = 2500.0 * tuning.map(|t| t.velocity).unwrap_or(1.0); // x10 total from original — was crawling relative to its own range
        let angle = direction.y.atan2(direction.x);

        commands.spawn((
            (Sprite {
                    color: Color::srgba(0.5, 0.3, 0.9, 0.8),
                    custom_size: Some(Vec2::splat(14.0)),
                    ..default()
                }, Transform {
                    translation: Vec3::new(weapon_pos.x, weapon_pos.y, 0.5),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                }),
            IonPulse {
                initial_energy: 100.0,
                current_energy: 100.0,
                decay_rate: 15.0, // Loses 15 energy per second in flight
                disable_duration: 5.0,
                owner: player_ship,
            },
            Velocity(direction * speed),
            GravityAffected { mass: 0.2 },
            GravityForce::default(),
        ));

        // Muzzle flash
        super::spawn_muzzle_flash(&mut commands, &fx, weapon_pos + direction * 20.0, direction, 16.0, Color::srgb(0.5, 0.3, 0.9));
    }
}

/// System: move ion pulses, decay energy, check hits
pub fn update_ion_pulses(
    time: Res<Time>,
    mut commands: Commands,
    // Without<AiShip>: pulse_query's &mut Transform has no filter proving it
    // can never match an AI ship entity, and ai_ship_query below reads
    // &Transform on AiShip — same missing-canceling-pair issue as
    // fire_laser_system's weapon_query/ai_module_query conflict.
    mut pulse_query: Query<(Entity, &mut IonPulse, &mut Transform, &Velocity, &mut Sprite), Without<crate::ai_ship::components::AiShip>>,
    mut creature_query: Query<(Entity, &Transform, &mut Creature), Without<IonPulse>>,
    mut ai_ship_query: Query<
        (Entity, &Transform, &Children, &mut crate::combat::shields::ShipShield),
        With<crate::ai_ship::components::AiShip>,
    >,
    mut ai_module_query: Query<(&mut Module, &GlobalTransform), Without<DestroyedModule>>,
    mut ai_damage_events: MessageWriter<crate::events::AiShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let dt = time.delta_secs();

    'pulses: for (entity, mut pulse, mut transform, velocity, mut sprite) in pulse_query.iter_mut() {
        // Decay energy over time
        pulse.current_energy -= pulse.decay_rate * dt;

        // Visual: pulse dims as energy drops
        let energy_ratio = (pulse.current_energy / pulse.initial_energy).clamp(0.0, 1.0);
        sprite.color.set_alpha(energy_ratio * 0.8);

        // Shrink as energy drops
        let size = 14.0 * energy_ratio.max(0.3);
        sprite.custom_size = Some(Vec2::splat(size));

        // Dead pulse
        if pulse.current_energy <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }

        // Gravity is already in Velocity -- apply_gravity_to_velocity
        // integrates it for everything GravityAffected; adding it here too
        // bent pulses at twice the field.

        // Move
        transform.translation.x += velocity.0.x * dt;
        transform.translation.y += velocity.0.y * dt;

        // Check hits
        let pulse_pos = transform.translation.truncate();

        // AI ships — this pulse previously only ever checked creatures, so
        // it was physically incapable of disabling or damaging a ship.
        // Shield absorbs it outright (no disable); otherwise the nearest
        // module within range takes light damage AND gets IonDisabled —
        // the pulse's actual point (disabling enemy weapons/systems), not
        // just chip damage.
        for (ai_entity, ai_transform, children, mut shield) in ai_ship_query.iter_mut() {
            let center = shield.world_center(ai_transform);
            let dist_to_ship = pulse_pos.distance(center);

            if shield.is_up() && dist_to_ship < shield.radius {
                shield.absorb(5.0 * energy_ratio);
                spawn_hit_effect(&mut commands, pulse_pos, Color::srgb(0.5, 0.8, 1.0), 14.0);
                commands.entity(entity).try_despawn();
                continue 'pulses;
            }

            if dist_to_ship < shield.radius + 60.0 {
                let mut best_module: Option<(Entity, f32)> = None;
                for child in children.iter() {
                    if let Ok((_, gt)) = ai_module_query.get(child) {
                        let d = pulse_pos.distance(gt.translation().truncate());
                        if d < 45.0 && best_module.map(|(_, bd)| d < bd).unwrap_or(true) {
                            best_module = Some((child, d));
                        }
                    }
                }
                if let Some((module_entity, _)) = best_module {
                    if let Ok((mut module, gt)) = ai_module_query.get_mut(module_entity) {
                        let disable_time = pulse.disable_duration * energy_ratio;
                        module.health = (module.health - 5.0 * energy_ratio).max(0.0);
                        commands.entity(module_entity).try_insert(IonDisabled { timer: disable_time });

                        let hit_pos = gt.translation().truncate();
                        spawn_hit_effect(&mut commands, hit_pos, Color::srgb(0.6, 0.4, 1.0), 16.0);
                        spawn_floating_damage(&mut commands, hit_pos, 5.0 * energy_ratio, Color::srgb(0.5, 0.3, 0.9));
                        ai_damage_events.write(crate::events::AiShipDamaged {
                            target: ai_entity,
                            source: crate::events::DamageSource::Explosion,
                            amount: 0.0,
                            position: Some(hit_pos),
                            direction: None,
                            attacker: Some(pulse.owner),
                        });
                    }
                    commands.entity(entity).try_despawn();
                    continue 'pulses;
                }
            }
        }

        for (_creature_entity, creature_transform, mut creature) in creature_query.iter_mut() {
            if creature.health <= 0.0 { continue; }

            let dist = pulse_pos.distance(creature_transform.translation.truncate());
            if dist > 30.0 { continue; }

            // HIT — disable based on remaining energy
            let disable_strength = energy_ratio; // Full energy = full disable, half energy = half duration
            let disable_time = pulse.disable_duration * disable_strength;

            // Small damage
            creature.health -= 5.0 * energy_ratio;

            // Stun: reduce creature speed temporarily (via notification for now)
            notifications.write(ShowNotification {
                message: format!("Ion hit! Target disrupted for {:.1}s", disable_time),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });

            // Visual
            spawn_hit_effect(&mut commands, pulse_pos, Color::srgb(0.6, 0.4, 1.0), 16.0);
            spawn_floating_damage(&mut commands, pulse_pos, 5.0 * energy_ratio, Color::srgb(0.5, 0.3, 0.9));

            commands.entity(entity).try_despawn();
            break;
        }
    }
}

/// System: tick ion disabled timers, re-enable modules
pub fn update_ion_disabled(
    mut commands: Commands,
    time: Res<Time>,
    mut disabled_query: Query<(Entity, &mut IonDisabled, &mut Module)>,
) {
    let dt = time.delta_secs();

    for (entity, mut disabled, mut module) in disabled_query.iter_mut() {
        disabled.timer -= dt;
        module.is_active = false; // Keep disabled

        if disabled.timer <= 0.0 {
            module.is_active = true; // Re-enable
            commands.entity(entity).remove::<IonDisabled>();
        }
    }
}

// ============================================================================
// PLASMA CASTER — dumb-fire superheated bolt, area damage on impact
// ============================================================================

/// System: fire Plasma Caster — had no firing system at all before this;
/// the module was fully defined in the registry (damage, range, sprite,
/// cost) but nothing ever spawned a shot for it, so it was a completely
/// dead weapon. Modeled as a slow, non-homing AoE bolt (reuses
/// MissileProjectile purely for its blast-radius hit resolution, which
/// already handles AI ships correctly via check_missile_hits — target is
/// left None so move_missiles never applies guidance, it just flies
/// straight and detonates on contact).
pub fn fire_plasma_system(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    time: Res<Time>,
    fire_state: Res<FireGroupState>,
    selection: Res<TargetSelection>,
    ship_query: Query<(Entity, &ShipPhysics), With<Ship>>,
    mut weapon_query: Query<(
        Entity, &Module, Has<CrewStation>, Option<&ModuleEfficiency>, &mut Weapon, &mut WeaponCooldown,
        &GlobalTransform, &FireGroup, &WeaponMount, &ChildOf,
        Option<&crate::building::customization::tuning::WeaponTuning>,
        Option<&ModuleTemperature>,
    ), Without<DestroyedModule>>,
    target_query: Query<&Transform, Without<Ship>>,
    mut fired_events: MessageWriter<crate::events::WeaponFired>,
    mut commands: Commands,
) {
    let Ok((player_ship, ship_physics)) = ship_query.single() else { return };

    for (entity, module, has_station, staffing, mut weapon, mut cooldown, global_transform, fire_group, mount, parent, tuning, temp) in weapon_query.iter_mut() {
        // A gun with nobody on it does not fire.
        if !crate::combat::weapon_is_crewed(has_station, staffing) {
            continue;
        }
        // Player ship only — see fire_weapons_system for why this matters.
        if parent.parent() != player_ship { continue; }
        if module.module_type != ModuleType::PlasmaCaster { continue; }
        if !module.is_active { continue; }
        // Tick before the thermal gate — a frozen cooldown reads as
        // "recently fired" in generate_heat and locks the gun hot forever.
        cooldown.timer.tick(time.delta());
        if !cooldown.timer.is_finished() { continue; }

        // Thermal throttle — same gate the laser/kinetics use.
        if let Some(temp) = temp {
            if temp.current >= temp.max_temp * 0.95 { continue; }
        }

        let group_firing = fire_state.firing[fire_group.group as usize % 4];
        if !group_firing { continue; }

        let Some(target_entity) = selection.target else { continue; };
        let Ok(target_transform) = target_query.get(target_entity) else { continue; };

        if weapon.ammo <= 0 { continue; }

        let weapon_pos = global_transform.translation().truncate();
        let target_pos = target_transform.translation.truncate();
        let direction = (target_pos - weapon_pos).normalize_or_zero();

        // Fixed/broadside mounts can't swivel outside their arc.
        if !is_in_firing_arc(ship_physics.rotation, &module.rotation, mount, direction) {
            continue;
        }

        cooldown.timer.reset();
        weapon.ammo = weapon.ammo.saturating_sub(1);
        fired_events.write(crate::events::WeaponFired {
            weapon_type: module.module_type,
            position: weapon_pos,
            from_player: true,
        });

        let speed = 2200.0 * tuning.map(|t| t.velocity).unwrap_or(1.0); // heavy superheated bolt, slower than kinetic rounds
        let angle = direction.y.atan2(direction.x);

        commands.spawn((
            (Sprite {
                    color: Color::srgba(1.0, 0.5, 0.15, 0.9),
                    custom_size: Some(Vec2::splat(20.0)),
                    ..default()
                }, Transform {
                    translation: Vec3::new(weapon_pos.x, weapon_pos.y, 0.5),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                }),
            super::new_projectiles::MissileProjectile {
                age: 0.0,
                damage: weapon.damage,
                target: None, // dumb-fire — not a guided missile
                // No motor, no seeker, no eject phase: a plasma bolt launches
                // at full speed and flies straight. It borrows this component
                // only for blast-radius hit resolution.
                arm_distance: 30.0,
                blast_radius: 45.0, // "area damage on impact"
                owner: entity,
                launch_dir: direction,
                life: 4.0,
                prev_pos: weapon_pos,
                // Our own ship, so check_missile_hits treats the hull it
                // starts inside as the launcher's, not as an enemy's: with
                // `None` the player read as foreign and every bolt cooked
                // off in its own mount. launch_axis stays ZERO -- "never an
                // obstruction" -- since a bolt aimed at a target has no tube.
                owner_ship: Some(player_ship),
                launch_cell: module.grid_position,
                ..default()
            },
            Velocity(direction * speed),
            GravityAffected { mass: 1.0 },
            GravityForce::default(),
        ));

        super::spawn_muzzle_flash(&mut commands, &fx, weapon_pos + direction * 20.0, direction, 16.0, Color::srgb(1.0, 0.5, 0.1));
    }
}

// ============================================================================
// EMP MISSILE — disables everything in blast radius including friendly
// ============================================================================

/// EMP missile warhead data
#[derive(Component)]
pub struct EmpWarhead {
    pub emp_radius: f32,
    pub disable_duration: f32,
    pub affects_friendly: bool,
}

/// System: fire EMP missiles (uses missile bay system)
pub fn fire_emp_missiles(
    time: Res<Time>,
    fire_state: Res<FireGroupState>,
    selection: Res<TargetSelection>,
    ship_query: Query<(Entity, &ShipPhysics), With<Ship>>,
    mut weapon_query: Query<(
        Entity, &Module, Has<CrewStation>, Option<&ModuleEfficiency>, &mut Weapon, &mut WeaponCooldown,
        &GlobalTransform, &FireGroup, &WeaponMount, &ChildOf,
    ), Without<DestroyedModule>>,
    target_query: Query<&Transform, Without<Ship>>,
    machine_stats: Query<&crate::building::multiblock::components::MachineStats>,
    signatures: Query<&crate::diplomacy::signatures::Signature>,
    mut fuel_state: ResMut<FuelState>,
    mut commands: Commands,
) {
    let Ok((player_ship, ship_physics)) = ship_query.single() else { return };

    for (entity, module, has_station, staffing, mut weapon, mut cooldown, global_transform, fire_group, mount, parent) in weapon_query.iter_mut() {
        // A gun with nobody on it does not fire.
        if !crate::combat::weapon_is_crewed(has_station, staffing) {
            continue;
        }
        // Player ship only — see fire_weapons_system for why this matters.
        if parent.parent() != player_ship { continue; }
        if module.module_type != ModuleType::EMPPulse { continue; }
        if !module.is_active { continue; }

        cooldown.timer.tick(time.delta());
        if !cooldown.timer.is_finished() { continue; }

        let group_firing = fire_state.firing[fire_group.group as usize % 4];
        if !group_firing { continue; }

        let Some(target_entity) = selection.target else { continue; };
        let Ok(target_transform) = target_query.get(target_entity) else { continue; };

        if weapon.ammo <= 0 { continue; }

        let weapon_pos = global_transform.translation().truncate();
        let target_pos = target_transform.translation.truncate();
        let direction = (target_pos - weapon_pos).normalize_or_zero();

        // Fixed/broadside mounts can't swivel outside their arc.
        if !is_in_firing_arc(ship_physics.rotation, &module.rotation, mount, direction) {
            continue;
        }

        // Bay chain length determines EMP radius
        let bay_count = machine_stats.get(entity)
            .map(|s| s.barrel_count.max(1))
            .unwrap_or(1) as f32;

        // A signature pulse (diplomacy::signatures) reaches its share wider.
        let emp_radius = (100.0 + bay_count * 60.0) * signatures.get(entity).map_or(1.0, |s| s.0.output);

        // Fuel cost
        let fuel_cost = 8.0 * bay_count;
        if fuel_state.current_fuel < fuel_cost { continue; }
        fuel_state.current_fuel -= fuel_cost;

        cooldown.timer.reset();
        weapon.ammo = weapon.ammo.saturating_sub(1);

        let speed = 300.0;
        let angle = direction.y.atan2(direction.x);

        // Spawn EMP missile
        commands.spawn((
            (Sprite {
                    color: Color::srgb(0.4, 0.3, 0.8),
                    custom_size: Some(Vec2::new(14.0, 8.0)),
                    ..default()
                }, Transform {
                    translation: Vec3::new(weapon_pos.x, weapon_pos.y, 0.5),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                }),
            super::new_projectiles::MissileProjectile {
                age: 0.0,
                damage: 5.0, // Low physical damage
                target: Some(target_entity),
                // A short pop clear of the hull, then a modest burn — an EMP
                // round is a delivery vehicle, not a killer, so it flies
                // slower and turns wider than a real seeker.
                burn_fuel: 1.2,
                reserve_fuel: 10.0,
                thrust: 500.0,
                max_lateral: 1800.0,
                arm_distance: 100.0,
                blast_radius: emp_radius,
                owner: entity,
                eject_time: 0.25,
                launch_dir: direction,
                life: 7.0,
                prev_pos: weapon_pos,
                // Same as the plasma bolt: our hull is the launcher's own.
                owner_ship: Some(player_ship),
                launch_cell: module.grid_position,
                ..default()
            },
            EmpWarhead {
                emp_radius,
                disable_duration: 6.0,
                affects_friendly: true,
            },
            Velocity(direction * speed),
            GravityAffected { mass: 3.0 },
            GravityForce::default(),
        ));
    }
}

/// System: when EMP missile hits, disable everything in radius.
/// Previously only ever detonated near a CREATURE and, even then, only ever
/// disabled the PLAYER's own modules (gated behind the player being in the
/// blast too) — meaning it could never actually do anything to the AI ship
/// it was fired at. Now detonates near its actual guided target (any
/// entity — creature or AI ship, via GlobalTransform) and disables every
/// module in radius regardless of owner: the player's own (half duration,
/// "affects_friendly") and any AI ship's (full duration).
pub fn emp_detonation(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut commands: Commands,
    missile_query: Query<(Entity, &Transform, &EmpWarhead, &super::new_projectiles::MissileProjectile)>,
    mut module_query: Query<(Entity, &Module, &GlobalTransform, &ChildOf), Without<DestroyedModule>>,
    target_position_query: Query<&GlobalTransform>,
    ship_query: Query<Entity, With<Ship>>,
    // MissileProjectile.owner is the launching WEAPON entity, not a ship —
    // resolve to the actual ship root the same way check_missile_hits does.
    owner_parent_query: Query<&ChildOf>,
    mut ai_damage_events: MessageWriter<crate::events::AiShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
    // What a ship-bound EMP actually reaches first: the bubble, or the hull.
    ai_ships: Query<
        (Entity, &Transform, &crate::combat::shields::ShipShield),
        With<crate::ai_ship::components::AiShip>,
    >,
    hulls: Query<(&GlobalTransform, &crate::building::ShipGrid)>,
) {
    let player_ship = ship_query.single().ok();

    for (missile_entity, missile_transform, emp, missile) in missile_query.iter() {
        let owner_ship = owner_parent_query.get(missile.owner).ok().map(|p| p.parent());
        if !missile.armed { continue; }
        // Spent this frame: move_missiles already queued its despawn.
        if missile.life <= 0.0 { continue; }

        let missile_pos = missile_transform.translation.truncate();

        // Detonate on whatever it meets first. Closing to within 40 of the
        // target's ROOT was the only fuse, and a ship's root sits inside its
        // own hull behind the shield -- so against a ship the round never
        // got there, and check_missile_hits used to pop it as a plain
        // warhead on the way in. Now: a live shield stops it at the bubble
        // (so a raised shield still buys distance from the burst), a dropped
        // one lets it reach the plating. The root test stays for creatures.
        let near_target = missile.target
            .and_then(|t| target_position_query.get(t).ok())
            .is_some_and(|gt| missile_pos.distance(gt.translation().truncate()) < 40.0);
        let hit_ship = ai_ships.iter().any(|(ai_entity, ai_transform, shield)| {
            if Some(ai_entity) == owner_ship { return false; }
            let center = shield.world_center(ai_transform);
            let on_bubble = shield.is_up()
                && missile_pos.distance(center) < shield.radius
                && shield.covers_arc(missile_pos - center);
            on_bubble || hulls.get(ai_entity).is_ok_and(|(gt, grid)| {
                let here = crate::building::world_to_cell(gt, missile_pos);
                grid.get(IVec2::new(here.x.round() as i32, here.y.round() as i32)).is_some()
            })
        });

        if !(near_target || hit_ship) { continue; }

        // EMP DETONATION!
        // The comment has always said "purple-blue expanding ring"; it was a
        // static opaque square up to 400 units across. spawn_explosion's shock
        // ring reaches radius * 3.4, so it now actually expands -- and stays
        // purple, which only works because `color` is wired up.
        super::spawn_explosion(
            &mut commands,
            &fx,
            missile_pos,
            emp.emp_radius / 3.0,
            Color::srgb(0.45, 0.35, 0.95),
        );

        let mut hit_player = false;
        for (module_entity, _module, module_gt, parent) in module_query.iter_mut() {
            let module_pos = module_gt.translation().truncate();
            if module_pos.distance(missile_pos) >= emp.emp_radius { continue; }

            let is_player_module = Some(parent.parent()) == player_ship;
            let duration = if is_player_module {
                if !emp.affects_friendly { continue; }
                hit_player = true;
                emp.disable_duration * 0.5 // Half duration for friendly
            } else {
                ai_damage_events.write(crate::events::AiShipDamaged {
                    target: parent.parent(),
                    source: crate::events::DamageSource::Explosion,
                    amount: 0.0,
                    position: Some(module_pos),
                    direction: None,
                    attacker: owner_ship,
                });
                emp.disable_duration
            };

            commands.entity(module_entity).try_insert(IonDisabled { timer: duration });
        }

        if hit_player {
            notifications.write(ShowNotification {
                message: "EMP BLAST! Your systems are disrupted!".into(),
                notification_type: NotificationType::Danger,
                duration: 3.0,
            });
        }

        notifications.write(ShowNotification {
            message: format!("EMP detonated! {:.0}m radius disruption!", emp.emp_radius),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });

        commands.entity(missile_entity).try_despawn();
    }
}
