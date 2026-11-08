# Fleet concepts: how to draw a faction

Pictures only. Nothing here touches game code. Every NPC ship and station in Afterlight is
being redesigned as concept art, and the developer approves, reworks or rejects each one on a
review page. The Terran Hegemony set (`terran.py`) is approved. It is the reference for quality
and method.

## The developer's rules (they rejected work that broke these)

1. **Top-down only.** The camera looks straight down on the ship. No side-view features:
   - no single-sided tower or sail sticking up from one edge;
   - no tail fin;
   - no long cigar body with a conning tower.

   Round 1 got this wrong and read as "2D submarines".
2. **Interiors are drawn by hand, room by room.**
   - No random or greedy fill (`gen.py`'s `fill()` is banned here).
   - Interiors should read like the developer's own ship, the Pincer (`designs/starter.json`,
     26x19 cells, 210 hull, about 65 working modules): readable corridors, deliberate rooms,
     the bridge in the middle of the hull, never on an edge.
   - Structured, designed patterns are fine, e.g. a symmetric rule painting a ring of radar
     arrays. Noise is not.
3. **Armour is selective.**
   - Put angled plates (`/`) on bow faces, prong and claw tips, and front-facing corners only.
   - Never put a plate on every outside tile.
4. **Not too rectangular.**
   - The Terran Lancer, Warden, Bulwark and Hauler were approved with the note "keep it but it
     look too rectangular".
   - Break outlines up: taper flanks, step corners diagonally, use real silhouettes (claws,
     rings, crescents, lattices, ribs).
   - Long straight box edges should be rare.
5. **Each faction has one silhouette grammar that holds at every size**, readable without colour.
   - In the game NPC ships are not tinted, so the faction can only be told apart by shape and
     hull material.
6. **Experimental, lore-driven designs are welcome** (Cosmoteer-style). The Terran set includes:
   - Oracle: a ring of radar arrays, because the Hegemony "sees 30 seconds ahead";
   - Homeward: an evacuation ark with habitation ribs.

## Size ladder (hull cells; the Pincer is 210)

| Class | Hull cells |
|---|---|
| Scout | 80–150 |
| Corvette | 150–300 |
| Frigate | 350–500 |
| Destroyer | 500–700 |
| Cruiser / battleship | 750–1300 |
| Flagship / boss | 1500–2000 (use open frames and voids; never solid blobs) |
| Defense platform | 60–150 |
| Station | 700–2500 |

## How to draw (`draw.py`)

```python
from draw import Canvas
c = Canvas("slug_key", "Titanium")   # Steel | Titanium | Composite | AbyssalAlloy
c.rows(x0, y_top, """
####
E..#####/
...
""")                                  # top line at y_top, going down; x0 = x of column 0
c.mirror()                            # copy y>0 onto y<0 (port/starboard symmetry); draw y>=0 only
c.rot4()                              # copy the x>0,y>=0 quadrant four ways (stations)
c.band(k, s)                          # paint square ring k of quadrant 1 (see terran.py bastion)
c.R(x0,x1,y0,y1,ch) / c.K(...) / c.P(x,y,"str") / c.cells[(x,y)] = ch
return c.build()
```

- **Orientation:** +x is the bow (right), +y is up, and the stern is at min x. Engines (`E`, or `N`
  for the 2-tall large engine) go on the aft-most column; they vent aft automatically.
- **Legend:** see the docstring of `terran.py` and `LEGEND` in `draw.py`. Extra symbols:
  - `P` plasma, `Z` creature containment (2x1), `Y` specimen vault, `J` signal buoy,
    `I` thermal imager, `*` cluster rocket, `^` guided missile, `&` mining drill, `~` stealth coating,
    `?` passive radar, `!` research lab (2x1), `$` vault, `%` adv. repair bay (2x1),
    `@` AI combat core (2x1), `+` force field, `=` signal jammer.
- **Multi-cell modules:** draw a block of the letter the size of the footprint.
  - BridgeWing `B` and SurgicalBay `S` are 3x2, either way round.
  - `G`, `C` are 2x2. `F` fusion and `h` docking hub are 3x3.
  - `K`, `L`, `U`, `r` (railgun), `D`, `N`, `Z`, `!`, `%`, `@` are 2x1.
  - A bridge straddling the centreline is drawn `BB` on rows y=1 and y=0 and then mirrored, which
    makes 2 wide x 3 tall.
  - Never let a multi-cell block touch another block of the same letter, or the tiler can't split
    them.
- **Walkability (the checker enforces it):**
  - Only `.` deck, doors `d` and crew posts are walkable. A cell is "interior" only if all 4
    neighbours are hull; rim cells are walls.
  - Every crew post (guns, engines, reactors, bridge, helm, quarters, scrubbers, radar, medbay,
    repair…) must touch walkable interior that connects to the main deck.
  - So a gun on the rim needs a deck or post cell directly inboard of it.
  - Hull must be at least 3 cells thick wherever crew walk.
- **Plates:**
  - A `/` goes in an empty cell that sits in an L of hull: hull on its left and below (or the
    mirrored combinations).
  - On a bow staircase, the plate sits just past the end of the shorter, outer row, above the
    longer row's cell.
- **Memory cores `M`:** at least 3 per ship, each at least 3 cells from the others.
- **Missile launchers** (`x`, `^`, `*`) must sit on a rim with 4 empty cells outside them.

## Workflow

1. Write `<module>.py` with `FACTION = {slug, name, tagline, grammar[5 lines], accent "#hex"}` and
   `CONCEPTS = [(key, fn, display_name, class_label, one_or_two_sentence_note), ...]`.
   Copy the shape of `terran.py`.
2. Build: `python3 build.py <module>`. Every line must print with **no PROBLEMS**.
3. Inspect a design as a grid with `python3 dbg.py <module> <key> [x0 x1 y0 y1]`:
   - upper case means an interior module, lower case means a module on the rim;
   - `.` is hallway, `#` is rim, `/` is a plate.
4. Look at it rendered and actually check the picture. Lineup:
   `python3 -c "import lineup; lineup.lineup('<slug>', px=7)"` writes
   `concepts/<slug>/_lineup.png`. Close-up: `fleet.render(fleet.load(path), px=16)`.
   Fix anything ugly, cramped, boxy or unreadable.
5. **Only edit your own `<module>.py` files** and `concepts/<slug>/`. Never edit `draw.py`,
   `gen.py`, `fleet.py`, `build.py`, `page*`, `terran.py` or another faction's files: other agents
   are working in this folder at the same time. If you need a helper, write it inside your own
   module. Don't commit; don't run cargo.
