use bevy::prelude::*;
use crate::components::{Ship, Module, ModuleType, HullSegment, Projectile, DestroyedModule, HullDestroyed};
use crate::ai_ship::components::{AiShip, AiShipType};
use bevy::platform::collections::HashSet;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

// ============================================================================
// SHIP SHIELDS — Starsector-style visible shield bubble.
// Every ship carries a shield that absorbs incoming fire until depleted;
// it recharges after a few seconds without hits. Only once the shield is
// down do shots reach the hull and damage individual blocks.
// ============================================================================

#[derive(Component)]
pub struct ShipShield {
    pub current: f32,
    /// Effective capacity. For the player this is `base_max` scaled by routed
    /// shield power each frame (see update_shields); for AI ships it stays at
    /// base_max.
    pub max: f32,
    /// Capacity at nominal power — the emitter-derived value set at spawn.
    /// Power routing scales `max` around this; it never changes after spawn.
    pub base_max: f32,
    pub recharge_rate: f32,
    /// Seconds without taking a hit before recharge begins
    pub recharge_delay: f32,
    pub since_hit: f32,
    /// Collision radius of the bubble
    pub radius: f32,
    /// Bubble center in ship-local space (centroid of the blocks — the ship
    /// root is often at one end of the layout, not the middle)
    pub center_offset: Vec2,
    /// Visual hit-flash intensity, decays each frame
    pub flash: f32,
    /// Player toggle (R). AI shields are always enabled.
    pub enabled: bool,
    /// Recent-hit power surge — extra power drain that decays over time
    pub surge: f32,
    /// SAMPLE omni arc: world-angle the low-coverage arc currently points at.
    /// Auto-swings toward the biggest incoming threat (update_player_shield_arc).
    pub arc_facing: f32,
    /// Half-width of the arc, radians. Player: low coverage. AI: PI (unused).
    pub arc_half: f32,
    /// Max swing speed of the arc, rad/s — fast but not instant.
    pub arc_traverse: f32,
    /// True when a shot is about to hit: the segment is lit and blocking.
    /// Between/without incoming fire it's off (the outline goes dark).
    pub arc_active: bool,
}

impl ShipShield {
    pub fn absorb(&mut self, damage: f32) {
        self.current = (self.current - damage).max(0.0);
        self.since_hit = 0.0;
        self.flash = (self.flash + damage * SHIELD_FLASH_PER_DAMAGE).min(SHIELD_FLASH_MAX);
    }
    pub fn is_up(&self) -> bool {
        self.enabled && self.current > 0.0
    }
    /// Whether the omni arc currently covers a hit arriving from `hit_dir`
    /// (world-space direction from the bubble centre to the impact). Outside
    /// the arc the shot slips past to the hull — coverage is low, so the arc
    /// has to swing to face threats (see update_player_shield_arc).
    pub fn covers_arc(&self, hit_dir: Vec2) -> bool {
        let h = hit_dir.normalize_or_zero();
        if h == Vec2::ZERO {
            return true;
        }
        Vec2::from_angle(self.arc_facing).dot(h) >= self.arc_half.cos()
    }
    /// World-space center of the bubble for hit tests
    pub fn world_center(&self, transform: &Transform) -> Vec2 {
        transform.translation.truncate()
            + (transform.rotation * self.center_offset.extend(0.0)).truncate()
    }
}

/// DEPTHS_NO_SHIELD_FX=1 (filming): no shield outline glow and no facing
/// arcs. The shields still block; only their drawing is off.
fn shield_fx_hidden() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("DEPTHS_NO_SHIELD_FX").ok().as_deref() == Some("1"))
}

/// Marker for the bubble visual (child sprite of the shielded ship)
#[derive(Component)]
pub struct ShieldBubble;

/// On the player's ShieldBubble sprite: the unmasked full-outline glow, so
/// `update_shield_segment` can light only the chunk facing an incoming shot
/// (a moving piece of the silhouette). AI ships have no ShieldSegment — their
/// bubble shows whole.
#[derive(Component)]
pub(crate) struct ShieldSegment {
    /// Base outline-glow alpha, one byte per texel (row-major, w*h).
    base_a: Vec<u8>,
    w: usize,
    h: usize,
    /// The sprite's local centre and world size — for texel↔local mapping.
    center: Vec2,
    size: Vec2,
    /// The texels with any glow at all (most of the texture is empty), and
    /// each one's angle about `angles_pivot`. The mask only ever depends on
    /// that angle, so it is worked out once per pivot instead of an atan2 per
    /// texel per frame -- about 1.3 million a frame on a capital's skin.
    lit: Vec<u32>,
    angles: Vec<f32>,
    angles_pivot: Option<Vec2>,
    /// The (facing, half-width) the texture was last masked for; while the
    /// arc holds still the texture isn't touched (or re-uploaded) at all.
    masked_for: Option<(f32, f32)>,
}

/// Live blocks only: a block that has been shot off no longer shapes the
/// ship, and hull segments count as much as modules — an outer-hull-only
/// nose was invisible to the extent before, so the bubble (and the hull hit
/// radius derived from it) stopped short of the real bow.
pub type LiveBlockFilter = (
    Or<(With<Module>, With<HullSegment>)>,
    Without<DestroyedModule>,
    Without<HullDestroyed>,
);

/// Bubble geometry that actually wraps the ship: centered on the blocks'
/// centroid (the root is often at one end of the layout), radius = farthest
/// block from that centroid plus margin. Root-centered fixed radii produced
/// huge off-center bubbles with the ship poking out one side.
fn ship_extent(ship: Entity, blocks: &Query<(&Transform, &ChildOf), LiveBlockFilter>) -> (Vec2, f32) {
    extent_of(
        blocks.iter()
            .filter(|(_, p)| p.parent() == ship)
            .map(|(t, _)| t.translation.truncate()),
    )
}

fn extent_of(positions: impl Iterator<Item = Vec2>) -> (Vec2, f32) {
    let positions: Vec<Vec2> = positions.collect();
    if positions.is_empty() {
        return (Vec2::ZERO, 200.0);
    }
    let centroid = positions.iter().sum::<Vec2>() / positions.len() as f32;
    let max_dist = positions.iter()
        .map(|p| p.distance(centroid))
        .fold(0.0_f32, f32::max);
    (centroid, (max_dist + 70.0).max(150.0))
}

/// Keeps every shield's centre/radius tracking the ship's LIVE extent.
/// Both attach systems computed it once at spawn and never again, so a
/// ship shot to pieces kept its launch-day hittable radius — the ghost
/// hitbox that let shots "connect" with empty space where blocks used to
/// be (combat/projectiles.rs and check_projectile_hits both bound their
/// hull search on `shield.radius`).
///
/// Per ship, only when that ship's layout changed (building::layout). It
/// used to rescan every block in the world for every shield whenever any
/// block anywhere died.
pub fn refresh_shield_extents(
    blocks: Query<&Transform, LiveBlockFilter>,
    mut shields: Query<(Entity, &mut ShipShield, &crate::building::layout::LayoutVersion, Option<&Children>)>,
    epoch: Res<crate::building::layout::LayoutEpoch>,
    mut seen_epoch: Local<Option<crate::building::layout::LayoutEpoch>>,
    mut built: Local<crate::building::layout::BuiltFrom>,
) {
    if *seen_epoch == Some(*epoch) {
        return;
    }
    *seen_epoch = Some(*epoch);
    for (ship, mut shield, version, children) in shields.iter_mut() {
        if !built.is_stale(ship, version) {
            continue;
        }
        built.mark(ship, version);
        let positions = children
            .into_iter()
            .flat_map(|c| c.iter())
            .filter_map(|c| blocks.get(c).ok())
            .map(|t| t.translation.truncate());
        let (center, radius) = extent_of(positions);
        shield.center_offset = center;
        shield.radius = radius;
    }
    built.retain(|e| shields.contains(e));
}

/// The grid cells the ship occupies (module footprints + hull segments). Cell
/// (gx,gy) maps to ship-local (gx*66, gy*66-33), the same layout spawner.rs and
/// decompression.rs use. Drives the shield-skin perimeter trace.
fn ship_cells(
    ship: Entity,
    modules: &Query<(&Module, &ChildOf)>,
    hulls: &Query<(&HullSegment, &ChildOf)>,
) -> HashSet<IVec2> {
    let mut cells = HashSet::new();
    for (m, p) in modules.iter() {
        if p.parent() != ship { continue; }
        for dx in 0..m.size.x.max(1) {
            for dy in 0..m.size.y.max(1) {
                cells.insert(m.grid_position + IVec2::new(dx, dy));
            }
        }
    }
    for (h, p) in hulls.iter() {
        if p.parent() == ship { cells.insert(h.grid_position); }
    }
    cells
}

/// Separable box blur (clamped edges). Three passes approximate a Gaussian —
/// enough to erase the block-grid bumps so the shield contour reads as a single
/// smooth curve rather than a stepped outline.
///
/// A running sum, not a fresh sum per texel: each step adds the texel
/// entering the window and drops the one leaving it, so the cost is the same
/// whatever the radius. The direct version added 2r+1 = 39 texels for every
/// output texel, six passes per skin, and took ~190 ms on the starter -- a
/// visible hitch every time a block came off.
fn box_blur(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        blur_line(&src[y * w..(y + 1) * w], 1, &mut tmp[y * w..], 1, w, r);
    }
    let mut out = vec![0.0f32; w * h];
    for x in 0..w {
        blur_line(&tmp[x..], w, &mut out[x..], w, h, r);
    }
    out
}

/// One line of `box_blur`: `n` samples `stride` apart, edges clamped.
fn blur_line(src: &[f32], stride: usize, dst: &mut [f32], dst_stride: usize, n: usize, r: usize) {
    let at = |i: isize| src[(i.clamp(0, n as isize - 1) as usize) * stride];
    let norm = 1.0 / (2 * r + 1) as f32;
    let r = r as isize;
    let mut sum: f32 = (-r..=r).map(at).sum();
    for i in 0..n as isize {
        dst[i as usize * dst_stride] = sum * norm;
        sum += at(i + r + 1) - at(i - r);
    }
}

/// Build the shield skin as a SMOOTH glowing outline around the ship: rasterise
/// the filled cell silhouette, blur it heavily (killing the blocky bumps), then
/// draw a soft glow band at a coverage threshold. The blur is what makes it a
/// clean smooth shape hugging the hull — no grid steps, no 90° angles. The gap
/// off the hull just falls out of the blur amount. Returns (texture, local
/// centre, local size).
fn build_shield_skin(cells: &HashSet<IVec2>) -> Option<BuiltSkin> {
    const CELL: f32 = 66.0;
    const HALF: f32 = 33.0;
    const TPU: f32 = 0.25;          // texels per world unit (1 texel = 4 units)
    const SMOOTH: f32 = 76.0;       // blur radius in world units → smoothness + gap
    const THRESH: f32 = 0.18;       // coverage iso-value the glow sits on (lower = further out)
    const BANDW: f32 = 0.085;       // glow width in coverage space (smaller = thinner)
    if cells.is_empty() { return None; }
    let centers: Vec<Vec2> = cells.iter()
        .map(|c| Vec2::new(c.x as f32 * CELL, c.y as f32 * CELL - HALF))
        .collect();
    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    for c in &centers { lo = lo.min(*c); hi = hi.max(*c); }
    let r_tex = ((SMOOTH * TPU).round() as usize).max(1);
    let pad = 3.0 * SMOOTH + 8.0;   // room for the blurred halo
    let tmin = Vec2::new(lo.x - HALF - pad, lo.y - HALF - pad);
    let tmax = Vec2::new(hi.x + HALF + pad, hi.y + HALF + pad);
    let world = tmax - tmin;
    let w = ((world.x * TPU).ceil() as usize).max(1);
    let h = ((world.y * TPU).ceil() as usize).max(1);

    // Rasterise filled silhouette (1 inside a cell square, 0 outside).
    let mut cov = vec![0.0f32; w * h];
    for c in &centers {
        // world square -> texel rect (row 0 = top = +Y)
        let x0 = (((c.x - HALF) - tmin.x) * TPU).floor().max(0.0) as usize;
        let x1 = ((((c.x + HALF) - tmin.x) * TPU).ceil() as usize).min(w);
        let ry0 = (((tmax.y - (c.y + HALF)) * TPU).floor().max(0.0)) as usize;
        let ry1 = (((tmax.y - (c.y - HALF)) * TPU).ceil() as usize).min(h);
        for ty in ry0..ry1 {
            for tx in x0..x1 { cov[ty * w + tx] = 1.0; }
        }
    }
    cov = box_blur(&cov, w, h, r_tex);
    cov = box_blur(&cov, w, h, r_tex);
    cov = box_blur(&cov, w, h, r_tex);

    // Glow band at the coverage iso-contour (a smooth ring outside the hull).
    let mut data = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let d = (cov[i] - THRESH) / BANDW;
        let k = (-d * d).exp();     // gaussian band centred on THRESH
        if k > 0.02 {
            data[i * 4] = 255; data[i * 4 + 1] = 255; data[i * 4 + 2] = 255;
            data[i * 4 + 3] = (k * 255.0) as u8;   // white; sprite.color tints it
        }
    }
    // Keep the unmasked alpha so the player's segment mask can rebuild from it.
    let base_a: Vec<u8> = (0..w * h).map(|i| data[i * 4 + 3]).collect();
    let img = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default(),
    );
    let lit = base_a.iter().enumerate().filter(|(_, a)| **a > 0).map(|(i, _)| i as u32).collect();
    Some((img, (tmin + tmax) * 0.5, world, base_a, w, h, lit))
}

/// Spawn the shield skin as a single child sprite carrying the generated
/// distance-field texture. Marked ShieldBubble so `update_shields` drives its
/// alpha/flash exactly like the old bubble.
fn spawn_shield_skin(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    owner: Entity,
    cells: &HashSet<IVec2>,
    is_player: bool,
) {
    let Some(built) = build_shield_skin(cells) else { return };
    spawn_built_skin(commands, images, owner, built, is_player);
}

/// What build_shield_skin makes: (texture, local centre, local size, base
/// alpha per texel, width, height, the texels with any glow).
type BuiltSkin = (Image, Vec2, Vec2, Vec<u8>, usize, usize, Vec<u32>);

fn spawn_built_skin(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    owner: Entity,
    (img, center, size, base_a, w, h, lit): BuiltSkin,
    is_player: bool,
) {
    let handle = images.add(img);
    // Spawned from inside the command, so an owner gone by the time it runs
    // leaves no skin behind and no error.
    commands.entity(owner).queue_silenced(move |mut owner: EntityWorldMut| {
        owner.with_children(|skins| {
            let mut e = skins.spawn((
                Sprite {
                    image: handle,
                    color: Color::srgba(0.5, 0.8, 1.0, 0.0), // alpha driven by update_shields / update_shield_segment
                    custom_size: Some(size),
                    ..default()
                },
                Transform::from_xyz(center.x, center.y, 0.9),
                ShieldBubble,
            ));
            // The player's bubble is masked into a moving segment; store the base glow.
            if is_player {
                e.insert(ShieldSegment { base_a, w, h, center, size, lit, angles: Vec::new(), angles_pivot: None, masked_for: None });
            }
        });
    });
}

/// Attach a shield to the player ship (scaled by Shield Emitter modules).
pub fn attach_player_shield(
    mut commands: Commands,
    ship_query: Query<Entity, (With<Ship>, Without<ShipShield>)>,
    module_query: Query<(&Module, &ChildOf)>,
    transform_query: Query<(&Transform, &ChildOf), LiveBlockFilter>,
) {
    let Ok(ship) = ship_query.single() else { return };

    // Wait until the ship actually has modules so the extent is real
    let emitters = module_query.iter()
        .filter(|(m, p)| p.parent() == ship && m.module_type == ModuleType::ShieldEmitter && m.health > 0.0)
        .count() as f32;
    let module_count = module_query.iter().filter(|(_, p)| p.parent() == ship).count();
    if module_count == 0 { return; }

    let max = SHIELD_BASE_HP + emitters * SHIELD_HP_PER_EMITTER;
    let (center, radius) = ship_extent(ship, &transform_query);

    // try_: a load can replace the ship the frame this is queued.
    commands.entity(ship).try_insert(ShipShield {
        current: max,
        max,
        base_max: max,
        recharge_rate: 12.0,
        recharge_delay: 4.0,
        since_hit: 999.0,
        radius,
        center_offset: center,
        flash: 0.0,
        enabled: true,
        surge: 0.0,
        arc_facing: 0.0,
        arc_half: SHIELD_ARC_HALF,
        arc_traverse: SHIELD_ARC_TRAVERSE,
        arc_active: false,
    });
    // The silhouette-hugging skin itself is built (and kept up to date as blocks
    // change) by refresh_player_shield_skin. Collision stays radius-based via
    // `radius`/`center_offset` above — the skin is purely a visual.
}

/// Rebuild the player's shield skin whenever the ship's occupied-cell set
/// changes — a block placed (on the next launch), or blocks destroyed in a
/// fight. Cheap: it hashes the cell set every frame and only regenerates the
/// distance-field texture when that hash actually changes, so it costs nothing
/// while the ship is unchanged.
///
/// Only looks when the ship's layout has changed, and draws the new skin on a
/// background thread: rasterising and blurring it is the costliest thing a
/// block loss sets off (~19 ms on a 2,300-block capital), and done inline it
/// was a visible hitch every time. The old skin stays up until the new one is
/// ready, a frame or two later. At most one redraw is in flight; changes that
/// land meanwhile are picked up by the next.
pub fn refresh_player_shield_skin(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut last_hash: Local<u64>,
    mut seen: Local<Option<(Entity, crate::building::layout::LayoutVersion)>>,
    mut job: Local<Option<(Entity, bevy::tasks::Task<Option<BuiltSkin>>)>>,
    mut cooldown: Local<f32>,
    time: Res<Time<Real>>,
    ship_query: Query<(Entity, &crate::building::layout::LayoutVersion, &Children), (With<Ship>, With<ShipShield>)>,
    module_query: Query<&Module>,
    hull_query: Query<&HullSegment>,
    skin_query: Query<(Entity, &ChildOf), With<ShieldBubble>>,
) {
    *cooldown -= time.delta_secs();
    // A finished redraw replaces the player's skin.
    if let Some((owner, task)) = job.as_mut() {
        let Some(built) = bevy::tasks::futures::check_ready(task) else { return };
        let owner = *owner;
        *job = None;
        if let Some(built) = built {
            for (e, p) in skin_query.iter() {
                if p.parent() == owner { commands.entity(e).try_despawn(); }
            }
            spawn_built_skin(&mut commands, &mut images, owner, built, true);
        }
    }

    // A ship losing blocks every frame would otherwise keep a redraw running
    // without pause; twice a second keeps the skin close behind the hull.
    let Ok((ship, version, children)) = ship_query.single() else { return };
    if *seen == Some((ship, *version)) || *cooldown > 0.0 {
        return;
    }
    *seen = Some((ship, *version));
    *cooldown = 0.5;
    let mut cells = HashSet::new();
    for child in children.iter() {
        if let Ok(m) = module_query.get(child) {
            for dx in 0..m.size.x.max(1) {
                for dy in 0..m.size.y.max(1) {
                    cells.insert(m.grid_position + IVec2::new(dx, dy));
                }
            }
        } else if let Ok(h) = hull_query.get(child) {
            cells.insert(h.grid_position);
        }
    }
    if cells.is_empty() { return; }
    // order-independent hash of the cell set
    let hash = cells.iter().fold(0u64, |acc, c| {
        acc.wrapping_add(
            (c.x as i64 as u64).wrapping_mul(73856093)
                ^ (c.y as i64 as u64).wrapping_mul(19349663),
        )
    });
    if hash == *last_hash { return; }
    *last_hash = hash;
    let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { build_shield_skin(&cells) });
    *job = Some((ship, task));
}

/// Attach shields to AI ships as they spawn, sized by faction.
pub fn attach_ai_shields(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    ai_query: Query<(Entity, &AiShipType, &Children), (With<AiShip>, Without<ShipShield>)>,
    transform_query: Query<(&Transform, &ChildOf), LiveBlockFilter>,
    module_query: Query<(&Module, &ChildOf)>,
    hull_query: Query<(&HullSegment, &ChildOf)>,
) {
    // DEPTHS_MOVETEST_ENEMY: zero-HP shield on the test dummy. The shield
    // component is still what defines the ship's hittable radius/center in
    // check_projectile_hits (it's not just an HP pool) — dropping the
    // component entirely last time meant ai_ship_query's `&mut ShipShield`
    // requirement silently excluded the dummy from hit detection altogether,
    // so shots passed straight through with no collision at all. Zero HP
    // means is_up() is false (no absorption) while the geometry stays real.
    let no_shield_hp = std::env::var("DEPTHS_MOVETEST_ENEMY").ok().as_deref() == Some("1");

    for (entity, ship_type, children) in ai_query.iter() {
        // Wait until the AI ship's modules exist so the extent is real
        if !children.iter().any(|c| transform_query.get(c).is_ok()) { continue; }

        // Deliberately weak: the shield is a brief opener, the real fight is
        // carving up the hull block by block.
        let max = if no_shield_hp { 0.0 } else {
            let base = match ship_type {
                AiShipType::Shepherd => 140.0,    // the hardest kill in the game
                AiShipType::EternalHegemony => 100.0,  // mega-battleship
                AiShipType::TerranHegemony => 40.0,      // tanky battleship
                AiShipType::CorpseStars => 30.0,
                AiShipType::GildedThrone => 24.0,
                AiShipType::StellarPreserve => 22.0,
                AiShipType::SynthesisCollective => 20.0,
                AiShipType::TheSilence => 12.0,
                AiShipType::BrokenChoir => 6.0,        // half-dead ghost ships
                AiShipType::RecursiveKingdom => 4.0,      // junk ships, paper shields
                AiShipType::MerchantPrinces => 18.0,      // a guild pays for its cargo's safety
                AiShipType::FreeHaulersCompact => 10.0,   // whatever was salvaged and still works
            };
            base * AI_SHIELD_HP_MULT
        };
        let (center, radius) = ship_extent(entity, &transform_query);
        // try_: the ship can be filed away or destroyed the frame this is queued.
        commands.entity(entity).try_insert(ShipShield {
            current: max,
            max,
            base_max: max,
            recharge_rate: 8.0,
            recharge_delay: 5.0,
            since_hit: 999.0,
            radius,
            center_offset: center,
            flash: 0.0,
            enabled: true,
            surge: 0.0,
            arc_facing: 0.0,
            arc_half: AI_ARC_HALF, // directional: faces the player, flanks are open
            arc_traverse: AI_ARC_TRAVERSE,
            arc_active: false,
        });
        // Same silhouette skin as the player (built once — AI ships don't get
        // rebuilt in a hangar; a destroyed one just leaves its skin slightly wide).
        let cells = ship_cells(entity, &module_query, &hull_query);
        spawn_shield_skin(&mut commands, &mut images, entity, &cells, false);
    }
}

/// Passive power the player's shield draws while enabled. Hits add a surge
/// on top (see ShipShield::absorb) — sustained fire can push the ship's
/// power balance negative, which collapses the shield AND silences weapons.
pub const SHIELD_UPKEEP_POWER: f32 = 10.0;

/// Below this routed-power multiplier the player's shield is considered
/// browned out: it stops recharging and slowly bleeds, so cutting shield
/// power visibly drops the bubble (see update_shields / PowerChannels).
const SHIELD_BROWNOUT_MULT: f32 = 0.35;
/// How fast a browned-out shield bleeds charge (per second).
const SHIELD_BROWNOUT_BLEED: f32 = 6.0;

/// How much routed shield power flexes the bubble's max capacity, relative to
/// the channel's ×0..×2 power range. At full shield power (×2) capacity rises
/// by this fraction; at zero power it drops by it. Deliberately gentler than
/// the raw power multiplier — routing power mostly buys faster recharge, with
/// a modest capacity bump on top. 0.35 → up to +35% / −35% HP.
const SHIELD_HP_POWER_BONUS: f32 = 0.35;

/// SAMPLE omni arc: a low-coverage but tanky shield patch that auto-swings to
/// block the most damaging incoming shot. Half-width → ~110° of coverage; it
/// can rotate fast (but not instantly), so simultaneous fire from other angles
/// slips past to the hull until the arc catches up.
const SHIELD_ARC_HALF: f32 = 0.7; // ~40° half → ~80° lit "patch"
const SHIELD_ARC_SOFT: f32 = 0.22; // angular fade at the segment ends (radians)
const SHIELD_ARC_TRAVERSE: f32 = 11.0; // rad/s — fast, so it reaches the shot in time
const SHIELD_ARC_TRACK_RANGE: f32 = 1400.0 * super::gun_assembly::RANGE_SCALE; // pre-aim at incoming fire this far out
const SHIELD_ARC_ACTIVE_RANGE: f32 = 650.0 * super::gun_assembly::RANGE_SCALE; // segment lights up when a shot is this close

// --- Enemy (flanking) shields ---
/// Enemy shields are directional too — they face the player but swing slowly,
/// so their front is a wall and their flanks/rear are open. Maneuver around to
/// land real hits.
const AI_ARC_HALF: f32 = 1.15; // ~66° half → ~132° of front coverage
const AI_ARC_TRAVERSE: f32 = 2.6; // rad/s — slow enough to out-orbit
/// Bump enemy shield HP so the covered front is worth flanking (thin shields
/// made the front/flank difference invisible). Tunable.
const AI_SHIELD_HP_MULT: f32 = 2.2;

/// Hit glow: the outline flares on impact, brighter the more damage the hit
/// dealt (flash accumulates in absorb, decays each frame).
const SHIELD_FLASH_ALPHA: f32 = 0.55;
const SHIELD_FLASH_PER_DAMAGE: f32 = 0.03; // ~30 dmg → a strong flash
const SHIELD_FLASH_MAX: f32 = 1.6;

/// Player bubble capacity with zero emitters, and the HP each live Shield
/// Emitter module adds. Used both to size the shield at attach and to keep it
/// tracking the emitter count as blocks are built or destroyed. Nudged up ~25%
/// from the original 40/40 for a bit more overall bubble, on top of the routed
/// power flex.
const SHIELD_BASE_HP: f32 = 50.0;
const SHIELD_HP_PER_EMITTER: f32 = 50.0;

/// Toggle the player's shield with R. Dropping it saves power; raising it
/// again starts from whatever charge remains.
pub fn toggle_player_shield(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut shield_query: Query<&mut ShipShield, With<Ship>>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    if !keyboard.just_pressed(KeyCode::KeyR) { return; }
    let Ok(mut shield) = shield_query.single_mut() else { return };
    shield.enabled = !shield.enabled;
    notifications.write(crate::events::ShowNotification {
        message: if shield.enabled { "Shield raised".into() } else { "Shield lowered".into() },
        notification_type: crate::events::NotificationType::Info,
        duration: 1.5,
    });
}

/// Keeps the player's shield capacity in step with its live Shield Emitter
/// count, so building an emitter at the dock grows the bubble and losing one in
/// a fight shrinks it — instead of the capacity being frozen at first-launch.
/// Only `base_max` is touched here; update_shields then applies the routed
/// power HP flex on top and re-clamps `current`. AI shields keep their fixed
/// faction capacity (this only runs for the Ship-marked player shield).
pub fn refresh_player_shield_capacity(
    module_query: Query<(&Module, &ChildOf)>,
    mut shield_query: Query<(Entity, &mut ShipShield), With<Ship>>,
) {
    let Ok((ship, mut shield)) = shield_query.single_mut() else { return };
    let emitters = module_query
        .iter()
        .filter(|(m, p)| {
            p.parent() == ship && m.module_type == ModuleType::ShieldEmitter && m.health > 0.0
        })
        .count() as f32;
    let new_base = SHIELD_BASE_HP + emitters * SHIELD_HP_PER_EMITTER;
    if (shield.base_max - new_base).abs() > f32::EPSILON {
        shield.base_max = new_base;
        // Capacity may have dropped (emitter destroyed) — don't leave current
        // above it. The routed HP flex re-clamps against the scaled max too.
        shield.current = shield.current.min(shield.base_max);
    }
}

/// Pre-aims the player's shield segment at the most dangerous incoming shot and
/// flags it `arc_active` only when a shot is about to hit. The arc keeps
/// tracking (invisibly) out to TRACK_RANGE so it's already in position, then
/// lights up (via update_shield_segment) once the shot crosses ACTIVE_RANGE —
/// so it "pops on right where the bullet lands." Facing is world-space; the
/// mask converts it into the (rotating) hull frame.
pub fn update_player_shield_arc(
    time: Res<Time>,
    mut ship_query: Query<(Entity, &Transform, &mut ShipShield), With<Ship>>,
    projectiles: Query<(&Transform, &Projectile)>,
    // Ship guns, the AI's included, fire these; the legacy kind above is
    // only creatures' now.
    rounds: Query<(&Transform, &crate::combat::new_projectiles::Projectile, &crate::components::Velocity)>,
    owners: Query<&ChildOf>,
) {
    let dt = time.delta_secs();
    let Ok((player, ship_transform, mut shield)) = ship_query.single_mut() else { return };
    let center = shield.world_center(ship_transform);

    // Most dangerous hostile shot heading at us; remember how close it is.
    let mut best: Option<(f32, f32, Vec2)> = None; // (damage, dist, pos)
    let mut consider = |p: Vec2, heading: Vec2, damage: f32| {
        let to_ship = center - p;
        let dist = to_ship.length();
        if dist > SHIELD_ARC_TRACK_RANGE {
            return;
        }
        if heading.dot(to_ship.normalize_or_zero()) <= 0.0 {
            return; // already flying past us
        }
        let better = match best {
            None => true,
            Some((bd, bdist, _)) => damage > bd || (damage == bd && dist < bdist),
        };
        if better {
            best = Some((damage, dist, p));
        }
    };
    for (t, proj) in projectiles.iter() {
        if !proj.owner.is_player() {
            consider(t.translation.truncate(), proj.direction, proj.damage);
        }
    }
    for (t, proj, velocity) in rounds.iter() {
        let ours = owners.get(proj.owner).is_ok_and(|p| p.parent() == player);
        if !ours {
            consider(t.translation.truncate(), velocity.0.normalize_or_zero(), proj.damage);
        }
    }

    // Swing toward it (rate-limited), and light up once it's about to land.
    // A brief flash-driven linger keeps the segment on through the impact.
    let mut incoming_close = false;
    if let Some((_, dist, pos)) = best {
        incoming_close = dist <= SHIELD_ARC_ACTIVE_RANGE;
        let dir = pos - center;
        if dir.length_squared() > 1.0 {
            let ta = dir.y.atan2(dir.x);
            let mut diff = ta - shield.arc_facing;
            while diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            }
            while diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }
            let max_step = shield.arc_traverse * dt;
            shield.arc_facing += diff.clamp(-max_step, max_step);
        }
    }
    shield.arc_active = incoming_close || shield.flash > 0.05;
}

/// Enemy shields are directional: they slowly swing to face the player, so the
/// front is covered but the flanks and rear are open — maneuver around to land
/// real hits. Draws a faint arc marking each enemy's protected facing so you
/// can read where to flank.
pub fn update_ai_shield_arcs(
    time: Res<Time>,
    mut gizmos: Gizmos,
    player: Query<&Transform, (With<Ship>, Without<AiShip>)>,
    mut ai: Query<(&Transform, &mut ShipShield), (With<AiShip>, Without<Ship>)>,
) {
    let dt = time.delta_secs();
    let Ok(player_tf) = player.single() else { return };
    let player_pos = player_tf.translation.truncate();

    for (tf, mut shield) in ai.iter_mut() {
        let center = shield.world_center(tf);
        let dir = player_pos - center;
        if dir.length_squared() > 1.0 {
            let target = dir.y.atan2(dir.x);
            let mut diff = target - shield.arc_facing;
            while diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            }
            while diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }
            let step = (AI_ARC_TRAVERSE * dt).min(diff.abs()) * diff.signum();
            shield.arc_facing += step;
        }

        // Faint arc showing the protected facing, brighter just after a block.
        if shield.is_up() && !shield_fx_hidden() {
            let alpha = (0.12 + shield.flash * 0.5).min(0.8);
            let color = Color::srgba(0.5, 0.8, 1.0, alpha);
            let r = shield.radius;
            let start = shield.arc_facing - shield.arc_half;
            let sweep = shield.arc_half * 2.0;
            let segs = 20;
            let mut prev = center + Vec2::from_angle(start) * r;
            for i in 1..=segs {
                let a = start + sweep * (i as f32 / segs as f32);
                let p = center + Vec2::from_angle(a) * r;
                gizmos.line_2d(prev, p, color);
                prev = p;
            }
        }
    }
}

/// Lights only the chunk of the player's outline glow facing the tracked shot —
/// a moving piece of the silhouette rather than the whole ring. Rewrites the
/// bubble texture's alpha to the base glow masked by the active angular window
/// (converted into the hull's rotating frame), and only while `arc_active`;
/// otherwise the bubble is hidden. AI bubbles are untouched (no ShieldSegment).
pub fn update_shield_segment(
    mut images: ResMut<Assets<Image>>,
    ship_query: Query<(Entity, &Transform, &ShipShield), With<Ship>>,
    mut seg_query: Query<(&mut Sprite, &mut ShieldSegment, &ChildOf)>,
) {
    let Ok((player, ship_transform, shield)) = ship_query.single() else { return };

    for (mut sprite, mut seg, parent) in seg_query.iter_mut() {
        if parent.parent() != player {
            continue;
        }
        // Off unless a shot's about to hit.
        if !shield.is_up() || !shield.arc_active || shield_fx_hidden() {
            sprite.color = Color::srgba(0.5, 0.8, 1.0, 0.0);
            seg.masked_for = None;
            continue;
        }

        // World facing → local (the sprite rotates with the hull).
        let ship_rot = ship_transform.rotation.to_euler(EulerRot::ZYX).0;
        let local_facing = shield.arc_facing - ship_rot;
        let pivot = shield.center_offset; // block centroid, in ship-local space
        let half = shield.arc_half;
        let soft = SHIELD_ARC_SOFT;

        if seg.angles_pivot != Some(pivot) {
            let tmin = seg.center - seg.size * 0.5;
            let texel_w = seg.size.x / seg.w as f32;
            let texel_h = seg.size.y / seg.h as f32;
            let (w, size_y) = (seg.w, seg.size.y);
            let angles = seg.lit.iter().map(|&i| {
                let (tx, ty) = (i as usize % w, i as usize / w);
                let lx = tmin.x + (tx as f32 + 0.5) * texel_w;
                let ly = tmin.y + size_y - (ty as f32 + 0.5) * texel_h;
                (ly - pivot.y).atan2(lx - pivot.x)
            }).collect();
            seg.angles = angles;
            seg.angles_pivot = Some(pivot);
            seg.masked_for = None;
        }

        // Re-mask only when the arc has visibly moved (~0.1 degrees).
        let moved = seg.masked_for.is_none_or(|(f, h)| (f - local_facing).abs() > 0.002 || (h - half).abs() > 0.002);
        if moved {
            let Some(mut image) = images.get_mut(&sprite.image) else { continue };
            let Some(buf) = image.data.as_mut() else { continue };
            let seg = &mut *seg;
            for (&i, &ang) in seg.lit.iter().zip(seg.angles.iter()) {
                let i = i as usize;
                let mut d = (ang - local_facing).rem_euclid(std::f32::consts::TAU);
                if d > std::f32::consts::PI {
                    d = std::f32::consts::TAU - d;
                }
                let mask = if d <= half {
                    1.0
                } else if d >= half + soft {
                    0.0
                } else {
                    let t = (d - half) / soft;
                    1.0 - t * t
                };
                buf[i * 4 + 3] = (seg.base_a[i] as f32 * mask) as u8;
            }
            seg.masked_for = Some((local_facing, half));
        }

        // Overall brightness: visible while active, flaring on impact.
        let charge = if shield.max > 0.0 { shield.current / shield.max } else { 0.0 };
        let alpha = (0.35 + 0.2 * charge + shield.flash * SHIELD_FLASH_ALPHA).min(0.95);
        sprite.color = Color::srgba(0.5, 0.85, 1.0, alpha);
    }
}

/// Recharge shields after a quiet period; decay hit-flash; drive bubble alpha.
/// The shield is a plain health pool — its only tie to the power grid is the
/// flat upkeep drawn while raised (see update_power_system).
pub fn update_shields(
    time: Res<Time>,
    channels: Res<crate::resources::PowerChannels>,
    player_query: Query<Entity, With<Ship>>,
    mut shield_query: Query<(Entity, &mut ShipShield, &Children)>,
    // The player's bubble carries ShieldSegment and is driven by
    // update_shield_segment instead — exclude it here so we only paint AI bubbles.
    mut bubble_query: Query<&mut Sprite, (With<ShieldBubble>, Without<ShieldSegment>)>,
) {
    let dt = time.delta_secs();
    let player = player_query.single().ok();

    for (entity, mut shield, children) in shield_query.iter_mut() {
        shield.since_hit += dt;
        shield.flash = (shield.flash - dt * 3.0).max(0.0);

        // Only the player's shield answers to power routing; AI shields run at
        // their fixed rate.
        let power_mult = if Some(entity) == player { channels.shields_mult } else { 1.0 };

        // Routed power gently flexes the player's max capacity around base_max
        // (a ×2 power channel → only ~+35% HP), then clamp current to it.
        if Some(entity) == player {
            let hp_factor = 1.0 + (power_mult - 1.0) * SHIELD_HP_POWER_BONUS;
            shield.max = shield.base_max * hp_factor;
            shield.current = shield.current.min(shield.max);
        }

        if power_mult < SHIELD_BROWNOUT_MULT {
            // Starved: the bubble can't hold — no recharge, and it bleeds down.
            shield.current = (shield.current - SHIELD_BROWNOUT_BLEED * dt).max(0.0);
        } else if shield.enabled
            && shield.since_hit > shield.recharge_delay
            && shield.current < shield.max
        {
            shield.current =
                (shield.current + shield.recharge_rate * power_mult * dt).min(shield.max);
        }

        // Ship-outline skin: visible while up (scaled by charge) and it glows
        // brighter on impact the more damage the hit dealt (flash accumulates
        // in absorb, decays above). Player and AI alike.
        for child in children.iter() {
            if let Ok(mut sprite) = bubble_query.get_mut(child) {
                let alpha = if shield.is_up() && !shield_fx_hidden() {
                    let charge = if shield.max > 0.0 { shield.current / shield.max } else { 0.0 };
                    (0.12 + 0.18 * charge + shield.flash * SHIELD_FLASH_ALPHA).min(0.9)
                } else {
                    0.0
                };
                sprite.color = Color::srgba(0.5, 0.8, 1.0, alpha);
            }
        }
    }
}

#[cfg(test)]
mod blur_tests {
    use super::*;

    /// The direct sum the running sum replaced, kept as the reference.
    fn box_blur_direct(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
        let ri = r as i32;
        let norm = 1.0 / (2 * r + 1) as f32;
        let mut tmp = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let s: f32 = (-ri..=ri).map(|k| src[y * w + (x as i32 + k).clamp(0, w as i32 - 1) as usize]).sum();
                tmp[y * w + x] = s * norm;
            }
        }
        let mut out = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let s: f32 = (-ri..=ri).map(|k| tmp[(y as i32 + k).clamp(0, h as i32 - 1) as usize * w + x]).sum();
                out[y * w + x] = s * norm;
            }
        }
        out
    }

    #[test]
    fn the_running_sum_matches_the_direct_blur() {
        // A blocky silhouette, like the skin rasterises, including radii
        // wider than the image (all clamped edge).
        for (w, h, r) in [(37, 23, 4), (64, 40, 19), (9, 5, 19), (1, 1, 3)] {
            let src: Vec<f32> = (0..w * h).map(|i| if (i * 7 + i / w * 3) % 5 < 2 { 1.0 } else { 0.0 }).collect();
            let fast = box_blur(&src, w, h, r);
            let slow = box_blur_direct(&src, w, h, r);
            for (a, b) in fast.iter().zip(&slow) {
                assert!((a - b).abs() < 1e-4, "{w}x{h} r={r}: {a} vs {b}");
            }
        }
    }
}
