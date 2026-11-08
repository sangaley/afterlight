# Procedural module art

The game's module sprites are generated procedurally (the commissioned artist
dropped out mid-project). Art direction: **dark, grimy, pixelated, block-filling
top-down machinery** in the spirit of Cosmoteer's part icons but pushed further
into the grim. Directional parts (engine nozzles, gun barrels) **protrude past
their block**. Colour language: **blue = power/energy, orange = thrust**.

## Regenerate everything

```bash
cd afterlight/tools/art
python3 darkset.py           # -> ./smooth_mach/*.png   (43 smooth sprites)
python3 pixelate_install.py  # pixelate + copy into ../../assets/sprites/modules/
```

(Requires Python 3 + Pillow.)

## Files

| File | Role |
|---|---|
| `rooms_lib.py`   | Base canvas + primitives. Footprint sizing = `60 + cells*66` (art), matching the engine. |
| `rooms_gen.py`   | `JOBS` table — every sprite and the footprints it needs — plus `fname()`. |
| `smoothmach.py`  | Smooth machinery primitives (plate / screen / glow / cylinder / etc.). |
| `cosmo.py`       | **Dark palette** + high-detail helpers (sphere-shade, grime, soot, core glow) + the reactor & thruster exemplars. |
| `darkset.py`     | **Main generator** — one builder per module, dark + block-filling, unique per block. |
| `pixelate_install.py` | Pixelate the smooth output and install into game assets. |
| `envfxgen.py`    | Effects + environment sprites (separate one-off generator). |

## How the pieces map to the game

- **Sprite → module** mapping lives in `src/sprite_map.rs::module_sprite_path`.
  Multi-cell modules point at their own footprint sprite (`railgun_2x1.png`,
  `large_reactor_3x3.png`, …) so nothing is ever stretched.
- **Directional overhang** is declared in `src/sprite_map.rs::sprite_overhang`
  (per sprite file: extension length + which end protrudes) and applied in
  `src/ship/spawner.rs` — the sprite is lengthened along its facing, offset so
  the housing stays centred on the cell, and raised in Z so the barrel/nozzle
  renders over the neighbour. Only 1×1 sprites currently overhang (their art
  aspect matches the cell exactly; multi-cell overhang needs the base-size fix).
- **Rotation**: `src/sprite_map.rs::sprite_base_rotation` aligns each sprite's
  native facing (engine art vents down, weapon art aims up) with the module's
  placement rotation.

## Notes / TODO

- Each *design* is unique, but sibling module types share a sprite file within a
  category (e.g. Cannon + Gatling both use `point_defense.png`). Split individual
  sprites out of `sprite_map.rs` if a specific module wants its own art.
- Multi-cell directional overhang (e.g. a 2×1 railgun barrel jutting out) is not
  done yet — needs the sprite base-size mismatch resolved first.
