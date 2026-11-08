use bevy::prelude::*;
use rand::Rng;
use crate::states::{GameState, SpatialSet};
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::sprite_map;
use crate::ai_ship::components::AiShip;

mod behaviors;
pub mod food_chain;
pub mod ecosystem;
pub mod corpse;
pub mod creature_combat;
pub mod noise_trail;
pub mod territory;
pub mod migration;


pub struct CreaturePlugin;

impl Plugin for CreaturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EcosystemState>()
            .init_resource::<EcosystemConfig>()
            .init_resource::<noise_trail::NoiseTrailTimer>();

        app.add_systems(
            Update,
            (
                spawn_creatures,
                behaviors::gravity_aware_wandering,
                ecosystem::update_creature_needs,
                ecosystem::ecosystem_ai_decisions,
                migration::follow_migration_path,
                creature_movement,
                creature_attack_system,
                creature_combat::creature_vs_creature_combat,
                check_creature_detection,
                parasite_attach_system,
            )
                .chain()
                .after(SpatialSet::Update)
                .run_if(in_state(GameState::Exploring)),
        )
        .add_systems(
            Update,
            (
                ecosystem::feeding_system,
                ecosystem::starvation_system,
                ecosystem::reproduction_system,
                ecosystem::population_balance,
                ecosystem::update_creature_memory,
                corpse::spawn_corpse_on_death,
                corpse::decay_corpses,
                noise_trail::emit_noise_trail,
                noise_trail::decay_noise_trails,
                territory::update_territories,
                migration::check_migration,
                cleanup_distant_creatures,
                animate_creature_sprites,
            )
                .run_if(in_state(GameState::Exploring)),
        );
    }
}

/// Spawns creatures based on distance from station and biome.
#[allow(unreachable_code, unused_variables)]
fn spawn_creatures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    time: Res<Time>,
    config: Res<GameConfig>,
    ship_state: Res<DepthState>,
    world_state: Res<WorldState>,
    ship_query: Query<&Transform, With<Ship>>,
    creature_query: Query<&Creature>,
    eco_state: Res<EcosystemState>,
    eco_config: Res<EcosystemConfig>,
    mut spawn_timer: Local<f32>,
    session_timer: Res<ExploringSessionTimer>,
) {
    // Creatures disabled per playtest feedback — "more annoying than scary."
    // Leaving the rest of the module (movement/attack/ecosystem systems)
    // wired up: with nothing ever spawning they just run over empty queries.
    // Remove this line to bring creatures back.
    return;

    // Don't spawn any hostile creatures in the first 30 seconds
    let hostile_warmup = 30.0;

    *spawn_timer += time.delta_secs();

    let total_creatures: u32 = eco_state.population_counts.values().sum();
    if total_creatures >= eco_config.max_total_creatures {
        return;
    }

    let depth = ship_state.current_depth.abs();
    let max_creatures = (8 + (depth / 300.0) as usize).min(eco_config.max_total_creatures as usize);
    if creature_query.iter().count() >= max_creatures {
        return;
    }

    let spawn_interval = (2.0 - (depth / 1500.0).min(1.0)) / config.creature_spawn_rate;
    if *spawn_timer >= spawn_interval {
        *spawn_timer = 0.0;

        // Weighted selection based on biome
        let weights = crate::world::biome_creature_weights(world_state.current_biome);
        let creature_type = if !weights.is_empty() {
            let total: f32 = weights.iter().map(|(_, w)| w).sum();
            let mut roll = rand::thread_rng().gen::<f32>() * total;
            let mut selected = weights[0].0;
            for (name, weight) in &weights {
                roll -= weight;
                if roll <= 0.0 {
                    selected = name;
                    break;
                }
            }
            match selected {
                "void_drifter" => CreatureType::VoidDrifter,
                "stalker" => CreatureType::Stalker,
                "leviathan" => CreatureType::Leviathan,
                "parasite_swarm" => CreatureType::ParasiteSwarm,
                _ => CreatureType::VoidDrifter,
            }
        } else {
            // Fallback: distance-based
            match depth {
                d if d < 300.0 => CreatureType::VoidDrifter,
                d if d < 800.0 => CreatureType::Stalker,
                _ => CreatureType::ParasiteSwarm,
            }
        };

        // Don't spawn hostile creatures during warmup
        if session_timer.elapsed < hostile_warmup && creature_type != CreatureType::VoidDrifter {
            return;
        }

        // Leviathans only spawn in deep space (1500+ distance)
        let creature_type = if creature_type == CreatureType::Leviathan && depth < 1500.0 {
            CreatureType::Stalker
        } else {
            creature_type
        };

        // Check per-type population cap
        let current_count = eco_state
            .population_counts
            .get(&creature_type)
            .copied()
            .unwrap_or(0);
        let type_cap = eco_config
            .per_type_caps
            .get(&creature_type)
            .copied()
            .unwrap_or(5);
        if current_count >= type_cap {
            return;
        }

        let ship_pos = ship_query
            .single()
            .map(|t| t.translation.truncate())
            .unwrap_or(Vec2::ZERO);

        let mut rng = rand::thread_rng();
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let dist = rng.gen_range(600.0..1000.0);
        let spawn_pos = ship_pos + Vec2::new(angle.cos() * dist, angle.sin() * dist);

        spawn_creature(&mut commands, &asset_server, &mut texture_atlas_layouts, creature_type, spawn_pos);

        // ParasiteSwarm spawns in clusters
        if creature_type == CreatureType::ParasiteSwarm {
            let group_size = rng.gen_range(3..8);
            for _ in 0..group_size {
                let offset = Vec2::new(rng.gen_range(-40.0..40.0), rng.gen_range(-40.0..40.0));
                spawn_creature(&mut commands, &asset_server, &mut texture_atlas_layouts, CreatureType::ParasiteSwarm, spawn_pos + offset);
            }
        }
    }
}

/// Spawn a creature entity with animated sprite sheet
fn spawn_creature(
    commands: &mut Commands,
    asset_server: &AssetServer,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
    creature_type: CreatureType,
    position: Vec2,
) {
    let mut rng = rand::thread_rng();

    let depth = position.y.abs();
    let depth_factor = 1.0 + (depth / 1000.0).min(2.0);
    let speed_factor = 1.0 + (depth / 2000.0).min(0.5);
    let range_factor = 1.0 + (depth / 3000.0).min(0.3);

    // (frame_w, frame_h, display_w, display_h, health, damage, speed, detection_range)
    let (frame_w, frame_h, base_w, base_h, health, damage, speed, range) = match creature_type {
        CreatureType::VoidDrifter =>     ( 64,  64,  45.0,  22.0,  10.0,  0.0,  15.0, 100.0),
        CreatureType::Stalker =>         ( 64,  64, 150.0,  54.0,  50.0, 15.0,  80.0, 350.0),
        CreatureType::Leviathan =>       (128,  64, 660.0, 180.0, 500.0,100.0,  40.0, 500.0),
        CreatureType::ParasiteSwarm =>   ( 64,  64,  30.0,  20.0,   8.0,  2.0,  70.0, 200.0),
    };

    let size_scale = rng.gen_range(0.75..1.35_f32);
    let w = base_w * size_scale;
    let h = base_h * size_scale;
    let health_scale = size_scale.powf(1.5);
    let speed_scale = 1.0 / size_scale.sqrt();

    let attack_time = match creature_type {
        CreatureType::Leviathan => 3.0,
        CreatureType::ParasiteSwarm => 0.5,
        CreatureType::Stalker => 1.5,
        CreatureType::VoidDrifter => 5.0,
    };

    let scaled_health = health * depth_factor * health_scale;
    let scaled_damage = damage * depth_factor * size_scale;
    let scaled_speed = speed * speed_factor * speed_scale;
    let scaled_range = range * range_factor;

    let eco_stats = food_chain::creature_ecosystem_stats(creature_type);
    let role = food_chain::food_chain_role(creature_type);

    let swim_frames = 4;
    let attack_frames = if creature_type == CreatureType::VoidDrifter { 0 } else { 2 };
    let total_frames = swim_frames + attack_frames;
    let anim_speed = match creature_type {
        CreatureType::ParasiteSwarm => 0.1,
        CreatureType::Leviathan => 0.25,
        CreatureType::VoidDrifter => 0.3,
        _ => 0.15,
    };

    let texture: Handle<Image> = asset_server.load(sprite_map::creature_sprite_path(creature_type));
    let layout = TextureAtlasLayout::from_grid(
        UVec2::new(frame_w, frame_h),
        total_frames,
        1,
        None,
        None,
    );
    let layout_handle = texture_atlas_layouts.add(layout);

    let mut entity_commands = commands.spawn((
        (Sprite {
                image: texture,
                custom_size: Some(Vec2::new(w, h)),
                texture_atlas: Some(TextureAtlas {
                    layout: layout_handle,
                    index: 0,
                }),
                ..default()
            }, Transform::from_xyz(position.x, position.y, 0.0)),
        Creature {
            creature_type,
            health: scaled_health,
            max_health: scaled_health,
            damage: scaled_damage,
            speed: scaled_speed,
            detection_range: scaled_range,
            attack_cooldown: 0.0,
            food_value: eco_stats.food_value * size_scale,
        },
        CreatureAI {
            state: if creature_type == CreatureType::VoidDrifter {
                CreatureAIState::Wandering
            } else {
                CreatureAIState::Wandering
            },
            target: None,
            home_position: position,
            wander_radius: match creature_type {
                CreatureType::VoidDrifter => 300.0,
                CreatureType::Leviathan => 500.0,
                _ => 200.0,
            },
        },
        AttackCooldown {
            timer: Timer::from_seconds(attack_time, TimerMode::Once),
        },
        Velocity(Vec2::ZERO),
        CreatureAnimation {
            timer: Timer::from_seconds(anim_speed, TimerMode::Repeating),
            swim_frames: swim_frames as usize,
            attack_frames: attack_frames as usize,
            total_frames: total_frames as usize,
            current_frame: 0,
        },
        CreatureNeeds {
            hunger: 20.0 + rng.gen_range(0.0..30.0),
            energy: 80.0 + rng.gen_range(0.0..20.0),
            hunger_rate: eco_stats.hunger_rate,
            energy_drain_rate: eco_stats.energy_drain_rate,
        },
        role,
        CreatureMemory::default(),
        // Gravity interaction — creatures get pulled by celestial bodies
        crate::celestial::components::GravityAffected {
            mass: match creature_type {
                CreatureType::VoidDrifter => 10.0,     // Light, drifts easily
                CreatureType::Stalker => 200.0,        // Medium, resists somewhat
                CreatureType::Leviathan => 5000.0,     // Massive, barely affected
                CreatureType::ParasiteSwarm => 5.0,    // Tiny, blown around
            },
        },
        crate::celestial::components::GravityForce::default(),
    ));

    if eco_stats.is_territorial {
        entity_commands.insert(Territory {
            center: position,
            radius: eco_stats.territory_radius,
            aggression: eco_stats.territory_aggression,
        });
    }

    if eco_stats.can_reproduce {
        entity_commands.insert(Reproductive {
            gestation_timer: 0.0,
            gestation_duration: eco_stats.gestation_duration,
            offspring_count: eco_stats.offspring_count,
            satiation_threshold: eco_stats.satiation_threshold,
        });
    }
}

/// Moves creatures based on AI state
fn creature_movement(
    time: Res<Time>,
    ship_query: Query<&Transform, With<Ship>>,
    mut creature_query: Query<(Entity, &mut Transform, &Creature, &CreatureAI, &mut Velocity), Without<Ship>>,
    corpse_query: Query<&Transform, (With<Corpse>, Without<Creature>, Without<Ship>)>,
    ai_ship_positions: Query<&Transform, (With<AiShip>, Without<Creature>, Without<Ship>)>,
) {
    let ship_pos = ship_query
        .single()
        .map(|t| t.translation.truncate())
        .unwrap_or(Vec2::ZERO);

    let creature_pos_map: std::collections::HashMap<Entity, Vec2> = creature_query
        .iter()
        .map(|(e, t, _, _, _)| (e, t.translation.truncate()))
        .collect();

    for (_entity, mut transform, creature, ai, mut velocity) in creature_query.iter_mut() {
        let pos = transform.translation.truncate();

        let target_pos = if let Some(ref eco_target) = ai.target {
            match eco_target {
                EcoTarget::Ship(_) => ship_pos,
                EcoTarget::AiShip(e) => {
                    ai_ship_positions.get(*e).ok()
                        .map(|t| t.translation.truncate())
                        .unwrap_or(ship_pos)
                }
                EcoTarget::Creature(e) => {
                    creature_pos_map.get(e).copied().unwrap_or(ship_pos)
                }
                EcoTarget::Corpse(e) => {
                    corpse_query.get(*e).ok()
                        .map(|t| t.translation.truncate())
                        .unwrap_or(ship_pos)
                }
                EcoTarget::Position(p) => *p,
            }
        } else {
            ai.home_position
        };

        // Wounded rage for combat creatures
        let health_pct = if creature.max_health > 0.0 { creature.health / creature.max_health } else { 1.0 };
        let rage_mult = if health_pct < 0.4 && creature.creature_type != CreatureType::VoidDrifter {
            1.5
        } else {
            1.0
        };

        let target_velocity = match ai.state {
            CreatureAIState::Hunting => {
                let direction = (target_pos - pos).normalize_or_zero();
                match creature.creature_type {
                    // Stalker: weaves side to side while approaching
                    CreatureType::Stalker => {
                        let perp = Vec2::new(-direction.y, direction.x);
                        let weave = (time.elapsed_secs() * 3.0).sin() * 0.4;
                        (direction + perp * weave).normalize_or_zero() * creature.speed * rage_mult
                    }
                    // Leviathan: slow sweeping approach, enraged when hurt
                    CreatureType::Leviathan => {
                        if health_pct < 0.5 {
                            direction * creature.speed * 1.8 * rage_mult
                        } else {
                            let perp = Vec2::new(-direction.y, direction.x);
                            let sweep = (time.elapsed_secs() * 1.2).sin() * 0.3;
                            (direction + perp * sweep).normalize_or_zero() * creature.speed * rage_mult
                        }
                    }
                    _ => direction * creature.speed,
                }
            }
            CreatureAIState::Attacking => {
                let direction = (target_pos - pos).normalize_or_zero();
                match creature.creature_type {
                    CreatureType::Stalker => direction * creature.speed * 2.5,
                    CreatureType::ParasiteSwarm => direction * creature.speed * 1.3,
                    CreatureType::Leviathan => {
                        if health_pct < 0.25 {
                            direction * creature.speed * 2.5 * rage_mult
                        } else {
                            direction * creature.speed * 1.8 * rage_mult
                        }
                    }
                    _ => direction * creature.speed,
                }
            }
            CreatureAIState::Fleeing => {
                let direction = (pos - target_pos).normalize_or_zero();
                direction * creature.speed * 1.5
            }
            CreatureAIState::Observing => {
                let to_target = target_pos - pos;
                let perp = Vec2::new(-to_target.y, to_target.x).normalize_or_zero();
                perp * creature.speed * 0.5
            }
            CreatureAIState::Feeding => {
                let direction = (target_pos - pos).normalize_or_zero();
                let dist = pos.distance(target_pos);
                if dist > 30.0 { direction * creature.speed * 0.3 } else { Vec2::ZERO }
            }
            CreatureAIState::Patrolling => {
                let to_home = ai.home_position - pos;
                let dist = to_home.length();
                let perp = Vec2::new(-to_home.y, to_home.x).normalize_or_zero();
                if dist > 200.0 {
                    (to_home.normalize_or_zero() * 0.5 + perp * 0.5).normalize_or_zero() * creature.speed * 0.4
                } else {
                    perp * creature.speed * 0.4
                }
            }
            CreatureAIState::Migrating => {
                let direction = (target_pos - pos).normalize_or_zero();
                direction * creature.speed * 0.7
            }
            CreatureAIState::Investigating => {
                let direction = (target_pos - pos).normalize_or_zero();
                direction * creature.speed * 0.6
            }
            CreatureAIState::Wandering => {
                let direction = (ai.home_position - pos).normalize_or_zero();
                direction * creature.speed * 0.3
            }
            CreatureAIState::Idle => Vec2::ZERO,
        };

        let lerp_factor = match creature.creature_type {
            CreatureType::Stalker if matches!(ai.state, CreatureAIState::Attacking) => 8.0,
            CreatureType::Leviathan => 0.8,
            _ => 2.0,
        };

        velocity.0 = velocity.0.lerp(target_velocity, lerp_factor * time.delta_secs());
        transform.translation.x += velocity.0.x * time.delta_secs();
        transform.translation.y += velocity.0.y * time.delta_secs();

        if velocity.0.length_squared() > 1.0 {
            let angle = velocity.0.y.atan2(velocity.0.x);
            transform.rotation = Quat::from_rotation_z(angle);
        }
    }
}

/// Creature attack system — damages player ship or AI ships
fn creature_attack_system(
    time: Res<Time>,
    ship_query: Query<(Entity, &Transform), With<Ship>>,
    mut creature_query: Query<(Entity, &Transform, &Creature, &CreatureAI, &mut AttackCooldown), Without<Ship>>,
    ai_ship_query: Query<(Entity, &Transform), (With<AiShip>, Without<Ship>)>,
    mut damage_events: MessageWriter<ShipDamaged>,
    mut ai_damage_events: MessageWriter<AiShipDamaged>,
    mut attacking_events: MessageWriter<CreatureAttacking>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let Ok((ship_entity, ship_transform)) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    for (creature_entity, transform, creature, ai, mut cooldown) in creature_query.iter_mut() {
        if !matches!(ai.state, CreatureAIState::Attacking) {
            continue;
        }

        cooldown.timer.tick(time.delta());
        if !cooldown.timer.is_finished() {
            continue;
        }

        let creature_pos = transform.translation.truncate();

        let health_pct = if creature.max_health > 0.0 { creature.health / creature.max_health } else { 1.0 };
        let rage_damage = if health_pct < 0.4 { creature.damage * 1.25 } else { creature.damage };

        if matches!(ai.target, Some(EcoTarget::Ship(_))) {
            let dist = creature_pos.distance(ship_pos);
            if dist > 100.0 { continue; }

            cooldown.timer.reset();

            attacking_events.write(CreatureAttacking {
                creature: creature_entity,
                target: ship_entity,
            });

            damage_events.write(ShipDamaged {
                source: DamageSource::Creature(creature_entity),
                amount: rage_damage,
                position: Some(creature_pos),
                direction: Some((creature_pos - ship_pos).normalize_or_zero()),
            });

            notifications.write(ShowNotification {
                message: format!("{:?} attacks! ({:.0} damage)", creature.creature_type, rage_damage),
                notification_type: NotificationType::Danger,
                duration: 2.0,
            });
        } else if let Some(EcoTarget::AiShip(ai_ship_entity)) = ai.target {
            if let Ok((ai_e, ai_transform)) = ai_ship_query.get(ai_ship_entity) {
                let ai_pos = ai_transform.translation.truncate();
                let dist = creature_pos.distance(ai_pos);
                if dist > 100.0 { continue; }

                cooldown.timer.reset();

                ai_damage_events.write(AiShipDamaged {
                    target: ai_e,
                    source: DamageSource::Creature(Entity::PLACEHOLDER),
                    amount: rage_damage,
                    position: Some(creature_pos),
                    direction: Some((creature_pos - ai_pos).normalize_or_zero()),
                    attacker: None, // creatures aren't part of the ship-target system
                });
            }
        }
    }
}

/// ParasiteSwarm: attaches to hull and slowly drains systems
fn parasite_attach_system(
    time: Res<Time>,
    parasite_query: Query<(&Transform, &Creature, &CreatureAI), Without<Ship>>,
    ship_query: Query<&Transform, With<Ship>>,
    mut damage_events: MessageWriter<ShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
    mut drain_timer: Local<f32>,
) {
    *drain_timer += time.delta_secs();
    if *drain_timer < 1.0 {
        return;
    }

    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    let mut attached_count = 0u32;
    for (transform, creature, ai) in parasite_query.iter() {
        if creature.creature_type != CreatureType::ParasiteSwarm { continue; }
        if !matches!(ai.state, CreatureAIState::Attacking) { continue; }
        if !matches!(ai.target, Some(EcoTarget::Ship(_))) { continue; }

        let dist = transform.translation.truncate().distance(ship_pos);
        if dist < 80.0 {
            attached_count += 1;
        }
    }

    if attached_count > 0 {
        *drain_timer = 0.0;
        let drain_damage = attached_count as f32 * 0.5;
        damage_events.write(ShipDamaged {
            source: DamageSource::Creature(Entity::PLACEHOLDER),
            amount: drain_damage,
            position: None,
            direction: None,
        });

        if attached_count >= 3 {
            notifications.write(ShowNotification {
                message: format!("Parasite swarm draining hull! ({} attached)", attached_count),
                notification_type: NotificationType::Danger,
                duration: 2.0,
            });
        }
    }
}

/// Checks if creatures detect the ship
fn check_creature_detection(
    ship_query: Query<&Transform, With<Ship>>,
    creature_query: Query<(Entity, &Transform, &Creature, &CreatureAI)>,
    mut spotted_events: MessageWriter<CreatureSpotted>,
    mut detected: Local<std::collections::HashSet<Entity>>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };

    for (entity, transform, creature, ai) in creature_query.iter() {
        let distance = transform.translation.truncate()
            .distance(ship_transform.translation.truncate());

        if matches!(ai.state, CreatureAIState::Hunting | CreatureAIState::Attacking) && !detected.contains(&entity) {
            detected.insert(entity);
            spotted_events.write(CreatureSpotted {
                creature: entity,
                creature_type: creature.creature_type,
                distance,
            });
        } else if matches!(ai.state, CreatureAIState::Wandering | CreatureAIState::Idle) {
            detected.remove(&entity);
        }
    }
}

/// Despawn creatures that are very far from the ship
fn cleanup_distant_creatures(
    mut commands: Commands,
    ship_query: Query<&Transform, With<Ship>>,
    creature_query: Query<(Entity, &Transform, &Creature, &CreatureAI)>,
    mut eco_state: ResMut<EcosystemState>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    for (entity, transform, creature, ai) in creature_query.iter() {
        let dist = transform.translation.truncate().distance(ship_pos);
        if dist > 2500.0 {
            if ai.state == CreatureAIState::Migrating { continue; }
            if let Some(count) = eco_state.population_counts.get_mut(&creature.creature_type) {
                *count = count.saturating_sub(1);
            }
            commands.entity(entity).despawn();
        }
    }
}

/// Animate creature sprite sheets
fn animate_creature_sprites(
    time: Res<Time>,
    mut query: Query<(
        &mut CreatureAnimation,
        &mut Sprite,
        Option<&CreatureAI>,
    )>,
) {
    for (mut anim, mut sprite, ai) in query.iter_mut() {
        anim.timer.tick(time.delta());
        if !anim.timer.just_finished() { continue; }

        let attacking = ai
            .map(|a| matches!(a.state, CreatureAIState::Attacking))
            .unwrap_or(false);

        if attacking && anim.attack_frames > 0 {
            let attack_start = anim.swim_frames;
            let attack_end = anim.swim_frames + anim.attack_frames;
            anim.current_frame = if anim.current_frame >= attack_start && anim.current_frame < attack_end - 1 {
                anim.current_frame + 1
            } else {
                attack_start
            };
        } else {
            anim.current_frame = (anim.current_frame + 1) % anim.swim_frames;
        }

        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = anim.current_frame;
        }
    }
}
