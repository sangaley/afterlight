//! Two systems queue commands in the same frame: one files a ship away, the
//! other hands it a shield. Which goes first depends on how the frame happens to
//! be scheduled, so the loser's target is sometimes already gone. Nothing is
//! wrong when that happens, and Bevy's default is to end the game over it.

use bevy::ecs::error::{match_severity, BevyError, ErrorContext};
use bevy::ecs::world::error::EntityMutableFetchError;
use bevy::prelude::*;

use crate::ai_ship::components::OwnedByAiShip;
use crate::combat::shields::ShieldBubble;
use crate::vfx::block_height::HeightShadow;
use crate::vfx::far_art::FarArtSprite;
use crate::vfx::ship_roof::{AiRoofTile, RoofTile};

/// A command whose entity is already gone is skipped and logged. Anything else
/// goes the way Bevy would send it, panics included.
pub fn survive_despawned(err: BevyError, ctx: ErrorContext) {
    if matches!(ctx, ErrorContext::Command { .. }) {
        if let Some(EntityMutableFetchError::NotSpawned(gone)) = err.downcast_ref::<EntityMutableFetchError>() {
            warn!("Skipped command `{}`: {gone}", ctx.name());
            return;
        }
    }
    match_severity(err, ctx);
}

/// What the same race leaves behind: a thing spawned as a child of a ship that
/// was gone by then loses its parent and stays where it is, a roof tile hanging
/// in space near the middle of the system, a crewman nobody counts.
#[allow(clippy::type_complexity)]
pub fn reap_stranded(
    mut commands: Commands,
    drawn: Query<
        Entity,
        (
            Or<(With<RoofTile>, With<AiRoofTile>, With<HeightShadow>, With<ShieldBubble>, With<FarArtSprite>)>,
            Without<ChildOf>,
        ),
    >,
    owned: Query<(Entity, &OwnedByAiShip), Without<ChildOf>>,
    alive: Query<()>,
) {
    for stranded in drawn.iter() {
        warn!("Removed {stranded}: its ship was gone before it was attached");
        commands.entity(stranded).try_despawn();
    }
    for (stranded, owner) in owned.iter() {
        // PLACEHOLDER: a tow beam marks itself as an AI ship's to dodge player
        // systems, and belongs to no ship at all.
        if owner.root != Entity::PLACEHOLDER && !alive.contains(owner.root) {
            warn!("Removed {stranded}: its ship was gone before it was attached");
            commands.entity(stranded).try_despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_ship::components::{AiShip, AiShipType};
    use crate::combat::shields::{attach_ai_shields, ShipShield};
    use crate::components::HullSegment;

    #[derive(Component)]
    struct Marker;

    /// The crash itself: the command arrives after the entity has gone.
    #[test]
    fn a_command_on_a_gone_entity_is_skipped() {
        let mut app = App::new();
        app.set_error_handler(survive_despawned);
        let gone = app.world_mut().spawn_empty().id();
        app.add_systems(Update, move |mut commands: Commands| {
            commands.entity(gone).despawn();
            commands.entity(gone).insert(Marker);
        });
        app.update();
        assert!(app.world().get_entity(gone).is_err());
    }

    /// Only that: other command errors are still real errors.
    #[test]
    #[should_panic]
    fn other_errors_still_panic() {
        survive_despawned(BevyError::from("something else"), ErrorContext::Command { name: "test".into() });
    }

    /// The race that was caught, on Bevy's default handler so it can't hide
    /// behind `survive_despawned`: a ship filed away the frame it's due a shield.
    #[test]
    fn a_ship_gone_the_frame_it_gets_its_shield_is_no_crash() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        let ship = app.world_mut().spawn((AiShip, AiShipType::BrokenChoir, Transform::default(), Visibility::default())).id();
        app.world_mut().spawn((HullSegment::default(), Transform::default(), ChildOf(ship)));
        let file_away = move |mut commands: Commands| commands.entity(ship).try_despawn();
        app.add_systems(Update, (file_away, attach_ai_shields).chain_ignore_deferred());
        app.update();
        assert!(app.world().get_entity(ship).is_err());
    }

    /// And a ship still there does get one.
    #[test]
    fn a_ship_still_there_gets_its_shield() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        let ship = app.world_mut().spawn((AiShip, AiShipType::BrokenChoir, Transform::default(), Visibility::default())).id();
        app.world_mut().spawn((HullSegment::default(), Transform::default(), ChildOf(ship)));
        app.add_systems(Update, attach_ai_shields);
        app.update();
        assert!(app.world().get::<ShipShield>(ship).is_some());
    }

    #[test]
    fn what_a_gone_ship_left_stranded_is_removed() {
        let mut app = App::new();
        app.add_systems(Update, reap_stranded);
        let dead = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(dead);
        let tile = app.world_mut().spawn(AiRoofTile(1.0)).id();
        let crew = app.world_mut().spawn(OwnedByAiShip { root: dead }).id();
        app.update();
        app.update();
        assert!(app.world().get_entity(tile).is_err(), "a roof tile hung in space");
        assert!(app.world().get_entity(crew).is_err(), "a crewman outlived his ship");
    }

    #[test]
    fn what_belongs_somewhere_is_left_alone() {
        let mut app = App::new();
        app.add_systems(Update, reap_stranded);
        let ship = app.world_mut().spawn_empty().id();
        let tile = app.world_mut().spawn((AiRoofTile(1.0), ChildOf(ship))).id();
        let crew = app.world_mut().spawn((OwnedByAiShip { root: ship }, ChildOf(ship))).id();
        // A section's blocks can sit parentless a moment; their ship is alive.
        let block = app.world_mut().spawn(OwnedByAiShip { root: ship }).id();
        let beam = app.world_mut().spawn(OwnedByAiShip { root: Entity::PLACEHOLDER }).id();
        app.update();
        app.update();
        for kept in [tile, crew, block, beam] {
            assert!(app.world().get_entity(kept).is_ok());
        }
    }
}
