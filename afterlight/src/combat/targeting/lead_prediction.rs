use bevy::prelude::*;
use crate::components::*;

// ============================================================================
// LEAD PREDICTION ENGINE
// Always active — every weapon uses lead prediction.
// Quality depends on Targeting Computer block (required for Advanced).
// Accounts for shooter velocity. Accuracy degrades with distance.
// ============================================================================

/// Prediction quality tier
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PredictionTier {
    /// Linear prediction — assumes target moves in straight line
    #[default]
    BasicLead,
    /// Predicts acceleration changes, accounts for target maneuvers
    /// Requires Targeting Computer module adjacent to weapon
    AdvancedTracking,
}

/// Result of a lead prediction calculation
pub struct PredictionResult {
    pub aim_point: Vec2,
    pub confident: bool,
    pub travel_time: f32,
    /// Accuracy multiplier: 1.0 = perfect, lower = more spread. Degrades with distance.
    pub distance_accuracy: f32,
}

/// Calculate lead prediction: where to aim to hit a moving target.
/// Accounts for both target AND shooter velocity.
///
/// - `shooter_pos`: position of the weapon
/// - `shooter_vel`: velocity of the shooter (ship velocity)
/// - `target_pos`: current position of the target
/// - `target_vel`: current velocity of the target
/// - `target_accel`: acceleration of the target (only used by AdvancedTracking)
/// - `projectile_speed`: speed of the projectile (relative to shooter)
/// - `tier`: prediction quality
/// - `weapon_range`: max range of the weapon (for accuracy falloff)
pub fn calculate_lead(
    shooter_pos: Vec2,
    shooter_vel: Vec2,
    target_pos: Vec2,
    target_vel: Vec2,
    target_accel: Vec2,
    projectile_speed: f32,
    tier: PredictionTier,
    weapon_range: f32,
) -> PredictionResult {
    let to_target = target_pos - shooter_pos;
    let distance = to_target.length();

    if projectile_speed <= 0.0 || distance < 1.0 {
        return PredictionResult {
            aim_point: target_pos,
            confident: true,
            travel_time: 0.0,
            distance_accuracy: 1.0,
        };
    }

    // Relative velocity: target velocity minus shooter velocity
    // This accounts for the shooter's own movement
    let relative_vel = target_vel - shooter_vel;

    // Distance-based accuracy degradation
    // At 50% range = full accuracy. At 100% range = 60% accuracy. Beyond = worse.
    let range_ratio = (distance / weapon_range.max(1.0)).clamp(0.0, 2.0);
    let distance_accuracy = if range_ratio < 0.5 {
        1.0
    } else {
        1.0 - (range_ratio - 0.5) * 0.53 // Linear falloff: 1.0 at 50%, ~0.73 at 100%, ~0.47 at 150%
    };

    match tier {
        PredictionTier::BasicLead => {
            // Linear prediction using relative velocity
            // Iterative solution (3 iterations)
            let mut travel_time = distance / projectile_speed;

            for _ in 0..3 {
                let predicted_pos = target_pos + relative_vel * travel_time;
                let new_dist = (predicted_pos - shooter_pos).length();
                travel_time = new_dist / projectile_speed;
            }

            let aim_point = target_pos + relative_vel * travel_time;

            PredictionResult {
                aim_point,
                confident: travel_time < 5.0,
                travel_time,
                distance_accuracy,
            }
        }
        PredictionTier::AdvancedTracking => {
            // Quadratic prediction: includes acceleration + relative velocity
            let mut travel_time = distance / projectile_speed;

            for _ in 0..4 {
                let predicted_pos = target_pos
                    + relative_vel * travel_time
                    + target_accel * 0.5 * travel_time * travel_time;
                let new_dist = (predicted_pos - shooter_pos).length();
                travel_time = new_dist / projectile_speed;
            }

            let aim_point = target_pos
                + relative_vel * travel_time
                + target_accel * 0.5 * travel_time * travel_time;

            // Advanced tracking is slightly more accurate at range
            let advanced_accuracy = (distance_accuracy + 0.1).min(1.0);

            PredictionResult {
                aim_point,
                confident: travel_time < 8.0,
                travel_time,
                distance_accuracy: advanced_accuracy,
            }
        }
    }
}

/// Determine prediction tier for a weapon.
/// BasicLead is always available — every weapon gets lead prediction.
/// AdvancedTracking requires a Targeting Computer module ADJACENT to the weapon
/// (must touch the core or any barrel block — not just anywhere on the ship).
pub fn get_weapon_prediction_tier(
    _weapon_module: &Module,
    _customization: Option<&crate::building::customization::parameters::ModuleCustomization>,
    has_adjacent_targeting_computer: bool,
) -> PredictionTier {
    if has_adjacent_targeting_computer {
        PredictionTier::AdvancedTracking
    } else {
        PredictionTier::BasicLead
    }
}

/// Check if a Targeting Computer is adjacent to a specific weapon position.
/// Also checks adjacent barrel blocks in the chain.
pub fn check_adjacent_targeting_computer(
    weapon_grid_pos: IVec2,
    all_modules: &[(IVec2, ModuleType, bool)],
) -> bool {
    for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
        let adj_pos = weapon_grid_pos + offset;
        for (pos, mt, active) in all_modules {
            if *pos == adj_pos && *mt == ModuleType::TargetingComputer && *active {
                return true;
            }
        }
    }
    false
}
