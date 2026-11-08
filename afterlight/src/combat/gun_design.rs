//! Saved gun designs: a built gun as data.
//!
//! A design is the gun's parts, laid out relative to its Firing Piece as if
//! the gun faced the bow (+x), plus the shell it fires. Designs live one per
//! file in `designs/guns/*.json`, like ship designs: save one from a built
//! gun's window, stamp it into a hull or onto a turret deck in build mode
//! (T to pick, Enter to place), or write one by hand -- faction guns are
//! meant to be drawn, the same as faction ships.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::shell::ShellDesign;
use crate::components::{Module, ModuleType, Rotation};

pub const GUN_DESIGN_DIR: &str = "designs/guns";

/// Where gun designs are read and written. AFTERLIGHT_GUN_DESIGN_DIR moves it
/// (tests write somewhere that isn't the repo).
pub fn gun_design_dir() -> std::path::PathBuf {
    std::env::var("AFTERLIGHT_GUN_DESIGN_DIR").map(Into::into).unwrap_or_else(|_| GUN_DESIGN_DIR.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GunDesignPart {
    pub module_type: ModuleType,
    /// From the firing piece, with the gun facing +x.
    pub offset: IVec2,
    pub rotation: Rotation,
    /// A pre-made weapon's part that stands on the turret of the design's
    /// own ring (`WeaponKits`): `offset` is still from the design's origin,
    /// the hull cell it stands over.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub turret: bool,
}

/// One part of a design laid down (`GunDesign::install`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Installed {
    pub module_type: ModuleType,
    /// In the hull's grid, or for a turret part in its turret's own grid.
    pub cell: IVec2,
    /// The hull cell it stands over (`cell` itself for a hull part).
    pub hull_cell: IVec2,
    pub rotation: Rotation,
    /// For a turret part, the cell of the ring it stands on.
    pub ring: Option<IVec2>,
}

/// Whether a part is a turret ring.
pub fn is_ring(t: ModuleType) -> bool {
    matches!(t, ModuleType::TurretRingSmall | ModuleType::TurretRing | ModuleType::TurretRingLarge)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GunDesign {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Every part, the firing piece included (at 0, 0, facing East).
    pub parts: Vec<GunDesignPart>,
    /// None: the default shell.
    #[serde(default)]
    pub shell: Option<ShellDesign>,
    /// The faction whose ships carry it (an AiShipType name, e.g.
    /// "TerranHegemony"); None for the player's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction: Option<String>,
    /// Fitted on point defence: shoots incoming missiles, never ships
    /// (combat::missile_defence). On a laser unit, its emitters do.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub point_defence: bool,
}

/// Clockwise quarter turns from East.
fn turns(r: Rotation) -> u8 {
    match r {
        Rotation::East => 0,
        Rotation::South => 1,
        Rotation::West => 2,
        Rotation::North => 3,
    }
}

/// `v` turned `k` quarter turns clockwise.
fn turn(v: IVec2, k: u8) -> IVec2 {
    (0..k % 4).fold(v, |v, _| IVec2::new(v.y, -v.x))
}

fn turn_rot(r: Rotation, k: u8) -> Rotation {
    (0..k % 4).fold(r, |r, _| r.rotate_cw())
}

impl GunDesign {
    /// A design from a built gun's blocks (its firing piece among them),
    /// turned so it faces +x.
    pub fn capture(name: impl Into<String>, blocks: &[Module], shell: Option<ShellDesign>) -> Option<Self> {
        let fp = blocks.iter().find(|m| m.module_type == ModuleType::FiringPiece)?;
        let undo = (4 - turns(fp.rotation)) % 4;
        let mut parts: Vec<GunDesignPart> = blocks
            .iter()
            .map(|m| GunDesignPart {
                turret: false,
                module_type: m.module_type,
                offset: turn(m.grid_position - fp.grid_position, undo),
                rotation: turn_rot(m.rotation, undo),
            })
            .collect();
        parts.sort_by_key(|p| (p.offset.x, p.offset.y));
        Some(Self {
            name: name.into(),
            description: String::new(),
            parts,
            shell: shell.filter(|s| *s != ShellDesign::default()),
            faction: None,
            point_defence: false,
        })
    }

    /// The parts laid down with the firing piece at `origin`, facing `facing`.
    pub fn placed(&self, origin: IVec2, facing: Rotation) -> Vec<(ModuleType, IVec2, Rotation)> {
        let k = turns(facing);
        self.parts
            .iter()
            .map(|p| (p.module_type, origin + turn(p.offset, k), turn_rot(p.rotation, k)))
            .collect()
    }

    /// Every part laid down with the design's origin at `origin`, facing
    /// `facing`: hull parts in the hull's grid, a pre-made weapon's turret
    /// parts in the grid of its own ring's turret (centred on the ring, as
    /// a turret's grid is). None if it has turret parts but no ring.
    ///
    /// Turning the design turns its hull parts only. What stands on the
    /// turret keeps the way it was drawn: a turret swings its own +x onto
    /// the target, so its guns must face +x on the turret whichever way the
    /// ring was set down -- turned with the rest, a gun fired 90 or 180
    /// degrees off its turret's aim and never came on target.
    pub fn install(&self, origin: IVec2, facing: Rotation) -> Option<Vec<Installed>> {
        let placed = self.placed(origin, facing);
        let ring_centre = |at: IVec2, t: ModuleType, r: Rotation| {
            let size = IVec2::splat(super::turret_deck::ring_size(t));
            let cells = crate::building::ShipGrid::cells_for(at, size, r, None);
            cells.iter().fold(IVec2::ZERO, |a, c| a + *c) / cells.len().max(1) as i32
        };
        let ring = self.parts.iter().zip(&placed).find(|(p, (t, _, _))| !p.turret && is_ring(*t)).map(|(p, &(t, at, r))| {
            // The ring's centre as drawn, which the turret parts are drawn round.
            (at, ring_centre(at, t, r), ring_centre(p.offset, t, p.rotation))
        });
        self.parts
            .iter()
            .zip(placed)
            .map(|(p, (module_type, hull_cell, rotation))| {
                if !p.turret {
                    return Some(Installed { module_type, cell: hull_cell, hull_cell, rotation, ring: None });
                }
                let (ring_at, centre, drawn_centre) = ring?;
                let cell = p.offset - drawn_centre;
                Some(Installed { module_type, cell, hull_cell: centre + cell, rotation: p.rotation, ring: Some(ring_at) })
            })
            .collect()
    }

    /// Whether it's a pre-made weapon with parts on a turret of its own.
    pub fn has_turret_parts(&self) -> bool {
        self.parts.iter().any(|p| p.turret)
    }

    pub fn cost(&self, registry: &crate::building::ModuleRegistry) -> u32 {
        self.parts.iter().map(|p| registry.get(p.module_type).cost).sum()
    }

    /// What it adds up to, worked out the same way a built one is.
    pub fn assembled(&self) -> Option<super::gun_assembly::GunAssembly> {
        let modules: Vec<(Entity, Module)> = self
            .placed(IVec2::ZERO, Rotation::East)
            .into_iter()
            .enumerate()
            .map(|(i, (t, at, r))| {
                let m = Module {
                    module_type: t, health: 1.0, max_health: 1.0, power_consumption: 0.0, power_generation: 0.0,
                    is_active: true, grid_position: at, size: IVec2::ONE, rotation: r,
                };
                (Entity::from_raw_u32(i as u32 + 1).unwrap(), m)
            })
            .collect();
        let shells = self
            .shell
            .clone()
            .and_then(|s| modules.iter().find(|(_, m)| m.module_type == ModuleType::FiringPiece).map(|(e, _)| (*e, s)))
            .into_iter()
            .collect();
        super::gun_assembly::assemble_with_shells(&modules, &shells).into_iter().next().map(|(_, g)| g)
    }

    /// A laser unit (a Laser Core among the parts): what its bank adds up to.
    pub fn laser(&self) -> Option<(super::lasers::BankStats, super::lasers::BankLayout)> {
        let placed = self.placed(IVec2::ZERO, Rotation::East);
        let core = placed.iter().find(|(t, _, _)| *t == ModuleType::LaserCore)?.1;
        let cells: bevy::platform::collections::HashMap<IVec2, ModuleType> = placed.iter().map(|(t, at, _)| (*at, *t)).collect();
        let layout = super::lasers::lay_out_bank(&cells, core);
        let stats = super::lasers::bank_stats(layout.cavities.len() as u32, layout.q_switches, layout.doublers, layout.destabilisers);
        Some((stats, layout))
    }

    /// Whether this part carries the design's point defence switch: a gun's
    /// firing piece, or a laser unit's emitters.
    pub fn point_defence_on(&self, part: ModuleType) -> bool {
        self.point_defence && matches!(part, ModuleType::FiringPiece | ModuleType::LaserEmitter)
    }

    /// "280 mm, 6 m barrel, 3 loaders, 27.8 rpm".
    pub fn summary(&self) -> String {
        match self.assembled() {
            Some(g) => format!(
                "{:.0} mm, {} m barrel, {} loader(s), {:.0} m/s, {:.1} rpm, {} parts",
                g.stats.gauge_mm, g.parts.barrel_m, g.parts.loaders.len(), g.stats.velocity_mps, g.stats.rpm, self.parts.len()
            ),
            None => match self.laser() {
                Some((stats, layout)) => format!(
                    "laser, {} cavities, {} pumps, {}, intensity {:.0}, {} emitter(s), {} parts",
                    layout.cavities.len(),
                    layout.pumps.len(),
                    if stats.shots == 0 { "steady beam".to_string() } else { format!("{} pulses/s", stats.shots) },
                    stats.intensity,
                    layout.emitters.len(),
                    self.parts.len()
                ),
                None => format!("{} parts, no firing piece or laser core", self.parts.len()),
            },
        }
    }

    /// A file name from the design's name.
    pub fn file_stem(&self) -> String {
        let s: String = self
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let s = s.trim_matches('_').to_string();
        if s.is_empty() { "gun".into() } else { s }
    }
}

/// Every gun design on disk, by name. Re-read when one is saved.
#[derive(Resource, Default, Debug)]
pub struct GunDesigns(pub Vec<GunDesign>);

pub fn read_gun_designs() -> Vec<GunDesign> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir(gun_design_dir()) else { return out };
    let mut paths: Vec<_> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    paths.sort();
    for path in paths {
        match std::fs::read_to_string(&path).map(|s| serde_json::from_str::<GunDesign>(&s)) {
            Ok(Ok(d)) => out.push(d),
            Ok(Err(e)) => warn!("gun design {:?} doesn't read: {}", path, e),
            Err(e) => warn!("gun design {:?}: {}", path, e),
        }
    }
    out
}

/// Write a design to `designs/guns/<name>.json`, never over another: a name
/// already taken gets a number.
pub fn write_gun_design(design: &GunDesign) -> std::io::Result<std::path::PathBuf> {
    let dir = gun_design_dir();
    std::fs::create_dir_all(&dir)?;
    let stem = design.file_stem();
    let mut path = dir.join(format!("{stem}.json"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem}_{n}.json"));
        n += 1;
    }
    let json = serde_json::to_string_pretty(design).map_err(std::io::Error::other)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Pre-made weapons for the builder's Pre-made tab: whole installations --
/// a turret's ring, motors, hoist and magazine with its gun on top, a
/// battery of launchers, a laser unit -- that a beginner can put down in
/// one go. Same file format as gun designs; read from `designs/kits`, in
/// file-name order.
#[derive(Resource, Default, Debug)]
pub struct WeaponKits(pub Vec<GunDesign>);

pub const KIT_DIR: &str = "designs/kits";

pub fn read_kits(dir: &std::path::Path) -> Vec<GunDesign> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    paths.sort();
    for path in paths {
        match std::fs::read_to_string(&path).map(|s| serde_json::from_str::<GunDesign>(&s)) {
            Ok(Ok(d)) => out.push(d),
            Ok(Err(e)) => warn!("weapon kit {:?} doesn't read: {}", path, e),
            Err(e) => warn!("weapon kit {:?}: {}", path, e),
        }
    }
    out
}

pub fn load_weapon_kits(mut kits: ResMut<WeaponKits>) {
    kits.0 = read_kits(std::path::Path::new(KIT_DIR));
    info!("{} pre-made weapons in {KIT_DIR}", kits.0.len());
}

pub fn load_gun_designs(mut designs: ResMut<GunDesigns>) {
    designs.0 = read_gun_designs();
    info!("{} gun designs in {:?}", designs.0.len(), gun_design_dir());
}

#[cfg(test)]
mod gun_design_tests {
    use super::*;
    use ModuleType::*;

    fn part(t: ModuleType, x: i32, y: i32) -> GunDesignPart {
        GunDesignPart { module_type: t, offset: IVec2::new(x, y), rotation: Rotation::East, turret: false }
    }

    /// Every pre-made weapon in designs/kits works as laid down, whichever
    /// way it's turned: parts don't overlap, a turret has motors and a hoist
    /// reaching magazines, its gun assembles and loads, a laser unit has
    /// cavities, pumps and an emitter.
    #[test]
    fn every_pre_made_weapon_works() {
        use crate::building::turret_build::{allowed_on_turret, may_overhang};
        use crate::combat::turret_deck::{deck_cells, ring_size};
        let kits = read_kits(std::path::Path::new(KIT_DIR));
        assert!(kits.len() >= 10, "the builder's Pre-made tab needs its kits");
        let registry = crate::building::registry::build_registry();
        for kit in &kits {
            for facing in [Rotation::East, Rotation::South, Rotation::West, Rotation::North] {
                let what = format!("{} facing {facing:?}", kit.name);
                let parts = kit.install(IVec2::new(10, 10), facing).unwrap_or_else(|| panic!("{what}: no ring"));
                let module = |i: &Installed| Module {
                    module_type: i.module_type, health: 1.0, max_health: 1.0, power_consumption: 0.0, power_generation: 0.0,
                    is_active: true, grid_position: i.cell, size: registry.get(i.module_type).size, rotation: i.rotation,
                };
                let mut taken = std::collections::HashSet::new();
                for i in parts.iter().filter(|i| i.ring.is_none()) {
                    for c in crate::building::ShipGrid::cells_for(i.cell, registry.get(i.module_type).size, i.rotation, None) {
                        assert!(taken.insert(c), "{what}: two parts on {c}");
                    }
                }
                if let Some(ring) = parts.iter().find(|i| is_ring(i.module_type)) {
                    let hull: Vec<(Entity, Module)> = parts
                        .iter()
                        .filter(|i| i.ring.is_none())
                        .enumerate()
                        .map(|(n, i)| (Entity::from_raw_u32(n as u32 + 1).unwrap(), module(i)))
                        .collect();
                    let ring_cells = crate::combat::turret_deck::ring_cells(&module(ring));
                    let motors = hull
                        .iter()
                        .filter(|(_, m)| m.module_type == ModuleType::TraverseMotor)
                        .filter(|(_, m)| ring_cells.iter().any(|r| (m.grid_position - *r).abs().max_element() <= 1))
                        .count();
                    assert!(motors > 0, "{what}: no motor turns it");
                    // A laser turret is fed light through a Beam Relay, a gun
                    // turret shells through a hoist.
                    if parts.iter().any(|i| i.ring.is_some() && i.module_type == ModuleType::LaserEmitter) {
                        let relay = hull.iter().any(|(_, m)| {
                            m.module_type == ModuleType::BeamRelay
                                && ring_cells.iter().any(|r| (m.grid_position - *r).abs().element_sum() == 1)
                        });
                        assert!(relay, "{what}: no Beam Relay beside the ring");
                    } else {
                        let feed = super::super::gun_assembly::hoist_reach(&hull, &ring_cells);
                        assert!(!feed.magazines.is_empty(), "{what}: the hoist reaches no magazine");
                    }
                    let deck: Vec<IVec2> = deck_cells(ring_size(ring.module_type));
                    let mut on_turret = std::collections::HashSet::new();
                    for i in parts.iter().filter(|i| i.ring.is_some()) {
                        if i.module_type == ModuleType::FiringPiece {
                            assert_eq!(i.rotation, Rotation::East, "{what}: its gun faces off the turret's aim");
                        }
                        assert!(allowed_on_turret(i.module_type), "{what}: {:?} can't go on a turret", i.module_type);
                        assert!(may_overhang(i.module_type) || deck.contains(&i.cell), "{what}: {:?} at {} is off the deck", i.module_type, i.cell);
                        assert!(on_turret.insert(i.cell), "{what}: two turret parts on {}", i.cell);
                    }
                }
                // Its gun, wherever it stands, assembles and loads.
                let gun = GunDesign {
                    parts: parts
                        .iter()
                        .filter(|i| !is_ring(i.module_type))
                        .map(|i| GunDesignPart { module_type: i.module_type, offset: i.cell, rotation: i.rotation, turret: false })
                        .collect(),
                    ..kit.clone()
                };
                let guns_on = |on_turret: bool| -> Vec<GunDesignPart> {
                    gun.parts.iter().zip(parts.iter().filter(|i| !is_ring(i.module_type))).filter(|(_, i)| i.ring.is_some() == on_turret).map(|(p, _)| *p).collect()
                };
                for on_turret in [true, false] {
                    let set = guns_on(on_turret);
                    if set.iter().any(|p| p.module_type == ModuleType::FiringPiece) {
                        let g = GunDesign { parts: set, ..kit.clone() }.assembled().unwrap_or_else(|| panic!("{what}: the gun doesn't assemble"));
                        assert!(g.stats.working_loaders > 0, "{what}: no loader takes its shell");
                        assert!(g.parts.barrel_m > 0, "{what}: no barrel");
                    }
                }
                if let Some((stats, layout)) = kit.laser() {
                    // Its emitter on the hull, or up on its turret through a relay.
                    assert!(stats.storage > 0.0 && !layout.pumps.is_empty() && !(layout.emitters.is_empty() && layout.relays.is_empty()), "{what}: laser {layout:?}");
                }
            }
        }
    }

    /// Every ship in the game (each faction's classes and fallback ship, the
    /// player's starters) carries working weapons: every turret part stands
    /// on a ring of its own ship, every ring has a motor and a hoist that
    /// reaches a magazine, and every turret's gun assembles and loads.
    #[test]
    fn every_ships_turrets_work() {
        use crate::building::blueprint::load_design_file;
        let registry = crate::building::registry::build_registry();
        let mut files: Vec<String> = ["starter", "pincer", "crab", "trident"].iter().map(|n| format!("designs/{n}.json")).collect();
        for entry in std::fs::read_dir("designs/factions").unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(std::fs::read_dir(&path).unwrap().flatten().map(|e| e.path().display().to_string()));
            } else {
                files.push(path.display().to_string());
            }
        }
        let mut turrets_seen = 0;
        for file in files.iter().filter(|f| f.ends_with(".json")) {
            let design = load_design_file(file).unwrap_or_else(|| panic!("{file} doesn't load"));
            let hull: Vec<(Entity, Module)> = design
                .modules
                .iter()
                .filter(|m| m.extras.as_ref().is_none_or(|e| e.turret.is_none()))
                .enumerate()
                .map(|(n, m)| {
                    let module = Module {
                        module_type: m.module_type, health: 1.0, max_health: 1.0, power_consumption: 0.0, power_generation: 0.0,
                        is_active: true, grid_position: m.grid_pos, size: registry.get(m.module_type).size, rotation: m.rotation,
                    };
                    (Entity::from_raw_u32(n as u32 + 1).unwrap(), module)
                })
                .collect();
            for (_, ring) in hull.iter().filter(|(_, m)| is_ring(m.module_type)) {
                turrets_seen += 1;
                let what = format!("{file}: ring at {}", ring.grid_position);
                let cells = crate::combat::turret_deck::ring_cells(ring);
                let motors = hull
                    .iter()
                    .filter(|(_, m)| m.module_type == ModuleType::TraverseMotor)
                    .any(|(_, m)| cells.iter().any(|r| (m.grid_position - *r).abs().max_element() <= 1));
                assert!(motors, "{what}: no motor");
                let on_top: Vec<ModuleType> = design
                    .modules
                    .iter()
                    .filter(|m| m.extras.as_ref().and_then(|e| e.turret) == Some(ring.grid_position))
                    .map(|m| m.module_type)
                    .collect();
                if on_top.contains(&ModuleType::LaserEmitter) {
                    // A laser turret: fed through a Beam Relay beside its ring.
                    let relay = hull.iter().any(|(_, m)| {
                        m.module_type == ModuleType::BeamRelay && cells.iter().any(|r| (m.grid_position - *r).abs().element_sum() == 1)
                    });
                    assert!(relay, "{what}: a laser turret with no Beam Relay");
                    continue;
                }
                assert!(!super::super::gun_assembly::hoist_reach(&hull, &cells).magazines.is_empty(), "{what}: no magazine fed");
                let parts: Vec<GunDesignPart> = design
                    .modules
                    .iter()
                    .filter(|m| m.extras.as_ref().and_then(|e| e.turret) == Some(ring.grid_position))
                    .map(|m| GunDesignPart { module_type: m.module_type, offset: m.grid_pos, rotation: m.rotation, turret: false })
                    .collect();
                // A turret swings its +x onto the target: its gun has to face +x.
                for p in parts.iter().filter(|p| p.module_type == ModuleType::FiringPiece) {
                    assert_eq!(p.rotation, Rotation::East, "{what}: its gun faces off the turret's aim");
                }
                let gun = GunDesign { name: what.clone(), description: String::new(), parts, shell: None, faction: None, point_defence: false };
                let g = gun.assembled().unwrap_or_else(|| panic!("{what}: no gun on it"));
                assert!(g.stats.working_loaders > 0 && g.parts.barrel_m > 0, "{what}: the gun doesn't load or has no barrel");
            }
            let rings: Vec<IVec2> = hull.iter().filter(|(_, m)| is_ring(m.module_type)).map(|(_, m)| m.grid_position).collect();
            for m in design.modules.iter().filter_map(|m| m.extras.as_ref().and_then(|e| e.turret).map(|r| (m, r))) {
                assert!(rings.contains(&m.1), "{file}: {:?} on a turret at {} with no ring", m.0.module_type, m.1);
            }
        }
        assert!(turrets_seen > 500, "the fleet carries its new turrets ({turrets_seen})");
    }

    #[test]
    fn a_laser_unit_design_reads_as_a_laser() {
        let d = GunDesign {
            name: "PD laser".into(),
            description: String::new(),
            parts: vec![
                part(LaserCore, 0, 0),
                part(LaserCavity, 0, 1),
                part(LaserPump, 0, 2),
                part(QSwitch, -1, 0),
                part(QSwitch, 0, -1),
                part(LaserEmitter, 1, 0),
                part(LaserLens, 2, 0),
            ],
            shell: None,
            faction: None,
            point_defence: true,
        };
        let (stats, layout) = d.laser().expect("a core makes it a laser");
        assert_eq!((layout.cavities.len(), layout.pumps.len(), layout.emitters.len()), (1, 1, 1));
        assert_eq!(stats.shots, 2, "two Q-switches pulse twice a second");
        assert!(d.summary().starts_with("laser, 1 cavities, 1 pumps, 2 pulses/s"), "{}", d.summary());
        assert!(d.point_defence_on(LaserEmitter) && !d.point_defence_on(LaserCore));
        let gun = GunDesign { parts: vec![part(FiringPiece, 0, 0)], ..d.clone() };
        assert!(gun.laser().is_none() && gun.point_defence_on(FiringPiece));
    }

    fn m(t: ModuleType, x: i32, y: i32, r: Rotation) -> Module {
        Module {
            module_type: t, health: 1.0, max_health: 1.0, power_consumption: 0.0, power_generation: 0.0,
            is_active: true, grid_position: IVec2::new(x, y), size: IVec2::ONE, rotation: r,
        }
    }

    /// A small gun facing North (barrel up the +y axis) at (5, 5).
    fn north_gun() -> Vec<Module> {
        vec![
            m(FiringPiece, 5, 5, Rotation::North),
            m(GunBarrel, 5, 6, Rotation::North),
            m(GunBarrel, 5, 7, Rotation::North),
            m(Loader, 5, 4, Rotation::North),
            m(GaugeIncreaser, 4, 5, Rotation::North),
        ]
    }

    #[test]
    fn a_captured_gun_is_stored_facing_the_bow() {
        let d = GunDesign::capture("test", &north_gun(), None).unwrap();
        let fp = d.parts.iter().find(|p| p.module_type == FiringPiece).unwrap();
        assert_eq!((fp.offset, fp.rotation), (IVec2::ZERO, Rotation::East));
        assert!(d.parts.contains(&GunDesignPart { module_type: GunBarrel, offset: IVec2::new(2, 0), rotation: Rotation::East, turret: false }));
        assert!(d.parts.contains(&GunDesignPart { module_type: Loader, offset: IVec2::new(-1, 0), rotation: Rotation::East, turret: false }));
        // Left of a north-facing gun is port; facing the bow, port is +y.
        assert!(d.parts.iter().any(|p| p.module_type == GaugeIncreaser && p.offset == IVec2::new(0, 1)));
    }

    #[test]
    fn placing_turns_it_back_whichever_way_it_faces() {
        let original = north_gun();
        let d = GunDesign::capture("test", &original, None).unwrap();
        let mut back = d.placed(IVec2::new(5, 5), Rotation::North);
        let mut want: Vec<_> = original.iter().map(|m| (m.module_type, m.grid_position, m.rotation)).collect();
        back.sort_by_key(|(_, c, _)| (c.x, c.y));
        want.sort_by_key(|(_, c, _)| (c.x, c.y));
        assert_eq!(back, want);
        let west = d.placed(IVec2::ZERO, Rotation::West);
        assert!(west.contains(&(GunBarrel, IVec2::new(-2, 0), Rotation::West)));
    }

    #[test]
    fn a_design_adds_up_like_the_gun_it_came_from() {
        let d = GunDesign::capture("test", &north_gun(), None).unwrap();
        let g = d.assembled().unwrap();
        assert_eq!(g.stats.gauge_mm, 115.0);
        assert_eq!(g.parts.barrel_m, 2);
    }

    #[test]
    fn the_default_shell_isnt_written_down() {
        let d = GunDesign::capture("test", &north_gun(), Some(ShellDesign::default())).unwrap();
        assert_eq!(d.shell, None);
    }

    #[test]
    fn a_design_from_the_gun_foundry_reads() {
        // What tools/gun-editor saves: the game's fields plus its own.
        let json = r#"{"name":"T","description":"","parts":[{"module_type":"FiringPiece","offset":[0,0],"rotation":"East"}],
            "shell":{"base":"Tracer","casings":3,"bodies":["He"],"head":"Ap"},"faction":"TerranHegemony","savedAt":"2026-10-26T10:00:00Z"}"#;
        let d: GunDesign = serde_json::from_str(json).unwrap();
        assert_eq!(d.faction.as_deref(), Some("TerranHegemony"));
        assert_eq!(d.shell.unwrap().casings, 3);
    }

    #[test]
    fn names_become_safe_file_names() {
        let d = GunDesign { name: "280 mm HE / Terran!".into(), description: String::new(), parts: vec![], shell: None, faction: None, point_defence: false };
        assert_eq!(d.file_stem(), "280_mm_he___terran");
    }
}
