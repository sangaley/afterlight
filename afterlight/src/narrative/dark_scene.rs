//! What a taken system looks like from inside it ("Live Cascade plan", 3):
//! the light drains away, the stars go out one by one, and the ships that
//! were here when it came are still here, stopped mid-trade and mid-fight.
//!
//! The light: a veil of black over the whole scene with a hole in it where
//! your own ship's lights reach. It closes in over a few seconds as you
//! arrive, and a system the dark is still reaching dims part of the way, a
//! warning before anything says so. A failing ward lets the hole shrink.
//!
//! The ships: a taken system has no traffic (ai_ship::simulation), so the
//! ones you find are hulks. If the dark takes the system you're in, the ships
//! around you freeze where they are; arriving later, you find a convoy stopped
//! in the station's approach and a fight that never finished, rounds still
//! hanging between the lines. Every hulk is a wreck you can salvage, rich
//! (poi::DARK_HAUL), and none of them ever moves again.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::ai_ship::components::{AiShip, AiShipState, AiShipType, AiShipWreck};
use crate::celestial::components::StarSystemMember;
use crate::celestial::resources::{GalaxyMap, StarSystemDef, SystemStreamingManager};
use crate::components::{HullSegment, Module, PoiType, PointOfInterest, Ship, Velocity, Wreck};
use crate::vfx::starfield::StarfieldStar;

use super::front::is_taken;

/// How much of the scene the veil hides at its darkest. Not all of it: the
/// hulks have to stay faintly there, or there's nothing to find.
pub const VEIL_MAX: f32 = 0.84;

/// How fast the light goes, and comes back, per second of flight.
const DRAIN_RATE: f32 = 0.12;

/// Share of the stars put out in a taken system.
const STARS_OUT: f32 = 0.92;

/// The colour of the void with no light left in it.
const DEAD_VOID: Vec3 = Vec3::new(0.004, 0.004, 0.008);

/// Above the whole world, below nothing that isn't UI.
const VEIL_Z: f32 = 40.0;

/// How dark the scene is around you: 0 lit, 1 taken. Eased, so the light
/// drains rather than switching off.
#[derive(Resource, Default, Debug)]
pub struct DarkView {
    pub level: f32,
}

/// How dark a system should look from inside it: a taken system all the way,
/// a system the dark is reaching part of the way, as a warning.
pub fn target_level(sys: Option<&StarSystemDef>) -> f32 {
    match sys {
        Some(s) if is_taken(s) => 1.0,
        Some(s) => (s.dark * 0.5).clamp(0.0, 0.5),
        None => 0.0,
    }
}

/// `level` moved toward `target` by one frame of draining.
pub fn ease(level: f32, target: f32, dt: f32) -> f32 {
    let step = DRAIN_RATE * dt;
    if (target - level).abs() <= step { target } else { level + step * (target - level).signum() }
}

/// Whether the star numbered `index` has gone out at this level. A fixed
/// share per star, so they go out one by one and come back in the same order.
pub fn star_out(index: u32, level: f32) -> bool {
    let h = index.wrapping_mul(2_654_435_761) as f32 / u32::MAX as f32;
    h < level * STARS_OUT
}

fn measure(
    time: Res<Time>,
    galaxy: Res<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
    mut view: ResMut<DarkView>,
) {
    let here = streaming.loaded_system.and_then(|id| galaxy.systems.iter().find(|s| s.id == id));
    view.level = ease(view.level, target_level(here), time.delta_secs());
}

// ---------------------------------------------------------------------------
// The veil
// ---------------------------------------------------------------------------

/// The hole in the dark: clear in the middle, black from the rim out to the
/// corners. Drawn at twice the reach of the ship's lights.
#[derive(Resource)]
pub struct VeilTexture(pub Handle<Image>);

/// Where the clear centre ends, as a share of the texture's half-width.
const HOLE_INNER: f32 = 0.5;

fn hole_image() -> Image {
    let size = 256u32;
    let c = size as f32 / 2.0;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let t = Vec2::new(x as f32 + 0.5 - c, y as f32 + 0.5 - c).length() / c;
            let u = ((t - HOLE_INNER) / (1.0 - HOLE_INNER)).clamp(0.0, 1.0);
            // Smoothstep, so there's no visible edge to the light.
            let a = u * u * (3.0 - 2.0 * u);
            let i = ((y * size + x) * 4) as usize;
            data[i + 3] = (a * 255.0).round() as u8;
        }
    }
    Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn make_veil_texture(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(VeilTexture(images.add(hole_image())));
}

/// One piece of the veil: the hole round the ship, or one of the four sheets
/// of black that carry it out past the edges of the screen.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub enum VeilPart {
    Hole,
    Left,
    Right,
    Top,
    Bottom,
}

/// Far enough to cover the screen at any zoom.
const SHEET: f32 = 4_000_000.0;

/// How far the ship's lights reach before the dark starts: just past its own
/// hull. A station a short flight off is already going into the dark.
pub fn light_reach(ship_radius: f32, failing_secs: f32) -> f32 {
    let reach = ship_radius * 0.9 + 200.0;
    // A failing ward lets the dark in: down to half the reach in 45 seconds.
    reach * (1.0 - 0.5 * (failing_secs / 45.0).clamp(0.0, 1.0))
}

#[allow(clippy::type_complexity)]
fn draw_veil(
    mut commands: Commands,
    view: Res<DarkView>,
    texture: Option<Res<VeilTexture>>,
    ward: Option<Res<super::ward::WardState>>,
    ship: Query<(&GlobalTransform, Option<&Children>), With<Ship>>,
    blocks: Query<&Transform, (Or<(With<Module>, With<HullSegment>)>, Without<VeilPart>)>,
    mut parts: Query<(Entity, &VeilPart, &mut Transform, &mut Sprite)>,
    mut radius: Local<(f32, u32)>,
) {
    if view.level <= 0.001 {
        for (e, ..) in parts.iter() {
            commands.entity(e).despawn();
        }
        return;
    }
    let Some(texture) = texture else { return };
    let Ok((at, children)) = ship.single() else { return };
    // The hull's reach, measured now and then rather than every frame.
    if radius.1 == 0 {
        radius.0 = children
            .into_iter()
            .flatten()
            .filter_map(|c| blocks.get(*c).ok())
            .map(|t| t.translation.truncate().length())
            .fold(0.0, f32::max);
    }
    radius.1 = (radius.1 + 1) % 60;

    let failing = ward.map_or(0.0, |w| w.failing);
    let outer = light_reach(radius.0, failing) / HOLE_INNER;
    let centre = at.translation().truncate();
    let alpha = view.level * VEIL_MAX;

    if parts.is_empty() {
        for part in [VeilPart::Hole, VeilPart::Left, VeilPart::Right, VeilPart::Top, VeilPart::Bottom] {
            let image = if part == VeilPart::Hole { texture.0.clone() } else { Handle::default() };
            commands.spawn((Sprite { image, color: Color::NONE, ..default() }, Transform::from_xyz(centre.x, centre.y, VEIL_Z), part));
        }
        return;
    }
    for (_, part, mut tf, mut sprite) in parts.iter_mut() {
        let (offset, size) = match part {
            VeilPart::Hole => (Vec2::ZERO, Vec2::splat(outer * 2.0)),
            VeilPart::Left => (Vec2::new(-outer - SHEET / 2.0, 0.0), Vec2::splat(SHEET)),
            VeilPart::Right => (Vec2::new(outer + SHEET / 2.0, 0.0), Vec2::splat(SHEET)),
            VeilPart::Top => (Vec2::new(0.0, outer + SHEET / 2.0), Vec2::new(outer * 2.0, SHEET)),
            VeilPart::Bottom => (Vec2::new(0.0, -outer - SHEET / 2.0), Vec2::new(outer * 2.0, SHEET)),
        };
        let p = centre + offset;
        tf.translation = Vec3::new(p.x, p.y, VEIL_Z);
        sprite.custom_size = Some(size);
        sprite.color = Color::srgba(0.0, 0.0, 0.0, alpha);
    }
}

/// The stars go out one at a time as the light drains.
fn put_out_stars(view: Res<DarkView>, mut stars: Query<(Entity, &mut Visibility), With<StarfieldStar>>, mut last: Local<f32>) {
    if (view.level - *last).abs() < 0.01 && !(view.level == 0.0 && *last != 0.0) {
        return;
    }
    *last = view.level;
    for (e, mut vis) in stars.iter_mut() {
        let want = if star_out(e.index_u32(), view.level) { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
    }
}

/// The void loses the last of its colour. After everything else that tints
/// it this frame (the camera's star glow, the interference).
fn drain_the_void(view: Res<DarkView>, mut clear: ResMut<ClearColor>) {
    if view.level <= 0.0 {
        return;
    }
    let c = clear.0.to_srgba();
    let now = Vec3::new(c.red, c.green, c.blue);
    let d = now.lerp(DEAD_VOID, view.level);
    clear.0 = Color::srgb(d.x, d.y, d.z);
}

fn clear_the_veil(mut commands: Commands, parts: Query<Entity, With<VeilPart>>, mut view: ResMut<DarkView>) {
    view.level = 0.0;
    for e in parts.iter() {
        commands.entity(e).despawn();
    }
}

// ---------------------------------------------------------------------------
// The hulks
// ---------------------------------------------------------------------------

/// A ship the dark stopped. It's a wreck from the first frame it has blocks,
/// and it never moves again.
#[derive(Component)]
pub struct FrozenHulk;

/// Marks that this visit to a taken system has its hulks. Despawned with the
/// system, so the next visit (or a load) puts them back.
#[derive(Component)]
pub struct DarkAnchor;

/// Who was fighting here when it came: the system's two nearest owned
/// neighbours, or two old enemies if the dark has those too.
fn combatants(galaxy: &GalaxyMap, id: u32) -> (AiShipType, AiShipType) {
    let mut near: Vec<AiShipType> = crate::celestial::galaxy::nearest_neighbors(galaxy, id, 6)
        .into_iter()
        .filter_map(|n| galaxy.systems.iter().find(|s| s.id == n).and_then(|s| s.faction))
        .filter(|f| crate::ai_ship::components::faction_fights(*f))
        .collect();
    near.dedup();
    match (near.first(), near.iter().find(|f| Some(*f) != near.first())) {
        (Some(a), Some(b)) => (*a, *b),
        (Some(a), None) if *a != AiShipType::CorpseStars => (*a, AiShipType::CorpseStars),
        _ => (AiShipType::TerranHegemony, AiShipType::CorpseStars),
    }
}

/// One stopped ship to spawn: who, where, facing which way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HulkPlan {
    pub faction: AiShipType,
    pub pos: Vec2,
    pub facing: f32,
}

/// A frozen round between the lines: where, and which way it was flying.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundPlan {
    pub pos: Vec2,
    pub facing: f32,
}

/// The scene the dark left at a station: a convoy stopped in the approach,
/// and a fight further out with its rounds still in the air. The same every
/// visit, from the system's own seed.
pub fn plan_scene(seed: u64, station: Vec2, keep_out: f32, sides: (AiShipType, AiShipType)) -> (Vec<HulkPlan>, Vec<RoundPlan>) {
    let mut rng = StdRng::seed_from_u64(seed ^ 0x0DA2_C0DE);
    let mut hulks = Vec::new();
    let mut rounds = Vec::new();

    // The convoy, nose to tail, waiting to dock.
    let lane = rng.gen_range(0.0..std::f32::consts::TAU);
    let out = Vec2::from_angle(lane);
    let traders = [AiShipType::MerchantPrinces, AiShipType::FreeHaulersCompact];
    for k in 0..rng.gen_range(3..=5) {
        let pos = station + out * (keep_out + 6_000.0 + k as f32 * 6_500.0) + out.perp() * rng.gen_range(-800.0..800.0);
        hulks.push(HulkPlan { faction: traders[rng.gen_range(0..2)], pos, facing: lane + std::f32::consts::PI });
    }

    // The fight, off to one side of the lane.
    let side = if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
    let centre = station + out.perp() * side * (keep_out + 30_000.0) + out * 8_000.0;
    let axis_angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let axis = Vec2::from_angle(axis_angle);
    let gap = 9_000.0;
    for (faction, sign) in [(sides.0, -1.0f32), (sides.1, 1.0)] {
        for k in 0..rng.gen_range(2..=3) {
            let spread = (k as f32 - 1.0) * 5_500.0;
            let pos = centre + axis * sign * (gap + rng.gen_range(0.0..2_000.0)) + axis.perp() * spread;
            let facing = if sign < 0.0 { axis_angle } else { axis_angle + std::f32::consts::PI } + rng.gen_range(-0.25..0.25);
            hulks.push(HulkPlan { faction, pos, facing });
        }
    }
    for _ in 0..rng.gen_range(12..=18) {
        let along = rng.gen_range(-gap * 0.8..gap * 0.8);
        let pos = centre + axis * along + axis.perp() * rng.gen_range(-6_000.0..6_000.0);
        let facing = if rng.gen_bool(0.5) { axis_angle } else { axis_angle + std::f32::consts::PI };
        rounds.push(RoundPlan { pos, facing });
    }
    (hulks, rounds)
}

/// On arriving in a taken system, put its stopped ships back; when the dark
/// takes the system you're in, stop the ships around you.
#[allow(clippy::too_many_arguments)]
fn freeze_the_fleets(
    mut commands: Commands,
    galaxy: Res<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
    anchors: Query<&StarSystemMember, With<DarkAnchor>>,
    live: Query<(Entity, &AiShipState), (With<AiShip>, Without<FrozenHulk>)>,
    registry: Res<crate::building::registry::ModuleRegistry>,
    asset_server: Res<AssetServer>,
    mut last: Local<Option<(u32, bool)>>,
) {
    let Some(id) = streaming.loaded_system else {
        *last = None;
        return;
    };
    let Some(def) = galaxy.systems.iter().find(|s| s.id == id) else { return };
    let taken = is_taken(def);
    let before = last.replace((id, taken));
    if !taken || anchors.iter().any(|m| m.system_id == id) {
        return;
    }
    commands.spawn((DarkAnchor, StarSystemMember { system_id: id }, Transform::default()));

    // Taken while you were here: everyone around you stops.
    if before == Some((id, false)) {
        for (e, state) in live.iter() {
            if !state.is_destroyed {
                commands.entity(e).try_insert((FrozenHulk, StarSystemMember { system_id: id }));
            }
        }
        return;
    }

    let site = crate::world::home_base::station_sites(def).into_iter().next();
    let (station, keep_out) = site.map_or((def.local_center + Vec2::new(60_000.0, 0.0), 2_000.0), |s| (s.pos, s.radius()));
    let (hulks, rounds) = plan_scene(def.seed, station, keep_out, combatants(&galaxy, id));
    info!("DARK: {} stopped ships and {} rounds left in {}", hulks.len(), rounds.len(), def.name);
    for h in hulks {
        let e = crate::ai_ship::spawner::spawn_ai_ship(h.faction, h.pos, &mut commands, &registry, &asset_server);
        commands.entity(e).try_insert((
            FrozenHulk,
            StarSystemMember { system_id: id },
            Transform::from_xyz(h.pos.x, h.pos.y, 0.0).with_rotation(Quat::from_rotation_z(h.facing)),
        ));
    }
    for r in rounds {
        commands.spawn((
            Sprite { color: Color::srgba(1.0, 0.85, 0.55, 0.8), custom_size: Some(Vec2::new(140.0, 14.0)), ..default() },
            Transform::from_xyz(r.pos.x, r.pos.y, 6.0).with_rotation(Quat::from_rotation_z(r.facing)),
            StarSystemMember { system_id: id },
        ));
    }
}

/// A frozen ship becomes a wreck as soon as it has blocks: dead power, no
/// shield, rich salvage. No kill is counted and nobody is told: nothing
/// happened here, it just stopped.
#[allow(clippy::type_complexity)]
fn still_the_hulks(
    mut commands: Commands,
    mut fresh: Query<(Entity, &mut AiShipState, &AiShipType, &Children), (With<FrozenHulk>, Without<AiShipWreck>)>,
    mut modules: Query<&mut Module>,
    mut shields: Query<&mut crate::combat::shields::ShipShield>,
) {
    for (e, mut state, faction, children) in fresh.iter_mut() {
        if children.is_empty() {
            continue;
        }
        state.is_destroyed = true;
        for c in children.iter() {
            if let Ok(mut m) = modules.get_mut(c) {
                m.is_active = false;
            }
        }
        if let Ok(mut s) = shields.get_mut(e) {
            s.enabled = false;
            s.current = 0.0;
        }
        let loot = crate::ai_ship::wreck::base_loot(*faction) * crate::celestial::poi::DARK_HAUL;
        commands.entity(e).try_insert((
            AiShipWreck { ship_type: *faction, loot_remaining: loot, intact_frac: 1.0 },
            PointOfInterest { poi_type: PoiType::Wreck, discovered: true },
            Wreck { loot_remaining: loot, is_explored: false },
        ));
    }
}

/// Nothing in the dark drifts, however hard it's hit, and no shield comes
/// back up. (A shield can arrive after the ship turns into a wreck, so it's
/// held down here rather than once.)
fn hold_still(mut hulks: Query<(&mut Velocity, Option<&mut crate::combat::shields::ShipShield>), With<FrozenHulk>>) {
    for (mut v, shield) in hulks.iter_mut() {
        if v.0 != Vec2::ZERO {
            v.0 = Vec2::ZERO;
        }
        if let Some(mut s) = shield {
            if s.enabled || s.current > 0.0 {
                s.enabled = false;
                s.current = 0.0;
            }
        }
    }
}

pub struct DarkScenePlugin;

impl Plugin for DarkScenePlugin {
    fn build(&self, app: &mut App) {
        use crate::states::GameState;
        app.init_resource::<DarkView>()
            .add_systems(Startup, make_veil_texture)
            .add_systems(
                Update,
                (measure, freeze_the_fleets, still_the_hulks, hold_still).run_if(in_state(GameState::Exploring)),
            )
            .add_systems(
                PostUpdate,
                (draw_veil, put_out_stars, drain_the_void)
                    .before(bevy::transform::TransformSystems::Propagate)
                    .run_if(in_state(GameState::Exploring)),
            )
            .add_systems(OnEnter(GameState::MainMenu), clear_the_veil);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(dark: f32) -> StarSystemDef {
        let mut s = crate::celestial::galaxy::generate_galaxy_map(42).systems[3].clone();
        s.dark = dark;
        s
    }

    /// Taken is fully dark; reaching dims part way, never past half.
    #[test]
    fn taken_is_dark_and_reaching_only_dims() {
        assert_eq!(target_level(Some(&sys(1.0))), 1.0);
        assert_eq!(target_level(Some(&sys(0.0))), 0.0);
        let reaching = target_level(Some(&sys(0.8)));
        assert!(reaching > 0.3 && reaching <= 0.5, "{reaching}");
        assert_eq!(target_level(None), 0.0);
    }

    /// The light drains over several seconds, not in one frame.
    #[test]
    fn the_light_drains_slowly() {
        let mut level = 0.0;
        for _ in 0..60 {
            level = ease(level, 1.0, 1.0 / 60.0);
        }
        assert!(level > 0.05 && level < 0.3, "after a second: {level}");
        for _ in 0..600 {
            level = ease(level, 1.0, 1.0 / 60.0);
        }
        assert_eq!(level, 1.0);
    }

    /// Most stars go out in a taken system, none in a lit one, and a star
    /// that's out at a lower level stays out deeper in.
    #[test]
    fn stars_go_out_in_order() {
        let out = |level: f32| (0..2000u32).filter(|i| star_out(*i, level)).count();
        assert_eq!(out(0.0), 0);
        let half = out(0.5);
        let full = out(1.0);
        assert!(full > 1700 && full < 1950, "{full}");
        assert!(half > 300 && half < full);
        assert!((0..2000u32).all(|i| !star_out(i, 0.5) || star_out(i, 1.0)));
    }

    /// The light reaches past the hull, and a failing ward lets it close in.
    #[test]
    fn a_failing_ward_lets_the_dark_in() {
        let lit = light_reach(800.0, 0.0);
        assert!(lit > 800.0 * 0.9);
        assert!(light_reach(800.0, 45.0) < lit * 0.55);
        assert_eq!(light_reach(800.0, 90.0), light_reach(800.0, 45.0));
    }

    /// The scene is the same every visit, and nothing in it sits on the
    /// station.
    #[test]
    fn the_scene_is_where_it_was_left() {
        let sides = (AiShipType::TerranHegemony, AiShipType::CorpseStars);
        let a = plan_scene(7, Vec2::ZERO, 2_000.0, sides);
        let b = plan_scene(7, Vec2::ZERO, 2_000.0, sides);
        assert_eq!(a, b);
        assert!(a.0.len() >= 3 + 4);
        assert!(a.0.iter().all(|h| h.pos.length() > 2_000.0 + 1_000.0));
        assert!(a.0.iter().any(|h| h.faction == sides.0) && a.0.iter().any(|h| h.faction == sides.1));
        assert!(!a.1.is_empty());
    }
}
