#!/usr/bin/env python3
"""Build the fleet rework review site: one page, every faction done so far.

For each ship: the painted shell and the lit interior as the game draws them
(render.py), the hand-drawn concept grid, what the refit gave it (refit.py),
and Approve / Rework / Reject with a note, kept in the artifact's database
(collection `verdicts`, doc `<slug>__<key>`: {verdict, note, at}).

    python3 tools/fleet-rework/site.py <slug> [<slug> ...]

Writes tools/fleet-rework/site/ (index.html and img/).
"""
import html, importlib, json, os, shutil, sys

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
CONCEPT_DIR = os.path.join(ROOT, "tools", "fleet-concepts")
sys.path.insert(0, CONCEPT_DIR)
import fleet  # noqa: E402  (the concept renderer)

SITE = os.path.join(HERE, "site")
OUT = os.path.join(HERE, "out")
REWORK = os.path.join(ROOT, "designs", "rework")

# The concept modules each faction's ships are drawn in, in page order.
MODULES = {
    "recursive_kingdom": ["rk_light", "rk_mid", "rk_trade", "rk_heavy", "rk_heavy2", "rk_heavy3"],
    "terran_hegemony": ["terran", "terran_v5a", "terran_v5b"],
    "eternal_hegemony": ["eternal", "eternal_v5a", "eternal_v5b"],
    "gilded_throne": ["gilded", "gilded_v5a", "gilded_v5b"],
    "merchant_princes": ["merchants", "merchants_v5a", "merchants_v5b"],
    "free_haulers_compact": ["compact", "compact_v5a", "compact_v5b"],
    "synthesis_collective": ["synthesis", "synthesis_v5a", "synthesis_v5b"],
    "stellar_preserve": ["preserve", "preserve_v5a", "preserve_v5b"],
    "broken_choir": ["choir", "choir_v5a", "choir_v5b"],
    "corpse_stars": ["corpse", "corpse_v5a", "corpse_v5b"],
    "the_silence": ["silence", "silence_v5a", "silence_v5b"],
    "shepherd": ["shepherd", "shepherd_v5a", "shepherd_v5b"],
}

# Ships redrawn off the arrowhead after the first full review (2026-11-18).
RESHAPED = {"eternal_hegemony": {"thurible", "remembrancer"}, "corpse_stars": {"memento", "sarcophagus"},
            "the_silence": {"mirage", "almoner"}, "broken_choir": {"keen"}, "shepherd": {"briar"},
            "merchant_princes": {"claimstaker"}, "gilded_throne": {"retainer"}}

# What the refit's report calls things, as a player would.
NAMES = {
    "CIWS Turret": "CIWS point-defence turret",
    "Light Autocannon": "Light autocannon turret",
    "Compact 170 mm Turret": "Compact 170 mm turret",
    "170 mm Turret": "170 mm turret",
    "Twin 225 mm Turret": "Twin 225 mm turret",
    "280 mm Spinal Gun": "280 mm spinal gun",
    "VLS Missile Silo": "VLS missile silo",
    "laser unit": "Laser unit",
    "Missile Warner": "Missile warner",
    "DecoyLauncher": "Decoy launcher",
    "ChaffDispenser": "Chaff dispenser",
}
ARMOUR = {"AblativeArmor": "ablative", "AngledAblativeArmor": "ablative", "ReflectivePlate": "reflective",
          "AngledReflectivePlate": "reflective", "ArmorPlate": "steel", "AngledArmorPlate": "steel"}
EXTRA = {"SalvageArm": "Salvage arm", "TractorBeam": "Tractor beam", "MiningDrill": "Mining drill", "DroneBay": "Drone bay",
         "BulkCargoHold": "Bulk cargo hold", "LargeCargo": "Large cargo", "SmallCargo": "Small cargo",
         "DockingHub": "Docking hub", "DockingPort": "Docking port", "PlasmaCaster": "Plasma caster",
         "IonDisruptor": "Ion disruptor", "EMPPulse": "EMP pulse", "SignalJammer": "Signal jammer"}
SCALE = 22.0  # game units per pixel in the lineup

# The trim (cuts.json), approved by the developer on 2026-11-18: per faction,
# the ships taken out of the roster and why. They are left off the site.
_CUTS_PATH = os.path.join(HERE, "cuts.json")
CUTS = json.load(open(_CUTS_PATH)) if os.path.exists(_CUTS_PATH) else {}


def section_of(cls):
    c = cls.lower()
    if "station" in c or "platform" in c:
        return 4
    if "trade" in c or "hauler" in c or "civilian" in c:
        return 3
    if "experimental" in c:
        return 2
    return 1


def webp(src, dst, max_w):
    im = Image.open(src).convert("RGBA")
    if im.width > max_w:
        im = im.resize((max_w, round(im.height * max_w / im.width)), Image.LANCZOS)
    im.save(dst, "WEBP", quality=88, method=6)
    return im.size


def views(shell_png, inside_png, long_side=1500, gap=24):
    """Shell and lit interior in one image: side by side for a tall ship, one
    above the other for a wide one. Returns the image and True if side by side."""
    a = Image.open(shell_png).convert("RGBA")
    b = Image.open(inside_png).convert("RGBA")
    side = a.height >= a.width
    if side:
        h = min(max(a.height, b.height), 900)
        a = a.resize((max(1, round(a.width * h / a.height)), h), Image.LANCZOS)
        b = b.resize((max(1, round(b.width * h / b.height)), h), Image.LANCZOS)
        out = Image.new("RGBA", (a.width + gap + b.width, h), (0, 0, 0, 0))
        out.alpha_composite(a, (0, 0)); out.alpha_composite(b, (a.width + gap, 0))
    else:
        w = min(max(a.width, b.width), 1200)
        a = a.resize((w, max(1, round(a.height * w / a.width))), Image.LANCZOS)
        b = b.resize((w, max(1, round(b.height * w / b.width))), Image.LANCZOS)
        out = Image.new("RGBA", (w, a.height + gap + b.height), (0, 0, 0, 0))
        out.alpha_composite(a, (0, 0)); out.alpha_composite(b, (0, a.height + gap))
    if max(out.size) > long_side:
        k = long_side / max(out.size)
        out = out.resize((round(out.width * k), round(out.height * k)), Image.LANCZOS)
    return out, side


def missile_text(m):
    if not m:
        return ""
    head = {"DumbNose": "no seeker", "Radar": "radar seeker", "Heat": "heat seeker", "CommandLink": "command link"}.get(m["head"], m["head"])
    return f'{m["motor"]} motor; {", ".join(m["body"])}; {head}'


def faction(slug, ships_out):
    mods = []
    for m in MODULES[slug]:
        if not os.path.exists(os.path.join(CONCEPT_DIR, m + ".py")):
            continue
        try:
            mods.append(importlib.import_module(m))
        except Exception as e:  # a module another agent is still writing
            print(f"  {m}: not ready ({e.__class__.__name__}), left out")
    if not mods:
        return None
    info = mods[0].FACTION
    img = os.path.join(SITE, "img", slug)
    shutil.rmtree(img, ignore_errors=True)  # only this build's pictures get published
    os.makedirs(img, exist_ok=True)
    ships, removed = [], []
    for mod in mods:
        for key, _fn, name, cls, note in mod.CONCEPTS:
            if key in CUTS.get(slug, {}).get("cut", {}):
                removed.append(name)
                continue
            rep_path = os.path.join(REWORK, slug, f"{key}.report.json")
            shell = os.path.join(OUT, slug, f"{key}_shell.png")
            if not (os.path.exists(rep_path) and os.path.exists(shell)):
                print(f"  {slug}/{key}: not refit or rendered yet, left out")
                continue
            rep = json.load(open(rep_path))
            design = json.load(open(os.path.join(REWORK, slug, f"{key}.json")))
            meta = json.load(open(os.path.join(OUT, slug, f"{key}.meta.json")))
            counts = {}
            for m in design["modules"]:
                counts[m["module_type"]] = counts.get(m["module_type"], 0) + 1
            armour = {}
            for t, kind in ARMOUR.items():
                if counts.get(t):
                    armour[kind] = armour.get(kind, 0) + counts[t]
            weapons = [(NAMES.get(k, k), n) for k, n in sorted(rep["report"].items(), key=lambda kv: -kv[1])
                       if k in NAMES and n > 0]
            extras = [(label, counts[t]) for t, label in EXTRA.items() if counts.get(t)]
            # One picture per ship (shell beside or above the lit interior) keeps
            # the whole fleet under the artifact's file limit.
            pair, side = views(shell, os.path.join(OUT, slug, f"{key}_inside.png"))
            pair.save(os.path.join(img, f"{key}.webp"), "WEBP", quality=86)
            ships.append({
                "key": key, "name": name, "cls": cls, "note": note, "section": section_of(cls),
                "concept_hull": rep["concept_hull"], "hull": rep["hull"], "blocks": rep["blocks"],
                "weapons": weapons, "armour": armour, "extras": extras,
                "missile": missile_text(rep.get("missile")) if counts.get("VlsSilo") else "",
                "pair": pair.size, "side": side, "zoom": meta["shell_zoom"],
            })
    ships.sort(key=lambda s: (s["section"], s["hull"]))
    # The lineup: every shell to one scale, smallest first, in rows.
    tiles = []
    for s in ships:
        im = Image.open(os.path.join(OUT, slug, f"{s['key']}_shell.png")).convert("RGBA")
        k = s["zoom"] / SCALE
        tiles.append((s["key"], im.resize((max(1, round(im.width * k)), max(1, round(im.height * k))), Image.LANCZOS)))
    width, x, y, row_h, placed = 1500, 0, 0, 0, []
    for key, im in tiles:
        if x and x + im.width > width:
            x, y, row_h = 0, y + row_h + 28, 0
        placed.append((key, im, x, y))
        x += im.width + 28
        row_h = max(row_h, im.height)
    lineup = Image.new("RGBA", (width, y + row_h + 4), (0, 0, 0, 0))
    for key, im, px, py in placed:
        lineup.alpha_composite(im, (px, py))
    lineup.save(os.path.join(img, "_lineup.webp"), "WEBP", quality=88)
    ships_out.extend((slug, s["key"]) for s in ships)
    return {"slug": slug, "info": info, "ships": ships, "lineup": lineup.size, "removed": removed}


SECTION_NAMES = {1: "Warships", 2: "Experimental", 3: "Traders and civilians", 4: "Station"}


def ship_card(slug, s):
    e = html.escape
    weapons = "".join(f'<li><span class="n">{n}</span> {e(w)}</li>' for w, n in s["weapons"]) or '<li class="none">No guns</li>'
    if s["missile"]:
        weapons += f'<li class="sub">Missile: {e(s["missile"])}</li>'
    armour = ", ".join(f"{n} {k}" for k, n in s["armour"].items()) or "none"
    extras = "".join(f'<li><span class="n">{n}</span> {e(w)}</li>' for w, n in s["extras"])
    doc = f"{slug}__{s['key']}"
    reason = CUTS.get(slug, {}).get("cut", {}).get(s["key"])
    cut = (f'<p class="cutnote"><b>Proposed cut.</b> {e(reason)} '
           f'<span>Reject to cut it, Approve to keep it.</span></p>') if reason else ""
    return f'''
<article class="ship{' proposed-cut' if reason else ''}" id="{doc}">
  <header class="ship-head">
    <div>
      <p class="cls">{e(s["cls"])}{' <span class="reshaped">New shape</span>' if s["key"] in RESHAPED.get(slug, ()) else ''}</p>
      <h3>{e(s["name"])}</h3>
    </div>
    <dl class="size">
      <div><dt>Drawn</dt><dd>{s["concept_hull"]} hull</dd></div>
      <div><dt>In game</dt><dd>{s["blocks"]:,} blocks</dd></div>
    </dl>
  </header>
  {cut}
  <p class="note">{e(s["note"])}</p>
  <figure class="pair"><img src="img/{slug}/{s["key"]}.webp" width="{s["pair"][0]}" height="{s["pair"][1]}" alt="{e(s["name"])}: painted shell and lit interior" loading="lazy"><figcaption>{"Left" if s["side"] else "Top"}: painted shell. {"Right" if s["side"] else "Bottom"}: inside, lit.</figcaption></figure>
  <div class="facts">
    <div><h4>Weapons and defence</h4><ul>{weapons}</ul></div>
    <div><h4>Armour</h4><p>{e(armour)}</p>{f'<h4>Fittings</h4><ul>{extras}</ul>' if extras else ''}</div>
  </div>
  <div class="verdict" data-doc="{doc}">
    <div class="choices" role="group" aria-label="Verdict for {e(s["name"])}">
      <button type="button" data-v="approve">Approve</button>
      <button type="button" data-v="rework">Rework</button>
      <button type="button" data-v="reject">Reject</button>
    </div>
    <label class="note-field"><span>Note</span><textarea id="note-{doc}" rows="2" placeholder="What to change, if anything"></textarea></label>
    <p class="saved" aria-live="polite"></p>
  </div>
</article>'''


# How each faction fights (the game's ai_ship/doctrine.rs), what it carries
# to do it, and how you beat it.
DOCTRINE = {
    "terran_hegemony": ("The line", "Holds long range in a battle line, every gun on one target, armour-piercing missiles in one timed salvo; the best aim in the game.",
                        "Long rifles firing solid armour-piercing darts.", "Get inside their range or round their flank: they turn slowly."),
    "eternal_hegemony": ("The throne", "Every fleet fights round its command ship and never gives ground while it stands; guns first, then the reactor, the biggest prey first.",
                         "The old pattern's lances, armour-piercing with a burst behind.", "Kill the command ship and the fleet breaks."),
    "gilded_throne": ("Hired precision", "Lasers from long range, working round your flank, your guns then your drive. Not paid to die: they leave when it gets costly.",
                      "Laser banks and mirror plate.", "Plain shells (the mirror plate only turns lasers), and pressing them."),
    "merchant_princes": ("Business", "Escorts screen with flak while the traders run, and shoot your engines so you stop chasing. Nobody fights to the death.",
                         "Proximity flak guns.", "Thin armour, but they call the Gilded Throne."),
    "free_haulers_compact": ("Brawl", "Cheap crews who close hard in groups that answer each other's calls from a long way off.",
                             "Breakers and scatterguns: high explosive from short barrels.", "Keep your distance: they have no reach."),
    "synthesis_collective": ("The swarm", "Small hulls rushing in from several sides, ion fire on your power, then plasma up close; the small and weak first.",
                             "Ion disruptors that knock systems out, plasma casters that burst, tractor tendrils.", "Point defence, gatlings and area damage; keep your shields up."),
    "stellar_preserve": ("Catch and keep", "They don't trade blows: they stop a ship (drive, then power) and tow it away, preferring the ones already failing.",
                         "EMP missiles and tractor booms.", "Keep your engines and power up, or break the tractors."),
    "broken_choir": ("The scream", "Ghosts that come straight on and never flee, jamming your radar and seekers, missiles in waves.",
                     "Signal jammers by the hundred, missile silos.", "Guns, not missiles; the jammers are big targets."),
    "corpse_stars": ("Entropy", "Ion and EMP on your drive and reactor, then the bone prow. They grow stronger as they break, and only a gutted ship turns away.",
                     "Ion disruptors, EMP pulses, railguns.", "Stay mobile and keep the range."),
    "the_silence": ("They leave", "Never shoot. Stealth keeps them off your scope until they're close; they shadow you, and if attacked they jam and slip away.",
                    "Stealth coating, jammers, no guns.", "Nothing to beat: they were only watching."),
    "shepherd": ("The hook", "A lone apex hunter that tractors one ship out of your group into its hook, then ion and plasma at close range.",
                 "Tractor beams lining the hook, ion disruptors, plasma casters.", "Stay together, keep your power up, shoot out the hook's beams."),
    "recursive_kingdom": ("Numbers", "Cheap identical copies that fly straight in, each on whatever is nearest, and let every rocket go at once.",
                          "Scrap guns throwing high explosive, dumb-rocket silos, scrap plate.", "Sidestep the salvo; gatlings and flak shred them."),
}


def doctrine_block(slug):
    d = DOCTRINE.get(slug)
    if not d:
        return ""
    e = html.escape
    name, how, gun, beat = d
    return (f'<div class="doctrine"><p><b>How they fight: {e(name)}.</b> {e(how)}</p>'
            f'<p><b>Carries:</b> {e(gun)} <b>Beat them:</b> {e(beat)}</p></div>')


def trim(slug, f):
    c = CUTS.get(slug)
    if not c:
        return ""
    gone = f.get("removed", [])
    if not gone:
        return ""
    return (f'<p class="trim"><b>Trimmed to {len(f["ships"])} ships.</b> Removed: {html.escape(", ".join(gone))}.</p>')


def page(factions):
    e = html.escape
    body = []
    total = sum(len(f["ships"]) for f in factions)
    for f in factions:
        info, slug = f["info"], f["slug"]
        grammar = "".join(f"<li>{e(g)}</li>" for g in info["grammar"])
        sections = []
        for sec in (1, 2, 3, 4):
            cards = [ship_card(slug, s) for s in f["ships"] if s["section"] == sec]
            if cards:
                sections.append(f'<h3 class="section">{SECTION_NAMES[sec]} <span>{len(cards)}</span></h3>' + "".join(cards))
        body.append(f'''
<section class="faction" id="{slug}" style="--faction:{info["accent"]}">
  <header class="faction-head">
    <p class="eyebrow">Faction</p>
    <h2>{e(info["name"])}</h2>
    <p class="tagline">{e(info["tagline"])}</p>
    <ul class="grammar">{grammar}</ul>
    {doctrine_block(slug)}
    {trim(slug, f)}
  </header>
  <figure class="lineup"><img src="img/{slug}/_lineup.webp" width="{f["lineup"][0]}" height="{f["lineup"][1]}" alt="Every {e(info["name"])} ship to one scale"><figcaption>All {len(f["ships"])} ships to one scale, painted as they'll fly</figcaption></figure>
  {"".join(sections)}
</section>''')
    nav = "".join(f'<a href="#{f["slug"]}" style="--faction:{f["info"]["accent"]}">{e(f["info"]["name"])} '
                  f'<span class="left" data-left="{f["slug"]}">{len(f["ships"])}</span></a>' for f in factions)
    nav = f'<nav class="factions" aria-label="Factions; the number is ships still without a verdict">{nav}</nav>'
    return (TEMPLATE.replace("{{BODY}}", "".join(body)).replace("{{TOTAL}}", str(total))
            .replace("{{NAV}}", nav if len(factions) > 1 else ""))


TEMPLATE = r'''<title>Afterlight Fleet Rework</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Chakra+Petch:wght@500;600;700&family=IBM+Plex+Sans:wght@400;500;600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
/* Layout: a long hangar manifest, one faction band at a time; each ship a bay with its two views side by side. */
:root {
  --bg: #0b0e13;
  --panel: #121720;
  --line: #222a36;
  --fg: #dfe5ee;
  --muted: #8b96a8;
  --accent: #e08a3c;
  --ok: #57c48a;
  --warn: #e0b34a;
  --bad: #e06464;
  --display: "Chakra Petch", "Avenir Next Condensed", "Arial Narrow", sans-serif;
  --body: "IBM Plex Sans", "Helvetica Neue", Arial, sans-serif;
  --mono: "IBM Plex Mono", ui-monospace, Menlo, monospace;
  color-scheme: dark;
}
* { box-sizing: border-box; }
body { background: var(--bg); color: var(--fg); font: 15px/1.55 var(--body); }
.wrap { max-width: 1240px; margin: 0 auto; padding-block: 40px 80px; padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); }
h1, h2, h3, h4 { font-family: var(--display); letter-spacing: 0.01em; text-wrap: balance; margin: 0; }
h1 { font-size: clamp(30px, 5vw, 46px); line-height: 1.05; }
.intro { display: grid; gap: 14px; max-width: 68ch; }
.intro p { margin: 0; color: var(--muted); }
.factions { position: sticky; top: 0; z-index: 5; display: flex; flex-wrap: wrap; gap: 6px; padding: 10px 0; margin: 18px 0 8px; background: var(--bg); border-bottom: 1px solid var(--line); }
.factions a { display: inline-flex; align-items: center; gap: 6px; padding: 5px 10px; border-radius: 999px; border: 1px solid var(--line); border-left: 3px solid var(--faction); color: var(--fg); text-decoration: none; font-size: 13px; white-space: nowrap; }
.factions a:hover, .factions a:focus-visible { border-color: var(--faction); }
.factions .left { font: 500 12px var(--mono); color: var(--muted); }
.factions .left.done { color: var(--ok); }
.ship.proposed-cut { border-color: var(--warn); }
.cutnote { margin: 10px 0 0; padding: 8px 12px; border-radius: 8px; border: 1px solid var(--warn); background: color-mix(in srgb, var(--warn) 10%, transparent); font-size: 14px; line-height: 1.45; }
.cutnote b { color: var(--warn); }
.cutnote span { display: block; margin-top: 2px; opacity: 0.75; font-size: 13px; }
.doctrine { margin-top: 14px; padding: 10px 14px; border-left: 3px solid var(--faction, var(--warn)); font-size: 15px; line-height: 1.5; }
.doctrine p { margin: 0 0 6px; }
.trim { margin-top: 14px; padding: 10px 14px; border-left: 3px solid var(--warn); font-size: 15px; line-height: 1.5; }
.reshaped { display: inline-block; margin-left: 8px; padding: 1px 7px; border-radius: 999px; background: var(--warn); color: #14110a; font: 600 11px var(--mono); letter-spacing: 0.04em; vertical-align: 1px; }
@media (max-width: 640px) { .factions { flex-wrap: nowrap; overflow-x: auto; } }
.eyebrow, .cls { font: 500 12px/1 var(--mono); letter-spacing: 0.12em; text-transform: uppercase; color: var(--accent); margin: 0; }
.tally { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 6px; }
.tally span { font: 500 13px/1 var(--mono); padding: 7px 10px; border: 1px solid var(--line); border-radius: 4px; color: var(--muted); }
.tally b { color: var(--fg); font-weight: 500; }
.faction { margin-top: 56px; display: grid; gap: 28px; }
.faction, .ship { scroll-margin-top: 64px; }
.faction-head { display: grid; gap: 10px; border-top: 2px solid var(--faction, var(--accent)); padding-top: 22px; }
.faction-head h2 { font-size: clamp(26px, 4vw, 36px); }
.tagline { margin: 0; max-width: 70ch; }
.grammar { margin: 4px 0 0; padding-left: 18px; color: var(--muted); max-width: 80ch; display: grid; gap: 4px; }
.lineup { margin: 0; background: var(--panel); border: 1px solid var(--line); border-radius: 6px; padding: 18px; overflow-x: auto; }
.lineup img { display: block; height: auto; }
figcaption { font: 12px/1.4 var(--mono); color: var(--muted); margin-top: 8px; }
h3.section { font-size: 20px; color: var(--muted); display: flex; gap: 10px; align-items: baseline; margin-top: 8px; }
h3.section span { font: 500 13px var(--mono); }
.ship { background: var(--panel); border: 1px solid var(--line); border-radius: 6px; padding: 20px; display: grid; gap: 16px; }
.ship-head { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: flex-end; gap: 12px; }
.ship-head h3 { font-size: 24px; }
.size { display: flex; gap: 18px; margin: 0; }
.size div { display: grid; gap: 2px; }
.size dt { font: 11px/1 var(--mono); letter-spacing: 0.1em; text-transform: uppercase; color: var(--muted); }
.size dd { margin: 0; font: 500 15px var(--mono); font-variant-numeric: tabular-nums; }
.note { margin: 0; max-width: 75ch; }
.pair { margin: 0; min-width: 0; display: grid; gap: 6px; }
.pair img { width: 100%; height: auto; max-height: 640px; object-fit: contain; background: #06080c; border-radius: 4px; padding: 10px; box-sizing: border-box; }
.facts { display: grid; grid-template-columns: 1.2fr 1fr; gap: 18px; align-items: start; }
.facts h4 { font-size: 13px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--muted); margin-bottom: 6px; }
.facts ul { list-style: none; margin: 0 0 10px; padding: 0; display: grid; gap: 3px; }
.facts p { margin: 0 0 10px; }
.facts .n { display: inline-block; min-width: 2.2ch; font: 500 14px var(--mono); color: var(--accent); font-variant-numeric: tabular-nums; }
.facts .sub { color: var(--muted); font-size: 13px; }
.facts .none { color: var(--muted); }
.concept { margin: 0; min-width: 0; }
.concept img { width: 100%; height: auto; max-height: 220px; object-fit: contain; background: #06080c; border-radius: 4px; }
.verdict { display: grid; grid-template-columns: auto 1fr; gap: 10px 16px; align-items: center; border-top: 1px solid var(--line); padding-top: 14px; }
.choices { display: flex; gap: 6px; flex-wrap: wrap; }
.choices button { font: 600 14px var(--display); letter-spacing: 0.03em; color: var(--fg); background: transparent; border: 1px solid var(--line); border-radius: 4px; padding: 8px 14px; cursor: pointer; }
.choices button:hover { border-color: var(--muted); }
.choices button:focus-visible, textarea:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.choices button[aria-pressed="true"][data-v="approve"] { background: var(--ok); border-color: var(--ok); color: #071a10; }
.choices button[aria-pressed="true"][data-v="rework"] { background: var(--warn); border-color: var(--warn); color: #1c1503; }
.choices button[aria-pressed="true"][data-v="reject"] { background: var(--bad); border-color: var(--bad); color: #1f0707; }
.choices button:disabled { opacity: 0.5; cursor: default; }
.note-field { display: flex; gap: 10px; align-items: center; min-width: 0; }
.note-field span { font: 11px var(--mono); letter-spacing: 0.1em; text-transform: uppercase; color: var(--muted); }
textarea { flex: 1; min-width: 0; font: 14px/1.4 var(--body); color: var(--fg); background: var(--bg); border: 1px solid var(--line); border-radius: 4px; padding: 7px 9px; resize: vertical; }
.saved { grid-column: 1 / -1; margin: 0; font: 12px var(--mono); color: var(--muted); min-height: 1em; }
.status { font: 12px var(--mono); color: var(--muted); }
@media (max-width: 760px) {
  .facts { grid-template-columns: 1fr; }
  .verdict { grid-template-columns: 1fr; }
}
@media (prefers-reduced-motion: reduce) { * { transition: none !important; } }
</style>
<div class="wrap">
  <header class="intro">
    <p class="eyebrow">Afterlight · fleet review</p>
    <h1>Fleet Rework</h1>
    <p>Every ship here is drawn by hand, then refit the way the game's current fleet was: doubled in size, armed with the turret, missile and point-defence kits, plated and painted. The pictures are the game's own renders of each design file. None of these ships is in the game yet.</p>
    <p>Ten ships marked <span class="reshaped">New shape</span> were redrawn so they stop reading as arrowheads: a skull, a mausoleum, a censer, a laurel wreath, an ensō, an alms bowl, a cracked bell, a crown of thorns, a survey frame and a peacock fan.</p>
    <p>Colour and detail pass: each faction now wears its own five-colour livery and motif, rooms are tinted by what they hold, and the plating carries panel seams, running lights and engine hatching. The Arbiter's stern blocks are now twin-drive engine nacelles.</p>
    <p>Trimmed: 60 near-repeats were taken out (each faction's header lists them), leaving 192 ships, 14 to 19 a faction. Every trader, station, flagship and new shape stayed.</p>
    <p>Each faction now fights its own way: its header says how, what it carries to do it, and how you beat it. The Terran, Kingdom, Compact and Merchant guns are new.</p>
    <p>Give each one a verdict. Rework or Reject with a note says what to change; your verdicts are saved here for the next round.</p>
    <div class="tally" aria-live="polite"><span><b id="t-total">{{TOTAL}}</b> ships</span><span><b id="t-approve">0</b> approved</span><span><b id="t-rework">0</b> to rework</span><span><b id="t-reject">0</b> rejected</span><span class="status" id="db-status">Loading verdicts…</span></div>
  </header>
  {{NAV}}
  {{BODY}}
</div>
<script>
(function () {
  const cards = Array.from(document.querySelectorAll(".verdict"));
  const state = {};
  const busy = {};
  const status = document.getElementById("db-status");
  let db = null;

  function tally() {
    const n = { approve: 0, rework: 0, reject: 0 };
    Object.values(state).forEach(function (v) { if (v && n[v.verdict] !== undefined) n[v.verdict]++; });
    ["approve", "rework", "reject"].forEach(function (k) { document.getElementById("t-" + k).textContent = n[k]; });
    document.querySelectorAll("[data-left]").forEach(function (el) {
      const slug = el.dataset.left;
      const left = cards.filter(function (c) { return c.dataset.doc.indexOf(slug + "__") === 0 && !(state[c.dataset.doc] || {}).verdict; }).length;
      el.textContent = left ? left + " left" : "done";
      el.classList.toggle("done", !left);
    });
  }

  function show(card) {
    const doc = card.dataset.doc;
    const v = state[doc] || {};
    card.querySelectorAll("button").forEach(function (b) { b.setAttribute("aria-pressed", String(v.verdict === b.dataset.v)); });
    const note = card.querySelector("textarea");
    if (document.activeElement !== note && (v.note || "") !== note.value) note.value = v.note || "";
    const saved = card.querySelector(".saved");
    saved.textContent = v.at ? "Saved " + new Date(v.at).toLocaleString() : "";
  }

  async function save(card, patch) {
    const doc = card.dataset.doc;
    if (!db) { card.querySelector(".saved").textContent = "Verdicts can't be saved in this view."; return; }
    if (busy[doc]) { busy[doc].next = Object.assign(busy[doc].next || {}, patch); return; }
    busy[doc] = {};
    const body = Object.assign({ verdict: "", note: "" }, state[doc] || {}, patch, { at: new Date().toISOString() });
    state[doc] = body;
    show(card); tally();
    try {
      await db.doc("verdicts/" + doc).set(body);
    } catch (e) {
      card.querySelector(".saved").textContent = "Not saved (" + (e && e.code || "error") + "). Try again.";
    }
    const next = busy[doc].next;
    delete busy[doc];
    if (next) save(card, next);
  }

  cards.forEach(function (card) {
    card.querySelectorAll("button").forEach(function (b) {
      b.addEventListener("click", function () {
        const cur = (state[card.dataset.doc] || {}).verdict;
        save(card, { verdict: cur === b.dataset.v ? "" : b.dataset.v, note: card.querySelector("textarea").value });
      });
    });
    const note = card.querySelector("textarea");
    note.addEventListener("change", function () { save(card, { note: note.value }); });
  });

  const use = window.claude && window.claude.use;
  if (!use) { status.textContent = "Open this page in Claude to save verdicts."; cards.forEach(function (c) { c.querySelectorAll("button").forEach(function (b) { b.disabled = true; }); }); return; }
  window.claude.use("db").then(function (d) {
    db = d;
    if (!db) { status.textContent = "Verdicts can't be loaded in this view."; return; }
    const timer = setTimeout(function () { status.textContent = "Verdicts can't be reached yet."; }, 10000);
    db.collection("verdicts").onSnapshot(function (snap) {
      snap.docs.forEach(function (d) { state[d.id] = d.data(); });
      cards.forEach(show);
      tally();
      if (!snap.metadata.fromCache) { clearTimeout(timer); status.textContent = "Verdicts saved here"; }
    }, function (e) {
      clearTimeout(timer);
      status.textContent = "Verdicts unavailable (" + (e && e.code || "error") + ")";
    });
  });
})();
</script>
'''


def main():
    slugs = sys.argv[1:] or list(MODULES)
    os.makedirs(SITE, exist_ok=True)
    order = []
    factions = [f for f in (faction(s, order) for s in slugs) if f]
    open(os.path.join(SITE, "index.html"), "w").write(page(factions))
    print(f"site: {len(order)} ships in {len(factions)} factions -> {SITE}/index.html")


if __name__ == "__main__":
    main()
