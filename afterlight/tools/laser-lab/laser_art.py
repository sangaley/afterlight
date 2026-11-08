"""Placeholder art for the laser parts, drawn on the gun parts' panel so they
sit beside them. Writes assets/sprites/laser_parts/*.png (128 px).

Run from afterlight/: python3 tools/laser-lab/laser_art.py
"""
import os
from PIL import Image, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "assets/sprites/laser_parts")
os.makedirs(OUT, exist_ok=True)
base = Image.open(os.path.join(ROOT, "assets/sprites/gun_parts/turret_plating.png")).convert("RGBA").resize((128, 128))


def tile():
    im = base.copy()
    ImageDraw.Draw(im).rectangle([10, 10, 117, 117], fill=(30, 35, 46, 255), outline=(70, 80, 98, 255), width=3)
    return im


def lit_box(d, box, fill, lit=(150, 165, 185), dark=(18, 20, 26)):
    x0, y0, x1, y1 = box
    d.rectangle(box, fill=fill)
    d.line([(x0, y1), (x0, y0), (x1, y0)], fill=lit, width=3)
    d.line([(x1, y0), (x1, y1), (x0, y1)], fill=dark, width=3)


def save(name, im):
    im.save(os.path.join(OUT, name + ".png"))


# Core: a bright crystal in a heavy collar, conduits out to all four sides.
im = tile(); d = ImageDraw.Draw(im)
for box in [(56, 10, 72, 40), (56, 88, 72, 118), (10, 56, 40, 72), (88, 56, 118, 72)]:
    d.rectangle(box, fill=(90, 100, 120))
lit_box(d, (34, 34, 94, 94), (60, 68, 84))
d.polygon([(64, 42), (86, 64), (64, 86), (42, 64)], fill=(235, 240, 255))
d.polygon([(64, 50), (78, 64), (64, 78), (50, 64)], fill=(255, 120, 110))
save("core", im)

# Coupler: a conduit cross.
im = tile(); d = ImageDraw.Draw(im)
d.rectangle([10, 52, 118, 76], fill=(110, 120, 140)); d.rectangle([52, 10, 76, 118], fill=(110, 120, 140))
d.rectangle([10, 60, 118, 68], fill=(255, 110, 100)); d.rectangle([60, 10, 68, 118], fill=(255, 110, 100))
save("coupler", im)

# Cavity: a long glass tube with mirrors at the ends.
im = tile(); d = ImageDraw.Draw(im)
lit_box(d, (18, 40, 110, 88), (40, 60, 110))
d.rectangle([26, 52, 102, 76], fill=(110, 160, 255)); d.rectangle([26, 60, 102, 66], fill=(210, 230, 255))
d.rectangle([18, 40, 26, 88], fill=(200, 205, 215)); d.rectangle([102, 40, 110, 88], fill=(200, 205, 215))
save("cavity", im)

# Pump: flash lamps in a reflector.
im = tile(); d = ImageDraw.Draw(im)
lit_box(d, (22, 22, 106, 106), (44, 72, 64))
for y in (36, 58, 80):
    d.rectangle([32, y, 96, y + 10], fill=(140, 255, 190)); d.rectangle([32, y + 3, 96, y + 6], fill=(240, 255, 245))
save("pump", im)

# Q-switch: a shutter of amber blades.
im = tile(); d = ImageDraw.Draw(im)
lit_box(d, (24, 24, 104, 104), (70, 56, 30))
for i in range(4):
    d.polygon([(64, 64), (40 + i * 16, 30), (52 + i * 16, 30)], fill=(240, 170, 70))
d.rectangle([58, 58, 70, 70], fill=(30, 20, 10))
save("qswitch", im)

# Frequency doubler: a violet crystal block.
im = tile(); d = ImageDraw.Draw(im)
lit_box(d, (28, 28, 100, 100), (58, 44, 90))
d.polygon([(40, 88), (64, 36), (88, 88)], fill=(185, 140, 255)); d.polygon([(52, 84), (64, 56), (76, 84)], fill=(235, 215, 255))
save("doubler", im)

# Destabiliser: a coil with warning stripes.
im = tile(); d = ImageDraw.Draw(im)
lit_box(d, (22, 22, 106, 106), (70, 40, 30))
for x in range(30, 100, 12):
    d.rectangle([x, 40, x + 6, 88], fill=(255, 130, 90))
d.rectangle([22, 96, 106, 106], fill=(200, 160, 40))
save("destabiliser", im)

# Emitter: the aperture the beam leaves through, facing up (north).
im = tile(); d = ImageDraw.Draw(im)
lit_box(d, (26, 30, 102, 112), (64, 70, 86))
d.rectangle([40, 10, 88, 40], fill=(90, 100, 120)); d.rectangle([50, 10, 78, 30], fill=(20, 22, 28))
d.rectangle([56, 14, 72, 26], fill=(255, 110, 100))
save("emitter", im)

# Lens: a glass element in a ring mount (beam axis north-south).
im = tile(); d = ImageDraw.Draw(im)
d.rectangle([40, 10, 88, 118], fill=(70, 80, 98))
d.rectangle([30, 50, 98, 78], fill=(110, 120, 140))
d.rectangle([44, 54, 84, 74], fill=(140, 190, 255)); d.rectangle([48, 58, 80, 62], fill=(230, 245, 255))
save("lens", im)

# Beam relay: a prism tower beside a ring.
im = tile(); d = ImageDraw.Draw(im)
lit_box(d, (30, 30, 98, 98), (60, 66, 82))
d.polygon([(46, 86), (82, 86), (64, 42)], fill=(255, 140, 120)); d.polygon([(56, 80), (72, 80), (64, 60)], fill=(255, 220, 210))
save("relay", im)
print("laser art written")
