use bevy::prelude::*;
use rand::Rng;

use crate::components::*;
use crate::events::*;
use super::components::*;

// ============================================================================
// SCAVENGER COMPETITION — wrecks are contested, not patient.
// A fresh kill rings the dinner bell: some time after a ship dies, a
// Recursive Kingdom wing spawns at the edge of the area and burns straight for
// the hulk (their brain already prioritizes wrecks — see ai_brain's
// Recursive Kingdom tree). Every swarm ship parked on a wreck EATS its loot,
// unit by unit. The player's choice: strip the wreck fast, drive the
// vultures off (they're brittle kamikaze junk), or cede the carcass.
// ============================================================================

/// Chance a kill attracts a wave at all.
const WAVE_CHANCE: f32 = 0.35;
/// How long after the kill the vultures arrive (randomized per wave) —
/// generous enough to strip most of a wreck first if you start promptly.
const WAVE_DELAY_MIN: f32 = 50.0;
const WAVE_DELAY_MAX: f32 = 110.0;
/// How far out the wave spawns from the wreck.
const SPAWN_DIST_MIN: f32 = 1600.0;
const SPAWN_DIST_MAX: f32 = 2200.0;
/// A swarm ship must be this close to a wreck to feed on it.
const FEED_RANGE: f32 = 260.0;
/// Seconds per loot unit eaten, per feeding ship.
const FEED_SECONDS: f32 = 8.0;

struct PendingWave {
    wreck: Entity,
    position: Vec2,
    timer: Timer,
    ships: u32,
}

#[derive(Resource, Default)]
pub struct ScavengerWaves {
    pending: Vec<PendingWave>,
}

/// A kill advertises itself — schedule the vultures.
pub fn schedule_scavenger_waves(
    mut waves: ResMut<ScavengerWaves>,
    mut destroyed_events: MessageReader<AiShipDestroyed>,
) {
    let mut rng = rand::thread_rng();
    for event in destroyed_events.read() {
        // The scavengers don't mourn their own.
        if event.ship_type == AiShipType::RecursiveKingdom {
            continue;
        }
        if rng.gen::<f32>() > WAVE_CHANCE {
            continue;
        }
        // Bigger carcasses draw bigger flocks.
        let ships = match event.ship_type {
            AiShipType::EternalHegemony | AiShipType::Shepherd => 3,
            AiShipType::TerranHegemony | AiShipType::CorpseStars => 2,
            _ => rng.gen_range(1..=2),
        };
        waves.pending.push(PendingWave {
            wreck: event.entity,
            position: event.position,
            timer: Timer::from_seconds(rng.gen_range(WAVE_DELAY_MIN..WAVE_DELAY_MAX), TimerMode::Once),
            ships,
        });
    }
}

/// When a wave's clock runs out and the carcass still has meat on it,
/// the vultures arrive at the edge and burn inward.
///
/// They join the world simulation rather than appearing as bare entities:
/// the block budget decides when they materialise, they keep their damage if
/// you drive them off and come back, and a save remembers them.
pub fn spawn_scavenger_waves(
    time: Res<Time>,
    mut waves: ResMut<ScavengerWaves>,
    wreck_query: Query<&Wreck>,
    mut sim: ResMut<super::components::WorldSimulation>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let mut rng = rand::thread_rng();
    let system_id = streaming.loaded_system.unwrap_or(0);
    waves.pending.retain_mut(|wave| {
        wave.timer.tick(time.delta());
        if !wave.timer.is_finished() {
            return true;
        }
        // Wreck already gone or picked clean — nothing to come for.
        if !wreck_query.get(wave.wreck).is_ok_and(|w| w.loot_remaining > 0) {
            return false;
        }
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let base = wave.position
            + Vec2::new(angle.cos(), angle.sin()) * rng.gen_range(SPAWN_DIST_MIN..SPAWN_DIST_MAX);
        // One fleet: they arrive together and leave together. Home is the
        // carcass: their patrol rings it and the wreck-chasing brain does
        // the rest.
        let classes: Vec<_> = (0..wave.ships)
            .map(|_| super::roster::roll(AiShipType::RecursiveKingdom, &mut rng, |_| true).map(|c| c.key))
            .collect();
        let (mut fleet, members) = super::fleet::new_fleet(
            AiShipType::RecursiveKingdom,
            super::fleet::FleetKind::Raiders,
            &super::fleet::hulls_of(AiShipType::RecursiveKingdom, &classes),
            system_id,
            base,
            wave.position,
            1_500.0,
            &mut rng,
        );
        fleet.heading = (wave.position - base).to_angle();
        fleet.destination = wave.position;
        sim.add_fleet(fleet, members);
        notifications.write(ShowNotification {
            message: "Scavenger swarm inbound - they smell the wreck!".into(),
            notification_type: NotificationType::Warning,
            duration: 4.0,
        });
        false
    });
}

/// Swarm ships parked on a wreck eat its loot, one unit per ship every
/// few seconds — the visible smoke puffs are your cargo leaving.
pub fn scavengers_feed(
    time: Res<Time>,
    mut commands: Commands,
    swarm_query: Query<(&Transform, &AiShipType, &AiShipState)>,
    mut wreck_query: Query<(Entity, &GlobalTransform, &mut Wreck, &mut AiShipWreck, &Children)>,
    block_pos_query: Query<&GlobalTransform, Or<(With<Module>, With<HullSegment>)>>,
    mut notifications: MessageWriter<ShowNotification>,
    mut tick: Local<f32>,
    mut warned: Local<std::collections::HashSet<Entity>>,
) {
    *tick += time.delta_secs();
    if *tick < FEED_SECONDS {
        return;
    }
    *tick = 0.0;

    let mut rng = rand::thread_rng();
    for (wreck_entity, wreck_gt, mut wreck, mut ai_wreck, children) in wreck_query.iter_mut() {
        if wreck.loot_remaining == 0 {
            continue;
        }
        let wreck_pos = wreck_gt.translation().truncate();
        let feeders = swarm_query
            .iter()
            .filter(|(t, ship_type, state)| {
                **ship_type == AiShipType::RecursiveKingdom
                    && !state.is_destroyed
                    && t.translation.truncate().distance(wreck_pos) < FEED_RANGE
            })
            .count() as u32;
        if feeders == 0 {
            continue;
        }

        let eaten = feeders.min(wreck.loot_remaining);
        wreck.loot_remaining -= eaten;
        ai_wreck.loot_remaining = ai_wreck.loot_remaining.saturating_sub(eaten);

        // Smoke puff over a random block per unit eaten — loss reads visually
        let blocks: Vec<Vec2> = children
            .iter()
            .filter_map(|c| block_pos_query.get(c).ok())
            .map(|gt| gt.translation().truncate())
            .collect();
        for _ in 0..eaten.min(3) {
            let at = if blocks.is_empty() {
                wreck_pos
            } else {
                blocks[rng.gen_range(0..blocks.len())]
            };
            crate::combat::spawn_hit_effect(&mut commands, at, Color::srgba(0.55, 0.45, 0.3, 0.7), 34.0);
        }

        if warned.insert(wreck_entity) {
            notifications.write(ShowNotification {
                message: "Scavengers are stripping the wreck!".into(),
                notification_type: NotificationType::Warning,
                duration: 4.0,
            });
        }
    }
}
