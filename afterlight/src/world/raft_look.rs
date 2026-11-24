//! Station pieces drawn the way the game draws a ship.
//!
//! The playtester (2026-11-24) wanted the city round a station -- its moored
//! ships, arms, hubs and sections -- to look like the ships they fly with: the
//! roof shell over the blocks (vfx::ship_roof) in faction paint, not the
//! domed gunmetal painting world::raft_art makes. A city is still pictures,
//! not blocks (Haven alone would be tens of thousands of blocks): each design
//! is laid out once on a hidden stand-in -- its blocks' art and the very roof
//! a ship of it would get -- its sprites are read back as stamps, and every
//! picture of it is painted over from those stamps on a thread, the same way
//! the far view paints a ship (vfx::far_art). The pictures keep their size and
//! their lights layer, so nothing else about the city changes.
//!
//! AFTERLIGHT_STATION_LOOK=painted keeps the old painting.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use bevy::prelude::*;

use super::raft::{design, faction_of, library, Design, CELL};
use super::raft_art::{BakeKey, RaftBakes};
use crate::building::registry::ModuleRegistry;
use crate::components::{HullLayer, HullMaterial, HullSegment, Module, ModuleType, Rotation};
use crate::vfx::far_art::{Canvas, Mip, Stamp};
use crate::vfx::ship_roof::{faction_paint, lay_roof, Paintwork, Scars};

/// Whether the city is drawn as ships are (the default) or painted.
pub fn ship_look() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("AFTERLIGHT_STATION_LOOK").map_or(true, |v| v != "painted"))
}

/// What a look is laid out for: a design in a paint (station_style theme id,
/// 0 for Haven's own metal).
type LookKey = (&'static str, u8);

/// One sprite of a laid-out design, kept after the stand-in is gone.
struct LookStamp {
    at: Vec2,
    angle: f32,
    size: Vec2,
    rect: Rect,
    flip_x: bool,
    flip_y: bool,
    tint: LinearRgba,
    art: Arc<Mip>,
    source: AssetId<Image>,
    z: f32,
}

/// A design laid out on a stand-in, waiting for its art to load.
struct Staged {
    stand_in: Entity,
    frames: u32,
    /// The block art under the roof, by path.
    blocks: Vec<(Handle<Image>, &'static str, (u8, u8, u8, u8), Vec2, f32)>,
}

#[derive(Resource, Default)]
pub struct ShipLooks {
    staged: HashMap<LookKey, Staged>,
    ready: HashMap<LookKey, Arc<Vec<LookStamp>>>,
    queue: VecDeque<LookKey>,
    queued: HashSet<LookKey>,
    /// Pictures already painted over (or being), by image.
    painted: HashSet<AssetId<Image>>,
    done: Arc<Mutex<Vec<(Handle<Image>, Image)>>>,
    mips: HashMap<AssetId<Image>, Arc<Mip>>,
    /// Frames since the last stand-in was laid out.
    since_stage: u32,
}

/// Where the stand-in is laid out: far off anywhere a camera goes.
const STAGE_AT: Vec3 = Vec3::new(9.0e7, 9.0e7, -50.0);
/// Frames a stand-in waits for its art before it's read as it is.
const MAX_WAIT: u32 = 240;
/// Frames between stand-ins, and how many may wait at once: laying a big
/// design's roof is thousands of sprites spawned and let go, and a city's
/// worth at one a frame made arriving at the Gilded capital run at 5-10 fps
/// for seconds (perf session, 2026-11-24). The old pictures show meanwhile.
const STAGE_EVERY: u32 = 4;
const MAX_STAGED: usize = 2;

/// The paint a look is laid out in: a ship in its own faction's, the
/// station's own metal in its owner's, Haven's in a plain gunmetal.
fn paint_for(key: LookKey) -> Paintwork {
    let seed = key.0.bytes().fold(7u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
    let faction = if library().contains_key(key.0) {
        super::raft_life::faction_named(faction_of(key.0))
    } else {
        key.1.checked_sub(1).and_then(|i| super::station_style::OWNERS.get(i as usize)).copied()
    };
    let Some(faction) = faction else {
        // Haven: nobody's colours, the gunmetal the Raft was always painted.
        let mut work = faction_paint(crate::ai_ship::components::AiShipType::TerranHegemony);
        work.scheme.main = Vec3::new(0.40, 0.42, 0.47);
        work.scheme.second = Vec3::new(0.24, 0.25, 0.29);
        work.scheme.trim = Vec3::new(0.85, 0.62, 0.3);
        work.livery.base = Color::srgb(0.40, 0.42, 0.47);
        work.livery.accent = Color::srgb(0.85, 0.62, 0.3);
        return work;
    };
    let mut work = faction_paint(faction);
    if crate::vfx::ship_roof::cosmo() {
        let u8s = |c: Color| {
            let s = c.to_srgba();
            [(s.red * 255.0) as u8, (s.green * 255.0) as u8, (s.blue * 255.0) as u8]
        };
        let palette = crate::vfx::paint_gen::faction_palette(seed, u8s(work.livery.base), u8s(work.livery.accent));
        work.scheme = palette.scheme();
        work.generate = Some((seed, palette));
    }
    work
}

/// The hull and parts of a design, as a ship of it would have them.
fn blocks_of(key: &'static str, d: &Design, registry: &ModuleRegistry) -> (Vec<HullSegment>, Vec<Module>) {
    let hull_seg = |at: IVec2, layer: HullLayer, material: HullMaterial| HullSegment {
        health: 1.0,
        max_health: 1.0,
        radiation_shielding: 0.0,
        is_depressurized: false,
        depressurization_level: 0.0,
        hull_layer: layer,
        material,
        grid_position: at,
    };
    let module = |t: ModuleType, at: IVec2, rotation: Rotation| Module {
        module_type: t,
        health: 1.0,
        max_health: 1.0,
        power_consumption: 0.0,
        power_generation: 0.0,
        is_active: true,
        grid_position: at,
        size: registry.get(t).size,
        rotation,
    };
    if library().contains_key(key) {
        if let Some(bp) = crate::building::blueprint::load_design_file(format!("designs/factions/{key}.json")) {
            let hull = bp.hull_cells.iter().map(|h| hull_seg(h.grid_pos, h.layer, h.material)).collect();
            let mods = bp
                .modules
                .iter()
                .filter(|m| !m.extras.as_ref().is_some_and(|e| e.turret.is_some()))
                .filter(|m| registry.defs.contains_key(&m.module_type))
                .map(|m| module(m.module_type, m.grid_pos, m.rotation))
                .collect();
            return (hull, mods);
        }
    }
    // A built part (world::sections): its cells, its parts where their art
    // names one.
    static BY_SPRITE: OnceLock<HashMap<&'static str, ModuleType>> = OnceLock::new();
    let by_sprite = BY_SPRITE.get_or_init(|| {
        registry.defs.keys().filter_map(|t| crate::sprite_map::module_sprite_path(*t).map(|p| (p, *t))).collect()
    });
    let mut hull = Vec::new();
    let mut mods = Vec::new();
    for c in &d.cells {
        // The hull's own art says what it is: a walkway, or plate of a metal.
        let (layer, material) = match c.sprite {
            "sprites/hull/hull_hallway.png" => (HullLayer::Hallway, HullMaterial::Titanium),
            "sprites/hull/hull_steel.png" => (HullLayer::Inner, HullMaterial::Steel),
            "sprites/hull/hull_composite.png" => (HullLayer::Inner, HullMaterial::Composite),
            _ => (HullLayer::Inner, HullMaterial::Titanium),
        };
        hull.push(hull_seg(c.at, if c.outer { HullLayer::Outer } else { layer }, material));
        if c.module && c.part.0 == 0 && c.part.1 + 1 == c.part.3 {
            if let Some(t) = c.kind.as_ref().or_else(|| by_sprite.get(c.sprite)) {
                if registry.get(*t).size == IVec2::new(c.part.2 as i32, c.part.3 as i32) {
                    mods.push(module(*t, c.at, Rotation::North));
                }
            }
        }
    }
    (hull, mods)
}

/// Lay out looks the city wants, read back the ones whose art has loaded,
/// and paint the city's pictures over from them.
#[allow(clippy::too_many_arguments)]
pub fn paint_ship_looks(
    mut commands: Commands,
    assets: Res<AssetServer>,
    registry: Option<Res<ModuleRegistry>>,
    bakes: Option<Res<RaftBakes>>,
    mut looks: ResMut<ShipLooks>,
    mut images: ResMut<Assets<Image>>,
    kids: Query<&Children>,
    sprites: Query<(&Sprite, &Transform)>,
    placed: Query<&Transform>,
) {
    if !ship_look() {
        return;
    }
    let (Some(bakes), Some(registry)) = (bakes, registry) else { return };
    let looks = &mut *looks;

    // Pictures painted on the threads go in.
    let finished: Vec<(Handle<Image>, Image)> = looks.done.lock().map(|mut d| d.drain(..).collect()).unwrap_or_default();
    for (handle, image) in finished {
        let _ = images.insert(&handle, image);
    }

    // Every picture of the city not yet painted over: from a look, if it's
    // ready; otherwise the look is asked for.
    let mut want: Vec<(BakeKey, Handle<Image>, Vec2, LookKey)> = Vec::new();
    for (key, body, size) in bakes.pictures() {
        if looks.painted.contains(&body.id()) {
            continue;
        }
        want.push((key, body.clone(), size, (key.0, key.2)));
    }
    for (key, body, size, look) in want {
        let Some(stamps) = looks.ready.get(&look).cloned() else {
            if looks.queued.insert(look) {
                looks.queue.push_back(look);
            }
            continue;
        };
        let Some(old) = images.get(&body) else { continue };
        let (w, h) = (old.width(), old.height());
        looks.painted.insert(body.id());
        let out = looks.done.clone();
        std::thread::spawn(move || {
            let image = paint(&stamps, key.1, w, h, size);
            if let Ok(mut d) = out.lock() {
                d.push((body, image));
            }
        });
    }
    // Pictures let go (sharp ones the camera left) are forgotten.
    let live: HashSet<AssetId<Image>> = bakes.pictures().map(|(_, h, _)| h.id()).collect();
    looks.painted.retain(|id| live.contains(id));

    // A new stand-in every few frames, a couple at a time.
    looks.since_stage += 1;
    let free = looks.since_stage >= STAGE_EVERY && looks.staged.len() < MAX_STAGED;
    if let Some(look) = if free { looks.queue.pop_front() } else { None } {
        looks.since_stage = 0;
        if let Some(d) = design(look.0) {
            let (hull, mods) = blocks_of(look.0, d, &registry);
            let stand_in = commands.spawn((Transform::from_translation(STAGE_AT), Visibility::Hidden)).id();
            let hull_refs: Vec<&HullSegment> = hull.iter().collect();
            let mod_refs: Vec<&Module> = mods.iter().collect();
            lay_roof(&mut commands, &assets, stand_in, (6.5, Quat::IDENTITY, 1.0), &hull_refs, &mod_refs, &Scars::default(), &paint_for(look), false);
            // The blocks' own art under the roof: what shows through it.
            let blocks = d
                .cells
                .iter()
                .map(|c| (assets.load(c.sprite), c.sprite, c.part, crate::building::grid_to_local(c.at), if c.module { 0.5 } else { 0.0 }))
                .collect();
            looks.staged.insert(look, Staged { stand_in, frames: 0, blocks });
        }
    }

    // Stand-ins whose art has all loaded are read back and let go.
    let keys: Vec<LookKey> = looks.staged.keys().copied().collect();
    for look in keys {
        let Some(st) = looks.staged.get_mut(&look) else { continue };
        st.frames += 1;
        if st.frames < 3 {
            continue;
        }
        // Everything laid under the stand-in, placed in its frame.
        let mut roof: Vec<(&Sprite, Transform)> = Vec::new();
        let mut walk: Vec<(Entity, Transform)> = vec![(st.stand_in, Transform::IDENTITY)];
        while let Some((at, frame)) = walk.pop() {
            for k in kids.get(at).map(|c| c.iter().collect::<Vec<_>>()).unwrap_or_default() {
                let Ok((sprite, t)) = sprites.get(k) else {
                    if let Ok(t) = placed.get(k) {
                        walk.push((k, frame.mul_transform(*t)));
                    }
                    continue;
                };
                let t = frame.mul_transform(*t);
                roof.push((sprite, t));
                walk.push((k, t));
            }
        }
        let default_art = Handle::<Image>::default().id();
        let loaded = |h: &Handle<Image>| h.id() == default_art || images.get(h).is_some();
        let all_in = roof.iter().all(|(s, _)| loaded(&s.image)) && st.blocks.iter().all(|b| loaded(&b.0));
        if !all_in && st.frames < MAX_WAIT {
            continue;
        }
        let Some(d) = design(look.0) else { continue };
        let centre = Vec2::new((d.min.x + d.max.x) as f32, (d.min.y + d.max.y) as f32) * 0.5 * CELL - Vec2::new(0.0, 33.0);
        let mut mip_of = |h: &Handle<Image>| -> Option<Arc<Mip>> {
            let id = h.id();
            if id == default_art {
                return Some(Arc::new(Mip::white()));
            }
            if let Some(m) = looks.mips.get(&id) {
                return Some(m.clone());
            }
            let image = images.get(id)?;
            image.data.as_ref()?;
            let (w, h) = (image.width(), image.height());
            let m = Arc::new(Mip::build(w, h, |x, y| image.get_color_at(x, y).map(|c| c.to_linear()).unwrap_or(LinearRgba::NONE)));
            looks.mips.insert(id, m.clone());
            Some(m)
        };
        let mut stamps: Vec<LookStamp> = Vec::new();
        for (handle, _, part, at, z) in &st.blocks {
            let Some(art) = mip_of(handle) else { continue };
            let src = art.source_size();
            let (pc, pr, pw, ph) = (part.0 as f32, part.1 as f32, part.2.max(1) as f32, part.3.max(1) as f32);
            let rect = Rect::new(pc * src.x / pw, pr * src.y / ph, (pc + 1.0) * src.x / pw, (pr + 1.0) * src.y / ph);
            stamps.push(LookStamp {
                at: *at - centre,
                angle: 0.0,
                size: Vec2::splat(CELL),
                rect,
                flip_x: false,
                flip_y: false,
                tint: LinearRgba::WHITE,
                art,
                source: handle.id(),
                z: *z,
            });
        }
        for (sprite, t) in &roof {
            let Some(art) = mip_of(&sprite.image) else { continue };
            let src = art.source_size();
            let rect = sprite.rect.unwrap_or(Rect::new(0.0, 0.0, src.x, src.y));
            stamps.push(LookStamp {
                at: t.translation.truncate() - centre,
                angle: t.rotation.to_euler(EulerRot::ZYX).0,
                size: sprite.custom_size.unwrap_or(rect.size()) * t.scale.truncate().abs(),
                rect,
                flip_x: sprite.flip_x,
                flip_y: sprite.flip_y,
                tint: sprite.color.to_linear(),
                art,
                source: sprite.image.id(),
                z: 2.0 + t.translation.z,
            });
        }
        stamps.sort_by(|a, b| a.z.total_cmp(&b.z));
        commands.entity(st.stand_in).despawn();
        looks.staged.remove(&look);
        looks.ready.insert(look, Arc::new(stamps));
    }
}

/// A design's picture turned `quarter` quarter-turns, `w` x `h` pixels over
/// `size` world units centred on the design, from its laid-out stamps.
fn paint(stamps: &[LookStamp], quarter: u8, w: u32, h: u32, size: Vec2) -> Image {
    let upp = size.x / w as f32;
    let mut canvas = Canvas { origin: -size * 0.5, w, h, px: vec![LinearRgba::NONE; (w * h) as usize], upp };
    let turn = quarter as f32 * std::f32::consts::FRAC_PI_2;
    for s in stamps {
        let stamp = Stamp {
            at: super::raft::turn(s.at, quarter),
            angle: s.angle + turn,
            size: s.size,
            rect: s.rect,
            flip_x: s.flip_x,
            flip_y: s.flip_y,
            tint: s.tint,
            burn: 0.0,
            art: &s.art,
            source: s.source,
            z: s.z,
        };
        canvas.paint(&stamp);
    }
    canvas.rim();
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in (0..h).rev() {
        for x in 0..w {
            let c = canvas.px[(y * w + x) as usize];
            let straight = if c.alpha > 0.0 {
                LinearRgba::new(c.red / c.alpha, c.green / c.alpha, c.blue / c.alpha, c.alpha.min(1.0))
            } else {
                LinearRgba::NONE
            };
            bytes.extend_from_slice(&Color::LinearRgba(straight).to_srgba().to_u8_array());
        }
    }
    super::raft_art::image(w as usize, h as usize, bytes)
}
