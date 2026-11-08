use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};
use smallvec::SmallVec;
use crate::states::{GameState, BuildState};
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::ship::spawn_module;
use crate::sprite_map;

pub mod customization;
pub mod armour;
pub mod footprints;
pub mod inspection;
pub mod multiblock;
pub mod build_history;
pub mod symmetry;
pub mod build_info;
pub mod clipboard;
pub mod templates;
pub mod template_ghost;

pub mod rooms;
pub mod layout;
pub mod registry;
pub mod stat_calculator;
pub mod blueprint;
pub mod turret_build;
pub mod gun_stamp;

pub use registry::ModuleRegistry;
pub use stat_calculator::StatCalculator;
pub use blueprint::BlueprintResource;

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(layout::LayoutPlugin)
            .init_state::<BuildState>()
            .init_resource::<BuildingState>()
            .init_resource::<rooms::RoomMap>()
            .init_resource::<GridOccupancy>()
            .init_resource::<BlueprintResource>()
            .init_resource::<build_history::BuildHistory>()
            .init_resource::<symmetry::SymmetryState>()
            .init_resource::<build_info::ModuleSearchState>()
            .init_resource::<clipboard::BuildClipboard>()
            .init_resource::<templates::TemplateState>()
            .insert_resource(registry::build_registry())
            .insert_resource({
                let mut reg = customization::parameters::CustomizationRegistry::default();
                customization::weapons::register_weapon_customizations(&mut reg);
                reg
            })
            .init_resource::<customization::custom_presets::CustomPresetLibrary>()
            .add_systems(Startup, customization::custom_presets::load_custom_presets)
            // Weapon tuning → live stats. Runs in every state (Changed-filtered,
            // so it's a no-op unless a slider moved or a save just loaded) —
            // tuned stats must persist into flight, only EDITING is dock-gated.
            .add_systems(Update, customization::tuning::apply_weapon_tuning)
            .add_systems(
                Update,
                (
                    update_grid_occupancy,
                    handle_build_input,
                    turret_build::switch_build_deck,
                    update_ghost_preview,
                    turret_build::validate_turret_ghost,
                    handle_module_placement,
                    draw_hull_rect_preview,
                    handle_module_removal,
                    turret_build::turret_deck_clicks,
                    turret_build::show_turrets_for_deck,
                    blueprint::save_blueprint_system,
                    blueprint::load_blueprint_system,
                    blueprint::delete_blueprint_system,
                    // Weapon customization (right-click inspect/Tier2/Tier3)
                    // shelved for now — revisit once most stations/bases are
                    // done. inspection::right_click_inspect and
                    // handle_customize_click still exist, just not wired in.
                )
                    .chain()
                    .run_if(in_state(GameState::StationDocked)),
            )
            // Placement/removal event processors also run while EXPLORING —
            // the ghost-rebuild system (ship::rebuild) respawns destroyed
            // blocks in flight via the same PlaceHullRequest /
            // PlaceModuleRequest events the build mode uses.
            .add_systems(
                Update,
                (
                    process_hull_placement,
                    process_module_placement,
                    process_module_removal,
                    process_hull_removal,
                    // Last in the chain so the grids reflect this frame's
                    // placements and removals, not the previous frame's.
                    update_ship_grids,
                )
                    .chain()
                    .run_if(in_state(GameState::StationDocked)
                        .or_else(in_state(GameState::Exploring))),
            )
            // Room detection runs in both surface and exploring
            .add_systems(
                Update,
                (
                    rooms::update_room_map,
                    rooms::update_room_power,
                ).run_if(in_state(GameState::StationDocked)
                    .or_else(in_state(GameState::Exploring))),
            )
            // Custom module stat recalculation + weapon sync (runs in all states)
            .add_systems(
                Update,
                (recalculate_custom_module_stats, sync_calculated_to_weapon).chain(),
            )
            // Multi-block machine systems (connection detection, stat calc, damage chain)
            .add_systems(
                Update,
                (
                    multiblock::connections::rebuild_machine_connections,
                    multiblock::connections::calculate_barrel_stress
                        .after(multiblock::connections::rebuild_machine_connections),
                    multiblock::stats::calculate_machine_stats
                        .after(multiblock::connections::rebuild_machine_connections),
                    multiblock::stats::apply_machine_stats_to_weapons
                        .after(multiblock::stats::calculate_machine_stats),
                    // Must run after apply_machine_stats_to_weapons or the two
                    // systems race to write Weapon.damage/range/fire_rate with
                    // no defined order — see apply_weapon_enhancers' doc comment.
                    // Runs while DOCKED too (not just Exploring) so the tuning
                    // window's live stat readout reflects the composed result,
                    // and folds WeaponTuning in during its per-frame reset.
                    multiblock::enhancers::apply_weapon_enhancers
                        .after(multiblock::stats::apply_machine_stats_to_weapons),
                    // Cooldown duration follows the final composed fire_rate.
                    customization::tuning::sync_weapon_cooldowns
                        .after(multiblock::enhancers::apply_weapon_enhancers),
                    multiblock::damage::process_block_destruction,
                ).run_if(in_state(GameState::StationDocked)
                    .or_else(in_state(GameState::Exploring))),
            )
            // Enhancer effects (separate system group to stay under tuple limit)
            .add_systems(
                Update,
                (
                    multiblock::enhancers::apply_hull_enhancers,
                    // Must run after update_ship_state recomputes noise_level
                    // from scratch each frame, or the SignalJammer reduction
                    // below gets silently overwritten depending on schedule
                    // order (both run under GameState::Exploring).
                    multiblock::enhancers::apply_utility_enhancers
                        .after(crate::ship::update_ship_state),
                    multiblock::enhancers::emergency_shutdown_system,
                    multiblock::enhancers::afterburner_system,
                ).run_if(in_state(GameState::Exploring)),
            )
            // Build mode tools (undo, symmetry, overlays, info)
            .add_systems(
                Update,
                (
                    build_history::undo_input,
                    symmetry::toggle_symmetry,
                    build_info::toggle_cost_summary,
                    build_info::toggle_power_overlay,
                    build_info::toggle_heat_overlay,
                    clipboard::clipboard_input,
                    clipboard::clipboard_paste,
                    clipboard::paste_ghost_preview,
                    templates::template_input,
                    template_ghost::update_template_ghost,
                    gun_stamp::sync_kit_count,
                    gun_stamp::stamp_gun_design,
                    gun_stamp::gun_stamp_ghost,
                    template_ghost::chain_delete_system,
                ).run_if(in_state(GameState::StationDocked)),
            );
    }
}

// ============================================================================
// GRID OCCUPANCY - tracks which cells are taken
// ============================================================================

#[derive(Resource, Default)]
pub struct GridOccupancy {
    pub cells: HashMap<IVec2, Entity>,
}

/// Per-ship map from ship-local grid cell to the live block sitting in it.
///
/// Grid coordinates are ship-local, so a single global map can only ever
/// describe ONE ship — an AI ship's (1,0) and the player's (1,0) are
/// different blocks that collide on the same key. That restriction is why
/// `GridOccupancy` is player-scoped, and why several systems exist twice
/// (`queue_detonation`/`queue_ai_detonation`,
/// `trigger_chain_reactions`/`ai_chain_reactions`): the AI copy had to
/// re-derive in world space what the player copy reads off the grid.
///
/// Two deliberate differences from `GridOccupancy`:
/// - **Live blocks only.** Destroyed modules and hull are dropped, so a
///   dead plate doesn't keep stopping shells.
/// - **Maintained in flight,** not just at dock. Blocks get shot off during
///   combat, and hit resolution walks this map while that's happening.
#[derive(Component, Default)]
pub struct ShipGrid {
    pub cells: HashMap<IVec2, Entity>,
}

/// A turret's gun house roof, as a grid of its own over the turret's
/// ShipGrid (combat::gun_house): a round walks this one first.
#[derive(Component, Default)]
pub struct RoofGrid(pub ShipGrid);

/// Per-block hit-resolution data, ADDITIVE alongside Module/HullSegment
/// (same pattern as ShipGrid). `ShipGrid` answers "which block is at this
/// cell"; `Block` answers "what does a round meet when it gets there".
/// Health deliberately stays on Module/HullSegment — two sources of truth
/// is worse than one awkward query.
///
/// `thickness` is in the same units as ammo penetration (0-90): hull seeds
/// from HullMaterial::damage_absorption() (15/30/50/80), modules are
/// unarmoured internals at 0. `slope` (0 = flat plate) is reserved for
/// angled armour — nothing reads it yet.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub cell: IVec2,
    pub kind: BlockKind,
    pub thickness: f32,
    /// Outward normal of the block's armoured face, in ship-local radians.
    ///
    /// `None` means the block is a plain box: it has no face of its own, so
    /// hit resolution uses whichever cell face the round arrived through.
    /// That is the overwhelming majority of blocks, and it's what makes an
    /// unsloped hull behave exactly as it did before slope existed.
    ///
    /// `Some(angle)` is a block whose SHAPE declares a facing — a wedge
    /// presents the same diagonal slab whichever side you approach from, so
    /// the normal is fixed in the hull's frame rather than derived per-face.
    pub facing: Option<f32>,
    /// Where the cut runs, for a shaped block: its material is the part of
    /// the cell with `p . facing <= cut` (p from the cell's centre, in
    /// cells). 0 cuts through the centre -- a wedge, a half plate; more
    /// keeps more of the cell (a bevel only chips the corner); less keeps
    /// less (the low half of a long slope).
    pub cut: f32,
    /// What a laser's intensity is measured against. The same as `thickness`
    /// on everything but reflective plate.
    pub laser_thickness: f32,
    /// Ablative: a round that hits this goes no further, however big it was.
    pub stops_round: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Hull,
    Module,
}

impl Block {
    pub fn hull(cell: IVec2, material: HullMaterial) -> Self {
        let thickness = material.damage_absorption();
        Self { cell, kind: BlockKind::Hull, thickness, facing: None, cut: 0.0, laser_thickness: thickness, stops_round: false }
    }

    /// A module with no armour value — machinery, which is what most modules
    /// are. Also the fallback when a struck block has no Block component.
    pub fn module(cell: IVec2) -> Self {
        Self { cell, kind: BlockKind::Module, thickness: 0.0, facing: None, cut: 0.0, laser_thickness: 0.0, stops_round: false }
    }

    /// A module as actually spawned: plating modules carry real armour and
    /// wedges carry a declared diagonal face.
    pub fn for_module(cell: IVec2, module_type: ModuleType, rotation: Rotation) -> Self {
        let thickness = module_thickness(module_type);
        Self {
            cell,
            kind: BlockKind::Module,
            thickness,
            facing: module_facing(module_type, rotation),
            cut: module_shape(module_type).map_or(0.0, |(_, cut)| cut),
            laser_thickness: match module_type {
                ModuleType::ReflectivePlate | ModuleType::AngledReflectivePlate => REFLECTIVE_LASER_THICKNESS,
                _ => thickness,
            },
            stops_round: matches!(module_type, ModuleType::AblativeArmor | ModuleType::AngledAblativeArmor),
        }
    }
}

/// What reflective plate counts as against a laser: a plain 60-intensity beam
/// burns in at 40%, and four doublers (intensity 120) at 80%.
const REFLECTIVE_LASER_THICKNESS: f32 = 150.0;

/// Armour rating of a module, in the same units as
/// `HullMaterial::damage_absorption` (Steel 15 … Abyssal Alloy 80).
///
/// Almost every module is machinery and armours nothing — an exposed reactor
/// should eat the round. The plating modules are the exception, and they're
/// deliberately rated BELOW their raw HP suggests: a wedge trades material for
/// angle, so it deflects better than a flat plate and stops less.
fn module_thickness(module_type: ModuleType) -> f32 {
    match module_type {
        ModuleType::StaggeredArmorPlate => 45.0, // premium, 6 cells, no straight seam
        ModuleType::ArmorPlate | ModuleType::CornerArmorPlate => 40.0,
        // Thin on purpose: even a 20 mm round bites these. See `stops_round`
        // and `laser_thickness` for what they're good at instead.
        ModuleType::AblativeArmor | ModuleType::AngledAblativeArmor => 5.0,
        ModuleType::ReflectivePlate | ModuleType::AngledReflectivePlate => 8.0,
        ModuleType::AngledArmorPlate => 30.0,    // less steel than a flat plate...
        ModuleType::BevelledArmorPlate => 38.0,  // a plate with its corner chipped
        ModuleType::SlopeArmorPlateLow | ModuleType::SlopeArmorPlateHigh
        | ModuleType::SlopeArmorPlateLowMirror | ModuleType::SlopeArmorPlateHighMirror => 32.0,
        ModuleType::HalfArmorPlate => 22.0,      // half the steel of a plate
        ModuleType::HullReinforcePlate => 30.0,
        ModuleType::AngledHullPlate => 15.0,     // ...and this one is only framing
        ModuleType::BallastBlock => 25.0,        // dense fill, no armour face
        _ => 0.0,
    }
}

/// Fixed outward normal for blocks whose shape declares one.
///
/// The wedge sprites (`vfx::block_visuals`) put their material along two
/// adjacent edges and expose the diagonal between them, so at `North` the
/// face looks out to the north-east — 45°. Each rotation carries it round to
/// the next corner, which is what `R` in build mode has always been picking;
/// it just didn't mean anything until now.
fn module_facing(module_type: ModuleType, rotation: Rotation) -> Option<f32> {
    module_shape(module_type).map(|(angle, _)| angle + rotation.to_radians())
}

/// A shaped block's cut, facing North: the outward normal of its cut face
/// (radians) and where the cut runs (`Block::cut`). Every angle piece is a
/// cell cut by one straight line.
pub fn module_shape(module_type: ModuleType) -> Option<(f32, f32)> {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
    // A slope rising one cell over two: the low half and the high half meet
    // at mid-height on the shared edge.
    let shallow = (1.0f32).atan2(-0.5);
    let shallow_mirror = (1.0f32).atan2(0.5);
    let half_step = 0.25 / (1.25f32).sqrt();
    Some(match module_type {
        ModuleType::AngledArmorPlate | ModuleType::AngledHullPlate
        | ModuleType::AngledAblativeArmor | ModuleType::AngledReflectivePlate => (FRAC_PI_4, 0.0),
        // Legs of 0.4 cell chipped off the corner.
        ModuleType::BevelledArmorPlate => (FRAC_PI_4, 0.6 / std::f32::consts::SQRT_2),
        ModuleType::SlopeArmorPlateLow => (shallow, -half_step),
        ModuleType::SlopeArmorPlateHigh => (shallow, half_step),
        ModuleType::SlopeArmorPlateLowMirror => (shallow_mirror, -half_step),
        ModuleType::SlopeArmorPlateHighMirror => (shallow_mirror, half_step),
        ModuleType::HalfArmorPlate => (FRAC_PI_2, 0.0),
        _ => return None,
    })
}

/// What a cell does to a missile trying to leave its tube.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SiloCell {
    /// Nothing there.
    Empty,
    /// Armour plating — a blow-out panel, not a bulkhead.
    Panel,
    /// Hull, machinery, anything a warhead is not getting through.
    Solid,
}

/// Plates a missile is allowed to punch out through on its way off the hull.
///
/// Without this every bow tube on every ship would read as blocked: the
/// armour `belt` deliberately places plating OUTBOARD of the hull, so a
/// launcher sitting on the outermost hull cell has its own skin in front of
/// it by construction.
pub fn is_blowout_panel(module_type: ModuleType) -> bool {
    matches!(
        module_type,
        ModuleType::AngledHullPlate
            | ModuleType::AngledArmorPlate
            | ModuleType::BevelledArmorPlate
            | ModuleType::SlopeArmorPlateLow
            | ModuleType::SlopeArmorPlateHigh
            | ModuleType::SlopeArmorPlateLowMirror
            | ModuleType::SlopeArmorPlateHighMirror
            | ModuleType::HalfArmorPlate
            | ModuleType::ArmorPlate
            | ModuleType::CornerArmorPlate
            | ModuleType::StaggeredArmorPlate
            | ModuleType::AblativeArmor
            | ModuleType::AngledAblativeArmor
            | ModuleType::ReflectivePlate
            | ModuleType::AngledReflectivePlate
    )
}

/// Whether a module is a missile launcher, and so subject to the silo rule.
/// A VLS Silo isn't: it launches up out of the deck, so nothing beside it
/// is in the way.
pub fn is_launcher(module_type: ModuleType) -> bool {
    matches!(
        module_type,
        ModuleType::HeavyMissile | ModuleType::GuidedMissile | ModuleType::ClusterRocket
            | ModuleType::LaunchPad | ModuleType::RailLauncher
    )
}

/// How many blow-out panels a missile will punch through before the tube
/// counts as buried. One is a skin; two is a wall someone built in front of
/// their own silo.
const MAX_PANEL_CELLS: usize = 1;

/// Cells to walk before calling an exit path clear. Longer than the widest
/// hull, so leaving the ship entirely is what ends the walk. It was tied to
/// MAX_WALK_STEPS (24, a per-frame projectile bound that has nothing to do
/// with hull size), so a silo deep in a capital more than 24 cells from its
/// own bow was declared clear and fired through its own plating.
pub const SILO_REACH: i32 = 256;

/// First block sealing a launcher's exit path, if any.
///
/// `cell_kind` is the caller's grid: `GridOccupancy` in build mode, the live
/// `ShipGrid` in flight — where destroyed blocks have already dropped out, so
/// shooting the tile off your own bow un-blocks the tube mid-fight.
///
/// `dir` must come from `Rotation::facing_offset`, which is the direction the
/// missile is actually ejected along.
pub fn silo_obstruction(
    muzzle: IVec2,
    dir: IVec2,
    cell_kind: impl Fn(IVec2) -> SiloCell,
) -> Option<IVec2> {
    let mut panels = 0;
    for step in 1..=SILO_REACH {
        let cell = muzzle + dir * step;
        match cell_kind(cell) {
            SiloCell::Empty => {}
            SiloCell::Panel => {
                panels += 1;
                if panels > MAX_PANEL_CELLS {
                    return Some(cell);
                }
            }
            SiloCell::Solid => return Some(cell),
        }
    }
    None
}

/// Launchers in a flat block list whose exit path is sealed.
///
/// Takes `(cell, module type or None for hull, rotation)` so a design can be
/// checked straight from blueprint or layout data, with no ECS. Shared by the
/// faction-layout and starter tests so both judge by exactly the rule the
/// game enforces at the trigger.
pub fn entombed_launchers(
    blocks: &[(IVec2, Option<ModuleType>, Rotation)],
) -> Vec<(IVec2, ModuleType, IVec2)> {
    // Modules win their cell over hull, matching how ShipGrid is built.
    let mut occupied: HashMap<IVec2, Option<ModuleType>> = HashMap::new();
    for (cell, mt, _) in blocks {
        let slot = occupied.entry(*cell).or_insert(*mt);
        if mt.is_some() {
            *slot = *mt;
        }
    }

    let mut out = Vec::new();
    for (cell, mt, rotation) in blocks {
        let Some(mt) = *mt else { continue };
        if !is_launcher(mt) {
            continue;
        }
        let blocked = silo_obstruction(*cell, rotation.facing_offset(), |c| {
            match occupied.get(&c) {
                None => SiloCell::Empty,
                Some(Some(m)) if is_blowout_panel(*m) => SiloCell::Panel,
                Some(_) => SiloCell::Solid,
            }
        });
        if let Some(blocker) = blocked {
            out.push((*cell, mt, blocker));
        }
    }
    out
}

/// Ship-local cell coordinates of a world point, given the ship's transform.
///
/// Blocks spawn at `grid * 66.0` with an asymmetric -33.0 y offset, which
/// several combat systems re-type inline and which is easy to drop.
pub fn world_to_cell(ship: &GlobalTransform, world: Vec2) -> Vec2 {
    let p = ship.affine().inverse().transform_point3(world.extend(0.0)).truncate();
    Vec2::new(p.x / 66.0, (p.y + 33.0) / 66.0)
}

/// One occupied cell met by `ShipGrid::walk`, in path order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridStep {
    pub entity: Entity,
    pub cell: IVec2,
    /// Distance along the walked segment at which the round entered this cell,
    /// in cell widths. With `span`, this reconstructs the entry and exit
    /// points — which is what a sub-cell shape test needs to clip against.
    pub t_enter: f32,
    /// Which side of the cell the ray came in through, as the unit offset
    /// from this cell toward the cell it arrived from (`(-1, 0)` = entered
    /// through the -x face). `(0, 0)` for the cell the walk started inside.
    pub entry_face: IVec2,
    /// Path length through the cell in cell widths — the line-of-sight
    /// thickness. 1.0 for a straight pass, sqrt(2) corner to corner. This is
    /// the number armour math should use directly; don't rebuild it as
    /// T / cos(theta).
    pub span: f32,
}

/// Bound on cells visited per walk. Exhaustion is a miss: a degenerate
/// direction must not loop, and nothing the game fires crosses 24 cells
/// (1,584 units) between two frames.
pub const MAX_WALK_STEPS: usize = 24;

/// Width of one grid cell in world units.
pub const GRID_SIZE: f32 = 66.0;

/// Ship-local centre of a grid cell.
///
/// The `-33.0` is a half-cell Y offset that every block spawner applies (see
/// `spawn_module`, `process_hull_placement`, `rooms::transform_to_grid`); it
/// is asymmetric and easy to drop, which is why it lives here rather than
/// being retyped. `local_to_grid` is its inverse.
pub fn grid_to_local(cell: IVec2) -> Vec2 {
    Vec2::new(cell.x as f32 * GRID_SIZE, cell.y as f32 * GRID_SIZE - 33.0)
}

/// Grid cell containing a ship-LOCAL point. World-space callers must undo the
/// ship's own transform first — see `cursor_to_ship_grid`.
pub fn local_to_grid(local: Vec2) -> IVec2 {
    IVec2::new(
        (local.x / GRID_SIZE).round() as i32,
        ((local.y + 33.0) / GRID_SIZE).round() as i32,
    )
}

impl ShipGrid {
    /// Swept walk from `from` to `to` (ship-local CELL coordinates — block
    /// centres sit on integers, cell (gx, gy) spans gx±0.5) yielding every
    /// live block the segment passes through, nearest first.
    ///
    /// Amanatides-Woo DDA, not Bresenham: Bresenham visits one cell per
    /// column and skips the diagonal corner cell, so a shell at the right
    /// angle would pass through solid armour. Consecutive steps through the
    /// same entity (multi-cell modules) are merged into one step whose span
    /// is the total path through that block.
    pub fn walk(&self, from: Vec2, to: Vec2) -> SmallVec<[GridStep; 8]> {
        let mut out = SmallVec::new();
        let delta = to - from;
        let len = delta.length();
        if !len.is_finite() {
            return out;
        }
        // Degenerate segment: a point test of the cell we're standing in.
        if len < 1e-4 {
            let cell = IVec2::new(from.x.round() as i32, from.y.round() as i32);
            if let Some(entity) = self.get(cell) {
                out.push(GridStep { entity, cell, entry_face: IVec2::ZERO, t_enter: 0.0, span: 0.0 });
            }
            return out;
        }
        let dir = delta / len;

        // Shift by half a cell so boundaries sit on integers.
        let p = from + Vec2::splat(0.5);
        let mut cell = IVec2::new(p.x.floor() as i32, p.y.floor() as i32);
        let step = IVec2::new(dir.x.signum() as i32 * (dir.x != 0.0) as i32,
                              dir.y.signum() as i32 * (dir.y != 0.0) as i32);
        let axis_t = |d: f32, pos: f32, c: i32, s: i32| -> (f32, f32) {
            if s == 0 {
                (f32::INFINITY, f32::INFINITY)
            } else {
                let boundary = if s > 0 { c as f32 + 1.0 } else { c as f32 };
                ((boundary - pos) / d, 1.0 / d.abs())
            }
        };
        let (mut t_max_x, t_delta_x) = axis_t(dir.x, p.x, cell.x, step.x);
        let (mut t_max_y, t_delta_y) = axis_t(dir.y, p.y, cell.y, step.y);

        let mut t_enter = 0.0_f32;
        let mut entry_face = IVec2::ZERO;
        for _ in 0..MAX_WALK_STEPS {
            let t_exit = t_max_x.min(t_max_y).min(len);
            let span = t_exit - t_enter;
            // A zero-span cell is a corner graze, not a pass-through.
            if span > 1e-6 {
                if let Some(entity) = self.get(cell) {
                    match out.last_mut() {
                        Some(last) if last.entity == entity => last.span += span,
                        _ => out.push(GridStep { entity, cell, entry_face, t_enter, span }),
                    }
                }
            }
            if t_exit >= len {
                break;
            }
            if t_max_x < t_max_y {
                cell.x += step.x;
                t_enter = t_max_x;
                t_max_x += t_delta_x;
                entry_face = IVec2::new(-step.x, 0);
            } else {
                cell.y += step.y;
                t_enter = t_max_y;
                t_max_y += t_delta_y;
                entry_face = IVec2::new(0, -step.y);
            }
        }
        out
    }

    /// Live block occupying a cell, if any.
    pub fn get(&self, cell: IVec2) -> Option<Entity> {
        self.cells.get(&cell).copied()
    }

    /// Whether any live block occupies a cell.
    pub fn contains(&self, cell: IVec2) -> bool {
        self.cells.contains_key(&cell)
    }

    /// Get all grid cells a module occupies given origin, size, and rotation.
    /// Uses SmallVec to avoid heap allocation for modules up to 2x2.
    ///
    /// `footprint` overrides the plain WxH rectangle with an explicit set of
    /// relative offsets (see `footprints::footprint_override`) for modules
    /// with a non-rectangular shape. `None` reproduces the original
    /// rectangle behavior exactly.
    pub fn cells_for(origin: IVec2, size: IVec2, rotation: Rotation, footprint: Option<&[IVec2]>) -> SmallVec<[IVec2; 4]> {
        let mut cells = SmallVec::new();
        match footprint {
            Some(offsets) => {
                for &offset in offsets {
                    cells.push(origin + rotation.rotate_offset(offset));
                }
            }
            None => {
                for x in 0..size.x {
                    for y in 0..size.y {
                        let offset = rotation.rotate_offset(IVec2::new(x, y));
                        cells.push(origin + offset);
                    }
                }
            }
        }
        cells
    }
}

impl GridOccupancy {
    /// Cells a module occupies. Same footprint maths as `ShipGrid::cells_for`
    /// — kept here so the ~8 static callers still compile while the grid
    /// migrates to the per-ship component.
    pub fn cells_for(origin: IVec2, size: IVec2, rotation: Rotation, footprint: Option<&[IVec2]>) -> SmallVec<[IVec2; 4]> {
        ShipGrid::cells_for(origin, size, rotation, footprint)
    }

    /// Check if all cells for a module placement are free
    pub fn can_place(&self, origin: IVec2, size: IVec2, rotation: Rotation, footprint: Option<&[IVec2]>) -> bool {
        for cell in Self::cells_for(origin, size, rotation, footprint) {
            if self.cells.contains_key(&cell) {
                return false;
            }
        }
        true
    }
}

/// Rebuilds grid occupancy from the PLAYER ship's modules and hull segments.
/// Skips rebuild when entity count hasn't changed (cheap change detection).
///
/// Scoped to the player ship only: AI ships reuse the same local grid
/// coordinates (their modules also sit at positions like (1,0)), so mixing
/// them into one global map made an AI ship's explosion at *its* (1,0)
/// damage the player's module at (1,0). Grid coordinates are only
/// meaningful per-ship.
fn update_grid_occupancy(
    module_query: Query<(Entity, &Module), Or<(Changed<Module>, Added<Module>)>>,
    hull_query: Query<(Entity, &HullSegment, &Transform), Or<(Changed<HullSegment>, Added<HullSegment>)>>,
    all_modules: Query<(Entity, &Module, &ChildOf)>,
    all_hulls: Query<(Entity, &HullSegment, &Transform, &ChildOf)>,
    ship_query: Query<Entity, With<Ship>>,
    mut occupancy: ResMut<GridOccupancy>,
    mut last_count: Local<usize>,
) {
    let Ok(player_ship) = ship_query.single() else { return };

    let current_count = all_modules.iter().count() + all_hulls.iter().count();
    let has_changes = !module_query.is_empty() || !hull_query.is_empty();
    if current_count == *last_count && !occupancy.cells.is_empty() && !has_changes {
        return;
    }
    *last_count = current_count;

    occupancy.cells.clear();

    for (entity, module, parent) in all_modules.iter() {
        if parent.parent() != player_ship { continue; }
        let footprint = footprints::footprint_override(module.module_type);
        let cells = GridOccupancy::cells_for(module.grid_position, module.size, module.rotation, footprint);
        for cell in cells {
            occupancy.cells.insert(cell, entity);
        }
    }

    for (entity, _hull, transform, parent) in all_hulls.iter() {
        if parent.parent() != player_ship { continue; }
        let grid = rooms::transform_to_grid(transform);
        occupancy.cells.insert(grid, entity);
    }
}

/// Rebuilds every ship's `ShipGrid` from its own live blocks.
///
/// Runs in flight as well as at dock, unlike `update_grid_occupancy`: hit
/// resolution walks these maps while blocks are being shot off, and a grid
/// that only refreshed at a station would send shells into cells whose
/// block died two fights ago.
///
/// Rebuilds only the ships whose `layout::LayoutVersion` moved. It used to
/// recount every block in the world each frame and rebuild EVERY ship when a
/// count changed -- and in a fight some block dies nearly every frame, so a
/// 2,000-block capital rebuilt its whole grid because a skiff lost a plate.
pub fn update_ship_grids(
    mut commands: Commands,
    // Turret bodies too: each turret deck is a little grid of its own.
    mut ships: Query<
        (Entity, &layout::LayoutVersion, Option<&Children>, Option<&mut ShipGrid>, Has<crate::combat::turret_deck::TurretBody>),
        Or<(With<Ship>, With<crate::ai_ship::components::AiShip>, With<crate::combat::turret_deck::TurretBody>)>,
    >,
    modules: Query<&Module, Without<DestroyedModule>>,
    hulls: Query<(&Transform, &HullSegment), Without<HullDestroyed>>,
    epoch: Res<layout::LayoutEpoch>,
    mut seen_epoch: Local<Option<layout::LayoutEpoch>>,
    mut built: Local<layout::BuiltFrom>,
) {
    // Nothing anywhere changed since the last pass: every grid is current.
    if *seen_epoch == Some(*epoch) {
        return;
    }
    *seen_epoch = Some(*epoch);

    for (ship, version, children, grid, is_turret) in ships.iter_mut() {
        if grid.is_some() && !built.is_stale(ship, version) {
            continue;
        }
        built.mark(ship, version);

        // A ship that just lost its last block still gets an empty grid
        // written rather than keeping a stale one.
        let mut cells: HashMap<IVec2, Entity> = HashMap::new();
        // A turret's gun house roof is a level of its own over its parts
        // (combat::gun_house): its own grid, which a round meets first.
        let mut roof: HashMap<IVec2, Entity> = HashMap::new();
        let children = children.map(|c| c.iter().collect::<Vec<_>>()).unwrap_or_default();
        for &child in &children {
            if let Ok(module) = modules.get(child) {
                let footprint = footprints::footprint_override(module.module_type);
                let level = if is_turret && crate::combat::gun_house::is_roof_block(module.module_type) { &mut roof } else { &mut cells };
                for cell in ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint) {
                    level.insert(cell, child);
                }
            }
        }
        if is_turret {
            commands.entity(ship).try_insert(RoofGrid(ShipGrid { cells: roof }));
        }
        // Hull is always 1x1, and spawns at `grid * 66.0` (y offset -33.0) on
        // player and AI ships alike, so the transform inverts cleanly. Modules
        // can't use this path: multi-cell ones are centred on their footprint
        // centroid, not on their origin cell. Hull goes in second and takes
        // a shared cell, as it always has.
        for &child in &children {
            if let Ok((transform, hull)) = hulls.get(child) {
                // A turret's plating sits on the turret's own grid, centred
                // on (0, 0) with no half-cell offset; its segments know
                // their cell.
                let cell = if is_turret { hull.grid_position } else { rooms::transform_to_grid(transform) };
                cells.insert(cell, child);
            }
        }

        match grid {
            Some(mut grid) => grid.cells = cells,
            None => { commands.entity(ship).try_insert(ShipGrid { cells }); }
        }
    }
    built.retain(|e| ships.contains(e));
}

// ============================================================================
// BUILD INPUT
// ============================================================================

/// Checks if a hull material is unlocked
fn is_hull_material_unlocked(material: HullMaterial, unlocks: &crate::resources::Unlocks) -> bool {
    match material {
        HullMaterial::Steel => true,
        HullMaterial::Titanium => unlocks.hull_types.contains(&"titanium".to_string()),
        HullMaterial::Composite => unlocks.hull_types.contains(&"composite".to_string()),
        HullMaterial::AbyssalAlloy => unlocks.hull_types.contains(&"abyssal_alloy".to_string()),
    }
}

/// Handles building mode input
fn handle_build_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut build_state: ResMut<BuildingState>,
    mut next_state: ResMut<NextState<BuildState>>,
    current_state: Res<State<BuildState>>,
    mut customization_state: ResMut<CustomizationState>,
    mut placement_state: ResMut<ComponentPlacementState>,
    registry: Res<ModuleRegistry>,
    unlocks: Res<crate::resources::Unlocks>,
    mut notifications: MessageWriter<ShowNotification>,
    standing: Res<crate::diplomacy::FactionStanding>,
) {
    // V: switch the selected block between plain and the signature versions
    // you've bought (diplomacy::signatures).
    if keyboard.just_pressed(KeyCode::KeyV) && *current_state.get() != BuildState::Inactive {
        if let BuildSelection::Module(module_type) = build_state.current_selection() {
            let owned: Vec<_> = standing
                .signatures
                .iter()
                .filter_map(|id| crate::diplomacy::signatures::get(id))
                .filter(|s| s.base == module_type)
                .collect();
            let message = if owned.is_empty() {
                format!("No signature {} bought. Military markets sell them.", module_type.name())
            } else {
                let at = build_state.signature_for(module_type).and_then(|cur| owned.iter().position(|s| s.id == cur.id));
                let next = match at {
                    None => Some(owned[0]),
                    Some(i) if i + 1 < owned.len() => Some(owned[i + 1]),
                    Some(_) => None,
                };
                build_state.signature = next.map(|s| s.id);
                match next {
                    Some(s) => format!("Building {}: {}, {}", s.name, s.edge.to_lowercase(), s.cost.to_lowercase()),
                    None => format!("Building plain {}", module_type.name()),
                }
            };
            notifications.write(ShowNotification { message, notification_type: NotificationType::Info, duration: 3.0 });
        }
    }
    // B: Toggle build mode
    if keyboard.just_pressed(KeyCode::KeyB) {
        match current_state.get() {
            BuildState::Inactive => next_state.set(BuildState::Placing),
            _ => next_state.set(BuildState::Inactive),
        }
    }

    // All keys below only apply when build mode is active
    if *current_state.get() == BuildState::Inactive {
        return;
    }

    // Tab: Cycle categories
    if keyboard.just_pressed(KeyCode::Tab) {
        build_state.next_category();
        build_state.auto_rotated = true; // Re-enable auto-rotation on selection change
        info!("Category: {} | {}", build_state.current_category().name(), build_state.selection_name());
    }

    // BracketRight / BracketLeft: Cycle items within category
    if keyboard.just_pressed(KeyCode::BracketRight) {
        build_state.next_item();
        build_state.auto_rotated = true;
        info!("Selected: {}", build_state.selection_name());
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        build_state.prev_item();
        build_state.auto_rotated = true;
        info!("Selected: {}", build_state.selection_name());
    }

    // R: Rotate (manual override, disables auto-rotation until ghost moves)
    if keyboard.just_pressed(KeyCode::KeyR) {
        build_state.rotation = build_state.rotation.rotate_cw();
        build_state.auto_rotated = false;
        info!("Rotation: {:?}", build_state.rotation);
    }

    // M: Cycle hull material (only in Hull category), skipping locked materials
    if keyboard.just_pressed(KeyCode::KeyM) {
        let materials = [
            HullMaterial::Steel,
            HullMaterial::Titanium,
            HullMaterial::Composite,
            HullMaterial::AbyssalAlloy,
        ];
        let current_idx = materials.iter().position(|&m| m == build_state.hull_material).unwrap_or(0);
        let mut found = false;
        for i in 1..materials.len() {
            let next = materials[(current_idx + i) % materials.len()];
            if is_hull_material_unlocked(next, &unlocks) {
                build_state.hull_material = next;
                found = true;
                break;
            }
        }
        if !found {
            notifications.write(ShowNotification {
                message: "No other hull materials unlocked yet.".into(),
                notification_type: NotificationType::Warning,
                duration: 3.0,
            });
        }
        info!("Material: {} ({:.0}m)", build_state.hull_material.name(), build_state.hull_material.radiation_shielding());
    }

    // X: Toggle deletion mode
    if keyboard.just_pressed(KeyCode::KeyX) {
        match current_state.get() {
            BuildState::Deleting => next_state.set(BuildState::Placing),
            _ => next_state.set(BuildState::Deleting),
        }
    }

    // G: Open customization panel for current selection (if customizable)
    if keyboard.just_pressed(KeyCode::KeyG) {
        if let BuildSelection::Module(module_type) = build_state.current_selection() {
            let module_def = registry.get(module_type);
            if module_def.customizable {
                customization_state.start_customizing(module_type);
                notifications.write(ShowNotification {
                    message: format!("* Quick Customizing {}", module_type.name()),
                    notification_type: NotificationType::Info,
                    duration: 2.0,
                });
            } else {
                notifications.write(ShowNotification {
                    message: format!("{} is not customizable", module_type.name()),
                    notification_type: NotificationType::Info,
                    duration: 1.5,
                });
            }
        }
    }

    // P: Open component placement panel for current selection (if customizable)
    if keyboard.just_pressed(KeyCode::KeyP) {
        if let BuildSelection::Module(module_type) = build_state.current_selection() {
            let module_def = registry.get(module_type);
            if module_def.customizable {
                placement_state.start_placing(module_type);
                next_state.set(BuildState::PlacingComponent);
                notifications.write(ShowNotification {
                    message: format!("+ Component Builder: {} - Click pieces to assemble", module_type.name()),
                    notification_type: NotificationType::Info,
                    duration: 3.0,
                });
            } else {
                notifications.write(ShowNotification {
                    message: format!("{} cannot be built from components", module_type.name()),
                    notification_type: NotificationType::Info,
                    duration: 1.5,
                });
            }
        }
    }
}

// ============================================================================
// GHOST PREVIEW & VALIDATION
// ============================================================================

/// Converts the mouse cursor to a ship-local grid cell.
///
/// Grid coordinates are ship-local (hull/module tiles are children of
/// the ship, positioned relative to it — see spawn_module /
/// process_hull_placement / rooms::transform_to_grid, all of which
/// use `grid_y * 66 - 33` as the local Y). The cursor position from
/// viewport_to_world_2d is in WORLD space, so it has to be
/// transformed into the ship's local space first — dividing world
/// coordinates directly by grid_size only produces the right cell when
/// the ship happens to be sitting exactly at world origin with zero
/// rotation, which is essentially never true once you've actually flown
/// anywhere. Every cursor→grid conversion must go through here.
/// AFTERLIGHT_BUILD_CELL=<x>,<y>: a scripted run has no mouse; build mode points
/// at this ship cell instead (with AFTERLIGHT_KEYS' MouseLeft / MouseRight).
pub(crate) fn build_cell_override() -> Option<IVec2> {
    static CELL: std::sync::OnceLock<Option<IVec2>> = std::sync::OnceLock::new();
    *CELL.get_or_init(|| {
        let v = std::env::var("AFTERLIGHT_BUILD_CELL").ok()?;
        let (x, y) = v.split_once(',')?;
        Some(IVec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
    })
}

pub fn cursor_to_ship_grid(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    ship_gt: &GlobalTransform,
) -> Option<IVec2> {
    let cursor_pos = window.cursor_position()
        .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())?;
    let grid_size = 66.0;
    let cursor_world = Vec3::new(cursor_pos.x, cursor_pos.y, 0.0);
    let local = ship_gt.rotation().inverse() * (cursor_world - ship_gt.translation());
    Some(IVec2::new(
        (local.x / grid_size).round() as i32,
        ((local.y + 33.0) / grid_size).round() as i32,
    ))
}

/// Updates ghost position and validates placement.
/// Tracks mouse in both Placing and Deleting modes.
fn update_ghost_preview(
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    ship_query: Query<(Entity, &GlobalTransform), (With<Ship>, Without<Camera>)>,
    mut build_state: ResMut<BuildingState>,
    current_state: Res<State<BuildState>>,
    occupancy: Res<GridOccupancy>,
    // Player modules only — wrecks keep real Module children since the
    // destruction rework, and their local grid coords poisoned the
    // positional rules (engines became unplaceable anywhere).
    module_query: Query<&Module, Without<crate::ai_ship::components::OwnedByAiShip>>,
    hull_query: Query<(&HullSegment, &Transform, &ChildOf)>,
    registry: Res<ModuleRegistry>,
    currency: Res<Currency>,
) {
    // Track mouse position in both Placing and Deleting modes
    let state = *current_state.get();
    if state != BuildState::Placing && state != BuildState::Deleting {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };
    let Ok((player, ship_gt)) = ship_query.single() else { return };
    let player_ship = Some(player);

    let Some(grid_pos) = build_cell_override().or_else(|| cursor_to_ship_grid(window, camera, camera_transform, ship_gt)) else {
        // Pointing at nothing. Say so, rather than leaving the ghost sitting
        // wherever it was last -- which on the first frame of build mode is
        // the ship's origin, dead centre of the screen.
        build_state.cursor_on_grid = false;
        return;
    };
    {
        build_state.cursor_on_grid = true;
        let ghost_moved = build_state.ghost_position != grid_pos;
        build_state.ghost_position = grid_pos;

        // Auto-rotate modules when ghost moves (unless user manually rotated)
        // On a turret deck there's no hull around the ghost to face away
        // from: parts keep the way they're turned.
        if ghost_moved && build_state.auto_rotated && build_state.deck == BuildDeck::Hull && build_state.premade_index().is_none() {
            if let BuildSelection::Module(mt) = build_state.current_selection() {
                // A launcher wants a clear LANE, not merely an empty
                // neighbour: auto_rotate would happily point a tube into a
                // one-cell gap with the rest of the hull behind it.
                let rot = if is_launcher(mt) {
                    auto_rotate_launcher(grid_pos, &occupancy, &module_query)
                        .or_else(|| auto_rotate(grid_pos, &occupancy))
                } else {
                    auto_rotate(grid_pos, &occupancy)
                };
                if let Some(rot) = rot {
                    build_state.rotation = rot;
                }
            }
        }

        // Only validate placement in Placing mode
        if state != BuildState::Placing {
            return;
        }

        let selection = build_state.current_selection();
        let rotation = build_state.rotation;

        // Determine size of what we're placing
        let size = match selection {
            BuildSelection::Hull(_) => IVec2::new(1, 1),
            BuildSelection::Module(mt) => registry.get(mt).size,
        };
        let footprint = match selection {
            BuildSelection::Hull(_) => None,
            BuildSelection::Module(mt) => footprints::footprint_override(mt),
        };

        // Block limit check. Hull counts only the player's own plating: any
        // ship or wreck parked near the berth used to add its hull to the
        // tally, refusing placements well under the limit.
        let player_hull = hull_query.iter().filter(|(_, _, p)| Some(p.parent()) == player_ship).count();
        let block_count = module_query.iter().count() + player_hull;
        let under_limit = block_count < crate::combat::limits::MAX_SHIP_BLOCKS;

        // Check overlap using GridOccupancy (supports multi-cell)
        let no_overlap = occupancy.can_place(grid_pos, size, rotation, footprint);

        // Adjacency check - at least one cell of the new module must be adjacent
        // to an existing module or hull segment
        let placement_cells = GridOccupancy::cells_for(grid_pos, size, rotation, footprint);
        let has_neighbor = placement_cells.iter().any(|&cell| {
            for offset in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let neighbor = cell + offset;
                if occupancy.cells.contains_key(&neighbor) {
                    return true;
                }
            }
            false
        });

        let is_first = module_query.iter().count() == 0 && player_hull == 0;

        // Positional rules for modules
        let position_ok = check_position_rules(
            &selection,
            grid_pos,
            &module_query,
        );

        // Affordability check
        let can_afford = match selection {
            BuildSelection::Hull(_) => currency.credits >= build_state.hull_material.cost(),
            BuildSelection::Module(mt) => currency.credits >= registry.get(mt).cost,
        };

        // Multi-block directional validation for extension blocks
        let multiblock_ok = {
            let selection_mt = match &selection {
                BuildSelection::Module(mt) => Some(*mt),
                _ => None,
            };
            if let Some(mt) = selection_mt {
                match multiblock::build_helpers::module_type_to_role(mt) {
                    Some(_) => {
                        // This is a multi-block extension — validate direction
                        // We can't pass the full query here, so check adjacency to any MachineBlock core
                        true // Detailed validation happens at placement time
                    }
                    None => true, // Not a multi-block module, no extra validation
                }
            } else {
                true
            }
        };

        // A launcher's exit path. ADVISORY, not a placement rule: a buried
        // silo is a mistake the player should be able to see and then make
        // anyway, the same as any other bad layout decision. What it costs
        // them is paid at the trigger, not at the shop.
        let silo_blocker = match selection {
            BuildSelection::Module(mt) if is_launcher(mt) => {
                let dir = rotation.facing_offset();
                // Furthest cell of the footprint along the exit heading — a
                // multi-cell tube fires from its nose, not its origin.
                let muzzle = placement_cells.iter().copied()
                    .max_by_key(|c| c.x * dir.x + c.y * dir.y)
                    .unwrap_or(grid_pos);
                silo_obstruction(muzzle, dir, |cell| match occupancy.cells.get(&cell) {
                    None => SiloCell::Empty,
                    // A hull tile is not in module_query, so the Err arm is
                    // hull — and hull stops a missile.
                    Some(&e) => match module_query.get(e) {
                        Ok(m) if is_blowout_panel(m.module_type) => SiloCell::Panel,
                        _ => SiloCell::Solid,
                    },
                })
            }
            _ => None,
        };

        let valid = no_overlap && (has_neighbor || is_first) && position_ok && can_afford && multiblock_ok && under_limit;
        build_state.is_valid_placement = valid;
        build_state.placement_reason = if valid {
            silo_blocker.map(|c| format!("Silo exit blocked at ({}, {})", c.x, c.y))
        } else if !no_overlap {
            Some("Overlaps existing module or hull".into())
        } else if !has_neighbor && !is_first {
            Some("Must be adjacent to existing structure".into())
        } else if !under_limit {
            Some(format!("Block limit reached ({}/{})", block_count, crate::combat::limits::MAX_SHIP_BLOCKS))
        } else if !position_ok {
            match &selection {
                BuildSelection::Module(mt) => {
                    let cat = mt.category();
                    if cat == ModuleCategory::Propulsion {
                        Some("Propulsion must be at the rear".into())
                    } else if cat == ModuleCategory::Crew {
                        Some("Crew quarters cannot be next to reactors".into())
                    } else {
                        Some("Position rule violated".into())
                    }
                }
                _ => Some("Position rule violated".into()),
            }
        } else {
            let cost = match &selection {
                BuildSelection::Hull(_) => build_state.hull_material.cost(),
                BuildSelection::Module(mt) => registry.get(*mt).cost,
            };
            Some(format!("Not enough credits (need {}c)", cost))
        };
    }
}

/// Auto-rotates a module to face outward from the ship.
/// Checks the 4 cardinal directions from `grid_pos`; the direction with the
/// fewest occupied neighbors is considered "outward".  Ties are broken by
/// preferring the direction away from the ship's center (0, 0).
fn auto_rotate(grid_pos: IVec2, occupancy: &GridOccupancy) -> Option<Rotation> {
    // Directions: (offset, Rotation that makes the module face that direction)
    let directions: [(IVec2, Rotation); 4] = [
        (IVec2::Y,     Rotation::North), // up
        (IVec2::NEG_Y, Rotation::South), // down
        (IVec2::X,     Rotation::East),  // right
        (IVec2::NEG_X, Rotation::West),  // left
    ];

    // Count how many of the 4 neighbors are occupied
    let neighbor_count: i32 = directions.iter()
        .map(|(off, _)| if occupancy.cells.contains_key(&(grid_pos + *off)) { 1 } else { 0 })
        .sum();

    // If no neighbors at all, can't determine orientation
    if neighbor_count == 0 {
        return None;
    }

    // For each direction, score it: prefer direction with NO neighbor (= outward edge)
    // then break ties by distance from center
    let mut best: Option<(Rotation, f32)> = None;
    for (off, rot) in &directions {
        let has_neighbor = occupancy.cells.contains_key(&(grid_pos + *off));
        if has_neighbor {
            continue; // This direction faces inward — skip
        }
        // Tie-break: prefer direction that points away from center
        let outward_score = (grid_pos.as_vec2() + off.as_vec2()).length();
        if best.map_or(true, |(_, s)| outward_score > s) {
            best = Some((*rot, outward_score));
        }
    }

    best.map(|(rot, _)| rot)
}

/// Picks the heading whose exit lane is actually clear, for a launcher.
///
/// `auto_rotate` scores a direction by whether the ADJACENT cell is empty,
/// which is the right question for a radar dish and the wrong one for a
/// missile tube: a gap one cell deep with hull behind it scores as "outward".
/// Ties break toward the direction pointing away from the ship's centre, so a
/// bow tube faces the bow rather than whichever clear lane was tested first.
fn auto_rotate_launcher(
    grid_pos: IVec2,
    occupancy: &GridOccupancy,
    module_query: &Query<&Module, Without<crate::ai_ship::components::OwnedByAiShip>>,
) -> Option<Rotation> {
    let cell_kind = |cell: IVec2| match occupancy.cells.get(&cell) {
        None => SiloCell::Empty,
        Some(&e) => match module_query.get(e) {
            Ok(m) if is_blowout_panel(m.module_type) => SiloCell::Panel,
            _ => SiloCell::Solid,
        },
    };

    [Rotation::North, Rotation::East, Rotation::South, Rotation::West]
        .into_iter()
        .filter(|rot| silo_obstruction(grid_pos, rot.facing_offset(), cell_kind).is_none())
        .max_by(|a, b| {
            let score = |r: &Rotation| {
                (grid_pos.as_vec2() + r.facing_offset().as_vec2()).length()
            };
            score(a).partial_cmp(&score(b)).unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Checks positional rules for module placement
fn check_position_rules(
    selection: &BuildSelection,
    grid_pos: IVec2,
    module_query: &Query<&Module, Without<crate::ai_ship::components::OwnedByAiShip>>,
) -> bool {
    match selection {
        BuildSelection::Hull(_) => true,
        BuildSelection::Module(mt) => {
            let cat = mt.category();
            match cat {
                // Propulsion: at the rear. The ship builds nose-right —
                // the starter vessel's engines sit at the LEFTMOST x —
                // so rear means minimum x, not maximum (the old check
                // was backwards and rejected every engine placement).
                ModuleCategory::Propulsion => {
                    let min_x = module_query.iter()
                        .filter(|m| m.module_type.category() != ModuleCategory::Propulsion)
                        .map(|m| m.grid_position.x)
                        .min();
                    min_x.map_or(true, |mn| grid_pos.x <= mn)
                }
                // Crew: not adjacent to power modules (heat/radiation)
                ModuleCategory::Crew => {
                    let adjacent_to_power = module_query.iter().any(|m| {
                        m.module_type.category() == ModuleCategory::Power
                            && (m.grid_position - grid_pos).as_vec2().length() < 1.5
                    });
                    !adjacent_to_power
                }
                _ => true,
            }
        }
    }
}

// ============================================================================
// PLACEMENT & REMOVAL INPUT
// ============================================================================

/// Handles placing new modules/hull via click.
/// Hull can also be painted by holding the button and dragging — one block
/// per cell the ghost passes through. Modules stay click-per-place so a drag
/// can't accidentally buy three reactors.
fn handle_module_placement(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut build_state: ResMut<BuildingState>,
    current_state: Res<State<BuildState>>,
    mut place_module_events: MessageWriter<PlaceModuleRequest>,
    mut place_hull_events: MessageWriter<PlaceHullRequest>,
    symmetry_state: Res<symmetry::SymmetryState>,
    occupancy: Res<GridOccupancy>,
    mut last_painted: Local<Option<IVec2>>,
    registry: Res<ModuleRegistry>,
    currency: Res<Currency>,
    module_query: Query<&Module, Without<crate::ai_ship::components::OwnedByAiShip>>,
    player_children: Query<&Children, With<Ship>>,
    hull_blocks: Query<(), With<HullSegment>>,
) {
    // A pre-made weapon goes down whole (gun_stamp), not block by block.
    if *current_state.get() != BuildState::Placing || build_state.deck != BuildDeck::Hull || build_state.premade_index().is_some() {
        build_state.hull_rect_anchor = None;
        return;
    }

    if !mouse.pressed(MouseButton::Left) {
        *last_painted = None;
    }

    // TEMP [DEBUG_BUILD]: diagnosing a report of placement silently failing
    // after returning from combat. Remove once root-caused.
    if mouse.just_pressed(MouseButton::Left) {
        info!(
            "[DEBUG_BUILD] click at grid_pos={:?} rotation={:?} selection={:?} valid={} reason={:?} occupancy_len={}",
            build_state.ghost_position, build_state.rotation, build_state.current_selection(),
            build_state.is_valid_placement, build_state.placement_reason, occupancy.cells.len()
        );
    }

    let is_hull = matches!(build_state.current_selection(), BuildSelection::Hull(_));

    // Shift-drag a hull layer: fill the rectangle on release. Nothing is
    // painted while the rectangle is being dragged out.
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if is_hull && shift && mouse.just_pressed(MouseButton::Left) && build_state.cursor_on_grid {
        build_state.hull_rect_anchor = Some(build_state.ghost_position);
    }
    if let Some(anchor) = build_state.hull_rect_anchor {
        if mouse.pressed(MouseButton::Left) && is_hull {
            return;
        }
        build_state.hull_rect_anchor = None;
        let BuildSelection::Hull(layer) = build_state.current_selection() else { return };
        if !mouse.just_released(MouseButton::Left) {
            return;
        }
        let mut cells = rect_cells(anchor, build_state.ghost_position);
        if symmetry_state.enabled {
            let mirrored: Vec<IVec2> = cells.iter().map(|c| symmetry::mirror_position(*c)).collect();
            cells.extend(mirrored);
        }
        // Same tally the ghost's limit check uses: our modules plus our hull.
        let player_hull = player_children.single().map_or(0, |c| c.iter().filter(|e| hull_blocks.contains(*e)).count());
        let player_blocks = module_query.iter().count() + player_hull;
        let cost = build_state.hull_material.cost().max(1);
        let budget = ((currency.credits / cost) as usize)
            .min(crate::combat::limits::MAX_SHIP_BLOCKS.saturating_sub(player_blocks));
        let order = hull_fill_order(&cells, |c| occupancy.cells.contains_key(&c), player_blocks == 0, budget);
        for grid_position in order {
            place_hull_events.write(PlaceHullRequest {
                layer,
                material: build_state.hull_material,
                grid_position,
                free: false,
            });
        }
        return;
    }

    let drag_paint = is_hull
        && mouse.pressed(MouseButton::Left)
        && *last_painted != Some(build_state.ghost_position);

    if (mouse.just_pressed(MouseButton::Left) || drag_paint) && build_state.is_valid_placement {
        let pos = build_state.ghost_position;
        let rot = build_state.rotation;
        *last_painted = Some(pos);

        match build_state.current_selection() {
            BuildSelection::Hull(layer) => {
                place_hull_events.write(PlaceHullRequest {
                    layer,
                    material: build_state.hull_material,
                    grid_position: pos,
                    free: false,
                });
                // Symmetry: mirror hull placement
                if symmetry_state.enabled {
                    let cost = build_state.hull_material.cost();
                    let selection = build_state.current_selection();
                    if let Some((mirror_pos, _)) = symmetry::valid_mirror(
                        pos, IVec2::ONE, rot, None, &occupancy,
                        |p| check_position_rules(&selection, p, &module_query),
                    ) {
                        // Both copies have to be affordable, or only the
                        // original goes down.
                        if currency.credits >= cost * 2 {
                            place_hull_events.write(PlaceHullRequest {
                                layer,
                                material: build_state.hull_material,
                                grid_position: mirror_pos,
                                free: false,
                            });
                        }
                    }
                }
            }
            BuildSelection::Module(module_type) => {
                // A signature version, if one is picked for this block (V).
                let signed = build_state.signature_for(module_type).map(|s| blueprint::ModuleExtras { signature: Some(s.id.to_string()), ..default() });
                place_module_events.write(PlaceModuleRequest {
                    module_type,
                    grid_position: pos,
                    rotation: rot,
                    custom_name: None,
                    subcomponents: None,
                    extras: signed.clone(),
                    free: false,
                });
                // Symmetry: mirror module placement. The copy gets the same
                // checks the ghost gave the original -- free footprint,
                // touching the hull, position rules, both affordable -- it
                // used to skip all of them.
                if symmetry_state.enabled {
                    let def = registry.get(module_type);
                    let footprint = footprints::footprint_override(module_type);
                    let selection = build_state.current_selection();
                    let mirror = symmetry::valid_mirror(
                        pos, def.size, rot, footprint, &occupancy,
                        |p| check_position_rules(&selection, p, &module_query),
                    );
                    if let Some((mirror_pos, mirror_rot)) =
                        mirror.filter(|_| currency.credits >= def.cost * 2)
                    {
                        place_module_events.write(PlaceModuleRequest {
                            module_type,
                            grid_position: mirror_pos,
                            rotation: mirror_rot,
                            custom_name: None,
                            subcomponents: None,
                            extras: signed,
                            free: false,
                        });
                    }
                }
            }
        }
    }
}

/// Every cell of the rectangle between two corners, inclusive.
pub fn rect_cells(a: IVec2, b: IVec2) -> Vec<IVec2> {
    let (lo, hi) = (a.min(b), a.max(b));
    (lo.y..=hi.y).flat_map(|y| (lo.x..=hi.x).map(move |x| IVec2::new(x, y))).collect()
}

/// Which of `cells` a fill should place, and in what order: the free ones
/// that join the ship, spreading out from the cells already touching it, so
/// that whatever the money or the block limit runs out on is the far edge,
/// never a stray island. Cells that can't reach the ship through the fill
/// are left out -- a block has to touch the hull to be placed. On a ship with
/// no blocks at all the fill starts from the first cell.
pub fn hull_fill_order(
    cells: &[IVec2],
    occupied: impl Fn(IVec2) -> bool,
    empty_ship: bool,
    budget: usize,
) -> Vec<IVec2> {
    let free: HashSet<IVec2> = cells.iter().copied().filter(|c| !occupied(*c)).collect();
    let sides = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];
    let mut queue: std::collections::VecDeque<IVec2> = cells
        .iter()
        .copied()
        .filter(|c| free.contains(c) && sides.iter().any(|s| occupied(*c + *s)))
        .collect();
    if queue.is_empty() && empty_ship {
        queue.extend(cells.first().copied().filter(|c| free.contains(c)));
    }
    let mut placed: HashSet<IVec2> = queue.iter().copied().collect();
    let mut order = Vec::new();
    while let Some(cell) = queue.pop_front() {
        if order.len() >= budget {
            break;
        }
        order.push(cell);
        for s in sides {
            let next = cell + s;
            if free.contains(&next) && placed.insert(next) {
                queue.push_back(next);
            }
        }
    }
    order
}

/// Outline of the rectangle a Shift-drag will fill.
fn draw_hull_rect_preview(
    build_state: Res<BuildingState>,
    ship: Query<&GlobalTransform, With<Ship>>,
    mut gizmos: Gizmos,
) {
    let Some(anchor) = build_state.hull_rect_anchor else { return };
    let Ok(ship) = ship.single() else { return };
    let (lo, hi) = (anchor.min(build_state.ghost_position), anchor.max(build_state.ghost_position));
    let half = Vec2::splat(GRID_SIZE * 0.5);
    let corners = [
        grid_to_local(lo) - half,
        grid_to_local(IVec2::new(hi.x, lo.y)) + Vec2::new(half.x, -half.y),
        grid_to_local(hi) + half,
        grid_to_local(IVec2::new(lo.x, hi.y)) + Vec2::new(-half.x, half.y),
    ];
    let world: Vec<Vec2> = corners.iter().map(|c| ship.transform_point(c.extend(0.0)).truncate()).collect();
    gizmos.linestrip_2d(world.iter().copied().chain(std::iter::once(world[0])), Color::srgb(0.5, 0.9, 1.0));
}

/// Handles removing modules.
/// In delete mode the button can be held and dragged to sweep away several
/// blocks — one removal per cell entered. Right-click removal while placing
/// stays single-click.
fn handle_module_removal(
    mouse: Res<ButtonInput<MouseButton>>,
    build_state: Res<BuildingState>,
    current_state: Res<State<BuildState>>,
    occupancy: Res<GridOccupancy>,
    module_query: Query<(Entity, &Module)>,
    hull_query: Query<Entity, With<HullSegment>>,
    mut remove_events: MessageWriter<RemoveModuleRequest>,
    mut remove_hull_events: MessageWriter<RemoveHullRequest>,
    mut last_deleted: Local<Option<IVec2>>,
    turrets: Query<(&crate::combat::turret_deck::HasTurretBody, &crate::combat::turret_deck::TurretBody)>,
    bodies: Query<&Children, With<crate::combat::turret_deck::TurretBody>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let state = *current_state.get();
    let in_deleting = state == BuildState::Deleting;
    let in_placing = state == BuildState::Placing;

    // The turret deck takes its own clicks (turret_build::turret_deck_clicks).
    if (!in_deleting && !in_placing) || build_state.deck != BuildDeck::Hull {
        return;
    }

    if !mouse.pressed(MouseButton::Left) {
        *last_deleted = None;
    }

    let drag_delete = in_deleting
        && mouse.pressed(MouseButton::Left)
        && *last_deleted != Some(build_state.ghost_position);

    let should_delete = drag_delete
        || (in_placing && mouse.just_pressed(MouseButton::Right));

    if should_delete {
        if in_deleting {
            *last_deleted = Some(build_state.ghost_position);
        }
        // Use GridOccupancy to find the entity at the clicked cell
        // This works for any cell a multi-cell module occupies, not just origin
        if let Some(&entity) = occupancy.cells.get(&build_state.ghost_position) {
            if let Ok((_, module)) = module_query.get(entity) {
                // Protect last power source
                if module.module_type.category() == ModuleCategory::Power {
                    let power_count = module_query.iter()
                        .filter(|(_, m)| m.module_type.category() == ModuleCategory::Power)
                        .count();
                    if power_count <= 1 {
                        return;
                    }
                }
                // A ring with a turret built on it would take the turret's
                // parts down with it, unrefunded.
                if let Ok((has, _)) = turrets.get(entity) {
                    let loaded = bodies.get(has.0).is_ok_and(|ch| ch.iter().any(|c| module_query.contains(c)));
                    if loaded {
                        notifications.write(ShowNotification {
                            message: "Clear the turret first (PgUp to its deck)".into(),
                            notification_type: NotificationType::Warning,
                            duration: 2.0,
                        });
                        return;
                    }
                }
                remove_events.write(RemoveModuleRequest { module: entity });
            } else if hull_query.get(entity).is_ok() {
                remove_hull_events.write(RemoveHullRequest { hull: entity });
            }
        }
    }
}

// ============================================================================
// EVENT PROCESSING
// ============================================================================

/// Processes PlaceHullRequest events
fn process_hull_placement(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut events: MessageReader<PlaceHullRequest>,
    ship_query: Query<Entity, With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
    mut currency: ResMut<Currency>,
    mut history: ResMut<build_history::BuildHistory>,
) {
    let Ok(ship) = ship_query.single() else { return };

    for event in events.read() {
        let grid_pos = event.grid_position;
        let material = event.material;

        // Refuse a paid block the player can't cover. The ghost already turns
        // red when it's unaffordable, but two requests can arrive in one
        // frame (symmetry sends a mirror copy) and both were validated
        // against the same balance: the second spawned for whatever was left,
        // saturating at 0c, while the history recorded full price -- so undo
        // refunded money that was never paid.
        if !event.free && currency.credits < material.cost() {
            notifications.write(ShowNotification {
                message: format!("Not enough credits ({}c needed)", material.cost()),
                notification_type: NotificationType::Warning,
                duration: 1.5,
            });
            continue;
        }

        // Tint by layer type for visual distinction
        let color = match event.layer {
            HullLayer::Outer => Color::WHITE,
            HullLayer::Inner => Color::srgb(0.9, 0.9, 0.9),
            // Decking carries its own sprite now; tinting it just dimmed it.
            HullLayer::Hallway => Color::WHITE,
            HullLayer::Void => Color::srgb(0.5, 0.5, 0.6),
            HullLayer::BulkheadDoor => Color::srgb(0.9, 0.8, 0.7),
        };

        let texture = asset_server
            .load(sprite_map::hull_layer_sprite_path(material, event.layer));

        let hull_entity = commands.spawn((
            (Sprite {
                    image: texture,
                    color,
                    custom_size: Some(Vec2::new(66.0, 66.0)),
                    ..default()
                }, Transform::from_xyz(
                    grid_pos.x as f32 * 66.0,
                    grid_pos.y as f32 * 66.0 - 33.0,
                    0.1,
                )),
            BaseSpriteColor(color),
            BaseHullStats {
                max_health: 100.0 * material.health_multiplier(),
                radiation_shielding: material.radiation_shielding(),
            },
            HullSegment {
                hull_layer: event.layer,
                material,
                radiation_shielding: material.radiation_shielding(),
                health: 100.0 * material.health_multiplier(),
                max_health: 100.0 * material.health_multiplier(),
                grid_position: grid_pos,
                ..default()
            },
            Block::hull(grid_pos, material),
        )).insert(ChildOf(ship)).id();

        let layer_name = match event.layer {
            HullLayer::Outer => "Outer Hull",
            HullLayer::Inner => "Inner Hull",
            HullLayer::Hallway => "Hallway",
            HullLayer::Void => "Void Space",
            HullLayer::BulkheadDoor => "Bulkhead Door",
        };

        if !event.free {
            let cost = material.cost();
            currency.credits = currency.credits.saturating_sub(cost);
            history.record(build_history::BuildAction::PlaceHull {
                entity: hull_entity,
                material,
                cost,
            });

            notifications.write(ShowNotification {
                message: format!("Placed {} ({}) -{}c", layer_name, material.name(), cost),
                notification_type: NotificationType::Success,
                duration: 1.5,
            });
        }
    }
}

/// Processes PlaceModuleRequest events (registry-based)
fn process_module_placement(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut events: MessageReader<PlaceModuleRequest>,
    ship_query: Query<Entity, With<Ship>>,
    registry: Res<ModuleRegistry>,
    mut placed_events: MessageWriter<ModulePlaced>,
    mut notifications: MessageWriter<ShowNotification>,
    mut currency: ResMut<Currency>,
    mut history: ResMut<build_history::BuildHistory>,
    contacts: Res<crate::diplomacy::contacts::Contacts>,
) {
    let Ok(ship) = ship_query.single() else { return };

    for event in events.read() {
        // The ward's pattern comes from the Abbess (narrative::ward); the yard
        // can't lay one before. Rebuilds and loaded designs (free) are already
        // known, so they pass.
        if event.module_type == ModuleType::DarkWard && !event.free && !crate::narrative::ward::pattern_known(&contacts) {
            notifications.write(ShowNotification {
                message: "Nobody here knows how to build a Dark Ward. The Silence keep the pattern.".into(),
                notification_type: NotificationType::Warning,
                duration: 3.0,
            });
            continue;
        }
        let signature = event.extras.as_ref().and_then(|e| e.signature.as_deref()).and_then(crate::diplomacy::signatures::get).filter(|s| s.base == event.module_type);
        let price = |registry: &ModuleRegistry| (registry.get(event.module_type).cost as f32 * signature.map_or(1.0, |s| s.build)).round() as u32;
        // Same guard as hull placement: never spawn a paid module on credit.
        if !event.free {
            let cost = price(&registry);
            if currency.credits < cost {
                notifications.write(ShowNotification {
                    message: format!("Not enough credits ({}c needed)", cost),
                    notification_type: NotificationType::Warning,
                    duration: 1.5,
                });
                continue;
            }
        }

        // Check if this is a custom module
        let entity = if let (Some(custom_name), Some(subcomponents)) = (&event.custom_name, &event.subcomponents) {
            // Spawn custom module with ship-components
            crate::ship::spawn_custom_module(
                &mut commands,
                &asset_server,
                ship,
                event.module_type,
                custom_name.clone(),
                event.grid_position,
                event.rotation,
                subcomponents.clone(),
                &registry,
            )
        } else {
            // Spawn regular module
            crate::ship::spawn_module_signed(
                &mut commands,
                &asset_server,
                ship,
                event.module_type,
                event.grid_position,
                event.rotation,
                &registry,
                signature,
            )
        };

        // Restore design state (tuning, fire group, ammo) if the request
        // carried any — blueprint loads and ghost rebuilds do.
        if let Some(extras) = &event.extras {
            blueprint::apply_module_extras(&mut commands, entity, extras);
        }

        placed_events.write(ModulePlaced {
            module: entity,
            module_type: event.module_type,
            grid_position: event.grid_position,
        });

        if !event.free {
            let cost = price(&registry);
            currency.credits = currency.credits.saturating_sub(cost);
            history.record(build_history::BuildAction::PlaceModule {
                entity,
                module_type: event.module_type,
                cost,
            });

            let message = if let Some(sig) = signature {
                format!("Placed {} -{}c", sig.name, cost)
            } else if event.custom_name.is_some() {
                format!("Placed Custom {} -{}c", event.module_type.name(), cost)
            } else {
                format!("Placed {} -{}c", event.module_type.name(), cost)
            };

            notifications.write(ShowNotification {
                message,
                notification_type: NotificationType::Success,
                duration: 1.5,
            });
        }
    }
}

/// Processes RemoveModuleRequest events
fn process_module_removal(
    mut commands: Commands,
    mut events: MessageReader<RemoveModuleRequest>,
    module_query: Query<&Module>,
    mut removed_events: MessageWriter<ModuleRemoved>,
    mut notifications: MessageWriter<ShowNotification>,
    mut currency: ResMut<Currency>,
    registry: Res<ModuleRegistry>,
) {
    for event in events.read() {
        if let Ok(module) = module_query.get(event.module) {
            let cost = registry.get(module.module_type).cost;
            let refund = (cost as f32 * 0.75) as u32;
            currency.credits += refund;

            removed_events.write(ModuleRemoved {
                module_type: module.module_type,
                grid_position: module.grid_position,
            });

            notifications.write(ShowNotification {
                message: format!("Removed {} +{}c refund", module.module_type.name(), refund),
                notification_type: NotificationType::Warning,
                duration: 1.5,
            });

            commands.entity(event.module).despawn();
        }
    }
}

/// Processes RemoveHullRequest events (build-mode hull deletion, 75% refund
/// like modules)
fn process_hull_removal(
    mut commands: Commands,
    mut events: MessageReader<RemoveHullRequest>,
    hull_query: Query<&HullSegment>,
    mut notifications: MessageWriter<ShowNotification>,
    mut currency: ResMut<Currency>,
) {
    for event in events.read() {
        if let Ok(hull) = hull_query.get(event.hull) {
            let refund = (hull.material.cost() as f32 * 0.75) as u32;
            currency.credits += refund;

            notifications.write(ShowNotification {
                message: format!("Removed {} hull +{}c refund", hull.material.name(), refund),
                notification_type: NotificationType::Warning,
                duration: 1.5,
            });

            commands.entity(event.hull).despawn();
        }
    }
}

// ============================================================================
// CUSTOM MODULE STAT RECALCULATION
// ============================================================================

/// Recalculates stats for custom modules when their ship-components change
fn recalculate_custom_module_stats(
    mut commands: Commands,
    changed_modules: Query<
        (Entity, &CustomModule, &Children),
        Or<(Changed<CustomModule>, Changed<Children>)>
    >,
    subcomponent_query: Query<&SubComponent>,
    registry: Res<ModuleRegistry>,
) {
    for (entity, custom_module, children) in changed_modules.iter() {
        // Collect all ship-component types from children
        let subcomponents: Vec<SubComponentType> = children.iter()
            .filter_map(|child| subcomponent_query.get(child).ok())
            .map(|sc| sc.subcomponent_type.clone())
            .collect();

        // Get base stats from registry
        let module_def = registry.get(custom_module.base_type);
        let base_stats = &module_def.base_stats;

        // Calculate new stats
        let calculated = StatCalculator::calculate_stats(
            custom_module.base_type,
            &subcomponents,
            base_stats,
        );

        // Insert or update CalculatedStats component
        commands.entity(entity).insert(calculated);
    }
}

/// Syncs CalculatedStats weapon data back to the Weapon component (max_ammo, clamped ammo).
fn sync_calculated_to_weapon(
    mut weapon_query: Query<(&mut Weapon, &CalculatedStats), Changed<CalculatedStats>>,
) {
    for (mut weapon, calculated) in weapon_query.iter_mut() {
        if let Some(ref ws) = calculated.weapon {
            weapon.max_ammo = ws.max_ammo;
            weapon.ammo = weapon.ammo.min(ws.max_ammo);
        }
    }
}

#[cfg(test)]
mod ship_grid_tests {
    use super::*;
    use crate::ai_ship::components::AiShip;

    fn hull_at(grid: IVec2) -> (HullSegment, Transform) {
        (
            HullSegment { grid_position: grid, ..default() },
            // Same placement maths ship/spawner.rs and ai_ship/spawner.rs use.
            Transform::from_xyz(grid.x as f32 * 66.0, grid.y as f32 * 66.0 - 33.0, 0.1),
        )
    }

    fn module_at(grid: IVec2, size: IVec2) -> Module {
        Module {
            module_type: ModuleType::AmmoBay,
            health: 100.0,
            max_health: 100.0,
            power_consumption: 0.0,
            power_generation: 0.0,
            is_active: true,
            grid_position: grid,
            size,
            rotation: Rotation::North,
        }
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(layout::LayoutPlugin);
        app.add_systems(Update, update_ship_grids);
        app
    }

    /// The whole reason the per-ship grid exists: two ships with blocks at the
    /// same ship-LOCAL cell. A single global map can only ever hold one of
    /// them, which is why GridOccupancy is player-scoped.
    #[test]
    fn grids_do_not_collide_across_ships() {
        let mut app = test_app();
        let player = app.world_mut().spawn(Ship).id();
        let enemy = app.world_mut().spawn(AiShip).id();

        let cell = IVec2::new(1, 0);
        let player_block = app.world_mut().spawn(hull_at(cell)).insert(ChildOf(player)).id();
        let enemy_block = app.world_mut().spawn(hull_at(cell)).insert(ChildOf(enemy)).id();

        app.update();

        assert_eq!(app.world().get::<ShipGrid>(player).unwrap().get(cell), Some(player_block));
        assert_eq!(app.world().get::<ShipGrid>(enemy).unwrap().get(cell), Some(enemy_block));
    }

    /// Destroyed blocks leave the grid — a dead plate must not keep stopping
    /// shells. This is the difference from GridOccupancy that hit resolution
    /// depends on.
    #[test]
    fn destroyed_blocks_leave_the_grid() {
        let mut app = test_app();
        let ship = app.world_mut().spawn(Ship).id();
        let cell = IVec2::new(2, 3);
        let block = app.world_mut().spawn(hull_at(cell)).insert(ChildOf(ship)).id();

        app.update();
        assert!(app.world().get::<ShipGrid>(ship).unwrap().contains(cell));

        app.world_mut().entity_mut(block).insert(HullDestroyed);
        app.update();

        assert!(!app.world().get::<ShipGrid>(ship).unwrap().contains(cell),
            "a destroyed plate is still occupying its cell");
    }

    /// A multi-cell module claims every cell of its footprint, so a shell
    /// crossing any of them finds the same entity.
    #[test]
    fn multi_cell_modules_claim_their_whole_footprint() {
        let mut app = test_app();
        let ship = app.world_mut().spawn(Ship).id();
        let origin = IVec2::new(0, 0);
        let module = app.world_mut()
            .spawn(module_at(origin, IVec2::new(3, 2)))
            .insert(ChildOf(ship))
            .id();

        app.update();

        let grid = app.world().get::<ShipGrid>(ship).unwrap();
        assert_eq!(grid.cells.len(), 6);
        for x in 0..3 {
            for y in 0..2 {
                assert_eq!(grid.get(origin + IVec2::new(x, y)), Some(module));
            }
        }
    }

    /// Hull spawns at `grid * 66.0` with a -33.0 y offset; the grid has to
    /// invert that exactly or every block lands one cell off.
    #[test]
    fn hull_transforms_round_trip_to_their_cell() {
        let mut app = test_app();
        let ship = app.world_mut().spawn(Ship).id();
        let cells = [IVec2::new(0, 0), IVec2::new(-4, 7), IVec2::new(5, -3)];
        for cell in cells {
            app.world_mut().spawn(hull_at(cell)).insert(ChildOf(ship));
        }

        app.update();

        let grid = app.world().get::<ShipGrid>(ship).unwrap();
        for cell in cells {
            assert!(grid.contains(cell), "hull at {cell:?} did not land in its own cell");
        }
    }
}

#[cfg(test)]
mod block_tests {
    use super::*;

    #[test]
    fn hull_thickness_seeds_from_material_absorption() {
        for material in [HullMaterial::Steel, HullMaterial::Titanium, HullMaterial::Composite, HullMaterial::AbyssalAlloy] {
            let block = Block::hull(IVec2::new(2, -1), material);
            assert_eq!(block.thickness, material.damage_absorption());
            assert_eq!(block.kind, BlockKind::Hull);
            assert_eq!(block.cell, IVec2::new(2, -1));
            assert_eq!(block.facing, None, "plain hull takes the face it was hit on");
        }
    }

    #[test]
    fn modules_are_unarmoured() {
        let block = Block::module(IVec2::ZERO);
        assert_eq!(block.kind, BlockKind::Module);
        assert_eq!(block.thickness, 0.0);
    }
}

#[cfg(test)]
mod walk_tests {
    use super::*;

    /// Grid with one distinct entity per listed cell, plus the World that
    /// owns those entities (dropping it would not invalidate the ids, but
    /// keeping it makes the intent obvious).
    fn grid_of(cells: &[IVec2]) -> (World, ShipGrid) {
        let mut world = World::new();
        let map = cells.iter().map(|&c| (c, world.spawn_empty().id())).collect();
        (world, ShipGrid { cells: map })
    }

    fn cells_hit(steps: &[GridStep]) -> Vec<IVec2> {
        steps.iter().map(|s| s.cell).collect()
    }

    #[test]
    fn straight_pass_visits_each_cell_once_with_unit_span() {
        let (_w, grid) = grid_of(&[IVec2::new(0, 0), IVec2::new(1, 0), IVec2::new(2, 0)]);
        let steps = grid.walk(Vec2::new(-1.0, 0.0), Vec2::new(3.0, 0.0));
        assert_eq!(cells_hit(&steps), vec![IVec2::new(0, 0), IVec2::new(1, 0), IVec2::new(2, 0)]);
        for s in &steps {
            assert!((s.span - 1.0).abs() < 1e-4, "span {}", s.span);
        }
        assert_eq!(steps[0].entry_face, IVec2::new(-1, 0));
    }

    #[test]
    fn a_225_unit_frame_step_still_registers() {
        // Railgun + APFSDS: 225 units/frame = 3.4 cells. A point test at the
        // new position skips clean over a one-cell wall; the swept walk
        // cannot.
        let (_w, grid) = grid_of(&[IVec2::new(0, 0)]);
        let before = Vec2::new(-2.0, 0.0);
        let after = before + Vec2::new(225.0 / 66.0, 0.0);
        assert!(after.x > 1.0, "the wall is strictly between the two samples");
        let steps = grid.walk(before, after);
        assert_eq!(cells_hit(&steps), vec![IVec2::new(0, 0)]);
    }

    #[test]
    fn much_faster_than_anything_fired_still_registers() {
        let (_w, grid) = grid_of(&[IVec2::new(5, 3)]);
        let steps = grid.walk(Vec2::new(-4.0, 0.0), Vec2::new(14.0, 6.0));
        assert_eq!(cells_hit(&steps), vec![IVec2::new(5, 3)]);
    }

    #[test]
    fn diagonal_visits_corner_cells_bresenham_would_skip() {
        // Slope 1/2 from (0,0) to (2,1) passes through (1,0) AND (1,1). A
        // one-cell-per-column stepper picks one of them.
        for corner in [IVec2::new(1, 0), IVec2::new(1, 1)] {
            let (_w, grid) = grid_of(&[corner]);
            let steps = grid.walk(Vec2::ZERO, Vec2::new(2.0, 1.0));
            assert_eq!(cells_hit(&steps), vec![corner], "corner {corner:?} was skipped");
        }
    }

    #[test]
    fn diagonal_span_is_line_of_sight_thickness() {
        let (_w, grid) = grid_of(&[IVec2::ZERO]);
        // Corner to corner of cell (0,0): from (-0.5,-0.5) to (0.5,0.5).
        let steps = grid.walk(Vec2::new(-1.0, -1.0), Vec2::new(1.0, 1.0));
        let cell = steps.iter().find(|s| s.cell == IVec2::ZERO).expect("centre cell hit");
        assert!((cell.span - 2f32.sqrt()).abs() < 1e-3, "span {}", cell.span);
    }

    #[test]
    fn multi_cell_module_is_one_step_with_summed_span() {
        let mut world = World::new();
        let module = world.spawn_empty().id();
        let grid = ShipGrid {
            cells: [(IVec2::new(0, 0), module), (IVec2::new(1, 0), module)].into_iter().collect(),
        };
        let steps = grid.walk(Vec2::new(-1.0, 0.0), Vec2::new(2.0, 0.0));
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].entity, module);
        assert!((steps[0].span - 2.0).abs() < 1e-4, "span {}", steps[0].span);
    }

    #[test]
    fn empty_and_missed_walks_yield_nothing() {
        let (_w, grid) = grid_of(&[IVec2::new(0, 3)]);
        assert!(grid.walk(Vec2::new(-1.0, 0.0), Vec2::new(3.0, 0.0)).is_empty());
        let (_w2, empty) = grid_of(&[]);
        assert!(empty.walk(Vec2::new(-1.0, 0.0), Vec2::new(3.0, 0.0)).is_empty());
    }

    #[test]
    fn zero_length_walk_is_a_point_test() {
        let (_w, grid) = grid_of(&[IVec2::new(2, -1)]);
        let inside = grid.walk(Vec2::new(2.2, -1.3), Vec2::new(2.2, -1.3));
        assert_eq!(cells_hit(&inside), vec![IVec2::new(2, -1)]);
        assert!(grid.walk(Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0)).is_empty());
    }

    #[test]
    fn walk_is_bounded_and_survives_nan() {
        let (_w, grid) = grid_of(&[IVec2::new(50, 0)]);
        // 200 cells requested, cap is MAX_WALK_STEPS: exhaustion is a miss.
        assert!(grid.walk(Vec2::new(-100.0, 0.0), Vec2::new(100.0, 0.0)).is_empty());
        assert!(grid.walk(Vec2::new(f32::NAN, 0.0), Vec2::new(1.0, 0.0)).is_empty());
        assert!(grid.walk(Vec2::ZERO, Vec2::new(f32::INFINITY, 0.0)).is_empty());
    }
}

#[cfg(test)]
mod hull_fill_tests {
    use super::*;

    #[test]
    fn a_rectangle_covers_both_corners_whichever_way_it_is_dragged() {
        let a = rect_cells(IVec2::new(2, 3), IVec2::new(0, 1));
        assert_eq!(a.len(), 9);
        assert!(a.contains(&IVec2::new(0, 1)) && a.contains(&IVec2::new(2, 3)));
    }

    #[test]
    fn a_fill_grows_out_from_the_hull_and_skips_what_is_built() {
        // Existing hull along x = 0; fill x = 0..=3, y = 0..=1.
        let built = |c: IVec2| c.x == 0;
        let order = hull_fill_order(&rect_cells(IVec2::new(0, 0), IVec2::new(3, 1)), built, false, usize::MAX);
        assert_eq!(order.len(), 6, "the two built cells are skipped");
        let first_two: Vec<i32> = order.iter().take(2).map(|c| c.x).collect();
        assert_eq!(first_two, vec![1, 1], "cells touching the hull go first");
        assert!(order.iter().all(|c| c.x > 0));
    }

    #[test]
    fn running_out_of_money_leaves_the_far_edge_not_an_island() {
        let built = |c: IVec2| c.x == 0 && c.y == 0;
        let order = hull_fill_order(&rect_cells(IVec2::new(1, 0), IVec2::new(5, 0)), built, false, 2);
        assert_eq!(order, vec![IVec2::new(1, 0), IVec2::new(2, 0)]);
    }

    #[test]
    fn a_rectangle_out_in_open_space_places_nothing() {
        let built = |c: IVec2| c == IVec2::ZERO;
        let order = hull_fill_order(&rect_cells(IVec2::new(5, 5), IVec2::new(7, 7)), built, false, usize::MAX);
        assert!(order.is_empty(), "a block has to touch the hull");
    }

    #[test]
    fn the_first_rectangle_on_an_empty_grid_is_placed_whole() {
        let order = hull_fill_order(&rect_cells(IVec2::new(0, 0), IVec2::new(2, 2)), |_| false, true, usize::MAX);
        assert_eq!(order.len(), 9);
    }
}
