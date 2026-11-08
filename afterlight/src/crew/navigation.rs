//! Interior navigation: which ship-local cells crew can walk, and how to get
//! between two of them.
//!
//! Crew walk on hallways and nothing else. `HullLayer::Hallway` is the only
//! surface that carries them; the outer shell, the structural `Inner` hull and
//! the void between hulls are all things you route AROUND. That makes where
//! the hallways go a real design decision — a post with no hallway route to it
//! cannot be manned, however much floor happens to surround it.
//!
//! The exception is the posts themselves. Crew have to stand somewhere to do
//! their job, so a module they occupy — a crew station or a bunk — is walkable
//! even though the machinery next to it isn't. Crossing one is expensive:
//! squeezing through the gun deck to reach the reactor should lose to walking
//! the corridor. Every other module is solid.
//!
//! A destroyed block drops out of the map the moment it is marked, which is
//! what makes battle damage sever routes for free.

use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};
use std::collections::{BinaryHeap, VecDeque};

use crate::ai_ship::components::AiShip;
use crate::building::{footprints, rooms::transform_to_grid, ShipGrid};
use crate::components::*;

/// What a crew member finds when they step into a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavCell {
    /// Laid decking. The only surface crew cross freely.
    Hallway,
    /// A module crew occupy — a crew station or a bunk. Walkable because
    /// somebody has to stand there, but dear enough that routing THROUGH one
    /// loses to any reasonable hallway.
    Post,
    /// A bulkhead door. Passable only while unsealed.
    Door { sealed: bool },
}

impl NavCell {
    /// Traversal cost in tenths, so paths stay integer-comparable. `MIN_COST`
    /// below must track the cheapest value here or the A* heuristic stops
    /// being admissible.
    pub const fn cost(self) -> u32 {
        match self {
            NavCell::Hallway => 6,
            NavCell::Door { .. } => 6,
            NavCell::Post => 14,
        }
    }

    pub const fn passable(self) -> bool {
        !matches!(self, NavCell::Door { sealed: true })
    }
}

/// Cheapest possible step. The heuristic scales by this to stay admissible.
const MIN_COST: u32 = 6;

/// Per-ship map of walkable interior, in ship-LOCAL cells.
///
/// Deliberately not derived from `ShipGrid`: that map holds one entity per
/// cell and hull wins ties, so on the starter the 35 cells carrying both a
/// hull segment and a module report only the hull. Navigation needs to know
/// about both, so it indexes the block queries itself.
#[derive(Component, Default)]
pub struct NavGrid {
    pub cells: HashMap<IVec2, NavCell>,
    /// Bumped on every rebuild so in-flight paths can notice the ship changed
    /// shape under them and replan.
    pub version: u32,
}

impl NavGrid {
    pub fn get(&self, cell: IVec2) -> Option<NavCell> {
        self.cells.get(&cell).copied()
    }

    /// Can a crew member stand here right now?
    pub fn passable(&self, cell: IVec2) -> bool {
        self.get(cell).is_some_and(NavCell::passable)
    }

    fn step_cost(&self, cell: IVec2) -> Option<u32> {
        self.get(cell).filter(|c| c.passable()).map(NavCell::cost)
    }

    /// Nearest passable cell to `from`, searched outward in rings. Used to
    /// place crew at spawn and to recover anyone who ends up standing in a
    /// cell that just got shot away.
    pub fn nearest_passable(&self, from: IVec2) -> Option<IVec2> {
        if self.passable(from) {
            return Some(from);
        }
        // Generous for a capital: the biggest hulls run 60+ cells across
        // (eternal_hegemony/citadel), and ships are meant to grow past that.
        // A ring search only gets this far when someone is far off the deck.
        const MAX_RING: i32 = 96;
        for ring in 1..=MAX_RING {
            let mut best: Option<(IVec2, i32)> = None;
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    // Only the shell of the ring — the inside was searched already.
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    let cell = from + IVec2::new(dx, dy);
                    if !self.passable(cell) {
                        continue;
                    }
                    let d = dx * dx + dy * dy;
                    if best.is_none_or(|(_, bd)| d < bd) {
                        best = Some((cell, d));
                    }
                }
            }
            if let Some((cell, _)) = best {
                return Some(cell);
            }
        }
        None
    }
}

const NEIGHBOURS: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];

/// A* frontier entry. `Ord` is deliberately reversed on `f` so `BinaryHeap`
/// (a max-heap) pops the cheapest node.
#[derive(PartialEq, Eq)]
struct Frontier {
    f: u32,
    g: u32,
    cell: IVec2,
}

impl Ord for Frontier {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .f
            .cmp(&self.f)
            // Tie-break toward the node already closer to the goal, then on
            // coordinates so equal-cost paths are deterministic run to run.
            .then_with(|| other.g.cmp(&self.g))
            .then_with(|| (self.cell.x, self.cell.y).cmp(&(other.cell.x, other.cell.y)))
    }
}

impl PartialOrd for Frontier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn heuristic(a: IVec2, b: IVec2) -> u32 {
    ((a.x - b.x).unsigned_abs() + (a.y - b.y).unsigned_abs()) * MIN_COST
}

/// Cheapest walk from `from` to `to`, inclusive of both ends.
///
/// Four-neighbour on purpose: diagonals would let crew cut the corner between
/// two blocked cells, which reads as walking through a wall junction.
///
/// `None` means genuinely unreachable — a severed section, a sealed bulkhead
/// with no way around, or a destination that is not deck at all. Callers are
/// expected to treat that as information, not as an error.
pub fn find_path(nav: &NavGrid, from: IVec2, to: IVec2) -> Option<Vec<IVec2>> {
    if !nav.passable(from) || !nav.passable(to) {
        return None;
    }
    if from == to {
        return Some(vec![from]);
    }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<IVec2, IVec2> = HashMap::new();
    let mut best_g: HashMap<IVec2, u32> = HashMap::new();

    best_g.insert(from, 0);
    open.push(Frontier { f: heuristic(from, to), g: 0, cell: from });

    while let Some(Frontier { g, cell, .. }) = open.pop() {
        if cell == to {
            let mut path = vec![cell];
            let mut current = cell;
            while let Some(&prev) = came_from.get(&current) {
                path.push(prev);
                current = prev;
            }
            path.reverse();
            return Some(path);
        }
        // A cheaper route to this cell was queued after this entry was.
        if best_g.get(&cell).is_some_and(|&best| g > best) {
            continue;
        }
        for offset in NEIGHBOURS {
            let next = cell + offset;
            let Some(step) = nav.step_cost(next) else { continue };
            let next_g = g + step;
            if best_g.get(&next).is_some_and(|&best| next_g >= best) {
                continue;
            }
            best_g.insert(next, next_g);
            came_from.insert(next, cell);
            open.push(Frontier { f: next_g + heuristic(next, to), g: next_g, cell: next });
        }
    }
    None
}

/// Every cell reachable from `from`. Cheaper than pathing to each candidate
/// in turn when a caller needs to ask "which of these many stations can this
/// crew member actually get to".
///
/// Only the tests call this so far; it is the query auto-assignment needs
/// once an unreachable post is supposed to go dark rather than just be
/// unwalkable-to.
#[allow(dead_code)]
pub fn reachable_from(nav: &NavGrid, from: IVec2) -> HashSet<IVec2> {
    let mut seen = HashSet::new();
    if !nav.passable(from) {
        return seen;
    }
    let mut queue = VecDeque::from([from]);
    seen.insert(from);
    while let Some(cell) = queue.pop_front() {
        for offset in NEIGHBOURS {
            let next = cell + offset;
            if nav.passable(next) && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

/// The deck rule, in one place: which hull layers a person can stand on.
/// `None` is a wall.
pub fn hull_nav_cell(layer: HullLayer, sealed: bool) -> Option<NavCell> {
    match layer {
        HullLayer::Hallway => Some(NavCell::Hallway),
        HullLayer::BulkheadDoor => Some(NavCell::Door { sealed }),
        HullLayer::Inner | HullLayer::Outer | HullLayer::Void => None,
    }
}

/// Rebuilds a ship's `NavGrid` from its own live blocks, when that ship's
/// layout changed (`building::layout`: a block placed, destroyed or removed,
/// or a bulkhead sealed or opened -- sealing a door changes where crew can
/// walk without adding or removing a block).
///
/// It used to recount every block in the world each frame and rebuild every
/// ship's grid on any change. Each rebuild bumps `version`, which sends that
/// ship's crew back to plan their paths, so one plate lost on a far-off skiff
/// had everyone aboard every ship re-pathing.
pub fn rebuild_nav_grids(
    mut commands: Commands,
    mut ships: Query<
        (Entity, &crate::building::layout::LayoutVersion, Option<&Children>, Option<&mut NavGrid>),
        Or<(With<Ship>, With<AiShip>)>,
    >,
    modules: Query<(&Module, Has<CrewStation>, Has<Quarters>, Has<BulkheadSealed>), Without<DestroyedModule>>,
    hulls: Query<(&HullSegment, &Transform, Has<BulkheadSealed>), Without<HullDestroyed>>,
    epoch: Res<crate::building::layout::LayoutEpoch>,
    mut seen_epoch: Local<Option<crate::building::layout::LayoutEpoch>>,
    mut built: Local<crate::building::layout::BuiltFrom>,
) {
    if *seen_epoch == Some(*epoch) {
        return;
    }
    *seen_epoch = Some(*epoch);

    for (ship, version, children, grid) in ships.iter_mut() {
        if grid.is_some() && !built.is_stale(ship, version) {
            continue;
        }
        built.mark(ship, version);
        let children: Vec<Entity> = children.map(|c| c.iter().collect()).unwrap_or_default();

        // A ship that just lost its last block gets an empty grid written
        // rather than keeping a stale one.
        let mut cells: HashMap<IVec2, NavCell> = HashMap::new();

        // Hull first: it lays the deck. Anything not listed here is a wall,
        // and absence from the map is what makes it impassable.
        for &child in &children {
            let Ok((hull, transform, sealed)) = hulls.get(child) else { continue };
            let Some(cell) = hull_nav_cell(hull.hull_layer, sealed) else { continue };
            cells.insert(transform_to_grid(transform), cell);
        }

        // Modules then claim their cells. Machinery is solid and takes its
        // cells OFF the map even if a hallway was laid under it — you can't
        // walk through a reactor because someone decked the floor first.
        // Posts do the opposite: a crew station or a bunk is somewhere a
        // person stands, so it goes on the map whether or not it sits on
        // decking.
        //
        // Posts are applied in a second pass so a post always wins its cell,
        // regardless of the order modules happen to come in.
        let mut posts: Vec<IVec2> = Vec::new();
        for &child in &children {
            let Ok((module, is_station, is_quarters, sealed)) = modules.get(child) else { continue };
            let footprint = footprints::footprint_override(module.module_type);
            let occupied =
                ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint);
            if module.module_type.is_containment_door() {
                // A doorway, not machinery. Taking its cells off the map like
                // any other module would wall a corridor off permanently and
                // strand whoever was behind it -- open or shut, forever.
                for cell in occupied {
                    cells.insert(cell, NavCell::Door { sealed });
                }
            } else if is_station || is_quarters {
                posts.extend(occupied);
            } else {
                for cell in occupied {
                    cells.remove(&cell);
                }
            }
        }
        for cell in posts {
            cells.insert(cell, NavCell::Post);
        }

        match grid {
            Some(mut grid) => {
                grid.cells = cells;
                grid.version = grid.version.wrapping_add(1);
            }
            None => {
                // try_insert: a ship can be despawned the same frame it is
                // rebuilt (recursive destruction despawn).
                commands.entity(ship).try_insert(NavGrid { cells, version: 1 });
            }
        }
    }
    built.retain(|e| ships.contains(e));
}

#[cfg(test)]
mod nav_tests {
    use super::*;
    use crate::building::blueprint::Blueprint;

    /// Builds a grid from an ASCII sketch, so a test's intent is visible in
    /// its own source. Rows read top-down; `=` hallway, `P` crew post,
    /// `d` open door, `D` sealed door, anything else solid.
    /// Cell (0,0) is the BOTTOM-left, matching ship-local grid orientation.
    fn sketch(rows: &[&str]) -> NavGrid {
        let mut cells = HashMap::new();
        let height = rows.len() as i32;
        for (row_index, row) in rows.iter().enumerate() {
            let y = height - 1 - row_index as i32;
            for (x, ch) in row.chars().enumerate() {
                let cell = match ch {
                    '=' => NavCell::Hallway,
                    'P' => NavCell::Post,
                    'd' => NavCell::Door { sealed: false },
                    'D' => NavCell::Door { sealed: true },
                    _ => continue,
                };
                cells.insert(IVec2::new(x as i32, y), cell);
            }
        }
        NavGrid { cells, version: 1 }
    }

    fn cost_of(nav: &NavGrid, path: &[IVec2]) -> u32 {
        path.iter().skip(1).map(|c| nav.get(*c).unwrap().cost()).sum()
    }

    #[test]
    fn walks_a_straight_corridor() {
        let nav = sketch(&["====="]);
        let path = find_path(&nav, IVec2::new(0, 0), IVec2::new(4, 0)).unwrap();
        assert_eq!(path.len(), 5);
        assert_eq!(path[0], IVec2::new(0, 0));
        assert_eq!(path[4], IVec2::new(4, 0));
    }

    #[test]
    fn routes_around_a_wall() {
        // A bulkhead across the middle with a gap at the top.
        let nav = sketch(&[
            "=====",
            "==#==",
            "==#==",
        ]);
        let path = find_path(&nav, IVec2::new(0, 0), IVec2::new(4, 0)).unwrap();
        // It must go up and over, never through the blocked column.
        assert!(path.iter().any(|c| c.y == 2), "path never used the gap: {path:?}");
        assert!(!path.contains(&IVec2::new(2, 0)));
        assert!(!path.contains(&IVec2::new(2, 1)));
    }

    #[test]
    fn a_sealed_door_is_a_wall_and_an_open_one_is_not() {
        let open = sketch(&["==d=="]);
        assert!(find_path(&open, IVec2::new(0, 0), IVec2::new(4, 0)).is_some());

        let sealed = sketch(&["==D=="]);
        assert_eq!(find_path(&sealed, IVec2::new(0, 0), IVec2::new(4, 0)), None);
    }

    #[test]
    fn walks_the_long_way_round_rather_than_through_the_gun_deck() {
        // Straight across shoves through six manned posts: 14*6 = 84.
        // The way round is nine hallway steps plus the post itself: 6*9 + 14 = 68.
        // Fewer cells is not less effort, and pushing past working crew loses.
        let nav = sketch(&[
            "=======",
            "=     =",
            "PPPPPPP",
        ]);
        let path = find_path(&nav, IVec2::new(0, 0), IVec2::new(6, 0)).unwrap();
        assert!(
            !path.contains(&IVec2::new(3, 0)),
            "crew shoved through the gun deck instead of using the hallway: {path:?}"
        );
        assert_eq!(cost_of(&nav, &path), 6 * 9 + 14);
    }

    #[test]
    fn a_severed_section_is_unreachable() {
        let nav = sketch(&[
            "==#==",
            "==#==",
            "==#==",
        ]);
        assert_eq!(find_path(&nav, IVec2::new(0, 0), IVec2::new(4, 0)), None);
        assert_eq!(reachable_from(&nav, IVec2::new(0, 0)).len(), 6);
    }

    #[test]
    fn nearest_passable_finds_the_deck_from_inside_a_wall() {
        let nav = sketch(&[
            "#####",
            "##=##",
            "#####",
        ]);
        assert_eq!(nearest_of(&nav, IVec2::new(0, 0)), Some(IVec2::new(2, 1)));
        // Already standing somewhere valid: don't move them.
        assert_eq!(nearest_of(&nav, IVec2::new(2, 1)), Some(IVec2::new(2, 1)));
    }

    fn nearest_of(nav: &NavGrid, cell: IVec2) -> Option<IVec2> {
        nav.nearest_passable(cell)
    }

    /// Derives a nav grid from design data with the same rules
    /// `rebuild_nav_grids` uses, so the real ships can be checked without
    /// standing up an ECS world.
    fn nav_from_design(design: &Blueprint) -> NavGrid {
        let registry = crate::building::registry::build_registry();
        let mut cells = HashMap::new();
        for hull in &design.hull_cells {
            if let Some(cell) = hull_nav_cell(hull.layer, false) {
                cells.insert(hull.grid_pos, cell);
            }
        }
        let mut posts = Vec::new();
        // A turret's parts stand in its own grid, up on the turret, not on
        // the deck crew walk -- as `rebuild_nav_grids` sees the ship's own
        // children, never a turret's.
        for module in design.modules.iter().filter(|m| m.extras.as_ref().is_none_or(|e| e.turret.is_none())) {
            let def = registry.get(module.module_type);
            let footprint = footprints::footprint_override(module.module_type);
            let occupied =
                ShipGrid::cells_for(module.grid_pos, def.size, module.rotation, footprint);
            let is_post = def.crew_station
                || matches!(def.companion, crate::building::registry::CompanionData::Quarters { .. });
            if module.module_type.is_containment_door() {
                // A doorway, not machinery — the same branch `rebuild_nav_grids`
                // takes. This helper had drifted: it took a door's cells OFF the
                // map like any other module, so the first hull to put an
                // emergency bulkhead in a one-cell corridor — which is the only
                // sensible place for one — read as a ship cut into six pieces
                // with 34 crew posts stranded, none of which was true in play.
                for cell in occupied {
                    cells.insert(cell, NavCell::Door { sealed: false });
                }
            } else if is_post {
                posts.extend(occupied);
            } else {
                for cell in occupied {
                    cells.remove(&cell);
                }
            }
        }
        for cell in posts {
            cells.insert(cell, NavCell::Post);
        }
        NavGrid { cells, version: 1 }
    }

    /// The whole premise of deriving the deck from `HullLayer::Inner`: the
    /// ship the player actually starts with must be walkable end to end
    /// without anyone authoring a single corridor. If this fails, crew would
    /// spawn into a ship whose stations they cannot reach.
    #[test]
    fn the_starter_ship_interior_is_one_connected_space() {
        let design = crate::ship::builtin_starter_design();
        let nav = nav_from_design(&design);
        assert!(!nav.cells.is_empty(), "starter design produced no walkable cells");

        let start = *nav.cells.keys().next().unwrap();
        let reachable = reachable_from(&nav, start);
        let unreachable: Vec<_> = nav
            .cells
            .keys()
            .filter(|c| !reachable.contains(*c))
            .copied()
            .collect();
        assert!(
            unreachable.is_empty(),
            "{} of {} interior cells are cut off from the rest: {:?}",
            unreachable.len(),
            nav.cells.len(),
            unreachable
        );
    }

    /// The builtin is only a fallback — `designs/starter.json` is what the
    /// game actually spawns, and it wins whenever it exists. A walkable
    /// builtin next to a shipped design nobody can walk is exactly the drift
    /// that would put crew on a ship they can't cross, so check the file.
    #[test]
    fn the_shipped_starter_design_is_walkable() {
        let design = crate::building::blueprint::load_design_file("designs/starter.json")
            .expect("designs/starter.json missing or unparseable");
        let nav = nav_from_design(&design);

        let halls = nav.cells.values().filter(|c| **c == NavCell::Hallway).count();
        assert!(halls > 0, "the shipped starter has no hallways - crew cannot move at all");

        let start = *nav
            .cells
            .iter()
            .find(|(_, c)| **c == NavCell::Hallway)
            .expect("no hallway to start from")
            .0;
        let reachable = reachable_from(&nav, start);
        let stranded: Vec<_> = nav
            .cells
            .iter()
            .filter(|(cell, kind)| **kind == NavCell::Post && !reachable.contains(*cell))
            .map(|(cell, _)| *cell)
            .collect();
        assert!(
            stranded.is_empty(),
            "{} crew posts have no hallway route and would go unmanned: {:?}",
            stranded.len(),
            stranded
        );
    }

    #[test]
    #[ignore]
    fn dump_module_table() {
        let reg = crate::building::registry::build_registry();
        let mut rows: Vec<String> = reg
            .defs
            .iter()
            .map(|(mt, d)| {
                let post = d.crew_station
                    || matches!(d.companion, crate::building::registry::CompanionData::Quarters { .. });
                format!(
                    "MOD\t{mt:?}\t{}\t{}\t{}\t{:?}\t{}",
                    d.size.x, d.size.y, post, d.category, d.cost
                )
            })
            .collect();
        rows.sort();
        for r in rows {
            println!("{r}");
        }
    }

    /// Regenerates designs/starter.json from the builtin. Run deliberately:
    ///   cargo test export_starter -- --ignored
    #[test]
    #[ignore]
    fn export_starter() {
        let design = crate::ship::builtin_starter_design();
        crate::building::blueprint::write_design_file("designs/starter.json", &design)
            .expect("write failed");
        println!(
            "EXPORTED {} hull cells, {} modules",
            design.hull_cells.len(),
            design.modules.len()
        );
    }

    /// Prints the starter's deck as ASCII and names anything cut off from it.
    ///
    /// Layout work needs to see the deck, and the deck is derived (hallways
    /// are every enclosed cell no module stands on), so it cannot be read off
    /// the design file. Run deliberately:
    ///   cargo test dump_starter_deck -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_starter_deck() {
        let design = crate::ship::builtin_starter_design();
        let nav = nav_from_design(&design);
        let xs: Vec<i32> = nav.cells.keys().map(|c| c.x).collect();
        let ys: Vec<i32> = nav.cells.keys().map(|c| c.y).collect();
        let (x0, x1) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
        let (y0, y1) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        for y in (y0..=y1).rev() {
            let mut row = String::new();
            for x in x0..=x1 {
                row.push(match nav.cells.get(&IVec2::new(x, y)) {
                    Some(NavCell::Hallway) => '.',
                    Some(NavCell::Post) => 'P',
                    Some(NavCell::Door { .. }) => 'D',
                    None => '#',
                });
            }
            println!("{y:3} |{row}|");
        }
        let start = *nav
            .cells
            .iter()
            .find(|(_, c)| **c == NavCell::Hallway)
            .unwrap()
            .0;
        let r = reachable_from(&nav, start);
        let mut bad: Vec<IVec2> = nav.cells.keys().filter(|c| !r.contains(*c)).copied().collect();
        bad.sort_by_key(|c| (c.x, c.y));
        println!("cells={} reachable={} x0={} y0={}", nav.cells.len(), r.len(), x0, y0);
        for c in bad {
            println!("  STRANDED {:?} {:?}", c, nav.cells.get(&c).unwrap());
        }
    }

    #[test]
    #[ignore]
    fn dump_candidate_components() {
        for name in ["pincer", "crab", "trident"] {
            let design = crate::building::blueprint::load_design_file(&format!("designs/{name}.json")).unwrap();
            let nav = nav_from_design(&design);
            let mut unseen: HashSet<IVec2> = nav.cells.keys().copied().collect();
            let mut comps: Vec<(usize, usize, IVec2)> = Vec::new();
            while let Some(&seed) = unseen.iter().next() {
                let reach = reachable_from(&nav, seed);
                let here: Vec<IVec2> = reach.iter().copied().filter(|c| unseen.contains(c)).collect();
                let posts = here.iter().filter(|c| nav.cells.get(*c) == Some(&NavCell::Post)).count();
                comps.push((here.len(), posts, seed));
                for c in here { unseen.remove(&c); }
            }
            comps.sort_by(|a, b| b.0.cmp(&a.0));
            println!("COMP {name}: {} cells in {} regions", nav.cells.len(), comps.len());
            for (n, p, seed) in comps.iter().take(6) {
                println!("   region {n:4} cells, {p:3} posts, seeded {seed:?}");
            }
        }
    }

    /// Every candidate hull in designs/ has to be as walkable as the one we
    /// ship, or it is not a candidate.
    ///
    /// These are generated by tools/hulls, and a generator is exactly the kind
    /// of thing that produces a beautiful ship with a gun nobody can reach.
    /// Checked by the game's own rules rather than by the script that drew
    /// them, because a generator that grades its own homework is worth
    /// nothing.
    #[test]
    fn the_candidate_hulls_are_walkable() {
        for name in ["pincer", "crab", "trident"] {
            let path = format!("designs/{name}.json");
            let Some(design) = crate::building::blueprint::load_design_file(&path) else {
                panic!("{path} is missing or will not parse");
            };
            let nav = nav_from_design(&design);
            assert!(!nav.cells.is_empty(), "{name}: no walkable cells at all");

            let start = *nav
                .cells
                .iter()
                .find(|(_, c)| **c == NavCell::Hallway)
                .unwrap_or_else(|| panic!("{name}: no corridor anywhere"))
                .0;
            let reachable = reachable_from(&nav, start);
            let stranded: Vec<_> = nav
                .cells
                .iter()
                .filter(|(cell, kind)| **kind == NavCell::Post && !reachable.contains(*cell))
                .map(|(cell, _)| *cell)
                .collect();
            assert!(
                stranded.is_empty(),
                "{name}: {} crew posts have no route to them: {:?}",
                stranded.len(),
                stranded
            );

            let cores: Vec<IVec2> = design
                .modules
                .iter()
                .filter(|m| m.module_type == ModuleType::MemoryCore)
                .map(|m| m.grid_pos)
                .collect();
            assert!(cores.len() >= 3, "{name}: only {} memory cores", cores.len());
            let spread = cores
                .iter()
                .flat_map(|a| cores.iter().map(move |b| (*a - *b).abs().max_element()))
                .max()
                .unwrap_or(0);
            assert!(spread >= 3, "{name}: cores clustered, {spread} apart at most");

            // On the hull deck: a turret's parts are in its own grid.
            let mut seen = HashSet::new();
            for m in design.modules.iter().filter(|m| m.extras.as_ref().is_none_or(|e| e.turret.is_none())) {
                assert!(seen.insert(m.grid_pos), "{name}: two modules at {:?}", m.grid_pos);
            }
        }
    }

    /// Losing an arm must be a wound, not a death.
    ///
    /// This used to ban any Power, LifeSupport or Crew block from sitting past
    /// the split, which is not the same thing and is stricter than the point.
    /// A ship with six scrubbers can put two on the claws and still breathe
    /// with both claws gone; the pincer does exactly that (120 air against 90
    /// needed) and failed a rule whose own comment said it was there to stop
    /// deaths. Position is the designer's business. The consequence is not.
    ///
    /// So: cut both arms off and ask whether what is left still runs.
    #[test]
    fn candidate_hulls_survive_losing_their_arms() {
        let registry = crate::building::registry::build_registry();
        for name in ["pincer", "crab", "trident"] {
            let design = crate::building::blueprint::load_design_file(&format!("designs/{name}.json"))
                .unwrap();

            // Where the hull splits into arms: the first column, walking
            // forward, that holds more than one run of cells.
            let mut columns: std::collections::BTreeMap<i32, Vec<i32>> = Default::default();
            for c in &design.hull_cells {
                columns.entry(c.grid_pos.x).or_default().push(c.grid_pos.y);
            }
            let mut split = i32::MAX;
            for (x, ys) in columns.iter_mut() {
                ys.sort_unstable();
                let runs = 1 + ys.windows(2).filter(|w| w[1] - w[0] > 1).count();
                if runs > 1 {
                    split = *x;
                    break;
                }
            }

            // What the body alone still provides, and still has to power.
            let (mut air, mut berths, mut generation, mut draw) = (0.0f32, 0u32, 0.0f32, 0.0f32);
            for m in design.modules.iter().filter(|m| m.grid_pos.x < split) {
                let def = registry.get(m.module_type);
                generation += def.power_generation;
                if crate::ship::starts_active(m.module_type) {
                    draw += def.power_consumption;
                }
                match def.companion {
                    crate::building::registry::CompanionData::OxygenScrubber { output } => {
                        air += output
                    }
                    crate::building::registry::CompanionData::Quarters { berths: b } => berths += b,
                    _ => {}
                }
            }

            let needed = berths as f32 * crate::resources::OXYGEN_PER_CREW;
            assert!(
                air >= needed,
                "{name}: with both arms gone the body makes {air} oxygen for {berths} crew, \
                 who need {needed} — the survivors suffocate"
            );
            assert!(
                generation >= draw,
                "{name}: with both arms gone the body generates {generation} against a \
                 {draw} launch draw — the survivors sit in the dark"
            );
        }
    }

    /// The starter has to carry an airlock, and a pallbearer has to be able to
    /// stand next to it.
    ///
    /// Burial at space (crew::burial) is gated on the ship having somewhere to
    /// put a body, so a starter without one means no player ever sees the
    /// mechanic. The airlock is a solid module, so it takes its own cell OFF
    /// the nav map — which is why it sits on a dead-end stub at the bow. Drop
    /// it into the middle of a corridor instead and this test still passes
    /// while the ship is quietly cut in half, so the reachability check above
    /// is the other half of the guard.
    #[test]
    fn the_starter_has_an_airlock_a_pallbearer_can_reach() {
        let design = crate::building::blueprint::load_design_file("designs/starter.json")
            .expect("designs/starter.json missing or unparseable");

        let airlocks: Vec<IVec2> = design
            .modules
            .iter()
            .filter(|m| {
                matches!(
                    m.module_type,
                    ModuleType::AirlockChamber | ModuleType::DockingPort
                )
            })
            .map(|m| m.grid_pos)
            .collect();
        assert!(
            !airlocks.is_empty(),
            "the shipped starter has no airlock - the dead can never be put out"
        );

        let nav = nav_from_design(&design);
        for lock in airlocks {
            let stand = nav.nearest_passable(lock).unwrap_or_else(|| {
                panic!("nowhere to stand near the airlock at {lock:?}")
            });
            assert_ne!(
                stand, lock,
                "the airlock's own cell is walkable, which it should not be"
            );
            assert!(
                (stand - lock).abs().element_sum() <= 2,
                "the nearest footing to the airlock at {lock:?} is {stand:?}, too far to work it"
            );
        }
    }

    /// Armour plates sit outboard, on cells with no hull under them. If they
    /// ever entered the nav map, crew would stroll out onto the plating.
    #[test]
    fn outboard_armour_is_not_walkable() {
        let design = crate::ship::builtin_starter_design();
        let hull: HashSet<IVec2> = design.hull_cells.iter().map(|h| h.grid_pos).collect();
        let nav = nav_from_design(&design);

        let plates = design
            .modules
            .iter()
            .filter(|m| !hull.contains(&m.grid_pos))
            .count();
        assert!(plates > 0, "starter has no outboard modules - test proves nothing");

        for module in &design.modules {
            if !hull.contains(&module.grid_pos) {
                assert!(
                    !nav.passable(module.grid_pos),
                    "outboard module at {:?} became walkable deck",
                    module.grid_pos
                );
            }
        }
    }
}
