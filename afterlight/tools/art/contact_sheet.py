import glob, os
from PIL import Image, ImageDraw, ImageFont

FONT = "/System/Library/Fonts/Supplemental/Arial.ttf"
try:
    f = ImageFont.truetype(FONT, 15)
except Exception:
    f = ImageFont.load_default()

files = sorted(glob.glob("assets/sprites/modules/*.png"))
CELL, PAD, LBL, COLS = 108, 12, 20, 8
rows = (len(files) + COLS - 1) // COLS
W = COLS * (CELL + PAD) + PAD
H = rows * (CELL + PAD + LBL) + PAD + 34
sheet = Image.new("RGBA", (W, H), (18, 20, 26, 255))
d = ImageDraw.Draw(sheet)
d.text((PAD, 10), "assets/sprites/modules/  -  %d sprites" % len(files), font=f, fill=(150, 160, 175))

for i, p in enumerate(files):
    cx = PAD + (i % COLS) * (CELL + PAD)
    cy = 34 + PAD + (i // COLS) * (CELL + PAD + LBL)
    im = Image.open(p).convert("RGBA")
    im.thumbnail((CELL, CELL), Image.LANCZOS)
    d.rectangle([cx-1, cy-1, cx+CELL, cy+CELL], outline=(44, 50, 60))
    sheet.paste(im, (cx + (CELL-im.width)//2, cy + (CELL-im.height)//2), im)
    name = os.path.basename(p)[:-4]
    if d.textlength(name, font=f) > CELL:
        while d.textlength(name + "..", font=f) > CELL and len(name) > 4:
            name = name[:-1]
        name += ".."
    d.text((cx, cy + CELL + 3), name, font=f, fill=(160, 170, 185))

sheet.save("tools/art/modules_sheet.png")
print("modules:", len(files))
