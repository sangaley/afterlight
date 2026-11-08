# Parked features

Complete but **intentionally disabled** features, kept out of the build so they
don't clutter (or compile into) the active tree. Nothing here is wired into
`main.rs`, so these files are **not compiled** — they may need small fixups
against the current codebase when revived.

## abyss_horror.rs
An "abyss horror" ambience layer built around real creatures watching/fleeing
the player. Disabled because creature spawning is currently off, which left it
producing false scares (phantom blips) with nothing behind them.

**To re-enable:** move `abyss_horror.rs` back to `src/`, re-add `mod abyss_horror;`
and `use abyss_horror::AbyssHorrorPlugin;` in `main.rs`, and add `AbyssHorrorPlugin`
to the plugin list (it sits by the Radar/Camera plugins). Needs creature spawning on.

## creatures/
The whole creature layer: spawning, AI, the ecosystem and food chain, corpses,
behaviours. 2,029 lines.

Parked 2026-09-21 by decision, not by accident. It had already been switched
off by an unconditional `return` at the top of `spawn_creatures` with the note
*"Creatures disabled per playtest feedback — more annoying than scary."* — so
the code ran over empty queries every frame and produced nothing.

Worse, the contract board carried on issuing jobs that depended on it. A
playthrough accepted "Kill 4 VoidDrifters" and then flew around for seven
minutes with nothing in the universe to kill. Kill and CaptureLive are no
longer generated.

**To revive:** move `creatures/` back to `src/`, re-add `mod creatures;` and
`use creatures::CreaturePlugin;` in `main.rs` with the plugin in the list,
delete the early `return` in `spawn_creatures`, and put Kill/CaptureLive back
into `contracts::generation::weighted_contract_types`. `abyss_horror.rs` in
this directory depends on live creatures and can come back at the same time.
