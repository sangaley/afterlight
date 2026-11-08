# Demo Checklist

What has to be true before Afterlight goes out as a playable demo.
Compiled 2026-09-20 from three audits of the tree at `cascade-story`.

`ROADMAP.md` is the plan; this is the punch list. Where the two disagree, this
file is newer.

Legend: `[ ]` open · `[x]` done · `[~]` partly done · `[-]` cut for after the demo.

**Status 2026-09-22:** 30 items closed on `cascade-story`, verified by an
eight-minute autoplay run of the real loop: no panics, credits growing, fuel
draining on throttle, plating unlocking, cascade climbing to 0.17 across two
warps. Remaining open items are listed below.

**Since:** distance now costs money (docking no longer fills the tank; prices
scale to 3x at the galaxy's edge), the expedition trail gives the player a
stated objective with a demo wall at tier 1, and hulks can be towed to a
station for the rare loot a boarding party cannot carry. Creatures stay off by
decision; the Kill contracts that depend on them are still being issued and
remain open below.

**Enemy ships have an inside.** Their hulls carried no hallway cells, so there
was nothing to see and the nav grids built for them came out empty. They get a
derived deck now. Everything inboard starts concealed and lights up two cells
around a hole, and a breach vents crew onto the same drifting-dead register the
airlock writes to. Living enemy crew still have no body, which is what keeps
them out of every player-only system and is the line this work does not cross.

**The starter ship is new.** The old hull was an isosceles wedge, mirror-
symmetric, and six of the ten faction hulls are symmetric lozenges too, so the
player's own ship read as one more of them. The replacement leans: command
tower forward and high, cargo keel aft and low, engineering at the stern, the
working end at the bow. Two doors divide the main corridor. Building it turned
up a memory-core placement rule that walled the engine room off from its own
crew, and closed the long-standing drift between `designs/starter.json` and
the Rust builtin.

**Towing is verified.** It had never been exercised: its only tests covered
the loot maths, and nothing had ever latched onto a hulk. Eighteen tests now
drive the real systems and found two defects. The beam check counted every
module in the world, so a Stellar Preserve ship anywhere in the system let you
tow with nothing aboard. And a derelict kept firing at the salvager, because
the fire system gated on a behaviour field that nothing updates after death.
Both fixed. Coming within latch range now says once whether you can tow and,
if not, what to fit.

---

## 1. Blockers — a player gets stuck, confused, or loses their save

- [x] **Tutorial step 13 of 14 cannot be completed.** It says "use the Crew
      button". The docked toolbar has no Crew button, and the crew menu is
      gated to `Exploring`, so pressing C while docked does nothing either. The
      player sits on 13/14 until they find `;` to dismiss.
      `tutorial.rs:422`, `ui/mod.rs:221`, `ui/mod.rs:370`
- [x] **The hull palette places the wrong block.** Seven hull items, six
      labels. "VOD" lays a Hallway, "BLK" lays Void, "ANG" lays a Bulkhead
      Door. Angled Hull Plate has no slot at all. The comment above the list
      says the order must match; it has not since `Hallway` was inserted.
      `ui/build_ui.rs:960-967` vs `resources.rs:908-925`
- [x] **Autosave writes enemy ships into the player's save.** Three unfiltered
      queries; AI hull, modules and crew carry the same components. Hull cells
      are recorded from *local* transforms, so an enemy plate at its own (2,3)
      is stored as yours. Fires every two minutes while exploring. The load
      path has the mirror defect: its despawn queries are unscoped too, so
      loading guts every live AI ship.
      `meta.rs:120-122`, `:279-281`, `:428-430`
- [x] **Nothing states a goal.** Menu says "Build your ship. Explore the void.
      Survive." The tutorial ends on "push deeper". No objective is ever named.
      `ui/mod.rs:3556`, `tutorial.rs:130`
- [x] **"All crew died" never fires while any enemy lives.** The crew query is
      unscoped, so one living AI crewman anywhere keeps it false. The player's
      whole crew can die and the run continues as a ghost ship.
      `ship/systems.rs:44`
- [x] **The player can launch with zero crew.** Starter-crew spawn is suppressed
      by any living AI crew. Combined with the above, a total-crew-loss run can
      neither end nor recover by docking. `crew/mod.rs:277-283`
- [x] **Docking services bill and heal across ship boundaries.** Repair Hull
      full-heals every AI ship in the world on the player's credits; Repair
      Modules charges for enemy battle damage; hire cost and the "berths full"
      check count enemy crew. `ui/mod.rs:4437-4442`, `:4530`, `:4824`

## 2. Introduced by the story work — mine to fix

- [x] **A second expedition can never reach the ending.** `discovered_logs` is
      a system `Local`, which outlives the run. `reset_for_new_game` clears
      `Statistics.logs_found` but cannot touch a `Local`, so every log read in
      run one — the finale included — is permanently unreadable until the
      process restarts. `world/mod.rs:327-338`
- [x] **`check_poi_discovery` is the unfixed twin of the log bug.** Log
      discovery was moved to `PostUpdate` for unpropagated transforms; its
      neighbour 150 lines up was left in `Update`. Every streamed point of
      interest reads as sitting at the origin on its first frame: toast spam
      and phantom contract completions. `world/mod.rs:46`, `:170-212`
- [x] **The ending plays in total silence.** `stop_flight_loops` fires on
      `OnExit(Exploring)` and `Truth` is entered from `Exploring`. `audio.rs:41`
- [x] **The built-in starter ship has no Memory Cores.** Three were added to
      `starter.json` but not to `builtin_starter_design()`. If the JSON is ever
      regenerated the core death condition and the autonomy mechanic silently
      switch off — the same fallback trap as the faction designs.
      `ship/spawner.rs:123`

## 3. The loop

- [x] **Four of seven contract objectives were dead or free.** All four now
      measure `distance_from_safety` (range to the nearest berth) instead of
      `DepthState.current_depth`, which was distance from the shared world
      origin and so read ≥180,000 in every system but Haven. `Kill` and
      `CaptureLive` both depended on creatures and are no longer generated at
      all; their weight went to the objectives that resolve. The two variants
      stay in the enum as the obvious home for a future hunt-a-ship job.
- [x] **Reach and survey contracts were free, for a second reason.** The
      measurement was fixed earlier; the targets never were. `contracts::
      tracking` also carried a private copy of the zone function still using
      the old 200/500/1000/2000 submarine thresholds that
      `world::depth_to_zone` abandoned when cruise speeds went up — so the
      board and the HUD disagreed about where the player stood, and a
      five-star survey asking for Black Hole Proximity was satisfied 2,000
      units off the berth while the HUD called that Near Space. The duplicate
      is deleted and reach targets are rescaled to the zone each star names.
      Three tests tie the two tables together.
- [x] **Difficulty is a cliff, not a ramp.** Creature biome, density and spawn
      rate read the same broken distance and saturate everywhere outside Haven.
      One warp takes you from harmless drifters to 1500-HP leviathans at
      maximum spawn rate. Fix: scale off the system's own `danger_tier`, which
      is already an authored weak-near/strong-far curve.
      `world/mod.rs:128-155`, `creatures/mod.rs:101-107`
- [x] **Crossing the galaxy is free.** Max jump is 500 fuel of a 1500 tank,
      four seconds, and refuel is free at every dock. A minute-one player can
      click the far edge and arrive parked beside a station in the hardest
      territory in the game. `celestial/warp.rs:15-28`
- [x] **Fuel burns while merely powered, not while thrusting.** Five engines at
      1.0 each is 4/sec sitting still: about six minutes a tank. The tutorial
      says "watch FUEL tick down as you burn", which is not what happens.
      `ship/movement.rs:255-278`
- [~] **Progression gates nothing.** Hull materials now unlock by how far out
      the player has actually been, so the gate opens and there is a reason to
      go. The build palette no longer hides anything: 40 modules were in no
      menu at all, including `ShieldEmitter` (shield strength was frozen for
      the whole run), `MemoryCore` (losing every one ends the run) and the
      entire Structural category, which had no `BuildCategory` variant and so
      took flat Armor Plate with it. 143 of 157 are now offered; the other 14
      are weapon subcomponents fitted in the customisation panel, superseded
      passage tiles, and one inert module, all listed in `NOT_IN_PALETTE` with
      a reason. Three tests in `building/registry.rs` and two in `resources.rs`
      keep the lists from drifting apart again.
      Still open: `Unlocks.modules` has no reader.
- [x] **Deposits are checked but never charged.** Accepting a contract is a
      free option with a downside only on death. `contracts/ui.rs:176-184`
- [x] **Blind warp can strand you.** Landing further than `SNAP_TOLERANCE` from
      any system spawns nothing — no station, so no refuel. Six minutes later
      you cannot jump out. `celestial/warp.rs:233-243`

- [x] **Your own missiles blew up inside your ship, about one launch in five.**
      Not a new fault — the old hull did it too, 3 cook-offs in 22 launches;
      the new hull's more enclosed tubes made it 8 in 30. `move_missiles` has
      always said blocks to the left and right of a tube guide a warhead
      rather than stopping it, and holds its heading while it threads its own
      ship — but the collision check cooked it off on *any* cell it touched.
      It touches plenty: the missile keeps the world-space velocity it
      launched with while the ship turns and thrusts underneath it, so the
      hull swings across a warhead flying straight down its own tube. An
      obstruction is now a cell on the tube's axis ahead of the mouth, which
      is what the build-time silo check already guarantees against. Measured
      after: 0 cook-offs in 45 launches.

## 4. Story reachability

- [x] **Give the player a thread to follow.** The finale log sits in a
      30,000–100,000 unit ring around one of six far systems and appears on no
      radar, map or minimap. Reuse the nav arrow and map marker that
      `contracts/bounty_nav.rs` already draws for DestroyShip contracts.
- [x] **The dread audio bed was inaudible for the whole demo.** Both layers
      started at cascade 0.30 *and ramped to full only at 1.0*, so even after
      lowering the start the first layer sat at about five per cent of the
      drone it was under. A layer can now name the level it peaks at, not just
      the one it starts at: the low bed runs 0.10 → 0.45, which a demo session
      (around 0.17) is inside. The tests read the real spawn values now — they
      used to repeat the numbers as literals, which is exactly why this was
      tunable out of reach without anything failing.

## 5. Sound — all from files already licensed and in the repo

- [x] **~50 seconds of opening silence.** No menu audio, no station ambience.
      `interior_hum.ogg` and `machine_loop_1-3.ogg` are sitting unused.
- [x] **Taking a hit makes no sound.** `ShipDamaged` has no handler; projectile
      impacts are explicitly unwired. Only a fully destroyed hull tile is
      audible. `combat/new_projectiles.rs:822`
- [x] **Pausing hard-cuts the entire soundscape** and restarts it from sample
      zero on resume, because `stop_flight_loops` fires on `OnExit(Exploring)`.
- [x] **Nothing marks launch**, the most cinematic beat in the first minute.
- [x] **`DockingCompleted` is dead code** — a handler with no writer. Wire it
      or delete it. `audio.rs:559`
- [x] **The main menu was silent.** Not the station half of this — that was
      fixed earlier — the menu itself. The first sound in the game arrived
      about fifty seconds in, at launch, and until then the only evidence the
      audio worked at all was that the buttons clicked. It now carries the deep
      drone, fading up over four seconds and sitting under the flight bed.
- [~] Unused licensed audio is down to six files (1.5 MB) from ten.
      `machine_loop_3` joined the berth rotation. Three `engine_*_loop` were
      deliberately rejected and stay out. The remainder are `alarm_loop_2` and
      two creature-themed ambiences (`alien_hive`, `alien_planet`) with nothing
      to attach to while creatures are off.

## 6. Cut for after the demo

- [-] Music. The bus drives one ambient drone. Biggest perceived-quality gain
      available and the only item that depends on assets not yet acquired.
- [-] Key rebinding. The controls tab is a read-only cheatsheet.
- [-] Colourblind options, despite a green/red build ghost and a
      green/yellow/red damage overlay.
- [-] Mines and manual fire: four systems exist and are registered nowhere, yet
      Mine ammo is issued and the UI says "Mine deployed!".
- [-] `crew_weapon_system` — weapons auto-firing at creatures has never worked.
- [-] Radiation (`ship/radiation.rs` entirely unregistered), `ResearchState`
      (write-only), `AICombatCore` (registered no-op), `Unlocks.blueprints_found`
      and `Statistics.ships_lost` (both dead fields).
- [~] Main menu art and motion. The parallax starfield the game already draws
      was gated out of `MainMenu`; it is in now, behind a backdrop thinned from
      98% to 72% so it shows through. Thinning it also revealed that the whole
      flight HUD — hull, power, fuel, credits, the control hints — had been
      sitting visible behind the title screen since the first frame, hidden
      only by the opaque backdrop. `setup_ui` runs in Startup and the game
      opens on the menu, so `hide_hud` on entering MainMenu had nothing to
      hide yet. The HUD now spawns hidden and `show_hud` reveals it on entering
      flight or a berth. Still open: motion, and any actual art.

## 7. Not mine to close

- [ ] **Steam app registration.** The only item that can block a date
      regardless of the code. Owner: you.
- [ ] Replace the 128 kbps Freesound previews for `engine_rumble_loop` and
      `hostile_atmosphere_loop` — `CREDITS.md` says to do this once a sound
      becomes prominent, and both now are.
- [ ] Decide whether celestial wrecks should appear on radar and map. Affects
      ordinary salvage as well as the ending.

---

## Corrections to ROADMAP.md

- `crew-walk` is **already merged** into master; Phase B lists it as pending.
- The parked "`starter.json` armour drift, 67 plates shipped vs 75 generated"
  note is stale: the file is now 120 hull cells, 71 modules, 32 plates.
- Open items 1 and 3 ("no demo goal", "New Expedition doesn't reset") are
  closed in code, though item 1's second half — *nothing tells a new player
  what they are for* — is still true.

## Branches

Two unmerged lanes, overlapping in four files (`components.rs`, `crew/mod.rs`,
`debug.rs`, `ui/mod.rs`): `suit-damage-control` (4 commits) and `cascade-story`
(15). Land the smaller one first.
