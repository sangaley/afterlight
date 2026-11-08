use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::building::customization::tuning::{SelectedAmmo, WeaponTuning};
use crate::combat::targeting::fire_groups::FireGroup;
use crate::components::*;
use crate::events::*;
use crate::states::BuildState;

// ============================================================================
// DATA MODEL
//
// The Blueprint is THE canonical ship design format — player saves, the
// starter vessel, and (in progress) AI faction ships all speak it. v1 files
// (hull + module types only) still load; v2 adds per-module state: weapon
// tuning, fire groups, ammo selection, custom modules.
// ============================================================================

/// Highest design version this build reads/writes.
pub const BLUEPRINT_VERSION: u32 = 2;

/// A saved ship blueprint
#[derive(Serialize, Deserialize, Clone)]
pub struct Blueprint {
    pub name: String,
    pub hull_cells: Vec<BlueprintHullCell>,
    pub modules: Vec<BlueprintModule>,
    pub created_at: String,
    /// Version tag for forward-compat; see BLUEPRINT_VERSION
    pub version: u32,
    /// Saved gun designs fitted by name (combat::gun_design), laid down as
    /// their parts when the design is read: draw a faction's gun once, fit
    /// it to every ship that carries it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guns: Vec<GunPlacement>,
}

/// One saved gun design fitted to a ship design.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GunPlacement {
    /// The gun design's name, or its file name without `.json`.
    pub design: String,
    /// Where its firing piece goes: a hull cell, or a turret cell when
    /// `turret` is set.
    pub at: IVec2,
    #[serde(default = "east")]
    pub facing: Rotation,
    /// On the turret standing on the ring at this hull cell.
    #[serde(default)]
    pub turret: Option<IVec2>,
}

fn east() -> Rotation {
    Rotation::East
}

/// Lay a design's fitted guns down as modules. A gun design nobody can find
/// is reported and left out.
pub fn expand_gun_placements(bp: &mut Blueprint, designs: &[crate::combat::gun_design::GunDesign]) {
    for fit in std::mem::take(&mut bp.guns) {
        let found = designs.iter().find(|d| d.name == fit.design || d.file_stem() == fit.design);
        let Some(gun) = found else {
            warn!("Design '{}' fits gun '{}', which isn't in designs/guns", bp.name, fit.design);
            continue;
        };
        // A pre-made weapon brings its own ring: its turret parts go on that
        // ring's turret, in the turret's grid.
        let parts: Vec<(ModuleType, IVec2, Rotation, Option<IVec2>)> = if gun.has_turret_parts() {
            let Some(installed) = gun.install(fit.at, fit.facing) else {
                warn!("Design '{}' fits '{}', which has turret parts but no ring", bp.name, fit.design);
                continue;
            };
            installed.into_iter().map(|i| (i.module_type, i.cell, i.rotation, i.ring)).collect()
        } else {
            gun.placed(fit.at, fit.facing).into_iter().map(|(t, at, r)| (t, at, r, fit.turret)).collect()
        };
        for (module_type, grid_pos, rotation, turret) in parts {
            let extras = ModuleExtras {
                turret,
                shell: gun.shell.clone().filter(|_| module_type == ModuleType::FiringPiece),
                point_defence: gun.point_defence_on(module_type),
                ..default()
            };
            bp.modules.push(BlueprintModule {
                module_type,
                grid_pos,
                rotation,
                custom_name: None,
                subcomponents: None,
                extras: (!extras.is_empty()).then_some(extras),
            });
        }
    }
}

impl Blueprint {
    /// Human-readable summary of the blueprint contents
    pub fn summary(&self) -> String {
        let module_count = self.modules.len();
        let hull_count = self.hull_cells.len();

        // Count unique module categories
        let mut categories: Vec<ModuleCategory> = self.modules.iter()
            .map(|m| m.module_type.category())
            .collect();
        categories.sort_by_key(|c| *c as u8);
        categories.dedup();
        let cat_count = categories.len();

        format!("{} hull, {} modules ({} categories)", hull_count, module_count, cat_count)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlueprintHullCell {
    pub grid_pos: IVec2,
    pub layer: HullLayer,
    pub material: HullMaterial,
}

/// The same design built `k` times larger out of ordinary blocks: every hull
/// cell becomes a k x k patch, and every module is repeated to fill the patch
/// its footprint now covers -- except weapons, which keep one copy each (a
/// station nine times the size with nine times the guns would be a wall of
/// fire, not a bigger station). Layers are recomputed: only cells on the new
/// outside edge stay Outer. `size` gives a module type's footprint size.
pub fn scale_design(design: &Blueprint, k: i32, size: impl Fn(ModuleType) -> IVec2) -> Blueprint {
    if k <= 1 {
        return design.clone();
    }
    use std::collections::HashSet;
    let mut cells: Vec<BlueprintHullCell> = Vec::with_capacity(design.hull_cells.len() * (k * k) as usize);
    for c in &design.hull_cells {
        for i in 0..k {
            for j in 0..k {
                cells.push(BlueprintHullCell { grid_pos: c.grid_pos * k + IVec2::new(i, j), ..c.clone() });
            }
        }
    }
    let filled: HashSet<IVec2> = cells.iter().map(|c| c.grid_pos).collect();
    for c in cells.iter_mut() {
        let edge = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y].iter().any(|d| !filled.contains(&(c.grid_pos + *d)));
        if c.layer == HullLayer::Outer && !edge {
            c.layer = HullLayer::Inner;
        }
    }
    let mut modules = Vec::new();
    for m in &design.modules {
        let fp = crate::building::ShipGrid::cells_for(IVec2::ZERO, size(m.module_type), m.rotation, crate::building::footprints::footprint_override(m.module_type));
        let mn = fp.iter().fold(IVec2::MAX, |a, c| a.min(*c));
        let mx = fp.iter().fold(IVec2::MIN, |a, c| a.max(*c));
        let span = mx - mn + IVec2::ONE;
        let weapon = m.module_type.category() == ModuleCategory::Weapons;
        let (copies_x, copies_y) = (k, k);
        for a in 0..copies_x {
            for b in 0..copies_y {
                if weapon && !(a == copies_x / 2 && b == copies_y / 2) {
                    continue;
                }
                let origin = (m.grid_pos + mn) * k + IVec2::new(a * span.x, b * span.y) - mn;
                modules.push(BlueprintModule { grid_pos: origin, ..m.clone() });
            }
        }
    }
    Blueprint { hull_cells: cells, modules, ..design.clone() }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlueprintModule {
    pub module_type: ModuleType,
    pub grid_pos: IVec2,
    pub rotation: Rotation,
    /// Custom-built module name (None = stock module)
    #[serde(default)]
    pub custom_name: Option<String>,
    /// Sub-components of a custom-built module
    #[serde(default)]
    pub subcomponents: Option<Vec<SubComponentType>>,
    /// Non-default per-module state (tuning, fire group, ammo)
    #[serde(default)]
    pub extras: Option<ModuleExtras>,
}

/// Per-module state a design carries beyond type + position. Only
/// non-default values are recorded, so v1-era files and untouched modules
/// serialize compactly.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ModuleExtras {
    #[serde(default)]
    pub tuning: Option<WeaponTuning>,
    #[serde(default)]
    pub fire_group: Option<u8>,
    #[serde(default)]
    pub ammo: Option<SelectedAmmo>,
    /// On a turret deck: the hull cell of the Turret Ring it stands on. The
    /// block's grid_pos is then in that turret's own grid, (0, 0) at the
    /// ring's centre (combat::turret_deck).
    #[serde(default)]
    pub turret: Option<IVec2>,
    /// A built gun's shell design, on its firing piece (combat::shell).
    #[serde(default)]
    pub shell: Option<crate::combat::shell::ShellDesign>,
    /// A launcher's missile design (combat::missile_design), when it isn't
    /// the launcher's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missile: Option<crate::combat::missile_design::MissileDesign>,
    /// On point defence: shoots at incoming missiles (combat::missile_defence).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub point_defence: bool,
    /// Built to a faction's signature (diplomacy::signatures), by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl ModuleExtras {
    pub fn is_empty(&self) -> bool {
        self.tuning.is_none() && self.fire_group.is_none() && self.ammo.is_none() && self.turret.is_none() && self.shell.is_none() && self.missile.is_none() && !self.point_defence && self.signature.is_none()
    }
}

/// Inserts a design's per-module state onto a freshly spawned module.
pub fn apply_module_extras(commands: &mut Commands, entity: Entity, extras: &ModuleExtras) {
    if let Some(tuning) = extras.tuning {
        commands.entity(entity).try_insert(tuning);
    }
    if let Some(group) = extras.fire_group {
        commands.entity(entity).try_insert(FireGroup { group });
    }
    if let Some(ammo) = extras.ammo {
        commands.entity(entity).try_insert(ammo);
    }
    if let Some(ring_cell) = extras.turret {
        commands.entity(entity).try_insert(crate::combat::turret_deck::PendingTurretMount(ring_cell));
    }
    if let Some(shell) = &extras.shell {
        commands.entity(entity).try_insert(crate::combat::shell::GunShell(shell.clone()));
    }
    if let Some(missile) = &extras.missile {
        commands.entity(entity).try_insert(crate::combat::missile_design::MissileLoadout(missile.clone()));
    }
    if extras.point_defence {
        commands.entity(entity).try_insert(crate::combat::targeting::fire_groups::InterceptMode);
    }
}

/// Resource holding available blueprints (scanned from disk)
#[derive(Resource)]
pub struct BlueprintResource {
    pub blueprints: Vec<Blueprint>,
    pub selected_index: usize,
    /// Cooldown to prevent rapid-fire saves
    pub save_cooldown: Timer,
    /// Whether the blueprint list has been scanned this session
    pub scanned: bool,
}

impl Default for BlueprintResource {
    fn default() -> Self {
        Self {
            blueprints: Vec::new(),
            selected_index: 0,
            save_cooldown: Timer::from_seconds(1.0, TimerMode::Once),
            scanned: false,
        }
    }
}

// ============================================================================
// DISK I/O
// ============================================================================

fn blueprints_dir() -> PathBuf {
    PathBuf::from("blueprints")
}

/// Scans the blueprints/ directory and loads all blueprint JSON files
pub fn scan_blueprints(resource: &mut BlueprintResource) {
    resource.blueprints.clear();
    let dir = blueprints_dir();
    if !dir.exists() {
        resource.scanned = true;
        return;
    }
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                match fs::read_to_string(&path) {
                    Ok(data) => match serde_json::from_str::<Blueprint>(&data) {
                        Ok(bp) => {
                            if bp.version > BLUEPRINT_VERSION {
                                warn!("Skipping blueprint {:?}: unsupported version {}", path, bp.version);
                                continue;
                            }
                            resource.blueprints.push(bp);
                        }
                        Err(e) => { warn!("Skipping invalid blueprint {:?}: {}", path, e); }
                    },
                    Err(e) => { warn!("Could not read blueprint {:?}: {}", path, e); }
                }
            }
        }
    }
    resource.blueprints.sort_by(|a, b| a.name.cmp(&b.name));
    resource.scanned = true;
    info!("Scanned {} blueprints from {:?}", resource.blueprints.len(), dir);
}

/// Writes a Blueprint to disk as pretty-printed JSON
fn write_blueprint_to_disk(bp: &Blueprint) -> Result<PathBuf, String> {
    let dir = blueprints_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("Cannot create blueprints dir: {}", e))?;
    }
    // Sanitize name for filesystem
    let safe_name: String = bp.name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    let path = dir.join(format!("{}.json", safe_name));
    let json = serde_json::to_string_pretty(bp)
        .map_err(|e| format!("Serialization failed: {}", e))?;
    fs::write(&path, json)
        .map_err(|e| format!("Write failed: {}", e))?;
    Ok(path)
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// Ctrl+S: save current ship's hull & modules as a named blueprint.
/// Only saves entities that are children of the Ship entity.

/// Queries needed to read a live ship back out as a Blueprint.
///
/// Bundled so the capture can be done from more than one place without
/// restating eight query types, and so there is exactly one definition of what
/// "this ship, as a design" means.
#[derive(bevy::ecs::system::SystemParam)]
pub struct DesignCapture<'w, 's> {
    pub hull_query: Query<'w, 's, &'static HullSegment>,
    pub module_query: Query<'w, 's, (Entity, &'static Module)>,
    pub tuning_query: Query<'w, 's, &'static WeaponTuning>,
    pub fire_group_query: Query<'w, 's, &'static FireGroup>,
    pub ammo_query: Query<'w, 's, &'static SelectedAmmo>,
    pub custom_query: Query<'w, 's, (&'static CustomModule, &'static Children)>,
    pub subcomp_query: Query<'w, 's, &'static SubComponent>,
    pub turret_query: Query<'w, 's, &'static Children, With<crate::combat::turret_deck::TurretBody>>,
    pub on_turret_query: Query<'w, 's, &'static crate::combat::turret_deck::OnTurret>,
    pub pending_turret_query: Query<'w, 's, &'static crate::combat::turret_deck::PendingTurretMount>,
    pub plate_query: Query<'w, 's, (), With<crate::combat::turret_deck::TurretPlate>>,
    pub shell_query: Query<'w, 's, &'static crate::combat::shell::GunShell>,
    pub missile_query: Query<'w, 's, &'static crate::combat::missile_design::MissileLoadout>,
    pub pd_query: Query<'w, 's, (), With<crate::combat::targeting::fire_groups::InterceptMode>>,
    pub signature_query: Query<'w, 's, &'static crate::diplomacy::signatures::Signature>,
}

impl DesignCapture<'_, '_> {
    /// Read the given ship's children back out as a design. Returns None when
    /// there is nothing to record.
    pub fn capture(&self, children: &Children, name: impl Into<String>) -> Option<Blueprint> {
        // The ship's own blocks, then what stands on its turret decks. A
        // turret's plating comes with its ring and is never recorded.
        let blocks: Vec<Entity> = children
            .iter()
            .chain(children.iter().filter_map(|c| self.turret_query.get(c).ok()).flat_map(|t| t.iter()))
            .filter(|e| !self.plate_query.contains(*e))
            .collect();
        let hull_cells: Vec<BlueprintHullCell> = blocks
            .iter()
            .filter_map(|&child| self.hull_query.get(child).ok())
            .map(|h| BlueprintHullCell {
                grid_pos: h.grid_position,
                layer: h.hull_layer,
                material: h.material,
            })
            .collect();

        let modules: Vec<BlueprintModule> = blocks
            .iter()
            .filter_map(|&child| self.module_query.get(child).ok())
            .map(|(entity, m)| {
                let tuning = self.tuning_query.get(entity).ok().copied().filter(|t| {
                    t.velocity != 1.0 || t.fire_rate != 1.0 || t.damage != 1.0
                });
                let fire_group = self
                    .fire_group_query
                    .get(entity)
                    .ok()
                    .map(|g| g.group)
                    .filter(|&g| g != 0);
                let ammo = self
                    .ammo_query
                    .get(entity)
                    .ok()
                    .copied()
                    .filter(|a| a.0 != crate::combat::ammo_types::KineticAmmoType::AP);
                let turret = self
                    .on_turret_query
                    .get(entity)
                    .map(|t| t.ring_cell)
                    .or_else(|_| self.pending_turret_query.get(entity).map(|p| p.0))
                    .ok();
                // Only a shell someone designed; the default is no entry.
                let shell = self
                    .shell_query
                    .get(entity)
                    .ok()
                    .map(|s| s.0.clone())
                    .filter(|s| *s != crate::combat::shell::ShellDesign::default());
                // Likewise only a missile someone designed.
                let missile = self
                    .missile_query
                    .get(entity)
                    .ok()
                    .map(|l| l.0.clone())
                    .filter(|d| *d != crate::combat::missile_design::MissileDesign::default_for(m.module_type));
                let point_defence = self.pd_query.contains(entity);
                let signature = self.signature_query.get(entity).ok().map(|s| s.0.id.to_string());
                let extras = ModuleExtras { tuning, fire_group, ammo, turret, shell, missile, point_defence, signature };

                let (custom_name, subcomponents) = match self.custom_query.get(entity) {
                    Ok((custom, module_children)) => (
                        Some(custom.custom_name.clone()),
                        Some(
                            module_children
                                .iter()
                                .filter_map(|c| self.subcomp_query.get(c).ok())
                                .map(|sc| sc.subcomponent_type.clone())
                                .collect(),
                        ),
                    ),
                    Err(_) => (None, None),
                };

                BlueprintModule {
                    module_type: m.module_type,
                    grid_pos: m.grid_position,
                    rotation: m.rotation,
                    custom_name,
                    subcomponents,
                    extras: if extras.is_empty() { None } else { Some(extras) },
                }
            })
            .collect();

        if hull_cells.is_empty() && modules.is_empty() {
            return None;
        }

        Some(Blueprint {
            name: name.into(),
            hull_cells,
            modules,
            created_at: chrono_lite_timestamp(),
            version: BLUEPRINT_VERSION,
            guns: Vec::new(),
        })
    }
}

pub fn save_blueprint_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    ship_query: Query<&Children, With<Ship>>,
    capture: DesignCapture,
    mut resource: ResMut<BlueprintResource>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    // Tick cooldown
    resource.save_cooldown.tick(time.delta());

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !(ctrl && keyboard.just_pressed(KeyCode::KeyS)) {
        return;
    }

    // Prevent rapid-fire saves
    if !resource.save_cooldown.is_finished() {
        return;
    }

    let Ok(children) = ship_query.single() else {
        notifications.write(ShowNotification {
            message: "No ship to save.".into(),
            notification_type: NotificationType::Warning,
            duration: 2.0,
        });
        return;
    };

    // The same reading of the ship every other "this ship as a design" uses,
    // turret decks included.
    let Some(mut blueprint) = capture.capture(children, String::new()) else {
        notifications.write(ShowNotification {
            message: "Nothing to save - build some hull/modules first.".into(),
            notification_type: NotificationType::Warning,
            duration: 2.0,
        });
        return;
    };
    let name = format!("sub_{}m_{}", blueprint.modules.len(), blueprint.created_at);
    blueprint.name = name.clone();

    let summary = blueprint.summary();

    match write_blueprint_to_disk(&blueprint) {
        Ok(path) => {
            info!("Blueprint saved to {:?}", path);
            scan_blueprints(&mut resource);
            resource.save_cooldown.reset();

            notifications.write(ShowNotification {
                message: format!("Saved '{}' ({})", name, summary),
                notification_type: NotificationType::Success,
                duration: 3.0,
            });
        }
        Err(e) => {
            notifications.write(ShowNotification {
                message: format!("Save failed: {}", e),
                notification_type: NotificationType::Danger,
                duration: 3.0,
            });
        }
    }
}

/// Ctrl+L: cycle through and load a saved blueprint.
/// Despawns existing hull, modules, and crew. Places blueprint contents for free.
pub fn load_blueprint_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut resource: ResMut<BlueprintResource>,
    hull_entities: Query<Entity, With<HullSegment>>,
    module_entities: Query<Entity, With<Module>>,
    crew_entities: Query<Entity, With<CrewMember>>,
    mut place_hull_events: MessageWriter<PlaceHullRequest>,
    mut place_module_events: MessageWriter<PlaceModuleRequest>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !(ctrl && keyboard.just_pressed(KeyCode::KeyL)) {
        return;
    }

    // Lazy-scan on first access
    if !resource.scanned {
        scan_blueprints(&mut resource);
    }

    if resource.blueprints.is_empty() {
        notifications.write(ShowNotification {
            message: "No blueprints found. Save one first with Ctrl+S.".into(),
            notification_type: NotificationType::Warning,
            duration: 2.5,
        });
        return;
    }

    let count = resource.blueprints.len();
    let idx = resource.selected_index % count;
    let blueprint = resource.blueprints[idx].clone();
    resource.selected_index = (idx + 1) % count;

    // Clear existing hull, modules, and crew
    for entity in hull_entities.iter() {
        commands.entity(entity).despawn();
    }
    for entity in module_entities.iter() {
        commands.entity(entity).despawn();
    }
    for entity in crew_entities.iter() {
        commands.entity(entity).despawn();
    }

    // Rebuild from blueprint — free placement (no cost, no per-item notifications)
    for hull_cell in &blueprint.hull_cells {
        place_hull_events.write(PlaceHullRequest {
            layer: hull_cell.layer,
            material: hull_cell.material,
            grid_position: hull_cell.grid_pos,
            free: true,
        });
    }

    for module in &blueprint.modules {
        place_module_events.write(PlaceModuleRequest {
            module_type: module.module_type,
            grid_position: module.grid_pos,
            rotation: module.rotation,
            custom_name: module.custom_name.clone(),
            subcomponents: module.subcomponents.clone(),
            extras: module.extras.clone(),
            free: true,
        });
    }

    let summary = blueprint.summary();
    let position = format!("{}/{}", idx + 1, count);

    notifications.write(ShowNotification {
        message: format!("Loaded '{}' [{}] ({})", blueprint.name, position, summary),
        notification_type: NotificationType::Success,
        duration: 3.0,
    });
}

/// Ctrl+D: delete the currently selected blueprint (only works in build mode)
pub fn delete_blueprint_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    build_state: Res<State<BuildState>>,
    mut resource: ResMut<BlueprintResource>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !(ctrl && keyboard.just_pressed(KeyCode::KeyD)) {
        return;
    }

    // Only allow deletion while in build mode
    if *build_state.get() == BuildState::Inactive {
        return;
    }

    if resource.blueprints.is_empty() {
        return;
    }

    // Delete the most recently loaded blueprint (selected_index - 1)
    let count = resource.blueprints.len();
    let idx = if resource.selected_index == 0 { count - 1 } else { resource.selected_index - 1 };
    let idx = idx % count;
    let bp_name = resource.blueprints[idx].name.clone();

    // Remove from disk
    let safe_name: String = bp_name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    let path = blueprints_dir().join(format!("{}.json", safe_name));
    if path.exists() {
        match fs::remove_file(&path) {
            Ok(_) => {
                notifications.write(ShowNotification {
                    message: format!("Deleted blueprint '{}'", bp_name),
                    notification_type: NotificationType::Warning,
                    duration: 2.0,
                });
            }
            Err(e) => {
                notifications.write(ShowNotification {
                    message: format!("Failed to delete '{}': {}", bp_name, e),
                    notification_type: NotificationType::Danger,
                    duration: 3.0,
                });
                return;
            }
        }
    }

    // Refresh list
    scan_blueprints(&mut resource);
    resource.selected_index = 0;
}

// ============================================================================
// DESIGN FILES & DIRECT SPAWNING
// Shared by the starter vessel (ship::spawner) and — next slice — AI ships.
// Unlike blueprint loading (event-driven, replaces the player ship), this
// path spawns a design's blocks directly under an existing root entity.
// ============================================================================

/// Loads a single design file. None if missing, unreadable, or from a
/// newer format version.
pub fn load_design_file(path: impl AsRef<Path>) -> Option<Blueprint> {
    let path = path.as_ref();
    let data = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Blueprint>(&data) {
        Ok(mut bp) if bp.version <= BLUEPRINT_VERSION => {
            if !bp.guns.is_empty() {
                expand_gun_placements(&mut bp, &crate::combat::gun_design::read_gun_designs());
            }
            Some(bp)
        }
        Ok(bp) => {
            warn!("Design {:?} has unsupported version {}", path, bp.version);
            None
        }
        Err(e) => {
            warn!("Design {:?} failed to parse: {}", path, e);
            None
        }
    }
}

/// Writes a design as pretty JSON, creating parent directories.
pub fn write_design_file(path: impl AsRef<Path>, bp: &Blueprint) -> Result<(), String> {
    let path = path.as_ref();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Cannot create {:?}: {}", dir, e))?;
    }
    let json = serde_json::to_string_pretty(bp).map_err(|e| format!("Serialize failed: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Write failed: {}", e))
}

/// Spawns every block of a design as children of `ship`, applying each
/// module's extras. Hull spawning mirrors building::process_hull_placement
/// (same tints, same stats) so designed ships and hand-built ships are
/// indistinguishable.
pub fn spawn_ship_from_design(
    commands: &mut Commands,
    asset_server: &AssetServer,
    registry: &crate::building::ModuleRegistry,
    ship: Entity,
    design: &Blueprint,
) {
    for cell in &design.hull_cells {
        let color = match cell.layer {
            HullLayer::Outer => Color::WHITE,
            HullLayer::Inner => Color::srgb(0.9, 0.9, 0.9),
            // Decking has its own sprite now, so it needs no tint -- the old
            // 45% darkening was what made a corridor look like a burnt plate.
            HullLayer::Hallway => Color::WHITE,
            HullLayer::Void => Color::srgb(0.5, 0.5, 0.6),
            HullLayer::BulkheadDoor => Color::srgb(0.9, 0.8, 0.7),
        };
        let texture = asset_server
            .load(crate::sprite_map::hull_layer_sprite_path(cell.material, cell.layer));
        let health = 100.0 * cell.material.health_multiplier();

        commands.spawn((
            (Sprite {
                    image: texture,
                    color,
                    // Full cell. At 64 in a 66-unit cell every hull plate had
                    // a 2-unit gap on each side and you could see open space
                    // between the blocks of your own ship.
                    custom_size: Some(Vec2::new(66.0, 66.0)),
                    ..default()
                }, Transform::from_xyz(
                    cell.grid_pos.x as f32 * 66.0,
                    cell.grid_pos.y as f32 * 66.0 - 33.0,
                    0.1,
                )),
            BaseSpriteColor(color),
            BaseHullStats {
                max_health: health,
                radiation_shielding: cell.material.radiation_shielding(),
            },
            HullSegment {
                hull_layer: cell.layer,
                material: cell.material,
                radiation_shielding: cell.material.radiation_shielding(),
                health,
                max_health: health,
                grid_position: cell.grid_pos,
                ..default()
            },
            crate::building::Block::hull(cell.grid_pos, cell.material),
            ChildOf(ship),
        ));
    }

    for m in &design.modules {
        let entity = if let (Some(name), Some(subs)) = (&m.custom_name, &m.subcomponents) {
            crate::ship::spawn_custom_module(
                commands, asset_server, ship, m.module_type,
                name.clone(), m.grid_pos, m.rotation, subs.clone(), registry,
            )
        } else {
            let signature = m.extras.as_ref().and_then(|e| e.signature.as_deref()).and_then(crate::diplomacy::signatures::get);
            crate::ship::spawn_module_signed(
                commands, asset_server, ship, m.module_type,
                m.grid_pos, m.rotation, registry, signature,
            )
        };
        if let Some(extras) = &m.extras {
            apply_module_extras(commands, entity, extras);
        }
    }
}

// ============================================================================
// HELPERS
// ============================================================================

/// Unix-seconds timestamp without external crate
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}


#[cfg(test)]
mod scale_design_tests {
    use super::*;

    fn design() -> Blueprint {
        let mut hull = Vec::new();
        for x in -2i32..=2 {
            for y in -1i32..=1 {
                let edge = x.abs() == 2 || y.abs() == 1;
                hull.push(BlueprintHullCell { grid_pos: IVec2::new(x, y), layer: if edge { HullLayer::Outer } else { HullLayer::Inner }, material: HullMaterial::Steel });
            }
        }
        let module = |t: ModuleType, x: i32, y: i32, r: Rotation| BlueprintModule {
            module_type: t, grid_pos: IVec2::new(x, y), rotation: r, custom_name: None, subcomponents: None, extras: None,
        };
        Blueprint {
            name: "t".into(),
            hull_cells: hull,
            modules: vec![
                module(ModuleType::LargeCargo, -1, 0, Rotation::North),
                module(ModuleType::LargeCargo, 1, 0, Rotation::West),
                module(ModuleType::Cannon, 0, 1, Rotation::North),
            ],
            created_at: String::new(),
            version: 1,
            guns: Vec::new(),
        }
    }

    fn size(t: ModuleType) -> IVec2 {
        if t == ModuleType::LargeCargo { IVec2::new(2, 1) } else { IVec2::ONE }
    }

    /// Three times the size: nine times the hull, cargo filling its patch,
    /// one gun, nothing overlapping, everything standing on hull.
    #[test]
    fn a_tripled_design_is_dense_and_keeps_one_gun_each() {
        let big = scale_design(&design(), 3, size);
        assert_eq!(big.hull_cells.len(), 15 * 9);
        assert_eq!(big.modules.iter().filter(|m| m.module_type == ModuleType::Cannon).count(), 1);
        assert_eq!(big.modules.iter().filter(|m| m.module_type == ModuleType::LargeCargo).count(), 18);
        let hull: std::collections::HashSet<IVec2> = big.hull_cells.iter().map(|c| c.grid_pos).collect();
        let mut used = std::collections::HashSet::new();
        for m in &big.modules {
            for c in crate::building::ShipGrid::cells_for(m.grid_pos, size(m.module_type), m.rotation, None) {
                assert!(hull.contains(&c), "{:?} at {} off the hull", m.module_type, c);
                assert!(used.insert(c), "{:?} overlaps at {}", m.module_type, c);
            }
        }
    }

    /// Only the new outside edge is Outer.
    #[test]
    fn only_the_new_edge_is_outer() {
        let big = scale_design(&design(), 3, size);
        let hull: std::collections::HashSet<IVec2> = big.hull_cells.iter().map(|c| c.grid_pos).collect();
        for c in &big.hull_cells {
            let edge = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y].iter().any(|d| !hull.contains(&(c.grid_pos + *d)));
            if c.layer == HullLayer::Outer {
                assert!(edge, "{} is Outer inside the hull", c.grid_pos);
            }
        }
    }
}

#[cfg(test)]
mod gun_placement_tests {
    use super::*;
    use crate::combat::gun_design::{GunDesign, GunDesignPart};

    #[test]
    fn a_fitted_gun_is_laid_down_as_its_parts() {
        let gun = GunDesign {
            name: "Little Gun".into(),
            description: String::new(),
            parts: vec![
                GunDesignPart { module_type: ModuleType::FiringPiece, offset: IVec2::ZERO, rotation: Rotation::East, turret: false },
                GunDesignPart { module_type: ModuleType::GunBarrel, offset: IVec2::X, rotation: Rotation::East, turret: false },
            ],
            shell: Some(crate::combat::shell::ShellDesign { head: crate::combat::shell::ShellHead::He, ..default() }),
            faction: None,
            point_defence: false,
        };
        let mut bp = Blueprint {
            name: "ship".into(), hull_cells: vec![], modules: vec![], created_at: String::new(), version: BLUEPRINT_VERSION,
            guns: vec![
                GunPlacement { design: "little_gun".into(), at: IVec2::new(3, 3), facing: Rotation::North, turret: Some(IVec2::new(1, 1)) },
                GunPlacement { design: "nothing by this name".into(), at: IVec2::ZERO, facing: Rotation::East, turret: None },
            ],
        };
        expand_gun_placements(&mut bp, &[gun]);
        assert!(bp.guns.is_empty());
        assert_eq!(bp.modules.len(), 2, "the unknown gun is left out");
        let barrel = bp.modules.iter().find(|m| m.module_type == ModuleType::GunBarrel).unwrap();
        assert_eq!((barrel.grid_pos, barrel.rotation), (IVec2::new(3, 4), Rotation::North));
        let fp = bp.modules.iter().find(|m| m.module_type == ModuleType::FiringPiece).unwrap();
        let extras = fp.extras.as_ref().unwrap();
        assert_eq!(extras.turret, Some(IVec2::new(1, 1)));
        assert_eq!(extras.shell.as_ref().unwrap().head, crate::combat::shell::ShellHead::He);
    }
}

#[cfg(test)]
mod turret_capture_tests {
    use super::*;
    use crate::combat::turret_deck::{OnTurret, TurretBody, TurretPlate};

    #[derive(Resource, Default)]
    struct Captured(Option<Blueprint>);

    fn module(t: ModuleType, at: IVec2) -> Module {
        Module {
            module_type: t, health: 1.0, max_health: 1.0, power_consumption: 0.0, power_generation: 0.0,
            is_active: true, grid_position: at, size: IVec2::ONE, rotation: Rotation::East,
        }
    }

    #[test]
    fn a_design_keeps_what_stands_on_its_turrets_and_not_their_plating() {
        let mut app = App::new();
        app.init_resource::<Captured>();
        app.add_systems(Update, |ship: Query<&Children, With<Ship>>, capture: DesignCapture, mut out: ResMut<Captured>| {
            out.0 = capture.capture(ship.single().unwrap(), "t");
        });
        let ship = app.world_mut().spawn(Ship).id();
        let ring = app.world_mut().spawn((module(ModuleType::TurretRing, IVec2::new(5, 1)), ChildOf(ship))).id();
        let body = app.world_mut().spawn((
            TurretBody::new(ring, IVec2::new(5, 1), 3),
            ChildOf(ship),
        )).id();
        app.world_mut().spawn((HullSegment { grid_position: IVec2::ZERO, ..default() }, TurretPlate, ChildOf(body)));
        let heat = crate::combat::shell::ShellDesign { head: crate::combat::shell::ShellHead::Heat, ..default() };
        app.world_mut().spawn((
            module(ModuleType::FiringPiece, IVec2::ZERO),
            OnTurret { ring_cell: IVec2::new(5, 1) },
            crate::combat::shell::GunShell(heat.clone()),
            ChildOf(body),
        ));
        app.update();

        let bp = app.world().resource::<Captured>().0.clone().unwrap();
        assert!(bp.hull_cells.is_empty(), "turret plating is the ring's, not the design's");
        assert_eq!(bp.modules.len(), 2);
        let fp = bp.modules.iter().find(|m| m.module_type == ModuleType::FiringPiece).unwrap();
        assert_eq!(fp.grid_pos, IVec2::ZERO);
        assert_eq!(fp.extras.as_ref().and_then(|e| e.turret), Some(IVec2::new(5, 1)));
        assert_eq!(fp.extras.as_ref().and_then(|e| e.shell.clone()), Some(heat), "its shell design too");
        let ring = bp.modules.iter().find(|m| m.module_type == ModuleType::TurretRing).unwrap();
        assert_eq!(ring.extras.as_ref().and_then(|e| e.turret), None);
    }
}
