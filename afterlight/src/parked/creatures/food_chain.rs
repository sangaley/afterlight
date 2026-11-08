use crate::components::{
    CreatureType, FoodChainRole, FoodChainTier,
};

/// Per-type ecosystem stats for spawning components
pub struct EcoStats {
    pub hunger_rate: f32,
    pub energy_drain_rate: f32,
    pub food_value: f32,
    pub is_territorial: bool,
    pub territory_radius: f32,
    pub territory_aggression: f32,
    pub can_reproduce: bool,
    pub gestation_duration: f32,
    pub offspring_count: u32,
    pub satiation_threshold: f32,
}

/// Returns the food chain role for a creature type
pub fn food_chain_role(creature_type: CreatureType) -> FoodChainRole {
    match creature_type {
        CreatureType::Leviathan => FoodChainRole {
            tier: FoodChainTier::Apex,
            prey_types: vec![
                CreatureType::Stalker,
                CreatureType::VoidDrifter,
            ],
            threat_types: vec![],
            attacks_ship: true,
        },
        CreatureType::Stalker => FoodChainRole {
            tier: FoodChainTier::Predator,
            prey_types: vec![
                CreatureType::VoidDrifter,
                CreatureType::ParasiteSwarm,
            ],
            threat_types: vec![CreatureType::Leviathan],
            attacks_ship: true,
        },
        CreatureType::ParasiteSwarm => FoodChainRole {
            tier: FoodChainTier::Scavenger,
            prey_types: vec![],
            threat_types: vec![CreatureType::Stalker, CreatureType::Leviathan],
            attacks_ship: true,
        },
        CreatureType::VoidDrifter => FoodChainRole {
            tier: FoodChainTier::Prey,
            prey_types: vec![],
            threat_types: vec![
                CreatureType::Stalker,
                CreatureType::Leviathan,
            ],
            attacks_ship: false,
        },
    }
}

/// Returns ecosystem stats for spawning creature components
pub fn creature_ecosystem_stats(creature_type: CreatureType) -> EcoStats {
    match creature_type {
        CreatureType::VoidDrifter => EcoStats {
            hunger_rate: 0.2,            // Low metabolism — passive
            energy_drain_rate: 0.05,
            food_value: 6.0,
            is_territorial: false,
            territory_radius: 0.0,
            territory_aggression: 0.0,
            can_reproduce: true,
            gestation_duration: 50.0,    // Slow reproduction
            offspring_count: 2,
            satiation_threshold: 50.0,
        },
        CreatureType::Stalker => EcoStats {
            hunger_rate: 0.5,            // Gets hungry, hunts when needed
            energy_drain_rate: 0.2,
            food_value: 25.0,
            is_territorial: true,
            territory_radius: 500.0,     // Larger territory
            territory_aggression: 0.7,
            can_reproduce: true,
            gestation_duration: 120.0,   // Slow — keeps population manageable
            offspring_count: 1,
            satiation_threshold: 65.0,
        },
        CreatureType::Leviathan => EcoStats {
            hunger_rate: 0.15,           // Barely needs to eat — apex predator
            energy_drain_rate: 0.05,
            food_value: 150.0,
            is_territorial: true,
            territory_radius: 1200.0,    // Massive territory
            territory_aggression: 1.0,
            can_reproduce: false,        // One per system
            gestation_duration: 0.0,
            offspring_count: 0,
            satiation_threshold: 0.0,
        },
        CreatureType::ParasiteSwarm => EcoStats {
            hunger_rate: 1.0,            // Always hungry — drives them to attach
            energy_drain_rate: 0.3,
            food_value: 2.0,
            is_territorial: false,
            territory_radius: 0.0,
            territory_aggression: 0.0,
            can_reproduce: true,
            gestation_duration: 30.0,    // Fast reproduction — swarm behavior
            offspring_count: 4,
            satiation_threshold: 30.0,
        },
    }
}
