#!/usr/bin/env python3
"""
Afterlight - Industrial Pixel Art Sprite Generator
Style: Dark & Gritty, Industrial, 64x64 pixel art
"""
from PIL import Image, ImageDraw
import random
import os

SPRITE_SIZE = 64
OUT = "/Users/fredericmoungpacrijanot/afterlight/assets/sprites"

# === COLOR PALETTES ===
# Dark industrial palette
STEEL_DARK = (45, 50, 58)
STEEL_MID = (70, 78, 88)
STEEL_LIGHT = (95, 105, 118)
STEEL_HIGHLIGHT = (120, 132, 148)

RUST_DARK = (80, 42, 28)
RUST_MID = (110, 58, 35)
RUST_LIGHT = (140, 78, 45)

RIVET = (55, 62, 72)
RIVET_HIGHLIGHT = (130, 140, 155)

WARNING_YELLOW = (180, 155, 40)
WARNING_BLACK = (30, 30, 30)

REACTOR_GLOW = (40, 180, 160)
REACTOR_CORE = (20, 120, 110)
REACTOR_DIM = (15, 80, 72)

ENGINE_ORANGE = (180, 100, 30)
ENGINE_RED = (160, 50, 30)

O2_BLUE = (40, 120, 180)
O2_LIGHT = (80, 160, 210)

WEAPON_RED = (160, 40, 40)
WEAPON_DARK = (100, 30, 30)

SONAR_GREEN = (40, 180, 80)
SONAR_DIM = (25, 100, 50)

LIGHT_YELLOW = (220, 200, 100)
LIGHT_WHITE = (240, 235, 200)

GLASS_BLUE = (60, 100, 140)
GLASS_LIGHT = (90, 140, 180)

PIPE_DARK = (50, 55, 62)
PIPE_MID = (65, 72, 82)

WATER_BLUE = (30, 70, 130)
WATER_LIGHT = (50, 100, 160)

BG_TRANSPARENT = (0, 0, 0, 0)

random.seed(42)  # Reproducible


def new_sprite():
    return Image.new("RGBA", (SPRITE_SIZE, SPRITE_SIZE), BG_TRANSPARENT)


def draw_metal_plate(draw, x, y, w, h, color=STEEL_MID, dark=STEEL_DARK, light=STEEL_LIGHT):
    """Draw an industrial metal plate with beveled edges."""
    draw.rectangle([x, y, x+w-1, y+h-1], fill=color)
    # Top/left highlight
    draw.line([(x, y), (x+w-1, y)], fill=light)
    draw.line([(x, y), (x, y+h-1)], fill=light)
    # Bottom/right shadow
    draw.line([(x, y+h-1), (x+w-1, y+h-1)], fill=dark)
    draw.line([(x+w-1, y), (x+w-1, y+h-1)], fill=dark)


def draw_rivets(draw, x, y, w, h, spacing=10):
    """Draw rivets around a metal plate."""
    for rx in range(x+4, x+w-2, spacing):
        draw.rectangle([rx, y+2, rx+1, y+3], fill=RIVET_HIGHLIGHT)
        draw.rectangle([rx, y+h-4, rx+1, y+h-3], fill=RIVET_HIGHLIGHT)
    for ry in range(y+4, y+h-2, spacing):
        draw.rectangle([x+2, ry, x+3, ry+1], fill=RIVET_HIGHLIGHT)
        draw.rectangle([x+w-4, ry, x+w-3, ry+1], fill=RIVET_HIGHLIGHT)


def draw_warning_stripes(draw, x, y, w, h):
    """Draw diagonal warning stripes."""
    for i in range(0, w + h, 6):
        x1 = x + i
        y1 = y
        x2 = x + i - h
        y2 = y + h
        color = WARNING_YELLOW if (i // 6) % 2 == 0 else WARNING_BLACK
        draw.line([(x1, y1), (x2, y2)], fill=color, width=2)


def add_rust_spots(draw, x, y, w, h, count=5):
    """Add random rust spots for gritty feel."""
    for _ in range(count):
        rx = random.randint(x+2, x+w-4)
        ry = random.randint(y+2, y+h-4)
        rs = random.randint(1, 3)
        color = random.choice([RUST_DARK, RUST_MID, RUST_LIGHT])
        draw.rectangle([rx, ry, rx+rs, ry+rs], fill=color)


def draw_pipe(draw, x1, y1, x2, y2, horizontal=True):
    """Draw an industrial pipe."""
    if horizontal:
        draw.rectangle([x1, y1, x2, y1+3], fill=PIPE_MID)
        draw.line([(x1, y1), (x2, y1)], fill=STEEL_LIGHT)
        draw.line([(x1, y1+3), (x2, y1+3)], fill=STEEL_DARK)
        # Joints
        for jx in range(x1, x2, 12):
            draw.rectangle([jx, y1-1, jx+2, y1+4], fill=PIPE_DARK)
    else:
        draw.rectangle([x1, y1, x1+3, y2], fill=PIPE_MID)
        draw.line([(x1, y1), (x1, y2)], fill=STEEL_LIGHT)
        draw.line([(x1+3, y1), (x1+3, y2)], fill=STEEL_DARK)
        for jy in range(y1, y2, 12):
            draw.rectangle([x1-1, jy, x1+4, jy+2], fill=PIPE_DARK)


def draw_vent(draw, x, y, w, h):
    """Draw ventilation slats."""
    for vy in range(y, y+h, 4):
        draw.line([(x, vy), (x+w, vy)], fill=STEEL_DARK)
        draw.line([(x, vy+1), (x+w, vy+1)], fill=STEEL_LIGHT)


def draw_gauge(draw, cx, cy, r, value=0.7, color=REACTOR_GLOW):
    """Draw a circular gauge/meter."""
    draw.ellipse([cx-r, cy-r, cx+r, cy+r], fill=STEEL_DARK, outline=STEEL_LIGHT)
    inner = int(r * 0.7)
    draw.ellipse([cx-inner, cy-inner, cx+inner, cy+inner], fill=(20, 20, 25))
    # Indicator
    import math
    angle = -math.pi/2 + value * math.pi * 1.5
    ex = cx + int(inner * 0.8 * math.cos(angle))
    ey = cy + int(inner * 0.8 * math.sin(angle))
    draw.line([(cx, cy), (ex, ey)], fill=color, width=1)


# ===================================================================
# MODULE SPRITES
# ===================================================================

def gen_small_reactor():
    """Small nuclear reactor - glowing core, pipes, industrial housing."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Base housing
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID, STEEL_DARK, STEEL_LIGHT)
    draw_rivets(d, 4, 4, 56, 56, 8)
    # Inner containment
    draw_metal_plate(d, 12, 12, 40, 40, STEEL_DARK)
    # Reactor core glow
    d.ellipse([20, 20, 44, 44], fill=REACTOR_DIM)
    d.ellipse([24, 24, 40, 40], fill=REACTOR_CORE)
    d.ellipse([28, 28, 36, 36], fill=REACTOR_GLOW)
    # Fuel rods (cross pattern)
    d.rectangle([30, 14, 33, 50], fill=(60, 70, 80))
    d.rectangle([14, 30, 50, 33], fill=(60, 70, 80))
    # Coolant pipes
    draw_pipe(d, 4, 16, 12, 16)
    draw_pipe(d, 52, 16, 60, 16)
    draw_pipe(d, 4, 44, 12, 44)
    draw_pipe(d, 52, 44, 60, 44)
    # Warning label
    draw_warning_stripes(d, 4, 54, 56, 6)
    add_rust_spots(d, 4, 4, 56, 56, 3)
    return img


def gen_large_reactor():
    """Large reactor - 128x64 (2x1 grid cells)."""
    img = Image.new("RGBA", (128, 64), BG_TRANSPARENT)
    d = ImageDraw.Draw(img)
    # Full housing
    draw_metal_plate(d, 2, 2, 124, 60, STEEL_MID)
    draw_rivets(d, 2, 2, 124, 60, 8)
    # Left containment
    draw_metal_plate(d, 8, 8, 50, 48, STEEL_DARK)
    d.ellipse([16, 14, 50, 48], fill=REACTOR_DIM)
    d.ellipse([22, 20, 44, 42], fill=REACTOR_CORE)
    d.ellipse([28, 26, 38, 36], fill=REACTOR_GLOW)
    # Right containment
    draw_metal_plate(d, 66, 8, 50, 48, STEEL_DARK)
    d.ellipse([74, 14, 108, 48], fill=REACTOR_DIM)
    d.ellipse([80, 20, 102, 42], fill=REACTOR_CORE)
    d.ellipse([86, 26, 96, 36], fill=REACTOR_GLOW)
    # Connection between cores
    draw_pipe(d, 56, 28, 70, 28)
    # Warning stripes
    draw_warning_stripes(d, 2, 54, 124, 8)
    # Gauges
    draw_gauge(d, 62, 16, 6, 0.8, REACTOR_GLOW)
    add_rust_spots(d, 2, 2, 124, 60, 8)
    return img


def gen_standard_engine():
    """Standard propulsion engine - pistons, exhaust."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 8, 56, 48, STEEL_MID)
    draw_rivets(d, 4, 8, 56, 48, 10)
    # Engine block
    draw_metal_plate(d, 10, 14, 32, 36, STEEL_DARK)
    # Pistons
    for py in [18, 28, 38]:
        d.rectangle([14, py, 24, py+6], fill=(80, 85, 92))
        d.rectangle([24, py+1, 30, py+5], fill=STEEL_LIGHT)
    # Exhaust port (right side)
    d.rectangle([44, 20, 58, 28], fill=STEEL_DARK)
    d.rectangle([44, 36, 58, 44], fill=STEEL_DARK)
    for ey in [22, 24, 26]:
        d.line([(46, ey), (56, ey)], fill=ENGINE_ORANGE)
    for ey in [38, 40, 42]:
        d.line([(46, ey), (56, ey)], fill=ENGINE_ORANGE)
    # Drive shaft
    d.rectangle([36, 28, 56, 34], fill=PIPE_MID)
    d.ellipse([50, 26, 60, 36], fill=STEEL_LIGHT, outline=STEEL_DARK)
    add_rust_spots(d, 4, 8, 56, 48, 4)
    return img


def gen_silent_drive():
    """Silent/stealth engine - sleeker, darker."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    dark_body = (35, 38, 45)
    draw_metal_plate(d, 6, 10, 52, 44, dark_body, (25, 28, 32), (50, 55, 62))
    # Smooth drive housing
    d.ellipse([12, 16, 44, 48], fill=(30, 34, 40))
    d.ellipse([16, 20, 40, 44], fill=(25, 28, 34))
    # Magnetic ring
    d.ellipse([20, 24, 36, 40], fill=dark_body, outline=(60, 140, 160))
    d.ellipse([24, 28, 32, 36], fill=(20, 80, 90))
    # Exhaust (subtle blue glow)
    for ex in range(46, 58, 2):
        alpha = 255 - (ex - 46) * 20
        d.line([(ex, 28), (ex, 36)], fill=(40, 100, 140))
    # Dampening panels
    for py in range(12, 50, 6):
        d.rectangle([48, py, 56, py+3], fill=(40, 44, 50))
    return img


def gen_torpedo_tube():
    """Torpedo launcher - tube, loading mechanism."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 6, 56, 52, STEEL_MID)
    draw_rivets(d, 4, 6, 56, 52, 10)
    # Torpedo tubes (2 barrels)
    for ty in [16, 36]:
        # Outer tube
        d.rectangle([8, ty, 54, ty+12], fill=STEEL_DARK)
        d.rectangle([10, ty+2, 52, ty+10], fill=(40, 44, 50))
        # Torpedo inside (red tip)
        d.rectangle([12, ty+3, 42, ty+9], fill=(55, 60, 68))
        d.rectangle([12, ty+4, 16, ty+8], fill=WEAPON_RED)
        # Tube opening
        d.rectangle([48, ty+1, 54, ty+11], fill=STEEL_DARK)
        d.ellipse([48, ty+1, 56, ty+11], fill=(30, 33, 38), outline=STEEL_LIGHT)
    # Loading mechanism
    d.rectangle([8, 28, 20, 36], fill=STEEL_LIGHT)
    draw_warning_stripes(d, 4, 52, 56, 6)
    add_rust_spots(d, 4, 6, 56, 52, 3)
    return img


def gen_point_defense():
    """Point defense turret - small rapid-fire gun."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Base mount
    draw_metal_plate(d, 12, 32, 40, 28, STEEL_MID)
    draw_rivets(d, 12, 32, 40, 28, 8)
    # Turret housing
    d.ellipse([16, 20, 48, 48], fill=STEEL_DARK)
    d.ellipse([20, 24, 44, 44], fill=STEEL_MID)
    # Gun barrels (twin)
    d.rectangle([28, 4, 31, 24], fill=STEEL_DARK)
    d.rectangle([33, 4, 36, 24], fill=STEEL_DARK)
    d.rectangle([28, 4, 31, 6], fill=STEEL_LIGHT)  # Muzzle
    d.rectangle([33, 4, 36, 6], fill=STEEL_LIGHT)
    # Targeting lens
    d.ellipse([29, 28, 35, 34], fill=WEAPON_RED)
    d.ellipse([30, 29, 34, 33], fill=(200, 60, 60))
    # Ammo belt indicator
    d.rectangle([14, 40, 22, 56], fill=WARNING_YELLOW)
    add_rust_spots(d, 12, 20, 40, 40, 2)
    return img


def gen_railgun():
    """Railgun - long barrel, magnetic coils."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Base
    draw_metal_plate(d, 8, 30, 48, 28, STEEL_MID)
    draw_rivets(d, 8, 30, 48, 28, 8)
    # Long barrel
    d.rectangle([26, 2, 37, 34], fill=STEEL_DARK)
    d.rectangle([28, 2, 35, 34], fill=(50, 55, 65))
    # Magnetic coils
    for cy in [8, 16, 24]:
        d.rectangle([22, cy, 41, cy+4], fill=(60, 65, 75))
        d.rectangle([24, cy+1, 39, cy+3], fill=(100, 140, 200))
    # Muzzle glow
    d.rectangle([28, 2, 35, 5], fill=(100, 150, 220))
    # Power conduit
    d.rectangle([38, 34, 42, 56], fill=PIPE_MID)
    draw_gauge(d, 16, 44, 5, 0.9, (100, 150, 220))
    return img


def gen_basic_quarters():
    """Crew quarters - bunk, porthole, industrial."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Bunk beds (left side)
    d.rectangle([8, 36, 30, 42], fill=(70, 60, 50))  # Lower bunk
    d.rectangle([8, 22, 30, 28], fill=(70, 60, 50))  # Upper bunk
    d.rectangle([8, 22, 10, 42], fill=STEEL_DARK)     # Frame
    d.rectangle([28, 22, 30, 42], fill=STEEL_DARK)
    # Blankets
    d.rectangle([10, 37, 28, 41], fill=(60, 70, 90))
    d.rectangle([10, 23, 28, 27], fill=(60, 70, 90))
    # Porthole (right side)
    d.ellipse([38, 14, 54, 30], fill=STEEL_DARK, outline=STEEL_LIGHT)
    d.ellipse([40, 16, 52, 28], fill=GLASS_BLUE)
    d.ellipse([42, 18, 50, 26], fill=GLASS_LIGHT)
    # Cross bar on porthole
    d.line([(40, 22), (52, 22)], fill=STEEL_DARK)
    d.line([(46, 16), (46, 28)], fill=STEEL_DARK)
    # Locker
    d.rectangle([40, 36, 52, 54], fill=STEEL_DARK)
    d.rectangle([44, 38, 48, 40], fill=STEEL_LIGHT)  # Handle
    # Light
    d.rectangle([28, 8, 36, 12], fill=LIGHT_YELLOW)
    add_rust_spots(d, 4, 4, 56, 56, 4)
    return img


def gen_oxygen_scrubber():
    """O2 scrubber - filters, fans, blue accents."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Main filter housing
    draw_metal_plate(d, 10, 10, 44, 30, STEEL_DARK)
    # Filter cartridges
    for fx in [14, 24, 34, 44]:
        d.rectangle([fx, 14, fx+4, 36], fill=O2_BLUE)
        d.rectangle([fx, 14, fx+4, 16], fill=O2_LIGHT)
    # Fan (bottom)
    cx, cy = 32, 48
    d.ellipse([cx-8, cy-8, cx+8, cy+8], fill=STEEL_DARK, outline=STEEL_LIGHT)
    # Fan blades
    for angle_off in [0, 90, 180, 270]:
        import math
        a = math.radians(angle_off + 15)
        ex = cx + int(6 * math.cos(a))
        ey = cy + int(6 * math.sin(a))
        d.line([(cx, cy), (ex, ey)], fill=STEEL_LIGHT, width=2)
    # Air flow arrows (vents on sides)
    draw_vent(d, 6, 12, 4, 26)
    draw_vent(d, 54, 12, 4, 26)
    # Status LED
    d.rectangle([48, 44, 52, 48], fill=O2_LIGHT)
    add_rust_spots(d, 4, 4, 56, 56, 2)
    return img


def gen_ballast_tank():
    """Ballast tank - water gauge, valves."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Tank body (rounded inner)
    d.rectangle([10, 10, 54, 50], fill=STEEL_DARK)
    # Water level (partially filled)
    d.rectangle([12, 28, 52, 48], fill=WATER_BLUE)
    d.rectangle([12, 28, 52, 32], fill=WATER_LIGHT)  # Surface shimmer
    # Gauge on left
    d.rectangle([6, 12, 10, 48], fill=STEEL_DARK)
    d.rectangle([7, 28, 9, 48], fill=O2_BLUE)  # Water level indicator
    # Valves (top)
    d.ellipse([18, 6, 26, 14], fill=STEEL_LIGHT, outline=STEEL_DARK)
    d.ellipse([36, 6, 44, 14], fill=STEEL_LIGHT, outline=STEEL_DARK)
    # Valve handles
    d.line([(20, 10), (24, 10)], fill=ENGINE_RED, width=2)
    d.line([(38, 10), (42, 10)], fill=ENGINE_RED, width=2)
    # Pipes
    draw_pipe(d, 22, 4, 22, 10)
    draw_pipe(d, 40, 4, 40, 10)
    add_rust_spots(d, 10, 10, 44, 40, 6)
    return img


def gen_floodlight():
    """Submarine floodlight - lens, housing, beam."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Mount bracket
    draw_metal_plate(d, 20, 32, 24, 28, STEEL_MID)
    draw_rivets(d, 20, 32, 24, 28, 8)
    # Light housing
    d.rectangle([14, 18, 50, 36], fill=STEEL_DARK)
    d.rectangle([16, 20, 48, 34], fill=(40, 44, 50))
    # Lens
    d.ellipse([20, 20, 44, 34], fill=LIGHT_YELLOW)
    d.ellipse([24, 22, 40, 32], fill=LIGHT_WHITE)
    # Beam effect (subtle)
    for by in range(4, 18, 2):
        spread = (18 - by) // 2
        d.line([(32-spread-4, by), (32+spread+4, by)], fill=(*LIGHT_YELLOW, 80))
    # Mount bolts
    d.rectangle([22, 34, 24, 36], fill=RIVET_HIGHLIGHT)
    d.rectangle([40, 34, 42, 36], fill=RIVET_HIGHLIGHT)
    return img


def gen_sonar_array():
    """Active sonar - dish, electronics."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Sonar dish
    d.arc([10, 8, 54, 44], 200, 340, fill=SONAR_GREEN, width=3)
    d.arc([14, 12, 50, 40], 200, 340, fill=SONAR_GREEN, width=2)
    d.arc([18, 16, 46, 36], 200, 340, fill=SONAR_DIM, width=2)
    # Central emitter
    d.ellipse([28, 22, 36, 30], fill=SONAR_GREEN)
    d.ellipse([30, 24, 34, 28], fill=(60, 220, 120))
    # Electronics panel (bottom)
    draw_metal_plate(d, 10, 44, 44, 14, STEEL_DARK)
    # LEDs
    for lx in range(14, 50, 6):
        color = SONAR_GREEN if random.random() > 0.3 else (80, 30, 30)
        d.rectangle([lx, 48, lx+3, 51], fill=color)
    # Cable
    draw_pipe(d, 8, 26, 8, 44, horizontal=False)
    return img


def gen_passive_sonar():
    """Passive sonar - hydrophone array, quieter look."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, (40, 44, 52))
    # Hydrophone array (vertical bars)
    for hx in range(12, 52, 6):
        h = random.randint(20, 40)
        y_start = 32 - h // 2
        d.rectangle([hx, y_start, hx+3, y_start+h], fill=STEEL_DARK)
        d.rectangle([hx, y_start, hx+3, y_start+2], fill=SONAR_DIM)
    # Processing unit
    draw_metal_plate(d, 14, 48, 36, 10, STEEL_DARK)
    for lx in range(18, 46, 4):
        d.rectangle([lx, 50, lx+2, 53], fill=SONAR_DIM)
    return img


def gen_cargo_hold():
    """Cargo hold - crates, shelving."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 12)
    # Shelving
    d.rectangle([8, 20, 56, 22], fill=STEEL_DARK)
    d.rectangle([8, 38, 56, 40], fill=STEEL_DARK)
    # Crates
    d.rectangle([10, 8, 26, 20], fill=(90, 75, 50))
    d.rectangle([10, 10, 26, 12], fill=(100, 85, 60))
    d.rectangle([30, 10, 42, 20], fill=(80, 70, 45))
    # Lower crates
    d.rectangle([10, 24, 24, 36], fill=(85, 72, 48))
    d.rectangle([28, 26, 44, 36], fill=(75, 65, 42))
    # Bottom items
    d.rectangle([12, 42, 30, 54], fill=(70, 80, 60))
    d.rectangle([34, 44, 52, 54], fill=(90, 75, 50))
    # Straps
    d.line([(10, 14), (26, 14)], fill=WARNING_YELLOW)
    d.line([(10, 30), (24, 30)], fill=WARNING_YELLOW)
    add_rust_spots(d, 4, 4, 56, 56, 3)
    return img


def gen_medical_bay():
    """Medical bay - bed, cabinet, red cross."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Bed
    d.rectangle([8, 32, 40, 54], fill=STEEL_DARK)
    d.rectangle([10, 34, 38, 52], fill=(200, 200, 210))  # White sheet
    d.rectangle([10, 34, 16, 40], fill=(180, 190, 200))  # Pillow
    # Medical cabinet
    d.rectangle([44, 10, 56, 40], fill=STEEL_DARK)
    d.rectangle([46, 12, 54, 20], fill=(200, 200, 210))
    d.rectangle([46, 22, 54, 30], fill=(200, 200, 210))
    d.rectangle([46, 32, 54, 38], fill=(200, 200, 210))
    # Red cross
    d.rectangle([48, 8, 52, 10], fill=WEAPON_RED)  # Top
    d.rectangle([14, 10, 30, 28], fill=(200, 200, 210))
    d.rectangle([20, 12, 24, 26], fill=WEAPON_RED)  # Vertical
    d.rectangle([16, 16, 28, 20], fill=WEAPON_RED)  # Horizontal
    # Monitor
    d.rectangle([8, 8, 12, 20], fill=STEEL_DARK)
    d.rectangle([8, 10, 11, 18], fill=SONAR_GREEN)
    add_rust_spots(d, 4, 4, 56, 56, 2)
    return img


def gen_repair_station():
    """Repair station - tools, workbench."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Workbench
    d.rectangle([8, 30, 56, 34], fill=(80, 65, 45))
    d.rectangle([8, 34, 12, 54], fill=STEEL_DARK)
    d.rectangle([52, 34, 56, 54], fill=STEEL_DARK)
    # Tools on pegboard (top)
    draw_metal_plate(d, 8, 6, 48, 22, (50, 55, 60))
    # Wrench
    d.rectangle([14, 10, 16, 24], fill=STEEL_LIGHT)
    d.rectangle([12, 10, 18, 14], fill=STEEL_LIGHT)
    # Hammer
    d.rectangle([24, 12, 26, 24], fill=(100, 80, 55))
    d.rectangle([22, 10, 28, 14], fill=STEEL_LIGHT)
    # Welding torch
    d.rectangle([34, 10, 36, 24], fill=STEEL_DARK)
    d.rectangle([33, 8, 37, 12], fill=ENGINE_ORANGE)
    # Spare parts on bench
    d.rectangle([14, 36, 22, 42], fill=STEEL_DARK)
    d.rectangle([28, 36, 34, 40], fill=RUST_MID)
    draw_gauge(d, 46, 42, 5, 0.5, ENGINE_ORANGE)
    add_rust_spots(d, 4, 4, 56, 56, 3)
    return img


def gen_battery():
    """Battery bank - cells, terminals."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Battery cells (4 large cells)
    for bx, by in [(8, 10), (34, 10), (8, 34), (34, 34)]:
        d.rectangle([bx, by, bx+22, by+20], fill=STEEL_DARK)
        d.rectangle([bx+2, by+2, bx+20, by+18], fill=(50, 55, 65))
        # Terminal (+ -)
        d.rectangle([bx+4, by, bx+8, by+3], fill=WEAPON_RED)
        d.rectangle([bx+14, by, bx+18, by+3], fill=(40, 40, 180))
        # Charge level
        charge = random.randint(6, 16)
        d.rectangle([bx+4, by+14, bx+4+charge, by+16], fill=REACTOR_GLOW)
    # Connection cables
    draw_pipe(d, 20, 30, 44, 30)
    draw_warning_stripes(d, 4, 54, 56, 6)
    return img


def gen_navigation():
    """Navigation console - screens, controls."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Main screen
    d.rectangle([8, 8, 42, 32], fill=STEEL_DARK)
    d.rectangle([10, 10, 40, 30], fill=(15, 25, 40))
    # Grid on screen
    for gx in range(10, 40, 6):
        d.line([(gx, 10), (gx, 30)], fill=(25, 45, 65))
    for gy in range(10, 30, 5):
        d.line([(10, gy), (40, gy)], fill=(25, 45, 65))
    # Blip
    d.rectangle([24, 18, 27, 21], fill=SONAR_GREEN)
    # Side panel
    d.rectangle([44, 8, 56, 32], fill=STEEL_DARK)
    for ly in range(12, 28, 4):
        color = random.choice([SONAR_GREEN, REACTOR_GLOW, (80, 30, 30)])
        d.rectangle([46, ly, 54, ly+2], fill=color)
    # Control panel (bottom)
    draw_metal_plate(d, 8, 36, 48, 20, STEEL_DARK)
    # Buttons
    for bx in range(12, 52, 8):
        d.rectangle([bx, 40, bx+4, 44], fill=STEEL_LIGHT)
    # Joystick
    d.rectangle([28, 46, 34, 52], fill=STEEL_LIGHT)
    d.ellipse([29, 44, 33, 48], fill=(60, 65, 75))
    return img


def gen_docking_port():
    """Docking port - airlock, clamps."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 8)
    # Airlock ring
    d.ellipse([10, 10, 54, 54], fill=STEEL_DARK, outline=STEEL_LIGHT)
    d.ellipse([16, 16, 48, 48], fill=(35, 40, 48))
    d.ellipse([20, 20, 44, 44], fill=STEEL_DARK, outline=WARNING_YELLOW)
    # Door (center)
    d.rectangle([26, 20, 38, 44], fill=STEEL_MID)
    d.line([(32, 20), (32, 44)], fill=STEEL_DARK)  # Door split
    # Handle
    d.rectangle([30, 30, 34, 34], fill=STEEL_LIGHT)
    # Clamp indicators
    for angle, pos in [(0, (32, 12)), (90, (52, 32)), (180, (32, 52)), (270, (12, 32))]:
        d.rectangle([pos[0]-2, pos[1]-2, pos[0]+2, pos[1]+2], fill=SONAR_GREEN)
    draw_warning_stripes(d, 4, 4, 14, 8)
    draw_warning_stripes(d, 46, 4, 14, 8)
    return img


def gen_salvage_arm():
    """Salvage arm - mechanical claw."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Base mount
    draw_metal_plate(d, 20, 40, 24, 20, STEEL_MID)
    draw_rivets(d, 20, 40, 24, 20, 8)
    # Arm segments
    d.rectangle([28, 20, 34, 42], fill=STEEL_DARK)
    d.rectangle([30, 22, 32, 40], fill=STEEL_MID)
    # Shoulder joint
    d.ellipse([24, 18, 38, 28], fill=STEEL_LIGHT, outline=STEEL_DARK)
    # Upper arm
    d.rectangle([26, 6, 36, 20], fill=STEEL_DARK)
    # Claw
    d.polygon([(24, 4), (20, 0), (22, 0), (28, 4)], fill=STEEL_LIGHT)
    d.polygon([(38, 4), (42, 0), (40, 0), (34, 4)], fill=STEEL_LIGHT)
    # Hydraulic lines
    d.line([(22, 36), (22, 22)], fill=ENGINE_ORANGE, width=2)
    d.line([(40, 36), (40, 22)], fill=ENGINE_ORANGE, width=2)
    # Warning
    draw_warning_stripes(d, 20, 56, 24, 4)
    return img


def gen_mine_layer():
    """Mine layer - mine storage, deployment tube."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Mine storage (3 mines)
    for my in [10, 24, 38]:
        d.ellipse([12, my, 28, my+12], fill=STEEL_DARK, outline=WEAPON_DARK)
        d.ellipse([16, my+2, 24, my+10], fill=(50, 55, 62))
        # Detonator spikes
        d.rectangle([10, my+4, 14, my+8], fill=WEAPON_RED)
        d.rectangle([26, my+4, 30, my+8], fill=WEAPON_RED)
    # Deployment tube
    d.rectangle([36, 8, 56, 52], fill=STEEL_DARK)
    d.rectangle([38, 10, 54, 50], fill=(35, 38, 45))
    # Rails
    d.rectangle([40, 10, 42, 50], fill=STEEL_LIGHT)
    d.rectangle([50, 10, 52, 50], fill=STEEL_LIGHT)
    draw_warning_stripes(d, 36, 50, 20, 6)
    add_rust_spots(d, 4, 4, 56, 56, 4)
    return img


def gen_depth_sensor():
    """Depth/pressure sensor."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Main pressure gauge (large)
    d.ellipse([12, 8, 52, 48], fill=STEEL_DARK, outline=STEEL_LIGHT)
    d.ellipse([16, 12, 48, 44], fill=(15, 20, 28))
    # Gauge markings
    import math
    cx, cy, r = 32, 28, 14
    for i in range(12):
        a = math.radians(-210 + i * 25)
        x1 = cx + int(r * math.cos(a))
        y1 = cy + int(r * math.sin(a))
        x2 = cx + int((r-3) * math.cos(a))
        y2 = cy + int((r-3) * math.sin(a))
        d.line([(x1, y1), (x2, y2)], fill=STEEL_LIGHT)
    # Needle
    a = math.radians(-210 + 7 * 25)  # About 60% reading
    nx = cx + int(12 * math.cos(a))
    ny = cy + int(12 * math.sin(a))
    d.line([(cx, cy), (nx, ny)], fill=WEAPON_RED, width=1)
    d.ellipse([cx-2, cy-2, cx+2, cy+2], fill=STEEL_LIGHT)
    # Digital readout
    d.rectangle([14, 50, 50, 58], fill=STEEL_DARK)
    d.rectangle([16, 52, 48, 56], fill=(15, 40, 30))
    # Numbers
    for dx in range(18, 46, 6):
        d.rectangle([dx, 53, dx+3, 55], fill=SONAR_GREEN)
    return img


def gen_pump():
    """Water pump - impeller, pipes."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Pump body
    d.ellipse([14, 14, 50, 50], fill=STEEL_DARK, outline=STEEL_LIGHT)
    d.ellipse([20, 20, 44, 44], fill=(40, 45, 52))
    # Impeller
    cx, cy = 32, 32
    import math
    for i in range(6):
        a = math.radians(i * 60)
        ex = cx + int(10 * math.cos(a))
        ey = cy + int(10 * math.sin(a))
        d.line([(cx, cy), (ex, ey)], fill=STEEL_LIGHT, width=2)
    d.ellipse([cx-3, cy-3, cx+3, cy+3], fill=PIPE_MID)
    # Input/output pipes
    draw_pipe(d, 4, 30, 16, 30)
    draw_pipe(d, 48, 30, 60, 30)
    # Water drops
    d.rectangle([8, 22, 10, 24], fill=WATER_LIGHT)
    d.rectangle([54, 22, 56, 24], fill=WATER_LIGHT)
    add_rust_spots(d, 14, 14, 36, 36, 3)
    return img


def gen_reinforced_hull():
    """Reinforced hull panel - heavy plating."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Heavy outer plate
    draw_metal_plate(d, 0, 0, 64, 64, STEEL_MID, STEEL_DARK, STEEL_LIGHT)
    # Cross bracing
    d.rectangle([0, 30, 64, 34], fill=STEEL_DARK)
    d.rectangle([30, 0, 34, 64], fill=STEEL_DARK)
    # Heavy rivets
    for rx in range(6, 60, 10):
        for ry in range(6, 60, 10):
            d.ellipse([rx, ry, rx+3, ry+3], fill=RIVET_HIGHLIGHT)
    # Plate divisions
    d.line([(0, 31), (64, 31)], fill=STEEL_LIGHT)
    d.line([(0, 33), (64, 33)], fill=STEEL_DARK)
    d.line([(31, 0), (31, 64)], fill=STEEL_LIGHT)
    d.line([(33, 0), (33, 64)], fill=STEEL_DARK)
    add_rust_spots(d, 0, 0, 64, 64, 8)
    return img


# === HULL MATERIAL VARIANTS ===

def gen_hull_steel():
    """Standard steel hull segment."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 0, 0, 64, 64, STEEL_MID, STEEL_DARK, STEEL_LIGHT)
    draw_rivets(d, 0, 0, 64, 64, 8)
    # Weld seams
    d.line([(0, 32), (64, 32)], fill=(75, 82, 92))
    d.line([(32, 0), (32, 64)], fill=(75, 82, 92))
    add_rust_spots(d, 0, 0, 64, 64, 6)
    return img


def gen_hull_titanium():
    """Titanium hull - lighter, silver-blue."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    ti_dark = (75, 85, 100)
    ti_mid = (100, 115, 135)
    ti_light = (130, 145, 165)
    draw_metal_plate(d, 0, 0, 64, 64, ti_mid, ti_dark, ti_light)
    draw_rivets(d, 0, 0, 64, 64, 10)
    d.line([(0, 32), (64, 32)], fill=ti_light)
    d.line([(32, 0), (32, 64)], fill=ti_light)
    return img


def gen_hull_composite():
    """Composite hull - darker, carbon fiber pattern."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    comp_dark = (30, 32, 38)
    comp_mid = (45, 48, 55)
    comp_light = (60, 65, 72)
    draw_metal_plate(d, 0, 0, 64, 64, comp_mid, comp_dark, comp_light)
    # Carbon fiber pattern (diagonal weave)
    for i in range(0, 128, 4):
        d.line([(i, 0), (i-64, 64)], fill=comp_dark)
        d.line([(i-64, 0), (i, 64)], fill=comp_dark)
    # Edge seal
    d.rectangle([0, 0, 2, 64], fill=comp_light)
    d.rectangle([62, 0, 64, 64], fill=comp_dark)
    d.rectangle([0, 0, 64, 2], fill=comp_light)
    d.rectangle([0, 62, 64, 64], fill=comp_dark)
    return img


def gen_hull_abyssal():
    """Abyssal alloy hull - dark with subtle glow."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    aby_dark = (20, 22, 30)
    aby_mid = (30, 35, 48)
    aby_light = (45, 52, 68)
    draw_metal_plate(d, 0, 0, 64, 64, aby_mid, aby_dark, aby_light)
    draw_rivets(d, 0, 0, 64, 64, 12)
    # Subtle bioluminescent veins
    import math
    for _ in range(5):
        sx = random.randint(10, 54)
        sy = random.randint(10, 54)
        for step in range(8):
            nx = sx + random.randint(-3, 3)
            ny = sy + random.randint(-3, 3)
            nx = max(2, min(62, nx))
            ny = max(2, min(62, ny))
            d.line([(sx, sy), (nx, ny)], fill=(30, 80, 100), width=1)
            sx, sy = nx, ny
    return img


# === ADDITIONAL MODULES ===

def gen_life_support():
    """Advanced life support system."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # CO2 scrubber (left)
    d.rectangle([8, 10, 28, 40], fill=STEEL_DARK)
    draw_vent(d, 10, 12, 16, 26)
    # Water recycler (right)
    d.rectangle([34, 10, 56, 40], fill=STEEL_DARK)
    d.ellipse([38, 14, 52, 28], fill=(35, 40, 50))
    d.ellipse([42, 18, 48, 24], fill=WATER_LIGHT)
    # Pipes connecting
    draw_pipe(d, 28, 24, 34, 24)
    # Control panel
    draw_metal_plate(d, 10, 44, 44, 12, STEEL_DARK)
    d.rectangle([14, 47, 18, 51], fill=O2_LIGHT)
    d.rectangle([22, 47, 26, 51], fill=SONAR_GREEN)
    d.rectangle([30, 47, 34, 51], fill=REACTOR_GLOW)
    return img


def gen_shield_generator():
    """Energy shield generator."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Central emitter
    d.ellipse([16, 16, 48, 48], fill=STEEL_DARK, outline=STEEL_LIGHT)
    d.ellipse([22, 22, 42, 42], fill=(20, 25, 35))
    # Shield energy (blue rings)
    d.ellipse([24, 24, 40, 40], fill=None, outline=(60, 120, 200), width=1)
    d.ellipse([26, 26, 38, 38], fill=None, outline=(80, 150, 230), width=1)
    d.ellipse([28, 28, 36, 36], fill=None, outline=(100, 180, 255), width=1)
    # Core
    d.ellipse([30, 30, 34, 34], fill=(120, 180, 255))
    # Power conduits (4 corners)
    for px, py in [(8, 8), (52, 8), (8, 52), (52, 52)]:
        d.rectangle([px, py, px+6, py+6], fill=STEEL_DARK)
        d.rectangle([px+1, py+1, px+5, py+5], fill=(40, 80, 140))
    return img


def gen_comm_array():
    """Communications array - antenna, dish."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 28, 56, 32, STEEL_MID)
    draw_rivets(d, 4, 28, 56, 32, 10)
    # Antenna mast
    d.rectangle([30, 4, 34, 30], fill=STEEL_DARK)
    # Dish
    d.arc([14, 2, 50, 24], 200, 340, fill=STEEL_LIGHT, width=2)
    d.arc([18, 6, 46, 22], 200, 340, fill=STEEL_MID, width=2)
    # Signal waves
    for r in [4, 8, 12]:
        d.arc([32-r, 6-r, 32+r, 6+r], 250, 290, fill=SONAR_GREEN, width=1)
    # Electronics
    draw_metal_plate(d, 10, 36, 44, 18, STEEL_DARK)
    for lx in range(14, 50, 5):
        c = random.choice([SONAR_GREEN, REACTOR_GLOW, WEAPON_RED])
        d.rectangle([lx, 40, lx+2, 43], fill=c)
    return img


def gen_turret():
    """Heavy turret weapon."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    # Base platform
    draw_metal_plate(d, 8, 36, 48, 24, STEEL_MID)
    draw_rivets(d, 8, 36, 48, 24, 8)
    # Turret ring
    d.ellipse([14, 28, 50, 48], fill=STEEL_DARK, outline=STEEL_LIGHT)
    # Turret body
    d.rectangle([20, 20, 44, 38], fill=STEEL_DARK)
    d.rectangle([22, 22, 42, 36], fill=STEEL_MID)
    # Main cannon
    d.rectangle([28, 4, 36, 24], fill=STEEL_DARK)
    d.rectangle([30, 4, 34, 22], fill=(55, 60, 70))
    d.rectangle([28, 4, 36, 8], fill=STEEL_LIGHT)  # Muzzle
    # Targeting optic
    d.ellipse([30, 26, 34, 30], fill=WEAPON_RED)
    draw_warning_stripes(d, 8, 54, 48, 6)
    add_rust_spots(d, 8, 20, 48, 38, 3)
    return img


def gen_research_lab():
    """Research laboratory module."""
    img = new_sprite()
    d = ImageDraw.Draw(img)
    draw_metal_plate(d, 4, 4, 56, 56, STEEL_MID)
    draw_rivets(d, 4, 4, 56, 56, 10)
    # Microscope
    d.rectangle([10, 14, 14, 38], fill=STEEL_DARK)
    d.rectangle([8, 10, 16, 14], fill=STEEL_LIGHT)
    d.rectangle([8, 38, 20, 42], fill=STEEL_DARK)
    # Specimen tanks
    for tx in [24, 38]:
        d.rectangle([tx, 10, tx+12, 34], fill=STEEL_DARK)
        d.rectangle([tx+1, 12, tx+11, 32], fill=(20, 60, 50))
        # Specimen blob
        d.ellipse([tx+3, 18, tx+9, 26], fill=(40, 140, 100))
    # Console
    d.rectangle([8, 44, 56, 56], fill=STEEL_DARK)
    d.rectangle([10, 46, 30, 54], fill=(15, 25, 40))
    for lx in range(34, 54, 4):
        d.rectangle([lx, 48, lx+2, 52], fill=random.choice([SONAR_GREEN, O2_LIGHT]))
    return img


# === GENERATE ALL SPRITES ===

def main():
    modules = {
        "small_reactor": gen_small_reactor,
        "large_reactor": gen_large_reactor,
        "standard_engine": gen_standard_engine,
        "silent_drive": gen_silent_drive,
        "torpedo_tube": gen_torpedo_tube,
        "point_defense": gen_point_defense,
        "railgun": gen_railgun,
        "basic_quarters": gen_basic_quarters,
        "oxygen_scrubber": gen_oxygen_scrubber,
        "ballast_tank": gen_ballast_tank,
        "floodlight": gen_floodlight,
        "sonar_array": gen_sonar_array,
        "passive_sonar": gen_passive_sonar,
        "cargo_hold": gen_cargo_hold,
        "medical_bay": gen_medical_bay,
        "repair_station": gen_repair_station,
        "battery": gen_battery,
        "navigation": gen_navigation,
        "docking_port": gen_docking_port,
        "salvage_arm": gen_salvage_arm,
        "mine_layer": gen_mine_layer,
        "depth_sensor": gen_depth_sensor,
        "pump": gen_pump,
        "reinforced_hull": gen_reinforced_hull,
        "life_support": gen_life_support,
        "shield_generator": gen_shield_generator,
        "comm_array": gen_comm_array,
        "turret": gen_turret,
        "research_lab": gen_research_lab,
    }

    hull_variants = {
        "hull_steel": gen_hull_steel,
        "hull_titanium": gen_hull_titanium,
        "hull_composite": gen_hull_composite,
        "hull_abyssal": gen_hull_abyssal,
    }

    # Generate module sprites
    module_dir = os.path.join(OUT, "modules")
    os.makedirs(module_dir, exist_ok=True)
    for name, gen_func in modules.items():
        img = gen_func()
        path = os.path.join(module_dir, f"{name}.png")
        img.save(path)
        print(f"  [MODULE] {path}")

    # Generate hull sprites
    hull_dir = os.path.join(OUT, "hull")
    os.makedirs(hull_dir, exist_ok=True)
    for name, gen_func in hull_variants.items():
        img = gen_func()
        path = os.path.join(hull_dir, f"{name}.png")
        img.save(path)
        print(f"  [HULL]   {path}")

    total = len(modules) + len(hull_variants)
    print(f"\nGenerated {total} sprites total.")
    print(f"  Modules: {len(modules)}")
    print(f"  Hulls:   {len(hull_variants)}")


if __name__ == "__main__":
    main()
