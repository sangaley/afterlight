# Fleet rework: drawing queue

Decisions (2026-11-18): about 20 ships per faction, 3 of them traders; approved hulls kept and refit
(weapons, armour, missile and shell designs, paint, interior lights); new ships drawn in each faction's
approved grammar; skeletons mirror-symmetric, asymmetry only as small details. Recursive Kingdom is
reviewed first; the other factions' new ships are drawn now, but their refit, paint and renders wait
until the developer has checked the Recursive Kingdom on the site.

Each faction gets two drawing jobs: `<module>_v5a.py` (8 light and medium warships) and
`<module>_v5b.py` (heavies, a flagship where missing, 2 experimentals, 3 traders). About ten agents run
at once on the 8 GB Mac; start the next job when one finishes.

Paint: every faction from its in-game livery (refit.py LIVERY). Story: each brief carries the
faction's lore from space-empire-story-bible.md and capital_worlds.rs.

Status 2026-11-18: ALL DRAWING DONE and committed (12 factions, ~260 ships). Refit done for all. Renders running (render_stream.sh in the session scratchpad, out/<slug>). Next: site.py (all factions), publish in two batches (file limit 255/publish), user verdicts. RK verdicts: 17 approved; hive, parliament, sierpinski, spiral unreviewed. Open: RK Ram shares a name with the approved Shepherd Ram (rename the RK one before integration).
Left to launch: none.

## Done (older notes)
- recursive_kingdom: rk_light, rk_mid, rk_heavy, rk_trade (rk_heavy keeps a spare sierpinski() out of CONCEPTS)

## Running
- recursive_kingdom: rk_heavy2, rk_heavy3
- broken_choir: choir_v5a, choir_v5b
- the_silence: silence_v5a, silence_v5b
- stellar_preserve: preserve_v5b (preserve_v5a done)
- merchant_princes: merchants_v5a, merchants_v5b
- free_haulers_compact: compact_v5a

## Waiting
- free_haulers_compact: compact_v5b (trading faction: large haulers, flagship, experimentals)
- synthesis_collective: synthesis_v5a, synthesis_v5b
- gilded_throne: gilded_v5a, gilded_v5b
- corpse_stars: corpse_v5a, corpse_v5b
- terran_hegemony: terran_v5a, terran_v5b (has 7 ships: about 13 new)
- eternal_hegemony: eternal_v5a, eternal_v5b (boss faction)
- shepherd: shepherd_v5a, shepherd_v5b (boss faction)
