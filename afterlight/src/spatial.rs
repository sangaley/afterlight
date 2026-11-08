use bevy::prelude::*;
use std::collections::HashMap;

use crate::ai_ship::components::AiShip;
use crate::combat::new_projectiles::MissileProjectile;
use crate::components::{Corpse, Creature, NoiseTrailPoint, Ship};
use crate::states::{GameState, SpatialSet};

/// Generic spatial hash grid mapping cells to the entities positioned in them.
/// `nearby()` returns candidates from any cell that could contain something
/// within `radius` — callers still do an exact distance check, but against a
/// small candidate set instead of every entity in the world.
#[derive(Default)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<IVec2, Vec<(Entity, Vec2)>>,
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self { cell_size, cells: HashMap::new() }
    }

    fn cell_of(&self, pos: Vec2) -> IVec2 {
        IVec2::new(
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        self.cells.entry(self.cell_of(pos)).or_default().push((entity, pos));
    }

    /// Entities in cells overlapping a circle of `radius` around `pos`.
    /// Over-inclusive at the cell boundary; still exact-distance-check the results.
    pub fn nearby(&self, pos: Vec2, radius: f32) -> impl Iterator<Item = (Entity, Vec2)> + '_ {
        let cell_radius = (radius / self.cell_size).ceil() as i32 + 1;
        let center = self.cell_of(pos);
        (-cell_radius..=cell_radius).flat_map(move |dx| {
            (-cell_radius..=cell_radius).filter_map(move |dy| {
                self.cells.get(&(center + IVec2::new(dx, dy)))
            }).flatten().copied()
        })
    }

    /// Nearest entity to `pos` within `radius`, if any (excludes `exclude` if given).
    pub fn nearest(&self, pos: Vec2, radius: f32, exclude: Option<Entity>) -> Option<(Entity, f32)> {
        self.nearby(pos, radius)
            .filter(|(e, _)| Some(*e) != exclude)
            .map(|(e, p)| (e, pos.distance(p)))
            .filter(|(_, dist)| *dist <= radius)
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }
}

/// Cell size tuned to typical creature detection ranges (100-350 units) and
/// weapon engagement ranges, so a query radius usually only touches a handful of cells.
const DEFAULT_CELL_SIZE: f32 = 200.0;

#[derive(Resource)]
pub struct CreatureGrid(pub SpatialGrid);
impl Default for CreatureGrid {
    fn default() -> Self { Self(SpatialGrid::new(DEFAULT_CELL_SIZE)) }
}

#[derive(Resource)]
pub struct CorpseGrid(pub SpatialGrid);
impl Default for CorpseGrid {
    fn default() -> Self { Self(SpatialGrid::new(DEFAULT_CELL_SIZE)) }
}

#[derive(Resource)]
pub struct NoiseTrailGrid(pub SpatialGrid);
impl Default for NoiseTrailGrid {
    fn default() -> Self { Self(SpatialGrid::new(DEFAULT_CELL_SIZE)) }
}

#[derive(Resource)]
pub struct AiShipGrid(pub SpatialGrid);
impl Default for AiShipGrid {
    fn default() -> Self { Self(SpatialGrid::new(DEFAULT_CELL_SIZE)) }
}

/// Smaller cells than the default: missiles/interceptors are engaged at close range.
#[derive(Resource)]
pub struct MissileGrid(pub SpatialGrid);
impl Default for MissileGrid {
    fn default() -> Self { Self(SpatialGrid::new(64.0)) }
}

fn rebuild_creature_grid(mut grid: ResMut<CreatureGrid>, query: Query<(Entity, &Transform), (With<Creature>, Without<Ship>)>) {
    grid.0.clear();
    for (entity, transform) in query.iter() {
        grid.0.insert(entity, transform.translation.truncate());
    }
}

fn rebuild_corpse_grid(mut grid: ResMut<CorpseGrid>, query: Query<(Entity, &Transform), With<Corpse>>) {
    grid.0.clear();
    for (entity, transform) in query.iter() {
        grid.0.insert(entity, transform.translation.truncate());
    }
}

fn rebuild_noise_trail_grid(mut grid: ResMut<NoiseTrailGrid>, query: Query<(Entity, &Transform), With<NoiseTrailPoint>>) {
    grid.0.clear();
    for (entity, transform) in query.iter() {
        grid.0.insert(entity, transform.translation.truncate());
    }
}

fn rebuild_ai_ship_grid(mut grid: ResMut<AiShipGrid>, query: Query<(Entity, &Transform), With<AiShip>>) {
    grid.0.clear();
    for (entity, transform) in query.iter() {
        grid.0.insert(entity, transform.translation.truncate());
    }
}

fn rebuild_missile_grid(mut grid: ResMut<MissileGrid>, query: Query<(Entity, &Transform), With<MissileProjectile>>) {
    grid.0.clear();
    for (entity, transform) in query.iter() {
        grid.0.insert(entity, transform.translation.truncate());
    }
}

/// Maintains per-frame spatial hash grids for entity populations that other
/// systems need to run "nearby X" queries against (creature AI perception,
/// weapon targeting, projectile/missile collision) without brute-force
/// scanning every entity in the world for every query.
pub struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CreatureGrid>()
            .init_resource::<CorpseGrid>()
            .init_resource::<NoiseTrailGrid>()
            .init_resource::<AiShipGrid>()
            .init_resource::<MissileGrid>()
            .configure_sets(Update, SpatialSet::Update.run_if(in_state(GameState::Exploring)))
            .add_systems(
                Update,
                (
                    rebuild_creature_grid,
                    rebuild_corpse_grid,
                    rebuild_noise_trail_grid,
                    rebuild_ai_ship_grid,
                    rebuild_missile_grid,
                )
                    .in_set(SpatialSet::Update),
            );
    }
}
