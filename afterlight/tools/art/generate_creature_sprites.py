#!/usr/bin/env python3
"""
Generate Barotrauma-style deep-sea creature sprites.
Dark silhouettes, bioluminescent accents, organic horror aesthetic.
Grotesque, alien, unsettling — the ocean wants to kill you.
"""

from PIL import Image, ImageDraw, ImageFilter
import math
import random
import os

random.seed(42)

HOSTILE_DIR = "assets/sprites/creatures/hostile"
AMBIENT_DIR = "assets/sprites/creatures/ambient"

os.makedirs(HOSTILE_DIR, exist_ok=True)
os.makedirs(AMBIENT_DIR, exist_ok=True)


def noise_color(base, variance=15):
    """Add subtle noise to a color for organic texture."""
    return tuple(
        max(0, min(255, c + random.randint(-variance, variance)))
        for c in base[:3]
    ) + (base[3] if len(base) > 3 else 255,)


def draw_organic_blob(draw, cx, cy, rx, ry, base_color, noise=True, segments=24):
    """Draw an irregular organic shape with noisy edges."""
    points = []
    for i in range(segments):
        angle = (2 * math.pi * i) / segments
        wobble = random.uniform(0.82, 1.18)
        x = cx + rx * wobble * math.cos(angle)
        y = cy + ry * wobble * math.sin(angle)
        points.append((x, y))
    draw.polygon(points, fill=base_color)
    if noise:
        for px in range(int(cx - rx - 2), int(cx + rx + 2)):
            for py in range(int(cy - ry - 2), int(cy + ry + 2)):
                dx = (px - cx) / max(rx, 1)
                dy = (py - cy) / max(ry, 1)
                if dx * dx + dy * dy < 1.0 and random.random() < 0.25:
                    nc = noise_color(base_color, 8)
                    try:
                        draw.point((px, py), fill=nc)
                    except Exception:
                        pass


def draw_glow(img, x, y, radius, color, intensity=0.7):
    """Draw a bioluminescent glow effect."""
    for r in range(radius, 0, -1):
        alpha = int(255 * intensity * (r / radius) * 0.3)
        glow_color = color[:3] + (min(255, alpha),)
        overlay = Image.new("RGBA", img.size, (0, 0, 0, 0))
        od = ImageDraw.Draw(overlay)
        od.ellipse([x - r, y - r, x + r, y + r], fill=glow_color)
        img = Image.alpha_composite(img, overlay)
    return img


def add_texture_noise(img, intensity=0.15):
    """Add subtle organic texture noise to non-transparent pixels."""
    pixels = img.load()
    w, h = img.size
    for x in range(w):
        for y in range(h):
            r, g, b, a = pixels[x, y]
            if a > 30:
                v = int(20 * intensity)
                nr = max(0, min(255, r + random.randint(-v, v)))
                ng = max(0, min(255, g + random.randint(-v, v)))
                nb = max(0, min(255, b + random.randint(-v, v)))
                pixels[x, y] = (nr, ng, nb, a)
    return img


def draw_eye(draw, x, y, size, glow_color=(0, 255, 200)):
    """Draw a glowing creature eye."""
    draw.ellipse(
        [x - size - 1, y - size - 1, x + size + 1, y + size + 1],
        fill=glow_color[:3] + (60,),
    )
    draw.ellipse([x - size, y - size, x + size, y + size], fill=glow_color)
    if size >= 2:
        draw.ellipse([x - 1, y - 1, x + 1, y + 1], fill=(255, 255, 255, 230))


def draw_teeth(draw, x_start, x_end, y, down=True, count=6, color=(200, 200, 190, 220)):
    """Draw a row of sharp teeth."""
    spacing = (x_end - x_start) / max(count, 1)
    for i in range(count):
        tx = x_start + i * spacing + spacing / 2
        th = random.randint(3, 6)
        if down:
            draw.polygon([(tx - 1, y), (tx + 1, y), (tx, y + th)], fill=color)
        else:
            draw.polygon([(tx - 1, y), (tx + 1, y), (tx, y - th)], fill=color)


# ============================================================================
# HOSTILE CREATURES
# ============================================================================


def generate_stalker():
    """Sleek predatory fish. Dark blue-black, cyan bioluminescent stripe, sharp teeth."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    body_color = (18, 22, 32, 255)
    body_points = [
        (8, 32), (12, 24), (20, 20), (35, 18), (48, 22), (56, 30),
        (58, 32), (56, 34), (48, 42), (35, 46), (20, 44), (12, 40),
    ]
    draw.polygon(body_points, fill=body_color)

    # Dorsal + ventral fins
    draw.polygon([(25, 20), (30, 10), (38, 18)], fill=(14, 18, 28, 240))
    draw.polygon([(28, 44), (32, 52), (38, 46)], fill=(14, 18, 28, 240))

    # Tail
    draw.polygon([(8, 32), (2, 22), (5, 30)], fill=(14, 18, 28, 240))
    draw.polygon([(8, 32), (2, 42), (5, 34)], fill=(14, 18, 28, 240))

    # Bioluminescent stripe
    for x in range(15, 52):
        y_off = int(math.sin((x - 15) * 0.08) * 2)
        intensity = int(80 + 40 * math.sin((x - 15) * 0.15))
        draw.point((x, 30 + y_off), fill=(0, intensity, intensity + 40, 180))
        draw.point((x, 31 + y_off), fill=(0, intensity, intensity + 40, 180))

    # Jaw + teeth
    draw.polygon(
        [(52, 28), (60, 31), (58, 32), (60, 33), (52, 36)], fill=(25, 15, 15, 255)
    )
    draw_teeth(draw, 53, 60, 31, down=True, count=4, color=(180, 180, 170, 230))
    draw_teeth(draw, 53, 60, 33, down=False, count=3, color=(180, 180, 170, 230))

    draw_eye(draw, 50, 28, 2, (0, 220, 180))
    add_texture_noise(img, 0.2)
    img = draw_glow(img, 50, 28, 6, (0, 220, 180), 0.4)
    img.save(f"{HOSTILE_DIR}/stalker.png")
    print("  stalker.png")


def generate_scavenger():
    """Crab/isopod scavenger. Multiple legs, armored, dull colors."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    base = (45, 35, 25, 255)
    dark = (30, 22, 16, 255)

    draw_organic_blob(draw, 32, 30, 20, 14, base, segments=16)

    # Shell segments
    for sx in range(16, 48, 5):
        draw.line([(sx, 18), (sx + 1, 42)], fill=dark, width=1)

    # 6 legs
    leg_color = (55, 40, 30, 230)
    for i, (lx, ly) in enumerate(
        [(14, 28), (18, 32), (22, 35), (42, 35), (46, 32), (50, 28)]
    ):
        if i < 3:
            draw.line([(lx, ly), (lx - 8, ly + 14)], fill=leg_color, width=2)
            draw.line([(lx - 8, ly + 14), (lx - 12, ly + 18)], fill=leg_color, width=1)
        else:
            draw.line([(lx, ly), (lx + 8, ly + 14)], fill=leg_color, width=2)
            draw.line([(lx + 8, ly + 14), (lx + 12, ly + 18)], fill=leg_color, width=1)

    # Mandibles
    draw.polygon([(12, 28), (6, 26), (8, 30)], fill=(60, 45, 35, 240))
    draw.polygon([(12, 32), (6, 34), (8, 30)], fill=(60, 45, 35, 240))

    draw_eye(draw, 13, 26, 1, (200, 150, 50))
    draw_eye(draw, 13, 34, 1, (200, 150, 50))
    add_texture_noise(img, 0.25)
    img = draw_glow(img, 13, 26, 4, (200, 150, 50), 0.3)
    img = draw_glow(img, 13, 34, 4, (200, 150, 50), 0.3)
    img.save(f"{HOSTILE_DIR}/scavenger.png")
    print("  scavenger.png")


def generate_ambusher():
    """Flat, wide ambush predator. Camouflaged, massive hidden mouth."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    body_color = (22, 28, 20, 255)
    body_points = [
        (6, 30), (10, 24), (20, 20), (44, 20), (54, 24), (58, 30),
        (58, 36), (54, 40), (44, 44), (20, 44), (10, 40), (6, 36),
    ]
    draw.polygon(body_points, fill=body_color)

    # Camouflage patches
    for _ in range(12):
        px, py = random.randint(12, 52), random.randint(22, 42)
        draw.ellipse(
            [px - random.randint(2, 5), py - random.randint(2, 4),
             px + random.randint(2, 5), py + random.randint(2, 4)],
            fill=(15, 20, 14, 200),
        )

    # Massive mouth
    draw.polygon([(4, 28), (2, 32), (4, 36), (10, 34), (10, 30)], fill=(40, 10, 10, 240))
    draw_teeth(draw, 3, 10, 29, down=True, count=4, color=(160, 155, 140, 220))
    draw_teeth(draw, 3, 10, 35, down=False, count=4, color=(160, 155, 140, 220))

    # Tiny eyes barely visible
    draw.point((16, 24), fill=(100, 180, 80, 180))
    draw.point((16, 25), fill=(100, 180, 80, 180))

    # Pectoral flaps
    draw.polygon([(20, 44), (15, 52), (25, 48)], fill=(18, 24, 16, 220))
    draw.polygon([(44, 44), (49, 52), (39, 48)], fill=(18, 24, 16, 220))
    add_texture_noise(img, 0.3)
    img.save(f"{HOSTILE_DIR}/ambusher.png")
    print("  ambusher.png")


def generate_electric_eel():
    """Serpentine body with electric blue/yellow crackling patterns."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    body_color = (16, 18, 30, 255)
    pts_top, pts_bot = [], []
    for x in range(8, 58):
        yc = 32 + int(8 * math.sin((x - 8) * 0.12))
        w = 5 + int(3 * math.sin((x - 8) * 0.06))
        if x < 15:
            w = max(2, int(w * (x - 8) / 7))
        if x > 50:
            w = max(1, int(w * (58 - x) / 8))
        pts_top.append((x, yc - w))
        pts_bot.append((x, yc + w))
    draw.polygon(pts_top + list(reversed(pts_bot)), fill=body_color)

    # Electric arcs
    for x in range(10, 55, 3):
        yc = 32 + int(8 * math.sin((x - 8) * 0.12))
        ai = random.randint(120, 255)
        yo = random.randint(-3, 3)
        draw.point((x, yc + yo), fill=(ai // 3, ai // 2, ai, 200))
        if random.random() < 0.4:
            for s in range(random.randint(2, 5)):
                draw.point(
                    (x + random.randint(-1, 1), yc + yo + s * random.choice([-1, 1])),
                    fill=(80, 150, ai, 150),
                )

    # Electrical nodes
    nodes = [
        (15, 32 + int(8 * math.sin(7 * 0.12))),
        (30, 32 + int(8 * math.sin(22 * 0.12))),
        (45, 32 + int(8 * math.sin(37 * 0.12))),
    ]
    for nx, ny in nodes:
        draw.ellipse([nx - 2, ny - 2, nx + 2, ny + 2], fill=(200, 180, 40, 220))

    # Head
    hx = 56
    hy = 32 + int(8 * math.sin(48 * 0.12))
    draw.ellipse([hx - 4, hy - 5, hx + 4, hy + 5], fill=(20, 22, 35, 255))
    draw_eye(draw, hx + 1, hy - 2, 1, (100, 200, 255))

    add_texture_noise(img, 0.15)
    for nx, ny in nodes:
        img = draw_glow(img, nx, ny, 5, (100, 180, 255), 0.3)
    img.save(f"{HOSTILE_DIR}/electric_eel.png")
    print("  electric_eel.png")


def generate_blind_hunter():
    """Massive, eyeless, huge jaw. Echolocation ridges. Pure horror."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    body_color = (28, 20, 22, 255)
    draw_organic_blob(draw, 28, 32, 22, 18, body_color, segments=20)

    # Massive jaw
    jaw = (35, 24, 26, 255)
    draw.polygon([(48, 24), (62, 30), (60, 32), (48, 30)], fill=jaw)
    draw.polygon([(48, 40), (62, 34), (60, 32), (48, 34)], fill=jaw)

    draw_teeth(draw, 48, 62, 30, down=True, count=7, color=(200, 190, 175, 240))
    draw_teeth(draw, 48, 62, 34, down=False, count=7, color=(200, 190, 175, 240))

    # Mouth interior
    draw.polygon([(50, 30), (58, 31), (58, 33), (50, 34)], fill=(60, 10, 10, 200))

    # Echolocation ridges instead of eyes
    for i in range(4):
        rx = 40 + i * 3
        draw.arc([rx - 6, 22, rx + 6, 42], 250, 290, fill=(50, 35, 38, 200), width=1)

    # Sensory pits — faint reddish where eyes would be
    draw.ellipse([46, 26, 49, 29], fill=(80, 20, 20, 160))
    draw.ellipse([46, 35, 49, 38], fill=(80, 20, 20, 160))

    # Scars
    for _ in range(6):
        sx, sy = random.randint(12, 44), random.randint(20, 44)
        draw.line(
            [(sx, sy), (sx + random.randint(3, 8), sy + random.randint(-2, 2))],
            fill=(40, 30, 32, 150), width=1,
        )

    # Tail
    draw.polygon([(6, 32), (2, 22), (8, 28)], fill=(22, 16, 18, 240))
    draw.polygon([(6, 32), (2, 42), (8, 36)], fill=(22, 16, 18, 240))

    add_texture_noise(img, 0.25)
    img = draw_glow(img, 47, 27, 4, (120, 30, 30), 0.3)
    img = draw_glow(img, 47, 37, 4, (120, 30, 30), 0.3)
    img.save(f"{HOSTILE_DIR}/blind_hunter.png")
    print("  blind_hunter.png")


def generate_lure_fish():
    """Anglerfish. Huge mouth, bioluminescent lure. Classic deep-sea nightmare."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    body_color = (20, 16, 24, 255)
    draw_organic_blob(draw, 30, 34, 18, 16, body_color, segments=18)

    # Massive mouth
    draw.polygon([(46, 24), (62, 30), (62, 38), (46, 44)], fill=(26, 18, 28, 255))
    draw.polygon([(50, 28), (60, 31), (60, 37), (50, 40)], fill=(50, 8, 12, 240))

    # Needle teeth
    for i in range(6):
        tx = 50 + i * 2
        tl = random.randint(4, 8)
        draw.line([(tx, 28), (tx + random.randint(-1, 1), 28 + tl)], fill=(200, 195, 180, 230), width=1)
        draw.line([(tx, 40), (tx + random.randint(-1, 1), 40 - tl)], fill=(200, 195, 180, 230), width=1)

    # Bioluminescent lure antenna
    lure = [(35, 20), (32, 12), (28, 8), (26, 6)]
    for i in range(len(lure) - 1):
        draw.line([lure[i], lure[i + 1]], fill=(40, 30, 45, 200), width=1)
    draw.ellipse([23, 3, 29, 9], fill=(80, 200, 255, 240))
    draw.ellipse([24, 4, 28, 8], fill=(150, 240, 255, 255))

    draw_eye(draw, 44, 30, 2, (180, 40, 40))
    draw.polygon([(14, 30), (8, 24), (10, 32)], fill=(18, 14, 22, 220))

    add_texture_noise(img, 0.2)
    img = draw_glow(img, 26, 6, 10, (80, 200, 255), 0.6)
    img = draw_glow(img, 44, 30, 4, (180, 40, 40), 0.3)
    img.save(f"{HOSTILE_DIR}/lure_fish.png")
    print("  lure_fish.png")


def generate_swarm_queen():
    """Bloated organic mass. Dark purple, glowing spawning nodes."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    draw_organic_blob(draw, 32, 32, 22, 22, (30, 15, 35, 255), segments=24)
    draw_organic_blob(draw, 32, 32, 16, 16, (22, 10, 28, 240), segments=16)

    # Spawning pustules
    nodes = [(18, 22), (44, 20), (14, 38), (48, 40), (32, 14), (32, 50)]
    for nx, ny in nodes:
        draw.ellipse([nx - 3, ny - 3, nx + 3, ny + 3], fill=(50, 120, 40, 220))
        draw.ellipse([nx - 1, ny - 1, nx + 1, ny + 1], fill=(80, 200, 60, 255))

    # Radiating tendrils
    for angle_deg in range(0, 360, 45):
        angle = math.radians(angle_deg + random.randint(-10, 10))
        length = random.randint(6, 12)
        sx = 32 + int(20 * math.cos(angle))
        sy = 32 + int(20 * math.sin(angle))
        ex = 32 + int((20 + length) * math.cos(angle))
        ey = 32 + int((20 + length) * math.sin(angle))
        draw.line([(sx, sy), (ex, ey)], fill=(35, 18, 40, 180), width=2)

    # Central orifice
    draw.ellipse([28, 28, 36, 36], fill=(60, 20, 50, 230))
    draw.ellipse([30, 30, 34, 34], fill=(120, 40, 80, 255))

    add_texture_noise(img, 0.2)
    for nx, ny in nodes:
        img = draw_glow(img, nx, ny, 5, (80, 200, 60), 0.4)
    img = draw_glow(img, 32, 32, 6, (120, 40, 80), 0.3)
    img.save(f"{HOSTILE_DIR}/swarm_queen.png")
    print("  swarm_queen.png")


def generate_leviathan():
    """Massive armored sea monster. 128x64. Segmented body, multiple red eyes, dorsal spines."""
    img = Image.new("RGBA", (128, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    body_color = (14, 12, 18, 255)

    # Segmented body
    for seg in range(8):
        cx = 95 - seg * 12
        ry = 22 - seg * 1.5
        rx = 14 - seg * 0.5
        draw_organic_blob(draw, cx, 32, rx, ry, noise_color(body_color, 5), segments=12, noise=False)
        draw.line(
            [(cx - rx + 2, int(32 - ry + 3)), (cx - rx + 2, int(32 + ry - 3))],
            fill=(30, 24, 35, 180), width=1,
        )

    # Armored head
    draw.polygon([(98, 16), (124, 26), (126, 32), (124, 38), (98, 48)], fill=(18, 14, 22, 255))
    draw.polygon([(100, 18), (110, 16), (115, 24), (105, 26)], fill=(28, 22, 32, 220))
    draw.polygon([(100, 46), (110, 48), (115, 40), (105, 38)], fill=(28, 22, 32, 220))

    # 3 pairs of glowing red eyes
    for i, (ex, ey_top) in enumerate([(108, 24), (114, 28), (118, 31)]):
        s = max(1, 2 - (i // 2))
        draw_eye(draw, ex, ey_top, s, (180, 0, 0))
        draw_eye(draw, ex, 64 - ey_top, s, (180, 0, 0))

    # Massive jaw
    draw.polygon([(122, 28), (128, 31), (128, 33), (122, 36)], fill=(40, 12, 14, 255))
    draw_teeth(draw, 120, 128, 29, down=True, count=5, color=(180, 175, 160, 240))
    draw_teeth(draw, 120, 128, 35, down=False, count=5, color=(180, 175, 160, 240))

    # Tail
    draw.polygon([(4, 32), (0, 18), (10, 28)], fill=(12, 10, 16, 230))
    draw.polygon([(4, 32), (0, 46), (10, 36)], fill=(12, 10, 16, 230))

    # Bioluminescent veins
    for x in range(15, 100, 2):
        sw = 22 - abs(x - 55) * 0.15
        vy = 32 - int(sw * 0.6) + int(2 * math.sin(x * 0.1))
        draw.point((x, vy), fill=(80, 0, 60, int(40 + 20 * math.sin(x * 0.2))))
        draw.point((x, 64 - vy), fill=(80, 0, 60, int(40 + 20 * math.sin(x * 0.2))))

    # Dorsal spines
    for sx in range(20, 95, 10):
        sh = random.randint(4, 8)
        sw = 22 - abs(sx - 55) * 0.15
        by = 32 - int(sw) + 2
        draw.polygon([(sx, by), (sx + 2, by - sh), (sx + 4, by)], fill=(24, 18, 30, 220))

    add_texture_noise(img, 0.2)
    for ex, ey_top in [(108, 24), (114, 28), (118, 31)]:
        img = draw_glow(img, ex, ey_top, 5, (180, 0, 0), 0.4)
        img = draw_glow(img, ex, 64 - ey_top, 5, (180, 0, 0), 0.4)
    img.save(f"{HOSTILE_DIR}/leviathan.png")
    print("  leviathan.png")


def generate_parasite():
    """Small, translucent, insectoid. Hooked appendages, feeding tube."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    draw_organic_blob(draw, 32, 32, 10, 7, (40, 30, 25, 160), segments=12)

    # Visible organs
    draw.ellipse([28, 30, 34, 35], fill=(80, 30, 30, 120))
    draw.ellipse([30, 28, 36, 32], fill=(60, 50, 20, 100))

    # 4 grasping hooks
    hc = (70, 50, 40, 220)
    for x1, y1, x2, y2, x3, y3 in [
        (42, 28, 50, 24, 48, 22), (42, 36, 50, 40, 48, 42),
        (22, 28, 16, 24, 18, 22), (22, 36, 16, 40, 18, 42),
    ]:
        draw.line([(x1, y1), (x2, y2)], fill=hc, width=1)
        draw.line([(x2, y2), (x3, y3)], fill=hc, width=1)

    # Feeding proboscis
    draw.line([(42, 32), (52, 32)], fill=(60, 20, 20, 200), width=1)
    draw.point((52, 32), fill=(100, 30, 30, 230))

    draw.point((40, 30), fill=(200, 200, 100, 200))
    draw.point((40, 34), fill=(200, 200, 100, 200))
    add_texture_noise(img, 0.15)
    img.save(f"{HOSTILE_DIR}/parasite.png")
    print("  parasite.png")


def generate_watcher():
    """Large single eye, trailing tentacles. All-seeing ethereal observer."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Eye structure
    draw.ellipse([16, 16, 48, 48], fill=(20, 25, 40, 240))
    draw.ellipse([22, 22, 42, 42], fill=(10, 30, 50, 255))
    draw.ellipse([28, 28, 36, 36], fill=(0, 150, 200, 255))
    draw.ellipse([30, 30, 34, 34], fill=(100, 220, 255, 255))
    draw.ellipse([31, 31, 33, 33], fill=(200, 255, 255, 255))

    # Fleshy membrane
    draw.arc([14, 14, 50, 50], 0, 360, fill=(25, 18, 30, 200), width=2)

    # Trailing tentacles
    for t in range(5):
        tx = 22 + t * 5
        pts = [(tx, 46)]
        for seg in range(6):
            pts.append((tx + int(3 * math.sin(seg * 0.8 + t)), 48 + seg * 3))
        for i in range(len(pts) - 1):
            alpha = max(40, 160 - i * 20)
            draw.line([pts[i], pts[i + 1]], fill=(22, 20, 35, alpha), width=1)

    # Sensory nubs
    for i in range(3):
        nx = 26 + i * 6
        draw.line([(nx, 18), (nx + random.randint(-2, 2), 12)], fill=(30, 22, 38, 180), width=1)

    add_texture_noise(img, 0.15)
    img = draw_glow(img, 32, 32, 14, (0, 150, 200), 0.5)
    img.save(f"{HOSTILE_DIR}/watcher.png")
    print("  watcher.png")


# ============================================================================
# AMBIENT CREATURES
# ============================================================================


def generate_small_fish():
    """Tiny silvery fish. 32x32."""
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    draw.polygon(
        [(6, 16), (10, 12), (22, 11), (26, 14), (26, 18), (22, 21), (10, 20)],
        fill=(140, 150, 160, 200),
    )
    draw.polygon([(6, 16), (2, 12), (4, 16), (2, 20)], fill=(120, 130, 140, 180))
    draw.point((23, 14), fill=(20, 20, 20, 255))
    draw.point((18, 13), fill=(180, 190, 200, 150))
    draw.point((14, 14), fill=(170, 180, 190, 120))
    add_texture_noise(img, 0.1)
    img.save(f"{AMBIENT_DIR}/small_fish.png")
    print("  small_fish.png")


def generate_jellyfish():
    """Translucent bell, trailing tentacles, faint ethereal glow."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    draw.ellipse([16, 8, 48, 34], fill=(120, 100, 160, 70))
    draw.ellipse([20, 12, 44, 30], fill=(140, 120, 180, 50))
    draw.arc([16, 8, 48, 34], 150, 390, fill=(150, 130, 190, 100), width=1)

    # Oral arms
    for t in range(3):
        tx = 26 + t * 6
        for seg in range(8):
            sy = 32 + seg * 3
            sx = tx + int(2 * math.sin(seg * 0.6 + t * 1.5))
            a = max(20, 80 - seg * 8)
            draw.point((sx, sy), fill=(140, 120, 180, a))

    # Thin tentacles
    for t in range(6):
        tx = 18 + t * 5
        for seg in range(10):
            sy = 34 + seg * 3
            sx = tx + int(3 * math.sin(seg * 0.4 + t * 0.7))
            draw.point((sx, sy), fill=(160, 140, 200, max(10, 60 - seg * 5)))

    add_texture_noise(img, 0.08)
    img = draw_glow(img, 32, 20, 12, (140, 120, 200), 0.25)
    img.save(f"{AMBIENT_DIR}/jellyfish.png")
    print("  jellyfish.png")


def generate_school_fish():
    """Cluster of small fish silhouettes."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    positions = [
        (12, 18), (24, 14), (38, 12), (50, 16),
        (8, 30), (20, 28), (34, 26), (48, 30),
        (14, 42), (28, 38), (42, 40), (54, 44),
    ]
    for fx, fy in positions:
        br = random.randint(100, 150)
        color = (br - 20, br, br + 10, random.randint(150, 220))
        s = random.uniform(0.7, 1.3)
        draw.polygon([
            (fx - 4 * s, fy), (fx - 2 * s, fy - 2 * s),
            (fx + 4 * s, fy - s), (fx + 5 * s, fy),
            (fx + 4 * s, fy + s), (fx - 2 * s, fy + 2 * s),
        ], fill=color)
        draw.polygon([
            (fx - 4 * s, fy), (fx - 7 * s, fy - 2 * s),
            (fx - 5 * s, fy), (fx - 7 * s, fy + 2 * s),
        ], fill=(color[0] - 20, color[1] - 20, color[2] - 20, color[3] - 40))
        draw.point((int(fx + 3 * s), int(fy - 0.5 * s)), fill=(30, 30, 30, 200))

    add_texture_noise(img, 0.1)
    img.save(f"{AMBIENT_DIR}/school_fish.png")
    print("  school_fish.png")


def generate_deep_fish():
    """Bioluminescent deep-sea fish. Dark body, photophores, large eyes."""
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    draw.polygon([
        (10, 32), (14, 24), (24, 20), (40, 18), (50, 24), (54, 32),
        (50, 40), (40, 46), (24, 44), (14, 40),
    ], fill=(12, 14, 22, 255))

    # Fins
    draw.polygon([(22, 20), (26, 12), (32, 18)], fill=(10, 12, 20, 230))
    draw.polygon([(24, 44), (28, 52), (34, 46)], fill=(10, 12, 20, 230))
    draw.polygon([(10, 32), (4, 26), (8, 30)], fill=(10, 12, 20, 230))
    draw.polygon([(10, 32), (4, 38), (8, 34)], fill=(10, 12, 20, 230))

    # Photophore line
    colors = [(0, 180, 200), (0, 160, 220), (20, 200, 180)]
    for i, px in enumerate(range(18, 48, 4)):
        py = 30 + int(2 * math.sin(i * 0.8))
        draw.ellipse([px - 1, py - 1, px + 1, py + 1], fill=random.choice(colors) + (200,))

    draw_eye(draw, 48, 28, 3, (0, 200, 160))
    add_texture_noise(img, 0.2)
    img = draw_glow(img, 48, 28, 6, (0, 200, 160), 0.4)
    for i, px in enumerate(range(18, 48, 4)):
        py = 30 + int(2 * math.sin(i * 0.8))
        img = draw_glow(img, px, py, 3, (0, 180, 200), 0.2)
    img.save(f"{AMBIENT_DIR}/deep_fish.png")
    print("  deep_fish.png")


def generate_giant_squid():
    """96x64. Long tentacles, large eyes, dark reddish-purple. Eldritch."""
    img = Image.new("RGBA", (96, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Mantle
    draw.polygon([
        (50, 20), (55, 16), (68, 14), (78, 18), (82, 26), (82, 38),
        (78, 46), (68, 50), (55, 48), (50, 44),
    ], fill=(35, 12, 28, 240))

    for _ in range(8):
        mx, my = random.randint(54, 78), random.randint(18, 46)
        draw.ellipse([mx - 3, my - 2, mx + 3, my + 2], fill=(28, 8, 22, 200))

    # Fins
    draw.polygon([(78, 20), (90, 14), (88, 24)], fill=(30, 10, 25, 220))
    draw.polygon([(78, 44), (90, 50), (88, 40)], fill=(30, 10, 25, 220))

    # Large eyes
    for ey_center in [29, 40]:
        draw.ellipse([52, ey_center - 5, 60, ey_center + 3], fill=(8, 8, 12, 255))
        draw.ellipse([54, ey_center - 3, 58, ey_center + 1], fill=(0, 100, 80, 255))
        draw.ellipse([55, ey_center - 2, 57, ey_center], fill=(150, 255, 200, 255))

    # 8 tentacles
    for t in range(8):
        ty = 24 + t * 3
        pts = [(50, ty)]
        for seg in range(10):
            pts.append((50 - seg * 4 - random.randint(0, 2), ty + int(4 * math.sin(seg * 0.5 + t * 0.4))))
        for i in range(len(pts) - 1):
            a = max(30, 180 - i * 16)
            draw.line([pts[i], pts[i + 1]], fill=(40, 15, 32, a), width=max(1, 2 - i // 4))

    # 2 long feeding tentacles
    for to in [28, 38]:
        pts = [(50, to)]
        for seg in range(14):
            pts.append((50 - seg * 3 - random.randint(0, 3), to + int(6 * math.sin(seg * 0.3 + to * 0.1))))
        for i in range(len(pts) - 1):
            draw.line([pts[i], pts[i + 1]], fill=(40, 15, 32, max(20, 160 - i * 10)), width=1 if i < 10 else 2)

    add_texture_noise(img, 0.2)
    img = draw_glow(img, 56, 29, 6, (0, 150, 120), 0.4)
    img = draw_glow(img, 56, 40, 6, (0, 150, 120), 0.4)
    img.save(f"{AMBIENT_DIR}/giant_squid.png")
    print("  giant_squid.png")


def generate_whale():
    """128x64. Enormous dark silhouette. Serene but massive."""
    img = Image.new("RGBA", (128, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Streamlined body
    draw.polygon([
        (8, 34), (4, 28), (10, 22), (25, 16), (50, 12), (80, 10),
        (100, 14), (115, 22), (122, 30), (124, 34),
        (122, 38), (115, 44), (100, 50), (80, 54), (50, 52),
        (25, 48), (10, 42), (4, 38),
    ], fill=(18, 22, 28, 245))

    # Lighter underbelly
    draw.polygon([
        (15, 38), (30, 42), (50, 46), (80, 48), (100, 44), (115, 40),
        (122, 36), (124, 34), (122, 38), (115, 44), (100, 50), (80, 54),
        (50, 52), (25, 48), (10, 42),
    ], fill=(28, 32, 38, 240))

    # Ventral grooves
    for gx in range(30, 100, 6):
        gy = 40 + int(4 * math.sin((gx - 30) * 0.04))
        draw.line([(gx, gy), (gx + 2, gy + 8)], fill=(14, 18, 24, 200), width=1)

    # Tail fluke
    draw.polygon([(4, 28), (0, 16), (8, 24)], fill=(14, 18, 24, 240))
    draw.polygon([(4, 38), (0, 50), (8, 42)], fill=(14, 18, 24, 240))

    # Pectoral fin
    draw.polygon([(60, 48), (50, 58), (70, 56), (75, 50)], fill=(16, 20, 26, 230))

    # Dorsal ridge
    for dx in range(40, 90, 3):
        draw.point((dx, 12 + int(2 * abs(math.sin((dx - 40) * 0.05))) - 1), fill=(22, 26, 32, 180))

    # Small eye
    draw.ellipse([112, 26, 116, 30], fill=(40, 45, 55, 255))
    draw.point((114, 28), fill=(80, 90, 100, 255))

    # Baleen
    for by in range(24, 36, 2):
        draw.line([(120, by), (124, by + 1)], fill=(60, 55, 50, 150), width=1)

    # Scars
    for _ in range(4):
        sx, sy = random.randint(30, 100), random.randint(16, 44)
        draw.line([(sx, sy), (sx + random.randint(5, 15), sy + random.randint(-2, 2))], fill=(24, 28, 34, 120), width=1)

    add_texture_noise(img, 0.12)
    img.save(f"{AMBIENT_DIR}/whale.png")
    print("  whale.png")


# ============================================================================

if __name__ == "__main__":
    print("Generating Barotrauma-style creature sprites...\n")
    print("Hostile creatures:")
    generate_stalker()
    generate_scavenger()
    generate_ambusher()
    generate_electric_eel()
    generate_blind_hunter()
    generate_lure_fish()
    generate_swarm_queen()
    generate_leviathan()
    generate_parasite()
    generate_watcher()

    print("\nAmbient creatures:")
    generate_small_fish()
    generate_jellyfish()
    generate_school_fish()
    generate_deep_fish()
    generate_giant_squid()
    generate_whale()

    print("\nDone! All 16 sprites generated.")
