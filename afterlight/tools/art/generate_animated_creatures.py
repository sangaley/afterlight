#!/usr/bin/env python3
"""
Generate animated creature sprite sheets.
Each creature gets a horizontal strip: 4 swim frames + 2 attack frames = 6 frames.
Frames are laid out left-to-right in a single row.
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

SWIM_FRAMES = 4
ATTACK_FRAMES = 2
TOTAL_FRAMES = SWIM_FRAMES + ATTACK_FRAMES


def noise_color(base, variance=15):
    return tuple(max(0, min(255, c + random.randint(-variance, variance))) for c in base[:3]) + (base[3] if len(base) > 3 else 255,)


def draw_organic_blob(draw, cx, cy, rx, ry, base_color, segments=24):
    points = []
    for i in range(segments):
        angle = (2 * math.pi * i) / segments
        wobble = random.uniform(0.82, 1.18)
        x = cx + rx * wobble * math.cos(angle)
        y = cy + ry * wobble * math.sin(angle)
        points.append((x, y))
    draw.polygon(points, fill=base_color)


def draw_eye(draw, x, y, size, glow_color=(0, 255, 200)):
    draw.ellipse([x-size-1, y-size-1, x+size+1, y+size+1], fill=glow_color[:3] + (60,))
    draw.ellipse([x-size, y-size, x+size, y+size], fill=glow_color)
    if size >= 2:
        draw.ellipse([x-1, y-1, x+1, y+1], fill=(255, 255, 255, 230))


def draw_teeth(draw, x_start, x_end, y, down=True, count=6, color=(200, 200, 190, 220)):
    spacing = (x_end - x_start) / max(count, 1)
    for i in range(count):
        tx = x_start + i * spacing + spacing / 2
        th = random.randint(3, 6)
        if down:
            draw.polygon([(tx-1, y), (tx+1, y), (tx, y+th)], fill=color)
        else:
            draw.polygon([(tx-1, y), (tx+1, y), (tx, y-th)], fill=color)


def draw_glow(img, x, y, radius, color, intensity=0.7):
    for r in range(radius, 0, -1):
        alpha = int(255 * intensity * (r / radius) * 0.3)
        glow_color = color[:3] + (min(255, alpha),)
        overlay = Image.new("RGBA", img.size, (0, 0, 0, 0))
        od = ImageDraw.Draw(overlay)
        od.ellipse([x-r, y-r, x+r, y+r], fill=glow_color)
        img = Image.alpha_composite(img, overlay)
    return img


def add_texture_noise(img, intensity=0.15):
    pixels = img.load()
    w, h = img.size
    for x in range(w):
        for y in range(h):
            r, g, b, a = pixels[x, y]
            if a > 30:
                v = int(20 * intensity)
                pixels[x, y] = (max(0, min(255, r+random.randint(-v,v))),
                                max(0, min(255, g+random.randint(-v,v))),
                                max(0, min(255, b+random.randint(-v,v))), a)
    return img


# ============================================================================
# Per-creature frame generators. Each returns a single frame Image.
# frame_idx: 0-3 = swim, 4-5 = attack
# ============================================================================

def stalker_frame(fw, fh, frame_idx):
    """Sleek predator with cyan stripe."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    # Swim undulation
    phase = frame_idx * (math.pi / 2) if frame_idx < 4 else 0
    tail_y = int(4 * math.sin(phase))
    body_color = (18, 22, 32, 255)
    # Body
    body = [(8, cy+tail_y), (12, cy-8+tail_y//2), (20, cy-12), (35, cy-14),
            (48, cy-10), (56, cy-2), (58, cy), (56, cy+2),
            (48, cy+10), (35, cy+14), (20, cy+12), (12, cy+8+tail_y//2)]
    draw.polygon(body, fill=body_color)
    # Fins
    fin_flap = int(3 * math.sin(phase + 1))
    draw.polygon([(25, cy-12), (30, cy-22+fin_flap), (38, cy-14)], fill=(14, 18, 28, 240))
    draw.polygon([(28, cy+12), (32, cy+20-fin_flap), (38, cy+14)], fill=(14, 18, 28, 240))
    # Tail
    draw.polygon([(8, cy+tail_y), (2, cy-10+tail_y), (5, cy-2+tail_y)], fill=(14, 18, 28, 240))
    draw.polygon([(8, cy+tail_y), (2, cy+10+tail_y), (5, cy+2+tail_y)], fill=(14, 18, 28, 240))
    # Bioluminescent stripe
    for x in range(15, 52):
        i = int(80 + 40 * math.sin((x-15)*0.15 + phase*0.5))
        draw.point((x, cy-2), fill=(0, i, i+40, 180))
        draw.point((x, cy-1), fill=(0, i, i+40, 180))
    # Jaw - wider in attack frames
    jaw_open = 4 if frame_idx >= 4 else 1
    draw.polygon([(52, cy-4-jaw_open), (60, cy-1), (58, cy), (60, cy+1), (52, cy+4+jaw_open)],
                 fill=(25, 15, 15, 255))
    if frame_idx >= 4:
        draw_teeth(draw, 53, 60, cy-3, down=True, count=5, color=(180, 180, 170, 230))
        draw_teeth(draw, 53, 60, cy+3, down=False, count=4, color=(180, 180, 170, 230))
    else:
        draw_teeth(draw, 54, 60, cy-1, down=True, count=3, color=(180, 180, 170, 230))
    draw_eye(draw, 50, cy-4, 2, (0, 220, 180))
    add_texture_noise(img, 0.2)
    img = draw_glow(img, 50, cy-4, 6, (0, 220, 180), 0.4)
    return img


def scavenger_frame(fw, fh, frame_idx):
    """Crab/isopod with animated legs."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    base = (45, 35, 25, 255)
    draw_organic_blob(draw, 32, cy-2, 20, 14, base, segments=16)
    # Shell segments
    for sx in range(16, 48, 5):
        draw.line([(sx, cy-14), (sx+1, cy+10)], fill=(30, 22, 16, 255), width=1)
    # Animated legs
    leg_color = (55, 40, 30, 230)
    leg_positions = [(14, cy-4), (18, cy), (22, cy+3), (42, cy+3), (46, cy), (50, cy-4)]
    for i, (lx, ly) in enumerate(leg_positions):
        leg_phase = int(5 * math.sin(phase + i * 0.8))
        if i < 3:
            draw.line([(lx, ly), (lx-8, ly+14+leg_phase)], fill=leg_color, width=2)
            draw.line([(lx-8, ly+14+leg_phase), (lx-12, ly+18+leg_phase)], fill=leg_color, width=1)
        else:
            draw.line([(lx, ly), (lx+8, ly+14-leg_phase)], fill=leg_color, width=2)
            draw.line([(lx+8, ly+14-leg_phase), (lx+12, ly+18-leg_phase)], fill=leg_color, width=1)
    # Mandibles - snap in attack
    mand_open = 4 if frame_idx >= 4 else 0
    draw.polygon([(12, cy-4), (6-mand_open, cy-6-mand_open), (8, cy-2)], fill=(60, 45, 35, 240))
    draw.polygon([(12, cy), (6-mand_open, cy+2+mand_open), (8, cy-2)], fill=(60, 45, 35, 240))
    draw_eye(draw, 13, cy-6, 1, (200, 150, 50))
    draw_eye(draw, 13, cy+2, 1, (200, 150, 50))
    add_texture_noise(img, 0.25)
    img = draw_glow(img, 13, cy-6, 4, (200, 150, 50), 0.3)
    return img


def ambusher_frame(fw, fh, frame_idx):
    """Flat ambush predator - barely moves when swimming, lunges on attack."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    body_color = (22, 28, 20, 255)
    # Slight body sway
    sway = int(2 * math.sin(frame_idx * math.pi / 2)) if frame_idx < 4 else 0
    # Body stretches forward on attack
    stretch = 4 if frame_idx >= 4 else 0
    body = [(6, cy+sway), (10, cy-8+sway), (20, cy-12), (44, cy-12),
            (54+stretch, cy-8), (58+stretch, cy),
            (58+stretch, cy+4), (54+stretch, cy+8), (44, cy+12), (20, cy+12), (10, cy+8+sway), (6, cy+4+sway)]
    draw.polygon(body, fill=body_color)
    # Camo patches
    random.seed(42)
    for _ in range(12):
        px, py = random.randint(12, 52), random.randint(cy-10, cy+10)
        draw.ellipse([px-random.randint(2,5), py-random.randint(2,4),
                      px+random.randint(2,5), py+random.randint(2,4)], fill=(15, 20, 14, 200))
    # Mouth - huge gape on attack
    mouth_open = 6 if frame_idx >= 4 else 2
    draw.polygon([(4, cy-mouth_open), (2, cy), (4, cy+mouth_open), (10, cy+mouth_open-2), (10, cy-mouth_open+2)],
                 fill=(40, 10, 10, 240))
    if frame_idx >= 4:
        draw_teeth(draw, 3, 10, cy-mouth_open+1, down=True, count=5, color=(160, 155, 140, 220))
        draw_teeth(draw, 3, 10, cy+mouth_open-1, down=False, count=5, color=(160, 155, 140, 220))
    draw.point((16, cy-8), fill=(100, 180, 80, 180))
    # Pectoral flaps animate
    flap = int(3 * math.sin(frame_idx * math.pi / 2 + 0.5))
    draw.polygon([(20, cy+12), (15, cy+20+flap), (25, cy+16)], fill=(18, 24, 16, 220))
    draw.polygon([(44, cy+12), (49, cy+20-flap), (39, cy+16)], fill=(18, 24, 16, 220))
    add_texture_noise(img, 0.3)
    random.seed(42 + frame_idx)
    return img


def electric_eel_frame(fw, fh, frame_idx):
    """Serpentine with electric arcs - arcs intensify on attack."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    body_color = (16, 18, 30, 255)
    phase = frame_idx * (math.pi / 2)
    # Sinusoidal body
    pts_top, pts_bot = [], []
    for x in range(8, 58):
        yc = cy + int(8 * math.sin((x-8)*0.12 + phase*0.3))
        w = 5 + int(3 * math.sin((x-8)*0.06))
        if x < 15: w = max(2, int(w * (x-8)/7))
        if x > 50: w = max(1, int(w * (58-x)/8))
        pts_top.append((x, yc - w))
        pts_bot.append((x, yc + w))
    draw.polygon(pts_top + list(reversed(pts_bot)), fill=body_color)
    # Electric arcs - more intense on attack
    arc_intensity = 2.0 if frame_idx >= 4 else 1.0
    random.seed(42 + frame_idx)
    for x in range(10, 55, 2):
        yc = cy + int(8 * math.sin((x-8)*0.12 + phase*0.3))
        ai = int(random.randint(80, 200) * arc_intensity)
        yo = random.randint(-3, 3)
        draw.point((x, yc+yo), fill=(ai//3, ai//2, min(255, ai), 200))
        if random.random() < 0.3 * arc_intensity:
            for s in range(random.randint(2, int(5*arc_intensity))):
                draw.point((x+random.randint(-2,2), yc+yo+s*random.choice([-1,1])),
                           fill=(80, 150, min(255, ai), 150))
    # Nodes
    nodes = [(15, cy+int(8*math.sin(7*0.12+phase*0.3))),
             (30, cy+int(8*math.sin(22*0.12+phase*0.3))),
             (45, cy+int(8*math.sin(37*0.12+phase*0.3)))]
    glow_r = 7 if frame_idx >= 4 else 5
    for nx, ny in nodes:
        draw.ellipse([nx-2, ny-2, nx+2, ny+2], fill=(200, 180, 40, 220))
    # Head
    hx, hy = 56, cy+int(8*math.sin(48*0.12+phase*0.3))
    draw.ellipse([hx-4, hy-5, hx+4, hy+5], fill=(20, 22, 35, 255))
    draw_eye(draw, hx+1, hy-2, 1, (100, 200, 255))
    add_texture_noise(img, 0.15)
    for nx, ny in nodes:
        img = draw_glow(img, nx, ny, glow_r, (100, 180, 255), 0.3 * arc_intensity)
    return img


def blind_hunter_frame(fw, fh, frame_idx):
    """Massive eyeless predator - jaw opens wider on attack."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    body_color = (28, 20, 22, 255)
    # Body sways
    sway = int(2 * math.sin(phase))
    draw_organic_blob(draw, 28, cy+sway, 22, 18, body_color, segments=20)
    # Jaw - much wider on attack
    jaw_open = 8 if frame_idx >= 4 else 3
    jaw_color = (35, 24, 26, 255)
    draw.polygon([(48, cy-jaw_open-5+sway), (62, cy-2+sway), (60, cy+sway), (48, cy-2+sway)], fill=jaw_color)
    draw.polygon([(48, cy+jaw_open+5+sway), (62, cy+2+sway), (60, cy+sway), (48, cy+2+sway)], fill=jaw_color)
    draw_teeth(draw, 48, 62, cy-jaw_open-2+sway, down=True, count=7, color=(200, 190, 175, 240))
    draw_teeth(draw, 48, 62, cy+jaw_open+2+sway, down=False, count=7, color=(200, 190, 175, 240))
    # Mouth interior
    draw.polygon([(50, cy-2+sway), (58, cy-1+sway), (58, cy+1+sway), (50, cy+2+sway)], fill=(60, 10, 10, 200))
    # Echolocation ridges
    ridge_pulse = int(2 * math.sin(phase * 2)) if frame_idx < 4 else 0
    for i in range(4):
        rx = 40 + i * 3 + ridge_pulse
        draw.arc([rx-6, cy-10+sway, rx+6, cy+10+sway], 250, 290, fill=(50, 35, 38, 200), width=1)
    # Sensory pits
    draw.ellipse([46, cy-6+sway, 49, cy-3+sway], fill=(80, 20, 20, 160))
    draw.ellipse([46, cy+3+sway, 49, cy+6+sway], fill=(80, 20, 20, 160))
    # Tail
    draw.polygon([(6, cy+sway), (2, cy-10+sway), (8, cy-4+sway)], fill=(22, 16, 18, 240))
    draw.polygon([(6, cy+sway), (2, cy+10+sway), (8, cy+4+sway)], fill=(22, 16, 18, 240))
    add_texture_noise(img, 0.25)
    img = draw_glow(img, 47, cy-5+sway, 4, (120, 30, 30), 0.3)
    img = draw_glow(img, 47, cy+5+sway, 4, (120, 30, 30), 0.3)
    return img


def lure_fish_frame(fw, fh, frame_idx):
    """Anglerfish - lure bobs, jaw opens on attack."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2 + 2
    phase = frame_idx * (math.pi / 2)
    body_color = (20, 16, 24, 255)
    draw_organic_blob(draw, 30, cy, 18, 16, body_color, segments=18)
    # Jaw - wider on attack
    jaw_open = 10 if frame_idx >= 4 else 4
    draw.polygon([(46, cy-jaw_open), (62, cy-2), (62, cy+2), (46, cy+jaw_open)], fill=(26, 18, 28, 255))
    draw.polygon([(50, cy-jaw_open+4), (60, cy-1), (60, cy+1), (50, cy+jaw_open-4)], fill=(50, 8, 12, 240))
    # Teeth
    tc = 8 if frame_idx >= 4 else 5
    for i in range(tc):
        tx = 50 + i * (10 // tc)
        tl = random.randint(3, 7)
        draw.line([(tx, cy-jaw_open+3), (tx, cy-jaw_open+3+tl)], fill=(200, 195, 180, 230), width=1)
        draw.line([(tx, cy+jaw_open-3), (tx, cy+jaw_open-3-tl)], fill=(200, 195, 180, 230), width=1)
    # Lure - bobs with animation
    lure_bob = int(3 * math.sin(phase))
    lure_glow = 0.8 if frame_idx >= 4 else 0.6  # brighter during attack (luring)
    lure = [(35, cy-14), (32, cy-20+lure_bob), (28, cy-24+lure_bob), (26, cy-26+lure_bob)]
    for i in range(len(lure)-1):
        draw.line([lure[i], lure[i+1]], fill=(40, 30, 45, 200), width=1)
    draw.ellipse([23, cy-29+lure_bob, 29, cy-23+lure_bob], fill=(80, 200, 255, 240))
    draw.ellipse([24, cy-28+lure_bob, 28, cy-24+lure_bob], fill=(150, 240, 255, 255))
    draw_eye(draw, 44, cy-4, 2, (180, 40, 40))
    draw.polygon([(14, cy-2), (8, cy-8), (10, cy)], fill=(18, 14, 22, 220))
    add_texture_noise(img, 0.2)
    img = draw_glow(img, 26, cy-26+lure_bob, 10, (80, 200, 255), lure_glow)
    return img


def swarm_queen_frame(fw, fh, frame_idx):
    """Bloated organic mass - pustules pulse."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    # Pulsating body
    pulse = int(2 * math.sin(phase))
    draw_organic_blob(draw, 32, cy, 22+pulse, 22+pulse, (30, 15, 35, 255), segments=24)
    draw_organic_blob(draw, 32, cy, 16, 16, (22, 10, 28, 240), segments=16)
    # Pustules - throb
    nodes = [(18, cy-10), (44, cy-12), (14, cy+6), (48, cy+8), (32, cy-18), (32, cy+18)]
    for i, (nx, ny) in enumerate(nodes):
        p_size = 3 + int(1.5 * math.sin(phase + i * 0.7))
        glow_i = 0.5 if frame_idx >= 4 else 0.4
        draw.ellipse([nx-p_size, ny-p_size, nx+p_size, ny+p_size], fill=(50, 120, 40, 220))
        draw.ellipse([nx-1, ny-1, nx+1, ny+1], fill=(80, 200, 60, 255))
    # Tendrils - wave
    for angle_deg in range(0, 360, 45):
        angle = math.radians(angle_deg + random.randint(-10, 10))
        length = random.randint(6, 12) + (3 if frame_idx >= 4 else 0)
        sx = 32 + int((20+pulse) * math.cos(angle))
        sy = cy + int((20+pulse) * math.sin(angle))
        wave = int(3 * math.sin(phase + angle))
        ex = 32 + int((20+pulse+length) * math.cos(angle)) + wave
        ey = cy + int((20+pulse+length) * math.sin(angle))
        draw.line([(sx, sy), (ex, ey)], fill=(35, 18, 40, 180), width=2)
    # Central orifice
    orifice_size = 5 if frame_idx >= 4 else 4
    draw.ellipse([32-orifice_size, cy-orifice_size, 32+orifice_size, cy+orifice_size], fill=(60, 20, 50, 230))
    draw.ellipse([30, cy-2, 34, cy+2], fill=(120, 40, 80, 255))
    add_texture_noise(img, 0.2)
    for nx, ny in nodes:
        img = draw_glow(img, nx, ny, 5, (80, 200, 60), 0.4)
    return img


def leviathan_frame(fw, fh, frame_idx):
    """Massive armored sea monster. 128x64."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    body_color = (14, 12, 18, 255)
    # Segmented body with undulation
    for seg in range(8):
        cx = 95 - seg * 12
        seg_sway = int(3 * math.sin(phase + seg * 0.4))
        ry = 22 - seg * 1.5
        rx = 14 - seg * 0.5
        draw_organic_blob(draw, cx, cy+seg_sway, rx, ry, noise_color(body_color, 5), segments=12)
    # Armored head
    draw.polygon([(98, cy-16), (124, cy-6), (126, cy), (124, cy+6), (98, cy+16)], fill=(18, 14, 22, 255))
    draw.polygon([(100, cy-14), (110, cy-16), (115, cy-8), (105, cy-6)], fill=(28, 22, 32, 220))
    draw.polygon([(100, cy+14), (110, cy+16), (115, cy+8), (105, cy+6)], fill=(28, 22, 32, 220))
    # Eyes - glow brighter on attack
    eye_glow = 0.6 if frame_idx >= 4 else 0.4
    for i, (ex, ey_top) in enumerate([(108, cy-8), (114, cy-4), (118, cy-1)]):
        s = max(1, 2 - (i // 2))
        draw_eye(draw, ex, ey_top, s, (180, 0, 0))
        draw_eye(draw, ex, fh - ey_top, s, (180, 0, 0))
    # Jaw - opens wide on attack
    jaw_open = 6 if frame_idx >= 4 else 2
    draw.polygon([(122, cy-jaw_open-2), (128, cy-1), (128, cy+1), (122, cy+jaw_open+2)], fill=(40, 12, 14, 255))
    draw_teeth(draw, 120, 128, cy-jaw_open-1, down=True, count=5, color=(180, 175, 160, 240))
    draw_teeth(draw, 120, 128, cy+jaw_open+1, down=False, count=5, color=(180, 175, 160, 240))
    # Tail with sway
    tail_sway = int(5 * math.sin(phase))
    draw.polygon([(4, cy+tail_sway), (0, cy-14+tail_sway), (10, cy-4+tail_sway)], fill=(12, 10, 16, 230))
    draw.polygon([(4, cy+tail_sway), (0, cy+14+tail_sway), (10, cy+4+tail_sway)], fill=(12, 10, 16, 230))
    # Dorsal spines
    for sx in range(20, 95, 10):
        sh = random.randint(4, 8)
        seg_idx = (95 - sx) // 12
        seg_sway = int(3 * math.sin(phase + seg_idx * 0.4))
        sw = 22 - abs(sx - 55) * 0.15
        by = cy - int(sw) + 2 + seg_sway
        draw.polygon([(sx, by), (sx+2, by-sh), (sx+4, by)], fill=(24, 18, 30, 220))
    add_texture_noise(img, 0.2)
    for ex, ey_top in [(108, cy-8), (114, cy-4), (118, cy-1)]:
        img = draw_glow(img, ex, ey_top, 5, (180, 0, 0), eye_glow)
        img = draw_glow(img, ex, fh-ey_top, 5, (180, 0, 0), eye_glow)
    return img


def parasite_frame(fw, fh, frame_idx):
    """Small translucent insectoid - hooks twitch."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    draw_organic_blob(draw, 32, cy, 10, 7, (40, 30, 25, 160), segments=12)
    draw.ellipse([28, cy-2, 34, cy+3], fill=(80, 30, 30, 120))
    # Hooks - twitch
    hc = (70, 50, 40, 220)
    hook_twitch = int(2 * math.sin(phase))
    for x1, y1, x2, y2, x3, y3 in [
        (42, cy-4, 50+hook_twitch, cy-8, 48+hook_twitch, cy-10),
        (42, cy+4, 50+hook_twitch, cy+8, 48+hook_twitch, cy+10),
        (22, cy-4, 16-hook_twitch, cy-8, 18-hook_twitch, cy-10),
        (22, cy+4, 16-hook_twitch, cy+8, 18-hook_twitch, cy+10),
    ]:
        draw.line([(x1, y1), (x2, y2)], fill=hc, width=1)
        draw.line([(x2, y2), (x3, y3)], fill=hc, width=1)
    # Proboscis - extends on attack
    prob_len = 14 if frame_idx >= 4 else 10
    draw.line([(42, cy), (42+prob_len, cy)], fill=(60, 20, 20, 200), width=1)
    draw.point((42+prob_len, cy), fill=(100, 30, 30, 230))
    draw.point((40, cy-2), fill=(200, 200, 100, 200))
    draw.point((40, cy+2), fill=(200, 200, 100, 200))
    add_texture_noise(img, 0.15)
    return img


def watcher_frame(fw, fh, frame_idx):
    """Giant eye with trailing tentacles - pupil shifts, tentacles wave."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    # Eye structure
    draw.ellipse([16, cy-16, 48, cy+16], fill=(20, 25, 40, 240))
    draw.ellipse([22, cy-10, 42, cy+10], fill=(10, 30, 50, 255))
    # Pupil shifts with phase
    px_shift = int(2 * math.sin(phase))
    py_shift = int(1 * math.cos(phase))
    draw.ellipse([28+px_shift, cy-4+py_shift, 36+px_shift, cy+4+py_shift], fill=(0, 150, 200, 255))
    draw.ellipse([30+px_shift, cy-2+py_shift, 34+px_shift, cy+2+py_shift], fill=(100, 220, 255, 255))
    draw.ellipse([31+px_shift, cy-1+py_shift, 33+px_shift, cy+1+py_shift], fill=(200, 255, 255, 255))
    # Membrane
    draw.arc([14, cy-18, 50, cy+18], 0, 360, fill=(25, 18, 30, 200), width=2)
    # Tentacles wave
    for t in range(5):
        tx = 22 + t * 5
        pts = [(tx, cy+14)]
        for seg in range(6):
            wave = int(3 * math.sin(seg * 0.8 + t + phase * 0.5))
            pts.append((tx + wave, cy + 16 + seg * 3))
        for i in range(len(pts)-1):
            alpha = max(40, 160 - i * 20)
            draw.line([pts[i], pts[i+1]], fill=(22, 20, 35, alpha), width=1)
    # Alert during attack - iris constricts
    glow_i = 0.7 if frame_idx >= 4 else 0.5
    add_texture_noise(img, 0.15)
    img = draw_glow(img, 32, cy, 14, (0, 150, 200), glow_i)
    return img


# ============================================================================
# AMBIENT CREATURES (simpler, 4 swim frames only, no attack)
# ============================================================================

def small_fish_frame(fw, fh, frame_idx):
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    tail_wag = int(2 * math.sin(phase))
    draw.polygon([(6, cy+tail_wag), (10, cy-4), (22, cy-5), (26, cy-2), (26, cy+2), (22, cy+5), (10, cy+4)],
                 fill=(140, 150, 160, 200))
    draw.polygon([(6, cy+tail_wag), (2, cy-4+tail_wag), (4, cy+tail_wag), (2, cy+4+tail_wag)],
                 fill=(120, 130, 140, 180))
    draw.point((23, cy-2), fill=(20, 20, 20, 255))
    add_texture_noise(img, 0.1)
    return img

def jellyfish_frame(fw, fh, frame_idx):
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    phase = frame_idx * (math.pi / 2)
    # Bell pulses
    pulse = int(2 * math.sin(phase))
    draw.ellipse([16-pulse, 8-pulse, 48+pulse, 34+pulse], fill=(120, 100, 160, 70))
    draw.ellipse([20, 12, 44, 30], fill=(140, 120, 180, 50))
    # Tentacles wave
    for t in range(6):
        tx = 18 + t * 5
        for seg in range(10):
            sy = 34 + pulse + seg * 3
            sx = tx + int(3 * math.sin(seg * 0.4 + t * 0.7 + phase * 0.5))
            draw.point((sx, sy), fill=(160, 140, 200, max(10, 60 - seg * 5)))
    add_texture_noise(img, 0.08)
    img = draw_glow(img, 32, 20, 12, (140, 120, 200), 0.25)
    return img

def school_fish_frame(fw, fh, frame_idx):
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    phase = frame_idx * (math.pi / 2)
    positions = [(12,18),(24,14),(38,12),(50,16),(8,30),(20,28),(34,26),(48,30),(14,42),(28,38),(42,40),(54,44)]
    for j, (fx, fy) in enumerate(positions):
        # Each fish has its own phase offset
        fish_sway = int(2 * math.sin(phase + j * 0.5))
        br = random.randint(100, 150)
        color = (br-20, br, br+10, random.randint(150, 220))
        s = random.uniform(0.7, 1.3)
        draw.polygon([
            (fx-4*s, fy+fish_sway), (fx-2*s, fy-2*s+fish_sway),
            (fx+4*s, fy-s+fish_sway), (fx+5*s, fy+fish_sway),
            (fx+4*s, fy+s+fish_sway), (fx-2*s, fy+2*s+fish_sway),
        ], fill=color)
        draw.polygon([
            (fx-4*s, fy+fish_sway), (fx-7*s, fy-2*s+fish_sway),
            (fx-5*s, fy+fish_sway), (fx-7*s, fy+2*s+fish_sway),
        ], fill=(color[0]-20, color[1]-20, color[2]-20, color[3]-40))
    return img

def deep_fish_frame(fw, fh, frame_idx):
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    sway = int(2 * math.sin(phase))
    draw.polygon([(10, cy+sway), (14, cy-8), (24, cy-12), (40, cy-14), (50, cy-8), (54, cy),
                  (50, cy+8), (40, cy+14), (24, cy+12), (14, cy+8)], fill=(12, 14, 22, 255))
    # Fins flap
    fin_f = int(3 * math.sin(phase + 1))
    draw.polygon([(22, cy-12), (26, cy-20+fin_f), (32, cy-14)], fill=(10, 12, 20, 230))
    draw.polygon([(24, cy+12), (28, cy+20-fin_f), (34, cy+14)], fill=(10, 12, 20, 230))
    # Tail
    draw.polygon([(10, cy+sway), (4, cy-6+sway), (8, cy-2+sway)], fill=(10, 12, 20, 230))
    draw.polygon([(10, cy+sway), (4, cy+6+sway), (8, cy+2+sway)], fill=(10, 12, 20, 230))
    # Photophores
    for i, px in enumerate(range(18, 48, 4)):
        py = cy-2 + int(2 * math.sin(i * 0.8))
        blink = 200 if (frame_idx + i) % 3 != 0 else 100
        draw.ellipse([px-1, py-1, px+1, py+1], fill=(0, 180, 200, blink))
    draw_eye(draw, 48, cy-4, 3, (0, 200, 160))
    add_texture_noise(img, 0.2)
    img = draw_glow(img, 48, cy-4, 6, (0, 200, 160), 0.4)
    return img

def giant_squid_frame(fw, fh, frame_idx):
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2
    phase = frame_idx * (math.pi / 2)
    # Mantle
    draw.polygon([(50, cy-12), (55, cy-16), (68, cy-18), (78, cy-14), (82, cy-6), (82, cy+6),
                  (78, cy+14), (68, cy+18), (55, cy+16), (50, cy+12)], fill=(35, 12, 28, 240))
    # Fins pulse
    fin_p = int(3 * math.sin(phase))
    draw.polygon([(78, cy-12), (90, cy-18+fin_p), (88, cy-8)], fill=(30, 10, 25, 220))
    draw.polygon([(78, cy+12), (90, cy+18-fin_p), (88, cy+8)], fill=(30, 10, 25, 220))
    # Eyes
    for ey in [cy-3, cy+8]:
        draw.ellipse([52, ey-5, 60, ey+3], fill=(8, 8, 12, 255))
        draw.ellipse([54, ey-3, 58, ey+1], fill=(0, 100, 80, 255))
    # Tentacles wave
    random.seed(42 + frame_idx)
    for t in range(8):
        ty = cy - 8 + t * 3
        pts = [(50, ty)]
        for seg in range(10):
            wave = int(4 * math.sin(seg * 0.5 + t * 0.4 + phase * 0.3))
            pts.append((50 - seg*4 - random.randint(0,2), ty + wave))
        for i in range(len(pts)-1):
            a = max(30, 180 - i * 16)
            draw.line([pts[i], pts[i+1]], fill=(40, 15, 32, a), width=max(1, 2-i//4))
    add_texture_noise(img, 0.2)
    img = draw_glow(img, 56, cy-3, 6, (0, 150, 120), 0.4)
    img = draw_glow(img, 56, cy+8, 6, (0, 150, 120), 0.4)
    return img

def whale_frame(fw, fh, frame_idx):
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cy = fh // 2 + 2
    phase = frame_idx * (math.pi / 2)
    tail_sway = int(4 * math.sin(phase))
    # Body
    draw.polygon([
        (8, cy+2+tail_sway), (4, cy-6+tail_sway//2), (10, cy-12), (25, cy-18), (50, cy-22), (80, cy-24),
        (100, cy-20), (115, cy-12), (122, cy-4), (124, cy),
        (122, cy+4), (115, cy+10), (100, cy+16), (80, cy+20), (50, cy+18),
        (25, cy+14), (10, cy+8+tail_sway//2),
    ], fill=(18, 22, 28, 245))
    # Underbelly
    draw.polygon([
        (15, cy+4), (30, cy+8), (50, cy+12), (80, cy+14), (100, cy+10), (115, cy+6),
        (122, cy+2), (124, cy), (122, cy+4), (115, cy+10), (100, cy+16), (80, cy+20),
        (50, cy+18), (25, cy+14), (10, cy+8+tail_sway//2),
    ], fill=(28, 32, 38, 240))
    # Tail fluke
    draw.polygon([(4, cy-6+tail_sway), (0, cy-18+tail_sway), (8, cy-8+tail_sway)], fill=(14, 18, 24, 240))
    draw.polygon([(4, cy+8+tail_sway), (0, cy+16+tail_sway), (8, cy+8+tail_sway)], fill=(14, 18, 24, 240))
    # Pectoral fin
    fin_f = int(3 * math.sin(phase + 1))
    draw.polygon([(60, cy+14), (50, cy+24+fin_f), (70, cy+22), (75, cy+16)], fill=(16, 20, 26, 230))
    # Eye
    draw.ellipse([112, cy-8, 116, cy-4], fill=(40, 45, 55, 255))
    draw.point((114, cy-6), fill=(80, 90, 100, 255))
    add_texture_noise(img, 0.12)
    return img


# ============================================================================
# ASSEMBLE SPRITE SHEETS
# ============================================================================

def make_sprite_sheet(frame_func, fw, fh, num_frames, filename, directory):
    """Generate a horizontal sprite sheet."""
    sheet = Image.new("RGBA", (fw * num_frames, fh), (0, 0, 0, 0))
    for i in range(num_frames):
        random.seed(42)  # Reset seed per frame for consistent base shapes
        frame = frame_func(fw, fh, i)
        sheet.paste(frame, (i * fw, 0))
    sheet.save(f"{directory}/{filename}")
    print(f"  {filename} ({num_frames} frames, {fw}x{fh} each)")


if __name__ == "__main__":
    print("Generating animated creature sprite sheets...\n")

    print("Hostile creatures (6 frames each: 4 swim + 2 attack):")
    make_sprite_sheet(stalker_frame,      64, 64, 6, "stalker.png",      HOSTILE_DIR)
    make_sprite_sheet(scavenger_frame,    64, 64, 6, "scavenger.png",    HOSTILE_DIR)
    make_sprite_sheet(ambusher_frame,     64, 64, 6, "ambusher.png",     HOSTILE_DIR)
    make_sprite_sheet(electric_eel_frame, 64, 64, 6, "electric_eel.png", HOSTILE_DIR)
    make_sprite_sheet(blind_hunter_frame, 64, 64, 6, "blind_hunter.png", HOSTILE_DIR)
    make_sprite_sheet(lure_fish_frame,    64, 64, 6, "lure_fish.png",    HOSTILE_DIR)
    make_sprite_sheet(swarm_queen_frame,  64, 64, 6, "swarm_queen.png",  HOSTILE_DIR)
    make_sprite_sheet(leviathan_frame,   128, 64, 6, "leviathan.png",    HOSTILE_DIR)
    make_sprite_sheet(parasite_frame,     64, 64, 6, "parasite.png",     HOSTILE_DIR)
    make_sprite_sheet(watcher_frame,      64, 64, 6, "watcher.png",      HOSTILE_DIR)

    print("\nAmbient creatures (4 swim frames):")
    make_sprite_sheet(small_fish_frame,   32, 32, 4, "small_fish.png",   AMBIENT_DIR)
    make_sprite_sheet(jellyfish_frame,    64, 64, 4, "jellyfish.png",    AMBIENT_DIR)
    make_sprite_sheet(school_fish_frame,  64, 64, 4, "school_fish.png",  AMBIENT_DIR)
    make_sprite_sheet(deep_fish_frame,    64, 64, 4, "deep_fish.png",    AMBIENT_DIR)
    make_sprite_sheet(giant_squid_frame,  96, 64, 4, "giant_squid.png",  AMBIENT_DIR)
    make_sprite_sheet(whale_frame,       128, 64, 4, "whale.png",        AMBIENT_DIR)

    print("\nDone! All animated sprite sheets generated.")
