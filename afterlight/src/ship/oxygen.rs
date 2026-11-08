use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;

// Helper function to get effective O2 generation (CalculatedStats or base OxygenScrubber)
fn get_o2_generation(calculated: Option<&CalculatedStats>, scrubber: &OxygenScrubber) -> f32 {
    calculated
        .and_then(|c| c.life_support.as_ref())
        .map(|ls| ls.o2_generation)
        .unwrap_or(scrubber.output)
}

/// Updates the oxygen system, including OxygenTank reserve fill/drain.
pub fn update_oxygen_system(
    time: Res<Time>,
    config: Res<GameConfig>,
    // Without<OwnedByAiShip>: player-only ship air pool — AI ships carry
    // real CrewMember data now too (see ai_ship::crew), unscoped this would
    // let nearby AI crews inflate the player's own O2 consumption.
    scrubber_query: Query<(&OxygenScrubber, &Module, Option<&CalculatedStats>, Option<&ModuleEfficiency>), Without<crate::ai_ship::components::OwnedByAiShip>>,
    crew_query: Query<&CrewMember, Without<crate::ai_ship::components::OwnedByAiShip>>,
    mut tank_query: Query<(&mut OxygenTankComp, &Module), Without<DestroyedModule>>,
    mut oxygen_state: ResMut<OxygenState>,
    mut oxygen_events: MessageWriter<OxygenStateChanged>,
) {
    let dt = time.delta_secs();

    // Calculate oxygen generation from active scrubbers
    let total_generation: f32 = scrubber_query
        .iter()
        .filter(|(_, module, _, _)| module.is_active)
        .map(|(scrubber, module, calculated_stats, eff)| {
            let efficiency = effective_efficiency(module, eff);
            get_o2_generation(calculated_stats, scrubber) * efficiency
        })
        .sum();

    // Calculate oxygen consumption from crew
    let crew_count = crew_query.iter().count() as f32;
    let total_consumption = crew_count * config.base_oxygen_consumption_per_crew;

    let mut balance = total_generation - total_consumption;

    // OxygenTank fill/drain logic
    if balance > 0.0 {
        // Surplus: fill tanks
        for (mut tank, module) in tank_query.iter_mut() {
            if !module.is_active || module.health <= 0.0 { continue; }
            if tank.stored < tank.capacity {
                let fill = (balance * dt).min(tank.capacity - tank.stored);
                tank.stored += fill;
                // Don't reduce balance — surplus just goes to both tank and main pool
            }
        }
    } else if balance < 0.0 {
        // Deficit: drain tanks to compensate
        let mut deficit = (-balance) * dt;
        for (mut tank, module) in tank_query.iter_mut() {
            if !module.is_active || module.health <= 0.0 { continue; }
            if deficit <= 0.0 { break; }
            let drain = deficit.min(tank.stored);
            tank.stored -= drain;
            deficit -= drain;
        }
        // If tanks covered the deficit, effective balance is 0
        let covered = (-balance) * dt - deficit;
        balance += covered / dt;
    }

    oxygen_state.total_oxygen_generation = total_generation;
    oxygen_state.total_oxygen_consumption = total_consumption;
    oxygen_state.oxygen_balance = balance;

    // Update current oxygen level
    let oxygen_delta = balance * dt;
    oxygen_state.current_oxygen = (oxygen_state.current_oxygen + oxygen_delta)
        .clamp(0.0, oxygen_state.max_oxygen);

    // Check for critical oxygen levels
    let oxygen_percentage = if oxygen_state.max_oxygen > 0.0 {
        oxygen_state.current_oxygen / oxygen_state.max_oxygen
    } else {
        1.0
    };
    if oxygen_percentage < 0.2 {
        oxygen_events.write(OxygenStateChanged {
            new_level: oxygen_percentage,
            is_critical: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o2_generation_uses_calculated_stats_when_available() {
        let scrubber = OxygenScrubber { output: 5.0 };
        let calculated = CalculatedStats {
            life_support: Some(LifeSupportStats {
                o2_generation: 12.0,
                co2_filtering: 0.0,
                crew_capacity: 0,
            }),
            ..Default::default()
        };

        let result = get_o2_generation(Some(&calculated), &scrubber);
        assert!((result - 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn o2_generation_falls_back_to_scrubber_output() {
        let scrubber = OxygenScrubber { output: 5.0 };

        // No calculated stats
        let result = get_o2_generation(None, &scrubber);
        assert!((result - 5.0).abs() < f32::EPSILON);

        // Calculated stats with no life_support
        let calculated = CalculatedStats::default();
        let result = get_o2_generation(Some(&calculated), &scrubber);
        assert!((result - 5.0).abs() < f32::EPSILON);
    }
}
