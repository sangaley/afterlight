//! Guns built from parts.
//!
//! A gun is a Firing Piece plus the parts it can reach. In front, along the
//! way it faces: an optional Mantlet, then a straight line of Barrels, the
//! last of which may be a Muzzle Brake; Rail Magnets sit beside the barrel.
//! Behind and beside it, touching it or each other: Gauge Increasers,
//! Breech Coolers, Recoil Absorbers, Loaders, Clip Racks and Shell Feeders.
//! A straight run of Loader blocks (along the way they face) is one loader,
//! 1 m long per block; Clip Racks count toward the loader they touch.
//!
//! A turret's guns are fed from below: an Ammo Hoist on the hull deck beside
//! the ring lifts shells up from the magazines it reaches, along Belt
//! Conveyors and Anti-Flash Doors. A cook-off on the turret flashes back down
//! the hoist into any magazine it reaches without passing a door
//! (`flash_down_hoists`).
//!
//! Worked out per ship whenever its layout changes (building::layout), and
//! written into the firing piece's BaseWeaponStats so the existing firing,
//! tuning and enhancer systems use it unchanged. The numbers come from
//! combat::gun_math, the same formulas the Gun Deck preview shows.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use std::collections::VecDeque;

use crate::building::layout::{BuiltFrom, LayoutEpoch, LayoutVersion};
use crate::building::multiblock::components::BaseWeaponStats;
use crate::components::{DestroyedModule, Module, ModuleType, WeaponMount};

use super::gun_math::{self, GunParts, GunStats, Loader};

/// What the parts around a firing piece add up to. On the firing piece.
#[derive(Component, Clone, Debug, Default)]
pub struct GunAssembly {
    pub parts: GunParts,
    pub stats: GunStats,
    /// Every block that belongs to this gun, the firing piece included.
    pub members: Vec<Entity>,
    /// Problems worth telling the builder, in plain words.
    pub problems: Vec<String>,
    /// Ship-local cell the round leaves from: just past the last barrel.
    pub muzzle_cell: IVec2,
    pub feeders: u32,
    /// Magazines this gun's feeders reach (directly or along belt
    /// conveyors), and the shells of them that are this gun's share.
    pub magazines: Vec<Entity>,
    pub magazine_shells: u32,
    /// Shells a minute the feeders can bring up from the magazines. Zero
    /// with no magazine reached.
    pub feed_rpm: f32,
    /// The ship this gun is on -- through its turret, for a turret gun.
    pub ship: Option<Entity>,
    /// Stands on a turret deck, fed by the hoist rather than by feeders.
    pub on_turret: bool,
}

/// On a turret: what its hoist reaches on the hull deck below.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct TurretFeed {
    pub hoist: Option<Entity>,
    /// Every magazine the hoist draws on.
    pub magazines: Vec<Entity>,
    /// The ones a flash down the hoist would reach: no Anti-Flash Door
    /// between them and it.
    pub flash: Vec<Entity>,
}

/// Where a built gun's round leaves from and which way its barrel points,
/// in world space, refreshed every frame (a turret gun moves with its
/// turret). The firing system reads this instead of the block's position.
#[derive(Component, Clone, Copy, Debug)]
pub struct GunMuzzle {
    pub world: Vec2,
    pub dir: Vec2,
}

impl GunAssembly {
    /// Every shell the gun can carry: its ready rack plus its share of the
    /// magazines behind it.
    pub fn max_ammo(&self) -> u32 {
        self.stats.ready_rack + self.magazine_shells
    }
}

/// A gun's ready rack as it empties and refills: shells in the loaders and
/// clips right now. While there are any the gun fires at its loaders' pace;
/// with the rack empty it can only fire as fast as the feeders bring shells
/// up from the magazine. War Thunder's first-stage rack.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct GunRack {
    pub ready: f32,
    pub last_ammo: u32,
}

/// AFTERLIGHT_GUN_TRACE=1: log each gun as it is assembled, and each shot.
pub fn trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("AFTERLIGHT_GUN_TRACE").ok().as_deref() == Some("1"))
}

/// Game units per second for each metre per second of muzzle velocity.
/// 850 m/s -- a full burn -- flies at 2,550 u/s, between the old cannon and
/// railgun speeds once the range they reach is taken into account.
///
/// Tripled with `RANGE_SCALE`: fights moved out from 7 km to 20, and a shell
/// has to arrive in about the same time or nothing that moves gets hit.
pub const GAME_SPEED_PER_MPS: f32 = 3.0 * RANGE_SCALE;
/// How much further every weapon reaches, and how much faster what it fires
/// flies, than the numbers they were first tuned with. Fights happen at 20 km:
/// the starter ship is 3.4 km long, and 7 km was two ship-lengths.
pub const RANGE_SCALE: f32 = 3.0;
/// The arc a spinal gun can fire in: it has no turret, you aim the ship.
pub const SPINAL_ARC_DEG: f32 = 8.0;
/// Metres of shell one Magazine block holds: big shells eat storage. At 20
/// a destroyer's turrets ran dry inside a minute and a half of a capital
/// fight.
pub const MAGAZINE_SHELL_M: f32 = 80.0;
/// Shells a minute an Ammo Hoist lifts into its turret: one every 1.5 s,
/// shared by the guns up there.
pub const HOIST_RPM: f32 = 40.0;
/// Shells a minute one Shell Feeder brings up from a magazine.
pub const FEEDER_RPM: f32 = 40.0;

/// Metres of shell a minute an Ammo Hoist lifts. Small shells go up many
/// at a time, so a point defence gun isn't held to a big gun's 40 a minute:
/// a 20 mm round (0.08 m) comes up 900 a minute. A big gun fires slower than
/// its hoist either way.
pub const HOIST_SHELL_M_PER_MIN: f32 = 72.0;

/// Shells a minute a hoist lifts of this length: never fewer than 40.
pub fn hoist_rpm(shell_m: f32) -> f32 {
    HOIST_RPM.max(HOIST_SHELL_M_PER_MIN / shell_m.max(0.01))
}

/// A built gun's round as drawn (effects/round.png, four times as long as
/// it's wide), sized by the shell: a 170 mm round about a third of a block,
/// 500 mm bigger, a 20 mm round a fleck. Was nearly twice this, and read as
/// fat orange dashes.
pub fn round_size(gauge_mm: f32) -> Vec2 {
    let small = (gauge_mm / 100.0).clamp(0.3, 1.0);
    let length = (10.0 + gauge_mm * 0.07) * small;
    Vec2::new(length, length * 0.25)
}

/// How far a gun reaches. A light round sheds its speed and wanders, so reach
/// goes with the square root of the bore against 150 mm, but not far: fights
/// are at 20 km and a 60 mm autocannon (20 km) has to take part, a 170 mm gun
/// reaching 23.6.
pub fn range_for(stats: &GunStats) -> f32 {
    let bore = (stats.gauge_mm / 150.0).sqrt().clamp(0.9, 1.15);
    (4000.0 + stats.velocity_mps * 4.0) * bore * stats.range_mult.max(1.0) * RANGE_SCALE
}

/// What a built gun's round does against what its shell design adds up to.
/// Ships were drawn twice as big, four times the blocks: at 1x a fight at
/// range took a big ship 2-9% of its hull in two minutes, half an hour to a
/// kill (combat lab, 2026-10-29). Tuned for a mid-size warship to die in
/// two or three minutes of focused fire (the user's call): 6x and 10x
/// made the enemy's guns as deadly, and a sniper fight is decided by range
/// more than by damage.
pub const BUILT_DAMAGE_SCALE: f32 = 4.0;

/// BUILT_DAMAGE_SCALE, or AFTERLIGHT_GUN_DAMAGE for tuning runs (tools/combat-lab).
pub fn built_damage_scale() -> f32 {
    static V: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_GUN_DAMAGE").ok().and_then(|v| v.parse().ok()).unwrap_or(BUILT_DAMAGE_SCALE))
}

/// Projectile caliber scale (combat::new_projectiles::caliber_scale), from
/// gauge: a 150 mm gun is 1.0, a 500 mm one 3.3.
pub fn caliber_for(stats: &GunStats) -> f32 {
    (stats.gauge_mm / 150.0).clamp(0.4, 3.4)
}

/// Penetration in the units ammo and armour use (steel plate is 15-80).
pub fn penetration_for(stats: &GunStats) -> f32 {
    stats.penetration_mm * 0.25
}

fn is_body(t: ModuleType) -> bool {
    matches!(
        t,
        ModuleType::GaugeIncreaser
            | ModuleType::GaugeReducer
            | ModuleType::Loader
            | ModuleType::ClipRack
            | ModuleType::ShellFeeder
            | ModuleType::BreechCooler
            | ModuleType::RecoilAbsorber
    )
}

/// What carries shells along the hull deck: belts, and the doors set in them.
fn is_feedway(t: Option<ModuleType>) -> bool {
    matches!(t, Some(ModuleType::BeltConveyor | ModuleType::AntiFlashDoor))
}

/// One ship's guns, from its live modules: (firing piece, assembly), with
/// the magazines shared out between them. Pure, so it can be tested without
/// a world.
pub fn assemble(modules: &[(Entity, Module)]) -> Vec<(Entity, GunAssembly)> {
    assemble_with_shells(modules, &HashMap::new())
}

/// `assemble`, with each firing piece's shell design.
pub fn assemble_with_shells(
    modules: &[(Entity, Module)],
    shells: &HashMap<Entity, crate::combat::shell::ShellDesign>,
) -> Vec<(Entity, GunAssembly)> {
    let mut guns = assemble_parts(modules, shells);
    share_magazines(&mut guns);
    guns
}

/// What an Ammo Hoist beside this ring reaches on the hull deck: the
/// magazines it can draw on along belts and doors, and those a flash could
/// reach -- along belts alone.
pub fn hoist_reach(hull: &[(Entity, Module)], ring_cells: &[IVec2]) -> TurretFeed {
    let kind: HashMap<IVec2, (Entity, ModuleType)> =
        hull.iter().map(|(e, m)| (m.grid_position, (*e, m.module_type))).collect();
    let mut hoists: Vec<IVec2> = ring_cells
        .iter()
        .flat_map(|c| [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y].map(|d| *c + d))
        .filter(|c| !ring_cells.contains(c))
        .filter(|c| kind.get(c).is_some_and(|(_, t)| *t == ModuleType::AmmoHoist))
        .collect();
    hoists.sort_by_key(|c| (c.x, c.y));
    let Some(hoist) = hoists.first().copied() else { return TurretFeed::default() };
    let walk = |through: &dyn Fn(Option<ModuleType>) -> bool| -> Vec<Entity> {
        let mut reached: Vec<IVec2> = Vec::new();
        let mut seen: HashSet<IVec2> = HashSet::from_iter([hoist]);
        let mut queue: VecDeque<IVec2> = VecDeque::from([hoist]);
        while let Some(c) = queue.pop_front() {
            for step in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let n = c + step;
                let t = kind.get(&n).map(|(_, t)| *t);
                if t == Some(ModuleType::Magazine) {
                    // Magazines side by side are one store: the walk carries
                    // on through them to the ones behind (and so does a flash).
                    if !reached.contains(&n) {
                        reached.push(n);
                    }
                    if seen.insert(n) {
                        queue.push_back(n);
                    }
                } else if through(t) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        reached.sort_by_key(|c| (c.x, c.y));
        reached.iter().map(|c| kind[c].0).collect()
    };
    TurretFeed {
        hoist: Some(kind[&hoist].0),
        magazines: walk(&|t| is_feedway(t)),
        flash: walk(&|t| t == Some(ModuleType::BeltConveyor)),
    }
}

/// The guns among these modules, before their magazines are shared out.
/// `shells`: each firing piece's shell design; one not listed fires the
/// default shell.
fn assemble_parts(modules: &[(Entity, Module)], shells: &HashMap<Entity, crate::combat::shell::ShellDesign>) -> Vec<(Entity, GunAssembly)> {
    let mut at: HashMap<IVec2, (Entity, ModuleType, crate::components::Rotation)> = HashMap::new();
    for (e, m) in modules {
        at.insert(m.grid_position, (*e, m.module_type, m.rotation));
    }
    let mut firing: Vec<(Entity, IVec2, crate::components::Rotation)> = modules
        .iter()
        .filter(|(_, m)| m.module_type == ModuleType::FiringPiece)
        .map(|(e, m)| (*e, m.grid_position, m.rotation))
        .collect();
    // Deterministic: the same ship always assembles the same way.
    firing.sort_by_key(|(_, c, _)| (c.x, c.y));

    let mut claimed: HashSet<IVec2> = HashSet::new();
    let mut out: Vec<(Entity, GunAssembly)> = Vec::new();
    let mut barrel_lines: HashMap<Entity, (Vec<IVec2>, bool, bool)> = HashMap::new();

    // 1. Barrel lines first: they're unambiguous, straight out the front.
    for &(fp, cell, rot) in &firing {
        claimed.insert(cell);
        let dir = rot.facing_offset();
        let mut c = cell + dir;
        let mut mantlet = false;
        if matches!(at.get(&c), Some((_, ModuleType::Mantlet, _))) && !claimed.contains(&c) {
            mantlet = true;
            claimed.insert(c);
            c += dir;
        }
        let mut barrel = Vec::new();
        let mut brake = false;
        while let Some((_, t, _)) = at.get(&c) {
            if claimed.contains(&c) || !matches!(t, ModuleType::GunBarrel | ModuleType::MuzzleBrake) {
                break;
            }
            claimed.insert(c);
            barrel.push(c);
            if *t == ModuleType::MuzzleBrake {
                brake = true;
                break;
            }
            c += dir;
        }
        barrel_lines.insert(fp, (barrel, mantlet, brake));
    }

    // 2. Body parts: a breadth-first search from every firing piece at once,
    // so a part touching two guns belongs to the nearer one.
    let mut owner: HashMap<IVec2, Entity> = HashMap::new();
    let mut queue: VecDeque<(IVec2, Entity)> = VecDeque::new();
    for &(fp, cell, _) in &firing {
        owner.insert(cell, fp);
        queue.push_back((cell, fp));
    }
    while let Some((c, fp)) = queue.pop_front() {
        for step in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let n = c + step;
            if owner.contains_key(&n) || claimed.contains(&n) {
                continue;
            }
            if let Some((_, t, _)) = at.get(&n) {
                if is_body(*t) {
                    owner.insert(n, fp);
                    queue.push_back((n, fp));
                }
            }
        }
    }

    for &(fp, cell, rot) in &firing {
        let (barrel, mantlet, brake) = barrel_lines.remove(&fp).unwrap_or_default();
        let dir = rot.facing_offset();
        let mine: Vec<IVec2> = owner.iter().filter(|(c, e)| **e == fp && **c != cell).map(|(c, _)| *c).collect();
        let kind = |c: &IVec2| at.get(c).map(|(_, t, _)| *t);
        let mut members = vec![fp];
        members.extend(mine.iter().filter_map(|c| at.get(c).map(|(e, _, _)| *e)));
        members.extend(barrel.iter().filter_map(|c| at.get(c).map(|(e, _, _)| *e)));
        if mantlet {
            if let Some((e, _, _)) = at.get(&(cell + dir)) {
                members.push(*e);
            }
        }

        // Rail magnets: beside any barrel cell.
        let side = IVec2::new(-dir.y, dir.x);
        let mut magnets = 0;
        for b in &barrel {
            for s in [side, -side] {
                let c = *b + s;
                if kind(&c) == Some(ModuleType::RailMagnet) && claimed.insert(c) {
                    magnets += 1;
                    if let Some((e, _, _)) = at.get(&c) {
                        members.push(*e);
                    }
                }
            }
        }

        let count = |t: ModuleType| mine.iter().filter(|c| kind(c) == Some(t)).count() as u32;
        let gauge = count(ModuleType::GaugeIncreaser);
        let reducers = count(ModuleType::GaugeReducer);
        let coolers = count(ModuleType::BreechCooler);
        let absorbers = count(ModuleType::RecoilAbsorber);
        let feeders = count(ModuleType::ShellFeeder);

        // Loaders: straight runs of Loader blocks along the way they face.
        let loader_cells: HashSet<IVec2> = mine.iter().copied().filter(|c| kind(c) == Some(ModuleType::Loader)).collect();
        let mut seen: HashSet<IVec2> = HashSet::new();
        let mut runs: Vec<Vec<IVec2>> = Vec::new();
        let mut sorted: Vec<IVec2> = loader_cells.iter().copied().collect();
        sorted.sort_by_key(|c| (c.x, c.y));
        for start in sorted {
            if seen.contains(&start) {
                continue;
            }
            let axis = at.get(&start).map(|(_, _, r)| r.facing_offset()).unwrap_or(IVec2::X).abs();
            let same_axis = |c: &IVec2| {
                loader_cells.contains(c) && at.get(c).map(|(_, _, r)| r.facing_offset().abs()) == Some(axis)
            };
            // Walk back to the start of the run, then forward along it.
            let mut head = start;
            while same_axis(&(head - axis)) && !seen.contains(&(head - axis)) {
                head -= axis;
            }
            let mut run = Vec::new();
            let mut c = head;
            while same_axis(&c) && !seen.contains(&c) {
                seen.insert(c);
                run.push(c);
                c += axis;
            }
            runs.push(run);
        }
        // Clip racks: each belongs to the loader it touches, or reaches
        // through other racks.
        let clip_cells: HashSet<IVec2> = mine.iter().copied().filter(|c| kind(c) == Some(ModuleType::ClipRack)).collect();
        let mut clip_owner: HashMap<IVec2, usize> = HashMap::new();
        let mut q: VecDeque<(IVec2, usize)> = VecDeque::new();
        for (i, run) in runs.iter().enumerate() {
            for c in run {
                q.push_back((*c, i));
            }
        }
        while let Some((c, i)) = q.pop_front() {
            for step in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let n = c + step;
                if clip_cells.contains(&n) && !clip_owner.contains_key(&n) {
                    clip_owner.insert(n, i);
                    q.push_back((n, i));
                }
            }
        }
        let loaders: Vec<Loader> = runs
            .iter()
            .enumerate()
            .map(|(i, run)| {
                let clips = clip_owner.values().filter(|o| **o == i).count() as f32;
                Loader { length_m: run.len() as u32, clips: clips / run.len().max(1) as f32 }
            })
            .collect();

        let parts = GunParts {
            gauge_increasers: gauge,
            gauge_reducers: reducers,
            loaders,
            barrel_m: barrel.len() as u32,
            coolers,
            absorbers,
            rail_magnets: magnets,
            muzzle_brake: brake,
            shell: shells.get(&fp).cloned().unwrap_or_default(),
        };
        let stats = gun_math::stats(&parts);

        let mut problems = Vec::new();
        if barrel.is_empty() {
            problems.push("No barrel. Lay Barrels in a straight line out of the firing piece's front.".to_string());
        } else if (barrel.len() as f32) < stats.burn_m {
            problems.push(format!(
                "The barrel is {:.1} m short of a full burn, so shells leave at {:.0}% speed.",
                stats.burn_m - barrel.len() as f32,
                100.0 * (barrel.len() as f32 / stats.burn_m).sqrt()
            ));
        }
        if parts.shell.casings == 0 && magnets == 0 {
            problems.push("The shell has no casings and nothing else drives it: it won't leave the barrel.".to_string());
        }
        if parts.loaders.is_empty() {
            problems.push("No loader. Put Loader blocks behind or beside the firing piece.".to_string());
        } else if stats.working_loaders == 0 {
            problems.push(format!(
                "Every loader is too short for a {:.2} m shell. Lengthen one or drop gauge.",
                stats.shell_m
            ));
        }
        if gauge > gun_math::MAX_GAUGE_INCREASERS {
            problems.push(format!("{} gauge increasers; only {} count.", gauge, gun_math::MAX_GAUGE_INCREASERS));
        }
        if stats.rpm > 0.0 && stats.rpm == stats.rpm_heat && stats.rpm_heat < stats.rpm_loaders {
            problems.push("Heat, not the loaders, sets the pace. Another Breech Cooler would let it fire faster.".to_string());
        }

        let muzzle_cell = barrel.last().copied().unwrap_or(cell + if mantlet { dir } else { IVec2::ZERO }) + dir;

        // Magazines: from each feeder, along belt conveyors, to any magazine.
        let mut reached: Vec<IVec2> = Vec::new();
        let mut seen_belt: HashSet<IVec2> = HashSet::new();
        let mut belt: VecDeque<IVec2> = mine.iter().copied().filter(|c| kind(c) == Some(ModuleType::ShellFeeder)).collect();
        while let Some(c) = belt.pop_front() {
            for step in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let n = c + step;
                match kind(&n) {
                    // Magazines side by side are one store.
                    Some(ModuleType::Magazine) => {
                        if !reached.contains(&n) {
                            reached.push(n);
                        }
                        if seen_belt.insert(n) {
                            belt.push_back(n);
                        }
                    }
                    t if is_feedway(t) && seen_belt.insert(n) => belt.push_back(n),
                    _ => {}
                }
            }
        }
        reached.sort_by_key(|c| (c.x, c.y));
        let magazines: Vec<Entity> = reached.iter().filter_map(|c| at.get(c).map(|(e, _, _)| *e)).collect();
        let feed_rpm = if magazines.is_empty() { 0.0 } else { FEEDER_RPM * feeders as f32 };
        out.push((fp, GunAssembly { parts, stats, members, problems, muzzle_cell, feeders, magazines, magazine_shells: 0, feed_rpm, ship: None, on_turret: false }));
    }
    out
}

/// Share each magazine between the guns that draw on it, in shells of each
/// gun's own size -- hull guns and turret guns alike.
fn share_magazines(out: &mut [(Entity, GunAssembly)]) {
    let mut users: HashMap<Entity, u32> = HashMap::new();
    for (_, g) in out.iter() {
        for m in &g.magazines {
            *users.entry(*m).or_default() += 1;
        }
    }
    for (_, g) in out.iter_mut() {
        let metres: f32 = g.magazines.iter().map(|m| MAGAZINE_SHELL_M / users[m] as f32).sum();
        g.magazine_shells = if g.stats.shell_m > 0.0 { (metres / g.stats.shell_m).floor() as u32 } else { 0 };
        if g.stats.working_loaders > 0 && g.on_turret {
            if g.feeders == 0 {
                g.problems.push(format!(
                    "No Ammo Hoist beside the ring: only the {} shells in the ready rack, then nothing until you rearm.",
                    g.stats.ready_rack
                ));
            } else if g.magazines.is_empty() {
                g.problems.push("The hoist reaches no Magazine. Put one beside it, or run a Belt Conveyor to one.".to_string());
            }
        } else if g.stats.working_loaders > 0 {
            if g.feeders == 0 {
                g.problems.push(format!(
                    "No Shell Feeder: only the {} shells in the ready rack, then nothing until you rearm.",
                    g.stats.ready_rack
                ));
            } else if g.magazines.is_empty() {
                g.problems.push("The feeders reach no Magazine. Put one beside a feeder, or run a Belt Conveyor to one.".to_string());
            }
        }
    }
}

/// Where each built gun's muzzle is this frame, and which way it points.
/// A gun's grid frame is its ship's (cell (x, y) at (66x, 66y - 33)) or its
/// turret's (no half-cell offset, (0, 0) at the ring's centre).
pub fn place_muzzles(
    mut commands: Commands,
    guns: Query<(Entity, &GunAssembly, &Module, &ChildOf)>,
    frames: Query<(&GlobalTransform, Has<super::turret_deck::TurretBody>)>,
) {
    for (fp, gun, module, parent) in guns.iter() {
        let Ok((frame, is_turret)) = frames.get(parent.parent()) else { continue };
        let local = |c: IVec2| {
            if is_turret {
                Vec2::new(c.x as f32 * 66.0, c.y as f32 * 66.0)
            } else {
                crate::building::grid_to_local(c)
            }
        };
        let world = frame.transform_point(local(gun.muzzle_cell).extend(0.0)).truncate();
        let breech = frame.transform_point(local(module.grid_position).extend(0.0)).truncate();
        let dir = (world - breech).normalize_or_zero();
        commands.entity(fp).try_insert(GunMuzzle { world, dir });
    }
}

/// Keep each built gun's ready rack: refill it from the magazines at the
/// feeders' pace, spend it as the gun fires, and hold the gun to the feed
/// rate once it's empty.
pub fn run_gun_racks(
    time: Res<Time>,
    mut commands: Commands,
    mut guns: Query<(Entity, &GunAssembly, &crate::components::Weapon, &mut BaseWeaponStats, Option<&mut GunRack>)>,
) {
    let dt = time.delta_secs();
    for (entity, gun, weapon, mut base, rack) in guns.iter_mut() {
        let rack_max = gun.stats.ready_rack as f32;
        let Some(mut rack) = rack else {
            commands.entity(entity).try_insert(GunRack { ready: rack_max.min(weapon.ammo as f32), last_ammo: weapon.ammo });
            continue;
        };
        // Shots fired since last frame come out of the rack first.
        if weapon.ammo < rack.last_ammo {
            rack.ready = (rack.ready - (rack.last_ammo - weapon.ammo) as f32).max(0.0);
        }
        rack.last_ammo = weapon.ammo;
        // The feeders top it up, from what's left in the magazines.
        let behind = weapon.ammo as f32 - rack.ready;
        if behind > 0.0 {
            rack.ready = (rack.ready + gun.feed_rpm / 60.0 * dt).min(rack_max).min(weapon.ammo as f32);
        }
        let rpm = if rack.ready >= 1.0 { gun.stats.rpm } else { gun.stats.rpm.min(gun.feed_rpm) };
        let rate = (rpm / 60.0).max(0.0);
        if (base.fire_rate - rate).abs() > 1e-4 {
            base.fire_rate = rate;
        }
    }
}

/// How fast a faction works its main guns, of what the parts allow. The Broken
/// Choir's 60 mm autocannons ran at 172 rpm a turret, and a ship of theirs landed
/// about 25 rounds a second: they'd take you apart before you'd turned round.
/// Point defence is left alone (20 mm, under the 60 mm this applies from): how
/// well they stop missiles was never the trouble.
pub fn faction_gun_pace(faction: crate::ai_ship::components::AiShipType) -> f32 {
    match faction {
        crate::ai_ship::components::AiShipType::BrokenChoir => 0.6,
        _ => 1.0,
    }
}

/// A firing piece that has never been assembled still carries the registry's
/// placeholder magazine; the first assembly fills it to the real size.
fn newly_spawned(weapon: &crate::components::Weapon) -> bool {
    weapon.max_ammo == FIRING_PIECE_PLACEHOLDER_AMMO
}
pub const FIRING_PIECE_PLACEHOLDER_AMMO: u32 = 40;

/// Re-assemble a ship's guns -- its own and its turrets' together, since they
/// share the magazines -- when its layout or a turret's changes, and hand
/// the numbers to each firing piece's weapon.
pub fn assemble_guns(
    mut commands: Commands,
    ships: Query<(Entity, &LayoutVersion, Option<&Children>), Without<super::turret_deck::TurretBody>>,
    turrets: Query<(&LayoutVersion, &super::turret_deck::TurretBody, Option<&Children>, Option<&super::gun_house::RoofFittings>)>,
    // A block on its way up to a turret (a design or save spawns it on the
    // ship for a frame first) isn't a hull part: assembled there, a turret
    // gun was sized as a feederless hull gun and kept that small magazine.
    modules: Query<&Module, (Without<DestroyedModule>, Without<super::turret_deck::PendingTurretMount>)>,
    mut firing: Query<(&mut BaseWeaponStats, &mut WeaponMount, &mut crate::components::Weapon)>,
    // A shell redesigned re-works its gun out, layout change or not.
    shells: Query<(Entity, &crate::combat::shell::GunShell)>,
    redesigned: Query<&ChildOf, Changed<crate::combat::shell::GunShell>>,
    // A roof fitting gained or lost re-works that turret's guns out too.
    refitted: Query<Entity, Changed<super::gun_house::RoofFittings>>,
    epoch: Res<LayoutEpoch>,
    mut seen_epoch: Local<Option<LayoutEpoch>>,
    mut built: Local<BuiltFrom>,
    factions: Query<&crate::ai_ship::components::AiShipType>,
) {
    // The ships (or turrets) whose shells changed since last frame.
    let redo: HashSet<Entity> = redesigned.iter().map(|p| p.parent()).chain(refitted.iter()).collect();
    if *seen_epoch == Some(*epoch) && redo.is_empty() {
        return;
    }
    *seen_epoch = Some(*epoch);
    let shell_designs: HashMap<Entity, crate::combat::shell::ShellDesign> =
        shells.iter().map(|(e, s)| (e, s.0.clone())).collect();
    let guns_among = |children: Option<&Children>| -> Vec<(Entity, Module)> {
        children
            .into_iter()
            .flatten()
            .filter_map(|c| modules.get(*c).ok().map(|m| (*c, m.clone())))
            .filter(|(_, m)| m.module_type.category() == crate::components::ModuleCategory::Guns)
            // The roof over a turret's guns is no part of them.
            .filter(|(_, m)| !super::gun_house::is_roof_block(m.module_type))
            .collect()
    };
    for (ship, version, children) in ships.iter() {
        let on_it: Vec<Entity> = children.into_iter().flatten().copied().filter(|c| turrets.contains(*c)).collect();
        let stale = built.is_stale(ship, version)
            || redo.contains(&ship)
            || on_it.iter().any(|t| redo.contains(t) || turrets.get(*t).is_ok_and(|(v, _, _, _)| built.is_stale(*t, v)));
        if !stale {
            continue;
        }
        built.mark(ship, version);
        let hull = guns_among(children);
        let mut guns = assemble_parts(&hull, &shell_designs);
        for t in &on_it {
            let Ok((v, body, t_children, fittings)) = turrets.get(*t) else { continue };
            built.mark(*t, v);
            let mut up_top = assemble_parts(&guns_among(t_children), &shell_designs);
            let feed = hoist_reach(&hull, &body.ring_cells());
            let guns_up = up_top.len().max(1) as f32;
            for (_, g) in up_top.iter_mut() {
                // The roof: vents cool every gun under it, rangefinders
                // tighten their spread (combat::gun_house).
                if let Some(f) = fittings {
                    if f.coolers() > 0 || f.rangefinders > 0 {
                        g.parts.coolers += f.coolers();
                        g.stats = super::gun_math::stats(&g.parts);
                        g.stats.spread_deg *= f.spread_mult();
                    }
                }
                let share = hoist_rpm(g.stats.shell_m) / guns_up;
                g.on_turret = true;
                g.feeders = u32::from(feed.hoist.is_some());
                g.magazines = feed.magazines.clone();
                g.feed_rpm = if g.magazines.is_empty() { 0.0 } else { share };
            }
            commands.entity(*t).try_insert(feed);
            guns.extend(up_top);
        }
        if guns.is_empty() {
            continue;
        }
        share_magazines(&mut guns);
        let pace = factions.get(ship).map_or(1.0, |f| faction_gun_pace(*f));
        for (fp, mut assembly) in guns {
            assembly.ship = Some(ship);
            if assembly.stats.gauge_mm >= super::gun_math::BASE_GAUGE_MM {
                assembly.stats.rpm *= pace;
            }
            if let Ok((mut base, mut mount, mut weapon)) = firing.get_mut(fp) {
                base.damage = assembly.stats.damage * built_damage_scale();
                base.range = range_for(&assembly.stats);
                base.fire_rate = (assembly.stats.rpm / 60.0).max(0.0);
                // A magazine shot off takes its shells with it.
                let was_full = weapon.ammo >= base.max_ammo;
                base.max_ammo = assembly.max_ammo();
                weapon.ammo = if was_full && newly_spawned(&weapon) { base.max_ammo } else { weapon.ammo.min(base.max_ammo) };
                mount.firing_arc = SPINAL_ARC_DEG;
            }
            if trace() {
                let s = &assembly.stats;
                info!(
                    "GUN assembled {:?}: {:.0} mm, shell {:.2} m, {} of {} loaders working, barrel {} m (burn {:.1}), {:.0} m/s, {:.0} dmg, {:.1} rpm (loaders {:.1}, heat {:.1}), spread {:.2} deg, {} parts, {} shells aboard, problems: {:?}",
                    fp, s.gauge_mm, s.shell_m, s.working_loaders, assembly.parts.loaders.len(), assembly.parts.barrel_m, s.burn_m,
                    s.velocity_mps, s.damage, s.rpm, s.rpm_loaders, s.rpm_heat, s.spread_deg, assembly.members.len(), assembly.max_ammo(), assembly.problems
                );
            }
            commands.entity(fp).try_insert(assembly);
        }
    }
    built.retain(|e| ships.contains(e) || turrets.contains(e));
}

/// A cook-off on a turret -- a clip rack or loader going up -- flashes down
/// its hoist into every magazine it reaches without passing an Anti-Flash
/// Door, and they go up too.
pub fn flash_down_hoists(
    gone: Query<(&DestroyedModule, &ChildOf), (Added<DestroyedModule>, With<super::turret_deck::OnTurret>)>,
    feeds: Query<(&TurretFeed, &ChildOf)>,
    mut modules: Query<&mut Module, Without<DestroyedModule>>,
    player: Query<(), With<crate::components::Ship>>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    for (part, turret) in gone.iter() {
        if trace() {
            info!("TURRET PART DESTROYED: {:?} on turret {:?}", part.original_type, turret.parent());
        }
        // What holds shells: the ready rack, and the round in a loader.
        if !matches!(part.original_type, ModuleType::ClipRack | ModuleType::Loader) {
            continue;
        }
        let Ok((feed, ship)) = feeds.get(turret.parent()) else { continue };
        let Some(hoist) = feed.hoist else { continue };
        if !modules.contains(hoist) {
            continue; // the hoist is gone: nothing to flash down
        }
        let mut lit = 0;
        for m in &feed.flash {
            if let Ok(mut mag) = modules.get_mut(*m) {
                mag.health = 0.0;
                lit += 1;
            }
        }
        if trace() {
            info!("TURRET FLASH down the hoist on {:?}: {} magazine(s) lit, {} behind doors", ship.parent(), lit, feed.magazines.len() - feed.flash.len());
        }
        if player.contains(ship.parent()) {
            let (message, kind) = if lit > 0 {
                (format!("FLASH! A turret cook-off went down the hoist: {lit} magazine(s) going up"), crate::events::NotificationType::Danger)
            } else if !feed.magazines.is_empty() {
                ("Turret cook-off: the anti-flash doors held".to_string(), crate::events::NotificationType::Warning)
            } else {
                continue;
            };
            notifications.write(crate::events::ShowNotification { message, notification_type: kind, duration: 4.0 });
        }
    }
}

#[cfg(test)]
mod gun_assembly_tests {
    use super::*;
    use crate::components::Rotation;

    fn m(t: ModuleType, x: i32, y: i32, r: Rotation) -> Module {
        Module {
            module_type: t,
            health: 100.0,
            max_health: 100.0,
            power_consumption: 0.0,
            power_generation: 0.0,
            is_active: true,
            grid_position: IVec2::new(x, y),
            size: IVec2::ONE,
            rotation: r,
        }
    }

    /// Build a world-free module list from (type, x, y, rotation).
    fn ship(parts: &[(ModuleType, i32, i32, Rotation)]) -> Vec<(Entity, Module)> {
        parts
            .iter()
            .enumerate()
            .map(|(i, (t, x, y, r))| (Entity::from_raw_u32(i as u32 + 1).unwrap(), m(*t, *x, *y, *r)))
            .collect()
    }

    use ModuleType::*;
    use Rotation::*;

    /// A 170 mm gun facing the bow: two increasers, a 1 m loader with one
    /// rack, a cooler, and four barrels.
    fn small_gun() -> Vec<(Entity, Module)> {
        ship(&[
            (FiringPiece, 0, 0, East),
            (GaugeIncreaser, 0, 1, East),
            (GaugeIncreaser, 0, -1, East),
            (Loader, -1, 0, East),
            (ClipRack, -2, 0, East),
            (BreechCooler, -1, 1, East),
            (GunBarrel, 1, 0, East),
            (GunBarrel, 2, 0, East),
            (GunBarrel, 3, 0, East),
            (GunBarrel, 4, 0, East),
            (ShellFeeder, -3, 0, East),
            (Magazine, -4, 0, East),
        ])
    }

    /// Only the Choir work their guns slower, and not to a crawl.
    #[test]
    fn the_broken_choir_fire_slower_than_their_guns_allow() {
        use crate::ai_ship::components::AiShipType::*;
        let pace = faction_gun_pace(BrokenChoir);
        assert!(pace < 1.0 && pace >= 0.5, "{pace}");
        for other in [CorpseStars, TerranHegemony, RecursiveKingdom, MerchantPrinces] {
            assert_eq!(faction_gun_pace(other), 1.0);
        }
    }

    #[test]
    fn parts_around_a_firing_piece_make_one_gun() {
        let guns = assemble(&small_gun());
        assert_eq!(guns.len(), 1);
        let g = &guns[0].1;
        assert_eq!(g.parts.gauge_increasers, 2);
        assert_eq!(g.parts.barrel_m, 4);
        assert_eq!(g.parts.coolers, 1);
        assert_eq!(g.parts.loaders, vec![gun_math::Loader { length_m: 1, clips: 1.0 }]);
        assert_eq!(g.stats.gauge_mm, 170.0);
        assert!(g.stats.rpm > 0.0);
        assert_eq!(g.muzzle_cell, IVec2::new(5, 0));
        assert_eq!(g.members.len(), 11, "the magazine isn't part of the gun");
        assert!(g.problems.is_empty(), "{:?}", g.problems);
    }

    #[test]
    fn barrels_only_count_in_a_straight_line_out_the_front() {
        let mut s = small_gun();
        s.push((Entity::from_raw_u32(99).unwrap(), m(GunBarrel, 5, 1, East))); // off the line
        let g = &assemble(&s)[0].1;
        assert_eq!(g.parts.barrel_m, 4);
    }

    #[test]
    fn a_straight_run_of_loader_blocks_is_one_long_loader() {
        let s = ship(&[
            (FiringPiece, 0, 0, East),
            (Loader, -1, 0, East),
            (Loader, -2, 0, East),
            (Loader, -3, 0, East),
            (GunBarrel, 1, 0, East),
        ]);
        let g = &assemble(&s)[0].1;
        assert_eq!(g.parts.loaders, vec![gun_math::Loader { length_m: 3, clips: 0.0 }]);
    }

    #[test]
    fn a_part_between_two_guns_goes_to_the_nearer() {
        let s = ship(&[
            (FiringPiece, 0, 0, East),
            (GaugeIncreaser, 0, 1, East),
            (GaugeIncreaser, 0, 2, East),
            (FiringPiece, 0, 4, East),
            (GaugeIncreaser, 0, 3, East),
        ]);
        let guns = assemble(&s);
        let gauges: Vec<u32> = guns.iter().map(|(_, g)| g.parts.gauge_increasers).collect();
        assert_eq!(gauges, vec![2, 1]);
    }

    #[test]
    fn rail_magnets_beside_the_barrel_add_speed() {
        let mut s = small_gun();
        s.push((Entity::from_raw_u32(98).unwrap(), m(RailMagnet, 2, 1, East)));
        s.push((Entity::from_raw_u32(97).unwrap(), m(RailMagnet, 3, -1, East)));
        let plain = &assemble(&small_gun())[0].1;
        let railed = &assemble(&s)[0].1;
        assert_eq!(railed.parts.rail_magnets, 2);
        assert!(railed.stats.velocity_mps > plain.stats.velocity_mps);
    }

    #[test]
    fn a_magazine_behind_the_feeder_adds_shells_by_size() {
        let g = &assemble(&small_gun())[0].1;
        assert_eq!(g.magazines.len(), 1);
        // 80 m of 0.51 m shells.
        assert_eq!(g.magazine_shells, 156);
        assert_eq!(g.feed_rpm, FEEDER_RPM);
        assert_eq!(g.max_ammo(), g.stats.ready_rack + 156);
    }

    #[test]
    fn magazines_side_by_side_are_one_store() {
        let mut s = small_gun();
        let next = |i: u32| Entity::from_raw_u32(300 + i).unwrap();
        let mag = s.iter().find(|(_, m)| m.module_type == Magazine).unwrap().1.grid_position;
        s.push((next(0), m(Magazine, mag.x - 1, mag.y, East)));
        s.push((next(1), m(Magazine, mag.x - 2, mag.y, East)));
        let g = &assemble(&s)[0].1;
        assert_eq!(g.magazines.len(), 3);
        assert_eq!(g.magazine_shells, (3.0 * MAGAZINE_SHELL_M / g.stats.shell_m) as u32);
    }

    #[test]
    fn belt_conveyors_carry_the_feed_to_a_distant_magazine() {
        let mut s = small_gun();
        s.retain(|(_, m)| m.module_type != Magazine);
        let next = |i: u32| Entity::from_raw_u32(200 + i).unwrap();
        s.push((next(0), m(BeltConveyor, -4, 0, East)));
        s.push((next(1), m(BeltConveyor, -5, 0, East)));
        s.push((next(2), m(Magazine, -6, 0, East)));
        let g = &assemble(&s)[0].1;
        assert_eq!(g.magazines, vec![next(2)]);
    }

    #[test]
    fn two_guns_on_one_magazine_split_it() {
        let s = ship(&[
            (FiringPiece, 0, 0, East), (Loader, -1, 0, East), (ShellFeeder, -2, 0, East),
            (Magazine, -3, 1, East),
            (FiringPiece, 0, 2, East), (Loader, -1, 2, East), (ShellFeeder, -2, 2, East),
            (BeltConveyor, -3, 0, East), (BeltConveyor, -3, 2, East),
        ]);
        let guns = assemble(&s);
        assert!(guns.iter().all(|(_, g)| g.magazines.len() == 1));
        // 40 m each of 0.18 m shells (60 mm gauge).
        assert!(guns.iter().all(|(_, g)| g.magazine_shells == 222), "{:?}", guns.iter().map(|g| g.1.magazine_shells).collect::<Vec<_>>());
    }

    /// A 3x3 ring over (0..3, 0..3); a hoist beside it at (3, 1); one
    /// magazine right next to the hoist, another down a belt behind a door.
    fn hoisted() -> (Vec<(Entity, Module)>, Vec<IVec2>) {
        let hull = ship(&[
            (AmmoHoist, 3, 1, North),
            (Magazine, 3, 2, North),
            (BeltConveyor, 4, 1, East),
            (AntiFlashDoor, 5, 1, East),
            (Magazine, 6, 1, North),
        ]);
        let ring: Vec<IVec2> = (0..3).flat_map(|x| (0..3).map(move |y| IVec2::new(x, y))).collect();
        (hull, ring)
    }

    #[test]
    fn a_hoist_draws_through_doors_but_a_flash_stops_at_them() {
        let (hull, ring) = hoisted();
        let feed = hoist_reach(&hull, &ring);
        assert_eq!(feed.hoist, Some(hull[0].0));
        assert_eq!(feed.magazines, vec![hull[1].0, hull[4].0], "both, through the door");
        assert_eq!(feed.flash, vec![hull[1].0], "only the one with no door in the way");
    }

    #[test]
    fn a_hoist_must_stand_beside_the_ring() {
        let (mut hull, ring) = hoisted();
        hull[0].1.grid_position = IVec2::new(8, 8);
        assert_eq!(hoist_reach(&hull, &ring), TurretFeed::default());
    }

    #[test]
    fn hull_and_turret_guns_split_a_shared_magazine() {
        let mut hull_gun = assemble_parts(&small_gun(), &HashMap::new());
        let up_top: Vec<(Entity, Module)> = small_gun()
            .into_iter()
            .filter(|(_, m)| !matches!(m.module_type, Magazine | ShellFeeder))
            .collect();
        let mut turret_gun = assemble_parts(&up_top, &HashMap::new());
        let magazine = hull_gun[0].1.magazines[0];
        turret_gun[0].1.on_turret = true;
        turret_gun[0].1.feeders = 1;
        turret_gun[0].1.magazines = vec![magazine];
        hull_gun.extend(turret_gun);
        share_magazines(&mut hull_gun);
        let alone = assemble(&small_gun())[0].1.magazine_shells;
        assert!(hull_gun.iter().all(|(_, g)| g.magazine_shells < alone), "each gets part of it");
        assert!(hull_gun.iter().all(|(_, g)| g.problems.is_empty()), "{:?}", hull_gun.iter().map(|g| &g.1.problems).collect::<Vec<_>>());
    }

    #[test]
    fn no_feeder_means_only_the_ready_rack() {
        let mut s = small_gun();
        s.retain(|(_, m)| m.module_type != ShellFeeder);
        let g = &assemble(&s)[0].1;
        assert_eq!(g.magazine_shells, 0);
        assert_eq!(g.max_ammo(), g.stats.ready_rack);
        assert!(g.problems.iter().any(|p| p.contains("No Shell Feeder")));
    }

    #[test]
    fn a_gun_with_nothing_to_load_it_says_so() {
        let s = ship(&[(FiringPiece, 0, 0, North), (GunBarrel, 0, 1, North)]);
        let g = &assemble(&s)[0].1;
        assert_eq!(g.stats.rpm, 0.0);
        assert!(g.problems.iter().any(|p| p.contains("No loader")));
    }
}

#[cfg(test)]
mod gun_rack_tests {
    use super::*;
    use crate::components::Weapon;

    /// A gun whose loaders could fire 60 a minute but whose one slow feeder
    /// brings up only 20: three shells ready, ten aboard.
    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, run_gun_racks);
        let mut gun = GunAssembly::default();
        gun.stats.rpm = 60.0;
        gun.stats.ready_rack = 3;
        gun.feed_rpm = 20.0;
        let weapon = Weapon { damage: 1.0, range: 1.0, fire_rate: 1.0, ammo: 10, max_ammo: 10 };
        let e = app.world_mut().spawn((gun, weapon, BaseWeaponStats { damage: 1.0, range: 1.0, fire_rate: 1.0, max_ammo: 10 })).id();
        app.update(); // fits the rack
        (app, e)
    }

    fn rate(app: &App, e: Entity) -> f32 {
        app.world().get::<BaseWeaponStats>(e).unwrap().fire_rate
    }

    fn advance(app: &mut App, secs: f32) {
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(secs));
        app.update();
    }

    #[test]
    fn the_rack_lets_the_gun_fire_at_its_loaders_pace_until_it_empties() {
        let (mut app, e) = app();
        advance(&mut app, 0.01);
        assert!((rate(&app, e) - 1.0).abs() < 1e-4, "60 rpm while the rack is full");
        app.world_mut().get_mut::<Weapon>(e).unwrap().ammo = 7; // three shots
        advance(&mut app, 0.01);
        assert!((rate(&app, e) - 20.0 / 60.0).abs() < 1e-3, "rack empty: held to the feed");
        advance(&mut app, 3.5); // 20 rpm refills one shell every 3 s
        assert!((rate(&app, e) - 1.0).abs() < 1e-4, "a shell back in the rack: full pace again");
    }
}

#[cfg(test)]
mod gun_muzzle_tests {
    use super::*;
    use crate::combat::turret_deck::TurretBody;
    use crate::components::Rotation;

    /// One firing piece on `frame`, its barrel ending three cells along +x.
    fn muzzle_on(frame: impl Bundle) -> GunMuzzle {
        let mut app = App::new();
        app.add_systems(Update, place_muzzles);
        let parent = app.world_mut().spawn(frame).id();
        let module = Module {
            module_type: ModuleType::FiringPiece, health: 1.0, max_health: 1.0, power_consumption: 0.0,
            power_generation: 0.0, is_active: true, grid_position: IVec2::ZERO, size: IVec2::ONE, rotation: Rotation::East,
        };
        let gun = GunAssembly { muzzle_cell: IVec2::new(3, 0), ..default() };
        let fp = app.world_mut().spawn((module, gun, ChildOf(parent))).id();
        app.update();
        *app.world().get::<GunMuzzle>(fp).unwrap()
    }

    #[test]
    fn a_hull_gun_fires_from_its_barrel_end_in_the_ship_frame() {
        let m = muzzle_on(GlobalTransform::from_xyz(1000.0, 0.0, 0.0));
        // Ship cell (x, y) sits at (66x, 66y - 33).
        assert!((m.world - Vec2::new(1000.0 + 198.0, -33.0)).length() < 1e-3, "{:?}", m.world);
        assert!((m.dir - Vec2::X).length() < 1e-5);
    }

    #[test]
    fn a_turret_gun_turns_with_its_turret() {
        let body = TurretBody { angle: std::f32::consts::FRAC_PI_2, ..TurretBody::new(Entity::PLACEHOLDER, IVec2::ZERO, 3) };
        let frame = GlobalTransform::from(
            Transform::from_xyz(100.0, 50.0, 0.0).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        );
        let m = muzzle_on((body, frame));
        // Turret cells have no half-cell offset; a quarter turn puts +x on +y.
        assert!((m.world - Vec2::new(100.0, 50.0 + 198.0)).length() < 1e-2, "{:?}", m.world);
        assert!((m.dir - Vec2::Y).length() < 1e-5, "{:?}", m.dir);
    }
}

#[cfg(test)]
mod flash_tests {
    use super::*;
    use crate::combat::turret_deck::{OnTurret, TurretBody};
    use crate::components::Rotation;

    fn module(t: ModuleType) -> Module {
        Module {
            module_type: t, health: 100.0, max_health: 100.0, power_consumption: 0.0, power_generation: 0.0,
            is_active: true, grid_position: IVec2::ZERO, size: IVec2::ONE, rotation: Rotation::North,
        }
    }

    /// A turret whose hoist reaches two magazines, one of them behind a door;
    /// then `part` on the turret is destroyed. Returns both magazines' health.
    fn cook_off(part: ModuleType, hoist_alive: bool) -> (f32, f32) {
        let mut app = App::new();
        app.add_message::<crate::events::ShowNotification>();
        app.add_systems(Update, flash_down_hoists);
        let ship = app.world_mut().spawn(crate::components::Ship).id();
        let hoist = app.world_mut().spawn((module(ModuleType::AmmoHoist), ChildOf(ship))).id();
        if !hoist_alive {
            app.world_mut().entity_mut(hoist).insert(DestroyedModule { original_type: ModuleType::AmmoHoist });
        }
        let open = app.world_mut().spawn((module(ModuleType::Magazine), ChildOf(ship))).id();
        let doored = app.world_mut().spawn((module(ModuleType::Magazine), ChildOf(ship))).id();
        let body = app.world_mut().spawn((
            TurretBody::new(Entity::PLACEHOLDER, IVec2::ZERO, 3),
            TurretFeed { hoist: Some(hoist), magazines: vec![open, doored], flash: vec![open] },
            ChildOf(ship),
        )).id();
        let rack = app.world_mut().spawn((module(part), OnTurret { ring_cell: IVec2::ZERO }, ChildOf(body))).id();
        app.update();
        app.world_mut().entity_mut(rack).insert(DestroyedModule { original_type: part });
        app.update();
        let hp = |e| app.world().get::<Module>(e).unwrap().health;
        (hp(open), hp(doored))
    }

    #[test]
    fn a_cook_off_flashes_down_the_hoist_to_the_magazine_with_no_door() {
        assert_eq!(cook_off(ModuleType::ClipRack, true), (0.0, 100.0));
    }

    #[test]
    fn a_part_with_no_shells_in_it_starts_no_flash() {
        assert_eq!(cook_off(ModuleType::GunBarrel, true), (100.0, 100.0));
    }

    #[test]
    fn with_the_hoist_gone_there_is_no_way_down() {
        assert_eq!(cook_off(ModuleType::ClipRack, false), (100.0, 100.0));
    }
}
