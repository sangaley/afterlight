# Afterlight — Art Brief

## The game in one line
A 2D top-down space-survival game about building a modular block ship, keeping it alive,
and fighting other block ships in a hostile void — industrial and grim, more *Barotrauma*
than *Star Wars*.

**Tone words:** utilitarian, industrial, cold, isolated, quietly ominous.
**Reference games:** Cosmoteer (ship construction feel), FTL (module readability),
Barotrauma (mood/tone), Heat Signature (minimalist space readability).

---

## Hard technical constraints

| Constraint | Spec |
|---|---|
| Format | PNG with transparency (sprite sheets welcome for animations) |
| View | Top-down 2D. Sprites are rotated freely by the engine |
| Lighting | Neutral / flat top-light only — **no baked directional shadows** (rotation breaks them) |
| Grid cell | 1 module cell = **128×128 px**. Multi-cell modules scale accordingly (2×1 = 256×128, 2×2 = 256×256, 3×2 = 384×256, 3×3 = 384×384) |
| Brightness | Keep base art **mid-bright**. The engine darkens sprites for damage states and turns wrecks dark grey — art that starts dark loses its damage feedback |
| Silhouette | Modules must be distinguishable at ~32 px on screen (players zoom far out in combat). Strong shape language > fine detail |
| Margins | Leave ~4 px transparent padding inside the canvas so rotation doesn't clip |

---

## Palette guidance

Desaturated blues/greys for hulls and void. **Saturation is reserved for meaning:**

- Orange/red — fire, explosions, damage, danger
- Electric blue — EMP, shields, energy weapons
- Yellow — kinetic tracers, caution
- Green — friendly UI, success

Color = information, not decoration. Combat must stay readable when twenty things explode at once.

---

## Asset list, in priority order

### 1. Hull tiles (highest priority)
Flat armor plates that tile seamlessly on the grid.
- 3 material tiers (basic steel → reinforced → advanced alloy), visually escalating
- Each tier: clean + damaged/cracked variant
- 128×128 px each

### 2. Ship modules (~15 to start)
All 1×1 (128×128) unless noted. Each needs a distinct silhouette:
- Small Reactor, Large Reactor (2×1)
- Standard Engine (visible nozzle — engine sprites imply thrust direction)
- Cannon, Railgun (long barrel), Coilgun, Gatling (multi-barrel) — turret-style, will be rotated
- Laser emitter, Missile bay (visible tubes)
- Cooling Pump, Heat Vent (radiator fins)
- Oxygen Scrubber, Cargo Hold, Crew Quarters, Ammo Bay (clearly "this explodes")

### 3. Effects sprite sheets
- Explosion: 6–8 frames, ~256×256
- Muzzle flash: 2–3 frames, kinetic + energy variants
- Fire/burning overlay that can sit on top of any module tile (looping, 4+ frames)
- Smoke puff (for burning wrecks losing cargo)
- Shield bubble (soft rim, mostly transparent center — engine handles the fading)
- Debris chunks: 4–6 small irregular scrap shapes, ~24–48 px (engine spawns and tints these)

### 4. Projectiles
Small, bright, readable at speed (~24–64 px long):
- Kinetic shell, railgun slug (long streak), gatling tracer
- Missile (with visible exhaust), plasma bolt, ion bolt
- The engine tints rounds per ammo type (brass AP, blue EMP, orange incendiary…) — design them tint-friendly (light/neutral base)

### 5. UI icons
Monochrome-friendly, ~64×64:
- One icon per module category: Power, Propulsion, Weapons, Life Support, Utility, Structural, Control, Crew
- 9 ammo-type icons: AP, APHE, HE-Frag, Incendiary, EMP, Flak, HEAT, HESH, APFSDS

### 6. Later (don't start yet)
- Creatures (ambient void life + hostiles)
- Station exteriors
- Planets/asteroids (currently procedurally generated — may stay that way)

---

## Delivery notes
- Consistent scale across all modules — they sit on the same grid next to each other
- Source files (Aseprite/PSD) appreciated alongside PNGs so tiers/variants can be derived
- Start with ONE hull tile + ONE weapon + the explosion sheet as a style test before
  committing to the full set
