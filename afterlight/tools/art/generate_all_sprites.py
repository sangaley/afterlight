#!/usr/bin/env python3
"""
Generate ALL game sprites in Barotrauma dark industrial/deep-sea style.
Part 1: Module sprites (25 unique), Hull sprites (4), Effect sprites (6)
"""

from PIL import Image, ImageDraw, ImageFilter
import math
import random
import os

random.seed(42)

MODULE_DIR = "assets/sprites/modules"
HULL_DIR = "assets/sprites/hull"
EFFECT_DIR = "assets/sprites/effects"
ENV_DIR = "assets/sprites/environment"

for d in [MODULE_DIR, HULL_DIR, EFFECT_DIR, ENV_DIR]:
    os.makedirs(d, exist_ok=True)

# Size: 66x66 (matches grid cell size)
CELL = 66


def noise_fill(draw, bbox, base_color, variance=8):
    """Fill a rectangle with noisy color for industrial texture."""
    x1, y1, x2, y2 = bbox
    for x in range(x1, x2):
        for y in range(y1, y2):
            c = tuple(max(0, min(255, ch + random.randint(-variance, variance))) for ch in base_color[:3])
            a = base_color[3] if len(base_color) > 3 else 255
            draw.point((x, y), fill=c + (a,))


def draw_rivets(draw, positions, color=(80, 85, 90, 255)):
    """Draw industrial rivets."""
    for x, y in positions:
        draw.ellipse([x-1, y-1, x+1, y+1], fill=color)
        draw.point((x, y-1), fill=(100, 105, 110, 200))


def draw_panel(draw, bbox, base_color, border_color=None):
    """Draw an industrial panel with border."""
    if border_color is None:
        border_color = tuple(max(0, c - 20) for c in base_color[:3]) + (255,)
    draw.rectangle(bbox, fill=base_color)
    draw.rectangle(bbox, outline=border_color, width=1)


def add_texture(img, intensity=0.1):
    """Add subtle noise to non-transparent pixels."""
    pixels = img.load()
    w, h = img.size
    for x in range(w):
        for y in range(h):
            r, g, b, a = pixels[x, y]
            if a > 30:
                v = int(15 * intensity)
                pixels[x, y] = (
                    max(0, min(255, r + random.randint(-v, v))),
                    max(0, min(255, g + random.randint(-v, v))),
                    max(0, min(255, b + random.randint(-v, v))), a)
    return img


def draw_glow_circle(draw, cx, cy, r, color, alpha=100):
    """Simple glow circle."""
    for i in range(r, 0, -1):
        a = int(alpha * (i / r) * 0.4)
        draw.ellipse([cx-i, cy-i, cx+i, cy+i], fill=color[:3] + (min(255, a),))


# ============================================================================
# MODULE SPRITES - Dark industrial Barotrauma style
# ============================================================================

def gen_small_reactor():
    """Small reactor / RTG - glowing core in metal housing."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Metal housing
    draw_panel(draw, [4, 4, 62, 62], (35, 38, 42, 255), (25, 28, 32, 255))
    draw_panel(draw, [8, 8, 58, 58], (40, 43, 48, 255))
    # Reactor core
    draw.ellipse([20, 20, 46, 46], fill=(30, 32, 36, 255), outline=(50, 55, 60, 255))
    draw.ellipse([24, 24, 42, 42], fill=(20, 60, 40, 255))
    draw.ellipse([28, 28, 38, 38], fill=(40, 180, 80, 220))
    draw.ellipse([31, 31, 35, 35], fill=(120, 255, 150, 255))
    # Cooling pipes
    for y in [14, 52]:
        draw.line([(12, y), (54, y)], fill=(45, 48, 52, 255), width=2)
    # Rivets
    draw_rivets(draw, [(8, 8), (58, 8), (8, 58), (58, 58), (33, 6), (33, 60)])
    draw_glow_circle(draw, 33, 33, 16, (40, 200, 80), 60)
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/small_reactor.png")

def gen_large_reactor():
    """Large reactor - 2x1 cell (132x66), bigger core, more pipes."""
    w = CELL * 2
    img = Image.new("RGBA", (w, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [2, 2, w-3, 63], (32, 35, 40, 255), (22, 25, 30, 255))
    draw_panel(draw, [6, 6, w-7, 59], (38, 42, 46, 255))
    # Twin cores
    for cx in [44, 88]:
        draw.ellipse([cx-14, 19, cx+14, 47], fill=(28, 30, 34, 255), outline=(48, 52, 56, 255))
        draw.ellipse([cx-10, 23, cx+10, 43], fill=(18, 55, 38, 255))
        draw.ellipse([cx-6, 27, cx+6, 39], fill=(35, 170, 75, 220))
        draw.ellipse([cx-3, 30, cx+3, 36], fill=(110, 245, 140, 255))
    # Connecting pipes
    draw.line([(58, 33), (74, 33)], fill=(50, 55, 60, 255), width=3)
    draw.line([(58, 28), (74, 28)], fill=(45, 48, 52, 255), width=2)
    draw.line([(58, 38), (74, 38)], fill=(45, 48, 52, 255), width=2)
    # Cooling
    for y in [12, 54]:
        draw.line([(10, y), (w-11, y)], fill=(44, 47, 52, 255), width=2)
    draw_rivets(draw, [(6, 6), (w-7, 6), (6, 59), (w-7, 59), (66, 6), (66, 59)])
    draw_glow_circle(draw, 44, 33, 14, (40, 200, 80), 50)
    draw_glow_circle(draw, 88, 33, 14, (40, 200, 80), 50)
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/large_reactor.png")

def gen_battery():
    """Battery bank - stacked cells with charge indicators."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (36, 38, 44, 255))
    # Battery cells
    for i in range(4):
        y = 10 + i * 13
        draw_panel(draw, [10, y, 56, y + 10], (28, 30, 35, 255), (45, 48, 52, 255))
        # Charge level bar
        fill_w = random.randint(20, 40)
        charge_color = (40, 160, 200, 200) if fill_w > 25 else (200, 160, 40, 200)
        draw.rectangle([12, y+2, 12+fill_w, y+8], fill=charge_color)
        # Terminal
        draw.rectangle([52, y+3, 55, y+7], fill=(180, 160, 40, 255))
    draw_rivets(draw, [(6, 6), (60, 6), (6, 60), (60, 60)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/battery.png")

def gen_standard_engine():
    """Engine/thruster - industrial propulsion unit, drawn facing right."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Main body
    draw_panel(draw, [4, 14, 44, 52], (34, 37, 42, 255), (24, 27, 32, 255))
    # Exhaust nozzle (right side)
    draw.polygon([(44, 10), (62, 4), (62, 62), (44, 56)], fill=(28, 30, 34, 255))
    draw.polygon([(48, 14), (60, 8), (60, 58), (48, 52)], fill=(22, 24, 28, 255))
    # Exhaust glow
    draw.polygon([(54, 18), (62, 14), (62, 52), (54, 48)], fill=(40, 80, 140, 100))
    # Turbine lines
    for y in range(18, 50, 4):
        draw.line([(8, y), (40, y)], fill=(42, 45, 50, 200), width=1)
    # Shaft
    draw.rectangle([38, 28, 50, 38], fill=(50, 54, 58, 255))
    draw_rivets(draw, [(6, 16), (6, 50), (42, 16), (42, 50)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/standard_engine.png")

def gen_silent_drive():
    """Silent drive - sleek, dampened, darker than regular engine."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 14, 44, 52], (22, 24, 30, 255), (18, 20, 24, 255))
    # Dampening layers
    for i in range(3):
        x = 8 + i * 12
        draw.rectangle([x, 18, x+8, 48], fill=(26, 28, 34, 255), outline=(20, 22, 26, 255))
    # Exhaust (muted)
    draw.polygon([(44, 14), (58, 10), (58, 56), (44, 52)], fill=(20, 22, 26, 255))
    draw.polygon([(48, 20), (56, 16), (56, 50), (48, 46)], fill=(16, 18, 22, 255))
    # Subtle blue hum
    draw.ellipse([46, 28, 56, 38], fill=(20, 40, 60, 80))
    draw_glow_circle(draw, 51, 33, 8, (30, 60, 100), 30)
    add_texture(img, 0.1)
    img.save(f"{MODULE_DIR}/silent_drive.png")

def gen_oxygen_scrubber():
    """Oxygen scrubber - pipes and filter housing."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (36, 40, 44, 255))
    # Filter cylinder
    draw.ellipse([14, 14, 52, 52], fill=(32, 36, 40, 255), outline=(48, 52, 56, 255))
    draw.ellipse([18, 18, 48, 48], fill=(30, 50, 45, 255))
    # Air flow arrows
    for y in [26, 33, 40]:
        draw.polygon([(22, y), (28, y-3), (28, y+3)], fill=(60, 120, 100, 180))
        draw.line([(28, y), (44, y)], fill=(60, 120, 100, 150), width=1)
    # Intake/exhaust pipes
    draw.rectangle([4, 28, 14, 38], fill=(42, 46, 50, 255))
    draw.rectangle([52, 28, 62, 38], fill=(42, 46, 50, 255))
    draw_rivets(draw, [(6, 6), (60, 6), (6, 60), (60, 60)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/oxygen_scrubber.png")

def gen_life_support():
    """CO2 scrubber / water recycler - dual filter system."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (34, 38, 42, 255))
    # Twin cylinders
    for cx in [25, 41]:
        draw.ellipse([cx-8, 12, cx+8, 54], fill=(30, 34, 38, 255), outline=(44, 48, 52, 255))
        draw.ellipse([cx-5, 16, cx+5, 50], fill=(28, 45, 55, 255))
    # Connecting pipe
    draw.rectangle([28, 30, 38, 36], fill=(40, 44, 48, 255))
    # Status light
    draw.ellipse([30, 8, 36, 14], fill=(40, 180, 80, 220))
    draw_rivets(draw, [(6, 6), (60, 6), (6, 60), (60, 60)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/life_support.png")

def gen_navigation():
    """Navigation console / helm - screens and controls."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (30, 33, 38, 255))
    # Main screen
    draw.rectangle([10, 8, 56, 38], fill=(8, 12, 18, 255), outline=(45, 50, 55, 255))
    # Screen content - sonar-like display
    draw.ellipse([18, 12, 48, 34], outline=(0, 80, 60, 150), width=1)
    draw.ellipse([26, 17, 40, 29], outline=(0, 60, 50, 120), width=1)
    draw.line([(33, 12), (33, 34)], fill=(0, 70, 55, 100), width=1)
    draw.line([(18, 23), (48, 23)], fill=(0, 70, 55, 100), width=1)
    # Blip
    draw.ellipse([36, 18, 39, 21], fill=(0, 200, 150, 200))
    # Control panel
    draw.rectangle([10, 42, 56, 58], fill=(34, 37, 42, 255), outline=(40, 44, 48, 255))
    # Buttons
    for bx in range(14, 54, 8):
        color = random.choice([(60, 160, 80, 200), (160, 60, 60, 200), (60, 100, 160, 200)])
        draw.ellipse([bx, 46, bx+4, 50], fill=color)
    # Throttle
    draw.rectangle([48, 44, 54, 56], fill=(50, 54, 58, 255), outline=(60, 64, 68, 255))
    add_texture(img, 0.12)
    img.save(f"{MODULE_DIR}/navigation.png")

def gen_torpedo_tube():
    """Torpedo launcher - cylindrical tubes, drawn facing right."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 8, 62, 58], (32, 35, 40, 255))
    # Twin tubes
    for cy in [24, 42]:
        draw.rectangle([8, cy-6, 58, cy+6], fill=(26, 28, 32, 255), outline=(42, 46, 50, 255))
        draw.rectangle([10, cy-4, 56, cy+4], fill=(22, 24, 28, 255))
        # Torpedo inside
        draw.ellipse([14, cy-3, 50, cy+3], fill=(50, 40, 35, 200))
        draw.polygon([(48, cy), (54, cy-3), (54, cy+3)], fill=(60, 50, 45, 220))
        # Muzzle
        draw.rectangle([54, cy-5, 60, cy+5], fill=(36, 38, 42, 255))
    # Loading mechanism
    draw.rectangle([8, 30, 20, 36], fill=(40, 44, 48, 255))
    draw_rivets(draw, [(6, 10), (60, 10), (6, 56), (60, 56)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/torpedo_tube.png")

def gen_point_defense():
    """Point defense turret - small auto-cannon."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (34, 37, 42, 255))
    # Turret base
    draw.ellipse([16, 16, 50, 50], fill=(30, 33, 38, 255), outline=(44, 48, 52, 255))
    draw.ellipse([20, 20, 46, 46], fill=(36, 40, 44, 255))
    # Gun barrel (pointing up)
    draw.rectangle([30, 4, 36, 26], fill=(40, 44, 48, 255), outline=(50, 54, 58, 255))
    draw.rectangle([31, 4, 35, 10], fill=(50, 54, 58, 255))
    # Rotation ring
    draw.arc([18, 18, 48, 48], 0, 360, fill=(50, 54, 58, 200), width=2)
    # Ammo indicator
    draw.rectangle([22, 50, 44, 56], fill=(28, 30, 34, 255))
    for ax in range(24, 42, 4):
        draw.rectangle([ax, 51, ax+2, 55], fill=(180, 140, 40, 200))
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/point_defense.png")

def gen_railgun():
    """Electric discharger / railgun - long barrel with energy coils."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (30, 33, 38, 255))
    # Main barrel
    draw.rectangle([26, 4, 40, 50], fill=(24, 26, 30, 255), outline=(44, 48, 52, 255))
    # Energy coils
    for y in range(10, 48, 6):
        draw.rectangle([22, y, 44, y+3], fill=(20, 40, 60, 200))
        draw.line([(24, y+1), (42, y+1)], fill=(40, 100, 180, 180), width=1)
    # Capacitor
    draw.rectangle([8, 40, 22, 58], fill=(28, 32, 36, 255), outline=(42, 46, 50, 255))
    draw.rectangle([44, 40, 58, 58], fill=(28, 32, 36, 255), outline=(42, 46, 50, 255))
    # Energy glow at muzzle
    draw.ellipse([28, 2, 38, 12], fill=(40, 100, 200, 100))
    draw_glow_circle(draw, 33, 7, 8, (40, 120, 220), 40)
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/railgun.png")

def gen_sonar_array():
    """Sonar array - dish/dome with radiating elements."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (32, 36, 40, 255))
    # Sonar dome
    draw.ellipse([12, 12, 54, 54], fill=(28, 32, 36, 255), outline=(46, 50, 54, 255))
    # Radiating rings
    for r in [8, 14, 20]:
        draw.arc([33-r, 33-r, 33+r, 33+r], 200, 340, fill=(0, 140, 120, 120), width=1)
    # Central element
    draw.ellipse([28, 28, 38, 38], fill=(24, 28, 32, 255))
    draw.ellipse([30, 30, 36, 36], fill=(0, 160, 130, 200))
    draw.ellipse([32, 32, 34, 34], fill=(60, 220, 180, 255))
    # Mounting
    draw.rectangle([30, 52, 36, 62], fill=(40, 44, 48, 255))
    draw_glow_circle(draw, 33, 33, 10, (0, 160, 130), 30)
    add_texture(img, 0.12)
    img.save(f"{MODULE_DIR}/sonar_array.png")

def gen_passive_sonar():
    """Passive sonar - hydrophone array, listening device."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (30, 34, 38, 255))
    # Hydrophone elements
    for i in range(3):
        cx = 20 + i * 13
        draw.ellipse([cx-6, 14, cx+6, 26], fill=(26, 30, 34, 255), outline=(42, 46, 50, 255))
        draw.line([(cx, 26), (cx, 44)], fill=(38, 42, 46, 255), width=2)
    # Processing unit
    draw.rectangle([12, 44, 54, 58], fill=(28, 32, 36, 255), outline=(40, 44, 48, 255))
    # Signal indicator
    for sx in range(16, 50, 4):
        h = random.randint(2, 8)
        draw.rectangle([sx, 56-h, sx+2, 56], fill=(0, 120, 100, 180))
    add_texture(img, 0.12)
    img.save(f"{MODULE_DIR}/passive_sonar.png")

def gen_depth_sensor():
    """Depth sensor / scanner - gauge and probe."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (34, 37, 42, 255))
    # Gauge
    draw.ellipse([14, 10, 52, 48], fill=(12, 14, 18, 255), outline=(46, 50, 54, 255))
    # Gauge markings
    for angle in range(210, 330, 15):
        rad = math.radians(angle)
        x1 = 33 + int(16 * math.cos(rad))
        y1 = 29 + int(16 * math.sin(rad))
        x2 = 33 + int(18 * math.cos(rad))
        y2 = 29 + int(18 * math.sin(rad))
        draw.line([(x1, y1), (x2, y2)], fill=(80, 85, 90, 200), width=1)
    # Needle
    needle_angle = math.radians(260)
    nx = 33 + int(14 * math.cos(needle_angle))
    ny = 29 + int(14 * math.sin(needle_angle))
    draw.line([(33, 29), (nx, ny)], fill=(200, 60, 40, 255), width=1)
    draw.ellipse([31, 27, 35, 31], fill=(60, 64, 68, 255))
    # Digital readout
    draw.rectangle([14, 50, 52, 60], fill=(8, 12, 16, 255), outline=(40, 44, 48, 255))
    draw.rectangle([18, 52, 48, 58], fill=(0, 80, 60, 180))
    add_texture(img, 0.12)
    img.save(f"{MODULE_DIR}/depth_sensor.png")

def gen_cargo_hold():
    """Cargo hold / ammo bay - crate storage."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (36, 38, 42, 255))
    # Shelves with crates
    for row in range(3):
        y = 8 + row * 18
        draw.line([(8, y+16), (58, y+16)], fill=(44, 48, 52, 255), width=2)
        for col in range(3):
            x = 10 + col * 16
            c = random.choice([(40, 35, 28, 240), (35, 38, 30, 240), (38, 34, 34, 240)])
            draw.rectangle([x, y+2, x+12, y+14], fill=c, outline=(50, 52, 48, 255))
            draw.line([(x+2, y+8), (x+10, y+8)], fill=(55, 58, 52, 200), width=1)
    draw_rivets(draw, [(6, 6), (60, 6), (6, 60), (60, 60)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/cargo_hold.png")

def gen_ballast_tank():
    """Ballast tank / fuel tank - cylindrical pressure vessel."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (32, 36, 40, 255))
    # Pressure vessel
    draw.rounded_rectangle([10, 10, 56, 56], radius=8, fill=(28, 32, 36, 255), outline=(44, 48, 52, 255))
    # Liquid level
    level = random.randint(20, 44)
    draw.rectangle([12, level, 54, 54], fill=(20, 50, 65, 180))
    # Pressure gauge
    draw.ellipse([24, 14, 36, 26], fill=(10, 12, 16, 255), outline=(48, 52, 56, 255))
    draw.ellipse([28, 18, 32, 22], fill=(0, 140, 100, 200))
    # Valve
    draw.rectangle([42, 12, 52, 20], fill=(46, 50, 54, 255))
    draw.line([(44, 10), (50, 10)], fill=(56, 60, 64, 255), width=2)
    draw_rivets(draw, [(12, 12), (54, 12), (12, 54), (54, 54)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/ballast_tank.png")

def gen_research_lab():
    """Research lab / specimen vault - scientific equipment."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (32, 36, 40, 255))
    # Specimen jar
    draw.rectangle([22, 10, 44, 44], fill=(20, 40, 50, 120), outline=(50, 55, 60, 255))
    # Something floating inside
    draw.ellipse([28, 20, 38, 32], fill=(40, 80, 60, 100))
    draw.point((33, 25), fill=(120, 200, 100, 180))
    # Equipment shelf
    draw.rectangle([8, 46, 58, 58], fill=(36, 40, 44, 255), outline=(44, 48, 52, 255))
    # Tools
    draw.rectangle([12, 48, 16, 56], fill=(60, 120, 80, 200))
    draw.rectangle([20, 48, 28, 56], fill=(80, 80, 120, 200))
    draw.ellipse([34, 49, 40, 55], fill=(40, 100, 140, 200))
    # Status light
    draw.ellipse([50, 8, 56, 14], fill=(200, 60, 40, 200))
    add_texture(img, 0.12)
    img.save(f"{MODULE_DIR}/research_lab.png")

def gen_basic_quarters():
    """Basic quarters / barracks / mess hall - bunks and living space."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (34, 36, 38, 255))
    # Bunk frame
    draw.rectangle([8, 8, 32, 30], fill=(28, 30, 34, 255), outline=(42, 46, 50, 255))
    draw.rectangle([10, 10, 30, 18], fill=(40, 42, 50, 200))  # mattress
    draw.rectangle([10, 20, 30, 28], fill=(40, 42, 50, 200))  # mattress
    # Desk
    draw.rectangle([36, 8, 58, 30], fill=(38, 34, 28, 255), outline=(48, 44, 38, 255))
    draw.rectangle([40, 12, 54, 26], fill=(8, 12, 16, 255))  # screen
    draw.ellipse([44, 16, 50, 22], fill=(0, 80, 60, 150))
    # Floor
    draw.rectangle([8, 34, 58, 58], fill=(30, 32, 36, 255))
    # Locker
    draw.rectangle([8, 36, 20, 56], fill=(32, 35, 40, 255), outline=(44, 48, 52, 255))
    draw.ellipse([12, 44, 16, 48], fill=(60, 64, 68, 255))
    # Overhead light
    draw.rectangle([28, 34, 38, 36], fill=(80, 80, 60, 200))
    add_texture(img, 0.12)
    img.save(f"{MODULE_DIR}/basic_quarters.png")

def gen_medical_bay():
    """Medical bay - operating table, monitors."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (34, 38, 40, 255))
    # Operating table
    draw.rectangle([12, 24, 54, 42], fill=(40, 44, 48, 255), outline=(52, 56, 60, 255))
    draw.rectangle([14, 26, 52, 40], fill=(60, 62, 66, 240))
    # Overhead surgical light
    draw.ellipse([26, 8, 40, 22], fill=(50, 54, 58, 255), outline=(60, 64, 68, 255))
    draw.ellipse([30, 12, 36, 18], fill=(200, 200, 180, 200))
    # Medical cross
    draw.rectangle([31, 28, 35, 38], fill=(180, 40, 40, 220))
    draw.rectangle([28, 31, 38, 35], fill=(180, 40, 40, 220))
    # Monitor
    draw.rectangle([8, 44, 24, 58], fill=(8, 12, 16, 255), outline=(42, 46, 50, 255))
    # Heartbeat line
    for x in range(10, 22):
        y = 51 + int(3 * math.sin(x * 0.8))
        draw.point((x, y), fill=(0, 200, 80, 200))
    # Supply cabinet
    draw.rectangle([42, 44, 58, 58], fill=(36, 40, 44, 255), outline=(46, 50, 54, 255))
    add_texture(img, 0.12)
    img.save(f"{MODULE_DIR}/medical_bay.png")

def gen_repair_station():
    """Repair bay - workbench with tools."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (36, 38, 42, 255))
    # Workbench
    draw.rectangle([8, 30, 58, 44], fill=(42, 38, 32, 255), outline=(52, 48, 42, 255))
    # Tools on wall
    # Wrench
    draw.line([(12, 10), (12, 26)], fill=(56, 60, 64, 255), width=2)
    draw.ellipse([9, 7, 15, 13], outline=(56, 60, 64, 255), width=1)
    # Hammer
    draw.line([(24, 12), (24, 26)], fill=(50, 44, 38, 255), width=2)
    draw.rectangle([20, 8, 28, 14], fill=(56, 60, 64, 255))
    # Welding torch
    draw.line([(38, 26), (38, 14)], fill=(44, 48, 52, 255), width=2)
    draw.ellipse([35, 8, 41, 14], fill=(200, 140, 40, 200))
    # Spare parts bins
    for bx in range(10, 56, 16):
        draw.rectangle([bx, 48, bx+12, 58], fill=(30, 34, 38, 255), outline=(44, 48, 52, 255))
    draw_rivets(draw, [(6, 6), (60, 6), (6, 60), (60, 60)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/repair_station.png")

def gen_floodlight():
    """Floodlight / searchlight - lamp housing with beam cone."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (32, 36, 40, 255))
    # Lamp housing
    draw.ellipse([18, 18, 48, 48], fill=(30, 33, 38, 255), outline=(46, 50, 54, 255))
    # Reflector
    draw.ellipse([22, 22, 44, 44], fill=(60, 58, 50, 255))
    draw.ellipse([26, 26, 40, 40], fill=(200, 190, 140, 240))
    draw.ellipse([30, 30, 36, 36], fill=(255, 245, 200, 255))
    # Beam cone (upward)
    draw.polygon([(28, 18), (33, 2), (38, 18)], fill=(255, 240, 180, 60))
    # Mount
    draw.rectangle([30, 48, 36, 62], fill=(40, 44, 48, 255))
    draw_glow_circle(draw, 33, 33, 14, (255, 240, 180), 40)
    add_texture(img, 0.1)
    img.save(f"{MODULE_DIR}/floodlight.png")

def gen_docking_port():
    """Docking port / airlock - heavy door mechanism."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (30, 34, 38, 255))
    # Airlock door frame
    draw.rectangle([14, 10, 52, 56], fill=(26, 28, 32, 255), outline=(48, 52, 56, 255), width=2)
    # Door halves
    draw.rectangle([16, 12, 33, 54], fill=(34, 37, 42, 255))
    draw.rectangle([33, 12, 50, 54], fill=(34, 37, 42, 255))
    # Center seam
    draw.line([(33, 12), (33, 54)], fill=(20, 22, 26, 255), width=2)
    # Warning stripe
    for y in range(12, 54, 6):
        c = (180, 140, 40, 200) if (y // 6) % 2 == 0 else (30, 30, 30, 200)
        draw.rectangle([16, y, 20, y+3], fill=c)
        draw.rectangle([46, y, 50, y+3], fill=c)
    # Handle
    draw.rectangle([28, 30, 38, 36], fill=(50, 54, 58, 255), outline=(60, 64, 68, 255))
    # Status light
    draw.ellipse([30, 6, 36, 12], fill=(40, 180, 80, 220))
    draw_rivets(draw, [(16, 12), (50, 12), (16, 54), (50, 54)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/docking_port.png")

def gen_salvage_arm():
    """Salvage arm - articulated mechanical arm, drawn facing right."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 20, 26, 46], (34, 38, 42, 255))
    # Shoulder joint
    draw.ellipse([20, 26, 32, 40], fill=(38, 42, 46, 255), outline=(50, 54, 58, 255))
    # Upper arm
    draw.polygon([(30, 30), (48, 22), (50, 28), (32, 36)], fill=(34, 38, 42, 255))
    # Elbow joint
    draw.ellipse([44, 20, 54, 32], fill=(38, 42, 46, 255), outline=(50, 54, 58, 255))
    # Forearm
    draw.polygon([(50, 24), (60, 34), (62, 38), (52, 28)], fill=(32, 36, 40, 255))
    # Claw
    draw.polygon([(58, 34), (64, 30), (64, 34)], fill=(44, 48, 52, 255))
    draw.polygon([(58, 38), (64, 42), (64, 38)], fill=(44, 48, 52, 255))
    # Hydraulic cylinder
    draw.line([(26, 42), (48, 28)], fill=(46, 50, 54, 200), width=2)
    draw_rivets(draw, [(6, 22), (6, 44)])
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/salvage_arm.png")

def gen_mine_layer():
    """Mine layer - deployment chute with mines."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_panel(draw, [4, 4, 62, 62], (32, 35, 40, 255))
    # Deployment chute
    draw.rectangle([20, 8, 46, 50], fill=(26, 28, 32, 255), outline=(44, 48, 52, 255))
    # Mines in chute
    for my in [14, 28, 42]:
        draw.ellipse([26, my-5, 40, my+5], fill=(40, 36, 30, 255), outline=(55, 50, 44, 255))
        # Detonator
        draw.ellipse([31, my-2, 35, my+2], fill=(180, 40, 40, 220))
    # Release mechanism
    draw.rectangle([20, 50, 46, 58], fill=(38, 42, 46, 255), outline=(48, 52, 56, 255))
    # Warning label
    draw.rectangle([8, 28, 18, 38], fill=(180, 140, 40, 200))
    draw.line([(10, 30), (16, 36)], fill=(30, 30, 30, 255), width=1)
    draw.line([(10, 36), (16, 30)], fill=(30, 30, 30, 255), width=1)
    add_texture(img, 0.15)
    img.save(f"{MODULE_DIR}/mine_layer.png")


# ============================================================================
# HULL SPRITES
# ============================================================================

def gen_hull(filename, base_color, accent_color, pattern="riveted"):
    """Generate a hull segment sprite."""
    img = Image.new("RGBA", (CELL, CELL), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.rectangle([0, 0, 65, 65], fill=base_color)
    # Border
    draw.rectangle([0, 0, 65, 65], outline=accent_color, width=2)
    if pattern == "riveted":
        draw_rivets(draw, [(6, 6), (60, 6), (6, 60), (60, 60), (33, 6), (33, 60), (6, 33), (60, 33)], accent_color)
        for y in [22, 44]:
            draw.line([(2, y), (64, y)], fill=accent_color[:3] + (100,), width=1)
    elif pattern == "plated":
        for y in range(0, 66, 16):
            draw.line([(0, y), (66, y)], fill=accent_color, width=1)
        for x in range(0, 66, 22):
            draw.line([(x, 0), (x, 66)], fill=accent_color, width=1)
        draw_rivets(draw, [(6, 6), (28, 6), (50, 6), (6, 22), (28, 22), (50, 22),
                           (6, 38), (28, 38), (50, 38), (6, 54), (28, 54), (50, 54)], accent_color)
    elif pattern == "composite":
        for i in range(0, 66, 4):
            alpha = 40 + (i % 8) * 5
            draw.line([(i, 0), (i, 66)], fill=accent_color[:3] + (alpha,), width=1)
        draw.line([(0, 33), (66, 33)], fill=accent_color, width=1)
    elif pattern == "alien":
        # Organic-looking veins
        for _ in range(8):
            sx, sy = random.randint(5, 60), random.randint(5, 60)
            for seg in range(6):
                ex = sx + random.randint(-8, 8)
                ey = sy + random.randint(-8, 8)
                draw.line([(sx, sy), (ex, ey)], fill=accent_color[:3] + (80,), width=1)
                sx, sy = ex, ey
        draw.ellipse([28, 28, 38, 38], fill=accent_color[:3] + (60,))
    add_texture(img, 0.12)
    img.save(f"{HULL_DIR}/{filename}")


# ============================================================================
# EFFECT SPRITES
# ============================================================================

def gen_torpedo_trail():
    img = Image.new("RGBA", (32, 8), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Torpedo body
    draw.rounded_rectangle([0, 1, 24, 7], radius=2, fill=(60, 55, 45, 255))
    draw.polygon([(24, 0), (30, 4), (24, 8)], fill=(70, 65, 55, 255))
    # Propulsion glow
    draw.ellipse([0, 2, 6, 6], fill=(200, 120, 40, 200))
    img.save(f"{EFFECT_DIR}/torpedo_trail.png")

def gen_enemy_projectile():
    img = Image.new("RGBA", (16, 8), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.ellipse([2, 1, 14, 7], fill=(200, 40, 40, 220))
    draw.ellipse([4, 2, 12, 6], fill=(255, 80, 60, 255))
    draw.ellipse([6, 3, 10, 5], fill=(255, 200, 100, 255))
    img.save(f"{EFFECT_DIR}/enemy_projectile.png")

def gen_bubble():
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.ellipse([2, 2, 14, 14], fill=(140, 180, 200, 60), outline=(160, 200, 220, 100))
    draw.ellipse([5, 4, 8, 7], fill=(200, 230, 255, 100))
    img.save(f"{EFFECT_DIR}/bubble.png")

def gen_electric_shock():
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Lightning bolts from center
    for angle in range(0, 360, 45):
        rad = math.radians(angle + random.randint(-10, 10))
        pts = [(16, 16)]
        cx, cy = 16, 16
        for seg in range(4):
            cx += int(4 * math.cos(rad) + random.randint(-2, 2))
            cy += int(4 * math.sin(rad) + random.randint(-2, 2))
            pts.append((cx, cy))
        for i in range(len(pts)-1):
            draw.line([pts[i], pts[i+1]], fill=(100, 180, 255, 200), width=1)
    draw.ellipse([12, 12, 20, 20], fill=(80, 160, 255, 120))
    img.save(f"{EFFECT_DIR}/electric_shock.png")

def gen_explosion():
    img = Image.new("RGBA", (48, 48), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Outer blast
    draw.ellipse([4, 4, 44, 44], fill=(200, 100, 20, 100))
    draw.ellipse([8, 8, 40, 40], fill=(240, 140, 40, 150))
    draw.ellipse([14, 14, 34, 34], fill=(255, 200, 80, 200))
    draw.ellipse([18, 18, 30, 30], fill=(255, 240, 180, 255))
    # Debris particles
    for _ in range(12):
        px = random.randint(2, 46)
        py = random.randint(2, 46)
        draw.point((px, py), fill=(255, random.randint(100, 200), 20, 200))
    img.save(f"{EFFECT_DIR}/explosion.png")

def gen_sonar_ring():
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    for r in [30, 28, 26]:
        alpha = int(80 * (r / 30))
        draw.ellipse([32-r, 32-r, 32+r, 32+r], outline=(0, 180, 140, alpha), width=2)
    img.save(f"{EFFECT_DIR}/sonar_ring.png")


# ============================================================================
# ENVIRONMENT / POI SPRITES
# ============================================================================

def gen_wreck():
    img = Image.new("RGBA", (128, 96), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Broken hull
    draw.polygon([(10, 50), (20, 30), (60, 20), (100, 25), (120, 40), (115, 70), (80, 80), (30, 75)],
                 fill=(28, 30, 34, 240))
    # Structural ribs
    for x in range(25, 110, 12):
        draw.line([(x, 25 + abs(x-60)//4), (x, 72 - abs(x-60)//6)], fill=(36, 38, 42, 200), width=2)
    # Breach
    draw.polygon([(55, 30), (75, 28), (72, 50), (58, 48)], fill=(12, 14, 18, 200))
    # Rust patches
    for _ in range(8):
        rx, ry = random.randint(20, 110), random.randint(30, 70)
        draw.ellipse([rx-4, ry-3, rx+4, ry+3], fill=(50, 35, 25, 120))
    # Debris
    for _ in range(5):
        dx, dy = random.randint(5, 120), random.randint(60, 90)
        draw.rectangle([dx, dy, dx+random.randint(3,8), dy+random.randint(2,5)], fill=(32, 34, 38, 180))
    add_texture(img, 0.2)
    img.save(f"{ENV_DIR}/wreck.png")

def gen_cave():
    img = Image.new("RGBA", (128, 96), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Cave opening
    draw.polygon([(10, 90), (20, 40), (35, 15), (64, 5), (93, 15), (108, 40), (118, 90)],
                 fill=(18, 20, 24, 240))
    # Inner darkness
    draw.polygon([(30, 85), (38, 45), (50, 28), (64, 22), (78, 28), (90, 45), (98, 85)],
                 fill=(6, 8, 12, 255))
    # Stalactites
    for sx in range(35, 95, 10):
        sh = random.randint(8, 20)
        draw.polygon([(sx-2, 20+abs(sx-64)//4), (sx, 20+abs(sx-64)//4+sh), (sx+2, 20+abs(sx-64)//4)],
                     fill=(22, 24, 28, 230))
    # Bioluminescent spots
    for _ in range(4):
        bx, by = random.randint(35, 90), random.randint(30, 75)
        draw.ellipse([bx-2, by-2, bx+2, by+2], fill=(0, 120, 100, 140))
    add_texture(img, 0.15)
    img.save(f"{ENV_DIR}/cave.png")

def gen_ruins():
    img = Image.new("RGBA", (128, 96), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Broken columns
    for cx in [25, 55, 85]:
        h = random.randint(40, 60)
        draw.rectangle([cx-6, 90-h, cx+6, 90], fill=(40, 42, 38, 230))
        # Capital
        draw.rectangle([cx-8, 90-h, cx+8, 90-h+6], fill=(45, 47, 43, 240))
        # Cracks
        for _ in range(3):
            sy = random.randint(90-h+8, 86)
            draw.line([(cx-4, sy), (cx+random.randint(-2, 4), sy+random.randint(4, 10))],
                     fill=(30, 32, 28, 180), width=1)
    # Floor tiles
    draw.rectangle([10, 82, 118, 92], fill=(35, 37, 33, 200))
    for tx in range(10, 118, 14):
        draw.line([(tx, 82), (tx, 92)], fill=(28, 30, 26, 180), width=1)
    # Mysterious glyphs
    for gx in [35, 75]:
        draw.rectangle([gx-3, 55, gx+3, 65], fill=(0, 100, 80, 100))
    add_texture(img, 0.15)
    img.save(f"{ENV_DIR}/ruins.png")

def gen_thermal_vent():
    img = Image.new("RGBA", (96, 96), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Vent chimney
    draw.polygon([(35, 90), (30, 50), (28, 30), (32, 20), (48, 10), (64, 20), (68, 30), (66, 50), (61, 90)],
                 fill=(30, 28, 24, 240))
    # Heat glow at top
    draw.ellipse([36, 6, 60, 22], fill=(80, 40, 10, 120))
    draw.ellipse([40, 10, 56, 18], fill=(160, 80, 20, 100))
    # Mineral deposits
    for _ in range(6):
        mx, my = random.randint(30, 65), random.randint(20, 80)
        draw.ellipse([mx-3, my-2, mx+3, my+2], fill=(60, 50, 30, 180))
    # Rising heat distortion (particles)
    for _ in range(10):
        px = random.randint(38, 58)
        py = random.randint(2, 30)
        a = random.randint(30, 80)
        draw.ellipse([px-1, py-1, px+1, py+1], fill=(200, 100, 30, a))
    add_texture(img, 0.2)
    img.save(f"{ENV_DIR}/thermal_vent.png")

def gen_settlement():
    img = Image.new("RGBA", (128, 96), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Structures
    for sx, sw, sh in [(20, 25, 35), (55, 30, 45), (95, 20, 30)]:
        draw.rectangle([sx, 85-sh, sx+sw, 85], fill=(32, 35, 38, 240), outline=(42, 45, 48, 255))
        # Windows
        for wy in range(85-sh+6, 80, 10):
            for wx in range(sx+4, sx+sw-4, 8):
                lit = random.random() < 0.4
                c = (80, 140, 100, 180) if lit else (12, 14, 18, 200)
                draw.rectangle([wx, wy, wx+4, wy+5], fill=c)
    # Connecting walkway
    draw.rectangle([45, 60, 55, 63], fill=(38, 40, 44, 220))
    # Dome
    draw.arc([50, 30, 90, 65], 180, 360, fill=(40, 44, 48, 200), width=2)
    # Ground
    draw.rectangle([5, 85, 123, 92], fill=(25, 27, 22, 200))
    add_texture(img, 0.15)
    img.save(f"{ENV_DIR}/settlement.png")

def gen_rock():
    img = Image.new("RGBA", (48, 48), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    pts = []
    for i in range(8):
        angle = (2 * math.pi * i) / 8
        r = 16 + random.randint(-4, 4)
        pts.append((24 + int(r * math.cos(angle)), 24 + int(r * math.sin(angle))))
    draw.polygon(pts, fill=(36, 38, 34, 240))
    # Cracks
    draw.line([(18, 20), (28, 28)], fill=(28, 30, 26, 180), width=1)
    add_texture(img, 0.2)
    img.save(f"{ENV_DIR}/rock.png")

def gen_kelp():
    img = Image.new("RGBA", (32, 96), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    for strand in range(3):
        sx = 8 + strand * 8
        pts = [(sx, 92)]
        for seg in range(12):
            y = 92 - seg * 7
            x = sx + int(4 * math.sin(seg * 0.5 + strand))
            pts.append((x, y))
        for i in range(len(pts)-1):
            c = (20, 50 + i * 3, 25, max(60, 220 - i * 12))
            draw.line([pts[i], pts[i+1]], fill=c, width=2)
            # Leaf
            if i % 3 == 1:
                lx, ly = pts[i]
                draw.ellipse([lx-3, ly-2, lx+5, ly+2], fill=(25, 60+i*2, 30, c[3]))
    img.save(f"{ENV_DIR}/kelp.png")

def gen_coral():
    img = Image.new("RGBA", (64, 48), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    colors = [(60, 30, 40, 220), (40, 50, 60, 220), (50, 40, 55, 220)]
    for branch in range(5):
        bx = 10 + branch * 11
        color = random.choice(colors)
        # Branch
        for seg in range(random.randint(3, 6)):
            y = 44 - seg * 6
            x = bx + int(3 * math.sin(seg * 0.8 + branch))
            draw.ellipse([x-3, y-2, x+3, y+2], fill=color)
    # Polyps
    for _ in range(8):
        px, py = random.randint(8, 56), random.randint(10, 40)
        draw.point((px, py), fill=(80, 140, 120, 160))
    add_texture(img, 0.15)
    img.save(f"{ENV_DIR}/coral.png")

def gen_bioluminescent_spot():
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Glowing orb
    for r in range(14, 0, -1):
        a = int(120 * (r / 14) * 0.5)
        draw.ellipse([16-r, 16-r, 16+r, 16+r], fill=(0, 180, 140, min(255, a)))
    draw.ellipse([12, 12, 20, 20], fill=(0, 200, 160, 150))
    draw.ellipse([14, 14, 18, 18], fill=(80, 255, 220, 200))
    img.save(f"{ENV_DIR}/bioluminescent_spot.png")

def gen_sand_mound():
    img = Image.new("RGBA", (64, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    pts = [(0, 30)]
    for x in range(0, 65, 4):
        y = 30 - int(10 * math.sin(x * 0.05) * math.sin(x * 0.03 + 1))
        pts.append((x, max(8, y)))
    pts.append((64, 30))
    draw.polygon(pts, fill=(40, 38, 30, 200))
    add_texture(img, 0.15)
    img.save(f"{ENV_DIR}/sand_mound.png")


# ============================================================================

if __name__ == "__main__":
    print("Generating Barotrauma-style game sprites...\n")

    print("Module sprites (25):")
    gen_small_reactor();    print("  small_reactor.png")
    gen_large_reactor();    print("  large_reactor.png")
    gen_battery();          print("  battery.png")
    gen_standard_engine();  print("  standard_engine.png")
    gen_silent_drive();     print("  silent_drive.png")
    gen_oxygen_scrubber();  print("  oxygen_scrubber.png")
    gen_life_support();     print("  life_support.png")
    gen_navigation();       print("  navigation.png")
    gen_torpedo_tube();     print("  torpedo_tube.png")
    gen_point_defense();    print("  point_defense.png")
    gen_railgun();          print("  railgun.png")
    gen_sonar_array();      print("  sonar_array.png")
    gen_passive_sonar();    print("  passive_sonar.png")
    gen_depth_sensor();     print("  depth_sensor.png")
    gen_cargo_hold();       print("  cargo_hold.png")
    gen_ballast_tank();     print("  ballast_tank.png")
    gen_research_lab();     print("  research_lab.png")
    gen_basic_quarters();   print("  basic_quarters.png")
    gen_medical_bay();      print("  medical_bay.png")
    gen_repair_station();   print("  repair_station.png")
    gen_floodlight();       print("  floodlight.png")
    gen_docking_port();     print("  docking_port.png")
    gen_salvage_arm();      print("  salvage_arm.png")
    gen_mine_layer();       print("  mine_layer.png")

    print("\nHull sprites (4):")
    gen_hull("hull_steel.png",    (38, 42, 46, 255), (50, 54, 58, 255), "riveted");    print("  hull_steel.png")
    gen_hull("hull_titanium.png", (44, 48, 55, 255), (56, 60, 68, 255), "plated");     print("  hull_titanium.png")
    gen_hull("hull_composite.png",(36, 40, 50, 255), (48, 52, 62, 255), "composite");  print("  hull_composite.png")
    gen_hull("hull_abyssal.png",  (22, 18, 30, 255), (40, 30, 55, 255), "alien");      print("  hull_abyssal.png")

    print("\nEffect sprites (6):")
    gen_torpedo_trail();    print("  torpedo_trail.png")
    gen_enemy_projectile(); print("  enemy_projectile.png")
    gen_bubble();           print("  bubble.png")
    gen_electric_shock();   print("  electric_shock.png")
    gen_explosion();        print("  explosion.png")
    gen_sonar_ring();       print("  sonar_ring.png")

    print("\nEnvironment sprites (10):")
    gen_wreck();            print("  wreck.png")
    gen_cave();             print("  cave.png")
    gen_ruins();            print("  ruins.png")
    gen_thermal_vent();     print("  thermal_vent.png")
    gen_settlement();       print("  settlement.png")
    gen_rock();             print("  rock.png")
    gen_kelp();             print("  kelp.png")
    gen_coral();            print("  coral.png")
    gen_bioluminescent_spot(); print("  bioluminescent_spot.png")
    gen_sand_mound();       print("  sand_mound.png")

    print("\nDone! All sprites generated.")
