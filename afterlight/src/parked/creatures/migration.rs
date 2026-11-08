use bevy::prelude::*;

use crate::components::{
    Creature, CreatureAI, CreatureAIState, CreatureNeeds, FoodChainRole,
    HungerDuration, MigrationPath, Territory,
};

/// Check if a creature should start migrating:
/// hungry (>60) for 45s+ and local prey is sparse
pub fn check_migration(
    time: Res<Time>,
    mut commands: Commands,
    mut creatures: Query<(
        Entity,
        &Transform,
        &Creature,
        &CreatureNeeds,
        &FoodChainRole,
        Option<&mut HungerDuration>,
        Option<&MigrationPath>,
    )>,
    other_creatures: Query<(Entity, &Transform, &Creature), Without<MigrationPath>>,
) {
    let dt = time.delta_secs();

    for (entity, transform, creature, needs, role, hunger_dur, existing_path) in creatures.iter_mut()
    {
        // Skip if already migrating
        if existing_path.is_some() {
            continue;
        }

        let pos = transform.translation.truncate();

        if needs.hunger > 60.0 {
            // Update or create hunger duration tracker
            if let Some(mut dur) = hunger_dur {
                dur.timer += dt;

                if dur.timer >= 45.0 {
                    // Check local prey density
                    let detection_range = creature.detection_range * 2.0;
                    let mut local_prey = 0u32;

                    // Count prey in range
                    for (_e, other_transform, other_creature) in other_creatures.iter() {
                        if pos.distance(other_transform.translation.truncate()) < detection_range
                            && role.prey_types.contains(&other_creature.creature_type)
                        {
                            local_prey += 1;
                        }
                    }

                    if local_prey < 2 {
                        // Generate migration path with 3-4 waypoints
                        let angle = rand::random::<f32>() * std::f32::consts::TAU;
                        let distance = 800.0 + rand::random::<f32>() * 600.0;
                        let depth_change = (rand::random::<f32>() - 0.5) * 400.0;

                        let waypoints = vec![
                            pos + Vec2::new(angle.cos() * distance * 0.33, angle.sin() * distance * 0.33 + depth_change * 0.33),
                            pos + Vec2::new(angle.cos() * distance * 0.66, angle.sin() * distance * 0.66 + depth_change * 0.66),
                            pos + Vec2::new(angle.cos() * distance, angle.sin() * distance + depth_change),
                        ];

                        commands.entity(entity).insert(MigrationPath {
                            waypoints,
                            current_waypoint: 0,
                            arrival_radius: 50.0,
                        });

                        // Reset hunger duration
                        dur.timer = 0.0;
                    }
                }
            } else {
                commands.entity(entity).insert(HungerDuration { timer: 0.0 });
            }
        } else if let Some(mut dur) = hunger_dur {
            // Reset timer if not hungry enough
            dur.timer = 0.0;
        }
    }
}

/// Creatures with MigrationPath move toward waypoints
pub fn follow_migration_path(
    mut commands: Commands,
    mut creatures: Query<(
        Entity,
        &Transform,
        &mut CreatureAI,
        &mut MigrationPath,
        Option<&mut Territory>,
    )>,
) {
    for (entity, transform, mut ai, mut path, territory) in creatures.iter_mut() {
        if path.waypoints.is_empty() || path.current_waypoint >= path.waypoints.len() {
            // Migration complete — establish new home
            let new_home = transform.translation.truncate();
            ai.home_position = new_home;
            ai.state = CreatureAIState::Wandering;
            ai.target = None;

            if let Some(mut terr) = territory {
                terr.center = new_home;
            }

            commands.entity(entity).remove::<MigrationPath>();
            continue;
        }

        let current_target = path.waypoints[path.current_waypoint];
        let pos = transform.translation.truncate();
        let dist = pos.distance(current_target);

        if dist < path.arrival_radius {
            path.current_waypoint += 1;
        } else {
            ai.state = CreatureAIState::Migrating;
            ai.target = Some(crate::components::EcoTarget::Position(current_target));
        }
    }
}
