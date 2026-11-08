"""Copy every class on the game's rosters (src/ai_ship/roster.rs) from the
approved concepts into designs/factions/<slug>/<key>.json, where the game
loads them. Run after a concept is approved and added to a roster."""
import json, os, re, shutil, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
SLUGS = {
    "TERRAN": "terran_hegemony", "ETERNAL": "eternal_hegemony", "GILDED": "gilded_throne",
    "CHOIR": "broken_choir", "SYNTHESIS": "synthesis_collective", "CORPSE": "corpse_stars",
    "PRESERVE": "stellar_preserve", "SILENCE": "the_silence", "SHEPHERD": "shepherd",
    "RECURSIVE": "recursive_kingdom", "MERCHANTS": "merchant_princes",
    "COMPACT": "free_haulers_compact",
}
src = open(os.path.join(ROOT, "src/ai_ship/roster.rs")).read()
copied = 0
for const, body in re.findall(r"const (\w+): &\[ShipClass\] = &\[(.*?)\];", src, re.S):
    slug = SLUGS[const]
    for key in re.findall(r'class\("(\w+)"', body):
        a = os.path.join(HERE, "concepts", slug, f"{key}.json")
        b = os.path.join(ROOT, "designs", "factions", slug, f"{key}.json")
        if not os.path.exists(a):
            sys.exit(f"missing concept {a}")
        os.makedirs(os.path.dirname(b), exist_ok=True)
        d = json.load(open(a))
        d.pop("_checks", None)
        json.dump(d, open(b, "w"), indent=1)
        copied += 1
# Stations and their gun platforms (src/diplomacy/stations.rs PLANS).
ENUM = {"TerranHegemony": "terran_hegemony", "EternalHegemony": "eternal_hegemony",
        "GildedThrone": "gilded_throne", "BrokenChoir": "broken_choir",
        "SynthesisCollective": "synthesis_collective", "CorpseStars": "corpse_stars",
        "StellarPreserve": "stellar_preserve", "TheSilence": "the_silence", "Shepherd": "shepherd",
        "RecursiveKingdom": "recursive_kingdom", "MerchantPrinces": "merchant_princes",
        "FreeHaulersCompact": "free_haulers_compact"}
stations = open(os.path.join(ROOT, "src/diplomacy/stations.rs")).read()
for enum, key, platform in re.findall(r'\(AiShipType::(\w+), plan\("(\w+)", "[^"]*", (?:Some\("(\w+)"\)|None)', stations):
    for k in filter(None, (key, platform)):
        slug = ENUM[enum]
        a = os.path.join(HERE, "concepts", slug, f"{k}.json")
        b = os.path.join(ROOT, "designs", "factions", slug, f"{k}.json")
        if not os.path.exists(a):
            sys.exit(f"missing concept {a}")
        d = json.load(open(a)); d.pop("_checks", None)
        json.dump(d, open(b, "w"), indent=1)
        copied += 1
print(f"exported {copied} designs")
