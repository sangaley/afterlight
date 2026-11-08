use bevy::prelude::*;

use crate::ai_ship::components::AiShip;
use crate::components::{BulkheadSealed, DestroyedModule, HullDestroyed, HullSegment, Module, Ship};

// ============================================================================
// LAYOUT VERSIONS
//
// Most of what the game derives from a ship's blocks -- the hit grid, rooms,
// walkable decks, power reach, shield size -- only changes when the SET of
// blocks changes: one is placed, destroyed, removed or (un)sealed. Those
// systems used to find out by recounting every block in the world each frame
// and rebuilding every ship when any count moved. In a fight a block dies
// nearly every frame, so in practice they rebuilt every ship every frame.
//
// Instead each ship carries a LayoutVersion that observers bump the moment one
// of its blocks changes. A system keeps the version it last built from and
// rebuilds that ship alone when the two differ. LayoutEpoch is bumped with
// every ship's, so "did anything anywhere change?" is one comparison.
//
// What it does NOT track: health above zero, is_active, efficiency, heat.
// Those move every frame and are read live by the systems that need them.
// ============================================================================

/// Bumped whenever this ship gains, loses, breaks or (un)seals a block.
/// Required on every `Ship` and `AiShip`, so it is always there to read.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct LayoutVersion(pub u64);

/// Bumped whenever any ship's LayoutVersion is.
#[derive(Resource, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct LayoutEpoch(pub u64);

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LayoutEpoch>()
            .register_required_components::<Ship, LayoutVersion>()
            .register_required_components::<AiShip, LayoutVersion>()
            .register_required_components::<crate::combat::turret_deck::TurretBody, LayoutVersion>()
            // A block joins a ship when its ChildOf is inserted (spawning it
            // under the ship does that in the same bundle) and leaves when
            // its ChildOf goes: despawned, re-parented onto debris, or
            // detached. Discard fires before the old ChildOf is gone, so the
            // ship it is leaving can still be read.
            .add_observer(block_joined)
            .add_observer(block_left)
            .add_observer(|on: On<Add, HullDestroyed>, parents: Query<&ChildOf>, ships: Query<&mut LayoutVersion>, epoch: ResMut<LayoutEpoch>| {
                bump_parent_of(on.entity, &parents, ships, epoch);
            })
            .add_observer(|on: On<Add, DestroyedModule>, parents: Query<&ChildOf>, ships: Query<&mut LayoutVersion>, epoch: ResMut<LayoutEpoch>| {
                bump_parent_of(on.entity, &parents, ships, epoch);
            })
            .add_observer(|on: On<Add, BulkheadSealed>, parents: Query<&ChildOf>, ships: Query<&mut LayoutVersion>, epoch: ResMut<LayoutEpoch>| {
                bump_parent_of(on.entity, &parents, ships, epoch);
            })
            .add_observer(|on: On<Remove, BulkheadSealed>, parents: Query<&ChildOf>, ships: Query<&mut LayoutVersion>, epoch: ResMut<LayoutEpoch>| {
                bump_parent_of(on.entity, &parents, ships, epoch);
            });
    }
}

fn block_joined(
    on: On<Insert, ChildOf>,
    blocks: Query<&ChildOf, Or<(With<HullSegment>, With<Module>)>>,
    ships: Query<&mut LayoutVersion>,
    epoch: ResMut<LayoutEpoch>,
) {
    if let Ok(parent) = blocks.get(on.entity) {
        bump(parent.parent(), ships, epoch);
    }
}

fn block_left(
    on: On<Discard, ChildOf>,
    blocks: Query<&ChildOf, Or<(With<HullSegment>, With<Module>)>>,
    ships: Query<&mut LayoutVersion>,
    epoch: ResMut<LayoutEpoch>,
) {
    if let Ok(parent) = blocks.get(on.entity) {
        bump(parent.parent(), ships, epoch);
    }
}

fn bump_parent_of(
    entity: Entity,
    parents: &Query<&ChildOf>,
    ships: Query<&mut LayoutVersion>,
    epoch: ResMut<LayoutEpoch>,
) {
    if let Ok(parent) = parents.get(entity) {
        bump(parent.parent(), ships, epoch);
    }
}

/// The ship may already be gone (a whole hull despawning takes its blocks
/// with it); then there is nothing left to rebuild and nothing to do.
fn bump(ship: Entity, mut ships: Query<&mut LayoutVersion>, mut epoch: ResMut<LayoutEpoch>) {
    if let Ok(mut v) = ships.get_mut(ship) {
        v.0 += 1;
        epoch.0 += 1;
    }
}

/// Which LayoutVersion each ship's derived data was last built from, for a
/// system to keep in a `Local`. A ship it has never seen is stale.
#[derive(Default)]
pub struct BuiltFrom(bevy::platform::collections::HashMap<Entity, u64>);

impl BuiltFrom {
    pub fn is_stale(&self, ship: Entity, version: &LayoutVersion) -> bool {
        self.0.get(&ship) != Some(&version.0)
    }

    pub fn mark(&mut self, ship: Entity, version: &LayoutVersion) {
        self.0.insert(ship, version.0);
    }

    /// Drop ships that no longer exist, so the map doesn't grow forever.
    pub fn retain(&mut self, mut alive: impl FnMut(Entity) -> bool) {
        self.0.retain(|e, _| alive(*e));
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(LayoutPlugin);
        app
    }

    fn hull() -> HullSegment {
        HullSegment {
            health: 100.0,
            max_health: 100.0,
            radiation_shielding: 0.0,
            is_depressurized: false,
            depressurization_level: 0.0,
            hull_layer: crate::components::HullLayer::Outer,
            material: crate::components::HullMaterial::Steel,
            grid_position: IVec2::ZERO,
        }
    }

    fn version(app: &App, ship: Entity) -> u64 {
        app.world().get::<LayoutVersion>(ship).unwrap().0
    }

    #[test]
    fn every_ship_carries_a_version_from_birth() {
        let mut app = app();
        let player = app.world_mut().spawn(Ship).id();
        assert_eq!(version(&app, player), 0);
    }

    #[test]
    fn placing_breaking_and_removing_a_block_each_bump_its_ship_only() {
        let mut app = app();
        let a = app.world_mut().spawn(Ship).id();
        let b = app.world_mut().spawn(LayoutVersion::default()).id();

        let block = app.world_mut().spawn((hull(), ChildOf(a))).id();
        let after_place = version(&app, a);
        assert!(after_place > 0, "placing a block bumps its ship");
        assert_eq!(version(&app, b), 0, "and no other ship");

        app.world_mut().entity_mut(block).insert(HullDestroyed);
        let after_break = version(&app, a);
        assert!(after_break > after_place, "a block breaking bumps its ship");

        app.world_mut().entity_mut(block).despawn();
        assert!(version(&app, a) > after_break, "a block going bumps its ship");
        assert_eq!(version(&app, b), 0);
        assert!(app.world().resource::<LayoutEpoch>().0 >= 3);
    }

    #[test]
    fn moving_a_block_to_another_ship_bumps_both() {
        let mut app = app();
        let a = app.world_mut().spawn(Ship).id();
        let b = app.world_mut().spawn(LayoutVersion::default()).id();
        let block = app.world_mut().spawn((hull(), ChildOf(a))).id();
        let (va, vb) = (version(&app, a), version(&app, b));
        app.world_mut().entity_mut(block).insert(ChildOf(b));
        assert!(version(&app, a) > va, "the ship it left");
        assert!(version(&app, b) > vb, "the ship it joined");
    }

    #[test]
    fn things_that_are_not_blocks_do_not_bump() {
        let mut app = app();
        let a = app.world_mut().spawn(Ship).id();
        app.world_mut().spawn((Transform::default(), ChildOf(a)));
        assert_eq!(version(&app, a), 0);
    }
}
