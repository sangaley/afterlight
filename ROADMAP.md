# Afterlight — Roadmap

Target: **playable demo on Steam.** Date under review — see "Picking a new date".
Originally Sep 3, 2026, set on Jul 20. That date passed on Sep 3 with the demo
unshipped. This revision (Sep 6) rewrites the plan against what is actually on
master rather than what the Jul 20 plan assumed.

**Read the honest version first:** the Jul 20 → Sep 5 runway went almost entirely
into combat depth. That work is excellent and it is the best part of the game.
It was also not what either previous plan said to spend the time on, twice
running. The gap that remains is not technical.

---

## Where we are

Started 2026-03-26. ~150 commits. Everything in the Jul 19 `qol-batch` list still
holds — build, explore, survive, fight, salvage, economy, repair, discover — plus
the following, all landed since:

- **Combat has real physical depth.** Armour that covers what's behind it, slope
  and ricochet, spall off the back of a plate, six exotic round types, per-block
  hit resolution walking a swept DDA over a per-ship grid, cook-offs, reactor
  meltdowns, ships striking colours, right-click aim lock onto a specific block.
- **Shields** — directional, strength-scaled, drawn as a moving lit segment of
  the ship's own outline rather than a bubble.
- **Collision physics** — ships, stations, asteroids, planets and stars are
  solid; momentum transfer, crash damage, AI avoidance, colliding debris.
- **Persistent galaxy** — 31 systems, hot/warm/cold simulation, discovery, warp,
  two stations per system. **Saved and restored properly.**
- **Enemies are real opponents** — per-faction guns, angle-aware ammunition
  switching, hold-and-shoot movement, heat that makes their guns overheat too.
- **Art** — a full procedural dark-pixel module set, per-footprint, unblocking
  the dependency on an outside artist.
- **Crew walk** (branch `crew-walk`, unmerged) — hallway tiles, A* interior
  navigation, damage severing routes, per-crew standing orders.

### Closed since the Aug 7 audit

Two of the three things that audit called blocking are **done**:

- ~~No onboarding at all~~ — `src/tutorial.rs` landed Aug 13. Ten guided steps:
  launch, thrust, scan, kill a training raider, salvage it, dock, build,
  contracts, crew. Wired and running.
- ~~Save/load doesn't persist galaxy position~~ — landed Aug 11, with quick-save.

### Still open, and now the whole list

1. **No demo goal.** `check_victory` still requires 2200m depth plus the
   `[UNTITLED]` log — a submarine-era condition in a game that has no depth any
   more, only radial distance. Nothing tells a new player what they are for.
2. **Progression gates nothing.** `Unlocks.modules` exists as data and **nothing
   reads it**; only hull *materials* are gated. All 155 modules are buildable in
   the first minute, so there is no ramp and no reason to come back.
3. **"New Expedition" doesn't start a new expedition.** It sets the state and
   nothing else — no reset of credits, inventory, or ship. There is no
   `reset_for_new_game` anywhere.
4. **No music.** 43 SFX across alarms/ambient/engines/impacts/ui/weapons; the
   core loop is covered. There is no `music/` directory.
5. **Steam app registration status unknown.** Flagged Aug 7 as "not sure, need
   info" and never resolved. **This is the only item that can block a date no
   matter how the code goes** — store review has its own lead time.

---

## Phase A — Give the demo a shape (~1.5 wks)

Nothing here is a new system. It is all deciding what the demo *is*.

- **Replace the victory condition** with something a demo player can reach in
  30–60 minutes and understand from the first screen. Retire the depth check.
- **Make progression real** — wire `Unlocks.modules` into the build palette so
  the ramp exists. The galaxy map is the natural spine: warp farther, meet worse
  factions, earn better hulls.
- **Make New Expedition reset the run.** Currently a fresh start inherits your
  last one.

## Phase B — Harden (~1.5 wks)

- Full playthrough passes from a cold start, looking for cliffs and dead ends.
- Balance the economy/salvage loop against the new progression gate.
- Merge `crew-walk`. Decide the two parked items: the `starter.json` armour
  drift (67 plates shipped vs 75 generated), and `crew_weapon_system` being dead
  code — which also means weapons auto-firing at creatures has never worked.
- Feature freeze. Bugs and balance only.

## Phase C — Ship

- Steam store assets against real in-game art, short trailer, submit with buffer.
- **Music is the stretch, not a requirement.** Cut it before cutting Phase A.

## Picking a new date

Phases A+B are ~3 weeks of work at 20–30 hrs/week, so **mid-October 2026** is
achievable *if* the Steam app is already registered. If it is not, resolve that
first — it is the one dependency that doesn't care how fast you code.

Do not set the date until the Steam question is answered. That is what made both
previous dates fiction.

---

## Development history

**2026-03-26 → 2026-07-19.** Submarine game → space conversion → combat,
building, destruction, salvage, economy, QoL, AI crew/power simulation, UI pass.
61 commits merged to master as `qol-batch` on Jul 19.

**2026-07-20.** First roadmap. Deadline set to Nov 12, moved to Sep 3 same day.
Galaxy map scope explicitly walked back to "seeded generation only", with the
runway earmarked for progression and onboarding.

**2026-08-07.** Full galaxy map built anyway — multi-system warp, map UI,
persistent hot/warm/cold simulation. Good work, and it consumed the Phase 2
window it had been warned against. Audit found onboarding, galaxy persistence and
progression as the blocking gaps.

**2026-08-09 → 08-17.** Procedural module art. Galaxy persistence fixed.
Tutorial written. Collision physics. Shields. Stations in every system.

**2026-08-24 → 09-05.** Combat depth, continuously: armour coverage, slope,
ricochet, spall, exotic ammunition, per-block hit resolution, wedge geometry,
angle-aware AI, heat parity. Roughly forty commits. This is the strongest part of
the game and none of it was on either plan.

**2026-09-05/06.** Crew walking: interior navigation, hallways, damage control,
standing orders. Roadmap rewritten against reality (this revision).

---

## Decisions locked in

- **2026-07-20:** Distribution = Steam. Time budget ~20-30 hrs/week.
- **2026-09-05:** Boarding, room-to-room crew combat and giving enemy crew bodies
  are **deferred until after release** — the user's own standing instruction,
  precisely because it is the feature they most want to build.
- **2026-09-06:** No new date until the Steam registration question is answered.

## Post-ship backlog

- Boarding / room-to-room crew combat / crew bodies on enemy ships (see above)
- Arrival gating staffing — travel time as a real cost
- Tractor beam + hangar salvage; deferred module mechanics; full music pass
- Stealth mechanics + alert ramping
