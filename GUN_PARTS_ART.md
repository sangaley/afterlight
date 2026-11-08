# Gun parts: Retro Diffusion prompts

The new blocks for Phase 2 (guns built from parts) and Phase 3 (turrets), as seen in the
[Gun Deck preview](https://claude.ai/artifact/29ZUwwSyaA7dxVWWc8qn22). Every placeholder on that
page has a prompt here. Read `ART_BRIEF.md` first; this sheet only adds what is specific to
gun parts.

## Settings for every part

| Setting | Value |
|---|---|
| Size | **128 × 128** per cell. The few 2-cell parts say so. |
| Background | Transparent (remove background), with about 4 px of empty margin. |
| View | Straight top-down. Flat, even top light, **no cast shadows**: the engine draws its own. |
| Facing | Anything with a direction points **UP** (north), like the existing weapon art. |
| Seed | Pick one seed on the style test below and reuse it for every part. |

**Paintable.** Ships will get a paint scheme, per-block paint and decals, so the art has to take
paint. Keep the **plating a neutral mid grey**: no blue tint, no baked colour scheme. Use colour
only where it means something, and keep those colours saturated so paint can leave them alone:

- **brass**: ammunition and anything that holds or moves it
- **icy blue**: coolant
- **copper**: rail magnets
- **yellow and black stripes**: hazard and safety parts
- **orange**: hot or dangerous

Keep the base mid-bright. The game darkens blocks as they take damage and turns wrecks dark grey,
so art that starts dark loses that feedback.

**Tiling parts** (loader, clip, barrel, conveyor, turret plating) sit end to end in a line, so their
top and bottom edges must meet seamlessly. Use the tileable option, or generate them and check two
stacked copies.

## Base template

Paste this, then add the part's own line from below:

```
top-down view of a single sci-fi warship gun component tile, square riveted steel base plate,
neutral mid-grey metal, flat even lighting, no shadows, industrial, grim, utilitarian,
clean pixel art, readable silhouette, transparent background
```

Negative prompt for all of them:

```
perspective, isometric, side view, cast shadow, drop shadow, text, logo, blue tint, glow,
lens flare, background, floor, multiple objects, cropped
```

## Style test first

As `ART_BRIEF.md` says, start with a small set before committing. Generate these three, check
them on the Gun Deck preview, then do the rest with the same seed:

1. Firing piece
2. Barrel segment
3. Autoloader segment

---

## The parts

Directional parts point up. "Tiles" means the part repeats along its length.

### Firing piece: `firing_piece.png`
The breech: the heart of every gun, one per gun.
```
heavy cylindrical gun breech block seen from above, thick round breech ring with a brass
locking collar, dark bore opening at the top edge, reinforced square housing, bolts around it
```

### Gauge increaser: `gauge_increaser.png`
A ring of extra bore steel around the breech. Up to 8 per gun.
```
square block with three concentric heavy steel rings and a vertical brass strap across the
centre, machined, dense, like a reinforcing collar for a large gun
```

### Autoloader segment: `autoloader.png` (tiles)
Rams shells into the breech. Built in lengths of 1 to 8.
```
long mechanical loading tray running bottom to top, a channel holding three brass artillery
shells lying lengthwise, rails on both sides, thin yellow and black hazard strip along one edge,
top and bottom edges seamless so segments join end to end
```

### Ammo clip segment: `ammo_clip.png` (tiles)
A ready rack beside a loader, three shells each.
```
ammunition rack seen from above, four brass shell bases in a two by two grid in round steel
cradles, retaining bars, top and bottom edges seamless so segments join end to end
```

### Ammo feeder: `ammo_feeder.png`
Pulls shells out of the ship's magazine.
```
small conveyor intake block, three chevron arrows pointing up in yellow, rollers, a hatch at
the bottom edge where shells come in
```

### Cooler: `cooler.png`
Carries heat away from the gun.
```
radiator block with four parallel vertical cooling fins, icy blue coolant lines running
between the fins, small pipe stubs on the edges
```

### Recoil absorber: `recoil_absorber.png`
Soaks up recoil. Points up toward the breech.
```
hydraulic recoil buffer seen from above, a heavy coil spring along the length, piston rod
pointing up, two cylinder clamps, mechanical
```

### Barrel segment: `barrel.png` (tiles)
One metre of gun tube. Barrels are long, so this repeats.
```
straight heavy gun barrel tube running bottom to top, narrower than the tile, dark metal with
a lighter highlight stripe along it, one reinforcing band near the top, top and bottom edges
seamless so segments join end to end, transparent on both sides of the tube
```

### Muzzle brake: `muzzle_brake.png`
The barrel's last segment, with side vents.
```
gun muzzle brake at the end of a barrel, pointing up, a slightly wider block with three pairs
of rectangular side vent slots, dark bore opening at the top edge, transparent around it
```

### Mantlet: `mantlet.png`
Thick armour in front of the breech, where turrets get hit.
```
thick sloped armour mantlet plate, heavy cast steel, a slot in the middle where the gun barrel
passes through toward the top, yellow and black hazard edge on the front face, very solid
```

### Rail magnet: `rail_magnet.png`
Sits beside the barrel and accelerates the shell electrically.
```
electromagnetic rail coil block, four thick copper coil windings across the tile, heavy
insulated mounts, power cable stubs, copper is the dominant colour
```

### Turret plating: `turret_plating.png` (tiles in every direction)
The turret's own floor and armour.
```
heavy armour plate floor tile for a gun turret, slightly lighter grey than hull plating,
thick riveted edges, subtle wear, all four edges seamless so tiles join in any direction
```

### Turret armour: `turret_armour.png` (tiles in every direction)
The turret's rim: the outer plates of its deck, heavier than the floor inside.
```
heavy armour plate tile for the outer rim of a gun turret, darker and thicker than the turret
floor, deep riveted border, subtle weld seams, all four edges seamless so tiles join in any direction
```

### Turret armour angle piece: `turret_armour_wedge.png`
The 45-degree pieces that chamfer the rim's corners. Same armour as above, cut on the diagonal:
the plate fills the **lower-left half** of the tile, the upper-right half is transparent.
```
heavy armour plate cut diagonally, the plate filling only the lower-left triangle of the tile,
a bevelled lit edge along the diagonal cut, upper-right triangle fully transparent
```

### Gun house: `gun_house.png` (stretched as a 9-slice)
The housing over a turret's breeches and loaders. It's stretched to fit each turret, so keep the
detail in the **outer 24 px** (the chamfered corners and edge) and the middle plain enough to stretch.
```
top-down armoured gun turret housing roof, chamfered corners, flat steel plate with a raised rim,
plain centre, the corners and edges carry all the detail, mid-grey, no hatch
```

### Gun house hatch: `gun_house_hatch.png`
Drawn on top of the gun house roof, toward the back.
```
round armoured commander's hatch seen from above, heavy hinge, brass locking ring, small
periscope, transparent around it
```

### Turret ring: `turret_ring.png`
The bearing under a turret, on the hull deck. Hidden while the turret stands on it.
```
large rotating bearing ring segment, gear teeth around a circular track, heavy brass bearing
race, bolted to a square deck plate
```

### Ammo hoist: `ammo_hoist.png`
The shaft shells ride up into the turret, at the ring's centre.
```
round vertical ammunition shaft opening seen from above, dark circular hole with a brass
elevator rail across it, heavy hatch frame, bolts
```

### Anti-flash door: `anti_flash_door.png`
Stops a turret cook-off reaching the magazine.
```
heavy armoured blast door hatch, diagonal yellow and black hazard stripes across the whole
face, thick frame, locking bolts at the sides
```

### Traverse motor: `traverse_motor.png`
Turns the turret. More motors turn it faster.
```
heavy electric motor with a large spur gear seen from above, gear teeth clearly visible,
motor housing and cooling ribs beside it
```

### Ammo store (magazine): `ammo_store.png`
The ship's magazine. The thing a flash reaches, so it should look dangerous.
```
armoured ammunition magazine crate, reinforced steel box with brass corner fittings and a
cross brace, warning markings, clearly "this explodes"
```

### Conveyor straight and corner: `conveyor.png` (tiles), `conveyor_corner.png`
Carries shells from the magazine to the feeders.
```
narrow ammunition conveyor belt running bottom to top with small yellow cross slats and
rollers, transparent either side, top and bottom edges seamless
```
For the corner, swap the middle line for: `ammunition conveyor belt turning ninety degrees
from the bottom edge to the right edge`.

---

## When they're done

Drop the PNGs into `afterlight/assets/sprites/gun_parts/` with the file names above. I'll
swap them into the Gun Deck preview so you can judge them on the ship before anything goes
into the game. Steam needs AI-generated art declared in its content survey, as it already does
for the module art.
