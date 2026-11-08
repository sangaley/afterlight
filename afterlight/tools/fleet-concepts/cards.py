"""Card images: each concept beside the Pincer at one shared scale."""
import os
from PIL import Image, ImageDraw
import fleet

PX = 16
BG = (12, 16, 25, 255)
_pincer = None

def pincer():
    global _pincer
    if _pincer is None:
        _pincer = fleet.render(fleet.load(os.path.join(fleet.ROOT, "designs", "starter.json")), px=PX, pad=1)
    return _pincer

def card(design, path):
    ship = fleet.render(design, px=PX, pad=1)
    p = pincer()
    gap, lab = 40, 22
    if ship.width + gap + p.width <= 1500:
        W = ship.width + gap + p.width + 2 * gap
        H = max(ship.height, p.height) + lab + 2 * gap
        sx, sy = gap, gap + (H - 2 * gap - lab - ship.height) // 2
        px_, py_ = gap + ship.width + gap, H - gap - lab - p.height
    else:
        W = max(ship.width, p.width) + 2 * gap
        H = ship.height + p.height + gap + lab + 2 * gap
        sx, sy = (W - ship.width) // 2, gap
        px_, py_ = gap, gap + ship.height + gap
    im = Image.new("RGBA", (W, H), BG)
    im.alpha_composite(ship, (sx, sy))
    pc = p.copy()
    pc.putalpha(pc.getchannel("A").point(lambda a: int(a * 0.55)))
    im.alpha_composite(pc, (px_, py_))
    d = ImageDraw.Draw(im)
    d.text((px_, py_ + p.height + 4), "your Pincer, same scale", fill=(140, 152, 170, 255))
    im.convert("RGB").save(path, optimize=True)
    return im.size
