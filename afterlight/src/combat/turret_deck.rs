//! Turrets: a small deck of their own that turns over the hull.
//!
//! A Turret Ring (1×1, 3×3 or 5×5) sits on the hull deck. Above it stands a
//! TurretBody: an entity of its own, a child of the ship, that rotates. The
//! turret's floor is plating (hull segments within the ring's reach) and the
//! guns and parts on it are ordinary modules parented to the body, laid out
//! on the turret's own little grid with (0, 0) at the ring's centre.
//!
//! Hanging the deck off its own entity keeps it out of everything that reads
//! the hull deck -- rooms, walk maps, power, break-off -- which only ever see
//! the ship's direct children. Bevy's transforms turn it with the ship.
//!
//! How fast a turret turns is its Traverse Motors (on the hull deck, touching
//! the ring) against everything standing on it (combat::gun_math).

use bevy::prelude::*;

use crate::components::{
    DestroyedModule, HullLayer, HullMaterial, HullSegment, Module, ModuleType, Ship,
};

/// Where the player is aiming this frame, in world space, in the order the
/// guns take it: a scripted run's AFTERLIGHT_AIM_AT, a locked block, (each
/// turret's own engaged block -- see `traverse_turrets`), the cursor, and
/// the selected target only with no cursor to follow. Turrets follow the
/// mouse: a selected ship used to win over it, so the main battery stayed
/// on whatever `\` had picked while the player aimed somewhere else.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PlayerAimPoint {
    pub first: Option<Vec2>,
    pub then: Option<Vec2>,
}

impl PlayerAimPoint {
    /// A turret's aim, given the block its guns have engaged on their own.
    pub fn for_turret(&self, engaged: Option<Vec2>) -> Option<Vec2> {
        self.first.or(engaged).or(self.then)
    }
}

pub fn update_player_aim_point(
    mut aim: ResMut<PlayerAimPoint>,
    lock: Res<crate::combat::targeting::AimLock>,
    selection: Res<crate::combat::targeting::TargetSelection>,
    targets: Query<&GlobalTransform>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    ship: Query<&GlobalTransform, With<Ship>>,
) {
    let cursor = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .and_then(|c| camera.single().ok().and_then(|(cam, gt)| cam.viewport_to_world_2d(gt, c).ok()));
    let scripted = crate::combat::new_projectiles::aim_at_override()
        .and_then(|p| ship.single().ok().map(|s| s.transform_point(p.extend(0.0)).truncate()));
    // A scripted run's aim comes first: the real mouse may be sitting over
    // the window and would otherwise steer the run.
    aim.first = scripted.or_else(|| lock.aim_point());
    aim.then = cursor.or_else(|| selection.target.and_then(|t| targets.get(t).ok()).map(|g| g.translation().truncate()));
}

/// The rotating deck above a turret ring.
#[derive(Component, Debug, Clone)]
pub struct TurretBody {
    /// The Turret Ring module it stands on.
    pub ring: Entity,
    /// That ring's grid cell on the hull deck: how designs and saves say
    /// which turret a block belongs to.
    pub ring_cell: IVec2,
    /// 1, 3 or 5.
    pub size: i32,
    /// Ship-local heading of the turret's +x axis, radians.
    pub angle: f32,
    /// Degrees a second it can turn right now.
    pub traverse_deg_s: f32,
    pub mass_t: f32,
    pub motors: u32,
    /// Its ring has been shot out: it can't turn, and waits where it is for
    /// the ring to be rebuilt under it.
    pub jammed: bool,
    /// What it's turning toward this frame, in world space: what its guns
    /// fire at, so a turret and its guns never disagree.
    pub aim: Option<Vec2>,
    /// A point defence gun on it has a missile to shoot at, and `aim` is
    /// where to meet it (combat::missile_defence).
    pub pd_engaged: bool,
    /// How its ring was laid: with `ring_cell` and `size`, enough to lay it
    /// again (meta saves a shot-out ring this way).
    pub ring_rotation: crate::components::Rotation,
}

impl TurretBody {
    /// The hull cells its ring covers, from how the ring was laid -- still
    /// known with the ring shot out.
    pub fn ring_cells(&self) -> Vec<IVec2> {
        let t = ring_type(self.size);
        crate::building::GridOccupancy::cells_for(
            self.ring_cell,
            IVec2::splat(self.size),
            self.ring_rotation,
            crate::building::footprints::footprint_override(t),
        )
        .into_iter()
        .collect()
    }

    pub fn new(ring: Entity, ring_cell: IVec2, size: i32) -> Self {
        Self {
            ring, ring_cell, size, angle: 0.0, traverse_deg_s: 0.0, mass_t: 0.0, motors: 0, jammed: false, aim: None, pd_engaged: false,
            ring_rotation: crate::components::Rotation::North,
        }
    }
}

/// On a Turret Ring: the body standing on it.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasTurretBody(pub Entity);

/// A block standing on a turret deck. Its Module.grid_position is in the
/// turret's own grid.
#[derive(Component, Debug, Clone, Copy)]
pub struct OnTurret {
    pub ring_cell: IVec2,
}

/// A turret's floor plating. Made from the ring, so it is never saved.
#[derive(Component, Debug, Clone, Copy)]
pub struct TurretPlate;

/// A block that belongs on the turret standing on the ring at this hull
/// cell, waiting for that turret to exist (designs and saves spawn every
/// block in one go; the turret body comes a frame later).
#[derive(Component, Debug, Clone, Copy)]
pub struct PendingTurretMount(pub IVec2);

/// The ship something on a ship belongs to: its parent, or for a block on a
/// turret deck, the ship the turret stands on.
pub fn ship_of(
    entity: Entity,
    parents: &Query<&ChildOf>,
    bodies: &Query<(), With<TurretBody>>,
) -> Option<Entity> {
    let p = parents.get(entity).ok()?.parent();
    if bodies.contains(p) {
        parents.get(p).ok().map(|g| g.parent())
    } else {
        Some(p)
    }
}

pub fn is_turret_ring(t: ModuleType) -> bool {
    matches!(t, ModuleType::TurretRingSmall | ModuleType::TurretRing | ModuleType::TurretRingLarge)
}

/// The ring that carries a turret of this size.
pub fn ring_type(size: i32) -> ModuleType {
    match size {
        1 => ModuleType::TurretRingSmall,
        5 => ModuleType::TurretRingLarge,
        _ => ModuleType::TurretRing,
    }
}

pub fn ring_size(t: ModuleType) -> i32 {
    match t {
        ModuleType::TurretRingSmall => 1,
        ModuleType::TurretRingLarge => 5,
        _ => 3,
    }
}

/// How far from the ring's centre a turret's deck reaches, in cells.
pub fn deck_radius(size: i32) -> f32 {
    size as f32 / 2.0 + 1.05
}

/// The turret deck's cells: everything within its radius.
pub fn deck_cells(size: i32) -> Vec<IVec2> {
    let r = deck_radius(size);
    let span = r.ceil() as i32;
    let mut cells = Vec::new();
    for x in -span..=span {
        for y in -span..=span {
            if Vec2::new(x as f32, y as f32).length() <= r {
                cells.push(IVec2::new(x, y));
            }
        }
    }
    cells
}

/// The deck's angle pieces: an empty cell with deck on two sides that meet
/// gets a 45-degree plate filling the half toward them, which turns the
/// stepped edge into straight chamfers. (cell, toward): `toward` points from
/// the piece into the deck, one step on each axis.
pub fn deck_wedges(size: i32) -> Vec<(IVec2, IVec2)> {
    let cells: std::collections::HashSet<IVec2> = deck_cells(size).into_iter().collect();
    let span = deck_radius(size).ceil() as i32 + 1;
    let mut out = Vec::new();
    for x in -span..=span {
        for y in -span..=span {
            let c = IVec2::new(x, y);
            if cells.contains(&c) {
                continue;
            }
            for d in [IVec2::new(1, 1), IVec2::new(1, -1), IVec2::new(-1, 1), IVec2::new(-1, -1)] {
                if cells.contains(&(c + IVec2::new(d.x, 0))) && cells.contains(&(c + IVec2::new(0, d.y))) {
                    out.push((c, d));
                    break;
                }
            }
        }
    }
    out
}

/// What a deck tile is: the floor inside, armour round the outside, and the
/// angle pieces (armour too) on the chamfers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlateKind {
    Floor,
    Armour,
    AngleArmour(IVec2),
}

pub fn plate_kind(size: i32, cell: IVec2) -> Option<PlateKind> {
    let cells: std::collections::HashSet<IVec2> = deck_cells(size).into_iter().collect();
    if cells.contains(&cell) {
        let edge = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y].iter().any(|d| !cells.contains(&(cell + *d)));
        return Some(if edge { PlateKind::Armour } else { PlateKind::Floor });
    }
    deck_wedges(size).into_iter().find(|(c, _)| *c == cell).map(|(_, d)| PlateKind::AngleArmour(d))
}

/// The armour a turret's rim is made of.
pub const TURRET_ARMOUR: HullMaterial = HullMaterial::Titanium;

/// Tonnes, for working out how fast a turret turns. The Gun Deck preview's
/// table.
pub fn part_mass_t(t: ModuleType) -> f32 {
    match t {
        ModuleType::FiringPiece => 4.0,
        ModuleType::GaugeIncreaser => 1.5,
        ModuleType::GaugeReducer => 0.3,
        // The roof: plates and fittings (combat::gun_house).
        ModuleType::ArmorPlate | ModuleType::AngledArmorPlate | ModuleType::BevelledArmorPlate
        | ModuleType::SlopeArmorPlateLow | ModuleType::SlopeArmorPlateHigh
        | ModuleType::SlopeArmorPlateLowMirror | ModuleType::SlopeArmorPlateHighMirror
        | ModuleType::AngledReflectivePlate | ModuleType::ReflectivePlate
        | ModuleType::AblativeArmor | ModuleType::AngledAblativeArmor => 0.5,
        ModuleType::HalfArmorPlate => 0.3,
        ModuleType::Rangefinder | ModuleType::RoofVent | ModuleType::Periscope | ModuleType::ArmouredHatch => 0.4,
        ModuleType::Loader => 1.2,
        ModuleType::ClipRack => 0.8,
        ModuleType::ShellFeeder => 0.5,
        ModuleType::BreechCooler => 1.5,
        ModuleType::RecoilAbsorber => 2.0,
        ModuleType::GunBarrel => 0.9,
        ModuleType::MuzzleBrake => 1.0,
        ModuleType::Mantlet => 3.0,
        ModuleType::RailMagnet => 1.5,
        _ => 1.0,
    }
}
pub const PLATE_MASS_T: f32 = 0.5;

/// What a gun of this bore weighs against a 100 mm one: a firing piece,
/// loader or barrel for a 20 mm round is a fraction of one for 170 mm.
/// Bigger guns weigh what they always have.
pub fn gun_weight_scale(gauge_mm: f32) -> f32 {
    (gauge_mm / 100.0).clamp(0.2, 1.0)
}

/// A deck plate's weight: a small ring's deck is a light platform.
pub fn plate_mass_t(ring_size: i32) -> f32 {
    if ring_size <= 1 { 0.1 } else { PLATE_MASS_T }
}

/// The tint on a turret deck's floor tiles.
pub const TURRET_PLATE_TINT: Color = Color::srgb(0.86, 0.92, 1.0);

/// The hull cells a ring covers. From its real footprint: a square block
/// laid with a turn still grows from its origin cell in the turned
/// direction, so origin + (half, half) is only right facing North.
pub fn ring_cells(ring: &Module) -> Vec<IVec2> {
    crate::building::GridOccupancy::cells_for(
        ring.grid_position,
        ring.size,
        ring.rotation,
        crate::building::footprints::footprint_override(ring.module_type),
    )
    .into_iter()
    .collect()
}

/// Where a turret body stands, ship-local: the centre of its ring.
pub fn ring_centre(ring: &Module) -> (IVec2, Vec2) {
    let cells = ring_cells(ring);
    let sum = cells.iter().fold(IVec2::ZERO, |a, c| a + *c);
    let centre = sum / cells.len().max(1) as i32;
    (centre, crate::building::grid_to_local(centre))
}

/// One tile of a turret's deck, at a cell of its own grid.
pub fn spawn_plate(commands: &mut Commands, asset_server: &AssetServer, body: Entity, cell: IVec2, kind: PlateKind) {
    let (material, image, angle, facing) = match kind {
        PlateKind::Floor => (HullMaterial::Steel, "sprites/gun_parts/turret_plating.png", 0.0, None),
        PlateKind::Armour => (TURRET_ARMOUR, "sprites/gun_parts/turret_armour.png", 0.0, None),
        // The art carries its plate in the lower-left half; turn it so the
        // plate sits against the deck. Its open diagonal faces away from the
        // deck, which is the face `Block::facing` gives the hit resolver.
        PlateKind::AngleArmour(d) => {
            let turn = match (d.x, d.y) {
                (-1, -1) => 0.0,
                (1, -1) => std::f32::consts::FRAC_PI_2,
                (1, 1) => std::f32::consts::PI,
                _ => -std::f32::consts::FRAC_PI_2,
            };
            (TURRET_ARMOUR, "sprites/gun_parts/turret_armour_wedge.png", turn, Some((-d.y as f32).atan2(-d.x as f32)))
        }
    };
    let health = 100.0 * material.health_multiplier();
    commands.spawn((
        // A shade cooler than the hull's own plating, so the deck reads as a
        // separate floor above it.
        Sprite {
            image: asset_server.load(image),
            custom_size: Some(Vec2::splat(66.0)),
            color: TURRET_PLATE_TINT,
            ..default()
        },
        Transform::from_xyz(cell.x as f32 * 66.0, cell.y as f32 * 66.0, 0.0).with_rotation(Quat::from_rotation_z(angle)),
        HullSegment {
            hull_layer: HullLayer::Outer,
            material,
            radiation_shielding: material.radiation_shielding(),
            health,
            max_health: health,
            grid_position: cell,
            ..default()
        },
        crate::components::BaseSpriteColor(TURRET_PLATE_TINT),
        crate::building::Block { facing, ..crate::building::Block::hull(cell, material) },
        TurretPlate,
        ChildOf(body),
    ));
}

/// The crew have rebuilt a turret's floor tile (ship::rebuild).
#[derive(Message, Debug, Clone, Copy)]
pub struct RebuildTurretPlate {
    pub ship: Entity,
    pub ring_cell: IVec2,
    pub cell: IVec2,
}

/// Lay rebuilt floor tiles back on their turret, unless one's already there.
pub fn rebuild_turret_plates(
    mut commands: Commands,
    mut requests: MessageReader<RebuildTurretPlate>,
    bodies: Query<(Entity, &TurretBody, &ChildOf, Option<&Children>), Without<crate::meta::DespawnAtFrameEnd>>,
    plates: Query<&HullSegment, (With<TurretPlate>, Without<crate::components::HullDestroyed>)>,
    asset_server: Res<AssetServer>,
) {
    for r in requests.read() {
        let Some((body, t, _, children)) = bodies.iter().find(|(_, t, p, _)| p.parent() == r.ship && t.ring_cell == r.ring_cell) else { continue };
        let there = children.into_iter().flatten().any(|c| plates.get(*c).is_ok_and(|h| h.grid_position == r.cell));
        if let (false, Some(kind)) = (there, plate_kind(t.size, r.cell)) {
            spawn_plate(&mut commands, &asset_server, body, r.cell, kind);
        }
    }
}

/// Give every ring a turret body; keep each body on its ring.
///
/// - A ring shot out jams its turret, which stays put -- parts, guns and all
///   -- until the ring is rebuilt on the same cell, and then stands on the new
///   one. (Destroyed blocks are despawned half a second later and rebuilt as
///   new entities, so the body can't simply follow its ring entity.)
/// - A ring that breaks away with a severed section takes its turret along.
/// - A ring removed in build mode, or a ship replaced by a load, takes its
///   turret with it.
pub fn ensure_turret_bodies(
    mut commands: Commands,
    // A load marks the old ship's blocks to go at the end of the frame; a
    // ring on its way out is no ring.
    rings: Query<
        (Entity, &Module, &ChildOf, &Transform, Option<&HasTurretBody>, Has<DestroyedModule>),
        (Without<crate::meta::DespawnAtFrameEnd>, Without<TurretBody>),
    >,
    // A body on its way out with a load is no longer anyone's to take on.
    mut bodies: Query<(Entity, &mut TurretBody, &ChildOf, &mut Transform), Without<crate::meta::DespawnAtFrameEnd>>,
    asset_server: Res<AssetServer>,
) {
    for (ring, module, parent, _, has, destroyed) in rings.iter() {
        if !is_turret_ring(module.module_type) || has.is_some() || destroyed {
            continue;
        }
        // A jammed turret waiting on this cell takes the new ring.
        let waiting = bodies.iter_mut().find(|(_, t, p, _)| {
            p.parent() == parent.parent() && t.ring_cell == module.grid_position && t.jammed && rings.get(t.ring).is_err()
        });
        if let Some((body, mut t, _, _)) = waiting {
            t.ring = ring;
            t.jammed = false;
            t.ring_rotation = module.rotation;
            commands.entity(ring).try_insert(HasTurretBody(body));
            continue;
        }
        let size = ring_size(module.module_type);
        let (_, at) = ring_centre(module);
        let body = commands
            .spawn((
                TurretBody { ring_rotation: module.rotation, ..TurretBody::new(ring, module.grid_position, size) },
                // Above the hull (0.1) and its modules (0.2-0.4): the turret
                // stands on the deck. Its own blocks layer within this.
                Transform::from_translation(at.extend(1.2)),
                Visibility::default(),
                ChildOf(parent.parent()),
                Name::new("Turret"),
            ))
            .id();
        for cell in deck_cells(size) {
            let kind = plate_kind(size, cell).unwrap_or(PlateKind::Floor);
            spawn_plate(&mut commands, &asset_server, body, cell, kind);
        }
        for (cell, toward) in deck_wedges(size) {
            spawn_plate(&mut commands, &asset_server, body, cell, PlateKind::AngleArmour(toward));
        }
        commands.entity(ring).try_insert(HasTurretBody(body));
    }
    for (body, mut t, parent, mut transform) in bodies.iter_mut() {
        match rings.get(t.ring) {
            Ok((_, _, ring_parent, ring_tf, _, destroyed)) => {
                if destroyed && !t.jammed {
                    t.jammed = true;
                    if crate::combat::gun_assembly::trace() {
                        info!("TURRET JAMMED {:?}: its ring at {:?} was shot out", body, t.ring_cell);
                    }
                }
                // The ring broke away with a severed section: the turret goes
                // where it went. A ring's sprite stands at its footprint's
                // centre, which is where the turret stands.
                if ring_parent.parent() != parent.parent() {
                    transform.translation.x = ring_tf.translation.x;
                    transform.translation.y = ring_tf.translation.y;
                    commands.entity(body).try_insert(ChildOf(ring_parent.parent()));
                }
            }
            // Ring gone. Shot out, the turret waits jammed for a rebuild;
            // taken away any other way, the turret goes with it.
            Err(_) if t.jammed => {}
            Err(_) => {
                commands.entity(body).try_despawn();
            }
        }
    }
}

/// Move blocks waiting for a turret onto it, once it exists.
#[allow(clippy::type_complexity)]
pub fn mount_pending_turret_blocks(
    mut commands: Commands,
    mut pending: Query<(Entity, &PendingTurretMount, &ChildOf, &mut Transform, Option<&mut Sprite>, Option<&Children>)>,
    modules: Query<&Module>,
    bodies: Query<(Entity, &TurretBody, &ChildOf)>,
    live_rings: Query<(), (With<Module>, Without<DestroyedModule>, Without<crate::meta::DespawnAtFrameEnd>)>,
    asset_server: Res<AssetServer>,
) {
    for (block, want, parent, mut transform, sprite, kids) in pending.iter_mut() {
        let ship = parent.parent();
        // Only a turret whose ring is alive: right after a load the old
        // ship's turret is still there for a frame, on a ring that's going,
        // and anything mounted on it went with it.
        let Some((body, _, _)) = bodies
            .iter()
            .find(|(_, t, p)| p.parent() == ship && t.ring_cell == want.0 && live_rings.contains(t.ring))
        else {
            continue;
        };
        // spawn_module laid it out in the ship's frame, where cell (x, y)
        // stands at (66x, 66y - 33). The turret's frame has no half-cell
        // offset: (0, 0) is the ring's centre.
        transform.translation.y += 33.0;
        // What comes out through the front of the gun house is drawn over it.
        let kind = modules.get(block).map(|m| m.module_type).ok();
        if kind.is_some_and(|t| OUT_FRONT.contains(&t)) {
            transform.translation.z = GUN_HOUSE_Z + 0.1;
        }
        // The roof over the guns: above them, in the deck's armour a shade
        // lighter. An angle piece is drawn from its own art rather than the
        // hull's flat-coloured triangle (combat::gun_house).
        if let Some(t) = kind.filter(|t| super::gun_house::is_roof_block(*t)) {
            transform.translation.z = super::gun_house::ROOF_Z;
            if let (Some(art), Some(mut sprite)) = (super::gun_house::roof_sprite(t), sprite) {
                sprite.image = asset_server.load(art);
                sprite.color = Color::WHITE;
                commands.entity(block).try_insert(crate::components::BaseSpriteColor(Color::WHITE));
                if t == ModuleType::AngledArmorPlate {
                    for k in kids.into_iter().flatten() {
                        commands.entity(*k).try_despawn();
                    }
                }
            }
        }
        commands
            .entity(block)
            .remove::<PendingTurretMount>()
            .insert((OnTurret { ring_cell: want.0 }, ChildOf(body)));
    }
}

/// Each turret's shape this frame, for turrets that must not turn into each
/// other or shoot each other: where every block on it stands, in its own
/// frame, and where it stands on its ship.
#[derive(Resource, Default, Debug)]
pub struct TurretShapes(pub Vec<TurretShape>);

#[derive(Debug, Clone)]
pub struct TurretShape {
    pub body: Entity,
    pub ship: Entity,
    /// Ship-local centre (the ring's).
    pub centre: Vec2,
    pub angle: f32,
    /// Turret-frame positions of its blocks: plating, parts, barrels.
    pub parts: Vec<Vec2>,
    /// Furthest any of them reaches from the centre.
    pub reach: f32,
}

impl TurretShape {
    /// Its blocks, ship-local, were it turned to `angle`.
    pub fn at(&self, angle: f32) -> impl Iterator<Item = Vec2> + '_ {
        let rot = Vec2::from_angle(angle);
        self.parts.iter().map(move |p| self.centre + rot.rotate(*p))
    }
}

/// Two blocks this close (ship-local units) are in each other's way. A cell
/// is 66; a little under, so turrets can stand edge to edge.
pub const TURRET_CLEARANCE: f32 = 50.0;

/// How many of `a`'s blocks (turned to `angle`) foul any other turret on the
/// same ship.
pub fn fouling(a: &TurretShape, angle: f32, others: &[TurretShape]) -> usize {
    let near: Vec<&TurretShape> = others
        .iter()
        .filter(|o| o.body != a.body && o.ship == a.ship)
        .filter(|o| o.centre.distance(a.centre) < a.reach + o.reach + TURRET_CLEARANCE)
        .collect();
    if near.is_empty() {
        return 0;
    }
    a.at(angle)
        .filter(|p| near.iter().any(|o| o.at(o.angle).any(|q| q.distance(*p) < TURRET_CLEARANCE)))
        .count()
}

pub fn measure_turrets(
    mut shapes: ResMut<TurretShapes>,
    bodies: Query<(Entity, &TurretBody, &Transform, &ChildOf, Option<&Children>)>,
    blocks: Query<&Transform, (Or<(With<Module>, With<HullSegment>)>, Without<DestroyedModule>, Without<crate::components::HullDestroyed>)>,
) {
    shapes.0.clear();
    for (body, t, tf, parent, children) in bodies.iter() {
        let parts: Vec<Vec2> = children
            .into_iter()
            .flatten()
            .filter_map(|c| blocks.get(*c).ok())
            .map(|b| b.translation.truncate())
            .collect();
        let reach = parts.iter().map(|p| p.length()).fold(0.0, f32::max);
        shapes.0.push(TurretShape { body, ship: parent.parent(), centre: tf.translation.truncate(), angle: t.angle, parts, reach });
    }
}

/// Parts that come out through the front of a gun house rather than under it.
pub const OUT_FRONT: [ModuleType; 4] = [ModuleType::GunBarrel, ModuleType::MuzzleBrake, ModuleType::Mantlet, ModuleType::RailMagnet];
/// Above the parts it covers (they draw at 0.2-0.4 in the turret's frame).
pub const GUN_HOUSE_Z: f32 = 0.45;

/// Weigh each turret, count its motors, and turn it toward what it should
/// point at -- the player's aim for the player's ship -- no faster than it
/// can.
pub fn traverse_turrets(
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut TurretBody, &mut Transform, &ChildOf, Option<&Children>, Option<&super::gun_house::RoofFittings>)>,
    parts: Query<&Module, Without<DestroyedModule>>,
    plates: Query<(), (With<TurretPlate>, Without<crate::components::HullDestroyed>)>,
    ships: Query<
        (
            &GlobalTransform,
            Option<&Children>,
            Option<&crate::ai_ship::components::AiShipTarget>,
            Option<&crate::components::Velocity>,
            Option<&crate::combat::missile_defence::MissileWarning>,
        ),
        Or<(With<Ship>, With<crate::ai_ship::components::AiShip>)>,
    >,
    // Where an AI ship's target is now.
    whereabouts: Query<&GlobalTransform>,
    hull_modules: Query<&Module, Without<DestroyedModule>>,
    rings: Query<&Module>,
    aim: Res<PlayerAimPoint>,
    build: Option<Res<State<crate::states::BuildState>>>,
    shapes: Option<Res<TurretShapes>>,
    engaged: Query<&crate::combat::targeting::auto_engage::AutoAimPoint>,
    guns: Query<&crate::combat::gun_assembly::GunAssembly>,
    // Point defence guns, and the missiles they might shoot at.
    pd: (
        Query<&crate::combat::gun_assembly::GunAssembly, With<crate::combat::targeting::fire_groups::InterceptMode>>,
        // A point defence gun sees only the missiles its ship's warners do.
        Query<(Entity, &Transform, &crate::components::Velocity), (With<crate::combat::new_projectiles::MissileProjectile>, Without<TurretBody>)>,
        // Laser emitters on point defence: the beam is instant, so they aim
        // where the missile is, not where it will be.
        Query<(), (With<crate::combat::lasers::LaserEmitter>, With<crate::combat::targeting::fire_groups::InterceptMode>)>,
    ),
) {
    let dt = time.delta_secs();
    // While building, turrets stand squared to the hull, so a turret's cell
    // lies exactly over a hull cell (building::turret_build).
    let building = build.is_some_and(|s| *s.get() != crate::states::BuildState::Inactive);
    for (entity, mut body, mut transform, parent, children, fittings) in bodies.iter_mut() {
        let mut mass = 0.0;
        // The biggest gun aboard sets how heavy its parts are.
        let gauge = children
            .into_iter()
            .flatten()
            .filter_map(|c| guns.get(*c).ok())
            .map(|g| g.stats.gauge_mm)
            .fold(0.0f32, f32::max);
        let scale = if gauge > 0.0 { gun_weight_scale(gauge) } else { 1.0 };
        for c in children.into_iter().flatten() {
            if let Ok(m) = parts.get(*c) {
                mass += part_mass_t(m.module_type) * scale;
            } else if plates.contains(*c) {
                mass += plate_mass_t(body.size);
            }
        }
        body.mass_t = mass;
        // Motors: Traverse Motor blocks on the hull deck touching the ring.
        let ring_cells: Vec<IVec2> = rings.get(body.ring).map(ring_cells).unwrap_or_default();
        // A turret on a severed section has no ship to aim for.
        let Ok((ship_gt, ship_children, ai_target, ship_vel, warning)) = ships.get(parent.parent()) else {
            continue;
        };
        body.motors = ship_children
            .into_iter()
            .flatten()
            .filter_map(|c| hull_modules.get(*c).ok())
            .filter(|m| m.module_type == ModuleType::TraverseMotor)
            .filter(|m| ring_cells.iter().any(|r| (m.grid_position - *r).abs().max_element() <= 1 && !ring_cells.contains(&m.grid_position)))
            .count() as u32;
        body.traverse_deg_s = if body.motors == 0 || body.jammed {
            0.0
        } else {
            // Periscopes on the roof: the layer spots and calls the turn sooner.
            super::gun_math::traverse_deg_per_s(body.motors, body.mass_t) * fittings.map_or(1.0, |f| f.traverse_mult())
        };

        if building {
            if body.angle != 0.0 {
                body.angle = 0.0;
                transform.rotation = Quat::IDENTITY;
            }
            continue;
        }
        // The player's turrets follow the player's aim -- with nothing locked,
        // the block their first gun has engaged, so the turret turns to what
        // its guns will actually shoot at. An AI ship's follow whatever it
        // has picked to fight.
        // A point defence gun on it takes the turret onto the incoming
        // missile that will arrive first, whoever's ship it is.
        let ship = parent.parent();
        let pd_aim = children.into_iter().flatten().find_map(|c| pd.0.get(*c).ok()).and_then(|g| {
            let from = ship_gt.affine().transform_point3(transform.translation.with_z(0.0)).truncate();
            crate::combat::missile_defence::pd_solution(
                ship_gt.translation().truncate(),
                from,
                g.stats.velocity_mps * crate::combat::gun_assembly::GAME_SPEED_PER_MPS,
                ship_vel.map(|v| v.0).unwrap_or(Vec2::ZERO),
                crate::combat::gun_assembly::range_for(&g.stats).min(crate::combat::missile_defence::PD_RANGE),
                warning
                    .into_iter()
                    .flat_map(|w| w.spotted.iter())
                    .filter_map(|e| pd.1.get(*e).ok())
                    .map(|(_, t, v)| (t.translation.truncate(), v.0)),
            )
        });
        let laser_pd = children.into_iter().flatten().any(|c| pd.2.contains(*c));
        let pd_aim = pd_aim.or_else(|| {
            laser_pd.then(|| {
                let from = ship_gt.affine().transform_point3(transform.translation.with_z(0.0)).truncate();
                crate::combat::missile_defence::pd_solution(
                    ship_gt.translation().truncate(),
                    from,
                    1.0e6,
                    Vec2::ZERO,
                    crate::combat::lasers::PD_REACH,
                    warning
                        .into_iter()
                        .flat_map(|w| w.spotted.iter())
                        .filter_map(|e| pd.1.get(*e).ok())
                        .map(|(_, t, v)| (t.translation.truncate(), v.0)),
                )
            })?
        });
        body.pd_engaged = pd_aim.is_some();
        if crate::combat::gun_assembly::trace() && (time.elapsed_secs() % 1.0) < dt
            && children.into_iter().flatten().any(|c| pd.0.contains(*c))
        {
            let (spotted, warners) = warning.map_or((0, 0), |w| (w.spotted.len(), w.warners));
            info!("PD turret {:?}: {} enemy missiles spotted by {} warners, engaged {}, {:.0} t, {:.0} deg/s", entity, spotted, warners, body.pd_engaged, body.mass_t, body.traverse_deg_s);
        }
        // A point defence turret is the ship's, not the gunner's: it turns
        // only onto missiles, and holds where it is between them -- never
        // after the cursor or the ship's chosen target.
        let on_pd = laser_pd || children.into_iter().flatten().any(|c| pd.0.contains(*c));
        let target = if pd_aim.is_some() || on_pd { pd_aim } else { match ai_target {
            Some(t) => t.entity.map(|e| t.aim_at.unwrap_or_else(|| whereabouts.get(e).map(|g| g.translation().truncate()).unwrap_or(t.position))),
            None => {
                let first_gun = children.into_iter().flatten().find(|c| engaged.contains(**c));
                aim.for_turret(first_gun.and_then(|c| engaged.get(*c).ok()).map(|a| a.point))
            }
        } };
        body.aim = target;
        let Some(target) = target else { continue };
        // Heading to the aim point, in the ship's frame.
        let centre = ship_gt.affine().transform_point3(transform.translation.with_z(0.0)).truncate();
        let world = (target - centre).to_angle();
        let ship_rot = ship_gt.to_scale_rotation_translation().1.to_euler(EulerRot::ZYX).0;
        let want = world - ship_rot;
        let mut diff = (want - body.angle).rem_euclid(std::f32::consts::TAU);
        if diff > std::f32::consts::PI {
            diff -= std::f32::consts::TAU;
        }
        let max = body.traverse_deg_s.to_radians() * dt;
        let next = body.angle + diff.clamp(-max, max);
        // A physical stop: it won't turn its guns into another turret. It
        // may always turn the way that fouls less, so two built overlapping
        // can still work themselves apart.
        let blocked = shapes.as_ref().and_then(|s| s.0.iter().find(|t| t.body == entity).map(|shape| (shape, s)))
            .is_some_and(|(shape, s)| fouling(shape, next, &s.0) > fouling(shape, body.angle, &s.0));
        if crate::combat::gun_assembly::trace() && (time.elapsed_secs() % 1.0) < dt && !on_pd {
            info!("TURRET {:?}: {} motors, {:.0} t, {:.0} deg/s, at {:.0} deg, wants {:.0} deg, stopped by another turret {}", entity, body.motors, body.mass_t, body.traverse_deg_s, body.angle.to_degrees(), want.to_degrees(), blocked);
        }
        if blocked {
            continue;
        }
        body.angle = next;
        transform.rotation = Quat::from_rotation_z(body.angle);
    }
}

#[cfg(test)]
mod turret_deck_tests {
    use super::*;

    #[test]
    fn a_bigger_ring_carries_a_bigger_deck() {
        let small = deck_cells(1).len();
        let medium = deck_cells(3).len();
        let large = deck_cells(5).len();
        assert!(small < medium && medium < large, "{small} {medium} {large}");
        assert!(deck_cells(3).contains(&IVec2::new(2, 1)));
        assert!(!deck_cells(3).contains(&IVec2::new(3, 0)));
    }

    fn app_with_ring() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Image>();
        app.add_systems(Update, ensure_turret_bodies);
        let ship = app.world_mut().spawn((Ship, Transform::default())).id();
        let ring = spawn_ring(&mut app, ship);
        app.update();
        app.update();
        (app, ship, ring)
    }

    fn spawn_ring(app: &mut App, ship: Entity) -> Entity {
        let module = Module {
            module_type: ModuleType::TurretRing, health: 100.0, max_health: 100.0, power_consumption: 0.0,
            power_generation: 0.0, is_active: true, grid_position: IVec2::new(4, 0), size: IVec2::splat(3),
            rotation: crate::components::Rotation::North,
        };
        app.world_mut().spawn((module, Transform::from_xyz(330.0, 33.0, 0.2), ChildOf(ship))).id()
    }

    fn the_body(app: &mut App) -> Option<(Entity, TurretBody)> {
        app.world_mut().query::<(Entity, &TurretBody)>().iter(app.world()).next().map(|(e, t)| (e, t.clone()))
    }

    #[test]
    fn a_shot_out_ring_jams_its_turret_and_a_rebuilt_one_frees_it() {
        let (mut app, ship, ring) = app_with_ring();
        let (body, _) = the_body(&mut app).expect("a ring gets a turret");
        // Shot out, then cleared away half a second later.
        app.world_mut().entity_mut(ring).insert(DestroyedModule { original_type: ModuleType::TurretRing });
        app.update();
        assert!(the_body(&mut app).unwrap().1.jammed);
        app.world_mut().entity_mut(ring).despawn();
        app.update();
        let (still, t) = the_body(&mut app).expect("the turret waits for its ring");
        assert_eq!(still, body);
        assert!(t.jammed);
        // The crew rebuild the ring on the same cell.
        let new_ring = spawn_ring(&mut app, ship);
        app.update();
        let (same, t) = the_body(&mut app).unwrap();
        assert_eq!(same, body, "the same turret, parts and all");
        assert_eq!(t.ring, new_ring);
        assert!(!t.jammed);
    }

    #[test]
    fn a_load_builds_turrets_fresh_rather_than_adopting_the_old_jammed_one() {
        let (mut app, ship, ring) = app_with_ring();
        let (old, _) = the_body(&mut app).unwrap();
        app.world_mut().entity_mut(ring).insert(DestroyedModule { original_type: ModuleType::TurretRing });
        app.update();
        app.world_mut().entity_mut(ring).despawn();
        // The load marks the old turret to go, and lays the saved ring.
        app.world_mut().entity_mut(old).insert(crate::meta::DespawnAtFrameEnd);
        let new_ring = spawn_ring(&mut app, ship);
        app.update();
        let fresh = app
            .world_mut()
            .query::<(Entity, &TurretBody)>()
            .iter(app.world())
            .find(|(e, _)| *e != old)
            .map(|(e, t)| (e, t.ring));
        assert_eq!(fresh.map(|f| f.1), Some(new_ring), "a new turret on the loaded ring");
    }

    #[test]
    fn a_ring_taken_away_in_build_mode_takes_its_turret() {
        let (mut app, _, ring) = app_with_ring();
        app.world_mut().entity_mut(ring).despawn();
        app.update();
        app.update();
        assert!(the_body(&mut app).is_none());
    }

    #[test]
    fn a_ring_that_breaks_away_takes_its_turret_along() {
        let (mut app, _, ring) = app_with_ring();
        let section = app.world_mut().spawn(Transform::default()).id();
        app.world_mut().entity_mut(ring).insert((ChildOf(section), Transform::from_xyz(-20.0, 5.0, 0.2)));
        app.update();
        app.update();
        let (body, _) = the_body(&mut app).unwrap();
        assert_eq!(app.world().get::<ChildOf>(body).unwrap().parent(), section);
        let at = app.world().get::<Transform>(body).unwrap().translation;
        assert_eq!((at.x, at.y), (-20.0, 5.0));
    }

    /// A turret with a 4-cell barrel along +x, and a plain deck beside it.
    fn pair() -> (TurretShape, TurretShape) {
        let gun = TurretShape {
            body: Entity::from_raw_u32(1).unwrap(), ship: Entity::PLACEHOLDER, centre: Vec2::ZERO, angle: 0.0,
            parts: (0..5).map(|x| Vec2::new(x as f32 * 66.0, 0.0)).collect(), reach: 264.0,
        };
        let deck = TurretShape {
            body: Entity::from_raw_u32(2).unwrap(), ship: Entity::PLACEHOLDER, centre: Vec2::new(0.0, 264.0), angle: 0.0,
            parts: vec![Vec2::ZERO], reach: 0.0,
        };
        (gun, deck)
    }

    #[test]
    fn a_turret_wont_swing_its_barrel_into_a_neighbour() {
        let (gun, deck) = pair();
        let all = vec![gun.clone(), deck];
        assert_eq!(fouling(&gun, 0.0, &all), 0, "pointing ahead it clears");
        assert!(fouling(&gun, std::f32::consts::FRAC_PI_2, &all) > 0, "swung toward the neighbour it hits it");
        assert_eq!(fouling(&gun, -std::f32::consts::FRAC_PI_2, &all), 0, "the other way is clear");
    }

    #[test]
    fn turrets_on_different_ships_never_foul() {
        let (gun, mut deck) = pair();
        deck.ship = Entity::from_raw_u32(9).unwrap();
        let all = vec![gun.clone(), deck];
        assert_eq!(fouling(&gun, std::f32::consts::FRAC_PI_2, &all), 0);
    }

    #[test]
    fn a_small_deck_is_an_octagon_of_plates_and_angle_pieces() {
        let wedges = deck_wedges(3);
        assert_eq!(wedges.len(), 4, "one angle piece at each corner step: {wedges:?}");
        assert!(wedges.contains(&(IVec2::new(2, 2), IVec2::new(-1, -1))), "pointing back into the deck");
        assert_eq!(plate_kind(3, IVec2::ZERO), Some(PlateKind::Floor));
        assert_eq!(plate_kind(3, IVec2::new(2, 0)), Some(PlateKind::Armour), "the rim is armour");
        assert_eq!(plate_kind(3, IVec2::new(2, 2)), Some(PlateKind::AngleArmour(IVec2::new(-1, -1))));
        assert_eq!(plate_kind(3, IVec2::new(4, 0)), None);
    }

    #[test]
    fn an_angle_piece_faces_away_from_the_deck() {
        // Its open diagonal looks out, so a round from outside meets the
        // slope and one crossing the empty corner misses it.
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Image>();
        let body = app.world_mut().spawn_empty().id();
        let assets = app.world().resource::<AssetServer>().clone();
        app.world_mut().commands().queue(move |w: &mut World| {
            let mut q = bevy::ecs::world::CommandQueue::default();
            let mut c = Commands::new(&mut q, w);
            spawn_plate(&mut c, &assets, body, IVec2::new(2, 2), PlateKind::AngleArmour(IVec2::new(-1, -1)));
            q.apply(w);
        });
        app.update();
        let block = app.world_mut().query::<&crate::building::Block>().single(app.world()).unwrap();
        let facing = block.facing.expect("an angle piece declares its face");
        assert!((Vec2::from_angle(facing) - Vec2::new(1.0, 1.0).normalize()).length() < 1e-5, "{facing}");
    }

    #[test]
    fn rings_are_turret_rings() {
        assert!(is_turret_ring(ModuleType::TurretRing));
        assert!(!is_turret_ring(ModuleType::FiringPiece));
        assert_eq!(ring_size(ModuleType::TurretRingLarge), 5);
    }
}
