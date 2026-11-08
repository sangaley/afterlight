use bevy::prelude::*;

use crate::components::*;
use super::components::*;

/// Sums child Engine noise into AiShipState.noise_level.
/// Inserts/removes AiShipRadarContact based on threshold.
pub fn ai_ship_noise_system(
    mut commands: Commands,
    mut ai_ships: Query<(Entity, &mut AiShipState, &Children), With<AiShip>>,
    engine_query: Query<(&Engine, &Module, &OwnedByAiShip)>,
) {
    for (entity, mut state, children) in ai_ships.iter_mut() {
        if state.is_destroyed {
            continue;
        }

        let mut total_noise = 0.0_f32;
        for child in children.iter() {
            if let Ok((engine, module, _)) = engine_query.get(child) {
                if module.is_active && module.health > 0.0 {
                    let efficiency = module.health / module.max_health;
                    total_noise += engine.noise_level * efficiency;
                }
            }
        }

        state.noise_level = total_noise;

        // Insert/remove radar contact based on noise threshold
        if total_noise > 20.0 {
            // try_insert: is_destroyed may flip true later this same frame in
            // another system, after which ai_ship_death_system recursively
            // despawns this root entity — a plain insert() queued earlier
            // this frame would then panic when commands flush.
            commands.entity(entity).try_insert(AiShipRadarContact {
                noise_signature: total_noise,
                revealed_timer: Timer::from_seconds(3.0, TimerMode::Once),
            });
        }
    }
}

/// Spawns NoiseTrailPoint entities at AI ship positions when noisy enough.
/// Creatures with trail detection (BlindHunter, Scavenger) follow these automatically.
pub fn ai_ship_noise_trail_system(
    time: Res<Time>,
    ai_ships: Query<(&Transform, &AiShipState), With<AiShip>>,
    mut commands: Commands,
    mut trail_timer: Local<f32>,
) {
    *trail_timer += time.delta_secs();
    if *trail_timer < 0.5 {
        return;
    }
    *trail_timer = 0.0;

    for (transform, state) in ai_ships.iter() {
        if state.is_destroyed || state.noise_level < 5.0 {
            continue;
        }

        let pos = transform.translation.truncate();
        commands.spawn((
            (Sprite {
                    color: Color::srgba(0.0, 0.0, 0.0, 0.0), 
                    custom_size: Some(Vec2::splat(1.0)),
                    ..default()
                }, Transform::from_xyz(pos.x, pos.y, -0.2)),
            NoiseTrailPoint {
                intensity: state.noise_level * 0.5,
                decay_rate: 2.0,
            },
        ));
    }
}

/// Decay radar contact timers on AI ships
pub fn ai_ship_radar_contact_decay(
    time: Res<Time>,
    mut commands: Commands,
    mut contacts: Query<(Entity, &mut AiShipRadarContact)>,
) {
    for (entity, mut contact) in contacts.iter_mut() {
        contact.revealed_timer.tick(time.delta());
        if contact.revealed_timer.is_finished() {
            commands.entity(entity).remove::<AiShipRadarContact>();
        }
    }
}
