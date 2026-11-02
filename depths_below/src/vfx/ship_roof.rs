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

use crate::ai_ship::components::OwnedByAiShip;
use crate::building::{footprints, grid_to_local, ShipGrid};
use crate::camera::CameraState;
use crate::components::{DestroyedModule, HullDestroyed, HullLayer, HullMaterial, HullSegment, Module, ModuleCategory, Ship};

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
        ArmorPlate | CornerArmorPlate | StaggeredArmorPlate | AblativeArmor | HullReinforcePlate
    )
}

/// How solid the roof is at a zoom.
pub fn roof_alpha(zoom: f32) -> f32 {
    ((zoom - ROOF_FROM) / (ROOF_SOLID - ROOF_FROM)).clamp(0.0, 1.0)
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

/// Lay the roof when it comes into view or the ship changes under it, and take
/// it away when you zoom back in.
#[allow(clippy::too_many_arguments)]
pub fn update_ship_roof(
    mut commands: Commands,
    assets: Res<AssetServer>,
    camera: Res<CameraState>,
    ships: Query<(Entity, &Scars), With<Ship>>,
    hull: Query<(&HullSegment, &ChildOf), (Without<HullDestroyed>, Without<OwnedByAiShip>)>,
    modules: Query<(&Module, &ChildOf), (Without<DestroyedModule>, Without<OwnedByAiShip>)>,
    mut tiles: Query<(Entity, &mut Sprite, &RoofTile)>,
    mut last: Local<Option<(Entity, usize, usize, usize)>>,
) {
    let alpha = roof_alpha(camera.zoom);
    let Ok((ship, scars)) = ships.single() else { return };
    if alpha <= 0.0 {
        if last.is_some() {
            for (tile, _, _) in tiles.iter() {
                commands.entity(tile).try_despawn();
            }
            *last = None;
        }
        return;
    }
    let mine_hull: Vec<(&HullSegment, &ChildOf)> = hull.iter().filter(|(_, p)| p.parent() == ship).collect();
    let mine_modules: Vec<&Module> = modules.iter().filter(|(_, p)| p.parent() == ship).map(|(m, _)| m).collect();
    let key = (ship, mine_hull.len(), mine_modules.len(), scars.0.len());
    // Tiles cleared from outside (clear_ship_roof) are laid again too.
    if *last == Some(key) && !tiles.is_empty() {
        for (_, mut sprite, tile) in tiles.iter_mut() {
            let want = alpha * tile.0;
            if (sprite.color.alpha() - want).abs() > 0.001 {
                sprite.color.set_alpha(want);
            }
        }
        return;
    }
    *last = Some(key);
    for (tile, _, _) in tiles.iter() {
        commands.entity(tile).try_despawn();
    }

    let cells: Vec<(IVec2, HullLayer)> = mine_hull.iter().map(|(s, _)| (s.grid_position, s.hull_layer)).collect();
    let footprints: Vec<(Vec<IVec2>, bool, bool)> = mine_modules
        .iter()
        .map(|m| {
            let cells = ShipGrid::cells_for(m.grid_position, m.size, m.rotation, footprints::footprint_override(m.module_type)).to_vec();
            (cells, stands_above(m), is_armour(m.module_type))
        })
        .collect();
    let parts: Vec<(Vec<IVec2>, bool)> = footprints.iter().map(|(c, above, _)| (c.clone(), *above)).collect();
    let (roof, burnt) = scarred(roof_cells(&cells, &parts), &scars.0);
    let frame = frame_cells(&cells, &footprints);
    // The ship's skin, cell by cell; the roof is the commonest of it.
    let skin: HashMap<IVec2, HullMaterial> = mine_hull.iter().map(|(s, _)| (s.grid_position, s.material)).collect();
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
    // read as no surface at all.
    let image = assets.load(roof_path(material));
    for cell in roof {
        let plate = cell.div_euclid(IVec2::splat(2));
        let q = cell.rem_euclid(IVec2::splat(2));
        // Image rows run top-down; grid y runs up.
        let rect = Rect::new(q.x as f32 * 128.0, (1 - q.y) as f32 * 128.0, q.x as f32 * 128.0 + 128.0, (2 - q.y) as f32 * 128.0);
        let tone = plate_tone(plate) * DECK_LIGHTEN;
        commands.spawn((
            Sprite {
                image: image.clone(),
                rect: Some(rect),
                color: Color::srgba(tone, tone, tone, alpha),
                custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                ..default()
            },
            Transform::from_translation(grid_to_local(cell).extend(ROOF_Z)),
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
    for cell in frame {
        commands.spawn((
            Sprite {
                color: Color::srgba(0.0, 0.0, 0.0, FRAME_SHADE * alpha),
                custom_size: Some(Vec2::splat(crate::building::GRID_SIZE)),
                ..default()
            },
            Transform::from_translation(grid_to_local(cell).extend(FRAME_Z)),
            RoofTile(FRAME_SHADE),
            ChildOf(ship),
        ));
    }
}

/// Weapons stand above the hull: zoomed out they stay in sight over the
/// roof wherever they're mounted, barrels and all.
fn stands_above(module: &Module) -> bool {
    // Towers and masts too: whatever casts a height shadow stands taller
    // than the plating (vfx::block_height).
    module.module_type.category() == ModuleCategory::Weapons
        || super::block_height::block_height(module.module_type).is_some()
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
    ships: Query<Entity, With<Ship>>,
    mut weapons: Query<(Entity, &Module, &ChildOf, &mut Transform, Option<&RaisedOverRoof>), Without<OwnedByAiShip>>,
) {
    let up = roof_alpha(camera.zoom) > 0.0;
    let Ok(ship) = ships.single() else { return };
    for (entity, module, parent, mut tf, raised) in weapons.iter_mut() {
        if parent.parent() != ship || !stands_above(module) {
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

/// The roof plate texture for a hull material.
/// A roof or frame tile laid over an AI ship (DEPTHS_SHOW_SHIPS, filming the
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
    roots: Query<(Entity, Option<&AiRoofLaid>, &Scars), With<crate::ai_ship::components::AiShip>>,
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
    for (root, laid, scars) in roots.iter() {
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
            commands.spawn((
                Sprite {
                    image: image.clone(),
                    rect: Some(rect),
                    color: Color::srgba(tone, tone, tone, alpha),
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
pub fn clear_ship_roof(mut commands: Commands, tiles: Query<Entity, With<RoofTile>>) {
    for tile in tiles.iter() {
        commands.entity(tile).try_despawn();
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
