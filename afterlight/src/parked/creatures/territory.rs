use bevy::prelude::*;

use crate::components::{CreatureMemory, Territory};

/// Slowly drift territory center toward remembered food-rich areas
pub fn update_territories(
    time: Res<Time>,
    mut creatures: Query<(&mut Territory, &CreatureMemory)>,
) {
    let dt = time.delta_secs();
    for (mut territory, memory) in creatures.iter_mut() {
        if memory.food_locations.is_empty() {
            continue;
        }

        // Average food location
        let avg_food: Vec2 = memory
            .food_locations
            .iter()
            .map(|(pos, _)| *pos)
            .sum::<Vec2>()
            / memory.food_locations.len() as f32;

        // Slowly drift territory center toward food (very slow, 2 units/s max)
        let drift = (avg_food - territory.center).clamp_length_max(2.0 * dt);
        territory.center += drift;
    }
}
