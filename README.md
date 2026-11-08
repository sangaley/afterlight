# Afterlight

A 2D space survival game built in Rust with Bevy. Build a ship at a station, staff it with crew, then push out into an increasingly hostile stretch of space to find out what's out there.

Space gets darker and stranger the farther you go. Scattered throughout are log entries from previous expeditions that piece together a cosmic horror story — something ancient is imprisoned out past the edge of charted space, and the creatures aren't wildlife, they're guards.

## The Loop

1. **Build** your ship at the station — place hull segments, engines, reactors, weapons, crew quarters, and life support modules on a grid
2. **Launch** — depart the station and begin exploring
3. **Survive** — manage power, oxygen, hull integrity, and crew while pushing farther out
4. **Salvage** — loot wrecks for resources and credits
5. **Dock** — find stations and outposts to repair, refuel, and resupply
6. **Discover** — find log entries that reveal the story and push you to go farther
7. **Return or die** — dock to rebuild, or lose everything

## Highlights

- **Freeform ship building** — over 140 buildable modules across hull, power, propulsion, weapons, crew, and utility categories. No preset layout; power routes through connected hull tiles, so placement matters.
- **Deep weapon customization** — cannons, railguns, reactors, and more can be assembled piece by piece (barrel, cooling, feed mechanism, etc.), with stats emerging from the actual build.
- **Cascading damage systems** — hull breaches vent air, fires spread and can chain into explosions, decompression drains oxygen and pulls loose items toward the breach.
- **Crew simulation** — 8 starting crew with oxygen needs, morale, and panic states. Staffed modules run better than empty ones.
- **Procedural space** — chunk-based generation across 5 distance zones and 8 biomes, with real gravity from stars, planets, and black holes.
- **Cosmic horror narrative** — 23+ log entries scattered across wrecks, caves, and ruins that escalate from mundane equipment failures to something much worse.

## Building & Running

Requires a recent [Rust toolchain](https://rustup.rs/). If you've never built a Rust
project before, follow [TUTORIAL.md](TUTORIAL.md) instead — it covers the whole setup
from a clean machine.

```bash
cd afterlight
cargo run                # Run the game (dev profile, dynamic linking)
cargo build --release    # Release build (LTO, single codegen unit)
cargo check              # Fast type-check without a full build
```

## Controls

| Key | Action |
|---|---|
| `WASD` | Ship movement |
| `Q` / `E` | Vertical thrusters |
| `Space` | Fire weapons |
| `Z` | Radar ping |
| `Tab` | Toggle radar display / cycle build categories |
| `B` | Toggle build mode |
| `[` / `]` | Cycle build items |
| `R` | Rotate module |
| `M` | Cycle hull material / map overlay |
| `X` | Delete mode |
| `C` | Crew menu |
| `F` | Dock / seal bulkhead (context-dependent) |
| `Esc` | Pause |
| `Enter` | Start / launch |

## Project Status

The core gameplay loop is fully functional: building, launching, surviving, fighting, salvaging, docking, and discovering logs all work. Missing pieces include progression depth (hull upgrade path, build costs), audio, and some gameplay polish (stealth mechanics, alert ramping, a tutorial).

## License

All rights reserved. See [LICENSE](LICENSE) — this repository is public for portfolio/demonstration purposes; it is not open source.
