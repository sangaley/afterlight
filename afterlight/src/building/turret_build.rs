//! Building on a turret deck.
//!
//! PageUp lifts build mode onto the turret decks, and again onto the gun
//! house roofs over them (combat::gun_house); PageDown steps back down. While building, every turret stands squared to the hull
//! (combat::turret_deck::traverse_turrets), so a turret's cell (x, y) sits
//! exactly over hull cell `centre + (x, y)`: the ghost, the cursor and the
//! click all work in hull cells as they always have, and only the checks and
//! the request change. A block placed up there is an ordinary module with
//! `ModuleExtras.turret` set, the same as a design or a save lays one down.

use bevy::prelude::*;

use super::blueprint::ModuleExtras;
use super::footprints;
use super::registry::ModuleRegistry;
use super::GridOccupancy;
use crate::combat::turret_deck::{self, TurretBody};
use crate::components::{DestroyedModule, Module, ModuleCategory, ModuleType, Ship};
use crate::events::{NotificationType, PlaceModuleRequest, RemoveModuleRequest, ShowNotification};
use crate::resources::{BuildDeck, BuildSelection, BuildingState, Currency};
use crate::states::BuildState;

/// Can this part stand on a turret deck? Gun parts can; what feeds a gun
/// from the ship's magazines, and what a turret turns on, stays below.
pub fn allowed_on_turret(t: ModuleType) -> bool {
    // A laser's emitter and lenses turn with a turret; its bank stays below.
    if matches!(t, ModuleType::LaserEmitter | ModuleType::LaserLens) {
        return true;
    }
    t.category() == ModuleCategory::Guns
        && !matches!(
            t,
            ModuleType::ShellFeeder
                | ModuleType::Magazine
                | ModuleType::BeltConveyor
                | ModuleType::TraverseMotor
                | ModuleType::AmmoHoist
                | ModuleType::AntiFlashDoor
        )
        && !turret_deck::is_turret_ring(t)
        && !crate::combat::gun_house::is_roof_block(t)
}

/// Can this stand on a turret's roof? Armour plates, angle pieces and the
/// roof fittings.
pub fn allowed_on_roof(t: ModuleType) -> bool {
    crate::combat::gun_house::is_roof_block(t)
}

/// A barrel reaches out past its turret's deck (and is no cover there --
/// combat::new_projectiles); everything else stands on the deck.
pub fn may_overhang(t: ModuleType) -> bool {
    matches!(t, ModuleType::GunBarrel | ModuleType::MuzzleBrake)
}

/// One of the player's turrets, as build mode sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeckUnder {
    pub body: Entity,
    pub ring_cell: IVec2,
    /// The ring's centre, in hull cells: the turret's (0, 0).
    pub centre: IVec2,
    pub size: i32,
}

/// The player's turret whose deck covers this hull cell -- the nearest, if
/// two decks overlap.
pub(crate) fn deck_under(cell: IVec2, decks: &[DeckUnder]) -> Option<DeckUnder> {
    decks
        .iter()
        .filter(|d| on_deck(cell - d.centre, d.size))
        .min_by_key(|d| (cell - d.centre).length_squared())
        .copied()
}

pub(crate) fn on_deck(local: IVec2, size: i32) -> bool {
    local.as_vec2().length() <= turret_deck::deck_radius(size)
}

pub(crate) fn player_decks(
    player: Entity,
    bodies: &Query<(Entity, &TurretBody, &ChildOf, Option<&Children>)>,
    rings: &Query<&Module, Without<DestroyedModule>>,
) -> Vec<DeckUnder> {
    bodies
        .iter()
        .filter(|(_, _, p, _)| p.parent() == player)
        .filter_map(|(body, t, _, _)| {
            let ring = rings.get(t.ring).ok()?;
            Some(DeckUnder { body, ring_cell: t.ring_cell, centre: turret_deck::ring_centre(ring).0, size: t.size })
        })
        .collect()
}

/// PageUp: up a level (hull, turret decks, roofs). PageDown: down one.
pub fn switch_build_deck(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<BuildState>>,
    mut build_state: ResMut<BuildingState>,
    ship: Query<Entity, With<Ship>>,
    bodies: Query<(Entity, &TurretBody, &ChildOf, Option<&Children>)>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if *current_state.get() == BuildState::Inactive {
        // Build mode always opens on the hull.
        build_state.deck = BuildDeck::Hull;
        return;
    }
    let want = if keyboard.just_pressed(KeyCode::PageUp) {
        match build_state.deck {
            BuildDeck::Hull => BuildDeck::Turret,
            _ => BuildDeck::Roof,
        }
    } else if keyboard.just_pressed(KeyCode::PageDown) {
        match build_state.deck {
            BuildDeck::Roof => BuildDeck::Turret,
            _ => BuildDeck::Hull,
        }
    } else {
        return;
    };
    if want == build_state.deck {
        return;
    }
    let Ok(player) = ship.single() else { return };
    if want != BuildDeck::Hull && !bodies.iter().any(|(_, _, p, _)| p.parent() == player) {
        info!("Build deck: no turret on the player's ship");
        notifications.write(ShowNotification {
            message: "No turrets yet: lay a Turret Ring on the hull deck first.".into(),
            notification_type: NotificationType::Info,
            duration: 2.5,
        });
        return;
    }
    build_state.deck = want;
    build_state.auto_rotated = false;
    info!("Build deck: {:?}", want);
    notifications.write(ShowNotification {
        message: match want {
            BuildDeck::Turret => "Turret deck (PgUp: gun house roof, PgDn: hull)".into(),
            BuildDeck::Roof => "Gun house roof: plates, angle pieces, fittings (PgDn: turret deck)".into(),
            BuildDeck::Hull => "Hull deck".into(),
        },
        notification_type: NotificationType::Info,
        duration: 1.5,
    });
}

/// On the turret deck, judge the ghost against the turret under it instead
/// of the hull. Runs after `update_ghost_preview` and overrides its verdict.
pub fn validate_turret_ghost(
    current_state: Res<State<BuildState>>,
    mut build_state: ResMut<BuildingState>,
    ship: Query<Entity, With<Ship>>,
    bodies: Query<(Entity, &TurretBody, &ChildOf, Option<&Children>)>,
    rings: Query<&Module, Without<DestroyedModule>>,
    parts: Query<&Module, Without<DestroyedModule>>,
    player_modules: Query<(), (With<Module>, Without<crate::ai_ship::components::OwnedByAiShip>)>,
    player_hull: Query<&ChildOf, With<crate::components::HullSegment>>,
    registry: Res<ModuleRegistry>,
    currency: Res<Currency>,
) {
    if build_state.deck == BuildDeck::Hull
        || *current_state.get() != BuildState::Placing
        || !build_state.cursor_on_grid
    {
        return;
    }
    let roof = build_state.deck == BuildDeck::Roof;
    let Ok(player) = ship.single() else { return };
    let decks = player_decks(player, &bodies, &rings);
    let verdict = (|| -> Result<(), String> {
        let deck = deck_under(build_state.ghost_position, &decks).ok_or("Not over a turret")?;
        let mt = match build_state.current_selection() {
            BuildSelection::Module(mt) => mt,
            BuildSelection::Hull(_) => return Err("A turret's plating comes with its ring".into()),
        };
        if roof && !allowed_on_roof(mt) {
            return Err(format!("{} doesn't go on a roof: plates, angle pieces and roof fittings do", mt.name()));
        }
        if !roof && !allowed_on_turret(mt) {
            return Err(if allowed_on_roof(mt) {
                format!("{} goes on the roof (PgUp)", mt.name())
            } else {
                format!("{} goes on the hull deck", mt.name())
            });
        }
        let def = registry.get(mt);
        let local = build_state.ghost_position - deck.centre;
        let cells = GridOccupancy::cells_for(local, def.size, build_state.rotation, footprints::footprint_override(mt));
        if !may_overhang(mt) && !cells.iter().all(|c| on_deck(*c, deck.size)) {
            return Err("Off the edge of the turret".into());
        }
        let taken = turret_cells(deck.body, &bodies, &parts, roof);
        if cells.iter().any(|c| taken.contains(c)) {
            return Err(if roof { "Overlaps the roof here".into() } else { "Overlaps a part on this turret".into() });
        }
        let hull = player_hull.iter().filter(|p| p.parent() == player).count();
        if player_modules.iter().count() + hull >= crate::combat::limits::MAX_SHIP_BLOCKS {
            return Err(format!("Block limit reached ({})", crate::combat::limits::MAX_SHIP_BLOCKS));
        }
        if currency.credits < def.cost {
            return Err(format!("Not enough credits (need {}c)", def.cost));
        }
        Ok(())
    })();
    build_state.is_valid_placement = verdict.is_ok();
    build_state.placement_reason = verdict.err();
}

/// The turret-local cells a turret's parts stand on -- or, `roof`, its roof
/// blocks: the two levels don't get in each other's way.
pub(crate) fn turret_cells(
    body: Entity,
    bodies: &Query<(Entity, &TurretBody, &ChildOf, Option<&Children>)>,
    parts: &Query<&Module, Without<DestroyedModule>>,
    roof: bool,
) -> Vec<IVec2> {
    let Ok((_, _, _, Some(children))) = bodies.get(body) else { return Vec::new() };
    children
        .iter()
        .filter_map(|c| parts.get(c).ok())
        .filter(|m| crate::combat::gun_house::is_roof_block(m.module_type) == roof)
        .flat_map(|m| {
            GridOccupancy::cells_for(m.grid_position, m.size, m.rotation, footprints::footprint_override(m.module_type))
        })
        .collect()
}

/// Click to place on the turret deck, right-click (or the delete tool) to take
/// a part off it.
pub fn turret_deck_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    current_state: Res<State<BuildState>>,
    build_state: Res<BuildingState>,
    ship: Query<Entity, With<Ship>>,
    bodies: Query<(Entity, &TurretBody, &ChildOf, Option<&Children>)>,
    rings: Query<&Module, Without<DestroyedModule>>,
    parts: Query<(Entity, &Module), Without<DestroyedModule>>,
    mut place: MessageWriter<PlaceModuleRequest>,
    mut remove: MessageWriter<RemoveModuleRequest>,
) {
    if build_state.deck == BuildDeck::Hull || !build_state.cursor_on_grid {
        return;
    }
    let roof = build_state.deck == BuildDeck::Roof;
    let state = *current_state.get();
    let Ok(player) = ship.single() else { return };
    let decks = player_decks(player, &bodies, &rings);
    let Some(deck) = deck_under(build_state.ghost_position, &decks) else { return };
    let local = build_state.ghost_position - deck.centre;

    let removing = (state == BuildState::Deleting && mouse.just_pressed(MouseButton::Left))
        || (state == BuildState::Placing && mouse.just_pressed(MouseButton::Right));
    if removing {
        let Ok((_, _, _, Some(children))) = bodies.get(deck.body) else { return };
        let hit = children
            .iter()
            .filter_map(|c| parts.get(c).ok())
            .filter(|(_, m)| crate::combat::gun_house::is_roof_block(m.module_type) == roof)
            .find(|(_, m)| {
                GridOccupancy::cells_for(m.grid_position, m.size, m.rotation, footprints::footprint_override(m.module_type))
                    .contains(&local)
            });
        if let Some((entity, _)) = hit {
            remove.write(RemoveModuleRequest { module: entity });
        }
        return;
    }

    if state == BuildState::Placing && mouse.just_pressed(MouseButton::Left) && build_state.is_valid_placement {
        let BuildSelection::Module(module_type) = build_state.current_selection() else { return };
        place.write(PlaceModuleRequest {
            module_type,
            grid_position: local,
            rotation: build_state.rotation,
            custom_name: None,
            subcomponents: None,
            extras: Some(ModuleExtras { turret: Some(deck.ring_cell), ..default() }),
            free: false,
        });
    }
}

/// Building on the hull, the turrets are lifted out of the way so the deck
/// under them can be seen; on the turret deck they are there to build on.
#[allow(clippy::type_complexity)]
pub fn show_turrets_for_deck(
    current_state: Res<State<BuildState>>,
    build_state: Res<BuildingState>,
    mut bodies: Query<&mut Visibility, (With<TurretBody>, Without<turret_deck::OnTurret>)>,
    mut roofs: Query<(&mut Visibility, &Module), With<turret_deck::OnTurret>>,
) {
    let building = *current_state.get() != BuildState::Inactive;
    let hide = building && build_state.deck == BuildDeck::Hull;
    let want = if hide { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in bodies.iter_mut() {
        if *v != want {
            *v = want;
        }
    }
    // Building on a turret's deck, its roof comes off so the parts under it
    // can be seen.
    let house = if building && build_state.deck == BuildDeck::Turret { Visibility::Hidden } else { Visibility::Inherited };
    for (mut v, m) in roofs.iter_mut() {
        if crate::combat::gun_house::is_roof_block(m.module_type) && *v != house {
            *v = house;
        }
    }
}

#[cfg(test)]
mod turret_build_tests {
    use super::*;

    fn deck(centre: IVec2, size: i32) -> DeckUnder {
        DeckUnder { body: Entity::PLACEHOLDER, ring_cell: centre, centre, size }
    }

    #[test]
    fn only_gun_parts_that_turn_with_a_turret_go_on_one() {
        assert!(allowed_on_turret(ModuleType::FiringPiece));
        assert!(allowed_on_turret(ModuleType::GunBarrel));
        assert!(allowed_on_turret(ModuleType::Loader));
        assert!(!allowed_on_turret(ModuleType::Magazine));
        assert!(!allowed_on_turret(ModuleType::TurretRing));
        assert!(!allowed_on_turret(ModuleType::TraverseMotor));
        assert!(!allowed_on_turret(ModuleType::Railgun), "only parts, not whole guns");
    }

    #[test]
    fn the_deck_reaches_a_cell_past_the_ring_and_no_further() {
        let d = [deck(IVec2::new(10, 0), 3)];
        assert!(deck_under(IVec2::new(10, 0), &d).is_some());
        assert!(deck_under(IVec2::new(12, 0), &d).is_some(), "ring edge + 1");
        assert!(deck_under(IVec2::new(13, 0), &d).is_none());
        assert!(deck_under(IVec2::new(12, 2), &d).is_none(), "corners are cut");
    }

    #[test]
    fn where_two_decks_overlap_the_nearer_turret_wins() {
        let d = [deck(IVec2::new(0, 0), 3), deck(IVec2::new(4, 0), 3)];
        assert_eq!(deck_under(IVec2::new(1, 0), &d).unwrap().centre, IVec2::ZERO);
        assert_eq!(deck_under(IVec2::new(3, 0), &d).unwrap().centre, IVec2::new(4, 0));
    }
}
