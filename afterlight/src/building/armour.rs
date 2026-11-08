//! Deriving angled plating from a hull silhouette.
//!
//! Shared by the AI faction layouts and the player's starter design — both
//! describe a hull as rows of `(y, x_min, x_max)`, so both get their armour
//! the same way. Change an outline and its plating follows, which keeps the
//! silhouette the single source of a ship's shape.

use bevy::prelude::*;

use crate::components::{ModuleType, Rotation};

/// One plate: where it goes, what it is, and which corner its face looks out
/// along. Rotation maps to that corner — North=NE, East=SE, South=SW, West=NW
/// (see `Block::for_module`). Ships point +X, so on a hull that reads as
/// North bow-top, East bow-bottom, West stern-top, South stern-bottom.
pub type Plate = (IVec2, ModuleType, Rotation);

/// Wrap a silhouette's stepped bow and stern — one plate per step of the
/// staircase, each turned to look out along the step it smooths.
///
/// Plates sit OUTBOARD of the hull, never on it: hull wins its own cell in
/// `ShipGrid`, so a plate sharing a hull cell would armour nothing.
///
/// A step is filled COMPLETELY, up to `MAX_STEP`. Leaving a bare cell in the
/// middle of what should be one unbroken diagonal is what made the hulls read
/// as spiky — the jaggedness came from plates sitting as isolated units with
/// hull showing between them, not from all of them being 45°. A continuous run
/// of the same triangle reads as a raked face; a broken one reads as spines.
///
/// Past MAX_STEP the row isn't a taper any more, it's a shoulder — a cruciform
/// sensor arm meeting a thin spine, say. Filling one of those packs the notch
/// solid and throws the silhouette away, so a shoulder gets its corner plated
/// and nothing else.
pub fn belt(rows: &[(i32, i32, i32)], plate: ModuleType) -> Vec<Plate> {
    /// Longest step still read as a taper rather than a shoulder.
    const MAX_STEP: i32 = 4;
    let mut out = Vec::new();
    // A row narrower than BOTH its neighbours is a step twice over, so the
    // same cell can be claimed from above and from below. First claim wins.
    let mut taken: std::collections::HashSet<IVec2> = std::collections::HashSet::new();
    for pair in rows.windows(2) {
        let (y_hi, a_hi, b_hi) = pair[0];
        let (y_lo, a_lo, b_lo) = pair[1];
        if y_hi - y_lo != 1 {
            continue;
        }
        // Upper half faces up, lower half faces down: the belt has to look
        // AWAY from the hull, and which way that is flips at the centreline.
        let (bow, stern) = if y_hi > 0 {
            (Rotation::North, Rotation::West)
        } else {
            (Rotation::East, Rotation::South)
        };
        let (narrow_y, wide_b, narrow_b) = if b_hi < b_lo { (y_hi, b_lo, b_hi) } else { (y_lo, b_hi, b_lo) };
        for x in (narrow_b + 1)..=(narrow_b + (wide_b - narrow_b).min(MAX_STEP)) {
            let cell = IVec2::new(x, narrow_y);
            if taken.insert(cell) {
                out.push((cell, plate, bow));
            }
        }
        let (narrow_y, wide_a, narrow_a) = if a_hi > a_lo { (y_hi, a_lo, a_hi) } else { (y_lo, a_hi, a_lo) };
        for x in (narrow_a - (narrow_a - wide_a).min(MAX_STEP))..narrow_a {
            let cell = IVec2::new(x, narrow_y);
            if taken.insert(cell) {
                out.push((cell, plate, stern));
            }
        }
    }
    out
}

/// Cap the dorsal and ventral edges.
///
/// `belt` works from the STEP between two rows, so the topmost and bottommost
/// rows — with no neighbour beyond them — come out bare. That leaves a ship
/// plated at bow and stern and naked along its back and belly, which is the
/// half of the outline you actually see side-on.
pub fn caps(rows: &[(i32, i32, i32)], plate: ModuleType, run: i32) -> Vec<Plate> {
    let mut out = Vec::new();
    let Some(top) = rows.iter().max_by_key(|r| r.0) else { return out };
    let Some(bottom) = rows.iter().min_by_key(|r| r.0) else { return out };
    for (&(y, a, b), outward, bow, stern) in [
        (top, 1, Rotation::North, Rotation::West),
        (bottom, -1, Rotation::East, Rotation::South),
    ] {
        let reach = run.min((b - a + 1) / 2);
        for i in 0..reach {
            out.push((IVec2::new(b - i, y + outward), plate, bow));
            out.push((IVec2::new(a + i, y + outward), plate, stern));
        }
    }
    out
}
