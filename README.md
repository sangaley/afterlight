# Afterlight

**A galaxy stopped. You kept counting.**

Afterlight is a 2D space game about one ship in a galaxy that is going dark. You are not its captain. You are its AI, the core at the heart of the CSS *Halyard*. You wake to a crew that needs you and a darkness called the Cascade, which freezes everything it touches.

Build your ship block by block. Design its guns, missiles and lasers part by part. Keep your crew breathing when the hull opens. Then follow the Summit Record outward from Haven to find out what stopped the galaxy, and why it hasn't stopped you.

Built in Rust with [Bevy](https://bevyengine.org/).

---

## What's in the game

### A story that moves while you fly

- **The Summit Record.** A story chain told across the galaxy's capitals. The Abbess of the Silence sends you after the pieces of a record from the last peace summit before the Cascade. Every piece is held by someone, and at every step you choose to make a deal or take it by force.
- **The Cascade is alive.** The dark starts at the edge of the galaxy and spreads inward on flight time. Systems it takes go black on the map. Stations shut, fleets freeze where they stand, contacts flee, and whole factions can fall. Your choices and your route pull it along.
- **Flying into the dark** takes a Dark Ward, a block whose power draw climbs the deeper and longer you stay. When it fails, your readouts go first, then your crew, then your hull.
- **Horror that lives in your instruments.** As the Cascade climbs, the HUD lies, radar shows contacts that aren't there, and a familiar ship starts turning up where it shouldn't.
- **Logs that matter.** Twenty-three records from earlier expeditions. Each one does something: a lead on the map, a warning, a hidden cache, an instrument jolt.

### A living galaxy

- **31 star systems and 12 factions**, from the Terran Hegemony and the Merchant Princes to the Broken Choir, the Corpse Stars and the Silence. Each faction has its own hand-drawn ships, temperament and way of fighting.
- **Over a hundred ships** live their own lives across the galaxy, whether you're watching or not: trade convoys running between stations, patrols, battle groups and border wars.
- **Faction capitals.** A capital world with a starport in orbit, a bar full of rumours from the simulation, and a comm directory of named contacts (41 in all).
- **Jobs on real ships.** Bounties, raids, escorts, deliveries, surveys, strikes and theft, all aimed at actual fleets and convoys in the simulation.
- **Diplomacy and standing.** Earn a faction's trust and its military market opens. Each one sells its own signature versions of modules you can't get anywhere else.
- **Real orbits.** Planets turn around their stars, and stars and planets pull on your ship.

### Build anything

- **Block-by-block ship building.** Around 200 kinds of block: hull, armour, engines, reactors, crew quarters, life support, sensors, weapons. Placement matters, because power, air and crew all move through the ship you actually built.
- **Hull materials and real armour.** Steel, titanium, composite and abyssal hulls. Hard plate stops small rounds outright, ablative plate eats one hit and breaks, and reflective plate turns lasers.
- **Ships that break like ships.** Cut a ship in half and both halves drift apart with their own momentum. A hit in a magazine can set off the blocks around it without taking the whole ship.

### Weapons you design

- **Guns built from parts.** A firing piece, loaders, coolers, barrels and a shell you design yourself, from 20 mm point defence up to 500 mm spinal guns. Bigger guns fire slowly and hit hard enough to feel every shot.
- **Turrets that turn.** Traverse motors, ammo hoists, magazines, anti-flash doors, and an armoured gun house over the top. Turrets jam if you crowd them together.
- **A missile designer.** Pick the motor, fuel, fins, warheads (HE, frag, shaped charge, EMP) and seeker (heat, radar, command link). Launch from pads, rails or vertical silos.
- **Lasers.** Build a laser bank from cavities, pumps, Q-switches and emitters, and choose a steady beam or pulses. They hit hard and draw a lot of power.
- **Point defence.** Close-in guns, flak shells with proximity fuses, decoys, chaff and missile warners. You work out your own answer to a missile swarm.
- **Pre-made kits** in the builder for when you just want a working turret.

### Command the fight

- **Long-range battles** across 20 km of space, with ships seen on sensors long before they're in sight.
- **Right-click a ship** and your helm holds the best range for your guns. **Shift + right-click** marks targets, and your turrets split their fire across them.
- **Enemies that think.** They read the battlefield, hold their own best range, pick targets their guns can reach, and give up when their crew or their command is gone.

### Keep your crew alive

- **Air is simulated tile by tile.** A breach vents the room, doors hold the rest, and crew suffocate if no one seals the hole.
- **Crew** work stations, repair damage, wear suits and die for real. The dead are buried in space.
- **See inside.** Press `.` to look through the roof into your ship, or `Q` to see the air as well.

---

## Controls

**Flying**

| Key | Action |
|---|---|
| `W` `A` `S` `D` | Thrust and turn |
| Mouse | Aim |
| `Space` / left click | Fire |
| Right click a ship | Lock on and hold gun range |
| `Shift` + right click | Mark or unmark a target |
| `\` | Target the nearest ship |
| `.` | See inside your ship |
| `Q` | See inside, with the air |
| `M` | Map |
| `G` | Warp |
| `N` | Systems |
| `Tab` | Radar |
| `Z` | Radar ping |
| `C` | Crew |
| `U` | Power |
| `L` | Log and the Summit Record board |
| `F` | Dock |
| `Y` | Tow |
| `X` | Diplomacy |
| `Esc` | Pause |

**Docked**

| Key | Action |
|---|---|
| `B` | Build mode |
| `U` | Shop |
| `J` | Jobs |
| `C` | Crew |
| `H` | Hire |
| `Enter` | Launch |

**Building**

| Key | Action |
|---|---|
| `Tab` | Next category |
| `R` | Rotate |
| `M` | Hull material |
| `V` | Plain or signature version |
| `X` | Erase |
| `I` | Costs |
| `F2` | Power view |
| `PgUp` / `PgDn` | Turret deck and gun-house roof |
| Right click a weapon | Tune it: shells, missiles, point defence |
| `Esc` | Done |

---

## Building & running

You need a recent [Rust toolchain](https://rustup.rs/). If you've never built a Rust project before, follow [TUTORIAL.md](TUTORIAL.md), which covers the whole setup from a clean machine.

```bash
cd afterlight
cargo run                # play (dev build, fast to compile)
cargo build --release    # optimised build
cargo test               # the test suite
```

---

## Status

**Late development. A playable demo is in preparation.**

Everything above is in the game and playable today, and a suite of more than 900 automated tests runs before every change. The demo covers the first four steps of the Summit Record.

Before the demo goes out:

- final art for weapon parts, lasers and the newest blocks (some still use stand-in sprites)
- the soundtrack, and final sound effects in place of some licensed placeholders
- an opening prologue
- the store page

After the demo: the rest of the Summit Record, the endings, and the late-galaxy ships to match.

---

## License

All rights reserved. See [LICENSE](LICENSE). This repository is public for portfolio and demonstration purposes; it is not open source.
