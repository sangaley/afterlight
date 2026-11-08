//! What an enemy ship shows of itself, and what comes out when you open one.
//!
//! An enemy used to be a flat dark slab. Its hull carried no hallway cells, so
//! there was no inside to look at, and every block it did have drew at full
//! brightness whether or not you had any business seeing it.
//!
//! Now the deck is derived at spawn like the player's, and everything inboard
//! starts concealed: the ship reads as a closed hull until you breach it, and
//! then only the few cells around the hole come up. You learn a ship by taking
//! it apart, which is the same thing the salvage loop already asks you to do.
//!
//! Crew come out of the hole too. They are never given bodies while alive --
//! AI crew deliberately carry no Transform, which is what keeps them out of
//! every player-only walking, repair and burial system without a single
//! exclusion filter to maintain. A vented crewman is chosen by headcount, the
//! same trick `AmmoHitBehavior::Irradiate` uses, and the body is spawned at
//! the hole rather than at a position the living crewman never had.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use rand::Rng;

use super::components::{AiShip, OwnedByAiShip};
use crate::components::{
    BaseSpriteColor, CrewMember, HullDestroyed, HullSegment, Module,
};
use crate::events::{CrewDamageSource, CrewDamaged};

/// How much of its colour a concealed block keeps.
///
/// Not zero. The ship still has to read as a ship at a distance, and a hull
/// that draws pure black loses its silhouette against space entirely.
const CONCEAL: f32 = 0.22;

/// How far in, in cells, a hole lets you see. Chebyshev, so it opens a square.
const REVEAL_RADIUS: i32 = 2;

/// Chance that opening a cell takes someone with it.
const VENT_CHANCE: f64 = 0.4;

/// How hard a body leaves through a hole, before the ship's own motion.
const VENT_SPEED: f32 = 90.0;

/// An inboard block the player has not yet opened up. Carries the colour it
/// will go back to.
#[derive(Component)]
pub struct Concealed(pub Color);

/// Every cell of this ship that has been opened to space. Kept on the ship
/// rather than read off the wreckage, because a destroyed hull entity is
/// despawned shortly after it dies -- the same reason `AirField` remembers its
/// own openings instead of recomputing them from what is still standing.
#[derive(Component, Default)]
pub struct Breaches(pub HashSet<IVec2>);

fn dim(c: Color) -> Color {
    let s = c.to_srgba();
    Color::srgb(s.red * CONCEAL, s.green * CONCEAL, s.blue * CONCEAL)
}

/// The closed hull over a concealed cell: plating in the ship's paint
/// (ai_ship::livery), a shade darker than its outer skin -- the way a
/// Cosmoteer ship reads as a painted hull, not a black hole.
fn roof(paint: &super::livery::Livery, material: crate::components::HullMaterial, cell: IVec2, lo: IVec2, hi: IVec2) -> Color {
    let c = paint.paint(super::spawner::hull_tint(material), paint.accent_at(cell, lo, hi)).to_srgba();
    Color::srgb(c.red * 0.82, c.green * 0.82, c.blue * 0.82)
}

/// Conceals the inboard blocks of any AI ship that just spawned.
///
/// The inside isn't drawn dark any more: the deck shows as closed plating in
/// the faction's paint, and the machinery on it is kept out of sight until a
/// hole opens it (`reveal_breached_interior`).
///
/// Concealment lives in `BaseSpriteColor`, not in the live sprite colour,
/// because `tint_damaged_modules` rewrites every module's colour from that
/// base every frame. Darkening the sprite alone would be undone on the next
/// tick; darkening the base composes with damage tinting instead of fighting
/// it.
#[allow(clippy::type_complexity)]
pub fn conceal_new_ai_interiors(
    mut commands: Commands,
    all_hull: Query<(&HullSegment, &ChildOf), With<OwnedByAiShip>>,
    factions: Query<&super::components::AiShipType>,
    mut new_hull: Query<
        (Entity, &HullSegment, &ChildOf, &mut Sprite, &BaseSpriteColor),
        (Added<OwnedByAiShip>, Without<Module>),
    >,
    mut new_modules: Query<
        (Entity, &Module, &ChildOf, &mut Sprite, &BaseSpriteColor),
        (Added<OwnedByAiShip>, Without<HullSegment>),
    >,
) {
    if new_hull.is_empty() && new_modules.is_empty() {
        return;
    }

    // Which cells of which ship are inboard. A module standing on Outer hull is
    // on the skin -- a turret, a sensor mast -- and stays visible, because you
    // can see those from outside a real ship too.
    let mut inboard: HashMap<(Entity, IVec2), bool> = HashMap::new();
    // Each ship's extent, for its paint pattern.
    let mut extent: HashMap<Entity, (IVec2, IVec2)> = HashMap::new();
    for (hull, parent) in all_hull.iter() {
        inboard.insert(
            (parent.parent(), hull.grid_position),
            !matches!(hull.hull_layer, crate::components::HullLayer::Outer),
        );
        let e = extent.entry(parent.parent()).or_insert((IVec2::MAX, IVec2::MIN));
        *e = (e.0.min(hull.grid_position), e.1.max(hull.grid_position));
    }

    for (entity, hull, parent, mut sprite, base) in new_hull.iter_mut() {
        if hull.hull_layer == crate::components::HullLayer::Outer {
            continue;
        }
        let true_color = base.0;
        let hidden = match (factions.get(parent.parent()), extent.get(&parent.parent())) {
            (Ok(faction), Some(&(lo, hi))) => roof(&super::livery::livery(*faction), hull.material, hull.grid_position, lo, hi),
            _ => dim(true_color),
        };
        sprite.color = hidden;
        commands
            .entity(entity)
            .insert((Concealed(true_color), BaseSpriteColor(hidden)));
    }

    for (entity, module, parent, mut sprite, base) in new_modules.iter_mut() {
        // A plate sits outboard and has no hull cell of its own; absence from
        // the map means "not inboard", which is the right answer for it.
        if !inboard
            .get(&(parent.parent(), module.grid_position))
            .copied()
            .unwrap_or(false)
        {
            continue;
        }
        let true_color = base.0;
        // A fully transparent base is a module that draws no square of its own
        // and builds its shape from child sprites. Stamping a colour back over
        // it would paint a solid rectangle where a wedge should be -- the same
        // trap `mix_color` and the wreck tint both fell into.
        if true_color.to_srgba().alpha <= 0.01 {
            continue;
        }
        // Out of sight under the closed deck.
        let hidden = Color::NONE;
        sprite.color = hidden;
        commands
            .entity(entity)
            .insert((Concealed(true_color), BaseSpriteColor(hidden)));
    }
}

/// Records new holes, and puts whoever was standing there into space.
pub fn record_ai_breaches(
    mut commands: Commands,
    opened: Query<(&HullSegment, &ChildOf), (Added<HullDestroyed>, With<OwnedByAiShip>)>,
    ships: Query<(&GlobalTransform, Option<&crate::components::Velocity>), With<AiShip>>,
    mut breached: Query<&mut Breaches>,
    mut crew: Query<(Entity, &mut CrewMember), With<OwnedByAiShip>>,
    children: Query<&Children>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut dead: ResMut<crate::crew::burial::DriftingDead>,
    mut wounds: MessageWriter<CrewDamaged>,
) {
    let mut rng = rand::thread_rng();

    for (hull, parent) in opened.iter() {
        let ship = parent.parent();
        let cell = hull.grid_position;

        match breached.get_mut(ship) {
            Ok(mut b) => {
                b.0.insert(cell);
            }
            Err(_) => {
                let mut b = Breaches::default();
                b.0.insert(cell);
                commands.entity(ship).try_insert(b);
            }
        }

        if !rng.gen_bool(VENT_CHANCE) {
            continue;
        }

        let Ok((ship_gt, velocity)) = ships.get(ship) else { continue };
        let Ok(kids) = children.get(ship) else { continue };

        // By headcount, not by position: these people have no position. The
        // first living hand aboard is as good as any, and which one it was is
        // not something the player can see.
        let Some(victim) = kids
            .iter()
            .find(|c| crew.get(*c).map(|(_, m)| m.health > 0.0).unwrap_or(false))
        else {
            continue;
        };
        let Ok((entity, mut member)) = crew.get_mut(victim) else { continue };

        // Out through the hole it actually came from, in world space.
        let local = Vec3::new(cell.x as f32 * 66.0, cell.y as f32 * 66.0 - 33.0, 0.0);
        let world = ship_gt.transform_point(local);
        let outward = (world.truncate() - ship_gt.translation().truncate())
            .normalize_or_zero();
        let drift = velocity.map(|v| v.0).unwrap_or(Vec2::ZERO) + outward * VENT_SPEED;

        let id = dead.add(
            streaming.loaded_system.unwrap_or(u32::MAX),
            world.truncate(),
            drift,
            0.0,
            member.name.clone(),
        );
        commands.spawn((
            Sprite {
                color: Color::srgb(0.8, 0.6, 0.5),
                custom_size: Some(Vec2::new(16.0, 16.0)),
                ..default()
            },
            Transform::from_translation(world),
            crate::crew::burial::DriftingCorpse { id, velocity: drift, spin: 3.0 },
        ));

        // Report the wound and let `report_crew_deaths` raise the death, the
        // same as `crew_suction` does. Writing CrewDied here as well would kill
        // the same person twice and leave two bodies.
        wounds.write(CrewDamaged {
            crew: entity,
            amount: member.health,
            source: CrewDamageSource::Decompression,
        });
        member.health = 0.0;
    }
}

/// Brings up the blocks around each hole, and leaves the rest dark.
pub fn reveal_breached_interior(
    mut commands: Commands,
    breached: Query<(Entity, &Breaches), Changed<Breaches>>,
    mut blocks: Query<(
        Entity,
        &ChildOf,
        &Concealed,
        &mut Sprite,
        &mut BaseSpriteColor,
    )>,
    hull_cells: Query<&HullSegment>,
    modules: Query<&Module>,
) {
    for (ship, holes) in breached.iter() {
        for (entity, parent, concealed, mut sprite, mut base) in blocks.iter_mut() {
            if parent.parent() != ship {
                continue;
            }
            let cell = hull_cells
                .get(entity)
                .map(|h| h.grid_position)
                .or_else(|_| modules.get(entity).map(|m| m.grid_position));
            let Ok(cell) = cell else { continue };

            let near = holes.0.iter().any(|hole| {
                (hole.x - cell.x).abs() <= REVEAL_RADIUS
                    && (hole.y - cell.y).abs() <= REVEAL_RADIUS
            });
            if !near {
                continue;
            }
            sprite.color = concealed.0;
            base.0 = concealed.0;
            commands.entity(entity).remove::<Concealed>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{CrewState, HullLayer, Velocity};

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<crate::crew::burial::DriftingDead>()
            .init_resource::<crate::celestial::resources::SystemStreamingManager>()
            .add_message::<CrewDamaged>()
            .add_systems(
                Update,
                (
                    conceal_new_ai_interiors,
                    record_ai_breaches,
                    reveal_breached_interior,
                )
                    .chain(),
            );
        app
    }

    fn ship(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                AiShip,
                Velocity(Vec2::ZERO),
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id()
    }

    fn hull(app: &mut App, ship: Entity, cell: IVec2, layer: HullLayer) -> Entity {
        app.world_mut()
            .spawn((
                HullSegment {
                    grid_position: cell,
                    hull_layer: layer,
                    health: 100.0,
                    max_health: 100.0,
                    ..default()
                },
                Sprite::default(),
                BaseSpriteColor(Color::WHITE),
                OwnedByAiShip { root: ship },
                ChildOf(ship),
            ))
            .id()
    }

    fn hand(app: &mut App, ship: Entity, name: &str) -> Entity {
        app.world_mut()
            .spawn((
                CrewMember {
                    name: name.into(),
                    health: 100.0,
                    max_health: 100.0,
                    oxygen: 100.0,
                    morale: 100.0,
                    state: CrewState::Idle,
                },
                OwnedByAiShip { root: ship },
                ChildOf(ship),
            ))
            .id()
    }

    fn concealed(app: &App, e: Entity) -> bool {
        app.world().entity(e).get::<Concealed>().is_some()
    }

    /// Inboard decking starts dark. This is the whole premise: an enemy reads
    /// as a closed hull until you open it.
    #[test]
    fn decking_spawns_concealed() {
        let mut app = app();
        let s = ship(&mut app);
        let deck = hull(&mut app, s, IVec2::new(0, 0), HullLayer::Hallway);
        app.update();

        assert!(concealed(&app, deck), "a corridor was visible from outside");
        let base = app.world().entity(deck).get::<BaseSpriteColor>().unwrap().0;
        assert!(
            base.to_srgba().red < 0.5,
            "concealment must live in BaseSpriteColor, or tint_damaged_modules \
             paints it bright again on the next frame"
        );
    }

    /// The skin is the skin. You can see a hull plate from outside a real ship
    /// and you can see one here.
    #[test]
    fn the_outer_skin_is_never_concealed() {
        let mut app = app();
        let s = ship(&mut app);
        let skin = hull(&mut app, s, IVec2::new(0, 0), HullLayer::Outer);
        app.update();
        assert!(!concealed(&app, skin));
    }

    /// A hole shows you what is around it, and nothing else. The point of the
    /// mechanic is that you learn a ship by taking it apart.
    #[test]
    fn a_hole_reveals_only_its_surroundings() {
        let mut app = app();
        let s = ship(&mut app);
        let near = hull(&mut app, s, IVec2::new(1, 0), HullLayer::Hallway);
        let far = hull(&mut app, s, IVec2::new(9, 0), HullLayer::Hallway);
        let breached = hull(&mut app, s, IVec2::new(0, 0), HullLayer::Hallway);
        app.update();
        assert!(concealed(&app, near) && concealed(&app, far));

        app.world_mut().entity_mut(breached).insert(HullDestroyed);
        app.update();

        assert!(!concealed(&app, near), "the cell beside the hole stayed dark");
        assert!(
            concealed(&app, far),
            "a single hole lit the whole ship up, which defeats the point"
        );
    }

    /// Revealing restores the real colour, not an approximation of it.
    #[test]
    fn revealing_restores_the_true_colour() {
        let mut app = app();
        let s = ship(&mut app);
        let deck = hull(&mut app, s, IVec2::new(1, 0), HullLayer::Hallway);
        let hole = hull(&mut app, s, IVec2::new(0, 0), HullLayer::Hallway);
        app.update();
        app.world_mut().entity_mut(hole).insert(HullDestroyed);
        app.update();

        let base = app.world().entity(deck).get::<BaseSpriteColor>().unwrap().0;
        assert_eq!(base, Color::WHITE);
    }

    /// Opening a ship puts people into space.
    ///
    /// Probabilistic by design -- not every hole finds someone -- so this opens
    /// thirty of them. At a 0.4 chance each, thirty misses is about one run in
    /// five million.
    #[test]
    fn breaches_vent_crew_into_space() {
        let mut app = app();
        let s = ship(&mut app);
        for i in 0..30 {
            hand(&mut app, s, &format!("Hand{i}"));
        }
        let holes: Vec<Entity> = (0..30)
            .map(|i| hull(&mut app, s, IVec2::new(i, 20), HullLayer::Outer))
            .collect();
        app.update();

        for h in holes {
            app.world_mut().entity_mut(h).insert(HullDestroyed);
            app.update();
        }

        let bodies = app.world().resource::<crate::crew::burial::DriftingDead>().bodies.len();
        assert!(bodies > 0, "thirty holes and nobody came out");

        let dead = app
            .world_mut()
            .query::<&CrewMember>()
            .iter(app.world())
            .filter(|c| c.health <= 0.0)
            .count();
        assert_eq!(
            dead, bodies,
            "one body per death, or the roster and the register disagree"
        );
    }

    /// A ship with nobody left aboard must not keep producing bodies.
    #[test]
    fn an_empty_ship_vents_nobody() {
        let mut app = app();
        let s = ship(&mut app);
        let holes: Vec<Entity> = (0..20)
            .map(|i| hull(&mut app, s, IVec2::new(i, 20), HullLayer::Outer))
            .collect();
        app.update();
        for h in holes {
            app.world_mut().entity_mut(h).insert(HullDestroyed);
            app.update();
        }
        assert_eq!(
            app.world().resource::<crate::crew::burial::DriftingDead>().bodies.len(),
            0
        );
    }
}
