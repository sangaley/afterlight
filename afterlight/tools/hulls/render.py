from PIL import Image
import json, os
exec(open('/tmp/ships/fit.py').read())

GAME = "/Users/shhh/afterlight-cascade/afterlight/"
HULLDIR = GAME + "assets/sprites/hull/"
OUT = "/Users/shhh/afterlight/.artifact-tmp/hulls/"
SPR = json.load(open('/tmp/ships/sprites.json'))
SPR.setdefault('RadarArray', 'sprites/modules/radar.png')
SPR.setdefault('Floodlight', 'sprites/modules/floodlight.png')

T = 26
cache = {}
def load(path, w, h):
    k = (path, w, h)
    if k not in cache:
        p = GAME + "assets/" + path
        if not os.path.exists(p):
            cache[k] = None
        else:
            cache[k] = Image.open(p).convert("RGBA").resize((w, h), Image.LANCZOS)
    return cache[k]

def hull_tile(name, mask):
    k = (name, mask)
    if k not in cache:
        cache[k] = Image.open(f"{HULLDIR}{name}_{mask:02d}.png").convert("RGBA").resize((T, T), Image.LANCZOS)
    return cache[k]

def render(cells, mods):
    taken = occupied(cells, mods)
    inner = interior(cells)
    xs=[c[0] for c in cells]; ys=[c[1] for c in cells]
    x0,x1,y0,y1 = min(xs),max(xs),min(ys),max(ys)
    img = Image.new("RGBA", ((x1-x0+1)*T, (y1-y0+1)*T), (0,0,0,0))

    def kind(c):
        if c not in cells: return None
        if c not in inner: return "hull_steel"
        return "hull_steel" if c in taken else "hull_hallway"

    for c in sorted(cells):
        name = kind(c)
        mask = 0
        for bit, d in ((1,(0,1)),(2,(0,-1)),(4,(-1,0)),(8,(1,0))):
            if kind((c[0]+d[0], c[1]+d[1])) == name:
                mask |= bit
        img.paste(hull_tile(name, mask), ((c[0]-x0)*T, (y1-c[1])*T), hull_tile(name, mask))

    for m in sorted(mods, key=lambda m: (m['y'], m['x'])):
        d = MODS[m['t']]
        path = SPR.get(m['t'])
        if not path: continue
        s = load(path, d['w']*T, d['h']*T)
        if s is None: continue
        img.paste(s, ((m['x']-x0)*T, (y1-(m['y']+d['h']-1))*T), s)
    return img

os.makedirs(OUT, exist_ok=True)
meta = []
BLURB = {
 'pincer': ("Pincer", "Two long arms around a deep open notch. Anything that comes at the bow is between your guns before it touches plating. Longest reach of the three, and the thinnest arms."),
 'trident': ("Trident", "Two arms and a spine that runs the full length between them. The spine is structure and gun mount both, and it is what stops the middle from being a hole -- your point exactly."),
 'crab': ("Crab", "Short heavy claws on a broad body. The most interior space of the three by some way, and the most deck to walk."),
}
for key, s in SHIPS.items():
    im = render(s['cells'], s['mods'])
    im.save(OUT + key + ".png", optimize=True)
    from collections import Counter
    cnt = Counter(m['t'] for m in s['mods'])
    xs=[c[0] for c in s['cells']]; ys=[c[1] for c in s['cells']]
    title, blurb = BLURB[key]
    meta.append(dict(
        key=key, title=title, blurb=blurb,
        cells=len(s['cells']), mods=len(s['mods']),
        deck=len(walkable(s['cells'], s['mods'])),
        w=max(xs)-min(xs)+1, h=max(ys)-min(ys)+1,
        cost=sum(MODS[m['t']]['cost'] for m in s['mods']),
        loadout=[[k, v] for k, v in sorted(cnt.items())],
    ))
    print("  %-8s %s" % (key, im.size))
open(OUT + "../hulls.js", "w").write("window.HULLS=" + json.dumps(meta) + ";\n")
print("done")
