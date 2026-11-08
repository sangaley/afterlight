use bevy::prelude::*;
use crate::components::*;
use crate::resources::{OxygenState, HullState};
use crate::events::{ShowNotification, NotificationType};
use crate::building::registry::{ModuleRegistry, CompanionData};
use crate::sprite_map;

/// Spawns the initial starter ship (guards against duplicates)
/// Ship-shaped hull with tapered bow, engines at stern, weapons on exterior
/// Proper ship layout: bridge forward, engines aft, weapons on hull edges
pub fn spawn_starter_ship(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut oxygen_state: ResMut<OxygenState>,
    mut hull_state: ResMut<HullState>,
    registry: Res<ModuleRegistry>,
    mut notifications: MessageWriter<ShowNotification>,
    existing_ship: Query<Entity, With<Ship>>,
    mut rebuild_queue: ResMut<crate::ship::rebuild::RebuildQueue>,
) {
    // Guard: don't spawn a second ship
    if !existing_ship.is_empty() {
        return;
    }
    info!("Spawning starter vessel...");

    // Fresh run — stale ghosts belonged to the previous ship (their
    // sprites died with it as children of the old root).
    rebuild_queue.ghosts.clear();

    // Initialize oxygen
    oxygen_state.max_oxygen = 1800.0;
    oxygen_state.current_oxygen = 1800.0;
    hull_state.hull_integrity = 1.0;

    // Spawn the main ship entity (invisible anchor for movement)
    let ship = commands.spawn(ship_root_bundle(crate::world::home_base::SPAWN_BERTH)).id();

    // The starter destroyer is design data, not spawn calls — see
    // builtin_starter_design(). designs/starter.json overrides the built-in
    // (exported there on first run so it can be edited as JSON).
    //
    // AFTERLIGHT_DESIGN=<path> launches some other hull instead, without touching
    // the shipped one. designs/airlab.json is a bare three-compartment test
    // rig for watching air, doors and force fields do their jobs.
    let design = std::env::var("AFTERLIGHT_DESIGN")
        .ok()
        .and_then(|path| {
            let loaded = crate::building::blueprint::load_design_file(&path);
            if loaded.is_none() {
                warn!("AFTERLIGHT_DESIGN={} could not be loaded; using the starter", path);
            } else {
                info!("Flying AFTERLIGHT_DESIGN={}", path);
            }
            loaded
        })
        .or_else(|| crate::building::blueprint::load_design_file("designs/starter.json"))
        .unwrap_or_else(|| {
            let design = builtin_starter_design();
            if let Err(e) = crate::building::blueprint::write_design_file("designs/starter.json", &design) {
                warn!("Could not export starter design: {}", e);
            }
            design
        });

    crate::building::blueprint::spawn_ship_from_design(
        &mut commands,
        &asset_server,
        &registry,
        ship,
        &design,
    );

    info!(
        "Starter vessel '{}' spawned ({} hull, {} modules)",
        design.name,
        design.hull_cells.len(),
        design.modules.len()
    );

    notifications.write(ShowNotification {
        message: "Systems online.".into(),
        notification_type: NotificationType::Info,
        duration: 8.0,
    });
}

/// Everything the player's ship root carries, at rest at `position`.
///
/// Shared by a new run (above) and a loaded save (meta.rs), which kept its
/// own copy: a grey 200x80 placeholder box drawn under the hull and no
/// GravityAffected/GravityForce, so a ship loaded from the main menu ignored
/// every star and planet for the rest of the session. Inserted over an
/// existing root, it also zeroes whatever motion the ship had.
pub fn ship_root_bundle(position: Vec2) -> impl Bundle {
    (
        Transform::from_translation(position.extend(0.0)),
        // The ship carries no sprite of its own, but its ~120 hull/module
        // children do. Bevy propagates visibility down the hierarchy, so
        // without Visibility here every child logs B0004 on spawn (120 warnings
        // a run) and parent-level visibility can never hide the ship.
        Visibility::default(),
        Ship,
        Velocity(Vec2::ZERO),
        Depth(0.0),
        ThrusterState {
            base_drift: 0.0,
            current: 0.0,
        },
        Health {
            current: 150.0,
            max: 150.0,
        },
        ShipPhysics::default(),
        crate::celestial::components::GravityAffected { mass: 5000.0 },
        crate::celestial::components::GravityForce::default(),
    )
}

/// The starter destroyer expressed as design data (Blueprint v2). This is
/// the built-in fallback; on first run it's exported to designs/starter.json
/// and the file wins from then on.
///
/// Star-Destroyer wedge, symmetric across the centerline (top-down, x forward):
///
///     stern (flat, engines)                       bow (sharp point)
///     [O][O][O][O][O][O][O][O][O][O]
///     [O][O][O][O][O][O][O][O][O][O][O][O][O][O]
///     [O][O][O][O][O][O][O][O][O][O][O][O][O][O][O][O][O][O][O][O]  <- spine
///     [O][O][O][O][O][O][O][O][O][O][O][O][O][O]
///     [O][O][O][O][O][O][O][O][O][O]
///
/// Stern: engine bank/reactors/fuel · Mid: crew + gun deck · Bow: bridge/missiles/armor
pub(crate) fn builtin_starter_design() -> crate::building::blueprint::Blueprint {
    use crate::building::blueprint::{Blueprint, BlueprintHullCell, BlueprintModule, BLUEPRINT_VERSION};

    // "Leaning tender" — a working salvage hull, not a warship.
    //
    // The previous starter was an isosceles wedge, mirror-symmetric across
    // y=0: a competent shape and the same shape as everything else in the
    // genre. Worse, six of the ten faction hulls are also roughly symmetric
    // lozenges, so the ship the player owns read as one more of them.
    //
    // This one leans. The dorsal line climbs forward into a command tower
    // over the bow; the ventral line bulges aft into a cargo blister under
    // the stern. Nothing about it is mirrored top to bottom, so it is
    // recognisable at any range and from either side, and the profile says
    // what the ship is for before a single label does: eyes forward and high,
    // weight carried low and behind.
    //
    // Each row is (y, x_min, x_max). One span per row is a hard requirement,
    // not a style: `building::armour::{belt,caps}` derive the plating from
    // exactly this table and assume a single contiguous run per row. A
    // genuinely split silhouette (a hammerhead, a catamaran) needs those
    // helpers taught about gaps first.
    let hull_rows: &[(i32, i32, i32)] = &[
        ( 5,   3,  6),   // tower cap
        ( 4,   1,  7),
        ( 3,  -3,  8),
        ( 2,  -8,  9),
        ( 1,  -9, 10),
        ( 0,  -9, 10),   // spine: engine face (-9) to bow tip (+10)
        (-1,  -9,  9),
        (-2,  -8,  7),
        (-3,  -7,  2),
        (-4,  -6, -1),   // cargo keel, aft
    ];

    // Two transverse doors on the main corridor, where the ship changes job:
    // engineering to crew amidships, and crew to the forward bay. They are
    // real `NavCell::Door`s, so sealing one closes a route rather than
    // decorating it -- the first structural step toward compartmentalisation
    // being something the player designs around.
    let bulkheads: [IVec2; 2] = [IVec2::new(-4, 0), IVec2::new(2, 0)];

    let mut hull_cells = Vec::new();
    for &(y, x_min, x_max) in hull_rows {
        for x in x_min..=x_max {
            let pos = IVec2::new(x, y);
            if bulkheads.contains(&pos) {
                continue;
            }
            let is_top = !hull_rows.iter().any(|&(ry, rxmin, rxmax)| ry == y + 1 && x >= rxmin && x <= rxmax);
            let is_bot = !hull_rows.iter().any(|&(ry, rxmin, rxmax)| ry == y - 1 && x >= rxmin && x <= rxmax);
            let layer = if is_top || is_bot || x == x_min || x == x_max {
                HullLayer::Outer
            } else {
                HullLayer::Inner
            };
            hull_cells.push(BlueprintHullCell {
                grid_pos: pos,
                layer,
                material: HullMaterial::Steel,
            });
        }
    }
    for pos in bulkheads {
        hull_cells.push(BlueprintHullCell {
            grid_pos: pos,
            layer: HullLayer::BulkheadDoor,
            material: HullMaterial::Steel,
        });
    }

    let m = |module_type: ModuleType, x: i32, y: i32, rotation: Rotation| BlueprintModule {
        module_type,
        grid_pos: IVec2::new(x, y),
        rotation,
        custom_name: None,
        subcomponents: None,
        extras: None,
    };

    // Weapon variant: fire group + tuning multipliers (0.5-2.0x, see
    // TUNING_MIN/MAX) + optional kinetic ammo (Bullet-type weapons only —
    // Cannon/Railgun/Coilgun/Gatling; missiles/energy weapons pass None).
    let mw = |module_type: ModuleType, x: i32, y: i32, rotation: Rotation,
              fire_group: u8, velocity: f32, fire_rate: f32, damage: f32,
              ammo: Option<crate::combat::ammo_types::KineticAmmoType>| BlueprintModule {
        module_type,
        grid_pos: IVec2::new(x, y),
        rotation,
        custom_name: None,
        subcomponents: None,
        extras: Some(crate::building::blueprint::ModuleExtras {
            tuning: Some(crate::building::customization::tuning::WeaponTuning { velocity, fire_rate, damage, traverse: 1.0 }),
            fire_group: Some(fire_group),
            ammo: ammo.map(crate::building::customization::tuning::SelectedAmmo),
            turret: None,
            shell: None,
            missile: None,
            point_defence: false,
            signature: None,
        }),
    };

    // The layout tells the ship's job, stern to bow.
    //
    // Engineering aft, where a breach vents away from everyone. Crew
    // amidships behind a door. Command high and forward in the tower. The
    // forward bay is the working end: breaker drill, cargo, and the airlock,
    // all within a few paces of each other, because that is the loop the
    // player spends their time in.
    //
    // Survivability is laid out rather than bought. The two reactors sit on
    // opposite sides of the spine, so one breach leaves the other running and
    // the ship browns out instead of dying. The engines are split across
    // three rows for the same reason: `check_ai_cripple` calls a ship dead
    // when guns AND engines are both under a quarter, and a single bank is a
    // single hit away from that.
    let modules = vec![
        // --- Engineering, aft ---
        m(ModuleType::StandardEngine, -9, 1, Rotation::West),
        m(ModuleType::StandardEngine, -9, 0, Rotation::West),
        m(ModuleType::StandardEngine, -9, -1, Rotation::West),
        m(ModuleType::StandardEngine, -8, 2, Rotation::West),
        m(ModuleType::StandardEngine, -8, -2, Rotation::West),
        // x=-8 and x=-7 stay clear at y=+-1: that pair of cells is the only
        // route from the outboard engines down to the spine, and filling
        // either one walls the engine room off from its own crew.
        m(ModuleType::FuelTank, -6, 1, Rotation::North),
        m(ModuleType::FuelTank, -6, -1, Rotation::North),
        m(ModuleType::StandardReactor, -5, 1, Rotation::North),
        m(ModuleType::StandardReactor, -5, -1, Rotation::North),
        m(ModuleType::CoolingPump, -6, 2, Rotation::North),
        m(ModuleType::CoolingPump, -6, -2, Rotation::North),
        m(ModuleType::HeatVent, -7, 2, Rotation::North),
        m(ModuleType::HeatVent, -7, -2, Rotation::North),
        m(ModuleType::ManeuverThruster, -5, 2, Rotation::North),
        m(ModuleType::ManeuverThruster, -4, -3, Rotation::North),
        m(ModuleType::OxygenScrubber, -4, 1, Rotation::North),
        m(ModuleType::OxygenScrubber, -4, -1, Rotation::North),
        // --- Crew, amidships, behind the aft door ---
        // Two berths, not four. Four doubled the roster to forty on the same
        // pair of scrubbers, which is a life-support change wearing a layout
        // change's clothes. The hull has room for more; fitting them is the
        // player's decision to make and to pay for.
        m(ModuleType::BasicQuarters, -3, 1, Rotation::North),
        m(ModuleType::BasicQuarters, -3, -1, Rotation::North),
        m(ModuleType::GalleyMess, -2, 2, Rotation::North),
        m(ModuleType::RepairBay, 0, 1, Rotation::North),
        m(ModuleType::SurgicalBay, -2, -2, Rotation::East),
        // Ventral tube, aft under the keel: the only clear lane on that side.
        mw(ModuleType::HeavyMissile, -3, -4, Rotation::South, 3, 1.0, 1.0, 1.1, None),
        // --- Command: tower forward and high ---
        m(ModuleType::BridgeWing, 4, 3, Rotation::East),
        m(ModuleType::RadarArray, 6, 2, Rotation::East),
        m(ModuleType::Floodlight, 9, 2, Rotation::East),
        // Dorsal tube on the tower cap, firing into open sky.
        mw(ModuleType::HeavyMissile, 5, 5, Rotation::North, 3, 1.0, 1.0, 1.1, None),
        // --- Gun deck, along the shoulder ---
        mw(ModuleType::Cannon, 1, 2, Rotation::East, 0, 1.0, 1.0, 1.15, Some(crate::combat::ammo_types::KineticAmmoType::APHE)),
        mw(ModuleType::Cannon, 1, -2, Rotation::East, 0, 1.0, 1.0, 1.15, Some(crate::combat::ammo_types::KineticAmmoType::APHE)),
        m(ModuleType::ShieldEmitter, 3, 2, Rotation::North),
        m(ModuleType::ShieldEmitter, 3, -1, Rotation::North),
        mw(ModuleType::Gatling, 5, -1, Rotation::East, 1, 1.0, 1.2, 1.0, Some(crate::combat::ammo_types::KineticAmmoType::Flak)),
        mw(ModuleType::Gatling, 7, 2, Rotation::East, 1, 1.0, 1.2, 1.0, Some(crate::combat::ammo_types::KineticAmmoType::Flak)),
        mw(ModuleType::Railgun, 6, 0, Rotation::East, 2, 1.15, 0.85, 1.2, Some(crate::combat::ammo_types::KineticAmmoType::APFSDS)),
        // --- Memory cores, one per compartment ---
        //
        // Placed by hand, not searched for. The search this replaces took the
        // first, middle and last free interior cell in (x, y) order, and the
        // first of those is by construction the aftmost cell on the lowest
        // row -- which on this hull is the single corridor feeding the
        // outboard engines. It walled the engine room off from its own crew,
        // and nothing said so.
        //
        // One per compartment, so no single breach takes two and the run
        // cannot end to one unlucky hit.
        m(ModuleType::MemoryCore, -3, 2, Rotation::North),
        m(ModuleType::MemoryCore, 2, -1, Rotation::North),
        m(ModuleType::MemoryCore, 7, 1, Rotation::North),
        // --- Forward bay: the working end ---
        m(ModuleType::SalvageArm, 9, 1, Rotation::East),
        m(ModuleType::BulkCargoHold, 8, -1, Rotation::North),
        m(ModuleType::AirlockChamber, 10, 0, Rotation::East),
    ];

    // The player's ship gets the same treatment as every faction hull: plating
    // derived from its own outline, one plate per step of the wedge, plus caps
    // along the dorsal and ventral edges.
    //
    // It had two AngledArmorPlates before, at (8,0) and (9,0) — both on the
    // SPINE, which is hull. Hull wins its own cell in ShipGrid, so neither was
    // ever the thing a round met; they were decoration. Anything derived here
    // lands outboard, where it can actually be hit.
    let occupied: std::collections::HashSet<IVec2> = hull_cells
        .iter()
        .map(|c| c.grid_pos)
        .chain(modules.iter().map(|m| m.grid_pos))
        .collect();
    let mut modules = modules;
    let plating = crate::building::armour::belt(hull_rows, ModuleType::AngledArmorPlate)
        .into_iter()
        .chain(crate::building::armour::caps(hull_rows, ModuleType::AngledArmorPlate, 2));
    for (grid_pos, module_type, rotation) in plating {
        if occupied.contains(&grid_pos) {
            continue;
        }
        modules.push(BlueprintModule {
            module_type,
            grid_pos,
            rotation,
            custom_name: None,
            subcomponents: None,
            extras: None,
        });
    }

    lay_hallways(&mut hull_cells, &modules);

    Blueprint {
        name: "starter_destroyer".into(),
        hull_cells,
        modules,
        created_at: "builtin".into(),
        version: BLUEPRINT_VERSION,
        guns: Vec::new(),
    }
}

/// Turns every enclosed cell no module stands on into walkable decking.
///
/// Derived rather than hand-listed, so the corridors follow the layout: move a
/// module and the space that opens up behind it becomes hallway on the next
/// export, with no second list to drift out of sync. What's left as `Inner` is
/// structure — the cells modules occupy, which crew route around anyway.
///
/// This is what makes the destroyer walkable under the hallways-only rule. It
/// is not cosmetic: without it every post on the ship is unreachable.
pub(crate) fn lay_hallways(
    hull_cells: &mut [crate::building::blueprint::BlueprintHullCell],
    modules: &[crate::building::blueprint::BlueprintModule],
) {
    lay_hallways_with(&crate::building::registry::build_registry(), hull_cells, modules);
}

/// As `lay_hallways`, with a registry the caller already has. Building one
/// per call is the whole module table; an AI spawn has one to hand.
pub(crate) fn lay_hallways_with(
    registry: &crate::building::registry::ModuleRegistry,
    hull_cells: &mut [crate::building::blueprint::BlueprintHullCell],
    modules: &[crate::building::blueprint::BlueprintModule],
) {
    let occupied: std::collections::HashSet<IVec2> = modules
        .iter()
        .flat_map(|m| {
            let def = registry.get(m.module_type);
            crate::building::ShipGrid::cells_for(
                m.grid_pos,
                def.size,
                m.rotation,
                crate::building::footprints::footprint_override(m.module_type),
            )
        })
        .collect();

    for cell in hull_cells.iter_mut() {
        if cell.layer == HullLayer::Inner && !occupied.contains(&cell.grid_pos) {
            cell.layer = HullLayer::Hallway;
        }
    }
}

/// Spawns a module entity using the registry for stats and companion components
/// Does this module come out of the yard switched on?
///
/// Essential systems start active; everything else starts inactive to save
/// power. The exceptions are all the same lesson learned repeatedly: a module
/// a player placed, that silently does nothing because of a hidden power
/// toggle, reads as a bug rather than as a mechanic. A gun that won't fire, a
/// radar that won't scan ("no active detection module"), and most recently an
/// airlock that won't take the dead — crew::burial skips an inactive lock, so
/// the bodies simply piled up on the deck with nothing anywhere saying why.
pub fn starts_active(module_type: ModuleType) -> bool {
    matches!(
        module_type.category(),
        ModuleCategory::Power
            | ModuleCategory::Propulsion
            | ModuleCategory::LifeSupport
            | ModuleCategory::Weapons
            | ModuleCategory::Guns
            | ModuleCategory::Lasers
            | ModuleCategory::Detection
    ) || matches!(
        module_type,
        ModuleType::HelmStation
            | ModuleType::ManeuverThruster
            | ModuleType::CoolingPump
            | ModuleType::HeatVent
            | ModuleType::BasicQuarters
            | ModuleType::Barracks
            | ModuleType::Floodlight
            | ModuleType::RepairBay
            | ModuleType::AirlockChamber
            | ModuleType::DockingPort
            // Without these `air::auto_containment` passes over them: a door
            // that was never switched on never shuts.
            | ModuleType::EmergencyBulkhead
            | ModuleType::AirlockValve
    )
}

pub fn spawn_module(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    module_type: ModuleType,
    grid_pos: IVec2,
    rotation: Rotation,
    registry: &ModuleRegistry,
) -> Entity {
    spawn_module_signed(commands, asset_server, parent, module_type, grid_pos, rotation, registry, None)
}

/// `spawn_module`, built to a faction's signature (diplomacy::signatures):
/// its health, power draw, colour and main number are the signature's from
/// the first frame, so a loaded block's saved health lands on the right
/// maximum. A signature for a different block is ignored.
#[allow(clippy::too_many_arguments)]
pub fn spawn_module_signed(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    module_type: ModuleType,
    grid_pos: IVec2,
    rotation: Rotation,
    registry: &ModuleRegistry,
    signature: Option<&'static crate::diplomacy::signatures::SignatureDef>,
) -> Entity {
    let def = registry.get(module_type);
    let signature = signature.filter(|s| s.base == module_type);

    // Calculate sprite size and center position for multi-cell modules
    let footprint = crate::building::footprints::footprint_override(module_type);
    let cells = crate::building::GridOccupancy::cells_for(grid_pos, def.size, rotation, footprint);
    let (min_x, max_x, min_y, max_y) = cells.iter().fold(
        (i32::MAX, i32::MIN, i32::MAX, i32::MIN),
        |(mnx, mxx, mny, mxy), c| (mnx.min(c.x), mxx.max(c.x), mny.min(c.y), mxy.max(c.y)),
    );
    let center_x = (min_x as f32 + max_x as f32) / 2.0 * 66.0;
    let center_y = (min_y as f32 + max_y as f32) / 2.0 * 66.0 - 33.0;

    let sprite_path = sprite_map::module_sprite_path(module_type)
        .unwrap_or("sprites/modules/small_reactor.png");
    let texture = asset_server.load(sprite_path);

    let visual_angle = rotation.to_radians() + sprite_map::sprite_base_rotation(module_type);

    // Sprite dimensions must cover the ROTATED cell bounds after the
    // sprite itself is rotated by visual_angle — which is NOT the cell
    // rotation (it includes each texture's base-art offset, e.g. engine
    // art drawn 90° off). So take the rotated cell bounds and un-rotate
    // them by the final visual angle: if it's an odd quarter-turn the
    // width/height swap. Anything else leaves multi-cell modules lying
    // 90° across their claimed cells ("between the grid").
    let bounds_w = (max_x - min_x) as f32;
    let bounds_h = (max_y - min_y) as f32;
    let quarter = ((visual_angle / std::f32::consts::FRAC_PI_2).round() as i32).rem_euclid(4);
    // Local (unrotated) sprite size. Barrel weapons get a forward-extended
    // canvas — their art is drawn 1:3 with the turret centered and the barrel
    // pointing up — so the barrel overhangs the cells ahead. Combined with the
    // raised z below, the barrel renders over neighbouring blocks. The extension
    // is applied in LOCAL space and then run through the same quarter-turn swap,
    // so the barrel follows the weapon's facing at any rotation.
    // 66 is the grid cell. This was 60, so every module was drawn 6 units
    // narrower than the cell it occupies -- a 9% gap on every side, and since
    // only about half the modules on a hull sit over a hull plate, most of
    // those gaps showed open space straight through the ship.
    let local_w = 66.0 + bounds_w * 66.0;
    // Directional parts (thruster nozzles, gun barrels) protrude PAST their
    // block: the sprite is lengthened along its local vertical axis by
    // `overhang`, and the whole sprite is nudged so the housing stays centred on
    // the cell while the extension hangs off the protruding end. `protrude` is
    // +1 when that end is the art's top (barrels), -1 when it's the bottom
    // (nozzles). The matching art carries the extra length in its canvas so
    // nothing stretches.
    let (overhang, protrude) = sprite_map::sprite_overhang(module_type);
    let local_h = 66.0 + bounds_h * 66.0;
    // Footprint dims after the quarter-turn swap (handles rotated multi-cell
    // aspect). The swap must run on the FOOTPRINT only.
    let (foot_w, foot_h) = if quarter % 2 == 1 {
        (local_h, local_w)
    } else {
        (local_w, local_h)
    };
    // The overhang lengthens the TEXTURE's own vertical axis (barrel/nozzle),
    // which is always sprite_h regardless of the footprint swap. Adding it before
    // the swap put it on the width for East/West-facing parts (odd quarter-turns)
    // and stretched them — the in-game gun/engine distortion.
    let sprite_w = foot_w;
    let sprite_h = foot_h + overhang;
    // Offset in art-local space (art +Y = up), rotated into world by the same
    // visual angle so the barrel/nozzle follows the part's facing at any turn.
    let sprite_off = Quat::from_rotation_z(visual_angle)
        * Vec3::new(0.0, protrude * overhang * 0.5, 0.0);
    // Raise directional parts so the protruding end renders over the neighbour.
    let sprite_z = if overhang > 0.0 { 0.4 } else { 0.2 };

    // The new module sprites carry their own colour and detail, so render
    // them at full white — the old per-module def.color multiply (a leftover
    // from the flat placeholder art that relied on colour to tell modules
    // apart) washed the detailed sprites into solid coloured blocks. Damage
    // darkening and wreck greying still work: they multiply DOWN from this
    // base (stored in BaseSpriteColor), and white is their neutral start.
    // Wedges draw nothing here: their whole point is a silhouette that ISN'T a
    // square, and the shared module sprite is a full textured square. Painting
    // a dark triangle over it just made a square with a triangle on it — the
    // outline never actually got cut. The triangle is built from child quads
    // below instead, so the block's shape is the shape.
    let is_wedge = matches!(
        module_type,
        ModuleType::AngledArmorPlate | ModuleType::AngledHullPlate | ModuleType::AngledAblativeArmor | ModuleType::AngledReflectivePlate
    );
    // Ablative and reflective plate share the plain plate's art: their colour
    // is how you tell the three armours apart in a fight.
    let module_base_color = if is_wedge {
        Color::NONE
    } else if matches!(module_type, ModuleType::AblativeArmor | ModuleType::ReflectivePlate) {
        def.color
    } else {
        Color::WHITE
    };
    let module_base_color = match signature {
        Some(sig) if !is_wedge => crate::diplomacy::signatures::tint(sig, module_base_color),
        _ => module_base_color,
    };
    let health = def.health * signature.map_or(1.0, |s| s.health);
    // A reactor's power is its power_generation; Reactor.output only drives
    // heat and the blast, so a signature reactor makes more power without
    // running hotter (its blast is the signature's own `blast`).
    let power_generation = def.power_generation * signature.filter(|_| def.power_generation > 0.0).map_or(1.0, |s| s.output);

    let module_entity = commands.spawn((
        (Sprite {
                image: texture,
                color: module_base_color,
                custom_size: Some(Vec2::new(sprite_w, sprite_h)),
                ..default()
            }, Transform {
                translation: Vec3::new(center_x + sprite_off.x, center_y + sprite_off.y, sprite_z),
                rotation: Quat::from_rotation_z(visual_angle),
                ..default()
            }),
        BaseSpriteColor(module_base_color),
        Module {
            module_type,
            health,
            max_health: health,
            power_consumption: def.power_consumption * signature.map_or(1.0, |s| s.power),
            power_generation,
            is_active: starts_active(module_type),
            grid_position: grid_pos,
            size: def.size,
            rotation,
        },
        crate::building::Block::for_module(grid_pos, module_type, rotation),
        Selectable,
    )).id();

    match signature.and_then(|sig| scale_companion(&def.companion, sig)) {
        Some(scaled) => insert_companion_components(commands, module_entity, &scaled),
        None => insert_companion_components(commands, module_entity, &def.companion),
    }
    if let Some(sig) = signature {
        commands.entity(module_entity).insert(crate::diplomacy::signatures::Signature(sig));
        // A reactor that "explodes harder": the same blast insert_companion_
        // components gives it, scaled, without touching the output its heat
        // is figured from.
        if let CompanionData::Reactor { output, explosion_risk: true, .. } = def.companion {
            if sig.blast != 1.0 {
                let blast_radius = if output >= 200.0 { 2.5 } else if output >= 100.0 { 2.0 } else { 1.5 };
                commands.entity(module_entity).insert(Explosive {
                    blast_radius: blast_radius * sig.blast.sqrt(),
                    blast_damage: output * 0.5 * sig.blast,
                    explosive_type: ExplosiveType::Reactor,
                });
            }
        }
    }

    // Gun turrets: the module sprite above is the static base mount; add a
    // pivot-centred barrel child sprite that the aim system rotates to track the
    // target (cursor for the player, the player ship for AI).
    if let Some(barrel_path) = sprite_map::turret_barrel_sprite(module_type) {
        let barrel = commands.spawn((
            Sprite {
                image: asset_server.load(barrel_path),
                custom_size: Some(Vec2::splat(132.0)),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 0.3),
            TurretBarrel,
        )).id();
        commands.entity(module_entity).add_child(barrel);
        commands.entity(module_entity).insert(Turret {
            turn_speed: sprite_map::turret_turn_speed(module_type),
            world_angle: 0.0,
        });
    }

    // WEDGES: the plate itself, as a staircase of quads filling the half-cell
    // the armour actually occupies — the same half `Block::facing` reports to
    // hit resolution, so what you see is what a round meets.
    //
    // Children inherit the parent's visual_angle, so this is authored once
    // facing north-east and R turns it with the block.
    if is_wedge {
        // Half the 1x1 module sprite, which is the full 66-unit cell now (see
        // local_w/local_h above). This was 30 from when sprites were 60, and
        // left a 3-unit seam between every wedge and the plates beside it.
        const H: f32 = 33.0;
        const BANDS: usize = 20;
        let band = (H * 2.0) / BANDS as f32;
        let face = def.color;
        // `face` IS the hull's tone now (sprite_map::HULL_TONE), so the body
        // is drawn at it unchanged — a wedge should be the same plating as the
        // block beside it, continued into half a cell. The old 35% darkening
        // was there to pull a much paler base down towards the hull; applied
        // to a base that already matches, it just cuts a dark notch instead.
        let body = face;
        for i in 0..BANDS {
            let y = H - band * (i as f32 + 0.5);
            // `facing` is an outward normal, so the plate's material sits
            // BEHIND it: the half with x + y <= 0. At height y that runs from
            // the left edge in to x = -y. Drawing the other half puts the
            // mass on the outside and leaves the plate hanging off the hull
            // with a gap where it should be bolted on.
            let width = (H - y).clamp(0.0, H * 2.0);
            if width <= 0.5 { continue; }
            let quad = commands.spawn((
                Sprite {
                    color: body,
                    custom_size: Some(Vec2::new(width, band + 0.5)),
                    ..default()
                },
                Transform::from_xyz(-H + width * 0.5, y, 0.05),
            )).id();
            commands.entity(module_entity).add_child(quad);
        }
        // Lit edge along the hypotenuse — the sloped face, and the only part of
        // the block that tells you which way it's turned.
        let edge = commands.spawn((
            Sprite {
                // The sloped face catches the light, and is the only part of
                // the block that says which way it is turned. Eased from 0.35
                // to 0.30 so it keeps the contrast it had against the old pale
                // body rather than becoming a stripe on the new dark one.
                color: face.mix(&Color::WHITE, 0.30),
                custom_size: Some(Vec2::new(H * 2.0 * std::f32::consts::SQRT_2 - 2.0, 3.0)),
                ..default()
            },
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.08),
                rotation: Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4),
                ..default()
            },
        )).id();
        commands.entity(module_entity).add_child(edge);
    }

    // FirebreakWall gets a marker component for fire blocking
    if module_type == ModuleType::FirebreakWall {
        commands.entity(module_entity).insert(FirebreakMarker);
    }

    // Insert CrewStation if this module type requires one. Priority
    // orders auto-assignment — vital systems staff first, because an
    // UNMANNED station doesn't run at all (see compute_module_efficiency),
    // and 8 crew never cover every station on a real ship.
    if def.crew_station {
        let priority = match module_type {
            ModuleType::HelmStation => 9,
            _ => match module_type.category() {
                ModuleCategory::Power => 10,
                ModuleCategory::Propulsion => 9,
                ModuleCategory::LifeSupport => 8,
                ModuleCategory::Weapons => 6,
                ModuleCategory::Control | ModuleCategory::Detection => 4,
                _ => 3,
            },
        };
        commands.entity(module_entity).insert(CrewStation {
            priority,
            assigned_crew: None,
            manually_assigned: false,
        });
    }

    // Insert ModuleTemperature for heat network — defaults by category
    let (max_temp, conductivity) = match module_type.category() {
        ModuleCategory::Power => {
            // Reactors run hotter
            if matches!(module_type, ModuleType::SmallReactor | ModuleType::StandardReactor
                | ModuleType::LargeReactor | ModuleType::FusionReactor | ModuleType::RTG) {
                (100.0, 0.8)
            } else {
                (80.0, 0.5)
            }
        }
        ModuleCategory::Propulsion => (80.0, 0.6),
        ModuleCategory::Weapons => (60.0, 0.4),
        ModuleCategory::Structural => (200.0, 0.3),
        _ => {
            // CoolingPump/HeatVent are heat-resistant and highly conductive
            if matches!(module_type, ModuleType::CoolingPump | ModuleType::HeatVent) {
                (150.0, 1.0)
            } else {
                (80.0, 0.5)
            }
        }
    };
    commands.entity(module_entity).insert(ModuleTemperature {
        current: 0.0,
        max_temp,
        conductivity,
    });

    // Storage modules that are explosive
    match module_type {
        ModuleType::FuelTank => {
            commands.entity(module_entity).insert(Explosive {
                blast_radius: 2.0,
                blast_damage: 40.0,
                explosive_type: ExplosiveType::Fuel,
            });
        }
        // Stored ammunition goes up hard enough to set off the store beside
        // it, so a round that finds a packed magazine can gut the ship --
        // which is what armour round your magazines is for. At 60-70 a bay
        // couldn't even crack its neighbour and a cook-off was a pop.
        ModuleType::AmmoBay => {
            commands.entity(module_entity).insert(Explosive {
                blast_radius: 2.5,
                blast_damage: 350.0,
                explosive_type: ExplosiveType::Ammo,
            });
        }
        // A gun's shells: the magazine is the big one, a clip rack holds a
        // few ready rounds.
        ModuleType::Magazine => {
            commands.entity(module_entity).insert(Explosive {
                blast_radius: 3.0,
                blast_damage: 450.0,
                explosive_type: ExplosiveType::Ammo,
            });
        }
        ModuleType::ClipRack => {
            commands.entity(module_entity).insert(Explosive {
                blast_radius: 1.5,
                blast_damage: 120.0,
                explosive_type: ExplosiveType::Ammo,
            });
        }
        ModuleType::BatteryBank => {
            commands.entity(module_entity).insert(Explosive {
                blast_radius: 1.0,
                blast_damage: 20.0,
                explosive_type: ExplosiveType::Battery,
            });
        }
        _ => {}
    }

    // Add ModuleCustomization for customizable weapons (Tier 2+3 support)
    if def.customizable && module_type.category() == ModuleCategory::Weapons {
        commands.entity(module_entity).insert(
            crate::building::customization::parameters::ModuleCustomization::default()
        );
    }

    // Stat tuning (power-budget sliders) — defaults are identity multipliers,
    // so AI ships spawning through this same path are unaffected.
    {
        use crate::building::customization::tuning;
        if tuning::is_tunable_weapon(module_type) {
            commands.entity(module_entity).insert(tuning::WeaponTuning::default());
        }
        if tuning::is_kinetic_weapon(module_type) {
            commands.entity(module_entity).insert(tuning::SelectedAmmo::default());
        }
    }

    // Add MachineBlock component for multi-block machines
    {
        use crate::building::multiblock::components::*;
        let machine_role = match module_type {
            // Weapon cores
            ModuleType::Cannon | ModuleType::Railgun | ModuleType::Coilgun |
            ModuleType::Gatling | ModuleType::Laser | ModuleType::PlasmaCaster |
            ModuleType::IonDisruptor | ModuleType::HeavyMissile | ModuleType::GuidedMissile |
            ModuleType::ClusterRocket | ModuleType::MiningDrill | ModuleType::TractorBeam |
            ModuleType::EMPPulse => Some((BlockRole::Core, true)),
            // Reactor cores
            ModuleType::SmallReactor | ModuleType::StandardReactor |
            ModuleType::LargeReactor | ModuleType::FusionReactor => Some((BlockRole::Core, true)),
            // Engine cores
            ModuleType::SmallEngine | ModuleType::StandardEngine |
            ModuleType::LargeEngine => Some((BlockRole::Core, true)),
            // Extension blocks
            ModuleType::BarrelExtension => Some((BlockRole::Barrel, false)),
            ModuleType::AmmoFeedUnit => Some((BlockRole::AmmoFeed, false)),
            ModuleType::CoolingJacket => Some((BlockRole::Cooling, false)),
            ModuleType::ReactorFuelRod => Some((BlockRole::FuelRod, false)),
            ModuleType::ReactorCooling => Some((BlockRole::Cooling, false)),
            ModuleType::EngineNozzle => Some((BlockRole::Nozzle, false)),
            ModuleType::ShieldEmitter => Some((BlockRole::ShieldEmitter, false)),
            _ => None,
        };

        // A firing piece's numbers come from the gun built around it
        // (combat::gun_assembly), which rewrites this snapshot whenever the
        // ship's layout changes. It needs one from the start: the enhancer
        // pass resets every weapon from its BaseWeaponStats before folding
        // tuning in, and a weapon without one would compound its tuning
        // every frame.
        // A launcher's likewise come from its missile design
        // (combat::missile_design::fit_missile_loadouts).
        if module_type == ModuleType::FiringPiece || crate::combat::missile_design::is_built_launcher(module_type) {
            if let CompanionData::Weapon { damage, range, fire_rate, ammo, .. } = &def.companion {
                commands.entity(module_entity).insert(crate::building::multiblock::components::BaseWeaponStats {
                    damage: *damage,
                    range: *range,
                    fire_rate: *fire_rate,
                    max_ammo: *ammo,
                });
            }
        }

        if let Some((role, is_core)) = machine_role {
            commands.entity(module_entity).insert(MachineBlock {
                role,
                connected_core: if is_core { Some(module_entity) } else { None },
                chain_distance: 0,
                next_in_chain: None,
                prev_in_chain: None,
            });

            if is_core {
                commands.entity(module_entity).insert(MachineStats::default());
                // Stable base-stat snapshot — see BaseWeaponStats docs for
                // why calculate_machine_stats must never read live Weapon
                // values as its "base".
                if let CompanionData::Weapon { damage, range, fire_rate, ammo, .. } = &def.companion {
                    commands.entity(module_entity).insert(BaseWeaponStats {
                        damage: *damage,
                        range: *range,
                        fire_rate: *fire_rate,
                        max_ammo: *ammo,
                    });
                }
            }

            // Barrel blocks get stress tracking and cascade risk
            if role == BlockRole::Barrel {
                commands.entity(module_entity).insert(BarrelStress {
                    load: 1,
                    effective_cascade_chance: 0.15,
                });
                commands.entity(module_entity).insert(CascadeRisk::default());
            }
        }
    }

    commands.entity(module_entity).insert(ChildOf(parent));

    module_entity
}

/// Spawns a custom module with ship-components
pub fn spawn_custom_module(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    base_type: ModuleType,
    custom_name: String,
    grid_pos: IVec2,
    rotation: Rotation,
    subcomponents: Vec<SubComponentType>,
    registry: &ModuleRegistry,
) -> Entity {
    // First spawn the base module using the standard function
    let module_entity = spawn_module(
        commands,
        asset_server,
        parent,
        base_type,
        grid_pos,
        rotation,
        registry,
    );

    // Add CustomModule component
    commands.entity(module_entity).insert(CustomModule {
        base_type,
        custom_name,
    });

    // Spawn ship-component entities as children
    for subcomponent_type in subcomponents {
        let subcomponent_entity = commands.spawn(SubComponent {
            subcomponent_type,
            parent_module: module_entity,
        }).id();

        commands.entity(subcomponent_entity).insert(ChildOf(module_entity));
    }

    // The recalculation system will automatically trigger due to Changed<Children>

    module_entity
}

/// A block's companion numbers, scaled by its signature: `output` on its main
/// number. None when the companion has nothing the
/// signature touches.
fn scale_companion(c: &CompanionData, sig: &crate::diplomacy::signatures::SignatureDef) -> Option<CompanionData> {
    let o = sig.output;
    Some(match *c {
        CompanionData::Engine { thrust, noise_level } => CompanionData::Engine { thrust: thrust * o, noise_level },
        CompanionData::OxygenScrubber { output } => CompanionData::OxygenScrubber { output: output * o },
        CompanionData::LifeSupport { o2_gen, co2_filter } => CompanionData::LifeSupport { o2_gen: o2_gen * o, co2_filter: co2_filter * o },
        CompanionData::Thruster { thrust_power } => CompanionData::Thruster { thrust_power: thrust_power * o },
        CompanionData::Cargo { capacity } => CompanionData::Cargo { capacity: capacity * o },
        CompanionData::Radar { range, noise_on_ping } => CompanionData::Radar { range: range * o, noise_on_ping },
        CompanionData::PassiveRadar { range } => CompanionData::PassiveRadar { range: range * o },
        CompanionData::Detection { range } => CompanionData::Detection { range: range * o },
        CompanionData::Repair { rate } => CompanionData::Repair { rate: rate * o },
        CompanionData::Quarters { berths } => CompanionData::Quarters { berths: (berths as f32 * o).round() as u32 },
        CompanionData::TargetingComputer { accuracy_bonus } => CompanionData::TargetingComputer { accuracy_bonus: accuracy_bonus * o },
        _ => return None,
    })
}

/// Inserts companion components on a module entity based on registry data
fn insert_companion_components(commands: &mut Commands, entity: Entity, companion: &CompanionData) {
    match companion {
        CompanionData::None => {}
        CompanionData::Reactor { output, max_heat, explosion_risk } => {
            commands.entity(entity).insert(Reactor {
                output: *output,
                heat: 0.0,
                max_heat: *max_heat,
                explosion_risk: *explosion_risk,
            });
            if *explosion_risk {
                let blast_radius = if *output >= 200.0 { 2.5 }
                    else if *output >= 100.0 { 2.0 }
                    else { 1.5 };
                commands.entity(entity).insert(Explosive {
                    blast_radius,
                    blast_damage: *output * 0.5,
                    explosive_type: ExplosiveType::Reactor,
                });
            }
        }
        CompanionData::Engine { thrust, noise_level } => {
            commands.entity(entity).insert(Engine {
                thrust: *thrust,
                fuel_consumption: 1.0,
                noise_level: *noise_level,
            });
        }
        CompanionData::OxygenScrubber { output } => {
            commands.entity(entity).insert(OxygenScrubber {
                output: *output,
            });
        }
        CompanionData::LifeSupport { o2_gen, co2_filter } => {
            commands.entity(entity).insert(LifeSupportSystem {
                o2_generation: *o2_gen,
                co2_filtering: *co2_filter,
                waste_recycling: 0.0,
            });
        }
        CompanionData::Thruster { thrust_power } => {
            commands.entity(entity).insert(Thruster {
                thrust_power: *thrust_power,
                current_output: 0.5,
            });
        }
        CompanionData::Cargo { capacity } => {
            commands.entity(entity).insert(CargoHold {
                capacity: *capacity,
                current_weight: 0.0,
            });
        }
        CompanionData::Weapon { damage, range, fire_rate, ammo, mount_type, ammo_type } => {
            commands.entity(entity).insert((
                Weapon {
                    damage: *damage,
                    range: *range,
                    fire_rate: *fire_rate,
                    ammo: *ammo,
                    max_ammo: *ammo,
                },
                WeaponCooldown {
                    timer: Timer::from_seconds(1.0 / fire_rate, TimerMode::Once),
                },
                WeaponMount {
                    mount_type: *mount_type,
                    // Fixed widened from 30° — that required aiming almost
                    // exactly where the weapon was physically mounted, so
                    // Fixed-mount guns (Railgun, some missiles) frequently
                    // just didn't fire at all while aiming normally.
                    firing_arc: match mount_type {
                        MountType::Fixed => 120.0,
                        MountType::Turret => 360.0,
                        MountType::Broadside => 180.0,
                    },
                },
                TargetingSystem {
                    tracking_speed: 1.0,
                    lock_on_time: 0.5,
                    max_targets: 1,
                },
                AmmoStorage {
                    ammo_type: *ammo_type,
                    capacity: *ammo * 2,
                    current: *ammo,
                },
                crate::combat::targeting::fire_groups::FireGroup::default(),
            ));
            // Physical ammo weapons are explosive (not energy Charge)
            if matches!(ammo_type, AmmoType::Missile | AmmoType::Bullet | AmmoType::Mine) {
                let capped_ammo = (*ammo).min(10) as f32;
                commands.entity(entity).insert(Explosive {
                    blast_radius: 1.5,
                    blast_damage: *damage * 0.3 * capped_ammo,
                    explosive_type: ExplosiveType::Ammo,
                });
            }
        }
        CompanionData::Radar { range, noise_on_ping } => {
            commands.entity(entity).insert(Radar {
                range: *range,
                noise_on_ping: *noise_on_ping,
                is_pinging: false,
            });
        }
        CompanionData::PassiveRadar { range } => {
            commands.entity(entity).insert(Radar {
                range: *range,
                noise_on_ping: 0.0,
                is_pinging: false,
            });
        }
        CompanionData::Detection { range } => {
            commands.entity(entity).insert(DetectionSystem {
                range: *range,
                is_passive: true,
                scan_interval: 2.0,
            });
        }
        CompanionData::Light { range, intensity, attracts_creatures } => {
            commands.entity(entity).insert(ShipLight {
                range: *range,
                intensity: *intensity,
                attracts_creatures: *attracts_creatures,
            });
        }
        CompanionData::Repair { rate } => {
            commands.entity(entity).insert(RepairSystem {
                repair_rate: *rate,
                hull_repair: true,
                module_repair: true,
            });
        }
        CompanionData::Navigation { map_range } => {
            commands.entity(entity).insert(NavigationComp {
                map_range: *map_range,
                autopilot: false,
            });
        }
        CompanionData::Docking => {
            commands.entity(entity).insert(DockingComp {
                docking_speed: 1.0,
            });
        }
        CompanionData::Salvage { range, efficiency } => {
            commands.entity(entity).insert(SalvageSystem {
                range: *range,
                efficiency: *efficiency,
            });
        }
        CompanionData::Quarters { berths } => {
            commands.entity(entity).insert(Quarters {
                berths: *berths,
            });
        }
        CompanionData::CrewFacility { facility_type } => {
            commands.entity(entity).insert(CrewFacility {
                facility_type: *facility_type,
            });
        }
        CompanionData::Capacitor { capacity, charge_rate } => {
            commands.entity(entity).insert(CapacitorComp {
                capacity: *capacity,
                charge: 0.0,
                charge_rate: *charge_rate,
            });
        }
        CompanionData::PowerConduit { throughput } => {
            commands.entity(entity).insert(PowerConduitComp {
                throughput: *throughput,
            });
        }
        CompanionData::FireSuppression { effectiveness } => {
            commands.entity(entity).insert(FireSuppressionComp {
                effectiveness: *effectiveness,
                active: true,
            });
        }
        CompanionData::RadiationShielding { shielding_bonus } => {
            commands.entity(entity).insert(RadiationShieldingComp {
                shielding_bonus: *shielding_bonus,
            });
        }
        CompanionData::DroneBay { drone_count, drone_range } => {
            commands.entity(entity).insert(DroneBayComp {
                drone_count: *drone_count,
                drone_range: *drone_range,
                drones_deployed: 0,
            });
        }
        CompanionData::CoolingPump { cooling_rate } => {
            commands.entity(entity).insert(CoolingPumpComp {
                cooling_rate: *cooling_rate,
            });
        }
        CompanionData::HeatVent { dissipation_rate } => {
            commands.entity(entity).insert(HeatVentComp {
                dissipation_rate: *dissipation_rate,
            });
        }
        CompanionData::Transformer { efficiency } => {
            commands.entity(entity).insert(TransformerComp {
                efficiency: *efficiency,
            });
        }
        CompanionData::OxygenTank { capacity } => {
            commands.entity(entity).insert(OxygenTankComp {
                capacity: *capacity,
                stored: *capacity,
            });
        }
        CompanionData::AmmoAutoloader { reload_bonus } => {
            commands.entity(entity).insert(AmmoAutoloaderComp {
                reload_bonus: *reload_bonus,
            });
        }
        CompanionData::ConveyorTube { speed } => {
            commands.entity(entity).insert(ConveyorTubeComp {
                speed: *speed,
            });
        }
        CompanionData::FuelProcessor { efficiency } => {
            commands.entity(entity).insert(FuelProcessorComp {
                efficiency: *efficiency,
            });
        }
        CompanionData::HullSeal { seal_rate } => {
            commands.entity(entity).insert(HullSealComp {
                seal_rate: *seal_rate,
            });
        }
        CompanionData::Airlock { suits } => {
            commands.entity(entity).insert(AirlockComp { suits: *suits });
        }
        CompanionData::ForceField { radius, power_per_tile } => {
            commands.entity(entity).insert(ForceFieldEmitter {
                radius: *radius,
                power_per_tile: *power_per_tile,
                holding: 0,
            });
        }
        CompanionData::TargetingComputer { accuracy_bonus } => {
            commands.entity(entity).insert(TargetingComputerComp {
                accuracy_bonus: *accuracy_bonus,
            });
        }
        CompanionData::MemoryCore { autonomy } => {
            commands.entity(entity).insert(MemoryCoreComp { autonomy: *autonomy });
        }
        CompanionData::AICombatCore { priority_bonus } => {
            commands.entity(entity).insert(AICombatCoreComp {
                priority_bonus: *priority_bonus,
            });
        }
        CompanionData::ResearchLab { research_speed } => {
            commands.entity(entity).insert(ResearchLabComp {
                research_speed: *research_speed,
            });
        }
    }
}

#[cfg(test)]
mod starter_tests {
    /// The starter's launchers must have a clear lane out, same as any
    /// faction ship. Both tubes shipped buried behind two hull cells, which
    /// under the cook-off rule means the player's own ship detonates a
    /// warhead in its bow the first time they press fire.
    #[test]
    fn starter_launchers_have_a_clear_lane() {
        let design = builtin_starter_design();
        let mut blocks: Vec<(IVec2, Option<ModuleType>, Rotation)> = design
            .hull_cells
            .iter()
            .map(|h| (h.grid_pos, None, Rotation::North))
            .collect();
        blocks.extend(
            design.modules.iter().map(|m| (m.grid_pos, Some(m.module_type), m.rotation)),
        );

        let buried = crate::building::entombed_launchers(&blocks);
        assert!(
            buried.is_empty(),
            "buried launchers: {:?}",
            buried
        );
    }

    use super::*;
    use std::collections::{HashMap, HashSet};

    fn is_plate(mt: ModuleType) -> bool {
        matches!(mt, ModuleType::AngledArmorPlate | ModuleType::AngledHullPlate)
    }

    /// The player's hull is held to the same rule as every faction hull:
    /// plating outboard, never buried. The starter used to carry two plates on
    /// its own spine, which ShipGrid resolves to hull — armour that could not
    /// be hit.
    #[test]
    fn starter_plating_is_outboard_and_attached() {
        let design = builtin_starter_design();
        let hull: HashSet<IVec2> = design.hull_cells.iter().map(|c| c.grid_pos).collect();
        let plates: Vec<_> = design.modules.iter().filter(|m| is_plate(m.module_type)).collect();

        assert!(plates.len() >= 8, "the wedge should carry a real belt, got {}", plates.len());
        for p in plates {
            assert!(!hull.contains(&p.grid_pos),
                "plate at {:?} is buried in hull and would never be hit", p.grid_pos);
            let touching = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y]
                .iter().any(|d| hull.contains(&(p.grid_pos + *d)));
            assert!(touching, "plate at {:?} is floating free of the ship", p.grid_pos);
        }
    }

    /// The hull must not be mirror-symmetric across its own centreline.
    ///
    /// This is the point of the shape. The previous starter was an isosceles
    /// wedge, and six of the ten faction hulls are roughly symmetric lozenges
    /// too, so the player's ship read as one more of the same. The lean --
    /// tower forward and high, cargo keel aft and low -- is what makes it
    /// recognisable at a distance and from either side. A later edit that
    /// tidies the rows back into a mirror would undo that silently, because
    /// every other test here would still pass.
    #[test]
    fn the_hull_leans() {
        let design = builtin_starter_design();
        let mut span: HashMap<i32, (i32, i32)> = HashMap::new();
        for cell in &design.hull_cells {
            let e = span.entry(cell.grid_pos.y).or_insert((i32::MAX, i32::MIN));
            e.0 = e.0.min(cell.grid_pos.x);
            e.1 = e.1.max(cell.grid_pos.x);
        }
        let mirrored = span
            .iter()
            .all(|(y, extent)| span.get(&-y).is_none_or(|other| other == extent));
        assert!(
            !mirrored,
            "every row matches its mirror across y=0, so the hull is a symmetric \
             wedge again and reads like every other ship in the game"
        );

        // And the lean must go the way the layout assumes: the tower reaches
        // higher than the keel drops, because the bridge is up there.
        let top = span.keys().max().copied().unwrap();
        let bottom = span.keys().min().copied().unwrap();
        assert!(
            top > -bottom,
            "the dorsal tower (y={top}) no longer out-reaches the ventral keel (y={bottom})"
        );
    }

    /// Both transverse doors must be on the main corridor, with deck on each
    /// side.
    ///
    /// A door with a wall behind it is a decoration. These two are the only
    /// thing dividing the ship into compartments, so if a module ever lands on
    /// the cell beside one, the division quietly stops existing.
    #[test]
    fn the_corridor_doors_divide_something() {
        let design = builtin_starter_design();
        let deck: HashMap<IVec2, HullLayer> = design
            .hull_cells
            .iter()
            .map(|c| (c.grid_pos, c.layer))
            .collect();
        let occupied: HashSet<IVec2> = design.modules.iter().map(|m| m.grid_pos).collect();

        let doors: Vec<IVec2> = design
            .hull_cells
            .iter()
            .filter(|c| c.layer == HullLayer::BulkheadDoor)
            .map(|c| c.grid_pos)
            .collect();
        assert_eq!(doors.len(), 2, "expected two transverse doors, got {}", doors.len());

        for door in doors {
            for side in [IVec2::X, IVec2::NEG_X] {
                let neighbour = door + side;
                let walkable = matches!(
                    deck.get(&neighbour),
                    Some(HullLayer::Hallway) | Some(HullLayer::BulkheadDoor)
                ) && !occupied.contains(&neighbour);
                assert!(
                    walkable,
                    "the door at {door:?} has no deck at {neighbour:?}, so it divides nothing"
                );
            }
        }
    }

    /// Nothing may double-book a cell — the derived plating is filtered against
    /// the hand-placed modules, and this is what proves the filter works.
    #[test]
    fn starter_has_no_overlapping_modules() {
        let design = builtin_starter_design();
        let mut seen = HashSet::new();
        for m in &design.modules {
            assert!(seen.insert(m.grid_pos),
                "two modules both at {:?}", m.grid_pos);
        }
    }
}

#[cfg(test)]
mod spawn_activation_tests {
    use super::*;

    /// Modules start inactive unless whitelisted, and anything a player places
    /// expecting it to just work has to be on that list. An airlock is one:
    /// `crew::burial` skips inactive locks, so an off-by-default airlock means
    /// the dead pile up on the deck with no explanation anywhere.
    ///
    /// This is the third time a hidden power toggle has read as a broken
    /// feature (guns, then radar, now this), which is why it is a test.
    #[test]
    fn airlocks_spawn_active() {
        for module_type in [ModuleType::AirlockChamber, ModuleType::DockingPort] {
            assert!(
                starts_active(module_type),
                "{module_type:?} spawns inactive - burial at space will silently never happen"
            );
        }
    }
}

#[cfg(test)]
mod starter_core_tests {
    use super::*;

    /// The built-in starter and the shipped JSON must agree about Memory
    /// Cores.
    ///
    /// designs/starter.json normally wins; the built-in is the fallback when
    /// the file is missing or has been regenerated. If only the JSON carries
    /// cores, a regenerated starter silently ships with none — switching off
    /// both the autonomy mechanic and the last-core death condition, with
    /// nothing reporting that anything is wrong. That is the same trap the
    /// faction designs had.
    #[test]
    fn the_builtin_starter_has_memory_cores() {
        let design = builtin_starter_design();
        let cores = design
            .modules
            .iter()
            .filter(|m| m.module_type == ModuleType::MemoryCore)
            .count();
        assert!(
            cores > 0,
            "the built-in starter has no Memory Cores, so a regenerated \
             starter.json would disable the core death condition silently"
        );

        // And they must not all sit in one place, or one hit ends the run.
        if cores > 1 {
            let positions: Vec<IVec2> = design
                .modules
                .iter()
                .filter(|m| m.module_type == ModuleType::MemoryCore)
                .map(|m| m.grid_pos)
                .collect();
            let spread = positions
                .iter()
                .flat_map(|a| positions.iter().map(move |b| (*a - *b).abs().max_element()))
                .max()
                .unwrap_or(0);
            assert!(spread >= 3, "cores are clustered ({spread} cells apart at most)");
        }
    }

    /// If the shipped design exists, it must agree with the built-in about
    /// whether this ship has a mind.
    #[test]
    fn the_shipped_starter_agrees() {
        let Some(shipped) = crate::building::blueprint::load_design_file("designs/starter.json")
        else {
            return; // not present in this checkout; the built-in test covers it
        };
        let shipped_cores = shipped
            .modules
            .iter()
            .filter(|m| m.module_type == ModuleType::MemoryCore)
            .count();
        assert!(
            shipped_cores > 0,
            "designs/starter.json has no Memory Cores but the built-in does — \
             the ship the player actually flies would have no mind"
        );
    }
}
