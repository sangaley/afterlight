//! Zoomed out, your ship reads as its parts, Cosmoteer's way.
//!
//! Close up the ship is a cutaway: decks, rooms, crew walking the halls. From
//! far out the decks and the crew on them are noise the size of a pixel. Past
//! ROOF_FROM zoom the bare floors -- corridors and deck cells with nothing
//! built on them -- are covered in light riveted deck plates, and the
//! hull's skin and armour take a darker shade, so the ship is a dark frame
//! around light decking with every part standing out on it.
//!
//! Every part stays on show. This used to roof over the machinery too, in one
//! grey of the hull's colour: the ship read as a solid slab and you couldn't
//! tell a reactor from a gun from a cargo hold. Cosmoteer never covers its
//! parts -- each one shows its own top -- and that is what you look at here:
//! the module art itself, reactors glowing, engines and guns where they sit.
//!
//! Tiles only stand while something stands under them: shoot a hole and the
//! deck opens there too. Zoomed in there are no tiles at all.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use crate::ai_ship::livery::{Livery, Pattern};

use crate::ai_ship::components::OwnedByAiShip;
use crate::building::{footprints, grid_to_local, ShipGrid};
use crate::camera::CameraState;
use crate::components::{DestroyedModule, HullDestroyed, HullLayer, HullMaterial, HullSegment, Module, ModuleCategory, PressureOverlayVisible, Ship};

/// Zoom where the roof starts to show, and where it's solid.
const ROOF_FROM: f32 = 3.5;
const ROOF_SOLID: f32 = 5.5;
/// Above modules (0.2-0.4), damage overlays (0.35) and crew (0.6).
const ROOF_Z: f32 = 0.7;

/// One cell of the roof, a child of the ship: deck plating or the armour
/// shade. Carries its alpha when fully zoomed out.
#[derive(Component)]
pub struct RoofTile(pub f32);

/// The decking over bare floors: lighter than the hull it covers, so the
/// insides read as one clean surface the parts stand on (Cosmoteer's light
/// corridor roofs) rather than as more of the same grey as the skin.
const DECK_LIGHTEN: f32 = 1.3;

/// How dark the armour frame goes: a black shade at this alpha over the
/// outer hull and the armour plates.
const FRAME_SHADE: f32 = 0.38;

/// Just over the hull (0.1) and the modules (0.2): covers the outer skin and
/// the armour plates, under everything that stands above them.
const FRAME_Z: f32 = 0.25;

/// Armour plates: part of the frame, not parts to pick out.
fn is_armour(module: crate::components::ModuleType) -> bool {
    use crate::components::ModuleType::*;
    matches!(
        module,
        ArmorPlate | CornerArmorPlate | StaggeredArmorPlate | AblativeArmor | ReflectivePlate | HullReinforcePlate
    )
}

/// How solid the roof is at a zoom.
pub fn roof_alpha(zoom: f32) -> f32 {
    ((zoom - ROOF_FROM) / (ROOF_SOLID - ROOF_FROM)).clamp(0.0, 1.0)
}

/// Looking inside your own ship (the toolbar's Inside button, or `.`): the
/// camera closes in until the ship fills the screen and the roof comes off.
/// Q's air view does the same. Press again and the camera goes back to the
/// zoom it came from.
#[derive(Resource, Default)]
pub struct SeeInside {
    pub on: bool,
    /// The zoom to go back to.
    back_to: Option<f32>,
}

impl SeeInside {
    pub fn is_on(&self) -> bool {
        self.on
    }
}

/// The key that looks inside. Every letter is taken in flight.
pub const SEE_INSIDE_KEY: KeyCode = KeyCode::Period;

/// How much of the screen's height the ship takes when you look inside.
const INSIDE_FILL: f32 = 0.75;

/// The zoom that fills `fill` of a view this tall with a ship this wide.
pub fn inside_zoom(ship_radius: f32, view_height: f32) -> f32 {
    if ship_radius <= 0.0 || view_height <= 0.0 {
        return crate::camera::DEFAULT_ZOOM;
    }
    2.0 * ship_radius / (INSIDE_FILL * view_height)
}

/// `.` toggles the inside view; Q (the air view) follows it in and out. Runs
/// before the air view's own toggle, so it reads whether that was up.
#[allow(clippy::too_many_arguments)]
pub fn toggle_see_inside(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inside: ResMut<SeeInside>,
    mut camera: ResMut<CameraState>,
    ships: Query<(&Children, Has<PressureOverlayVisible>), With<Ship>>,
    blocks: Query<&Transform, Or<(With<Module>, With<HullSegment>)>>,
    windows: Query<&Window>,
) {
    let Ok((children, air_view)) = ships.single() else { return };
    let want = if keyboard.just_pressed(SEE_INSIDE_KEY) {
        !inside.on
    } else if keyboard.just_pressed(KeyCode::KeyQ) {
        // The air view is about to flip: inside goes with it.
        !air_view
    } else {
        return;
    };
    if want == inside.on {
        return;
    }
    inside.on = want;
    if want {
        let radius = children
            .iter()
            .filter_map(|c| blocks.get(c).ok())
            .map(|t| t.translation.truncate().length() + 47.0)
            .fold(0.0_f32, f32::max);
        let height = windows.iter().next().map_or(720.0, |w| w.height());
        inside.back_to = Some(camera.zoom.max(camera.fight_zoom.unwrap_or(0.0)));
        camera.user_zoomed = true;
        camera.fight_framing_off |= camera.fight_zoom.is_some();
        camera.zoom = inside_zoom(radius, height).clamp(camera.min_zoom, camera.max_zoom);
    } else if let Some(z) = inside.back_to.take() {
        camera.zoom = z.clamp(camera.min_zoom, camera.max_zoom);
    }
}

/// How solid your own ship's roof is: none while you're looking inside, or
/// while the air view is up -- the roof covers the corridors, which are most
/// of what that view tints.
fn own_roof_alpha(zoom: f32, inside: &SeeInside, air_view: bool) -> f32 {
    if inside.on || air_view { 0.0 } else { roof_alpha(zoom) }
}

/// Whether your ship's roof shows the way it does in flight, is held on for
/// the builder's paint mode, or is off (docked, not painting).
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlayerRoofView<'w> {
    state: Option<Res<'w, State<crate::states::GameState>>>,
    paint: Option<Res<'w, crate::building::paint_mode::PaintMode>>,
}

impl PlayerRoofView<'_> {
    pub fn alpha(&self, flying: f32) -> f32 {
        if self.paint.as_ref().is_some_and(|p| p.on) {
            return 1.0;
        }
        match self.state.as_ref().map(|s| *s.get()) {
            Some(crate::states::GameState::Exploring) | None => flying,
            _ => 0.0,
        }
    }
}

/// Which cells the deck plating covers: every interior floor cell -- a hull
/// cell that is not the outer skin -- with no part standing on it. Parts are
/// never covered; each module is its footprint and whether it stands above
/// the hull (kept for callers, all modules show now).
pub fn roof_cells(hull: &[(IVec2, HullLayer)], modules: &[(Vec<IVec2>, bool)]) -> HashSet<IVec2> {
    let built: HashSet<IVec2> = modules.iter().flat_map(|(cells, _)| cells.iter().copied()).collect();
    hull.iter()
        .filter(|(c, layer)| *layer != HullLayer::Outer && !built.contains(c))
        .map(|(c, _)| *c)
        .collect()
}

/// The darker frame: the outer skin wherever no part sits on it, and every
/// armour plate's footprint.
pub fn frame_cells(hull: &[(IVec2, HullLayer)], modules: &[(Vec<IVec2>, bool, bool)]) -> HashSet<IVec2> {
    let built: HashSet<IVec2> = modules
        .iter()
        .filter(|(_, _, armour)| !armour)
        .flat_map(|(cells, _, _)| cells.iter().copied())
        .collect();
    let mut frame: HashSet<IVec2> = hull
        .iter()
        .filter(|(c, layer)| *layer == HullLayer::Outer && !built.contains(c))
        .map(|(c, _)| *c)
        .collect();
    for (cells, _, armour) in modules {
        if *armour {
            frame.extend(cells.iter().copied());
        }
    }
    frame
}

/// Lay your ship's roof when it comes into view or the ship changes under
/// it, and take it away when you zoom back in. `fit_roofs` keeps it sized
/// and lit from frame to frame.
#[allow(clippy::too_many_arguments)]
pub fn update_ship_roof(
    mut commands: Commands,
    assets: Res<AssetServer>,
    camera: Res<CameraState>,
    inside: Res<SeeInside>,
    view: PlayerRoofView,
    ships: Query<(Entity, &Scars, Has<PressureOverlayVisible>, &GlobalTransform, Option<&ShipPaint>), With<Ship>>,
    hull: Query<(&HullSegment, &ChildOf), (Without<HullDestroyed>, Without<OwnedByAiShip>)>,
    modules: Query<(&Module, &ChildOf), (Without<DestroyedModule>, Without<OwnedByAiShip>)>,
    tiles: Query<(Entity, &ChildOf), With<RoofTile>>,
    mut last: Local<Option<(Entity, usize, usize, usize, u64)>>,
) {
    let Ok((ship, scars, air_view, ship_gt, paint)) = ships.single() else { return };
    let alpha = view.alpha(own_roof_alpha(camera.zoom, &inside, air_view));
    let mine = |t: &(Entity, &ChildOf)| t.1.parent() == ship;
    if alpha <= 0.0 {
        if last.is_some() {
            for (tile, _) in tiles.iter().filter(mine) {
                commands.entity(tile).try_despawn();
            }
            *last = None;
        }
        return;
    }
    let mine_hull: Vec<&HullSegment> = hull.iter().filter(|(_, p)| p.parent() == ship).map(|(h, _)| h).collect();
    let mine_modules: Vec<&Module> = modules.iter().filter(|(_, p)| p.parent() == ship).map(|(m, _)| m).collect();
    let paint = paint.cloned().unwrap_or_default();
    let key = (ship, mine_hull.len(), mine_modules.len(), scars.0.len(), paint.stamp());
    // Tiles cleared from outside (clear_ship_roof) are laid again too.
    if *last == Some(key) && tiles.iter().any(|t| mine(&t)) {
        return;
    }
    *last = Some(key);
    for (tile, _) in tiles.iter().filter(mine) {
        commands.entity(tile).try_despawn();
    }
    let turn = ship_gt.to_scale_rotation_translation().1;
    lay_roof(&mut commands, &assets, ship, (camera.zoom, turn, alpha), &mine_hull, &mine_modules, scars, &paint.resolve(), false);
}

/// Lay a ship's whole roof: plates, skin, layers, angles, art and edges.
#[allow(clippy::too_many_arguments)]
pub fn lay_roof(
    commands: &mut Commands,
    assets: &AssetServer,
    ship: Entity,
    (zoom, turn, alpha): (f32, Quat, f32),
    mine_hull: &[&HullSegment],
    mine_modules: &[&Module],
    scars: &Scars,
    paint: &Paintwork,
    wrecked: bool,
) {
    let cells: Vec<(IVec2, HullLayer)> = mine_hull.iter().map(|s| (s.grid_position, s.hull_layer)).collect();
    let footprints: Vec<(Vec<IVec2>, bool, bool)> = mine_modules
        .iter()
        .map(|m| {
            let cells = ShipGrid::cells_for(m.grid_position, m.size, m.rotation, footprints::footprint_override(m.module_type)).to_vec();
            (cells, stands_above(m), is_armour(m.module_type))
        })
        .collect();
    // AFTERLIGHT_ROOF_ALL=1 (a preview): the roof covers every part but the
    // guns and engines, which break through it.
    let roof_all = roof_all();
    let hull_cells: HashSet<IVec2> = mine_hull.iter().map(|s| s.grid_position).collect();
    let shown: Vec<bool> = mine_modules
        .iter()
        .zip(&footprints)
        .map(|(m, (cells, _, _))| !roof_all || shows_through(m, cells, &hull_cells))
        .collect();
    let parts: Vec<(Vec<IVec2>, bool)> = footprints.iter().zip(&shown).filter(|(_, s)| **s).map(|((c, above, _), _)| (c.clone(), *above)).collect();
    let (roof, burnt) = scarred(roof_cells(&cells, &parts), &scars.0);
    // Where the walkways are (the roof over bare floor): the low deck. The
    // roof over the rooms stands a step above it.
    // Doors and bracing sit flush with the walkways: they're the walls
    // between rooms, not rooms.
    let all_parts: Vec<(Vec<IVec2>, bool)> = footprints
        .iter()
        .zip(mine_modules)
        .filter(|(_, m)| !roof_all || !flush(m.module_type))
        .map(|((c, above, _), _)| (c.clone(), *above))
        .collect();
    let low: HashSet<IVec2> = if roof_all { roof_cells(&cells, &all_parts) } else { roof.clone() };
    // Armour plates are skin too: left out, they showed their bare art in
    // steps between the angled armour along every slant.
    let framed: Vec<(Vec<IVec2>, bool, bool)> = footprints.iter().zip(&shown).filter(|((_, _, armour), s)| **s || *armour).map(|(f, _)| f.clone()).collect();
    let frame = frame_cells(&cells, if roof_all { &framed } else { &footprints });
    // The ship's skin, cell by cell; the roof is the commonest of it.
    let skin: HashMap<IVec2, HullMaterial> = mine_hull.iter().map(|s| (s.grid_position, s.material)).collect();
    let mut counts: Vec<(HullMaterial, usize)> = Vec::new();
    for m in skin.values() {
        match counts.iter_mut().find(|(k, _)| k == m) {
            Some((_, n)) => *n += 1,
            None => counts.push((*m, 1)),
        }
    }
    let material = counts.into_iter().max_by_key(|(_, n)| *n).map_or(HullMaterial::Steel, |(m, _)| m);

    // Laid in plates of 2x2 cells (tools/art/gen_roof_panels.py): flat in
    // the hull's colour with a seam and rivets round each plate, and each
    // plate a shade off its neighbours. Tiling the hull texture itself shaded
    // every cell top to bottom and read as corrugated sheet; one flat colour
    // read as no surface at all. Painted, the plates are a quieter print of
    // the same art: full-contrast seams and rivets under paint read as a grid.
    let image = assets.load(if roof_all { roof_path_quiet(material) } else { roof_path(material) });
    let types: Vec<crate::components::ModuleType> = mine_modules.iter().map(|m| m.module_type).collect();
    let bridge = bridge_cells(&footprints, &types);
    let raised: HashSet<IVec2> = roof.iter().copied().filter(|c| !low.contains(c)).collect();
    let walkway: HashSet<IVec2> = roof.iter().copied().filter(|c| low.contains(c)).collect();
    let rooms = rooms_of(&footprints, &types, Some(&raised));
    let room_size: HashMap<usize, usize> = raised.iter().fold(HashMap::new(), |mut m, c| {
        *m.entry(rooms[c]).or_insert(0) += 1;
        m
    });
    // Rooms with roof art of their own lie flat: no centre plate over the art.
    let art_rooms: HashSet<usize> = if roof_art() {
        let mut kinds: HashMap<usize, Option<crate::components::ModuleType>> = HashMap::new();
        for ((cells, _, _), t) in footprints.iter().zip(&types) {
            for c in cells.iter().filter(|c| raised.contains(*c)) {
                let Some(r) = rooms.get(c) else { continue };
                let k = kinds.entry(*r).or_insert(Some(*t));
                if *k != Some(*t) {
                    *k = None;
                }
            }
        }
        kinds.into_iter().filter(|(_, k)| k.is_some_and(|t| art_kind(t).is_some())).map(|(r, _)| r).collect()
    } else {
        HashSet::new()
    };
    let height = layers(&walkway, &raised, &bridge, &rooms, &room_size, &art_rooms);
    // Each room's kind, or None for a bay of mixed parts.
    let room_kind: HashMap<usize, Option<ModuleCategory>> = {
        let mut kinds: HashMap<usize, Option<ModuleCategory>> = HashMap::new();
        for ((cells, _, _), t) in footprints.iter().zip(&types) {
            for c in cells.iter().filter(|c| raised.contains(*c)) {
                let Some(r) = rooms.get(c) else { continue };
                let k = kinds.entry(*r).or_insert(Some(t.category()));
                if *k != Some(t.category()) {
                    *k = None;
                }
            }
        }
        kinds
    };
    let (lo, hi) = hull_cells.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
    let livery = paint.livery;
    let scheme = paint.scheme;
    let (blo, bhi) = bridge.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
    // A roof cell's colour, before the plate texture.
    let painted = |cell: IVec2| -> Color {
        let tone = plate_tone(cell.div_euclid(IVec2::splat(2)));
        if !roof_all {
            let tone = tone * DECK_LIGHTEN;
            return Color::srgb(tone, tone, tone);
        }
        // A tile painted by hand in the builder wears that paint, the
        // bridge's too.
        if let Some(slot) = paint.slots.get(&cell) {
            let layer = if bridge.contains(&cell) {
                BRIDGE
            } else if frame.contains(&cell) && !roof.contains(&cell) {
                SKIN
            } else {
                height.get(&cell).copied().unwrap_or(ROOM)
            };
            return over_plate(scheme.colour(*slot, layer) * tone, material);
        }
        if bridge.contains(&cell) {
            let has_glass = height.values().any(|h| *h == GLASS);
            if !has_glass || height.get(&cell) == Some(&GLASS) {
                return over_plate(glass(cell, blo, bhi), material);
            }
            // The bridge's frame: the lightest paint on the ship.
            return over_plate(scheme.main * 0.98 * tone, material);
        }
        let layer = if frame.contains(&cell) && !roof.contains(&cell) { SKIN } else { height.get(&cell).copied().unwrap_or(ROOM) };
        let band = accent_at(&livery, cell, lo, hi);
        let slot = paint_slot(layer, band);
        // The hallways' trim warm too: sand, and burnt orange where the
        // band crosses -- the arms are mostly hallway.
        if layer == TRIM && scheme.warm {
            let paint = if band { Vec3::new(0.82, 0.44, 0.17) } else { Vec3::new(0.76, 0.67, 0.5) };
            return over_plate(paint * 0.66 * tone, material);
        }
        // Rooms in their own warm paints, by what they are.
        if slot == Slot::Main && matches!(layer, ROOM | PLATE) {
            if let Some(warm) = rooms.get(&cell).and_then(|r| room_kind.get(r)).and_then(|k| scheme.room(*k)) {
                return over_plate(warm * if layer == PLATE { 0.9 } else { 0.82 } * tone, material);
            }
        }
        over_plate(scheme.colour(slot, layer) * tone, material)
    };
    // A wreck's roof is burnt dull.
    let colour = |cell: IVec2| -> Color {
        let c = painted(cell);
        if wrecked { crate::ai_ship::components::wreck_shade(c) } else { c }
    };
    for &cell in &roof {
        let q = cell.rem_euclid(IVec2::splat(2));
        // Image rows run top-down; grid y runs up.
        let rect = Rect::new(q.x as f32 * 128.0, (1 - q.y) as f32 * 128.0, q.x as f32 * 128.0 + 128.0, (2 - q.y) as f32 * 128.0);
        commands.spawn((
            Sprite {
                image: image.clone(),
                rect: Some(rect),
                color: colour(cell).with_alpha(alpha),
                custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                ..default()
            },
            Transform::from_translation(grid_to_local(cell).extend(ROOF_Z + lift(height.get(&cell).copied().unwrap_or(ROOM)))),
            RoofTile(1.0),
            ChildOf(ship),
        ));
    }
    for cell in burnt {
        commands.spawn((
            Sprite {
                color: Color::srgba(SCAR.0, SCAR.1, SCAR.2, SCAR_SHADE * alpha),
                custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                ..default()
            },
            Transform::from_translation(grid_to_local(cell).extend(ROOF_Z)),
            RoofTile(SCAR_SHADE),
            ChildOf(ship),
        ));
    }
    for cell in &frame {
        if roof_all {
            // The hull skin and armour: a layer of plating of its own, the
            // lowest, painted -- solid, so angle pieces cut over it match.
            let q = cell.rem_euclid(IVec2::splat(2));
            let rect = Rect::new(q.x as f32 * 128.0, (1 - q.y) as f32 * 128.0, q.x as f32 * 128.0 + 128.0, (2 - q.y) as f32 * 128.0);
            commands.spawn((
                Sprite { image: image.clone(), rect: Some(rect), color: colour(*cell).with_alpha(alpha), custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)), ..default() },
                Transform::from_translation(grid_to_local(*cell).extend(ROOF_Z)),
                RoofTile(1.0),
                ChildOf(ship),
            ));
            continue;
        }
        let (shade, share) = (Color::BLACK, FRAME_SHADE);
        commands.spawn((
            Sprite {
                color: shade.with_alpha(share * alpha),
                custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                ..default()
            },
            Transform::from_translation(grid_to_local(*cell).extend(FRAME_Z)),
            RoofTile(share),
            ChildOf(ship),
        ));
    }
    if roof_all {
        let lay = Lay { ship, zoom, turn, alpha, trim: scheme.trim };
        let tri = assets.load(roof_path_tri(material));
        if let Some(path) = std::env::var_os("AFTERLIGHT_ROOF_DUMP") {
            // For mock-ups: every hull cell, what it is under the roof.
            let kind: HashMap<IVec2, crate::components::ModuleType> = footprints.iter().zip(&types).flat_map(|((c, _, _), t)| c.iter().map(move |c| (*c, *t))).collect();
            let mut out = String::new();
            for c in &hull_cells {
                let class = if bridge.contains(c) { "bridge" } else if raised.contains(c) { "room" } else if walkway.contains(c) { "walk" } else if frame.contains(c) { "frame" } else { "hole" };
                let t = kind.get(c).map(|t| format!("{t:?}")).unwrap_or_default();
                let cat = kind.get(c).map(|t| format!("{:?}", t.category())).unwrap_or_default();
                out += &format!("{} {} {} {} {} {}\n", c.x, c.y, class, rooms.get(c).copied().unwrap_or(0), t, cat);
            }
            for m in mine_modules {
                if !hull_cells.contains(&m.grid_position) {
                    out += &format!("{} {} armour 0 {:?} {:?}\n", m.grid_position.x, m.grid_position.y, m.module_type, m.rotation);
                }
            }
            let _ = std::fs::write(path, out);
        }
        lay_marks(commands, &lay, assets, (&footprints, &types, &scheme), &rooms, &raised, &bridge);
        // The hallway floors' grating.
        for (cell, h) in &height {
            if *h == FLOOR {
                commands.spawn((
                    Sprite {
                        image: assets.load("sprites/roof_marks/grate.png"),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.32 * alpha),
                        custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                        ..default()
                    },
                    Transform::from_translation(grid_to_local(*cell).extend(ROOF_Z + 0.004)),
                    RoofTile(0.32),
                    ChildOf(ship),
                ));
            }
        }
        let surface: HashSet<IVec2> = roof.union(&frame).copied().collect();
        let shell: HashSet<IVec2> = hull_cells.union(&frame).copied().collect();
        // Open space against the hull with nothing built in it: where the
        // hull's outline steps, an angle piece can fill the notch.
        let occupied: HashSet<IVec2> = footprints.iter().flat_map(|(c, _, _)| c.iter().copied()).collect();
        let outside: HashSet<IVec2> = shell
            .iter()
            .flat_map(|c| [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y].map(|d| *c + d))
            .filter(|n| !shell.contains(n) && !occupied.contains(n))
            .collect();
        let mut closed = lay_outline(commands, &lay, &tri, &frame, &outside, &colour);
        closed.extend(lay_angles(commands, &lay, &tri, (&rooms, &room_size, &height), &surface, &colour));
        // Angled armour: where each sits and which way its slope faces.
        let wedges: Vec<(IVec2, Vec2)> = mine_modules
            .iter()
            .filter(|m| is_wedge(m.module_type))
            .map(|m| (m.grid_position, Vec2::from_angle(std::f32::consts::FRAC_PI_4 + m.rotation.to_radians())))
            .collect();
        lay_depth(commands, &lay, (&rooms, &bridge, &closed, &wedges), &height, &shell);
    }
}

/// Toward the light: the opposite of the way block shadows fall
/// (vfx::block_height). World-fixed, so as the ship turns its edges light
/// and shade the way its guns' shadows already do.
const LIGHT_TO: Vec2 = Vec2::new(-0.6, 0.8);

/// What an edge of the full roof is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeKind {
    /// A room roof's own edge where it stands over a walkway: lit toward the
    /// light, dark away from it.
    Lip,
    /// The shadow a room roof throws on the deck beside it.
    Step,
    /// The hull's lit rim.
    Rim,
    /// The shadow the whole hull throws on space.
    Drop,
    /// The bridge's lit windows, along its front.
    Window,
    /// Light caught on the bridge glass.
    Glint,
    /// A whole cell's shadow thrown off a tall part, away from the light.
    Cast,
}

/// One edge of the full roof: a strip along a straight edge, square to the
/// grid or at 45 degrees. Its width holds a few screen pixels however far
/// out the camera goes -- an edge a fixed few world units wide is gone by the
/// zoom you fly at -- and its shade follows the light as the ship turns.
#[derive(Component, Clone, Copy, Debug)]
pub struct Edge {
    /// Middle of the edge, ship-local.
    at: Vec2,
    /// Out of the shape it bounds, ship-local, unit length.
    normal: Vec2,
    /// Along the edge.
    length: f32,
    kind: EdgeKind,
    /// How many layers it drops: a shadow falls further off a taller step.
    rise: f32,
    /// The ship's trim paint, for its outline.
    tint: Vec3,
}

impl Edge {
    /// World width at a zoom: so many screen pixels, within bounds that keep
    /// it a line close in and stop it swallowing the cell far out.
    pub fn width(&self, zoom: f32) -> f32 {
        let (px, least, most) = match self.kind {
            EdgeKind::Lip => (2.0, 5.0, 16.0),
            EdgeKind::Rim => (1.5, 4.0, 12.0),
            EdgeKind::Step => (1.75 * self.rise, 7.0 * self.rise, (17.0 * self.rise).min(60.0)),
            EdgeKind::Drop => (5.0, 22.0, 70.0),
            EdgeKind::Window => (2.4, 7.0, 20.0),
            EdgeKind::Glint => (3.0, 8.0, 22.0),
            // For a cast shadow: how far it falls.
            EdgeKind::Cast => (10.0, 40.0, 120.0),
        };
        (px * zoom).clamp(least, most)
    }

    fn inside(&self) -> bool {
        matches!(self.kind, EdgeKind::Lip | EdgeKind::Rim | EdgeKind::Window)
    }

    /// Colour and how much of it, facing the light by `facing` (1 straight
    /// at it, -1 straight away).
    pub fn shade(&self, facing: f32) -> (Vec3, f32) {
        const WHITE: Vec3 = Vec3::new(1.0, 0.97, 0.92);
        let away = (-facing).max(0.0);
        match self.kind {
            EdgeKind::Lip if facing > 0.0 => (WHITE, 0.42 * facing),
            EdgeKind::Lip => (Vec3::ZERO, 0.75 * away),
            // The hull's outline in the trim paint, brighter where it faces
            // the light.
            EdgeKind::Rim => (self.tint, 0.55 + 0.35 * facing.max(0.0)),
            EdgeKind::Step => (Vec3::ZERO, (0.45 + 0.06 * self.rise).min(0.72) * away),
            EdgeKind::Drop => (Vec3::ZERO, 0.7 * away),
            EdgeKind::Window => (Vec3::new(0.62, 0.9, 1.0), 0.9),
            EdgeKind::Glint => (Vec3::new(0.85, 0.95, 1.0), 0.16),
            EdgeKind::Cast => (Vec3::ZERO, 0.5),
        }
    }

    /// Size, place and shade the strip for this zoom and ship heading.
    pub fn fit(&self, sprite: &mut Sprite, tf: &mut Transform, zoom: f32, turn: Quat, alpha: f32) {
        let w = self.width(zoom);
        if self.kind == EdgeKind::Cast {
            // The cell's square, slid away from the light (world-fixed, so
            // carried into the ship's frame).
            let away = (turn.inverse() * (-LIGHT_TO.normalize()).extend(0.0)).truncate();
            let centre = self.at + away * w;
            let (rgb, share) = self.shade(0.0);
            let colour = Color::srgba(rgb.x, rgb.y, rgb.z, share * alpha);
            let size = Vec2::splat(self.length);
            if sprite.custom_size != Some(size) {
                sprite.custom_size = Some(size);
            }
            if sprite.color != colour {
                sprite.color = colour;
            }
            if tf.translation.truncate() != centre {
                tf.translation.x = centre.x;
                tf.translation.y = centre.y;
            }
            return;
        }
        // Drawn across x, then turned to face along the normal.
        let size = Vec2::new(w, self.length);
        let centre = self.at + self.normal * (w * 0.5) * if self.inside() { -1.0 } else { 1.0 };
        let facing = (turn * self.normal.extend(0.0)).truncate().dot(LIGHT_TO.normalize());
        let (rgb, share) = self.shade(facing);
        let colour = Color::srgba(rgb.x, rgb.y, rgb.z, share * alpha);
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
        if sprite.color != colour {
            sprite.color = colour;
        }
        if tf.translation.truncate() != centre {
            tf.translation.x = centre.x;
            tf.translation.y = centre.y;
        }
        let rotation = Quat::from_rotation_z(self.normal.y.atan2(self.normal.x));
        if tf.rotation != rotation {
            tf.rotation = rotation;
        }
    }
}

/// What the edges are laid with: the ship, the camera's zoom, the ship's
/// heading and the roof's alpha.
struct Lay {
    ship: Entity,
    zoom: f32,
    turn: Quat,
    alpha: f32,
    /// The ship's trim paint.
    trim: Vec3,
}

impl Lay {
    fn edge(&self, commands: &mut Commands, at: Vec2, normal: Vec2, length: f32, kind: EdgeKind, z: f32) {
        self.edge_rise(commands, at, normal, length, kind, 2.0, z);
    }

    fn edge_rise(&self, commands: &mut Commands, at: Vec2, normal: Vec2, length: f32, kind: EdgeKind, rise: f32, z: f32) {
        let edge = Edge { at, normal, length, kind, rise, tint: self.trim };
        // Sized from the start: a sprite spawned at no size has no bounds to
        // draw within, and the renderer never shows it.
        let (mut sprite, mut tf) = (Sprite::default(), Transform::from_translation(at.extend(z)));
        edge.fit(&mut sprite, &mut tf, self.zoom, self.turn, self.alpha);
        commands.spawn((sprite, tf, RoofTile(1.0), edge, ChildOf(self.ship)));
    }

    /// Along the side of `cell` that faces `d`.
    fn side(&self, commands: &mut Commands, cell: IVec2, d: IVec2, kind: EdgeKind, z: f32) {
        self.side_rise(commands, cell, d, kind, 2.0, z);
    }

    fn side_rise(&self, commands: &mut Commands, cell: IVec2, d: IVec2, kind: EdgeKind, rise: f32, z: f32) {
        let g = crate::building::GRID_SIZE;
        self.edge_rise(commands, grid_to_local(cell) + d.as_vec2() * (g * 0.5), d.as_vec2(), g, kind, rise, z);
    }

    /// A right triangle of roof, its right angle at `corner` and its legs
    /// `leg` long running back from it along the grid.
    fn triangle(&self, commands: &mut Commands, image: &Handle<Image>, corner: Vec2, toward: IVec2, leg: f32, colour: Color, z: f32) {
        // The art is the lower-left half of a square: turn it so its right
        // angle sits at the corner asked for.
        let turn = match (toward.x, toward.y) {
            (-1, -1) => 0.0,
            (1, -1) => std::f32::consts::FRAC_PI_2,
            (1, 1) => std::f32::consts::PI,
            _ => -std::f32::consts::FRAC_PI_2,
        };
        let centre = corner - toward.as_vec2() * (leg * 0.5);
        commands.spawn((
            Sprite { image: image.clone(), color: colour.with_alpha(self.alpha), custom_size: Some(Vec2::splat(leg)), ..default() },
            Transform::from_translation(centre.extend(z)).with_rotation(Quat::from_rotation_z(turn)),
            RoofTile(1.0),
            ChildOf(self.ship),
        ));
    }
}

/// Above the roof's straight lips, the angle pieces and their own edges.
const ANGLE_Z: f32 = ROOF_Z + 0.0135;
const ANGLE_STEP_Z: f32 = ROOF_Z + 0.014;
const ANGLE_LIP_Z: f32 = ROOF_Z + 0.0145;

/// The hull's outline smoothed: where it steps like a stair -- a notch of
/// open space with hull skin on two sides that meet -- an angle piece of the
/// skin fills the notch's half against the hull, with the outline's trim and
/// the hull's shadow running along the slope. Returns the hull sides closed
/// over.
fn lay_outline(
    commands: &mut Commands,
    lay: &Lay,
    image: &Handle<Image>,
    skin: &HashSet<IVec2>,
    outside: &HashSet<IVec2>,
    colour: &dyn Fn(IVec2) -> Color,
) -> HashSet<(IVec2, IVec2)> {
    let g = crate::building::GRID_SIZE;
    let corners = [IVec2::new(1, 1), IVec2::new(-1, 1), IVec2::new(1, -1), IVec2::new(-1, -1)];
    let mut closed = HashSet::new();
    let mut cells: Vec<IVec2> = outside.iter().copied().collect();
    cells.sort_by_key(|c| (c.x, c.y));
    for w in cells {
        let hits: Vec<IVec2> = corners
            .iter()
            .copied()
            .filter(|s| skin.contains(&(w + IVec2::new(s.x, 0))) && skin.contains(&(w + IVec2::new(0, s.y))))
            .collect();
        let [s] = hits[..] else { continue };
        // A notch: open on its other two sides.
        if skin.contains(&(w - IVec2::new(s.x, 0))) || skin.contains(&(w - IVec2::new(0, s.y))) {
            continue;
        }
        let (a, b) = (w + IVec2::new(s.x, 0), w + IVec2::new(0, s.y));
        let corner = grid_to_local(w) + s.as_vec2() * (g * 0.5);
        lay.triangle(commands, image, corner, s, g, colour(a), ROOF_Z);
        closed.insert((a, IVec2::new(-s.x, 0)));
        closed.insert((b, IVec2::new(0, -s.y)));
        let normal = -s.as_vec2().normalize();
        let length = g * std::f32::consts::SQRT_2;
        lay.edge(commands, grid_to_local(w), normal, length, EdgeKind::Rim, ROOF_Z + 0.012);
        lay.edge(commands, grid_to_local(w), normal, length, EdgeKind::Drop, -0.05);
    }
    closed
}

/// Angles on the roof, Cosmoteer's way, between any two layers:
///
/// - Steps filled out: a cell with one layer standing over it on two sides
///   that meet takes that layer's half of the cell, so an edge that runs in
///   steps -- a hallway along a slanted hull, a room's ragged side -- is one
///   slope.
/// - Outer corners cut at 45 degrees where a layer's corner stands out over
///   a lower one: a cell's worth on a big room, half on everything else.
///
/// Returns the cell sides a fill has closed over, which get no lip.
#[allow(clippy::too_many_arguments)]
fn lay_angles(
    commands: &mut Commands,
    lay: &Lay,
    image: &Handle<Image>,
    (rooms, room_size, height): (&HashMap<IVec2, usize>, &HashMap<usize, usize>, &HashMap<IVec2, u8>),
    surface: &HashSet<IVec2>,
    colour: &dyn Fn(IVec2) -> Color,
) -> HashSet<(IVec2, IVec2)> {
    let g = crate::building::GRID_SIZE;
    let corners = [IVec2::new(1, 1), IVec2::new(-1, 1), IVec2::new(1, -1), IVec2::new(-1, -1)];
    let h = |c: IVec2| height.get(&c).copied().unwrap_or(SKIN);
    // What a layer is one piece of: a room, the hallways, the bridge.
    let group = |c: IVec2| -> Option<usize> {
        match h(c) {
            FLOOR | TRIM => Some(usize::MAX),
            BRIDGE | GLASS => Some(usize::MAX - 1),
            ROOM | PLATE => rooms.get(&c).copied(),
            _ => None,
        }
    };
    // A block or two on its own cut at every corner is a pill, and a ship
    // of pills reads as scattered: rooms of three cells or more only.
    let sized = |c: IVec2| match h(c) {
        ROOM | PLATE => rooms.get(&c).and_then(|r| room_size.get(r)).is_some_and(|n| *n >= 3),
        _ => true,
    };
    let mut closed = HashSet::new();
    let mut filled = HashSet::new();
    let mut cells: Vec<IVec2> = surface.iter().copied().collect();
    cells.sort_by_key(|c| (c.x, c.y));

    for &w in &cells {
        let hw = h(w);
        let hits: Vec<IVec2> = corners
            .iter()
            .copied()
            .filter(|s| {
                let (a, b) = (w + IVec2::new(s.x, 0), w + IVec2::new(0, s.y));
                surface.contains(&a) && surface.contains(&b) && h(a) == h(b) && h(a) > hw && group(a).is_some() && group(a) == group(b) && sized(a)
            })
            .collect();
        let [s] = hits[..] else { continue };
        // A notch, not a channel: nothing stands over it on its other sides.
        let (o1, o2) = (w - IVec2::new(s.x, 0), w - IVec2::new(0, s.y));
        if [o1, o2].iter().any(|o| surface.contains(o) && h(*o) > hw) {
            continue;
        }
        let (a, b) = (w + IVec2::new(s.x, 0), w + IVec2::new(0, s.y));
        let corner = grid_to_local(w) + s.as_vec2() * (g * 0.5);
        let up = lift(h(a));
        lay.triangle(commands, image, corner, s, g, colour(a), ANGLE_Z + up);
        closed.insert((a, IVec2::new(-s.x, 0)));
        closed.insert((b, IVec2::new(0, -s.y)));
        filled.insert(w);
        let normal = -s.as_vec2().normalize();
        lay.edge(commands, grid_to_local(w), normal, g * std::f32::consts::SQRT_2, EdgeKind::Lip, ANGLE_LIP_Z + up);
        lay.edge_rise(commands, grid_to_local(w), normal, g * std::f32::consts::SQRT_2, EdgeKind::Step, (h(a) - hw) as f32, ANGLE_STEP_Z + lift(hw));
    }

    for &c in &cells {
        let hc = h(c);
        if hc < TRIM || filled.contains(&c) || !sized(c) {
            continue;
        }
        let r = group(c);
        for s in corners {
            let (n1, n2, n3) = (c + IVec2::new(s.x, 0), c + IVec2::new(0, s.y), c + s);
            let open = |n: IVec2| surface.contains(&n) && !filled.contains(&n) && h(n) < hc;
            if !(open(n1) && open(n2) && open(n3)) {
                continue;
            }
            // How far each edge runs straight from the corner.
            let run = |step: IVec2, out: IVec2| {
                (0..8)
                    .take_while(|k| {
                        let p = c - step * *k;
                        h(p) == hc && group(p) == r && h(p + out) < hc
                    })
                    .count()
            };
            let along = run(IVec2::new(s.x, 0), IVec2::new(0, s.y)).min(run(IVec2::new(0, s.y), IVec2::new(s.x, 0)));
            if along < 2 {
                continue;
            }
            // One size of cut per kind, so corners repeat ship-wide.
            let leg = if hc == ROOM && along >= 4 { 1.0 } else { 0.5 } * g;
            let corner = grid_to_local(c) + s.as_vec2() * (g * 0.5);
            let up = lift(hc);
            lay.triangle(commands, image, corner, s, leg, colour(n3), ANGLE_Z + up);
            let normal = s.as_vec2().normalize();
            let mid = corner - s.as_vec2() * (leg * 0.5);
            lay.edge(commands, mid, normal, leg * std::f32::consts::SQRT_2, EdgeKind::Lip, ANGLE_LIP_Z + up);
            lay.edge_rise(commands, mid, normal, leg * std::f32::consts::SQRT_2, EdgeKind::Step, (hc - h(n3)) as f32, ANGLE_STEP_Z + up);
        }
    }
    closed
}

/// Depth for a full roof, in layers: the hull skin lowest, the hallways a
/// layer up -- trim along their sides, the floor between sunk a little --
/// the rooms above them, a big room's centre plate above that, and the
/// bridge highest. Every drop from one layer to the next has a lip, lit on
/// the side toward the light and dark on the other, and throws a shadow on
/// the layer below as long as the drop is deep. The whole hull has a lit rim
/// and throws its shadow on space, and the bridge has its windows lit along
/// the bow.
#[allow(clippy::too_many_arguments)]
fn lay_depth(
    commands: &mut Commands,
    lay: &Lay,
    (rooms, bridge, closed, wedges): (&HashMap<IVec2, usize>, &HashSet<IVec2>, &HashSet<(IVec2, IVec2)>, &[(IVec2, Vec2)]),
    height: &HashMap<IVec2, u8>,
    hull: &HashSet<IVec2>,
) {
    const SIDES: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];
    let g = crate::building::GRID_SIZE;
    for (&c, &h) in height {
        for d in SIDES {
            let n = c + d;
            if closed.contains(&(c, d)) || !hull.contains(&n) {
                continue;
            }
            let below = height.get(&n).copied().unwrap_or(SKIN);
            if h > below {
                lay.side(commands, c, d, EdgeKind::Lip, ROOF_Z + 0.012 + lift(h));
                lay.side_rise(commands, c, d, EdgeKind::Step, (h - below) as f32, ROOF_Z + 0.011 + lift(below).max(if h >= BRIDGE { 0.015 } else { 0.0 }));
            } else if h == below && h >= ROOM && rooms.get(&c) != rooms.get(&n) {
                // Two rooms side by side: each its own roof piece, with
                // its own lip where they meet.
                lay.side(commands, c, d, EdgeKind::Lip, ROOF_Z + 0.012);
            }
        }
    }
    // The windows: along the bridge's front (the bow is +x), a pane to a
    // cell, on the foremost cell of each row -- one unbroken row of glass.
    let glass: Vec<IVec2> = bridge.iter().copied().filter(|c| height.get(c) == Some(&GLASS)).collect();
    let glass: Vec<IVec2> = if glass.is_empty() { bridge.iter().copied().collect() } else { glass };
    let mut front: HashMap<i32, i32> = HashMap::new();
    for c in &glass {
        let x = front.entry(c.y).or_insert(c.x);
        *x = (*x).max(c.x);
    }
    for (y, x) in front {
        let cell = IVec2::new(x, y);
        let at = grid_to_local(cell) + Vec2::X * (g * 0.5 - g * 0.12);
        lay.edge(commands, at, Vec2::X, g * 0.8, EdgeKind::Window, ROOF_Z + 0.04);
    }
    // The bridge stands tall: its whole shape throws a shadow across the
    // roof beside it, away from the light.
    for c in bridge {
        lay.edge(commands, grid_to_local(*c), Vec2::X, g, EdgeKind::Cast, ROOF_Z + 0.015);
    }
    // One glint across the glass, corner to corner.
    if !glass.is_empty() {
        let (lo, hi) = glass.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        let centre = (grid_to_local(lo) + grid_to_local(hi)) * 0.5;
        let span = (hi - lo + IVec2::ONE).as_vec2() * g;
        let normal = Vec2::new(1.0, -1.0).normalize();
        lay.edge(commands, centre, normal, span.min_element() * 0.9, EdgeKind::Glint, ROOF_Z + 0.039);
    }
    // The outline runs along each angled plate's slope, not round the
    // square sides under it, which showed through its open half as a stair.
    let plated: HashSet<IVec2> = wedges.iter().map(|(c, _)| *c).collect();
    for (cell, normal) in wedges {
        let length = g * std::f32::consts::SQRT_2;
        lay.edge(commands, grid_to_local(*cell), *normal, length, EdgeKind::Rim, RAISED_Z + 0.09);
        lay.edge(commands, grid_to_local(*cell), *normal, length, EdgeKind::Drop, -0.05);
    }
    for &c in hull {
        for d in SIDES {
            if !hull.contains(&(c + d)) && !closed.contains(&(c, d)) && !plated.contains(&(c + d)) {
                lay.side(commands, c, d, EdgeKind::Rim, ROOF_Z + 0.012);
                lay.side(commands, c, d, EdgeKind::Drop, -0.05);
            }
        }
    }
}

/// Your ship's paint: weathered bone-grey plating with a rust-orange stripe
/// down each flank. AI ships wear their faction's (ai_ship::livery).
pub fn player_livery() -> Livery {
    Livery { base: Color::srgb(0.70, 0.68, 0.63), accent: Color::srgb(0.78, 0.44, 0.2), pattern: Pattern::Stripe }
}

/// Whether a cell takes the accent. The ship's bow is +x, as a livery reads.
fn accent_at(livery: &Livery, cell: IVec2, lo: IVec2, hi: IVec2) -> bool {
    livery.accent_at(cell, lo, hi)
}

/// The roof plate texture's average colour: painting divides it out, so
/// the paint comes out as the colour asked for with the plate's seams and
/// rivets still on it.
fn plate_mean(material: HullMaterial) -> Vec3 {
    match material {
        HullMaterial::Steel => Vec3::new(0.223, 0.26, 0.304),
        HullMaterial::Titanium => Vec3::new(0.301, 0.338, 0.382),
        HullMaterial::Composite => Vec3::new(0.19, 0.213, 0.258),
        HullMaterial::AbyssalAlloy => Vec3::new(0.174, 0.168, 0.242),
    }
}

/// A colour that turns the plate texture into `want` on average.
fn over_plate(want: Vec3, material: HullMaterial) -> Color {
    let lin = |c: Vec3| Color::srgb(c.x, c.y, c.z).to_linear();
    let (w, m) = (lin(want), lin(plate_mean(material)));
    Color::LinearRgba(LinearRgba::rgb(w.red / m.red, w.green / m.green, w.blue / m.blue))
}

/// Which of the ship's paints a tile takes, Cosmoteer's way: each tile is
/// painted on its own, so colours follow the structure rather than one
/// flood over all of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum Slot {
    /// The ship's main colour.
    Main,
    /// Its second colour: bands and panels.
    Second,
    /// The bright trim: the outline, the angled armour.
    Trim,
    /// Bare deck, unpainted.
    Bare,
    /// A colour of the player's own, picked in the builder (sRGB 0-255).
    Custom([u8; 3]),
}

/// A ship's three paints.
#[derive(Clone, Copy, Debug)]
pub struct Scheme {
    pub main: Vec3,
    pub second: Vec3,
    pub trim: Vec3,
    /// Rooms in a range of warm paints by what they are (`Scheme::room`).
    pub warm: bool,
}

impl Scheme {
    /// A tile's colour: its paint, shaded for its layer -- the skin darkest,
    /// higher layers catching more light.
    pub fn colour(&self, slot: Slot, layer: u8) -> Vec3 {
        let paint = match slot {
            Slot::Main => self.main,
            Slot::Second => self.second,
            Slot::Trim => self.trim,
            Slot::Bare => return Vec3::new(0.2, 0.21, 0.23),
            Slot::Custom(c) => Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) / 255.0,
        };
        let shade = match layer {
            SKIN => 0.55,
            TRIM => 0.62,
            ROOM => 0.82,
            PLATE => 0.9,
            _ => 0.8,
        };
        paint * shade
    }
}

impl Scheme {
    /// A room's own paint by what it is, in the Salvager's warm range: the
    /// inside of the ship in oranges, ambers and yellows rather than one
    /// colour. None keeps the scheme's main colour.
    pub fn room(&self, kind: Option<ModuleCategory>) -> Option<Vec3> {
        if !self.warm {
            return None;
        }
        Some(match kind {
            Some(ModuleCategory::Power) => Vec3::new(0.92, 0.70, 0.26),     // amber
            Some(ModuleCategory::Storage) => Vec3::new(0.80, 0.64, 0.24),   // mustard
            Some(ModuleCategory::LifeSupport) => Vec3::new(0.74, 0.62, 0.45), // sand
            Some(ModuleCategory::Propulsion) | Some(ModuleCategory::Guns) | Some(ModuleCategory::Weapons) => Vec3::new(0.66, 0.33, 0.18), // rust
            Some(ModuleCategory::Lasers) => Vec3::new(0.82, 0.44, 0.17),    // burnt orange
            Some(ModuleCategory::Utility) | Some(ModuleCategory::Detection) => Vec3::new(0.82, 0.76, 0.62), // cream
            Some(ModuleCategory::Crew) | Some(ModuleCategory::Control) => self.main, // bone
            Some(ModuleCategory::Structural) => self.second,
            None => Vec3::new(0.74, 0.62, 0.45), // a mixed bay: sand
        })
    }
}

/// The default paint for your ship (AFTERLIGHT_ROOF_PAINT picks another).
pub fn scheme() -> Scheme {
    static ON: std::sync::OnceLock<Scheme> = std::sync::OnceLock::new();
    *ON.get_or_init(|| scheme_named(std::env::var("AFTERLIGHT_ROOF_PAINT").ok().as_deref().unwrap_or("salvager")))
}

/// The ready-made schemes, by name: "salvager", "raider", "navy".
pub fn scheme_named(name: &str) -> Scheme {
    match name {
        // Gunmetal with deep red panels and bright red trim.
        "raider" => Scheme { main: Vec3::new(0.34, 0.34, 0.38), second: Vec3::new(0.52, 0.13, 0.13), trim: Vec3::new(0.95, 0.22, 0.18), warm: false },
        // Navy with white panels and cyan trim.
        "navy" => Scheme { main: Vec3::new(0.24, 0.31, 0.47), second: Vec3::new(0.86, 0.88, 0.9), trim: Vec3::new(0.42, 0.85, 1.0), warm: false },
        // Bone with charcoal panels and orange trim.
        _ => Scheme { main: Vec3::new(0.70, 0.68, 0.63), second: Vec3::new(0.33, 0.34, 0.37), trim: Vec3::new(0.86, 0.46, 0.18), warm: true },
    }
}

/// A ship's paint as the builder keeps it: its three colours and the tiles
/// painted by hand. On your ship; saved with it.
#[derive(Component, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShipPaint {
    pub main: [f32; 3],
    pub second: [f32; 3],
    pub trim: [f32; 3],
    /// Rooms in warm paints by what they are (the Salvager look).
    #[serde(default)]
    pub warm: bool,
    /// Tiles painted by hand, and with what.
    #[serde(default)]
    pub slots: Vec<(IVec2, Slot)>,
}

impl Default for ShipPaint {
    fn default() -> Self {
        Self::from_scheme(scheme())
    }
}

impl ShipPaint {
    pub fn from_scheme(s: Scheme) -> Self {
        Self { main: s.main.to_array(), second: s.second.to_array(), trim: s.trim.to_array(), warm: s.warm, slots: Vec::new() }
    }

    pub fn scheme(&self) -> Scheme {
        Scheme { main: Vec3::from_array(self.main), second: Vec3::from_array(self.second), trim: Vec3::from_array(self.trim), warm: self.warm }
    }

    /// Changes whenever the paint does, so the roof is laid again.
    pub fn stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for v in self.main.iter().chain(&self.second).chain(&self.trim) {
            v.to_bits().hash(&mut h);
        }
        self.warm.hash(&mut h);
        for (c, s) in &self.slots {
            (c.x, c.y, s).hash(&mut h);
        }
        h.finish()
    }

    /// What the roof is laid with.
    pub fn resolve(&self) -> Paintwork {
        Paintwork { scheme: self.scheme(), livery: player_livery(), slots: self.slots.iter().copied().collect() }
    }
}

/// Everything a roof's paint is decided by.
pub struct Paintwork {
    pub scheme: Scheme,
    /// Where the paint pattern's band runs.
    pub livery: Livery,
    /// Tiles painted by hand.
    pub slots: HashMap<IVec2, Slot>,
}

/// A faction's ship paint from its livery: the main colour its map colour
/// a little greyed toward plating, the second and the trim its accent.
pub fn faction_paint(faction: crate::ai_ship::components::AiShipType) -> Paintwork {
    let livery = crate::ai_ship::livery::livery(faction);
    let v = |c: Color| {
        let s = c.to_srgba();
        Vec3::new(s.red, s.green, s.blue)
    };
    let base = v(livery.base);
    let grey = Vec3::new(0.6, 0.6, 0.62);
    // Its pattern (bands, chevrons, a painted bow...) in its accent, so a
    // ship says whose it is from across the system.
    let accent = v(livery.accent);
    let scheme = Scheme { main: grey.lerp(base, 0.62), second: accent * 0.9, trim: accent, warm: false };
    Paintwork { scheme, livery, slots: HashMap::new() }
}

/// A tile's paint from its layer and whether the paint pattern's band runs
/// through it. Rooms are the main colour, the band across them the second;
/// a big room's centre plate takes the other one of the two, so it stands
/// out from the room it sits on; the hull skin is the main colour with the
/// band in trim, and the hallways are trimmed in the main colour around a
/// bare floor.
pub fn paint_slot(layer: u8, band: bool) -> Slot {
    match (layer, band) {
        (FLOOR, _) => Slot::Bare,
        (TRIM, _) => Slot::Main,
        (SKIN, true) => Slot::Trim,
        (SKIN, false) => Slot::Main,
        (PLATE, true) => Slot::Main,
        (PLATE, false) => Slot::Second,
        (_, true) => Slot::Second,
        _ => Slot::Main,
    }
}

/// The bridge canopy at a cell: dark glass, brighter toward its bow-left
/// corner where it catches the light (its glint is an edge, `lay_depth`).
fn glass(cell: IVec2, lo: IVec2, hi: IVec2) -> Vec3 {
    let span = (hi - lo).max(IVec2::ONE).as_vec2();
    let f = (cell - lo).as_vec2() / span;
    // Toward the bow (+x) and to port (+y).
    let lit = 0.5 * (f.x + f.y);
    BRIDGE_GLASS * (0.8 + 0.45 * lit)
}

/// The layers of a full roof, low to high.
const SKIN: u8 = 0;
const FLOOR: u8 = 2;
const TRIM: u8 = 3;
const ROOM: u8 = 5;
const PLATE: u8 = 6;
const BRIDGE: u8 = 8;
const GLASS: u8 = 10;

/// How far above the roof a layer's tiles and edges are drawn: the bridge
/// stands over everything round it, its shadow included.
fn lift(layer: u8) -> f32 {
    match layer {
        GLASS => 0.026,
        BRIDGE => 0.02,
        _ => 0.0,
    }
}

/// Each roofed cell's layer. A hallway cell with hallway all round is sunk
/// floor; one along a hallway's side is its trim. A room's cells are room
/// roof, but a big room (nine cells or more) has a centre plate standing on
/// it: every cell with its own room all round. The bridge stands highest.
pub fn layers(
    walkway: &HashSet<IVec2>,
    raised: &HashSet<IVec2>,
    bridge: &HashSet<IVec2>,
    rooms: &HashMap<IVec2, usize>,
    room_size: &HashMap<usize, usize>,
    flat: &HashSet<usize>,
) -> HashMap<IVec2, u8> {
    const ROUND: [IVec2; 8] = [
        IVec2::new(1, 0), IVec2::new(-1, 0), IVec2::new(0, 1), IVec2::new(0, -1),
        IVec2::new(1, 1), IVec2::new(1, -1), IVec2::new(-1, 1), IVec2::new(-1, -1),
    ];
    let mut height = HashMap::new();
    for &c in walkway {
        let sunk = ROUND[..4].iter().all(|d| walkway.contains(&(c + *d)));
        height.insert(c, if sunk { FLOOR } else { TRIM });
    }
    for &c in raised {
        let layer = if bridge.contains(&c) {
            // A frame round the bridge, the glass set in it a tier higher.
            if ROUND[..4].iter().all(|d| bridge.contains(&(c + *d))) { GLASS } else { BRIDGE }
        } else {
            let r = rooms.get(&c);
            let big = r.and_then(|r| room_size.get(r)).is_some_and(|n| *n >= 9) && !r.is_some_and(|r| flat.contains(r));
            let inside = ROUND.iter().all(|d| raised.contains(&(c + *d)) && !bridge.contains(&(c + *d)) && rooms.get(&(c + *d)) == r);
            if big && inside { PLATE } else { ROOM }
        };
        height.insert(c, layer);
    }
    height
}

/// The bridge: every command block joined to a bridge wing or the helm.
/// A navigation console off on its own elsewhere is a room like any other.
pub fn bridge_cells(footprints: &[(Vec<IVec2>, bool, bool)], types: &[crate::components::ModuleType]) -> HashSet<IVec2> {
    use crate::components::ModuleType::{BridgeWing, HelmStation};
    let command: HashSet<IVec2> = footprints.iter().zip(types).filter(|(_, t)| is_bridge(**t)).flat_map(|((c, _, _), _)| c.iter().copied()).collect();
    let mut bridge: HashSet<IVec2> = footprints
        .iter()
        .zip(types)
        .filter(|(_, t)| matches!(t, BridgeWing | HelmStation))
        .flat_map(|((c, _, _), _)| c.iter().copied())
        .collect();
    let mut todo: Vec<IVec2> = bridge.iter().copied().collect();
    while let Some(c) = todo.pop() {
        for d in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let n = c + d;
            if command.contains(&n) && bridge.insert(n) {
                todo.push(n);
            }
        }
    }
    bridge
}

/// Weapons stand above the hull: zoomed out they stay in sight over the
/// roof wherever they're mounted, barrels and all.
fn stands_above(module: &Module) -> bool {
    // Towers and masts too: whatever casts a height shadow stands taller
    // than the plating (vfx::block_height).
    module.module_type.category() == ModuleCategory::Weapons
        || super::block_height::block_height(module.module_type).is_some()
}

/// What breaks through a full roof: guns, the gun and laser mounts that show
/// outside, and engines and thrusters.
///
/// An engine or thruster deep inside the hull (a manoeuvring jet between
/// rooms) has no bell outside to show: it stays under the roof. Only one on
/// the skin or touching open space breaks through.
fn shows_through(module: &Module, cells: &[IVec2], hull: &HashSet<IVec2>) -> bool {
    use crate::components::ModuleType;
    const SIDES: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];
    match module.module_type.category() {
        ModuleCategory::Weapons => true,
        // A built gun's parts are roofed like any room; its barrels and the
        // ring a turret stands on come out through the roof.
        ModuleCategory::Guns => {
            crate::combat::turret_deck::OUT_FRONT.contains(&module.module_type)
                || matches!(module.module_type, ModuleType::TurretRingSmall | ModuleType::TurretRing | ModuleType::TurretRingLarge)
        }
        ModuleCategory::Propulsion => cells.iter().any(|c| !hull.contains(c) || SIDES.iter().any(|s| !hull.contains(&(*c + *s)))),
        _ => matches!(module.module_type, ModuleType::LaserEmitter),
    }
}

/// The bridge glass: a dark tinted canopy, the one roof that isn't paint.
const BRIDGE_GLASS: Vec3 = Vec3::new(0.11, 0.16, 0.23);

/// The command deck: under a full roof it wears a glass canopy with its
/// windows lit, and stands a deck above the rooms round it.
pub fn is_bridge(t: crate::components::ModuleType) -> bool {
    t.category() == ModuleCategory::Control
}

/// Armour cut on a slant: under a full roof it rides a layer of its own,
/// above the roof, in the ship's paint.
pub fn is_angled(t: crate::components::ModuleType) -> bool {
    use crate::components::ModuleType::*;
    matches!(
        t,
        AngledArmorPlate | AngledHullPlate | AngledAblativeArmor | AngledReflectivePlate | BevelledArmorPlate
            | SlopeArmorPlateLow | SlopeArmorPlateHigh | SlopeArmorPlateLowMirror | SlopeArmorPlateHighMirror
    )
}

/// Drawn as a staircase of child quads, not a sprite of its own
/// (ship::spawner).
fn is_wedge(t: crate::components::ModuleType) -> bool {
    use crate::components::ModuleType::*;
    matches!(t, AngledArmorPlate | AngledHullPlate | AngledAblativeArmor | AngledReflectivePlate)
}

/// Which room each built cell belongs to: touching blocks of the same kind
/// are one room, so a hold of twelve cargo blocks is one roof, not twelve.
///
/// With `raised` (the room roofs): tiny rooms of a block or two that touch
/// are one machinery bay, whatever their kinds.
pub fn rooms_of(footprints: &[(Vec<IVec2>, bool, bool)], types: &[crate::components::ModuleType], raised: Option<&HashSet<IVec2>>) -> HashMap<IVec2, usize> {
    let owner: HashMap<IVec2, usize> = footprints.iter().enumerate().flat_map(|(i, (cells, _, _))| cells.iter().map(move |c| (*c, i))).collect();
    let mut parent: Vec<usize> = (0..footprints.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for (cell, &i) in &owner {
        for d in [IVec2::X, IVec2::Y] {
            if let Some(&j) = owner.get(&(*cell + d)) {
                if i != j && types[i] == types[j] {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    parent[a] = b;
                }
            }
        }
    }
    if let Some(raised) = raised {
        let mut size: HashMap<usize, usize> = HashMap::new();
        for (cell, &i) in &owner {
            if raised.contains(cell) {
                *size.entry(find(&mut parent, i)).or_insert(0) += 1;
            }
        }
        let small = |p: &mut [usize], i: usize| size.get(&find(p, i)).is_some_and(|n| *n <= 2);
        for (cell, &i) in &owner {
            if !raised.contains(cell) {
                continue;
            }
            for d in [IVec2::X, IVec2::Y] {
                let n = *cell + d;
                if let (true, Some(&j)) = (raised.contains(&n), owner.get(&n)) {
                    if small(&mut parent, i) && small(&mut parent, j) {
                        let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                        parent[a] = b;
                    }
                }
            }
        }
    }
    owner.into_iter().map(|(c, i)| (c, find(&mut parent, i))).collect()
}

/// Walls between rooms rather than rooms: doors and bracing, flush with
/// the walkways under a full roof.
fn flush(t: crate::components::ModuleType) -> bool {
    use crate::components::ModuleType::*;
    t.category() == ModuleCategory::Structural || matches!(t, EmergencyBulkhead | FirebreakWall | AntiFlashDoor)
}

/// Under a full roof, the rooms with fittings on the outside of the hull
/// wear roof art of their own kind (tools/art/gen_roof_art.py) instead of
/// plain plates with a mark. AFTERLIGHT_ROOF_ART=0 turns it off.
pub fn roof_art() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| roof_all() && std::env::var("AFTERLIGHT_ROOF_ART").ok().as_deref() != Some("0"))
}

/// Which roof art a kind of room wears.
pub fn art_kind(t: crate::components::ModuleType) -> Option<&'static str> {
    use crate::components::ModuleType::*;
    let name = format!("{t:?}");
    // Only what shows on the outside of a hull: housings, radiators,
    // vents, intakes and hatches. Batteries, cabins, the mess, sick bay and
    // the like are inside the ship and keep plain plating -- zoomed out is
    // the shell.
    Some(match t {
        BulkCargoHold | SmallCargo => "cargo",
        OxygenScrubber => "scrubber",
        CoolingPump | ReactorCooling => "cooling",
        HeatVent => "vent",
        _ if name.contains("Reactor") && !name.contains("Cooling") => "reactor",
        _ => return None,
    })
}

/// How strongly the roof art shows over the plating under it: quiet, so it
/// reads as fittings pressed into the hull rather than a picture on it.
const ART_SHARE: f32 = 0.62;
const DECAL_SHARE: f32 = 0.7;

/// Lay a room's roof art: two-by-two pieces wherever a square of the room
/// fits, single-cell pieces in what's left. Each piece is the art tinted
/// with the room's paint, and its decal -- lights, hazard stripes -- in its
/// own colours on top.
fn lay_art(commands: &mut Commands, lay: &Lay, assets: &AssetServer, cells: &[IVec2], kind: &str, paint: Vec3) {
    let g = crate::building::GRID_SIZE;
    let room: HashSet<IVec2> = cells.iter().copied().collect();
    let mut used: HashSet<IVec2> = HashSet::new();
    let mut order: Vec<IVec2> = cells.to_vec();
    // Bottom-left first, so squares tile from one corner.
    order.sort_by_key(|c| (c.y, c.x));
    // The art's grey averages 0.62 (gen_roof_art.py MEAN): tint it to the paint.
    let tint = over_art(paint, {
        let m = Color::srgb(0.62, 0.62, 0.62).to_linear();
        Vec3::new(m.red, m.green, m.blue)
    });
    let mut put = |at: Vec2, n: u32| {
        let size = Some(Vec2::splat(g * n as f32));
        commands.spawn((
            Sprite { image: assets.load(format!("sprites/roof_art/{kind}_{n}.png")), color: tint.with_alpha(ART_SHARE * lay.alpha), custom_size: size, ..default() },
            Transform::from_translation(at.extend(ROOF_Z + 0.003)),
            RoofTile(ART_SHARE),
            ChildOf(lay.ship),
        ));
        commands.spawn((
            Sprite { image: assets.load(format!("sprites/roof_art/{kind}_{n}_decal.png")), color: Color::WHITE.with_alpha(DECAL_SHARE * lay.alpha), custom_size: size, ..default() },
            Transform::from_translation(at.extend(ROOF_Z + 0.0035)),
            RoofTile(DECAL_SHARE),
            ChildOf(lay.ship),
        ));
    };
    for c in order {
        if used.contains(&c) {
            continue;
        }
        let square = [c, c + IVec2::X, c + IVec2::Y, c + IVec2::ONE];
        if square.iter().all(|q| room.contains(q) && !used.contains(q)) {
            used.extend(square);
            put((grid_to_local(c) + grid_to_local(c + IVec2::ONE)) * 0.5, 2);
        } else {
            used.insert(c);
            put(grid_to_local(c), 1);
        }
    }
}

/// A room's roof says what it is, the way Cosmoteer's do: a reactor's ring,
/// cargo's hazard hatching, life support's vents, a crew deck's hatch. Rows
/// of the same room then read as rows. Machinery bays of mixed small parts
/// and anything else stay plain.
#[allow(clippy::too_many_arguments)]
fn lay_marks(
    commands: &mut Commands,
    lay: &Lay,
    assets: &AssetServer,
    (footprints, types, scheme): (&[(Vec<IVec2>, bool, bool)], &[crate::components::ModuleType], &Scheme),
    rooms: &HashMap<IVec2, usize>,
    raised: &HashSet<IVec2>,
    bridge: &HashSet<IVec2>,
) {
    use crate::components::ModuleType;
    let g = crate::building::GRID_SIZE;
    let kind: HashMap<IVec2, ModuleType> = footprints.iter().zip(types).flat_map(|((c, _, _), t)| c.iter().map(move |c| (*c, *t))).collect();
    // Each room's cells, and its one kind (None: a mixed bay).
    let mut by: HashMap<usize, (Vec<IVec2>, Option<ModuleType>)> = HashMap::new();
    for c in raised.iter().filter(|c| !bridge.contains(*c)) {
        let (Some(&r), Some(&t)) = (rooms.get(c), kind.get(c)) else { continue };
        let e = by.entry(r).or_insert((Vec::new(), Some(t)));
        e.0.push(*c);
        if e.1 != Some(t) {
            e.1 = None;
        }
    }
    let z = ROOF_Z + 0.005;
    let stamp = |commands: &mut Commands, at: Vec2, size: f32, path: &'static str, rgb: Vec3, share: f32| {
        commands.spawn((
            Sprite {
                image: assets.load(path),
                color: Color::srgba(rgb.x, rgb.y, rgb.z, share * lay.alpha),
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_translation(at.extend(z)),
            RoofTile(share),
            ChildOf(lay.ship),
        ));
    };
    const DARK: Vec3 = Vec3::new(0.02, 0.02, 0.03);
    let mut rooms: Vec<_> = by.into_values().collect();
    rooms.sort_by_key(|(cells, _)| cells.iter().min_by_key(|c| (c.x, c.y)).copied().map(|c| (c.x, c.y)));
    for (cells, t) in rooms {
        let Some(t) = t else { continue };
        if let Some(art) = art_kind(t).filter(|_| roof_art()) {
            let paint = scheme.room(Some(t.category())).unwrap_or(scheme.main) * 0.82;
            lay_art(commands, lay, assets, &cells, art, paint);
            continue;
        }
        let (lo, hi) = cells.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        let centre = (grid_to_local(lo) + grid_to_local(hi)) * 0.5;
        let side = ((hi - lo + IVec2::ONE).min_element() as f32 * g).min(3.0 * g);
        match t.category() {
            ModuleCategory::Power => {
                let glow = if t == ModuleType::BatteryBank { Vec3::new(0.4, 0.9, 0.85) } else { Vec3::new(1.0, 0.68, 0.28) };
                stamp(commands, centre, side * 0.78, "sprites/roof_marks/ring.png", glow, 0.6);
            }
            ModuleCategory::Storage => {
                for c in &cells {
                    stamp(commands, grid_to_local(*c), g, "sprites/roof_marks/hatch.png", DARK, 0.22);
                }
            }
            ModuleCategory::LifeSupport => {
                for c in &cells {
                    stamp(commands, grid_to_local(*c), g, "sprites/roof_marks/grille.png", DARK, 0.4);
                }
            }
            ModuleCategory::Crew => stamp(commands, centre, g * 0.85, "sprites/roof_marks/hatch_door.png", DARK, 0.35),
            _ => {}
        }
    }
}

/// On angled armour already dressed for a full roof.
#[derive(Component)]
pub struct AngleDressed;

/// Under a full roof, angled armour on your ship takes the ship's paint, a
/// shade lighter than the dark frame of skin and flat plate it stands on,
/// with its slope catching the light -- a layer of its own over the hull.
#[allow(clippy::type_complexity)]
pub fn dress_angled_armour(
    mut commands: Commands,
    ships: Query<(Entity, Option<&ShipPaint>), With<Ship>>,
    plates: Query<(Entity, &Module, &ChildOf, Option<&Children>), (Without<AngleDressed>, Without<OwnedByAiShip>, Without<DestroyedModule>)>,
    hull: Query<(&HullSegment, &ChildOf)>,
    mut quads: Query<(&mut Sprite, &Transform), (Without<Module>, Without<super::block_height::HeightShadow>)>,
) {
    if !roof_all() {
        return;
    }
    let Ok((ship, ship_paint)) = ships.single() else { return };
    let work = ship_paint.cloned().unwrap_or_default().resolve();
    let trim = work.scheme.trim;
    let mine: Vec<_> = plates.iter().filter(|(_, m, p, _)| p.parent() == ship && is_angled(m.module_type)).collect();
    if mine.is_empty() {
        return;
    }
    let (lo, hi) = hull
        .iter()
        .filter(|(_, p)| p.parent() == ship)
        .fold((IVec2::MAX, IVec2::MIN), |(lo, hi), (h, _)| (lo.min(h.grid_position), hi.max(h.grid_position)));
    let livery = player_livery();
    for (entity, module, _, children) in mine {
        let _ = (&livery, lo, hi);
        // Angled armour is trim: the bright paint along the hull's slants,
        // unless painted by hand.
        let t = match work.slots.get(&module.grid_position) {
            Some(slot) => work.scheme.colour(*slot, ROOM) / 0.82 * 0.8,
            None => trim * 0.8,
        };
        let body = Color::srgb(t.x, t.y, t.z);
        if is_wedge(module.module_type) {
            // Children not spawned yet: next frame.
            let Some(children) = children else { continue };
            for child in children.iter() {
                if let Ok((mut sprite, tf)) = quads.get_mut(child) {
                    // The rotated child is the lit slope; the rest is plate.
                    let slope = tf.rotation != Quat::IDENTITY;
                    sprite.color = if slope { body.mix(&Color::WHITE, 0.45) } else { body };
                }
            }
        } else {
            // The block's base colour: damage tinting reads it every frame.
            commands.entity(entity).try_insert(crate::components::BaseSpriteColor(body));
        }
        commands.entity(entity).try_insert(AngleDressed);
    }
}


/// When your ship's paint changes (the builder's paint mode), its guns and
/// angled armour are painted again.
pub fn repaint_on_paint_change(
    mut commands: Commands,
    changed: Query<(), (With<Ship>, Changed<ShipPaint>)>,
    painted: Query<Entity, (Or<(With<GunPainted>, With<AngleDressed>)>, Without<OwnedByAiShip>)>,
) {
    if changed.is_empty() {
        return;
    }
    for e in painted.iter() {
        commands.entity(e).try_remove::<(GunPainted, AngleDressed)>();
    }
}

/// On a gun block or turret plate already painted for a full roof.
#[derive(Component)]
pub struct GunPainted;

/// The average colour of a picture, linear, over its solid pixels.
fn art_mean(image: &Image) -> Option<Vec3> {
    let (w, h) = (image.width(), image.height());
    let (mut sum, mut n) = (Vec3::ZERO, 0.0);
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let Ok(c) = image.get_color_at(x, y) else { return None };
            let l = c.to_linear();
            if l.alpha > 0.5 {
                sum += Vec3::new(l.red, l.green, l.blue);
                n += 1.0;
            }
        }
    }
    (n > 0.0).then(|| (sum / n).max(Vec3::splat(0.02)))
}

/// A tint that turns art of average colour `mean` into `want` on average.
fn over_art(want: Vec3, mean: Vec3) -> Color {
    let w = Color::srgb(want.x, want.y, want.z).to_linear();
    let gain = (Vec3::new(w.red, w.green, w.blue) / mean).min(Vec3::splat(8.0));
    Color::LinearRgba(LinearRgba::rgb(gain.x, gain.y, gain.z))
}

/// Under a full roof your guns wear the ship's paint, in the roof's own
/// block design: a turret's gun house is laid in the same quiet plates as the
/// room roofs, its angle pieces painted to match, its deck the darker paint
/// of the hull, and its barrels and the guns on the hull in a dark gunmetal
/// of the paint. A blue-grey steel turret on a bone and orange ship read as
/// something bolted on from another ship.
#[allow(clippy::type_complexity)]
pub fn paint_guns(
    mut commands: Commands,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    ships: Query<(Entity, Option<&ShipPaint>), With<Ship>>,
    factions: Query<&crate::ai_ship::components::AiShipType, With<crate::ai_ship::components::AiShip>>,
    bodies: Query<(&crate::combat::turret_deck::TurretBody, &ChildOf)>,
    hull: Query<(&HullSegment, &ChildOf), Without<crate::combat::turret_deck::TurretPlate>>,
    mut blocks: Query<
        (Entity, &mut Sprite, &ChildOf, Option<&Module>, Has<crate::combat::turret_deck::OnTurret>, Has<crate::combat::turret_deck::TurretPlate>),
        (
            Without<GunPainted>,
            Without<crate::combat::turret_deck::PendingTurretMount>,
            Or<(With<Module>, With<crate::combat::turret_deck::TurretPlate>)>,
        ),
    >,
    mut means: Local<HashMap<AssetId<Image>, Vec3>>,
) {
    use crate::combat::gun_house::is_roof_block;
    use crate::combat::turret_deck::OUT_FRONT;
    use crate::components::ModuleType;
    if !roof_all() {
        return;
    }
    let player = ships.single().ok();
    // Each ship's paint and extent, looked up once a frame.
    let mut paints: HashMap<Entity, (Paintwork, IVec2, IVec2)> = HashMap::new();
    for (entity, mut sprite, parent, module, on_turret, plate) in &mut blocks {
        // Whose it is, and the hull cell its paint pattern reads from.
        let (owner, cell) = if let Ok((body, body_parent)) = bodies.get(parent.parent()) {
            (body_parent.parent(), Some(body.ring_cell))
        } else {
            (parent.parent(), module.map(|m| m.grid_position))
        };
        let work = if player.is_some_and(|(p, _)| p == owner) {
            player.and_then(|(_, p)| p).cloned().unwrap_or_default().resolve()
        } else if let Ok(faction) = factions.get(owner) {
            faction_paint(*faction)
        } else {
            continue;
        };
        let (work, lo, hi) = paints.entry(owner).or_insert_with(|| {
            let (lo, hi) = hull.iter().filter(|(_, p)| p.parent() == owner).fold((IVec2::MAX, IVec2::MIN), |(lo, hi), (h, _)| (lo.min(h.grid_position), hi.max(h.grid_position)));
            (work, lo, hi)
        });
        let (scheme, livery, lo, hi) = (work.scheme, work.livery, *lo, *hi);
        let kind = module.map(|m| m.module_type);
        let gun = kind.is_some_and(|t| matches!(t.category(), ModuleCategory::Weapons | ModuleCategory::Guns) || matches!(t, ModuleType::LaserEmitter));
        enum Coat {
            House,
            HouseArt,
            Deck,
            Metal,
        }
        let coat = if plate {
            Coat::Deck
        } else if on_turret && kind.is_some_and(|t| t == ModuleType::ArmorPlate) {
            Coat::House
        } else if on_turret && kind.is_some_and(is_roof_block) {
            Coat::HouseArt
        } else if kind.is_some_and(|t| OUT_FRONT.contains(&t)) || (!on_turret && gun) {
            Coat::Metal
        } else {
            // A part under the gun house, or not a gun: left as it is.
            commands.entity(entity).try_insert(GunPainted);
            continue;
        };
        if sprite.color.alpha() == 0.0 {
            commands.entity(entity).try_insert(GunPainted);
            continue;
        }
        let band = cell.is_some_and(|c| accent_at(&livery, c, lo, hi));
        // A gun on a tile painted by hand wears that paint.
        let slot = cell.and_then(|c| work.slots.get(&c).copied()).unwrap_or(paint_slot(ROOM, band));
        let paint = scheme.colour(slot, ROOM) / 0.82;
        let colour = match coat {
            Coat::House => {
                // The gun house roof in the room roofs' own plates.
                let q = module.map_or(IVec2::ZERO, |m| m.grid_position).rem_euclid(IVec2::splat(2));
                sprite.image = assets.load(roof_path_quiet(HullMaterial::Steel));
                sprite.rect = Some(Rect::new(q.x as f32 * 128.0, (1 - q.y) as f32 * 128.0, q.x as f32 * 128.0 + 128.0, (2 - q.y) as f32 * 128.0));
                over_plate(paint * 0.92, HullMaterial::Steel)
            }
            Coat::HouseArt | Coat::Deck | Coat::Metal => {
                let id = sprite.image.id();
                let mean = match means.get(&id) {
                    Some(m) => *m,
                    None => {
                        // Not loaded yet: next frame.
                        let Some(m) = images.get(id).and_then(art_mean) else { continue };
                        means.insert(id, m);
                        m
                    }
                };
                let want = match coat {
                    Coat::HouseArt => paint * 0.86,
                    Coat::Deck => scheme.colour(Slot::Main, SKIN),
                    _ => scheme.main * 0.42,
                };
                over_art(want, mean)
            }
        };
        sprite.color = colour;
        commands.entity(entity).try_insert((crate::components::BaseSpriteColor(colour), GunPainted));
    }
}

/// The full painted roof, on every ship. AFTERLIGHT_ROOF_ALL=0 goes back to
/// the old plain decking over your corridors alone.
pub fn roof_all() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("AFTERLIGHT_ROOF_ALL").ok().as_deref() != Some("0"))
}

/// A weapon lifted over the roof, and the height it came from.
#[derive(Component)]
pub struct RaisedOverRoof(f32);

/// Above the roof.
const RAISED_Z: f32 = ROOF_Z + 0.05;

/// While the roof shows, the ship's weapons ride above it; zoomed back in
/// they return to their own height (under the crew who man them).
pub fn raise_weapons_over_roof(
    mut commands: Commands,
    camera: Res<CameraState>,
    inside: Res<SeeInside>,
    ships: Query<(Entity, Has<PressureOverlayVisible>), With<Ship>>,
    ais: Query<(), With<crate::ai_ship::components::AiShip>>,
    mut weapons: Query<(Entity, &Module, &ChildOf, &mut Transform, Option<&RaisedOverRoof>)>,
) {
    let mine = ships.single().ok();
    let own_up = mine.is_some_and(|(_, air)| own_roof_alpha(camera.zoom, &inside, air) > 0.0);
    let ai_up = roof_all() && roof_alpha(camera.zoom) > 0.0;
    for (entity, module, parent, mut tf, raised) in weapons.iter_mut() {
        let owner = parent.parent();
        let up = if mine.is_some_and(|(s, _)| s == owner) {
            own_up
        } else if ais.contains(owner) {
            ai_up
        } else {
            continue;
        };
        let lifted = if roof_all() {
            (stands_above(module) && !is_bridge(module.module_type)) || is_angled(module.module_type)
        } else {
            stands_above(module)
        };
        if !lifted {
            continue;
        }
        match (up, raised) {
            (true, None) => {
                commands.entity(entity).try_insert(RaisedOverRoof(tf.translation.z));
                tf.translation.z = RAISED_Z;
            }
            (false, Some(RaisedOverRoof(z))) => {
                tf.translation.z = *z;
                commands.entity(entity).try_remove::<RaisedOverRoof>();
            }
            _ => {}
        }
    }
}

/// Off the flight screen weapons go back to their own height.
pub fn lower_weapons(mut commands: Commands, mut raised: Query<(Entity, &mut Transform, &RaisedOverRoof)>) {
    for (entity, mut tf, RaisedOverRoof(z)) in raised.iter_mut() {
        tf.translation.z = *z;
        commands.entity(entity).try_remove::<RaisedOverRoof>();
    }
}

/// On an AI ship whose full roof is laid, with what it was laid for.
#[derive(Component)]
pub struct RoofLaid(u64);

/// Ships whose roof is laid in a frame at most: a fleet coming into view
/// spreads its roofs over a few frames.
const ROOFS_PER_FRAME: usize = 2;

/// Every AI ship wears the full roof too, in its faction's paint and
/// pattern: laid when the camera pulls out far enough to show it, laid
/// again when the ship changes under it, taken away when it zooms back in.
#[allow(clippy::type_complexity)]
pub fn update_ai_ship_roofs(
    mut commands: Commands,
    assets: Res<AssetServer>,
    camera: Res<CameraState>,
    roots: Query<
        (Entity, Option<&RoofLaid>, &Scars, &GlobalTransform, &crate::ai_ship::components::AiShipType, Has<crate::ai_ship::components::AiShipWreck>),
        With<crate::ai_ship::components::AiShip>,
    >,
    hull: Query<(&HullSegment, &OwnedByAiShip), (Without<HullDestroyed>, Without<crate::combat::turret_deck::TurretPlate>)>,
    modules: Query<
        (&Module, &OwnedByAiShip),
        (Without<DestroyedModule>, Without<crate::combat::turret_deck::OnTurret>, Without<crate::combat::turret_deck::PendingTurretMount>),
    >,
    tiles: Query<(Entity, &ChildOf), With<RoofTile>>,
    mut turn_of: Local<usize>,
) {
    if !roof_all() {
        return;
    }
    let alpha = roof_alpha(camera.zoom);
    let despawn_on = |commands: &mut Commands, root: Entity| {
        for (tile, parent) in tiles.iter() {
            if parent.parent() == root {
                commands.entity(tile).try_despawn();
            }
        }
    };
    if alpha <= 0.0 {
        for (root, laid, ..) in roots.iter() {
            if laid.is_some() {
                despawn_on(&mut commands, root);
                commands.entity(root).try_remove::<RoofLaid>();
            }
        }
        return;
    }
    let mut hull_by: HashMap<Entity, Vec<&HullSegment>> = HashMap::new();
    for (h, owner) in hull.iter() {
        hull_by.entry(owner.root).or_default().push(h);
    }
    let mut mods_by: HashMap<Entity, Vec<&Module>> = HashMap::new();
    for (m, owner) in modules.iter() {
        mods_by.entry(owner.root).or_default().push(m);
    }
    // The ships whose roof is out of date. Taken in turns, a few a frame: in
    // a big fight the same few ships being hit every frame used to take every
    // turn, and the rest kept the roof they had before they were shot up.
    let mut stale = Vec::new();
    for (root, laid, scars, gt, faction, wrecked) in roots.iter() {
        let hulls = hull_by.get(&root).map(|v| v.as_slice()).unwrap_or(&[]);
        let mods = mods_by.get(&root).map(|v| v.as_slice()).unwrap_or(&[]);
        if hulls.is_empty() {
            continue;
        }
        let key = (hulls.len() as u64) << 40 ^ (mods.len() as u64) << 20 ^ scars.0.len() as u64 ^ (wrecked as u64) << 63;
        if laid.is_some_and(|l| l.0 == key) {
            continue;
        }
        stale.push((root, key, scars, gt, faction, wrecked, hulls, mods));
    }
    if stale.is_empty() {
        return;
    }
    stale.sort_by_key(|s| s.0);
    let first = *turn_of % stale.len();
    *turn_of = turn_of.wrapping_add(ROOFS_PER_FRAME);
    for i in 0..ROOFS_PER_FRAME.min(stale.len()) {
        let (root, key, scars, gt, faction, wrecked, hulls, mods) = stale[(first + i) % stale.len()];
        despawn_on(&mut commands, root);
        let turn = gt.to_scale_rotation_translation().1;
        lay_roof(&mut commands, &assets, root, (camera.zoom, turn, alpha), hulls, mods, scars, &faction_paint(*faction), wrecked);
        commands.entity(root).try_insert(RoofLaid(key));
    }
}

/// On a wrecked block hidden under the roof, with the visibility it had.
#[derive(Component)]
pub struct WreckageHidden(Visibility);

/// Under the roof, what's been shot away is gone. A block shot away leaves a
/// rebuild ghost where it stood (your ship's crew rebuild from it), and a
/// part takes half a second to go -- zoomed out, over the roof, those read as
/// whole plating, so a ship that had been hit looked untouched. While a
/// ship's roof shows they're hidden: the hit is a hole, open to space where
/// the plating went, burnt floor where it held. Close in they're back, ghosts
/// and all.
#[allow(clippy::type_complexity)]
pub fn hide_wreckage_under_roof(
    mut commands: Commands,
    camera: Res<CameraState>,
    inside: Res<SeeInside>,
    view: PlayerRoofView,
    player: Query<(Entity, Has<PressureOverlayVisible>), With<Ship>>,
    mut wreckage: Query<
        (Entity, &mut Visibility, &ChildOf, Has<crate::ai_ship::components::OwnedByAiShip>, Option<&WreckageHidden>, Has<crate::ship::rebuild::RebuildGhostSprite>),
        Or<(With<DestroyedModule>, With<HullDestroyed>, With<WreckageHidden>, With<crate::ship::rebuild::RebuildGhostSprite>)>,
    >,
    alive: Query<(), (Without<DestroyedModule>, Without<HullDestroyed>)>,
) {
    let own = player.single().ok();
    let own_alpha = own.map_or(0.0, |(_, air)| view.alpha(own_roof_alpha(camera.zoom, &inside, air)));
    let ai_alpha = if roof_all() { roof_alpha(camera.zoom) } else { 0.0 };
    for (block, mut visibility, parent, ai, hidden, ghost) in &mut wreckage {
        let mine = !ai && own.is_some_and(|(ship, _)| parent.parent() == ship);
        let alpha = if mine { own_alpha } else { ai_alpha };
        // Rebuilt (no longer wreckage): back as it was.
        let wrecked = ghost || alive.get(block).is_err();
        let hide = wrecked && alpha >= 0.5;
        match (hide, hidden) {
            (true, None) => {
                commands.entity(block).try_insert(WreckageHidden(*visibility));
                *visibility = Visibility::Hidden;
            }
            (false, Some(was)) => {
                *visibility = was.0;
                commands.entity(block).try_remove::<WreckageHidden>();
            }
            _ => {}
        }
    }
}

/// Keep every ship's roof sized and lit: each edge's width held in screen
/// pixels for the zoom, its shade following the light as the ship turns,
/// and every tile faded with the zoom.
#[allow(clippy::type_complexity)]
pub fn fit_roofs(
    camera: Res<CameraState>,
    inside: Res<SeeInside>,
    view: PlayerRoofView,
    owners: Query<(&GlobalTransform, Has<Ship>, Has<PressureOverlayVisible>)>,
    mut tiles: Query<(&mut Sprite, &RoofTile, Option<&Edge>, &mut Transform, &ChildOf)>,
    mut seen: Local<HashMap<Entity, Option<(f32, Quat)>>>,
) {
    seen.clear();
    for (mut sprite, tile, edge, mut tf, parent) in tiles.iter_mut() {
        let owner = parent.parent();
        let Some((alpha, turn)) = *seen.entry(owner).or_insert_with(|| {
            owners.get(owner).ok().map(|(gt, mine, air)| {
                let alpha = if mine { view.alpha(own_roof_alpha(camera.zoom, &inside, air)) } else { roof_alpha(camera.zoom) };
                (alpha, gt.to_scale_rotation_translation().1)
            })
        }) else {
            continue;
        };
        if let Some(edge) = edge {
            edge.fit(&mut sprite, &mut tf, camera.zoom, turn, alpha);
            continue;
        }
        let want = alpha * tile.0;
        if (sprite.color.alpha() - want).abs() > 0.001 {
            sprite.color.set_alpha(want);
        }
    }
}

/// The roof plate texture for a hull material.
/// A roof or frame tile laid over an AI ship (AFTERLIGHT_SHOW_SHIPS, filming the
/// prologue): the same zoomed-out look the player's ship has. Kept apart from
/// RoofTile, which update_ship_roof clears whenever the player's ship changes.
#[derive(Component)]
pub struct AiRoofTile(pub f32);

/// On an AI ship whose roof is laid, with the block counts it was laid for.
#[derive(Component)]
pub struct AiRoofLaid(usize, usize, usize);

/// The floor cells a ship has lost a module from. Zoomed out, a floor with
/// nothing on it gets a clean deck plate, so a module shot off used to be
/// replaced by fresh plating and the ship looked untouched: 1,253 blocks down
/// to 536 and the same picture. These get a burn instead.
///
/// `.1` is the hull cells shot away, for the far picture: a hole in a dark ship
/// against dark space doesn't show from 20 km, so its torn edges glow.
#[derive(Component, Default, Debug)]
pub struct Scars(pub HashSet<IVec2>, pub HashSet<IVec2>);

/// Colour of a scar: burnt metal.
const SCAR: (f32, f32, f32) = (0.055, 0.035, 0.03);
const SCAR_SHADE: f32 = 0.95;

/// Keep each ship's scars: a module lost marks its cells, a module rebuilt
/// clears them.
#[allow(clippy::type_complexity)]
pub fn note_scars(
    lost: Query<(&Module, &ChildOf), Added<crate::components::DestroyedModule>>,
    built: Query<(&Module, &ChildOf), Added<Module>>,
    hull_lost: Query<(&HullSegment, &ChildOf), Added<HullDestroyed>>,
    hull_built: Query<(&HullSegment, &ChildOf), Added<HullSegment>>,
    mut ships: Query<&mut Scars>,
) {
    let cells = |m: &Module| ShipGrid::cells_for(m.grid_position, m.size, m.rotation, footprints::footprint_override(m.module_type));
    for (module, parent) in built.iter() {
        if let Ok(mut scars) = ships.get_mut(parent.parent()) {
            if !scars.0.is_empty() {
                for cell in cells(module) {
                    scars.0.remove(&cell);
                }
            }
        }
    }
    for (module, parent) in lost.iter() {
        if let Ok(mut scars) = ships.get_mut(parent.parent()) {
            scars.0.extend(cells(module));
        }
    }
    for (hull, parent) in hull_built.iter() {
        if let Ok(mut scars) = ships.get_mut(parent.parent()) {
            scars.1.remove(&hull.grid_position);
        }
    }
    for (hull, parent) in hull_lost.iter() {
        if let Ok(mut scars) = ships.get_mut(parent.parent()) {
            scars.1.insert(hull.grid_position);
        }
    }
}

/// Where the far picture glows, with how bright, 0..1: about half of what's
/// burnt out, and the edge of every hole. All of a wound would be a solid orange
/// blob, and a hole's middle is empty space.
pub fn embers(scars: &Scars, standing: &HashSet<IVec2>) -> Vec<(IVec2, f32)> {
    const SIDES: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];
    let speckled = |c: &IVec2| (c.x.wrapping_mul(73) ^ c.y.wrapping_mul(179)).rem_euclid(2) == 0;
    let burns = scars.0.iter().filter(|c| speckled(c) && standing.contains(*c)).map(|c| (*c, 1.0));
    let edges = scars.1.iter().filter(|c| !standing.contains(*c) && SIDES.iter().any(|s| standing.contains(&(**c + *s)))).map(|c| (*c, 0.7));
    burns.chain(edges).collect()
}

/// The floor cells showing a scar, and the roof with them taken out.
pub fn scarred(roof: HashSet<IVec2>, scars: &HashSet<IVec2>) -> (HashSet<IVec2>, Vec<IVec2>) {
    let burnt: Vec<IVec2> = roof.iter().filter(|c| scars.contains(*c)).copied().collect();
    let clean = roof.into_iter().filter(|c| !scars.contains(c)).collect();
    (clean, burnt)
}

/// Lay the zoomed-out roof on every AI ship, and fade it with the zoom.
#[allow(clippy::type_complexity)]
pub fn update_ai_roofs(
    mut commands: Commands,
    assets: Res<AssetServer>,
    camera: Res<CameraState>,
    roots: Query<(Entity, Option<&AiRoofLaid>, &Scars, Has<crate::ai_ship::components::AiShipWreck>), With<crate::ai_ship::components::AiShip>>,
    hull: Query<(&HullSegment, &OwnedByAiShip), Without<HullDestroyed>>,
    modules: Query<(&Module, &OwnedByAiShip), Without<DestroyedModule>>,
    mut tiles: Query<(Entity, &mut Sprite, &AiRoofTile, &ChildOf)>,
) {
    let alpha = roof_alpha(camera.zoom);
    for (_, mut sprite, tile, _) in tiles.iter_mut() {
        let want = alpha * tile.0;
        if (sprite.color.alpha() - want).abs() > 0.001 {
            sprite.color.set_alpha(want);
        }
    }
    let mut hull_by: HashMap<Entity, Vec<&HullSegment>> = HashMap::new();
    for (h, owner) in hull.iter() {
        hull_by.entry(owner.root).or_default().push(h);
    }
    let mut mods_by: HashMap<Entity, Vec<&Module>> = HashMap::new();
    for (m, owner) in modules.iter() {
        mods_by.entry(owner.root).or_default().push(m);
    }
    for (root, laid, scars, wrecked) in roots.iter() {
        let hulls = hull_by.get(&root).map(|v| v.as_slice()).unwrap_or(&[]);
        let mods = mods_by.get(&root).map(|v| v.as_slice()).unwrap_or(&[]);
        if hulls.is_empty() || laid.is_some_and(|l| l.0 == hulls.len() && l.1 == mods.len() && l.2 == scars.0.len()) {
            continue;
        }
        for (tile, _, _, parent) in tiles.iter() {
            if parent.parent() == root {
                commands.entity(tile).try_despawn();
            }
        }
        let cells: Vec<(IVec2, HullLayer)> = hulls.iter().map(|s| (s.grid_position, s.hull_layer)).collect();
        let footprints: Vec<(Vec<IVec2>, bool, bool)> = mods
            .iter()
            .map(|m| {
                let cells = ShipGrid::cells_for(m.grid_position, m.size, m.rotation, footprints::footprint_override(m.module_type)).to_vec();
                (cells, stands_above(m), is_armour(m.module_type))
            })
            .collect();
        let parts: Vec<(Vec<IVec2>, bool)> = footprints.iter().map(|(c, above, _)| (c.clone(), *above)).collect();
        let mut counts: Vec<(HullMaterial, usize)> = Vec::new();
        for s in hulls {
            match counts.iter_mut().find(|(k, _)| *k == s.material) {
                Some((_, n)) => *n += 1,
                None => counts.push((s.material, 1)),
            }
        }
        let material = counts.into_iter().max_by_key(|(_, n)| *n).map_or(HullMaterial::Steel, |(m, _)| m);
        let image = assets.load(roof_path(material));
        let (roof, burnt) = scarred(roof_cells(&cells, &parts), &scars.0);
        for cell in burnt {
            commands.spawn((
                Sprite {
                    color: Color::srgba(SCAR.0, SCAR.1, SCAR.2, SCAR_SHADE * alpha),
                    custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                    ..default()
                },
                Transform::from_translation(grid_to_local(cell).extend(ROOF_Z)),
                AiRoofTile(SCAR_SHADE),
                ChildOf(root),
            ));
        }
        for cell in roof {
            let plate = cell.div_euclid(IVec2::splat(2));
            let q = cell.rem_euclid(IVec2::splat(2));
            let rect = Rect::new(q.x as f32 * 128.0, (1 - q.y) as f32 * 128.0, q.x as f32 * 128.0 + 128.0, (2 - q.y) as f32 * 128.0);
            let tone = plate_tone(plate) * DECK_LIGHTEN;
            let color = Color::srgba(tone, tone, tone, alpha);
            commands.spawn((
                Sprite {
                    image: image.clone(),
                    rect: Some(rect),
                    color: if wrecked { crate::ai_ship::components::wreck_shade(color) } else { color },
                    custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                    ..default()
                },
                Transform::from_translation(grid_to_local(cell).extend(ROOF_Z)),
                AiRoofTile(1.0),
                ChildOf(root),
            ));
        }
        for cell in frame_cells(&cells, &footprints) {
            commands.spawn((
                Sprite {
                    color: Color::srgba(0.0, 0.0, 0.0, FRAME_SHADE * alpha),
                    custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                    ..default()
                },
                Transform::from_translation(grid_to_local(cell).extend(FRAME_Z)),
                AiRoofTile(FRAME_SHADE),
                ChildOf(root),
            ));
        }
        commands.entity(root).try_insert(AiRoofLaid(hulls.len(), mods.len(), scars.0.len()));
    }
}

/// The plates printed quieter, for paint.
fn roof_path_quiet(material: HullMaterial) -> &'static str {
    match material {
        HullMaterial::Steel => "sprites/hull/roof_steel_quiet.png",
        HullMaterial::Titanium => "sprites/hull/roof_titanium_quiet.png",
        HullMaterial::Composite => "sprites/hull/roof_composite_quiet.png",
        HullMaterial::AbyssalAlloy => "sprites/hull/roof_abyssal_quiet.png",
    }
}

/// Half a quiet plate cut on the diagonal: the roof's angle pieces.
fn roof_path_tri(material: HullMaterial) -> &'static str {
    match material {
        HullMaterial::Steel => "sprites/hull/roof_steel_tri.png",
        HullMaterial::Titanium => "sprites/hull/roof_titanium_tri.png",
        HullMaterial::Composite => "sprites/hull/roof_composite_tri.png",
        HullMaterial::AbyssalAlloy => "sprites/hull/roof_abyssal_tri.png",
    }
}

fn roof_path(material: HullMaterial) -> &'static str {
    match material {
        HullMaterial::Steel => "sprites/hull/roof_steel.png",
        HullMaterial::Titanium => "sprites/hull/roof_titanium.png",
        HullMaterial::Composite => "sprites/hull/roof_composite.png",
        HullMaterial::AbyssalAlloy => "sprites/hull/roof_abyssal.png",
    }
}

/// Each plate a shade lighter or darker than the next (0.93 to 1.05), the
/// same every time for the same plate.
pub fn plate_tone(plate: IVec2) -> f32 {
    let h = (plate.x as u32).wrapping_mul(0x9E37_79B1) ^ (plate.y as u32).wrapping_mul(0x85EB_CA77);
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    0.93 + 0.12 * ((h >> 8) & 0xFF) as f32 / 255.0
}

/// Off the flight screen (docked, building, paused) the roof comes off;
/// update_ship_roof lays it again on return.
pub fn clear_ship_roof(mut commands: Commands, tiles: Query<Entity, With<RoofTile>>, laid: Query<Entity, With<RoofLaid>>) {
    for tile in tiles.iter() {
        commands.entity(tile).try_despawn();
    }
    for root in laid.iter() {
        commands.entity(root).try_remove::<RoofLaid>();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_wound_glows_at_its_edge_and_not_all_over() {
        let standing: HashSet<IVec2> = (0..5).map(|x| IVec2::new(x, 0)).collect();
        // A long hole, one row up from the hull and two rows up.
        let holes: HashSet<IVec2> = (0..5).flat_map(|x| [IVec2::new(x, 1), IVec2::new(x, 2)]).collect();
        let glow = embers(&Scars(HashSet::new(), holes), &standing);
        assert_eq!(glow.len(), 5, "only the row touching what's left: {glow:?}");
        assert!(glow.iter().all(|(c, _)| c.y == 1));
        // A burn out on a floor that's still there speckles.
        let burns: HashSet<IVec2> = standing.clone();
        let speckle = embers(&Scars(burns, HashSet::new()), &standing);
        assert!(!speckle.is_empty() && speckle.len() < 5, "{speckle:?}");
    }

    #[test]
    fn a_lost_module_leaves_a_burn_and_not_a_clean_plate() {
        let roof: HashSet<IVec2> = [IVec2::new(0, 0), IVec2::new(1, 0)].into_iter().collect();
        let scars: HashSet<IVec2> = [IVec2::new(1, 0), IVec2::new(9, 9)].into_iter().collect();
        let (clean, burnt) = scarred(roof, &scars);
        assert!(clean.contains(&IVec2::new(0, 0)) && !clean.contains(&IVec2::new(1, 0)));
        assert_eq!(burnt, vec![IVec2::new(1, 0)], "only floor that's still there burns");
    }

    use super::*;

    #[test]
    fn the_roof_fades_in_as_you_pull_back() {
        assert_eq!(roof_alpha(1.8), 0.0, "the default view is the cutaway");
        assert_eq!(roof_alpha(ROOF_SOLID + 1.0), 1.0);
        assert!(roof_alpha(4.5) > 0.0 && roof_alpha(4.5) < 1.0);
    }

    #[test]
    fn neighbouring_plates_differ_and_each_stays_the_same() {
        let tones: Vec<f32> = (0..8).map(|x| plate_tone(IVec2::new(x, 3))).collect();
        assert!(tones.iter().all(|t| (0.93..=1.05).contains(t)));
        assert!(tones.windows(2).any(|w| (w[0] - w[1]).abs() > 0.01), "plates all the same shade");
        assert_eq!(plate_tone(IVec2::new(-4, 9)), plate_tone(IVec2::new(-4, 9)));
    }

    /// Decks are plated over; parts never are -- each shows its own top.
    #[test]
    fn bare_decks_are_plated_and_every_part_shows() {
        // A 5x3 box: outer skin around a 3x1 interior.
        let mut hull = Vec::new();
        for x in 0..5 {
            for y in 0..3 {
                let edge = x == 0 || x == 4 || y == 0 || y == 2;
                hull.push((IVec2::new(x, y), if edge { HullLayer::Outer } else { HullLayer::Hallway }));
            }
        }
        let reactor = (vec![IVec2::new(2, 1)], false);
        let gun = (vec![IVec2::new(5, 1)], true);
        let roof = roof_cells(&hull, &[reactor, gun]);
        assert!(roof.contains(&IVec2::new(1, 1)) && roof.contains(&IVec2::new(3, 1)), "the corridor");
        assert!(!roof.contains(&IVec2::new(2, 1)), "the reactor shows its own top");
        assert!(!roof.contains(&IVec2::new(5, 1)), "the gun on the bow");
        assert!(!roof.contains(&IVec2::new(0, 1)), "the skin is the frame, not deck");
    }

    /// The frame is the bare outer skin and the armour plates, not the parts
    /// mounted on the skin.
    #[test]
    fn the_skin_and_armour_make_the_dark_frame() {
        let hull = vec![
            (IVec2::new(0, 0), HullLayer::Outer),
            (IVec2::new(1, 0), HullLayer::Outer),
            (IVec2::new(2, 0), HullLayer::Hallway),
        ];
        let engine = (vec![IVec2::new(1, 0)], false, false);
        let plate = (vec![IVec2::new(3, 0)], false, true);
        let frame = frame_cells(&hull, &[engine, plate]);
        assert!(frame.contains(&IVec2::new(0, 0)), "bare skin");
        assert!(!frame.contains(&IVec2::new(1, 0)), "the engine on the skin shows");
        assert!(frame.contains(&IVec2::new(3, 0)), "the armour plate");
        assert!(!frame.contains(&IVec2::new(2, 0)), "the deck");
    }

    #[test]
    fn a_weapon_inside_the_hull_stays_in_sight() {
        let mut hull = Vec::new();
        for x in 0..5 {
            for y in 0..5 {
                let edge = x == 0 || x == 4 || y == 0 || y == 4;
                hull.push((IVec2::new(x, y), if edge { HullLayer::Outer } else { HullLayer::Hallway }));
            }
        }
        let turret = (vec![IVec2::new(2, 2)], true);
        let quarters = (vec![IVec2::new(1, 1)], false);
        let roof = roof_cells(&hull, &[turret, quarters]);
        assert!(!roof.contains(&IVec2::new(2, 2)), "the turret in the middle");
        assert!(!roof.contains(&IVec2::new(1, 1)), "the quarters beside it");
        assert!(roof.contains(&IVec2::new(3, 3)), "the bare deck beside both");
    }
}
