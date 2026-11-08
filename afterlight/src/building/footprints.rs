use bevy::prelude::IVec2;
use crate::components::ModuleType;

// ============================================================================
// FOOTPRINTS
//
// Every module occupies the plain WxH rectangle its `ModuleDef.size` names.
// What a block claims on the grid is exactly what its sprite covers, and the
// two cannot disagree.
//
// There WAS a table here of non-rectangular shapes -- an L-tromino corner
// plate, a T-tetromino sickbay and bridge wing, an S-tetromino staggered
// plate, plus-pentomino hubs -- each picked for what the module does. They
// read well on paper and badly on screen: `size` is the bounding box, so the
// sprite covered six cells while a T-shaped sickbay owned four, and the two
// corners it did not own were painted over the corridor beside it. Cutting
// the art to the shape fixed the overlap and left blocks whose silhouette did
// not match anything else on the hull. Squares won.
//
// The hook stays because it costs nothing and threading it back through
// ShipGrid, navigation and the spawner would be the expensive part. Add a
// shape here and `vfx::footprint_tiles` in the history (dd5674d) is the code
// that cuts its art to match -- a non-rectangular block needs both or it
// overlaps its neighbours again.
// ============================================================================

pub fn footprint_override(_module_type: ModuleType) -> Option<&'static [IVec2]> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::ShipGrid;
    use std::collections::HashMap;

    /// No two modules may claim the same cell, on any ship anyone flies.
    ///
    /// Worth a test because the footprint table moved three times in a day —
    /// L and T shapes in, then out again — and every change alters how many
    /// cells a block claims. Growing a Bridge Wing from four cells to six put
    /// it on top of a Memory Core on the crab and a Tractor Beam on the
    /// pincer, and nothing failed: `ShipGrid` just lets one win the cell, so
    /// the loser is a block you paid for that quietly does nothing.
    ///
    /// `starter_pre_*` are archived snapshots of older ships and are skipped
    /// on purpose — they are history, not something anyone flies.
    #[test]
    fn no_design_stacks_two_modules_in_one_cell() {
        let registry = crate::building::registry::build_registry();
        let mut checked = 0;

        let mut check = |label: &str, design: &crate::building::blueprint::Blueprint| {
            // Keyed by deck as well as cell: a block on a turret stands on
            // that turret's own grid (ModuleExtras.turret), where (0, 0) is
            // the ring's centre, not the hull's (0, 0).
            let mut owner: HashMap<(Option<bevy::prelude::IVec2>, bevy::prelude::IVec2), crate::components::ModuleType> =
                HashMap::new();
            for m in &design.modules {
                let def = registry.get(m.module_type);
                let deck = m.extras.as_ref().and_then(|e| e.turret);
                for cell in ShipGrid::cells_for(
                    m.grid_pos,
                    def.size,
                    m.rotation,
                    footprint_override(m.module_type),
                ) {
                    if let Some(other) = owner.insert((deck, cell), m.module_type) {
                        panic!(
                            "{label}: {:?} at {:?} claims {cell:?}, already taken by {other:?}",
                            m.module_type, m.grid_pos
                        );
                    }
                }
            }
            checked += 1;
        };

        check("builtin starter", &crate::ship::builtin_starter_design());
        // Faction class designs live one folder deeper, designs/factions/<slug>/.
        let class_dirs: Vec<_> = std::fs::read_dir("designs/factions")
            .expect("designs/factions/")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        let mut files: Vec<_> = std::fs::read_dir("designs")
            .expect("designs/")
            .chain(std::fs::read_dir("designs/factions").expect("designs/factions/"))
            .chain(class_dirs.iter().flat_map(|d| std::fs::read_dir(d).expect("class folder")))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter(|p| !p.to_string_lossy().contains("starter_pre_"))
            .collect();
        files.sort();
        for path in files {
            let name = path.to_string_lossy().to_string();
            let Some(design) = crate::building::blueprint::load_design_file(&name) else {
                panic!("{name} will not parse");
            };
            check(&name, &design);
        }
        assert!(checked > 10, "expected every shipped design, checked {checked}");
    }
}
