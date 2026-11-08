use bevy::prelude::*;
use rand::Rng;

use crate::components::{CrewMember, CrewState};
use super::components::OwnedByAiShip;

const AI_CREW_NAMES: [&str; 10] = [
    "Vance", "Kade", "Orlo", "Brix", "Sten", "Yuri", "Pell", "Doss", "Wren", "Kray",
];

/// Spawns `count` bare-bones crew for an AI ship — health/morale/state only,
/// no Sprite/Transform. AI crew never walk between rooms or render, and
/// omitting Transform is also what keeps them out of every player-only
/// room-location/repair/emergency system (they all require GlobalTransform
/// or the CrewRoomLocation that's derived from it, so an entity without
/// Transform simply never matches those queries). What they DO need —
/// compute_module_efficiency, auto_assign_crew — already query CrewMember/
/// CrewStation with no Transform requirement, so staffing works unmodified.
pub fn spawn_ai_crew(commands: &mut Commands, root: Entity, count: u32) {
    let mut rng = rand::thread_rng();
    for _ in 0..count {
        let name = AI_CREW_NAMES[rng.gen_range(0..AI_CREW_NAMES.len())];
        commands.spawn((
            CrewMember {
                name: name.to_string(),
                health: 100.0,
                max_health: 100.0,
                oxygen: 100.0,
                morale: 100.0,
                state: CrewState::Idle,
            },
            OwnedByAiShip { root },
            ChildOf(root),
        ));
    }
}
