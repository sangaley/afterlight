use bevy::prelude::*;
use super::components::PlanetType;

/// A star emits a radiation flare — random timing, intensity scales with star size
#[derive(Message)]
pub struct RadiationFlare {
    pub star: Entity,
    pub intensity: f32,
    pub position: Vec2,
    pub radius: f32,
}

/// A star has been destroyed — triggers supernova, planets go flying
#[derive(Message)]
pub struct StarDestroyed {
    pub star: Entity,
    pub position: Vec2,
    pub supernova_radius: f32,
    pub freed_planets: Vec<Entity>,
}

/// A planet was consumed by a black hole
#[derive(Message)]
pub struct PlanetConsumed {
    pub planet: Entity,
    pub black_hole: Entity,
    pub planet_type: PlanetType,
}

/// Any body consumed by a black hole (planets, asteroids, debris)
#[derive(Message)]
pub struct BodyConsumed {
    pub entity: Entity,
    pub black_hole: Entity,
    pub mass_gained: f32,
}

/// Warning when gravity pull becomes significant on the ship
#[derive(Message)]
pub struct GravityWarning {
    pub source: Entity,
    pub pull_strength: f32,
}

/// Supernova shockwave expanding outward
#[derive(Message)]
pub struct SupernovaShockwave {
    pub origin: Vec2,
    pub damage: f32,
    pub radius: f32,
}

/// Warp jump initiated — either a known system or a blind point in space
#[derive(Message)]
pub struct WarpJumpStarted {
    pub target: super::resources::GalaxyWarpTarget,
}

/// Warp jump completed. system_id is None if the jump landed in empty space
/// (a blind point with no system close enough to snap to).
#[derive(Message)]
pub struct WarpJumpCompleted {
    pub system_id: Option<u32>,
}
