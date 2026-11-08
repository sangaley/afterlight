use bevy::prelude::*;
use crate::components::*;
use crate::events::*;
use crate::building::rooms::RoomMap;

// ============================================================================
// 1. WEAPON HEAT VISUAL — barrel glows orange→red as it heats up
// ============================================================================

/// System: tint weapon module sprites based on their temperature
pub fn weapon_heat_visual(
    weapon_query: Query<(&Module, &ModuleTemperature, &Children), (With<Weapon>, Without<DestroyedModule>)>,
    mut sprite_query: Query<&mut Sprite>,
) {
    for (_module, temp, children) in weapon_query.iter() {
        let heat_ratio = (temp.current / temp.max_temp).clamp(0.0, 1.0);

        if heat_ratio < 0.3 { continue; } // Don't tint cool weapons

        // Calculate heat tint: orange at 50%, bright red at 100%
        let heat_r = 0.3 + heat_ratio * 0.7;
        let heat_g = (0.4 * (1.0 - heat_ratio)).max(0.0);
        let _heat_b = 0.0;
        let heat_alpha = (heat_ratio - 0.3) * 0.5; // Starts showing at 30%

        // Tint all child sprites (the block visual layers)
        for child in children.iter() {
            if let Ok(mut sprite) = sprite_query.get_mut(child) {
                // Blend heat color onto existing color
                let base = sprite.color;
                sprite.color = Color::srgb(
                    (base.to_srgba().red + heat_r * heat_alpha).min(1.0),
                    (base.to_srgba().green * (1.0 - heat_alpha) + heat_g * heat_alpha).max(0.0),
                    (base.to_srgba().blue * (1.0 - heat_alpha)).max(0.0),
                );
            }
        }
    }
}

// ============================================================================
// 2. DAMAGE DIRECTION INDICATOR — arrow pointing toward damage source
// ============================================================================

/// Component for a damage direction arrow on the HUD
#[derive(Component)]
pub struct DamageDirectionArrow {
    pub direction: Vec2,
    pub timer: f32,
    pub max_time: f32,
}

/// System: spawn damage direction arrows when ship takes damage
pub fn spawn_damage_indicators(
    mut commands: Commands,
    mut damage_events: MessageReader<ShipDamaged>,
    ship_query: Query<&Transform, With<Ship>>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    for event in damage_events.read() {
        // Get damage direction
        let direction = if let Some(dir) = event.direction {
            dir
        } else if let Some(pos) = event.position {
            (pos - ship_pos).normalize_or_zero()
        } else {
            continue; // No direction info
        };

        if direction.length_squared() < 0.01 { continue; }

        // Spawn arrow indicator at edge of screen toward damage source
        let angle = direction.y.atan2(direction.x);
        let arrow_dist = 150.0; // Distance from center of screen
        let arrow_pos = Vec2::new(angle.cos() * arrow_dist, angle.sin() * arrow_dist);

        commands.spawn((
            (Sprite {
                    color: Color::srgba(1.0, 0.2, 0.1, 0.8),
                    custom_size: Some(Vec2::new(20.0, 8.0)),
                    ..default()
                }, Transform {
                    translation: Vec3::new(
                        ship_pos.x + arrow_pos.x,
                        ship_pos.y + arrow_pos.y,
                        2.0,
                    ),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                }),
            DamageDirectionArrow {
                direction,
                timer: 2.0,
                max_time: 2.0,
            },
        ));
    }
}

/// System: update and fade damage direction arrows
pub fn update_damage_indicators(
    mut commands: Commands,
    time: Res<Time>,
    ship_query: Query<&Transform, With<Ship>>,
    mut arrow_query: Query<(Entity, &mut DamageDirectionArrow, &mut Transform, &mut Sprite), Without<Ship>>,
) {
    let dt = time.delta_secs();
    let ship_pos = ship_query.single()
        .map(|t| t.translation.truncate())
        .unwrap_or(Vec2::ZERO);

    for (entity, mut arrow, mut transform, mut sprite) in arrow_query.iter_mut() {
        arrow.timer -= dt;

        if arrow.timer <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }

        // Fade out
        let alpha = (arrow.timer / arrow.max_time).clamp(0.0, 1.0) * 0.8;
        sprite.color.set_alpha(alpha);

        // Keep arrow at fixed distance from ship
        let angle = arrow.direction.y.atan2(arrow.direction.x);
        let arrow_dist = 150.0;
        transform.translation.x = ship_pos.x + angle.cos() * arrow_dist;
        transform.translation.y = ship_pos.y + angle.sin() * arrow_dist;
    }
}

// ============================================================================
// 3. LEVIATHAN WEAK POINT — visible spot, 3x damage when hit
// ============================================================================

/// Component marking a creature's weak point
#[derive(Component)]
pub struct WeakPoint {
    pub offset_angle: f32,  // Angle offset from creature facing direction
    pub radius: f32,        // How big the weak spot is
    pub damage_mult: f32,   // Damage multiplier when hit
}

/// Visual marker for the weak point
#[derive(Component)]
pub struct WeakPointVisual;

/// System: attach weak points to Leviathans when they spawn
pub fn attach_weak_points(
    mut commands: Commands,
    leviathan_query: Query<(Entity, &Creature), Without<WeakPoint>>,
) {
    for (entity, creature) in leviathan_query.iter() {
        if creature.creature_type != CreatureType::Leviathan { continue; }

        // Weak point on the underside
        commands.entity(entity).insert(WeakPoint {
            offset_angle: std::f32::consts::PI, // Opposite of facing direction
            radius: 40.0,
            damage_mult: 3.0,
        });

        // Visual indicator — glowing spot
        let visual = commands.spawn((
            (Sprite {
                    color: Color::srgba(1.0, 0.4, 0.2, 0.6),
                    custom_size: Some(Vec2::splat(30.0)),
                    ..default()
                }, Transform::from_xyz(0.0, -60.0, 0.1)),
            WeakPointVisual,
        )).id();

        commands.entity(entity).add_child(visual);
    }
}

/// System: update weak point visual position as Leviathan moves
pub fn update_weak_point_visuals(
    leviathan_query: Query<(&Velocity, &WeakPoint, &Children), With<Creature>>,
    mut visual_query: Query<&mut Transform, With<WeakPointVisual>>,
) {
    for (velocity, weak_point, children) in leviathan_query.iter() {
        // Weak point rotates based on creature facing direction
        let facing_angle = if velocity.0.length_squared() > 1.0 {
            velocity.0.y.atan2(velocity.0.x)
        } else {
            0.0
        };

        let wp_angle = facing_angle + weak_point.offset_angle;
        let wp_offset = Vec2::new(wp_angle.cos() * 60.0, wp_angle.sin() * 60.0);

        for child in children.iter() {
            if let Ok(mut transform) = visual_query.get_mut(child) {
                transform.translation.x = wp_offset.x;
                transform.translation.y = wp_offset.y;
            }
        }
    }
}

/// Modified damage check — projectiles hitting weak point do 3x damage
/// This is called by the projectile collision system via a public function
pub fn check_weak_point_hit(
    projectile_pos: Vec2,
    creature_pos: Vec2,
    creature_velocity: Vec2,
    weak_point: &WeakPoint,
) -> f32 {
    let facing_angle = if creature_velocity.length_squared() > 1.0 {
        creature_velocity.y.atan2(creature_velocity.x)
    } else {
        0.0
    };

    let wp_angle = facing_angle + weak_point.offset_angle;
    let wp_world_pos = creature_pos + Vec2::new(wp_angle.cos() * 60.0, wp_angle.sin() * 60.0);

    let dist_to_wp = projectile_pos.distance(wp_world_pos);
    if dist_to_wp < weak_point.radius {
        weak_point.damage_mult
    } else {
        1.0
    }
}

// ============================================================================
// 4. PARASITE BOARDING — enter ship through breaches, attack crew
// ============================================================================

/// Component marking a parasite that has boarded the ship
#[derive(Component)]
pub struct BoardedParasite {
    pub room_id: Option<usize>,
    pub attack_timer: f32,
    pub health: f32,
}

/// System: parasites near hull breaches can board the ship
pub fn parasite_boarding(
    mut commands: Commands,
    parasite_query: Query<(Entity, &Transform, &Creature, &CreatureAI), Without<BoardedParasite>>,
    hull_query: Query<(&HullSegment, &GlobalTransform)>,
    ship_query: Query<&Transform, With<Ship>>,
    room_map: Res<RoomMap>,
    mut notifications: MessageWriter<ShowNotification>,
    mut boarding_timer: Local<f32>,
    time: Res<Time>,
) {
    *boarding_timer += time.delta_secs();
    if *boarding_timer < 2.0 { return; } // Check every 2 seconds
    *boarding_timer = 0.0;

    let Ok(_ship_transform) = ship_query.single() else { return; };

    for (entity, transform, creature, _ai) in parasite_query.iter() {
        if creature.creature_type != CreatureType::ParasiteSwarm { continue; }
        if creature.health <= 0.0 { continue; }

        let parasite_pos = transform.translation.truncate();

        // Check if near a breached hull segment
        for (hull, hull_gt) in hull_query.iter() {
            if !hull.is_depressurized { continue; }

            let hull_pos = hull_gt.translation().truncate();
            let dist = parasite_pos.distance(hull_pos);

            if dist < 50.0 {
                // BOARD!
                let room_id = room_map.tile_to_room.get(&hull.grid_position).copied();

                commands.entity(entity).insert(BoardedParasite {
                    room_id,
                    attack_timer: 0.0,
                    health: creature.health,
                });

                // Make the parasite invisible (it's inside now)
                commands.entity(entity).insert(Visibility::Hidden);

                notifications.write(ShowNotification {
                    message: "BREACH! Parasites boarding the ship!".into(),
                    notification_type: NotificationType::Danger,
                    duration: 3.0,
                });

                break;
            }
        }
    }
}

/// System: boarded parasites attack crew and damage modules
pub fn boarded_parasite_damage(
    time: Res<Time>,
    mut parasite_query: Query<(Entity, &mut BoardedParasite)>,
    // Our crew, not everybody's: unfiltered, parasites chewing through the
    // player's corridors were also eating the crew of every hostile on the
    // map, which is both wrong and invisible.
    mut crew_query: Query<
        (Entity, &mut CrewMember),
        Without<crate::ai_ship::components::OwnedByAiShip>,
    >,
    mut notifications: MessageWriter<ShowNotification>,
    mut damage_events: MessageWriter<CrewDamaged>,
    mut damage_timer: Local<f32>,
) {
    let dt = time.delta_secs();
    *damage_timer += dt;
    if *damage_timer < 1.0 { return; }
    *damage_timer = 0.0;

    let mut boarded_count = 0u32;

    for (_entity, mut parasite) in parasite_query.iter_mut() {
        boarded_count += 1;
        parasite.attack_timer += 1.0;
    }

    if boarded_count == 0 { return; }

    // Damage random crew member
    let crew_count = crew_query.iter().filter(|(_, c)| c.health > 0.0).count();
    if crew_count > 0 {
        let damage_per_parasite = 1.5;
        let total_damage = boarded_count as f32 * damage_per_parasite;

        // Distribute damage across crew
        let share = total_damage / crew_count as f32;
        for (entity, mut crew) in crew_query.iter_mut() {
            if crew.health <= 0.0 { continue; }
            crew.health = (crew.health - share).max(0.0);
            damage_events.write(CrewDamaged {
                crew: entity,
                amount: share,
                source: CrewDamageSource::Boarders,
            });
        }

        if boarded_count >= 3 {
            notifications.write(ShowNotification {
                message: format!("{} parasites inside! Crew taking damage!", boarded_count),
                notification_type: NotificationType::Danger,
                duration: 2.0,
            });
        }
    }
}

/// System: crew kills boarded parasites over time when in Repairing state
pub fn crew_fights_boarders(
    time: Res<Time>,
    mut commands: Commands,
    crew_query: Query<&CrewMember>,
    mut parasite_query: Query<(Entity, &mut BoardedParasite)>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let repairing_crew = crew_query.iter()
        .filter(|c| c.state == CrewState::Repairing && c.health > 0.0)
        .count();

    if repairing_crew == 0 { return; }

    let kill_rate = repairing_crew as f32 * 2.0 * time.delta_secs();

    for (entity, mut parasite) in parasite_query.iter_mut() {
        parasite.health -= kill_rate;
        if parasite.health <= 0.0 {
            commands.entity(entity).try_despawn();
            notifications.write(ShowNotification {
                message: "Crew eliminated a boarded parasite!".into(),
                notification_type: NotificationType::Success,
                duration: 2.0,
            });
            return; // One kill per frame
        }
    }
}
