//! The summit site: where it happened, and where the demo ends.
//!
//! The tear on the galaxy map (narrative::summit::scar_radius) was only a
//! mark. Now its middle is a place. When the black box has said what
//! happened there (step 2) the site is pinned: a point in open space, clear
//! of every system, that never moves again. It is in the dark, so the warp
//! drive wants a Dark Ward to go there, and the Abbess gives the ward's
//! pattern at the same step.
//!
//! Jump into the tear and you are at the site: the veil closes in as in any
//! taken system (narrative::dark_scene), the ward strains (narrative::ward),
//! and the summit is still there, stopped at 14:03: the delegations' ships
//! in a ring, each at its berth, and one berth empty. Once the Abbess has
//! asked you to carry the pieces there (after the fourth), bringing the ship
//! into that berth plays the record out (narrative::ending).
//!
//! The site is a point, not a taken system: a taken system in the middle of
//! the map would be a second front for the dark to spread from.

use bevy::prelude::*;

use crate::ai_ship::components::AiShipType;
use crate::celestial::galaxy::SNAP_TOLERANCE;
use crate::celestial::resources::{GalaxyMap, GalaxyWarpTarget, SystemStreamingManager};
use crate::components::Ship;
use crate::diplomacy::contacts::Contacts;
use crate::states::GameState;

use super::marlow::MarlowMoment;

/// How near the pinned point (galaxy units) open space counts as the site.
pub const SITE_REACH: f32 = 250_000.0;
/// The ward's draw at the site, as a share of what a taken system one deep
/// asks: the site is the story's own errand, and it has to be flyable.
pub const SITE_DRAW: f32 = 0.5;
/// Berths round the hall: eleven delegations and the observer's.
pub const BERTHS: usize = 12;
/// The hall is an avenue: the delegations' ships in two rows either side of
/// the way in, and the empty berth at the far end of it. `LANE` is how far
/// each row stands off the middle of the avenue, `FIRST` how far in the
/// first pair is, `STEP` the gap from one pair to the next, and
/// `BERTH_REACH` how close to the empty berth counts as taking it.
///
/// The ships are capital hulls, kilometres long: a first layout, a ring
/// with berths four kilometres apart, had them across the arriving ship's
/// path, and it lost a third of its hull finding that out. At full burn a
/// ship covers about a kilometre a second, so the avenue is about forty
/// seconds long.
const LANE: f32 = 7_000.0;
const FIRST: f32 = 6_000.0;
const STEP: f32 = 6_000.0;
const BERTH_REACH: f32 = 2_500.0;

/// Whether the ship is at the summit site right now.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct AtSite(pub bool);

/// Where the site is, for systems that have no business reading the whole
/// story record (the warp drive).
#[derive(Resource, Default, Debug)]
pub struct SitePlace(pub Option<Vec2>);

/// Where the hall has been laid out in local space this visit: where you
/// came in, and the empty berth.
#[derive(Resource, Default, Debug)]
pub struct Hall(pub Option<(Vec2, Vec2)>);

/// A delegation's ship, stopped at its berth.
#[derive(Component)]
pub struct Delegate;

/// Where to pin the site: where the summit station was
/// (summit::summit_site), moved out along the same line until it is clear
/// of every system, so a jump aimed at it never snaps to a star.
pub fn pin(galaxy: &GalaxyMap) -> Vec2 {
    let wanted = super::summit::summit_site(galaxy);
    let clear = |p: Vec2| galaxy.systems.iter().all(|s| s.galaxy_pos.distance(p) >= SNAP_TOLERANCE * 2.0);
    let along = wanted.normalize_or(Vec2::X);
    let side = along.perp();
    // Nearest clear point: a widening search either way along the road out,
    // then off to the sides of it.
    (0..40)
        .flat_map(|i| {
            let d = i as f32 * SNAP_TOLERANCE * 0.5;
            [wanted + along * d, wanted - along * d, wanted + side * d, wanted - side * d]
        })
        .find(|p| clear(*p))
        .unwrap_or(wanted)
}

/// Whether a point in open space is at the site.
pub fn is_at(site: Option<Vec2>, galaxy_pos: Vec2) -> bool {
    site.is_some_and(|s| s.distance(galaxy_pos) < SITE_REACH)
}

/// Whether the warp drive will make a blind jump to `target`: anywhere, or
/// into the site with a working ward aboard.
pub fn may_jump(site: Option<Vec2>, target: GalaxyWarpTarget, ward_aboard: bool) -> bool {
    match target {
        GalaxyWarpTarget::BlindPoint(pos) => ward_aboard || !is_at(site, pos),
        _ => true,
    }
}

/// The berths of the hall, laid out from where you came in (`door`) along
/// the way you were heading (`ahead`, a unit vector): the first is the
/// empty one, at the far end of the avenue; the rest stand in two rows
/// either side of the way to it, a pair at a time.
pub fn berths(door: Vec2, ahead: Vec2) -> Vec<Vec2> {
    let across = ahead.perp();
    let pairs = (BERTHS - 1).div_ceil(2);
    let mut out = vec![door + ahead * (FIRST + pairs as f32 * STEP)];
    out.extend((0..BERTHS - 1).map(|k| {
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        door + ahead * (FIRST + (k / 2) as f32 * STEP) + across * LANE * side
    }));
    out
}

/// Whose ship stands at berth `i` (from 1; berth 0 is the empty one): the
/// powers that hold ground, and the Hegemony's old flag for the eleventh.
pub fn delegation(i: usize) -> AiShipType {
    let powers = crate::celestial::territory::TERRITORIAL;
    powers.get(i - 1).copied().unwrap_or(AiShipType::TerranHegemony)
}

/// Pin the site when the black box has said where it was, and keep the
/// drive's copy of it. A save from before the site existed, picked up with
/// the fourth piece already in hand, is asked to carry them now.
fn keep_the_site(mut contacts: ResMut<Contacts>, galaxy: Res<GalaxyMap>, mut place: ResMut<SitePlace>, mut cards: ResMut<super::reader::LogQueue>) {
    if contacts.summit.done >= 2 && contacts.story.site.is_none() && !galaxy.systems.is_empty() {
        let at = pin(&galaxy);
        contacts.story.site = Some(at);
        info!("SITE pinned at {at:?}");
    }
    if contacts.summit.done >= super::summit::last_step() && !contacts.story.carry && !contacts.story.ending_seen {
        contacts.story.carry = true;
        contacts.summit.ended = true;
        let card = &super::ending::data().carry;
        // (summit::advance says this itself when the piece is won; this is
        // for a run that had the piece before there was anything to say.)
        cards.push_card("OVER THE COMM", card.title.clone(), card.text.clone());
        cards.set_gist(&card.title, card.gist.clone());
        info!("SITE: the Abbess asks for the pieces to be carried there");
    }
    if place.0 != contacts.story.site {
        place.0 = contacts.story.site;
    }
}

/// Are we there?
fn watch_for_the_site(streaming: Res<SystemStreamingManager>, place: Res<SitePlace>, mut at: ResMut<AtSite>, mut moments: MessageWriter<MarlowMoment>) {
    let now = streaming.loaded_system.is_none() && is_at(place.0, streaming.current_galaxy_pos);
    if at.0 != now {
        at.0 = now;
        info!("SITE: {}", if now { "arrived" } else { "left" });
        if now {
            moments.write(MarlowMoment("site"));
        }
    }
}

/// Lay the hall out when you arrive, and clear it away when you leave: the
/// delegations' ships at their berths, stopped (dark_scene stills them into
/// wrecks), and one berth empty, toward you.
#[allow(clippy::too_many_arguments)]
fn stage_the_hall(
    mut commands: Commands,
    at: Res<AtSite>,
    mut hall: ResMut<Hall>,
    mut ship: Query<(&Transform, &mut crate::components::Velocity), With<Ship>>,
    delegates: Query<Entity, With<Delegate>>,
    registry: Res<crate::building::registry::ModuleRegistry>,
    assets: Res<AssetServer>,
) {
    if !at.0 {
        if hall.0.is_some() {
            hall.0 = None;
            for e in &delegates {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    if hall.0.is_some() {
        return;
    }
    let Ok((you, mut speed)) = ship.single_mut() else { return };
    // Nothing moves here. A ship comes out of a jump at a kilometre a second
    // and would coast into the empty berth without anyone choosing to: it
    // arrives stopped, like everything else, and going in is a decision.
    speed.0 = Vec2::ZERO;
    let here = you.translation.truncate();
    // Ahead of the ship, wherever it happens to be pointing.
    let ahead = (you.rotation * Vec3::X).truncate().normalize_or(Vec2::X);
    let ring = berths(here, ahead);
    hall.0 = Some((here, ring[0]));
    for (i, at) in ring.iter().enumerate().skip(1) {
        let flag = delegation(i);
        let e = crate::ai_ship::spawner::spawn_ai_ship(flag, *at, &mut commands, &registry, &assets);
        // Bows to the chair, as they sat for the vote: along the avenue, so
        // no hull lies across the way in. Each is marked at the edge of the
        // screen, because in this dark you would pass them unseen.
        commands.entity(e).try_insert((
            Delegate,
            super::dark_scene::FrozenHulk,
            super::summit::StoryBeacon(crate::ai_ship::components::faction_display_name(flag).to_string()),
            Transform::from_xyz(at.x, at.y, 0.0).with_rotation(Quat::from_rotation_z(ahead.to_angle())),
        ));
    }
    // The empty one: where to bring her in.
    commands.spawn((Delegate, super::summit::StoryBeacon("EMPTY BERTH".into()), Transform::from_xyz(ring[0].x, ring[0].y, 0.0)));
    info!("SITE: the hall is laid out, {} ships at their berths, one berth empty", BERTHS - 1);
}

/// Bring the ship into the empty berth with the pieces aboard and the
/// record plays out. Once a run.
fn take_the_berth(
    at: Res<AtSite>,
    hall: Res<Hall>,
    ship: Query<&Transform, With<Ship>>,
    mut marks: ParamSet<(ResMut<Contacts>, super::ending::Run)>,
    mut playing: ResMut<super::ending::Playing>,
    mut next: ResMut<NextState<GameState>>,
) {
    let (true, Some((_, berth))) = (at.0, hall.0) else { return };
    {
        let contacts = marks.p0();
        if !contacts.story.carry || contacts.story.ending_seen {
            return;
        }
    }
    let Ok(you) = ship.single() else { return };
    if you.translation.truncate().distance(berth) > BERTH_REACH {
        return;
    }
    marks.p0().story.ending_seen = true;
    info!("SITE: the ship is in the empty berth");
    super::ending::play_the_record(&marks.p1(), &mut playing, &mut next);
}

/// `AFTERLIGHT_SITE=1`: a few seconds into flight, with four pieces in hand
/// and the Abbess having asked, jump to the site. Pair it with
/// `AFTERLIGHT_WARD=1` to fly it with a ward, or without to see the dark
/// take the ship there.
fn site_from_env(
    mut commands: Commands,
    time: Res<Time>,
    mut contacts: ResMut<Contacts>,
    ship: Query<Entity, (With<Ship>, Without<crate::celestial::components::WarpCharging>)>,
    mut waited: Local<f32>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    if std::env::var("AFTERLIGHT_SITE").ok().as_deref() != Some("1") {
        *done = true;
        return;
    }
    // The story as it stands when the Abbess has asked.
    if contacts.summit.done < super::summit::last_step() {
        let last = super::summit::last_step();
        contacts.summit = super::summit::SummitProgress { hailed: true, commissioned: true, done: last, ended: true };
        contacts.story.carry = true;
        contacts.story.errands = super::summit::ERRANDS;
        contacts.story.errands_given = super::summit::ERRANDS + 1;
    }
    *waited += time.delta_secs();
    let (Some(site), Ok(ship)) = (contacts.story.site, ship.single()) else { return };
    if *waited < 4.0 {
        return;
    }
    *done = true;
    info!("SITE: AFTERLIGHT_SITE jumps the ship to {site:?}");
    commands.entity(ship).insert(crate::celestial::components::WarpCharging {
        target: GalaxyWarpTarget::BlindPoint(site),
        charge_timer: Timer::from_seconds(0.5, TimerMode::Once),
        fuel_cost: 0.0,
    });
}

fn forget_on_new_run(mut at: ResMut<AtSite>, mut hall: ResMut<Hall>, mut place: ResMut<SitePlace>) {
    *at = AtSite::default();
    *hall = Hall::default();
    *place = SitePlace::default();
}

pub struct SitePlugin;

impl Plugin for SitePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AtSite>()
            .init_resource::<SitePlace>()
            .init_resource::<Hall>()
            .add_systems(
                Update,
                (site_from_env, keep_the_site, watch_for_the_site, stage_the_hall, take_the_berth).chain().run_if(in_state(GameState::Exploring)),
            )
            .add_systems(OnEnter(GameState::MainMenu), forget_on_new_run);
    }
}

#[cfg(test)]
mod site_tests {
    use super::*;

    /// A jump aimed at the site must arrive in open space, not at a star
    /// that happens to be near: the pin is clear of every system's snap.
    #[test]
    fn the_site_never_snaps_to_a_system() {
        for seed in [1u64, 7, 42, 777, 20_261_126] {
            let galaxy = crate::celestial::galaxy::generate_galaxy_map(seed);
            let site = pin(&galaxy);
            for s in &galaxy.systems {
                assert!(s.galaxy_pos.distance(site) >= SNAP_TOLERANCE * 2.0, "seed {seed}: {} is {:.0} from the site", s.name, s.galaxy_pos.distance(site));
            }
            assert!(crate::celestial::galaxy::system_within_snap_tolerance(&galaxy, site).is_none(), "seed {seed}");
            // Still inside the tear as the map draws it, and never on Haven.
            let scar = crate::narrative::summit::scar_radius(2).unwrap();
            assert!(site.distance(crate::narrative::summit::summit_site(&galaxy)) < scar * 2.0, "seed {seed}: pinned far from the tear");
            assert!(site.length() > SNAP_TOLERANCE * 2.0, "seed {seed}: the site is on Haven");
        }
    }

    /// Pinned once, it stays: the Choir losing their capital moves where the
    /// station is reckoned to have been, and must not move the site.
    #[test]
    fn the_site_stays_put_when_the_choir_move() {
        let mut app = App::new();
        app.insert_resource(crate::celestial::galaxy::generate_galaxy_map(42))
            .init_resource::<SitePlace>()
            .init_resource::<super::super::reader::LogQueue>()
            .insert_resource(Contacts { summit: super::super::summit::SummitProgress { hailed: true, commissioned: true, done: 2, ended: false }, ..default() })
            .add_systems(Update, keep_the_site);
        app.update();
        let pinned = app.world().resource::<Contacts>().story.site.expect("step 2 didn't pin the site");
        assert_eq!(app.world().resource::<SitePlace>().0, Some(pinned));
        // The Choir fall back to another system.
        {
            let mut galaxy = app.world_mut().resource_mut::<GalaxyMap>();
            let capital = galaxy.systems.iter().position(|s| s.capital && s.faction == Some(AiShipType::BrokenChoir)).unwrap();
            galaxy.systems[capital].capital = false;
            galaxy.systems[capital].faction = None;
        }
        app.update();
        assert_eq!(app.world().resource::<Contacts>().story.site, Some(pinned));
        // And it rides in the save.
        let saved = serde_json::to_string(app.world().resource::<Contacts>()).unwrap();
        assert_eq!(serde_json::from_str::<Contacts>(&saved).unwrap().story.site, Some(pinned));
        // Before the black box, there is no site.
        let mut early = App::new();
        early
            .insert_resource(crate::celestial::galaxy::generate_galaxy_map(42))
            .init_resource::<SitePlace>()
            .init_resource::<super::super::reader::LogQueue>()
            .insert_resource(Contacts { summit: super::super::summit::SummitProgress { hailed: true, commissioned: true, done: 1, ended: false }, ..default() })
            .add_systems(Update, keep_the_site);
        early.update();
        assert_eq!(early.world().resource::<Contacts>().story.site, None);
    }

    /// The drive goes anywhere in open space but the site, and there only
    /// with a ward; and being at the site means open space near the pin.
    #[test]
    fn the_site_wants_a_ward() {
        let site = Some(Vec2::new(900_000.0, 400_000.0));
        let inside = GalaxyWarpTarget::BlindPoint(site.unwrap() + Vec2::new(SITE_REACH * 0.5, 0.0));
        let outside = GalaxyWarpTarget::BlindPoint(site.unwrap() + Vec2::new(SITE_REACH * 1.5, 0.0));
        assert!(!may_jump(site, inside, false), "into the site with no ward");
        assert!(may_jump(site, inside, true));
        assert!(may_jump(site, outside, false));
        assert!(may_jump(site, GalaxyWarpTarget::System(3), false), "systems are the ward's own rule, not the site's");
        assert!(may_jump(None, inside, false), "no site yet, nothing to refuse");
        assert!(is_at(site, site.unwrap()) && !is_at(None, site.unwrap()));
    }

    /// Into the empty berth with the pieces aboard and the Abbess having
    /// asked: the record plays, through the real system, and only ever once.
    /// Without her asking, or away from the berth, nothing happens.
    #[test]
    fn the_ending_plays_once() {
        use super::super::ending::Playing;
        use bevy::state::app::StatesPlugin;
        let berth = Vec2::new(7_000.0, 0.0);
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .insert_state(GameState::Exploring)
            .insert_resource(crate::celestial::galaxy::generate_galaxy_map(42))
            .init_resource::<crate::resources::Statistics>()
            .init_resource::<crate::resources::Currency>()
            .init_resource::<Playing>()
            .insert_resource(AtSite(true))
            .insert_resource(Hall(Some((Vec2::new(15_000.0, 0.0), berth))))
            .insert_resource(Contacts { summit: super::super::summit::SummitProgress { hailed: true, commissioned: true, done: 4, ended: true }, ..default() })
            .add_systems(Update, take_the_berth);
        let ship = app.world_mut().spawn((Ship, Transform::from_xyz(0.0, 0.0, 0.0))).id();
        let playing = |app: &App| matches!(*app.world().resource::<Playing>(), Playing::Assembled { .. });
        let put = |app: &mut App, at: Vec2| {
            app.world_mut().get_mut::<Transform>(ship).unwrap().translation = at.extend(0.0);
            app.update();
        };
        // In the berth, but nobody has asked for the pieces to be brought.
        put(&mut app, berth);
        assert!(!playing(&app), "it played unasked");
        // Asked, but still out in the hall.
        app.world_mut().resource_mut::<Contacts>().story.carry = true;
        put(&mut app, Vec2::ZERO);
        assert!(!playing(&app), "it played from across the hall");
        // Asked, and in the berth.
        put(&mut app, berth + Vec2::new(BERTH_REACH * 0.5, 0.0));
        assert!(playing(&app), "the record didn't play");
        assert!(app.world().resource::<Contacts>().story.ending_seen);
        assert!(matches!(app.world().resource::<NextState<GameState>>(), NextState::Pending(GameState::Truth)));
        // Sitting in the berth afterwards doesn't start it again.
        *app.world_mut().resource_mut::<Playing>() = Playing::Truth;
        put(&mut app, berth);
        assert!(!playing(&app), "it played twice");
    }

    /// Eleven ships at eleven berths and one berth left empty, at the far
    /// end of an avenue between them; nobody is berthed on top of anybody,
    /// and nobody across the way in.
    #[test]
    fn eleven_berths_and_one_empty() {
        let (you, ahead) = (Vec2::new(100.0, -50.0), Vec2::new(0.6, 0.8));
        let ring = berths(you, ahead);
        assert_eq!(ring.len(), BERTHS);
        assert_eq!(ring.len() - 1, 11);
        let along = |p: Vec2| (p - you).dot(ahead);
        let off = |p: Vec2| (p - you).dot(ahead.perp()).abs();
        // The empty berth is dead ahead, past every delegation.
        assert!(off(ring[0]) < 1.0, "the empty berth is off the avenue");
        assert!(ring.iter().skip(1).all(|b| along(*b) < along(ring[0])), "a delegation sits beyond the chair");
        // About forty seconds at a kilometre a second: a flight, not a hop.
        assert!((30_000.0..=60_000.0).contains(&along(ring[0])), "{}", along(ring[0]));
        // Every delegation stands well clear of the way in, on one side or
        // the other, six on one and five on the other.
        for b in ring.iter().skip(1) {
            assert!(off(*b) >= 6_000.0, "a hull {:.0} from the middle of the avenue", off(*b));
            assert!(along(*b) > 0.0);
        }
        let left = ring.iter().skip(1).filter(|b| (**b - you).dot(ahead.perp()) > 0.0).count();
        assert_eq!((left, ring.len() - 1 - left), (6, 5));
        for (i, a) in ring.iter().enumerate() {
            for b in ring.iter().skip(i + 1) {
                assert!(a.distance(*b) >= 5_900.0, "berths {:.0} apart", a.distance(*b));
            }
        }
        // Every berth but the first has a delegation, all of them real.
        let flags: Vec<AiShipType> = (1..BERTHS).map(delegation).collect();
        assert_eq!(flags.len(), 11);
        assert!(crate::celestial::territory::TERRITORIAL.iter().all(|f| flags.contains(f)));
        // The empty berth is a real flight away, not on top of the ship.
        assert!(ring[0].distance(you) > BERTH_REACH * 10.0);
    }
}
