#!/usr/bin/env python3
"""
Afterlight - Creature & Environment & VFX Sprite Generator
Style: Dark & Gritty, Industrial, Barotrauma-inspired
"""
from PIL import Image, ImageDraw
import random
import math
import os

OUT = "/Users/fredericmoungpacrijanot/afterlight/assets/sprites"
random.seed(42)

# Dark ocean palette
DARK_BG = (0, 0, 0, 0)
DEEP_BLUE = (15, 25, 50)
OCEAN_DARK = (10, 20, 40)

# Creature palettes
FLESH_DARK = (60, 45, 40)
FLESH_MID = (85, 65, 55)
FLESH_LIGHT = (110, 85, 70)
BONE_WHITE = (180, 175, 160)
EYE_RED = (200, 40, 30)
EYE_GLOW = (220, 60, 40)
BIOLUM_CYAN = (40, 200, 180)
BIOLUM_BLUE = (30, 120, 200)
BIOLUM_GREEN = (40, 180, 80)
BIOLUM_PURPLE = (120, 50, 180)
TOOTH_WHITE = (200, 195, 180)
SCALE_DARK = (40, 50, 55)
SCALE_MID = (55, 68, 72)
SCALE_LIGHT = (75, 90, 95)
BLOOD_RED = (140, 25, 20)
TENTACLE_DARK = (50, 35, 60)
TENTACLE_MID = (75, 55, 85)
JELLY_BLUE = (60, 120, 200, 160)
JELLY_PURPLE = (100, 60, 180, 140)
ELECTRIC_YELLOW = (220, 200, 50)
ELECTRIC_BLUE = (60, 160, 255)

def new_sprite(size=64):
    return Image.new("RGBA", (size, size), DARK_BG)

def draw_eye(d, cx, cy, r=3, color=EYE_RED, glow=EYE_GLOW):
    d.ellipse([cx-r-1, cy-r-1, cx+r+1, cy+r+1], fill=glow)
    d.ellipse([cx-r, cy-r, cx+r, cy+r], fill=color)
    d.ellipse([cx-1, cy-1, cx+1, cy+1], fill=(255, 255, 255))

def draw_teeth(d, x, y, w, top=True):
    for tx in range(x, x+w, 4):
        if top:
            d.polygon([(tx, y), (tx+2, y+4), (tx+4, y)], fill=TOOTH_WHITE)
        else:
            d.polygon([(tx, y), (tx+2, y-4), (tx+4, y)], fill=TOOTH_WHITE)

# === HOSTILE CREATURES ===

def gen_scavenger():
    """Scavenger - small bottom-feeder, crab-like."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Body
    d.ellipse([16, 24, 48, 44], fill=SCALE_MID, outline=SCALE_DARK)
    d.ellipse([20, 26, 44, 42], fill=SCALE_LIGHT)
    # Shell ridges
    for rx in range(22, 42, 4):
        d.line([(rx, 26), (rx, 42)], fill=SCALE_DARK)
    # Legs (6)
    for lx, side in [(14, -1), (20, -1), (26, -1), (36, 1), (42, 1), (48, 1)]:
        d.line([(lx, 38), (lx + side*6, 52)], fill=SCALE_DARK, width=2)
        d.line([(lx + side*6, 52), (lx + side*10, 56)], fill=SCALE_DARK, width=2)
    # Claws
    d.polygon([(10, 20), (4, 14), (8, 12), (16, 22)], fill=SCALE_MID)
    d.polygon([(54, 20), (60, 14), (56, 12), (48, 22)], fill=SCALE_MID)
    # Eyes
    draw_eye(d, 24, 26, 2, (180, 160, 40))
    draw_eye(d, 40, 26, 2, (180, 160, 40))
    return img

def gen_stalker():
    """Stalker - sleek predator fish, dark, bioluminescent stripe."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Streamlined body
    body = [(8, 32), (20, 20), (44, 18), (56, 28), (58, 32),
            (56, 36), (44, 46), (20, 44), (8, 32)]
    d.polygon(body, fill=SCALE_DARK)
    # Lighter belly
    d.polygon([(12, 33), (20, 38), (44, 40), (54, 34),
               (54, 36), (44, 44), (20, 42), (12, 35)], fill=SCALE_MID)
    # Bioluminescent stripe
    for sx in range(16, 52, 2):
        y = 30 + int(2 * math.sin(sx * 0.3))
        d.rectangle([sx, y, sx+1, y+1], fill=BIOLUM_CYAN)
    # Dorsal fin
    d.polygon([(28, 20), (32, 10), (40, 18)], fill=SCALE_DARK)
    # Tail fin
    d.polygon([(6, 24), (2, 16), (10, 28)], fill=SCALE_DARK)
    d.polygon([(6, 40), (2, 48), (10, 36)], fill=SCALE_DARK)
    # Eye
    draw_eye(d, 48, 28, 3)
    # Teeth
    draw_teeth(d, 52, 32, 8)
    return img

def gen_ambusher():
    """Ambusher - flat, camouflaged, wide mouth."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Flat wide body
    d.ellipse([6, 22, 58, 50], fill=(55, 60, 50))
    d.ellipse([8, 24, 56, 48], fill=(65, 70, 58))
    # Camouflage spots
    for _ in range(12):
        sx = random.randint(12, 52)
        sy = random.randint(26, 46)
        sr = random.randint(2, 5)
        d.ellipse([sx-sr, sy-sr, sx+sr, sy+sr], fill=(50, 55, 45))
    # Huge mouth
    d.arc([20, 28, 58, 48], 0, 180, fill=BLOOD_RED, width=2)
    draw_teeth(d, 22, 38, 32)
    draw_teeth(d, 22, 38, 32, top=False)
    # Small eyes on top
    draw_eye(d, 26, 24, 2, (200, 180, 40))
    draw_eye(d, 42, 24, 2, (200, 180, 40))
    # Tail
    d.polygon([(4, 30), (0, 24), (0, 42), (4, 38)], fill=(55, 60, 50))
    return img

def gen_electric_eel():
    """Electric Eel - long sinuous body, electric sparks."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Sinuous body
    points = []
    for i in range(32):
        x = 4 + i * 1.8
        y = 32 + 10 * math.sin(i * 0.4)
        points.append((x, y))
    # Draw thick body
    for i in range(len(points)-1):
        x1, y1 = points[i]
        x2, y2 = points[i+1]
        width = 6 - abs(i - 16) * 0.3
        width = max(2, width)
        d.line([(x1, y1), (x2, y2)], fill=(60, 65, 45), width=int(width))
    # Yellow belly stripe
    for i in range(len(points)-1):
        x1, y1 = points[i]
        x2, y2 = points[i+1]
        d.line([(x1, y1+2), (x2, y2+2)], fill=ELECTRIC_YELLOW, width=1)
    # Electric sparks
    for _ in range(8):
        sx = random.randint(10, 54)
        sy = random.randint(16, 48)
        for _ in range(3):
            ex = sx + random.randint(-6, 6)
            ey = sy + random.randint(-6, 6)
            d.line([(sx, sy), (ex, ey)], fill=ELECTRIC_BLUE, width=1)
            sx, sy = ex, ey
    # Head
    d.ellipse([52, 26, 62, 38], fill=(65, 70, 48))
    draw_eye(d, 58, 30, 2, ELECTRIC_YELLOW)
    return img

def gen_blind_hunter():
    """Blind Hunter - no eyes, huge mouth, echolocation bumps."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Bulky body
    d.ellipse([10, 16, 54, 52], fill=FLESH_DARK)
    d.ellipse([14, 20, 50, 48], fill=FLESH_MID)
    # No eyes - scarred flesh where eyes would be
    d.line([(36, 24), (42, 20)], fill=FLESH_DARK, width=2)
    d.line([(36, 24), (42, 28)], fill=FLESH_DARK, width=2)
    # Echolocation bumps on head
    for bx in [38, 44, 50]:
        d.ellipse([bx, 18, bx+4, 22], fill=FLESH_LIGHT)
    # Massive gaping mouth
    d.arc([20, 26, 58, 50], 330, 210, fill=BLOOD_RED, width=3)
    d.ellipse([30, 30, 56, 48], fill=(40, 15, 15))
    # Rows of teeth
    for tx in range(32, 54, 3):
        d.line([(tx, 32), (tx, 36)], fill=TOOTH_WHITE, width=1)
        d.line([(tx, 46), (tx, 42)], fill=TOOTH_WHITE, width=1)
    # Tail
    d.polygon([(8, 28), (2, 20), (2, 44), (8, 36)], fill=FLESH_DARK)
    return img

def gen_lure_fish():
    """Lure Fish - anglerfish with bioluminescent lure."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Round body
    d.ellipse([14, 20, 50, 52], fill=SCALE_DARK)
    d.ellipse([18, 24, 46, 48], fill=(45, 40, 50))
    # Lure stalk
    d.line([(38, 20), (42, 8), (48, 4)], fill=(60, 55, 65), width=2)
    # Lure glow
    d.ellipse([44, 0, 54, 10], fill=BIOLUM_CYAN)
    d.ellipse([46, 2, 52, 8], fill=(100, 240, 220))
    d.ellipse([47, 3, 51, 7], fill=(200, 255, 240))
    # Huge mouth
    d.arc([16, 32, 52, 56], 0, 180, fill=BLOOD_RED, width=2)
    draw_teeth(d, 18, 44, 30)
    draw_teeth(d, 18, 44, 30, top=False)
    # Eye
    draw_eye(d, 40, 28, 4)
    # Small fins
    d.polygon([(14, 34), (6, 28), (8, 36)], fill=SCALE_DARK)
    d.polygon([(14, 42), (6, 48), (8, 40)], fill=SCALE_DARK)
    return img

def gen_swarm_queen():
    """Swarm Queen - large, spawns drones, many eyes."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Large bulbous body
    d.ellipse([8, 8, 56, 56], fill=TENTACLE_DARK)
    d.ellipse([12, 12, 52, 52], fill=TENTACLE_MID)
    # Multiple eyes (cluster)
    for ex, ey in [(28, 20), (36, 18), (24, 28), (40, 26),
                   (32, 24), (30, 32), (38, 34)]:
        draw_eye(d, ex, ey, 2, BIOLUM_GREEN, (60, 200, 100))
    # Egg sacs
    for sx, sy in [(16, 40), (24, 46), (36, 48), (46, 42)]:
        d.ellipse([sx-3, sy-3, sx+3, sy+3], fill=(80, 100, 70))
        d.ellipse([sx-1, sy-1, sx+1, sy+1], fill=(120, 160, 100))
    # Tentacles
    for angle in range(0, 360, 45):
        a = math.radians(angle)
        sx = 32 + int(22 * math.cos(a))
        sy = 32 + int(22 * math.sin(a))
        ex = 32 + int(30 * math.cos(a + 0.2))
        ey = 32 + int(30 * math.sin(a + 0.2))
        d.line([(sx, sy), (ex, ey)], fill=TENTACLE_DARK, width=2)
    return img

def gen_leviathan():
    """Leviathan - massive boss, armored, glowing markings."""
    img = Image.new("RGBA", (128, 64), DARK_BG)  # 2x1 for boss size
    d = ImageDraw.Draw(img)
    # Massive armored body
    d.ellipse([20, 4, 110, 60], fill=(35, 40, 50))
    d.ellipse([24, 8, 106, 56], fill=(45, 52, 62))
    # Armor plates
    for px in range(30, 100, 12):
        d.rectangle([px, 10, px+10, 54], fill=(40, 46, 55), outline=(55, 62, 72))
    # Bioluminescent markings
    for px in range(28, 100, 8):
        y = 32 + int(8 * math.sin(px * 0.15))
        d.rectangle([px, y, px+4, y+2], fill=BIOLUM_CYAN)
    # Head
    d.ellipse([96, 12, 126, 52], fill=(40, 46, 55))
    # Massive jaw
    d.arc([100, 24, 128, 50], 310, 50, fill=BLOOD_RED, width=3)
    draw_teeth(d, 108, 36, 18)
    # Eyes (large, menacing)
    draw_eye(d, 112, 22, 5, EYE_RED, (240, 80, 60))
    # Dorsal spines
    for sx in range(36, 96, 10):
        d.polygon([(sx, 8), (sx+3, 0), (sx+6, 8)], fill=(50, 56, 65))
    # Tail
    d.polygon([(20, 20), (4, 4), (2, 32), (4, 60), (20, 44)], fill=(40, 46, 55))
    return img

def gen_parasite():
    """Parasite - small, fast, attaches to hull."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Small segmented body
    for i, (sx, sy) in enumerate([(32, 28), (28, 32), (24, 36), (20, 38)]):
        size = 8 - i
        d.ellipse([sx-size, sy-size, sx+size, sy+size], fill=FLESH_DARK if i % 2 == 0 else FLESH_MID)
    # Head
    d.ellipse([34, 22, 48, 36], fill=FLESH_MID)
    # Mandibles
    d.polygon([(48, 26), (56, 22), (52, 28)], fill=BLOOD_RED)
    d.polygon([(48, 32), (56, 36), (52, 30)], fill=BLOOD_RED)
    # Hook legs
    for lx, ly in [(36, 36), (30, 40), (24, 42)]:
        d.line([(lx, ly), (lx+4, ly+8)], fill=FLESH_DARK, width=1)
        d.line([(lx, ly-8), (lx+4, ly-16)], fill=FLESH_DARK, width=1)
    # Eyes
    draw_eye(d, 42, 26, 2, (200, 200, 40))
    draw_eye(d, 42, 32, 2, (200, 200, 40))
    return img

def gen_watcher():
    """Watcher - passive until provoked, huge single eye."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Jellyfish-like dome
    d.ellipse([10, 6, 54, 42], fill=(40, 45, 65))
    d.ellipse([14, 10, 50, 38], fill=(50, 58, 78))
    # Single massive eye
    d.ellipse([20, 14, 44, 34], fill=(20, 20, 30))
    d.ellipse([24, 18, 40, 30], fill=(30, 80, 120))
    d.ellipse([28, 20, 36, 28], fill=(50, 150, 200))
    d.ellipse([30, 22, 34, 26], fill=(200, 200, 220))
    # Trailing tendrils
    for tx in range(16, 50, 6):
        length = random.randint(14, 24)
        for ty in range(40, 40 + length, 2):
            wobble = int(2 * math.sin(ty * 0.5 + tx))
            d.rectangle([tx + wobble, ty, tx + wobble + 1, ty + 1], fill=(45, 50, 70))
    return img

# === AMBIENT CREATURES ===

def gen_small_fish():
    """Small generic fish."""
    img = new_sprite(32)
    d = ImageDraw.Draw(img)
    d.polygon([(4, 16), (10, 10), (22, 10), (26, 14), (26, 18),
               (22, 22), (10, 22), (4, 16)], fill=(80, 100, 120))
    d.polygon([(2, 12), (0, 8), (6, 14)], fill=(70, 90, 110))
    d.polygon([(2, 20), (0, 24), (6, 18)], fill=(70, 90, 110))
    d.ellipse([22, 14, 25, 17], fill=(40, 40, 50))
    d.point((23, 15), fill=(200, 200, 200))
    return img

def gen_jellyfish():
    """Bioluminescent jellyfish."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Bell
    d.ellipse([16, 8, 48, 32], fill=(60, 80, 160, 140))
    d.ellipse([20, 12, 44, 28], fill=(80, 120, 200, 120))
    d.ellipse([26, 16, 38, 24], fill=(120, 180, 240, 100))
    # Oral arms
    for tx in [22, 28, 34, 40]:
        for ty in range(32, 58, 2):
            wobble = int(3 * math.sin(ty * 0.4 + tx * 0.3))
            alpha = max(40, 160 - (ty - 32) * 5)
            d.rectangle([tx + wobble, ty, tx + wobble + 1, ty + 1],
                       fill=(80, 100, 180, alpha))
    # Glow
    d.ellipse([28, 18, 36, 24], fill=(150, 200, 255, 80))
    return img

def gen_school_fish():
    """Small school of fish (group sprite)."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    fish_color = (70, 100, 130)
    fish_light = (90, 120, 150)
    positions = [(10, 14), (24, 10), (40, 16), (14, 30), (30, 28),
                 (46, 32), (20, 44), (36, 46), (50, 42)]
    for fx, fy in positions:
        d.polygon([(fx, fy), (fx+4, fy-3), (fx+10, fy-2), (fx+12, fy),
                   (fx+10, fy+2), (fx+4, fy+3), (fx, fy)], fill=fish_color)
        d.polygon([(fx-2, fy-2), (fx-4, fy-4), (fx, fy)], fill=fish_light)
        d.polygon([(fx-2, fy+2), (fx-4, fy+4), (fx, fy)], fill=fish_light)
        d.point((fx+10, fy), fill=(30, 30, 40))
    return img

def gen_deep_fish():
    """Deep sea fish - dark, small bioluminescent spots."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    d.polygon([(8, 32), (16, 20), (48, 18), (56, 28), (56, 36),
               (48, 46), (16, 44), (8, 32)], fill=(30, 35, 45))
    d.polygon([(12, 33), (16, 38), (48, 40), (52, 34)], fill=(35, 40, 50))
    # Bioluminescent spots
    for _ in range(6):
        bx = random.randint(18, 48)
        by = random.randint(22, 42)
        d.ellipse([bx-1, by-1, bx+1, by+1], fill=BIOLUM_CYAN)
    # Large eye
    draw_eye(d, 50, 28, 3, (30, 150, 180), BIOLUM_CYAN)
    # Fins
    d.polygon([(6, 26), (2, 18), (2, 46), (6, 38)], fill=(30, 35, 45))
    d.polygon([(28, 18), (32, 8), (36, 18)], fill=(25, 30, 40))
    return img

def gen_giant_squid():
    """Giant squid - large tentacles, big eyes."""
    img = Image.new("RGBA", (96, 64), DARK_BG)
    d = ImageDraw.Draw(img)
    # Mantle
    d.ellipse([50, 8, 90, 56], fill=(60, 40, 70))
    d.ellipse([54, 12, 86, 52], fill=(75, 55, 85))
    # Head
    d.ellipse([38, 16, 60, 48], fill=(65, 45, 75))
    # Big eyes
    draw_eye(d, 46, 24, 5, (200, 180, 40), (220, 200, 60))
    draw_eye(d, 46, 40, 5, (200, 180, 40), (220, 200, 60))
    # Tentacles
    for i in range(8):
        y = 20 + i * 4
        for tx in range(38, 4, -2):
            wobble = int(3 * math.sin(tx * 0.3 + i))
            d.rectangle([tx, y + wobble, tx + 1, y + wobble + 2],
                       fill=TENTACLE_MID if tx % 4 == 0 else TENTACLE_DARK)
    # Two long feeding tentacles
    for y_base in [26, 38]:
        for tx in range(38, 0, -2):
            wobble = int(4 * math.sin(tx * 0.2 + y_base))
            d.rectangle([tx, y_base + wobble, tx + 1, y_base + wobble + 1],
                       fill=BIOLUM_PURPLE)
    return img

def gen_whale():
    """Whale - gentle giant, huge."""
    img = Image.new("RGBA", (128, 64), DARK_BG)
    d = ImageDraw.Draw(img)
    # Massive body
    d.ellipse([10, 8, 110, 56], fill=(50, 55, 65))
    d.ellipse([14, 12, 106, 52], fill=(60, 68, 78))
    # Lighter belly
    d.ellipse([20, 30, 100, 54], fill=(75, 82, 92))
    # Eye (small for body size)
    d.ellipse([96, 22, 102, 28], fill=(40, 40, 50))
    d.point((99, 25), fill=(150, 150, 160))
    # Mouth line
    d.arc([80, 24, 116, 44], 0, 90, fill=(45, 50, 60), width=2)
    # Tail fluke
    d.polygon([(10, 24), (0, 8), (0, 20)], fill=(50, 55, 65))
    d.polygon([(10, 40), (0, 56), (0, 44)], fill=(50, 55, 65))
    # Dorsal fin
    d.polygon([(50, 12), (56, 2), (62, 12)], fill=(55, 60, 70))
    # Barnacles
    for _ in range(5):
        bx = random.randint(30, 90)
        by = random.randint(14, 28)
        d.ellipse([bx, by, bx+3, by+3], fill=(80, 85, 75))
    return img

# === ENVIRONMENT ===

def gen_rock():
    """Ocean floor rock."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    rock_dark = (35, 40, 48)
    rock_mid = (50, 58, 65)
    rock_light = (70, 78, 85)
    # Main rock shape
    d.polygon([(8, 56), (4, 38), (12, 24), (28, 18), (44, 20),
               (56, 30), (60, 44), (56, 56)], fill=rock_mid)
    # Shadow
    d.polygon([(8, 56), (4, 38), (12, 28), (20, 40), (16, 56)], fill=rock_dark)
    # Highlight
    d.polygon([(28, 18), (36, 20), (40, 28), (32, 26)], fill=rock_light)
    # Algae patches
    for _ in range(4):
        ax = random.randint(12, 52)
        ay = random.randint(28, 52)
        d.rectangle([ax, ay, ax+3, ay+2], fill=(30, 60, 40))
    return img

def gen_coral():
    """Coral cluster - dark muted colors."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    colors = [(100, 50, 50), (80, 45, 60), (90, 55, 45), (70, 50, 55)]
    for i in range(5):
        cx = random.randint(12, 52)
        base_y = 56
        color = colors[i % len(colors)]
        # Coral branch
        height = random.randint(16, 36)
        for cy in range(base_y, base_y - height, -2):
            width = max(2, 6 - (base_y - cy) // 6)
            wobble = int(2 * math.sin(cy * 0.3 + cx))
            d.rectangle([cx + wobble - width, cy, cx + wobble + width, cy + 2], fill=color)
        # Tips
        d.ellipse([cx - 3, base_y - height - 2, cx + 3, base_y - height + 2],
                 fill=tuple(min(255, c + 30) for c in color))
    return img

def gen_kelp():
    """Kelp strand - tall seaweed."""
    img = Image.new("RGBA", (32, 96), DARK_BG)
    d = ImageDraw.Draw(img)
    kelp_dark = (25, 50, 30)
    kelp_mid = (35, 70, 40)
    kelp_light = (50, 90, 55)
    # Main stalk
    for y in range(90, 4, -2):
        wobble = int(4 * math.sin(y * 0.08))
        width = max(1, 3 - y // 40)
        d.rectangle([16 + wobble - width, y, 16 + wobble + width, y + 2], fill=kelp_mid)
    # Leaves
    for y in range(80, 10, -12):
        wobble = int(4 * math.sin(y * 0.08))
        side = 1 if random.random() > 0.5 else -1
        leaf_x = 16 + wobble + side * 4
        leaf_x2 = leaf_x + 8 * side
        lx0, lx1 = min(leaf_x, leaf_x2), max(leaf_x, leaf_x2)
        d.ellipse([lx0, y - 4, lx1, y + 4], fill=kelp_light)
    return img

def gen_thermal_vent():
    """Hydrothermal vent - volcanic, hot particles."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    vent_dark = (40, 30, 25)
    vent_mid = (60, 45, 35)
    vent_hot = (140, 80, 30)
    # Rock chimney
    d.polygon([(20, 58), (16, 40), (22, 28), (32, 24), (42, 28),
               (48, 40), (44, 58)], fill=vent_mid)
    d.polygon([(22, 58), (18, 42), (24, 30), (30, 28)], fill=vent_dark)
    # Vent opening
    d.ellipse([24, 22, 40, 30], fill=(60, 20, 10))
    d.ellipse([26, 24, 38, 28], fill=vent_hot)
    # Heat shimmer / particles rising
    for _ in range(15):
        px = random.randint(24, 40)
        py = random.randint(4, 22)
        size = random.randint(1, 3)
        alpha = max(60, 200 - py * 8)
        d.ellipse([px, py, px + size, py + size], fill=(180, 100, 40, alpha))
    # Mineral deposits
    for _ in range(6):
        mx = random.randint(16, 48)
        my = random.randint(36, 56)
        d.rectangle([mx, my, mx + 2, my + 2], fill=(120, 110, 50))
    return img

def gen_biolum_spot():
    """Bioluminescent spot on ocean floor."""
    img = new_sprite(32)
    d = ImageDraw.Draw(img)
    cx, cy = 16, 16
    # Outer glow
    for r in range(14, 2, -2):
        alpha = max(20, 120 - r * 8)
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(30, 140, 120, alpha))
    # Core
    d.ellipse([cx - 3, cy - 3, cx + 3, cy + 3], fill=BIOLUM_CYAN)
    d.ellipse([cx - 1, cy - 1, cx + 1, cy + 1], fill=(150, 255, 240))
    return img

# === VFX ===

def gen_sonar_ring():
    """Sonar ping expanding ring."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    cx, cy = 32, 32
    for r in [28, 24, 20]:
        alpha = 60 + (28 - r) * 15
        d.ellipse([cx-r, cy-r, cx+r, cy+r], outline=(40, 200, 100, alpha), width=2)
    d.ellipse([cx-2, cy-2, cx+2, cy+2], fill=(60, 255, 140))
    return img

def gen_torpedo_trail():
    """Torpedo projectile with trail."""
    img = Image.new("RGBA", (48, 16), DARK_BG)
    d = ImageDraw.Draw(img)
    # Torpedo body
    d.rectangle([28, 4, 44, 12], fill=(70, 75, 85))
    d.rectangle([44, 5, 48, 11], fill=(160, 40, 40))
    # Trail
    for tx in range(0, 28, 3):
        alpha = tx * 8
        d.rectangle([tx, 6, tx + 2, 10], fill=(200, 200, 200, alpha))
    return img

def gen_explosion():
    """Explosion effect."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    cx, cy = 32, 32
    # Outer blast
    d.ellipse([cx-24, cy-24, cx+24, cy+24], fill=(180, 80, 20, 120))
    d.ellipse([cx-18, cy-18, cx+18, cy+18], fill=(220, 120, 30, 160))
    d.ellipse([cx-12, cy-12, cx+12, cy+12], fill=(240, 180, 60, 200))
    d.ellipse([cx-6, cy-6, cx+6, cy+6], fill=(255, 240, 200, 240))
    # Debris
    for _ in range(12):
        a = random.random() * math.pi * 2
        r = random.randint(10, 28)
        px = cx + int(r * math.cos(a))
        py = cy + int(r * math.sin(a))
        d.rectangle([px, py, px + 2, py + 2], fill=(200, 100, 30))
    return img

def gen_bubble():
    """Water bubbles."""
    img = new_sprite(32)
    d = ImageDraw.Draw(img)
    positions = [(8, 20), (14, 10), (20, 24), (12, 4), (22, 14)]
    for bx, by in positions:
        r = random.randint(2, 5)
        d.ellipse([bx-r, by-r, bx+r, by+r], outline=(100, 160, 200, 140), width=1)
        d.point((bx-1, by-1), fill=(180, 220, 255, 160))
    return img

def gen_electric_shock():
    """Electric shock effect."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    cx, cy = 32, 32
    for _ in range(12):
        a = random.random() * math.pi * 2
        r = random.randint(8, 28)
        points = [(cx, cy)]
        x, y = cx, cy
        for _ in range(4):
            x += int(r / 4 * math.cos(a) + random.randint(-4, 4))
            y += int(r / 4 * math.sin(a) + random.randint(-4, 4))
            points.append((x, y))
        for i in range(len(points) - 1):
            d.line([points[i], points[i+1]], fill=ELECTRIC_BLUE, width=1)
    d.ellipse([cx-4, cy-4, cx+4, cy+4], fill=(200, 220, 255, 180))
    return img


def main():
    hostile = {
        "scavenger": gen_scavenger,
        "stalker": gen_stalker,
        "ambusher": gen_ambusher,
        "electric_eel": gen_electric_eel,
        "blind_hunter": gen_blind_hunter,
        "lure_fish": gen_lure_fish,
        "swarm_queen": gen_swarm_queen,
        "leviathan": gen_leviathan,
        "parasite": gen_parasite,
        "watcher": gen_watcher,
    }
    ambient = {
        "small_fish": gen_small_fish,
        "jellyfish": gen_jellyfish,
        "school_fish": gen_school_fish,
        "deep_fish": gen_deep_fish,
        "giant_squid": gen_giant_squid,
        "whale": gen_whale,
    }
    environment = {
        "rock": gen_rock,
        "coral": gen_coral,
        "kelp": gen_kelp,
        "thermal_vent": gen_thermal_vent,
        "bioluminescent_spot": gen_biolum_spot,
    }
    effects = {
        "sonar_ring": gen_sonar_ring,
        "torpedo_trail": gen_torpedo_trail,
        "explosion": gen_explosion,
        "bubble": gen_bubble,
        "electric_shock": gen_electric_shock,
    }

    for name, gen in hostile.items():
        img = gen()
        path = os.path.join(OUT, "creatures", "hostile", f"{name}.png")
        img.save(path)
        print(f"  [HOSTILE]  {name}.png ({img.size[0]}x{img.size[1]})")

    for name, gen in ambient.items():
        img = gen()
        path = os.path.join(OUT, "creatures", "ambient", f"{name}.png")
        img.save(path)
        print(f"  [AMBIENT]  {name}.png ({img.size[0]}x{img.size[1]})")

    for name, gen in environment.items():
        img = gen()
        path = os.path.join(OUT, "environment", f"{name}.png")
        img.save(path)
        print(f"  [ENV]      {name}.png ({img.size[0]}x{img.size[1]})")

    for name, gen in effects.items():
        img = gen()
        path = os.path.join(OUT, "effects", f"{name}.png")
        img.save(path)
        print(f"  [VFX]      {name}.png ({img.size[0]}x{img.size[1]})")

    total = len(hostile) + len(ambient) + len(environment) + len(effects)
    print(f"\nGenerated {total} sprites:")
    print(f"  Hostile:     {len(hostile)}")
    print(f"  Ambient:     {len(ambient)}")
    print(f"  Environment: {len(environment)}")
    print(f"  VFX:         {len(effects)}")


if __name__ == "__main__":
    main()
