//! AFTERLIGHT_COMBAT_REPORT=1: a line every five seconds on how a fight stands,
//! for the combat test runs (tools/combat-lab): the player's hull, power,
//! crew and blocks, and every enemy ship in sensor range -- its class, hull,
//! distance and what it's doing. Nothing at all without the switch.

use bevy::prelude::*;

use crate::ai_ship::components::{AiShip, AiShipBehavior, AiShipClass, AiShipState, AiShipType, OwnedByAiShip};
use crate::components::{CrewMember, DestroyedModule, HullDestroyed, HullSegment, Module, Ship};

/// Seconds between report lines.
const EVERY_S: f32 = 5.0;

pub fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("AFTERLIGHT_COMBAT_REPORT").is_some())
}

#[allow(clippy::too_many_arguments)]
pub fn combat_report(
    time: Res<Time>,
    mut next: Local<f32>,
    mut start_blocks: Local<Option<usize>>,
    hull: Res<crate::resources::HullState>,
    power: Res<crate::resources::PowerState>,
    player: Query<(&Transform, Option<&Children>), With<Ship>>,
    blocks: Query<(), (Or<(With<Module>, With<HullSegment>)>, Without<DestroyedModule>, Without<HullDestroyed>)>,
    crew: Query<(), (With<CrewMember>, Without<OwnedByAiShip>)>,
    enemies: Query<(Entity, &Transform, &AiShipType, Option<&AiShipClass>, &AiShipState, &AiShipBehavior, Option<&crate::ai_ship::awareness::Awareness>), With<AiShip>>,
    // Every enemy block still standing, by its ship: hull integrity alone
    // counts bare floor, and most rounds land on what stands over it.
    enemy_blocks: Query<&OwnedByAiShip, (Or<(With<Module>, With<HullSegment>)>, Without<DestroyedModule>, Without<HullDestroyed>)>,
    mut enemy_start: Local<bevy::platform::collections::HashMap<Entity, usize>>,
    // What did it since the last line: hits by source, and what blew up.
    mut hits: MessageReader<crate::events::ShipDamaged>,
    mut explosions: MessageReader<crate::events::ModuleExploded>,
    mut tally: Local<(bevy::platform::collections::HashMap<String, (u32, f32)>, u32)>,
) {
    if !on() {
        return;
    }
    for hit in hits.read() {
        let entry = tally.0.entry(format!("{:?}", hit.source)).or_default();
        entry.0 += 1;
        entry.1 += hit.amount;
    }
    tally.1 += explosions.read().count() as u32;
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + EVERY_S;
    let Ok((at, children)) = player.single() else { return };
    let at = at.translation.truncate();
    let alive = children.map_or(0, |c| c.iter().filter(|e| blocks.contains(*e)).count());
    let start = *start_blocks.get_or_insert(alive);
    let mut standing: bevy::platform::collections::HashMap<Entity, usize> = Default::default();
    for owner in enemy_blocks.iter() {
        *standing.entry(owner.root).or_insert(0) += 1;
    }
    let mut seen: Vec<String> = enemies
        .iter()
        .filter_map(|(ship, tf, faction, class, state, behavior, mind)| {
            let d = tf.translation.truncate().distance(at);
            let now_blocks = standing.get(&ship).copied().unwrap_or(0);
            let first = *enemy_start.entry(ship).or_insert(now_blocks);
            (d <= crate::camera::SENSOR_RANGE * 1.5).then(|| {
                format!(
                    "{:?}/{} hull={:.2} blocks={}/{} d={:.0} {}",
                    faction,
                    class.map_or("-", |c| c.0),
                    state.hull_integrity,
                    now_blocks,
                    first,
                    d,
                    if state.is_destroyed { "DESTROYED".to_string() } else { format!("{behavior:?}:{:?}", mind.map_or(crate::ai_ship::awareness::Tactic::None, |m| m.tactic)) }
                )
            })
        })
        .collect();
    seen.sort();
    if !tally.0.is_empty() || tally.1 > 0 {
        let mut sources: Vec<String> = tally.0.iter().map(|(k, (n, d))| format!("{k} x{n} {d:.0}dmg")).collect();
        sources.sort();
        info!("COMBAT HITS t={:.0} {} | explosions {}", now, sources.join(", "), tally.1);
    }
    tally.0.clear();
    tally.1 = 0;
    info!(
        "COMBAT t={:.0} player hull={:.2} blocks={}/{} power={:.0}/{:.0} crew={} | {}",
        now,
        hull.hull_integrity,
        alive,
        start,
        power.total_power_generation,
        power.total_power_consumption,
        crew.iter().count(),
        seen.join(" ; ")
    );
}
