//! Towing a hulk home.
//!
//! Stripping a wreck where it died already works: crew suit up, cross over,
//! and carry it back a piece at a time. That stays. Towing is the other
//! option, and the point is that it is a *choice* rather than an upgrade.
//!
//! Strip in place and you get what a boarding party can carry — scrap and
//! common goods, quickly, and you leave whenever you like. Drag the whole
//! hulk to a station and the yard opens it properly: everything still aboard,
//! plus the rare classes a crewman cannot pry loose and carry home through
//! vacuum.
//!
//! The cost is the trip. A hulk under tow adds its mass to yours, so you
//! accelerate and turn like something much heavier, and it makes noise, which
//! is already what draws hunters. Scavengers are meanwhile eating whatever you
//! left behind. One at a time.

use bevy::prelude::*;
use rand::Rng;

use crate::ai_ship::components::AiShipWreck;
use crate::components::{Module, ModuleType, Ship, ShipPhysics, Velocity, Wreck};
use crate::events::{NotificationType, ShowNotification};
use crate::resources::{Inventory, ItemType, NoiseState};
use crate::states::GameState;

/// How close the ship must be to latch on.
const LATCH_RANGE: f32 = 900.0;

/// Where the hulk rides, behind the ship along its facing.
const TOW_OFFSET: f32 = 620.0;

/// How quickly the hulk settles into its tow position. Low, so it swings
/// rather than snapping — it should feel dragged, not welded.
const TOW_FOLLOW: f32 = 2.2;

/// Mass a towed hulk adds, per unit of loot still aboard. Everything in
/// `ship_movement` divides force by `physics.mass`, so this costs acceleration
/// and turning without touching the movement code at all.
const MASS_PER_LOOT: f32 = 220.0;

/// Noise a tow adds. Noise already draws hunters, which is the point: you are
/// slow and loud at the same time.
const TOW_NOISE: f32 = 45.0;

/// The hulk currently under tow, and what the ship weighed before it.
#[derive(Resource, Default)]
pub struct Towing {
    pub hulk: Option<Entity>,
    base_mass: Option<f32>,
}

/// Latch onto or release the nearest hulk. Requires a Tractor Beam aboard —
/// the module has existed in the registry since long before this, described as
/// "Pulls objects toward ship. Salvage.", and did nothing at all.
#[allow(clippy::type_complexity)]
fn toggle_tow(
    keys: Res<ButtonInput<KeyCode>>,
    mut tow: ResMut<Towing>,
    ship: Query<(&GlobalTransform, &ShipPhysics), With<Ship>>,
    // Without<OwnedByAiShip>: the player's own beam, not anyone's. Stellar
    // Preserve hulls carry two TractorBeams each, so an unscoped count let
    // you latch a hulk with no beam aboard as long as a Preserve ship was
    // somewhere in the system.
    modules: Query<
        (&Module, Option<&crate::diplomacy::signatures::Signature>),
        (
            Without<crate::components::DestroyedModule>,
            Without<crate::ai_ship::components::OwnedByAiShip>,
        ),
    >,
    wrecks: Query<(Entity, &GlobalTransform, &AiShipWreck)>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    // Y, not T: T is hold-to-free-look. Y's only other binding is build-mode
    // symmetry, which cannot be active while flying.
    if !keys.just_pressed(KeyCode::KeyY) {
        return;
    }
    let Ok((ship_gt, _)) = ship.single() else { return };

    if tow.hulk.is_some() {
        tow.hulk = None;
        notifications.write(ShowNotification {
            message: "Tow released.".into(),
            notification_type: NotificationType::Info,
            duration: 3.0,
        });
        return;
    }

    // The best beam aboard sets the reach: a signature beam (diplomacy::
    // signatures) latches from further out.
    let reach = modules
        .iter()
        .filter(|(m, _)| m.module_type == ModuleType::TractorBeam && m.is_active)
        .map(|(_, sig)| sig.map_or(1.0, |s| s.0.output))
        .reduce(f32::max);
    let has_beam = reach.is_some();
    let latch = LATCH_RANGE * reach.unwrap_or(1.0);
    if !has_beam {
        notifications.write(ShowNotification {
            message: "No tractor beam aboard, or it has no power.".into(),
            notification_type: NotificationType::Warning,
            duration: 4.0,
        });
        return;
    }

    let ship_pos = ship_gt.translation().truncate();
    let nearest = wrecks
        .iter()
        .map(|(e, gt, w)| (e, gt.translation().truncate().distance(ship_pos), w))
        .filter(|(_, d, _)| *d < latch)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    match nearest {
        Some((entity, _, wreck)) => {
            tow.hulk = Some(entity);
            notifications.write(ShowNotification {
                message: format!(
                    "Tow latched. {} hulk under tow - heavy and loud. Bring it to a station.",
                    crate::ai_ship::components::faction_display_name(wreck.ship_type)
                ),
                notification_type: NotificationType::Success,
                duration: 6.0,
            });
        }
        None => {
            notifications.write(ShowNotification {
                message: "Nothing in range to tow.".into(),
                notification_type: NotificationType::Warning,
                duration: 3.0,
            });
        }
    }
}

/// Tell the player, once, that a hulk can come home whole.
///
/// Nothing in the game mentioned towing. The wreck notification offers "F:
/// salvage detail" and stops there, the starter ship carries no tractor beam,
/// and the key is Y -- so the entire mechanic was reachable only by reading
/// the source. Two different hints, because the answer depends on what is
/// aboard: with a beam, say which key; without one, say what to buy. Each
/// fires once per run.
fn hint_towing(
    tow: Res<Towing>,
    ship: Query<&GlobalTransform, With<Ship>>,
    modules: Query<
        &Module,
        (
            Without<crate::components::DestroyedModule>,
            Without<crate::ai_ship::components::OwnedByAiShip>,
        ),
    >,
    wrecks: Query<&GlobalTransform, With<AiShipWreck>>,
    mut notifications: MessageWriter<ShowNotification>,
    mut shown: Local<(bool, bool)>,
) {
    if tow.hulk.is_some() || (shown.0 && shown.1) {
        return;
    }
    let Ok(ship_gt) = ship.single() else { return };
    let ship_pos = ship_gt.translation().truncate();

    let in_range = wrecks
        .iter()
        .any(|gt| gt.translation().truncate().distance(ship_pos) < LATCH_RANGE);
    if !in_range {
        return;
    }

    let has_beam = modules
        .iter()
        .any(|m| m.module_type == ModuleType::TractorBeam && m.is_active);

    let (seen, message) = if has_beam {
        (
            &mut shown.0,
            "Y: tow this hulk to a station. The yard opens it properly - everything \
             aboard, plus what a boarding party cannot carry. It is heavy and loud."
                .to_string(),
        )
    } else {
        (
            &mut shown.1,
            "A tractor beam would let you tow this hulk home whole, for the classes \
             a boarding party cannot carry. Fitted from the Weapons tab at any dock."
                .to_string(),
        )
    };
    if *seen {
        return;
    }
    *seen = true;
    notifications.write(ShowNotification {
        message,
        notification_type: NotificationType::Info,
        duration: 9.0,
    });
}

/// Drag the hulk along behind, and make the ship feel it.
#[allow(clippy::type_complexity)]
fn drag_hulk(
    time: Res<Time>,
    mut tow: ResMut<Towing>,
    mut ship: Query<(&GlobalTransform, &mut ShipPhysics), With<Ship>>,
    mut hulks: Query<(&mut Transform, &AiShipWreck), Without<Ship>>,
    mut noise: ResMut<NoiseState>,
) {
    let Ok((ship_gt, mut physics)) = ship.single_mut() else { return };

    let Some(hulk) = tow.hulk else {
        // Give the ship its own weight back.
        if let Some(base) = tow.base_mass.take() {
            physics.mass = base;
        }
        return;
    };

    let Ok((mut hulk_tf, wreck)) = hulks.get_mut(hulk) else {
        // It was destroyed or despawned underneath us.
        tow.hulk = None;
        if let Some(base) = tow.base_mass.take() {
            physics.mass = base;
        }
        return;
    };

    let base = *tow.base_mass.get_or_insert(physics.mass);
    physics.mass = base + wreck.loot_remaining as f32 * MASS_PER_LOOT;
    noise.noise_level += TOW_NOISE * time.delta_secs();

    let ship_pos = ship_gt.translation().truncate();
    let facing = ship_gt.compute_transform().rotation;
    let behind = ship_pos - (facing * Vec3::Y).truncate().normalize_or_zero() * TOW_OFFSET;

    let here = hulk_tf.translation.truncate();
    let next = here + (behind - here) * (time.delta_secs() * TOW_FOLLOW).min(1.0);
    hulk_tf.translation.x = next.x;
    hulk_tf.translation.y = next.y;
}

/// What a hulk is worth when the yard opens it, as opposed to what a boarding
/// party can carry off it.
///
/// Everything still aboard comes out, and on top of that the classes a crewman
/// cannot pry loose and haul home through vacuum. Those rare items are the
/// reason to take the slow road; later they are what better weapons and an
/// upgrade path should be built out of.
pub fn hulk_yard_value(wreck: &AiShipWreck, rng: &mut impl Rng) -> Vec<ItemType> {
    let mut out = Vec::new();
    for _ in 0..wreck.loot_remaining {
        out.push(match rng.gen_range(0..100) {
            0..=54 => ItemType::ScrapMetal,
            55..=74 => ItemType::Crystal,
            75..=89 => ItemType::FuelCell,
            _ => ItemType::BioSample,
        });
    }

    // The rare tier scales with how intact the hull was when it died, so the
    // way you killed it still decides the harvest. A clean kill that struck
    // its colours yields more than a reactor breach that gutted half of it.
    let rare = ((wreck.loot_remaining as f32 * 0.35) * wreck.intact_frac).round() as u32;
    for _ in 0..rare.max(1) {
        out.push(if rng.gen::<f32>() < 0.25 {
            ItemType::AncientArtifact
        } else {
            ItemType::RareAlloy
        });
    }
    out
}

/// Arriving at a station with a hulk in tow opens it.
fn process_hulk_on_dock(
    mut tow: ResMut<Towing>,
    mut commands: Commands,
    hulks: Query<&AiShipWreck>,
    mut inventory: ResMut<Inventory>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let Some(hulk) = tow.hulk.take() else { return };
    let Ok(wreck) = hulks.get(hulk) else { return };

    let mut rng = rand::thread_rng();
    let haul = hulk_yard_value(wreck, &mut rng);
    let (recovered, rare) = load_haul(&haul, &mut inventory);
    let lost = haul.len() - recovered;
    let name = crate::ai_ship::components::faction_display_name(wreck.ship_type);

    // add_item refuses what the hold cannot take, and this used to ignore
    // that and announce the whole haul as recovered.
    let (message, notification_type) = if lost == 0 {
        (
            format!(
                "{} hulk broken up: {} units recovered, {} of them rare. Nothing a boarding party could have carried.",
                name, recovered, rare
            ),
            NotificationType::Success,
        )
    } else {
        (
            format!(
                "{} hulk broken up: {} units recovered ({} rare), {} lost - the hold is full.",
                name, recovered, rare, lost
            ),
            NotificationType::Warning,
        )
    };
    notifications.write(ShowNotification { message, notification_type, duration: 9.0 });

    commands.entity(hulk).try_despawn();
}

fn is_rare(item: &ItemType) -> bool {
    matches!(item, ItemType::RareAlloy | ItemType::AncientArtifact)
}

/// Stows what the hold has room for, rare items first: they are what the tow
/// was for, so a nearly full hold should turn away the scrap, not them.
/// Returns (units stowed, rare units stowed).
fn load_haul(haul: &[ItemType], inventory: &mut Inventory) -> (usize, usize) {
    let (rare, common): (Vec<ItemType>, Vec<ItemType>) = haul.iter().partition(|i| is_rare(i));
    let mut recovered = 0;
    let mut rare_recovered = 0;
    for item in rare.iter().chain(&common) {
        if inventory.add_item(*item, 1) {
            recovered += 1;
            if is_rare(item) {
                rare_recovered += 1;
            }
        }
    }
    (recovered, rare_recovered)
}

fn release_on_exit(mut tow: ResMut<Towing>) {
    tow.hulk = None;
}

pub struct TowingPlugin;

impl Plugin for TowingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Towing>()
            .add_systems(
                Update,
                (toggle_tow, drag_hulk, hint_towing)
                    .chain()
                    .run_if(in_state(GameState::Exploring)),
            )
            .add_systems(OnEnter(GameState::StationDocked), process_hulk_on_dock)
            .add_systems(OnEnter(GameState::MainMenu), release_on_exit)
            .add_systems(OnEnter(GameState::GameOver), release_on_exit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_ship::components::AiShipType;

    fn hulk(loot: u32, intact: f32) -> AiShipWreck {
        AiShipWreck { ship_type: AiShipType::TerranHegemony, loot_remaining: loot, intact_frac: intact }
    }

    /// Towing must yield the rare classes, because that is the entire reason
    /// to take the slow, loud, heavy road instead of stripping and leaving.
    #[test]
    fn the_yard_gives_what_a_boarding_party_cannot() {
        let mut rng = rand::thread_rng();
        let haul = hulk_yard_value(&hulk(10, 1.0), &mut rng);
        let rare = haul
            .iter()
            .filter(|i| matches!(i, ItemType::RareAlloy | ItemType::AncientArtifact))
            .count();
        assert!(rare > 0, "a towed hulk produced nothing rare, so towing is pointless");
        assert!(haul.len() > 10, "the yard returned less than was aboard");
    }

    /// How you killed it still decides the harvest. A gutted hulk must be
    /// worth less than one that struck its colours, or careful shooting stops
    /// mattering the moment towing exists.
    #[test]
    fn a_clean_kill_is_worth_more() {
        let mut rng = rand::thread_rng();
        let clean: usize = (0..40)
            .map(|_| rare_count(&hulk_yard_value(&hulk(10, 1.0), &mut rng)))
            .sum();
        let gutted: usize = (0..40)
            .map(|_| rare_count(&hulk_yard_value(&hulk(10, 0.2), &mut rng)))
            .sum();
        assert!(clean > gutted, "clean {clean} vs gutted {gutted}");
    }

    /// Even a wrecked hulk is worth towing at all, or the choice collapses to
    /// "only tow perfect kills" and the mechanic is dead most of the time.
    #[test]
    fn even_a_gutted_hulk_pays_something() {
        let mut rng = rand::thread_rng();
        let haul = hulk_yard_value(&hulk(2, 0.05), &mut rng);
        assert!(rare_count(&haul) > 0);
    }

    /// A full hold turns items away. They must be reported as lost, not as
    /// recovered, and the rare ones -- the reason for the tow -- go in first.
    #[test]
    fn a_full_hold_reports_what_it_turned_away() {
        let mut inv = Inventory { max_capacity: 10.0, ..Default::default() };
        let haul = [ItemType::ScrapMetal, ItemType::ScrapMetal, ItemType::RareAlloy];
        let (recovered, rare) = load_haul(&haul, &mut inv);
        assert_eq!((recovered, rare), (1, 1), "rare alloy (8) first, then no room for 5-weight scrap");
        assert_eq!(inv.items.get(&ItemType::RareAlloy).copied(), Some(1));
        assert_eq!(inv.items.get(&ItemType::ScrapMetal), None);
    }

    /// Mass has to actually bite. ship_movement divides force by mass, so this
    /// is what makes a tow feel heavy without touching the movement code.
    #[test]
    fn a_tow_is_heavy() {
        assert!(MASS_PER_LOOT > 100.0, "a hulk barely changes how the ship handles");
        assert!(TOW_NOISE > 0.0, "a tow should be loud as well as slow");
    }

    use crate::ai_ship::components::OwnedByAiShip;
    use bevy::input::ButtonInput;
    use std::time::Duration;

    /// The towing systems, wired the way the plugin wires them but without
    /// states, so a test can step them directly.
    ///
    /// These systems had never been exercised by anything. Every existing test
    /// here covers `hulk_yard_value`, a pure function that decides what a hulk
    /// is worth -- nothing had ever latched onto one.
    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<Towing>()
            .init_resource::<NoiseState>()
            .init_resource::<Inventory>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<ShowNotification>()
            .add_systems(Update, (toggle_tow, drag_hulk).chain());
        app
    }

    /// A bare tractor beam module. `Module` has no Default, and every field
    /// but the two this test cares about is irrelevant here.
    fn beam(active: bool) -> Module {
        Module {
            module_type: ModuleType::TractorBeam,
            health: 60.0,
            max_health: 60.0,
            power_consumption: 25.0,
            power_generation: 0.0,
            is_active: active,
            grid_position: IVec2::ZERO,
            size: IVec2::ONE,
            rotation: crate::components::Rotation::North,
        }
    }

    /// A player ship at `pos` with `beams` working tractor beams aboard.
    ///
    /// GlobalTransform is written directly rather than propagated: there is no
    /// TransformPlugin here, and a propagated transform would read as the
    /// origin on the frame the entity is spawned anyway.
    fn spawn_player(app: &mut App, pos: Vec2, beams: usize) -> Entity {
        let ship = app
            .world_mut()
            .spawn((
                Ship,
                ShipPhysics { mass: 1000.0, ..default() },
                Transform::from_translation(pos.extend(0.0)),
                GlobalTransform::from_translation(pos.extend(0.0)),
            ))
            .id();
        for i in 0..beams {
            let mut m = beam(true);
            m.grid_position = IVec2::new(i as i32, 0);
            app.world_mut().spawn(m);
        }
        ship
    }

    fn spawn_hulk(app: &mut App, pos: Vec2, loot: u32) -> Entity {
        app.world_mut()
            .spawn((
                hulk(loot, 1.0),
                Transform::from_translation(pos.extend(0.0)),
                GlobalTransform::from_translation(pos.extend(0.0)),
            ))
            .id()
    }

    /// One genuine fresh press of Y, the way a player produces one.
    ///
    /// `press` on a key already held does not re-register `just_pressed`, so
    /// without the reset every call after the first is a no-op and the tow
    /// looks like it can be latched but never dropped. That is the harness,
    /// not the game: Bevy rebuilds this resource from OS events each frame.
    fn press_y(app: &mut App) {
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.clear();
            input.reset(KeyCode::KeyY);
            input.press(KeyCode::KeyY);
        }
        app.update();
    }

    /// Advance the clock one frame with no key held.
    ///
    /// The clear matters. Nothing here plays the part of Bevy's input plugin,
    /// which rebuilds `ButtonInput` from OS events every frame, so a
    /// `just_pressed` left over from `press_y` would still read as pressed on
    /// the next update -- and `toggle_tow` would latch and release the hulk on
    /// alternating frames for as long as the test ran.
    fn tick(app: &mut App, secs: f32) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(secs));
        app.update();
    }

    /// The whole point, start to finish: press the key near a hulk and it is
    /// on the hook.
    #[test]
    fn pressing_the_key_latches_a_hulk_in_range() {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO, 1);
        let wreck = spawn_hulk(&mut app, Vec2::new(LATCH_RANGE * 0.5, 0.0), 6);

        press_y(&mut app);

        assert_eq!(
            app.world().resource::<Towing>().hulk,
            Some(wreck),
            "a hulk well inside latch range was not picked up"
        );
    }

    /// Range has to mean something, or the key is just "collect nearest wreck
    /// anywhere".
    #[test]
    fn a_hulk_out_of_range_is_not_latched() {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO, 1);
        spawn_hulk(&mut app, Vec2::new(LATCH_RANGE * 1.5, 0.0), 6);

        press_y(&mut app);

        assert!(app.world().resource::<Towing>().hulk.is_none());
    }

    /// You must own the beam.
    ///
    /// The module query was unscoped, and Stellar Preserve hulls carry two
    /// TractorBeams each -- so with one of their ships anywhere in the system,
    /// a player with no beam at all could latch on. Same cross-ship
    /// contamination this codebase keeps producing: a query that means "the
    /// player's modules" written as "every module".
    #[test]
    fn an_enemys_tractor_beam_is_not_yours() {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO, 0);
        // An enemy ship in the same system, carrying beams of its own.
        for _ in 0..2 {
            app.world_mut()
                .spawn((beam(true), OwnedByAiShip { root: Entity::PLACEHOLDER }));
        }
        spawn_hulk(&mut app, Vec2::new(100.0, 0.0), 6);

        press_y(&mut app);

        assert!(
            app.world().resource::<Towing>().hulk.is_none(),
            "latched a hulk using an enemy ship's tractor beam"
        );
    }

    /// An unpowered beam is not a beam.
    #[test]
    fn a_dead_beam_cannot_latch() {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO, 0);
        app.world_mut().spawn(beam(false));
        spawn_hulk(&mut app, Vec2::new(100.0, 0.0), 6);

        press_y(&mut app);

        assert!(app.world().resource::<Towing>().hulk.is_none());
    }

    /// Press again to drop it.
    #[test]
    fn pressing_again_releases() {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO, 1);
        spawn_hulk(&mut app, Vec2::new(200.0, 0.0), 6);

        press_y(&mut app);
        assert!(app.world().resource::<Towing>().hulk.is_some());
        press_y(&mut app);
        assert!(app.world().resource::<Towing>().hulk.is_none());
    }

    /// The hulk has to actually come along, or the tow is a status effect.
    #[test]
    fn a_towed_hulk_follows_the_ship() {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO, 1);
        let wreck = spawn_hulk(&mut app, Vec2::new(300.0, 300.0), 6);
        press_y(&mut app);

        let start = app.world().entity(wreck).get::<Transform>().unwrap().translation.truncate();
        for _ in 0..30 {
            tick(&mut app, 0.1);
        }
        let end = app.world().entity(wreck).get::<Transform>().unwrap().translation.truncate();

        assert!(
            end.distance(start) > 1.0,
            "the hulk never moved: it latched but nothing dragged it"
        );
        // TOW_OFFSET behind a ship facing its default direction.
        assert!(
            end.length() <= TOW_OFFSET + 1.0,
            "the hulk settled {} out, further than the {TOW_OFFSET} tow line",
            end.length()
        );
    }

    /// A tow is heavy and loud while it is on, and both go away when it comes
    /// off. Mass especially: `ship_movement` divides force by mass, so a mass
    /// left inflated after release would quietly ruin the ship forever.
    #[test]
    fn mass_and_noise_return_when_the_tow_is_dropped() {
        let mut app = app();
        let ship = spawn_player(&mut app, Vec2::ZERO, 1);
        spawn_hulk(&mut app, Vec2::new(200.0, 0.0), 6);

        let base = app.world().entity(ship).get::<ShipPhysics>().unwrap().mass;
        press_y(&mut app);
        tick(&mut app, 0.5);

        let towing_mass = app.world().entity(ship).get::<ShipPhysics>().unwrap().mass;
        assert!(towing_mass > base, "towing a six-unit hulk changed nothing about the ship");
        assert!(
            app.world().resource::<NoiseState>().noise_level > 0.0,
            "a tow should be loud"
        );

        press_y(&mut app);
        tick(&mut app, 0.5);
        assert_eq!(
            app.world().entity(ship).get::<ShipPhysics>().unwrap().mass,
            base,
            "the ship kept the hulk's weight after dropping it"
        );
    }

    /// If the hulk is destroyed underneath the tow, the ship must not keep its
    /// weight.
    #[test]
    fn losing_the_hulk_gives_the_weight_back() {
        let mut app = app();
        let ship = spawn_player(&mut app, Vec2::ZERO, 1);
        let wreck = spawn_hulk(&mut app, Vec2::new(200.0, 0.0), 6);

        let base = app.world().entity(ship).get::<ShipPhysics>().unwrap().mass;
        press_y(&mut app);
        tick(&mut app, 0.5);
        app.world_mut().entity_mut(wreck).despawn();
        tick(&mut app, 0.5);

        assert!(app.world().resource::<Towing>().hulk.is_none());
        assert_eq!(app.world().entity(ship).get::<ShipPhysics>().unwrap().mass, base);
    }

    /// Docking with a hulk on the line is what pays.
    #[test]
    fn docking_with_a_hulk_pays_out_and_consumes_it() {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO, 1);
        let wreck = spawn_hulk(&mut app, Vec2::new(200.0, 0.0), 8);
        press_y(&mut app);
        assert!(app.world().resource::<Towing>().hulk.is_some());

        app.world_mut().run_system_cached(process_hulk_on_dock).unwrap();
        app.update();

        let inv = app.world().resource::<Inventory>();
        let rare: u32 = inv
            .items
            .iter()
            .filter(|(k, _)| matches!(k, ItemType::RareAlloy | ItemType::AncientArtifact))
            .map(|(_, v)| *v)
            .sum();
        assert!(rare > 0, "docking with a hulk produced nothing rare");
        assert!(
            app.world().resource::<Towing>().hulk.is_none(),
            "the tow was still attached after the yard broke it up"
        );
        assert!(
            app.world().get_entity(wreck).is_err(),
            "the hulk survived being broken up, so it can be sold twice"
        );
    }

    /// Docking with nothing in tow must not touch the inventory.
    #[test]
    fn docking_empty_pays_nothing() {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO, 1);
        app.world_mut().run_system_cached(process_hulk_on_dock).unwrap();
        assert_eq!(app.world().resource::<Inventory>().items.len(), 0);
    }

    fn hints(app: &mut App) -> Vec<String> {
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<ShowNotification>>()
            .drain()
            .map(|n| n.message)
            .collect()
    }

    /// With a beam aboard, the hint names the key.
    #[test]
    fn nearing_a_hulk_with_a_beam_names_the_key() {
        let mut app = app();
        app.add_systems(Update, hint_towing);
        spawn_player(&mut app, Vec2::ZERO, 1);
        spawn_hulk(&mut app, Vec2::new(200.0, 0.0), 6);

        tick(&mut app, 0.1);

        let said = hints(&mut app).join(" ");
        assert!(said.contains("Y:"), "no hint naming the tow key: {said:?}");
    }

    /// Without one, it names what to buy -- the starter ship has no tractor
    /// beam, so this is the hint a new player actually gets.
    #[test]
    fn nearing_a_hulk_without_a_beam_says_what_to_buy() {
        let mut app = app();
        app.add_systems(Update, hint_towing);
        spawn_player(&mut app, Vec2::ZERO, 0);
        spawn_hulk(&mut app, Vec2::new(200.0, 0.0), 6);

        tick(&mut app, 0.1);

        let said = hints(&mut app).join(" ");
        assert!(
            said.contains("tractor beam"),
            "a player with no beam was told nothing: {said:?}"
        );
    }

    /// Once per run, not once per frame.
    #[test]
    fn the_hint_does_not_repeat() {
        let mut app = app();
        app.add_systems(Update, hint_towing);
        spawn_player(&mut app, Vec2::ZERO, 1);
        spawn_hulk(&mut app, Vec2::new(200.0, 0.0), 6);

        tick(&mut app, 0.1);
        let _ = hints(&mut app);
        for _ in 0..10 {
            tick(&mut app, 0.1);
        }
        assert!(hints(&mut app).is_empty(), "the tow hint repeated");
    }

    /// And not at all when there is no hulk to tow.
    #[test]
    fn no_hint_with_nothing_in_range() {
        let mut app = app();
        app.add_systems(Update, hint_towing);
        spawn_player(&mut app, Vec2::ZERO, 1);
        spawn_hulk(&mut app, Vec2::new(LATCH_RANGE * 3.0, 0.0), 6);

        tick(&mut app, 0.1);

        assert!(hints(&mut app).is_empty());
    }

    fn rare_count(v: &[ItemType]) -> usize {
        v.iter().filter(|i| matches!(i, ItemType::RareAlloy | ItemType::AncientArtifact)).count()
    }
}
