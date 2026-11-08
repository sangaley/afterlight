"""Every concept of a faction side by side at one scale, with the Pincer: python3 lineup.py terran_hegemony [px]"""
import glob, json, os, sys
from PIL import Image, ImageDraw
import fleet

def lineup(slug, px=7, order=None, out=None, width=1700):
    here = os.path.dirname(os.path.abspath(__file__))
    files = order or sorted(glob.glob(os.path.join(here, "concepts", slug, "*.json")))
    items = [("Pincer (you)", fleet.load(os.path.join(fleet.ROOT, "designs", "starter.json")))]
    items += [(os.path.basename(f)[:-5], fleet.load(f)) for f in files]
    imgs = [(n, fleet.render(d, px=px, pad=1)) for n, d in items]
    gap, label = 18, 16
    rows, row, w = [], [], 0
    for n, im in imgs:
        if row and w + im.width + gap > width:
            rows.append(row); row, w = [], 0
        row.append((n, im)); w += im.width + gap
    rows.append(row)
    H = sum(max(im.height for _, im in r) + label + gap for r in rows) + gap
    sheet = Image.new("RGBA", (width, H), (12, 16, 25, 255))
    d = ImageDraw.Draw(sheet)
    y = gap
    for r in rows:
        h = max(im.height for _, im in r)
        x = gap
        for n, im in r:
            sheet.alpha_composite(im, (x, y + h - im.height))
            d.text((x, y + h + 3), n, fill=(200, 210, 225, 255))
            x += im.width + gap
        y += h + label + gap
    sheet.save(out or os.path.join(here, "concepts", slug, "_lineup.png"))
    return sheet

if __name__ == "__main__":
    lineup(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 7)
