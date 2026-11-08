# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

The crate is NESTED: `Cargo.toml` sits beside this file in `afterlight/`,
one level below the git repository root. Every command here runs from this
directory. Run them from the repo root and cargo reports "could not find
`Cargo.toml`", which reads like a broken checkout rather than a wrong `cd`.

```bash
cargo build              # Build (dev profile, dynamic linking enabled)
cargo run                # Run the game
cargo build --release    # Release build (LTO enabled, single codegen unit)
cargo check              # Fast type-check without full build
cargo clippy             # Lint (if clippy installed)
cargo test               # Unit tests (in-file `#[cfg(test)]` modules)

# Write designs/factions/<slug>.json for any faction that has none. Normally
# a layout self-exports on its first spawn, so a faction nobody has met yet
# has no file -- which is how Void Titan stayed unexported from July 2026,
# when designs started winning over layouts, until October.
cargo test export_missing_faction_designs -- --ignored
```

No CI/CD pipeline, and playtesting is still how most behaviour gets judged.
The test suite is no longer thin, though: ~166 in-file tests, and they are not
all pure logic. Several modules stand up a headless `App` and run real systems
over it — air pressure and venting, crew suction, interior walking, engine-room
staffing, deaths and burial, faction layout invariants.

Two things to know before writing one:

- Test modules are named for what they cover (`air_tests`, `suit_tests`,
  `walking_tests`), NOT `mod tests`. Grepping for `mod tests` finds nothing
  and makes a well-covered file look untested.
- Do NOT use `MinimalPlugins` for anything rate-based. Its `TimePlugin`
  rewrites `Time` from the wall clock every update, so a hand-advanced delta
  is discarded and per-second rates measure as frozen — which looks like a
  physics bug, not a harness bug. Use `App::new()` plus
  `init_resource::<Time>()` and drive the clock yourself; see
  `air_tests::sim_app`.

## Parallel sessions: one worktree each

Several Claude sessions run against this repo at once. They must NOT share a
working tree. Two of them in `/Users/shhh/afterlight` at the same time cost a
full afternoon: one session's half-finished `DecorationType` refactor broke the
other's build, a `cargo build` raced a concurrent one and reported errors that
did not exist, and 177 modified files from two unrelated workstreams ended up
tangled in the same diff - `src/ui/mod.rs` had to be committed hunk by hunk to
separate them.

Give each session its own worktree, as a sibling directory:

```bash
git worktree add ../afterlight-<topic> -b <topic>   # new lane
git worktree list                                     # who is where
git worktree remove ../afterlight-<topic>           # when the branch lands
```

`/Users/shhh/afterlight` itself is a worktree like any other - don't treat it
as the place work happens by default. Check `git worktree list` and `git branch
--show-current` before your first edit, and before every commit.

### Disk

Each worktree builds into its own `target/`, and a Bevy debug build is 6GB
fresh and grows past 25GB with incremental artifacts. This volume filled to
zero bytes three times in one session; a full disk shows up as
`rustc interrupted by SIGSEGV`, not as a disk error, and it takes the running
game down with it. Two useful habits:

```bash
df -h /                                    # before starting a long build
rm -rf target/debug/incremental            # ~2GB, pure cache, always safe
```

`cargo clean` reclaims everything at the cost of one full rebuild - fine on a
worktree you are done with, disruptive on one another session is building in.

## Architecture

Bevy 0.19 ECS **space survival game**. 2D, sprite-based, grid-based building system (66.0 unit cells). Originally a ship game, fully converted to space theme.

### Plugin Structure (registered in `main.rs`)

| Plugin | Location | Responsibility |
|---|---|---|
| **EventsPlugin** | `events.rs` | Registers all game events |
| **ShipPlugin** | `ship/` | Ship movement, physics, power, oxygen, radiation, hull, combat, decompression, radar |
| **WorldPlugin** | `world/` | Chunk management, biomes, POI discovery, zone transitions, procedural generation |
| **CreaturePlugin** | `creatures/` | Hostile creature AI/spawning, ambient life (space motes, pulsing spores, cosmic whales) |
| **CrewPlugin** | `crew/` | Crew spawning, needs (O2/morale), AI, suffocation, death |
| **BuildingPlugin** | `building/` | Grid placement/removal, occupancy, room detection, module registry |
| **UiPlugin** | `ui/` | HUD, menus (main/pause/game-over), build ghost, notifications, overlays |
| **MetaPlugin** | `meta.rs` | Persistence (unlocks JSON), inventory, currency, statistics |

### Core Data Flow

1. **Components** (`components.rs`): All ECS components live here. Central types: `Module`, `ModuleType`, `ModuleCategory`, `Rotation`, `HullSegment`, `HullMaterial`, `Creature`, `CrewMember`.

2. **Resources** (`resources.rs`): Global state. Key resources: `ShipState`, `BuildingState`, `GameConfig`, `WorldState`, `ChunkManager`, `Inventory`, `Unlocks`, `Statistics`.

3. **Events** (`events.rs`): All events registered in `EventsPlugin`. Grouped by domain: ship damage/breach, building place/remove, crew damage/death, creature spotted/attack, world/UI/save-load.

4. **States** (`states.rs`): `GameState` (MainMenu, Loading, StationDocked, Exploring, Docked, Paused, GameOver) and `BuildState` (Inactive, Placing, Moving, Connecting, Deleting).

### Space Theme Key Systems

- **Radiation damage** (`ship/radiation.rs`): Replaces old pressure system. Radiation intensity scales with distance from safe zones. Hull segments have `radiation_shielding` ratings per material tier.
- **Decompression** (`ship/decompression.rs`): Hull breaches cause air to escape (rooms have `air_level` 1.0→0.0). Drains oxygen. Crew seal breaches to restore air. Fire is extinguished by vacuum (low air).
- **Thrusters** (`ship/movement.rs`): Space physics with minimal drag — momentum is forever, you must thrust to stop. See Game Controls for the bindings.
- **Zones**: NearOrbit → AsteroidBelt → DeepSpace → Nebula → BlackHole
- **Biomes**: OpenVoid, AsteroidField, CrystalFormation, VoidRift, ThermalVents, IceShells, DeadZone, AncientRuins

### Module Registry System

`building/registry.rs` defines `ModuleRegistry` — a data-driven HashMap<ModuleType, ModuleDef> with stats, size, color, and `CompanionData` for every module type. `ship/spawner.rs::spawn_module()` reads the registry to spawn entities with the correct `Module` component plus companion components (Reactor, Engine, Weapon+WeaponCooldown+WeaponMount+TargetingSystem+AmmoStorage, Radar, etc.).

**To add a new module**: Add variant to `ModuleType` enum, add it to the relevant `ModuleCategory::module_types()` list, add `ModuleDef` entry in `build_registry()`, and if needed add a new `CompanionData` variant + handling in `spawn_module()`.

### Grid & Building System

- Grid cell size: 66.0 world units
- Two grid indexes, both `HashMap<IVec2, Entity>` keyed by ship-LOCAL cell:
  - `GridOccupancy` (resource) — the **player's** blocks, rebuilt only while
    `StationDocked`. Drives build-mode placement, ghosts, clipboard, inspection.
    Goes stale on launch, which is fine for what reads it.
  - `ShipGrid` (component, one per ship) — **live** blocks only (destroyed ones
    drop out), maintained in flight as well as at dock. Grid coordinates are
    ship-local, so one global map can only ever describe one ship; this is that
    index without the restriction. Combat/hit resolution reads this one.
  - Migration in progress: `GridOccupancy` and its ~30 call sites are untouched
    so far. `cells_for` lives on `ShipGrid`, with `GridOccupancy` delegating.
- Multi-cell modules supported via `ModuleDef.size` (e.g., LargeReactor is 2x1)
- Placement validation: no overlap + adjacency required + positional rules (propulsion at rear, crew not near power)
- Build mode input/UI is `StationDocked` only, but the placement and removal
  event processors also run while `Exploring` — `ship::rebuild` respawns shot-off
  blocks in flight through the same `PlaceHullRequest`/`PlaceModuleRequest` events
- Build flow: input -> `PlaceModuleRequest`/`PlaceHullRequest` event -> process system -> `ModulePlaced` event

### Key Conventions

- Events use typed enums (`ModuleType`, `CreatureType`) not strings
- Existing systems query companion components (`Weapon`, `Radar`, `Engine`, etc.) not `ModuleType` — this keeps them backward-compatible when new module types are added
- Systems are `.chain()`-ed within plugins and gated by `.run_if(in_state(...))` on `GameState`/`BuildState`
- Hull segments and modules are children of the ship entity
- Notifications use `ShowNotification` events with `NotificationType` (Info/Warning/Danger/Success)

### Game Controls

- WASD: ship movement (W/S throttle, A/D strafe — the top-bar THRS meter reads the W/S throttle), Space: fire weapons, Z: radar ping. Q/E are unbound: the old vertical thrusters were a submarine holdover that pushed along world Y regardless of facing.
- J: mission board (docked, or flying near any station), Shift+J: show/hide the top-right contract tracker (`contracts::ui::ContractHudVisible`)
- The HUD "HAVEN" readout is radial distance from the origin in km — `ui::format_range_km` formats every range readout; internals still call it `depth` (`DepthState::current_depth`)
- F: dock at any station (see Stations below), U: shop, H: hiring, G: hold to warp-dash

### Combat model

Fights are decided by **subsystems**, not by grinding hull pools (a Terran Hegemony battleship is 160 tiles x 500 HP — minutes of held fire). Four rules carry it:

- **Defeat is a systems condition.** `combat::check_ai_cripple` — once a ship's guns AND engines are both under 25%, the crew strikes colors and it becomes an intact, salvage-rich derelict. `AiShipDestroyed::cause` (`ShipDeathCause::Struck` / `Meltdown` / `Gutted`) shapes the wreck and its loot.
- **Reactor kills are a phase, not a frame.** Breaching the last live reactor starts `ReactorMeltdown` (8s): shield down for good, ship goes berserk on whoever cracked it, then detonates and guts half the remaining blocks. A spare reactor absorbs the breach — that's why bosses last longer.
- **Armour covers what's behind it.** In `new_projectiles.rs`, a module under live plating only takes `ammo_types::armor_pass_through` of the round (APFSDS 0.9 … flak 0, unspecialised/beams 0.15); the rest hits the plate. Once the plate is gone the module is exposed and takes everything.
- **Anti-drag valves.** Enemy magazines/fuel cook off (`combat::ai_chain_reactions`), and fighters withdraw at ≤35% guns rather than waiting to be finished.

Player-side aiming: right-click any enemy block to lock the battery onto it (`combat::targeting::aim_lock`) — kinetics, beams and turret barrels all converge there, it auto-fires inside max weapon range, walks to the neighbouring block when the locked one dies, and right-clicking empty space releases everything back to manual fire.

### Stations

`world/home_base.rs` owns every station. Each star system carries `STATIONS_PER_SYSTEM` (2) **full** stations — dock with F and you get build mode, shop, bounty board and hiring at all of them (Haven is no longer special apart from its fixed position and name). Placement is derived, not stored: `station_sites(system_id, local_center)` spreads them on a golden-angle ring 180k-420k out from the system center, so a system's stations never share a screen. Haven is system 0 slot 0 at the fixed `STATION_POS`, beside the spawn berth.

- `SystemStations` (resource) holds the loaded system's sites; `refresh_system_stations` follows `SystemStreamingManager::loaded_system` and `sync_station_entities` spawns/despawns the structures to match. Everything downstream (docking, radar, minimap, M map, contract boards, shop prices) reads that one resource.
- Global station index = `system_id * STATIONS_PER_SYSTEM + slot`; that index keys contract boards (`ContractState::available_by_station`), `station_types::station_type` pricing and `station_display_name`.
- B: toggle build mode, Tab: cycle build categories, [/]: cycle items, R: rotate, M: cycle hull material, X: delete mode
- Build QoL: hull placement supports click-drag painting; delete mode supports drag; hull segments are deletable (75% refund); Ctrl+Z undoes the last paid placement; Ctrl+Click select → Ctrl+C/V copy/paste (R rotates pending paste); Escape backs out paste → selection → build mode → pause
- C: crew menu, M: map/inventory overlay, P: module panel (while paused), ESC: pause, Enter: start/launch
- Controller (`gamepad.rs`, `ControllerLayout` resource): left stick throttle/strafe, right stick aim (persists until mouse moves), RT fire, LT brake, A confirm, B cancel/pause, X interact/dock, Y radar ping, LB cycle target, RB build mode, dpad menu nav, select map. Digital buttons bridge to KeyCodes in PreUpdate; analog goes through `InputState` (`gamepad_aim`). Build mode is still mouse-only.
