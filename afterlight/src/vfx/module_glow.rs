//! The modules' lights, glowing in their system's colour.
//!
//! The module art draws its lights in a few blues, greens and oranges, flat.
//! Each module with lights (tools/art/gen_module_glow.py found them) gets two
//! sprites over its art: the lights recoloured by what the module is, with a
//! halo spilling onto the deck round it, and a white-hot centre on the
//! brightest of them. Reactors burn gold, AI and research cores violet, the
//! medbay red, shields magenta, life support mint, sensors lime... so the
//! inside of a ship reads by colour, the way its outside does. The halo
//! breathes slowly, each module at its own pace, and goes out when the
//! module is destroyed.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey};

use crate::components::{DestroyedModule, Module, ModuleType};

/// The modules' art with masks in sprites/module_glow, by file stem.
fn masked() -> &'static HashSet<String> {
    static SET: std::sync::OnceLock<HashSet<String>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        std::fs::read_dir("assets/sprites/module_glow")
            .map(|dir| {
                dir.filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix("_glow.png")).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// A module's light colour (sRGB) and how strongly it glows; None keeps the
/// art's own lights and adds no glow.
///
/// Three tiers, so the eye has somewhere to land: the ship's heart (reactors,
/// bridge, shields, computers) burns brightest; what it fights and lives by
/// (guns, engines, medbay, power) sits in the middle; the quiet systems
/// (air, sensors, cooling, quarters) are a murmur, and storage is all but
/// dark -- a hold full of lit dots outshone the reactor. The warm hues are
/// held to three (reactor amber, power yellow, weapon red) so they still
/// say what each thing is.
pub fn glow_colour(t: ModuleType) -> Option<([u8; 3], f32)> {
    use ModuleType::*;
    const AMBER: [u8; 3] = [255, 184, 64];
    Some(match t {
        // The heart.
        SmallReactor | StandardReactor | LargeReactor | FusionReactor | RTG | ThermalVentGenerator => (AMBER, 1.0),
        MemoryCore | AutopilotCore | AICombatCore | TargetingComputer | ResearchLab => ([178, 104, 255], 0.9),
        ShieldEmitter | IonDisruptor | EMPPulse | StealthCoating | SignalJammer | EmergencyForceField | ContainmentField => ([255, 76, 196], 0.9),
        // Periwinkle: the bridge's cores are near-white in the lifted art,
        // so a white tint left them bare bulbs that outshone everything.
        NavigationConsole | HelmStation | BridgeWing | EngineeringStation => ([124, 136, 255], 0.75),
        DarkWard => ([132, 56, 255], 1.0),
        // What it fights and lives by.
        Cannon | Railgun | Coilgun | Gatling | Laser | PlasmaCaster | HeavyMissile | GuidedMissile | ClusterRocket | LaunchPad | RailLauncher | VlsSilo
        | AmmoAutoloader | LaserCore | LaserEmitter | DecoyLauncher | ChaffDispenser => ([255, 72, 52], 0.7),
        SmallEngine | StandardEngine | LargeEngine | SilentDrive | JetDrive | Afterburner | EngineNozzle => ([140, 186, 255], 0.7),
        MedBay | SurgicalBay | RepairBay | AdvancedRepairBay => ([255, 74, 104], 0.7),
        BatteryBank | Capacitor | PowerConduit | OverchargeCapacitor | Transformer | PowerRegulator | SolarCell => ([255, 226, 56], 0.6),
        // Fuel is the reactor's: its amber, a step down.
        FuelTank | FuelProcessor | FuelEnrichmentUnit | ReactorFuelRod => (AMBER, 0.6),
        Floodlight | Searchlight | DeepFloodlight => ([255, 244, 214], 0.8),
        // The quiet systems.
        OxygenScrubber | CO2Scrubber | AdvancedOxygenator | WasteRecycler | AirCirculator | OxygenTank | AtmosphereMonitor => ([72, 255, 186], 0.45),
        RadarArray | AdvancedRadar | PassiveRadar | DepthScanner | HydrophoneArray | ThermalImager | MissileWarner | CreatureScanner | MineralScanner
        | ProximityAlarm | Rangefinder | Periscope | SignalBuoy => ([176, 255, 72], 0.45),
        TractorBeam | SalvageArm | DockingPort | DockingHub | AirlockChamber | DroneBay | MiningDrill | MineralExtractor => ([60, 226, 226], 0.5),
        CoolingPump | HeatVent | CoolantRun | ReactorCooling | CoolingJacket => ([96, 176, 255], 0.45),
        CreatureContainment | SpecimenVault | CryoStorage => ([150, 255, 236], 0.5),
        // Cabin lamps: a soft, pale warm, not another orange.
        BasicQuarters | Barracks | OfficerQuarters | MessHall | GalleyMess | RecRoom | WellnessHub | TrainingRoom => ([255, 214, 172], 0.3),
        // Storage: all but dark.
        SmallCargo | LargeCargo | BulkCargoHold | AmmoBay | SupplyCrates => (AMBER, 0.15),
        _ => return None,
    })
}

/// A light pool, added onto what's under it (assets/shaders/module_glow.wgsl):
/// the path lights' (vfx::path_lights). Module halos use HaloMaterial.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct GlowMaterial {
    /// Linear colour, strength folded in.
    #[uniform(0)]
    pub colour: LinearRgba,
    /// x: phase, y: breath depth, z: breath speed.
    #[uniform(0)]
    pub breath: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub mask: Handle<Image>,
}

impl Material2d for GlowMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/module_glow.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _layout: &MeshVertexBufferLayoutRef, _key: Material2dKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        additive(descriptor);
        Ok(())
    }
}

/// Light adds onto what's under it, and leaves the alpha there alone.
fn additive(descriptor: &mut RenderPipelineDescriptor) {
    let add = BlendState {
        color: BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
        alpha: BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
    };
    if let Some(fragment) = descriptor.fragment.as_mut() {
        for target in fragment.targets.iter_mut().flatten() {
            target.blend = Some(add);
        }
    }
}

/// The modules' halos, all of them one material on one quad, so a ship's
/// halos draw together instead of one draw call each
/// (assets/shaders/module_halo.wgsl). Which mask, which colour and which
/// breath phase a halo has rides on its MeshTag (see `tag`).
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct HaloMaterial {
    /// Each module art's place in the packed masks: uv offset xy, size zw.
    #[uniform(0)]
    pub rects: [Vec4; MAX_ARTS],
    /// The light colours, linear, strength folded in.
    #[uniform(0)]
    pub colours: [Vec4; MAX_COLOURS],
    #[texture(1)]
    #[sampler(2)]
    pub mask: Handle<Image>,
}

const MAX_ARTS: usize = 128;
const MAX_COLOURS: usize = 64;

impl Material2d for HaloMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/module_halo.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/module_halo.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _layout: &MeshVertexBufferLayoutRef, _key: Material2dKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        additive(descriptor);
        Ok(())
    }
}

/// A halo's MeshTag: which art (bits 0-7), which colour (8-15), which of
/// eight breath phases (16-18).
pub fn tag(art: usize, colour: usize, phase: u8) -> u32 {
    art as u32 | (colour as u32) << 8 | ((phase & 7) as u32) << 16
}

/// One kind of glow art (the halo masks, the recoloured lights or the
/// white-hot cores) for every module, packed side by side into one picture,
/// so their sprites share a texture and batch.
pub struct Packed {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    /// Where each art sits, in uv (offset xy, size zw).
    pub uv: Vec<Vec4>,
}

/// Where each picture goes in a packed sheet, left to right in rows, with a
/// clear gap round each so filtering never bleeds a neighbour in. Returns
/// the sheet's size and each picture's corner.
pub fn pack(sizes: &[UVec2]) -> (UVec2, Vec<UVec2>) {
    const WIDTH: u32 = 2048;
    const GAP: u32 = 2;
    let (mut x, mut y, mut row, mut wide) = (GAP, GAP, 0, 0);
    let mut at = Vec::with_capacity(sizes.len());
    for size in sizes {
        if x + size.x + GAP > WIDTH && x > GAP {
            x = GAP;
            y += row + GAP;
            row = 0;
        }
        at.push(UVec2::new(x, y));
        x += size.x + GAP;
        wide = wide.max(x);
        row = row.max(size.y);
    }
    (UVec2::new(wide.max(1), (y + row + GAP).max(1)), at)
}

fn read_png(path: &str) -> Option<Image> {
    let bytes = std::fs::read(path).ok()?;
    let image = Image::from_buffer(
        &bytes,
        bevy::image::ImageType::Extension("png"),
        bevy::image::CompressedImageFormats::NONE,
        true,
        bevy::image::ImageSampler::Default,
        bevy::asset::RenderAssetUsages::default(),
    )
    .ok()?;
    (image.texture_descriptor.format == bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb && image.data.is_some()).then_some(image)
}

/// Packs `{art}_{kind}.png` for every art into one sheet. None if any of
/// them is missing or not plain RGBA, so the caller can fall back.
pub fn pack_kind(arts: &[String], kind: &str, images: &mut Assets<Image>, layouts: &mut Assets<TextureAtlasLayout>) -> Option<Packed> {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let pictures: Vec<Image> = arts.iter().map(|a| read_png(&format!("assets/sprites/module_glow/{a}_{kind}.png"))).collect::<Option<_>>()?;
    let sizes: Vec<UVec2> = pictures.iter().map(|p| p.size()).collect();
    let (sheet, at) = pack(&sizes);
    let mut out = Image::new_fill(
        Extent3d { width: sheet.x, height: sheet.y, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    let mut layout = TextureAtlasLayout::new_empty(sheet);
    let mut uv = Vec::with_capacity(arts.len());
    let data = out.data.as_mut()?;
    for ((picture, size), corner) in pictures.iter().zip(&sizes).zip(&at) {
        let src = picture.data.as_ref()?;
        let row = size.x as usize * 4;
        for y in 0..size.y as usize {
            let to = ((corner.y as usize + y) * sheet.x as usize + corner.x as usize) * 4;
            data[to..to + row].copy_from_slice(&src[y * row..(y + 1) * row]);
        }
        layout.add_texture(URect::from_corners(*corner, *corner + *size));
        let s = sheet.as_vec2();
        uv.push(Vec4::new(corner.x as f32 / s.x, corner.y as f32 / s.y, size.x as f32 / s.x, size.y as f32 / s.y));
    }
    Some(Packed { image: images.add(out), layout: layouts.add(layout), uv })
}

/// Everything the glow draws from: the three packed sheets, the one halo
/// material and quad, and which colour sits where in its table.
pub struct GlowKit {
    arts: HashMap<String, usize>,
    lit: Packed,
    core: Packed,
    halo: Handle<HaloMaterial>,
    quad: Handle<Mesh>,
    colours: Vec<[u32; 4]>,
}

impl GlowKit {
    fn build(images: &mut Assets<Image>, layouts: &mut Assets<TextureAtlasLayout>, meshes: &mut Assets<Mesh>, materials: &mut Assets<HaloMaterial>) -> Option<Self> {
        let mut arts: Vec<String> = masked().iter().cloned().collect();
        arts.sort();
        arts.truncate(MAX_ARTS);
        let glow = pack_kind(&arts, "glow", images, layouts)?;
        let lit = pack_kind(&arts, "lit", images, layouts)?;
        let core = pack_kind(&arts, "core", images, layouts)?;
        let mut rects = [Vec4::ZERO; MAX_ARTS];
        rects[..glow.uv.len()].copy_from_slice(&glow.uv);
        let halo = materials.add(HaloMaterial { rects, colours: [Vec4::ZERO; MAX_COLOURS], mask: glow.image });
        Some(Self {
            arts: arts.into_iter().enumerate().map(|(i, a)| (a, i)).collect(),
            lit,
            core,
            halo,
            quad: meshes.add(Rectangle::from_size(Vec2::ONE)),
            colours: Vec::new(),
        })
    }

    /// The colour's place in the halo material's table, adding it the first
    /// time it is seen; None once the table is full.
    fn colour(&mut self, light: LinearRgba, materials: &mut Assets<HaloMaterial>) -> Option<usize> {
        let key = [light.red, light.green, light.blue, light.alpha].map(f32::to_bits);
        if let Some(i) = self.colours.iter().position(|c| *c == key) {
            return Some(i);
        }
        if self.colours.len() >= MAX_COLOURS {
            return None;
        }
        let mut material = materials.get_mut(&self.halo)?;
        material.colours[self.colours.len()] = Vec4::new(light.red, light.green, light.blue, 1.0);
        self.colours.push(key);
        Some(self.colours.len() - 1)
    }
}

/// On a module once its glow is laid.
#[derive(Component)]
pub struct Glowing;

/// One of a module's glow sprites, taken away when the module is destroyed.
#[derive(Component)]
pub struct GlowLight;

/// How far past the module's art the halo reaches, each side (the masks'
/// PAD).
const PAD: f32 = 0.25;

/// How strongly the halos light the deck (AFTERLIGHT_GLOW sets it, for
/// tuning).
fn halo_strength() -> f32 {
    static G: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *G.get_or_init(|| std::env::var("AFTERLIGHT_GLOW").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0))
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn add_module_glow(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HaloMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    modules: Query<(Entity, &Module, &Sprite), (Without<Glowing>, Without<DestroyedModule>)>,
    mut kit: Local<Option<Option<GlowKit>>>,
) {
    if modules.is_empty() {
        return;
    }
    let kit = kit.get_or_insert_with(|| {
        let kit = GlowKit::build(&mut images, &mut layouts, &mut meshes, &mut materials);
        if kit.is_none() {
            warn!("module glow: couldn't pack the glow art, modules won't glow");
        }
        kit
    });
    let Some(kit) = kit.as_mut() else { return };
    for (entity, module, sprite) in &modules {
        let Some(size) = sprite.custom_size else { continue };
        let Some(path) = assets.get_path(sprite.image.id()) else { continue };
        let Some(stem) = path.path().file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
        commands.entity(entity).try_insert(Glowing);
        let Some((rgb, strength)) = glow_colour(module.module_type) else { continue };
        let Some(&art) = kit.arts.get(&stem) else { continue };
        let colour = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
        // The lights themselves dimmer on the quieter systems: a strong
        // colour drawn flat over a bright core reads as a bare bulb.
        let lit = {
            let l = colour.to_linear() * (0.25 + 0.75 * strength);
            Color::LinearRgba(LinearRgba { alpha: 1.0, ..l })
        };
        let Some(light) = kit.colour(colour.to_linear() * (strength * halo_strength()), &mut materials) else { continue };
        let big = size * (1.0 + 2.0 * PAD);
        // Eight phases, so a ship's lights don't breathe together.
        let phase = (entity.index_u32().wrapping_mul(2_654_435_761) >> 29) as u8;
        let packed = |p: &Packed, color: Color| Sprite {
            image: p.image.clone(),
            texture_atlas: Some(TextureAtlas { layout: p.layout.clone(), index: art }),
            color,
            custom_size: Some(big),
            flip_x: sprite.flip_x,
            flip_y: sprite.flip_y,
            ..default()
        };
        let lit_sprite = packed(&kit.lit, lit.with_alpha(0.85));
        let core_sprite = packed(&kit.core, Color::srgb(1.0, 0.98, 0.93).with_alpha(0.7 * strength * strength));
        let (quad, halo) = (kit.quad.clone(), kit.halo.clone());
        commands.entity(entity).with_children(|c| {
            // The lights recoloured...
            c.spawn((lit_sprite, Transform::from_xyz(0.0, 0.0, 0.003), GlowLight));
            // ...their light on the deck round them (the quad is one unit
            // across, scaled to the halo; a negative scale flips it with
            // the art)...
            let flip = Vec2::new(if sprite.flip_x { -1.0 } else { 1.0 }, if sprite.flip_y { -1.0 } else { 1.0 });
            let t = Transform::from_xyz(0.0, 0.0, 0.004).with_scale((big * flip).extend(1.0));
            c.spawn((Mesh2d(quad), MeshMaterial2d(halo), bevy::mesh::MeshTag(tag(art, light, phase)), t, GlowLight));
            // ...and the hottest of them white.
            c.spawn((core_sprite, Transform::from_xyz(0.0, 0.0, 0.005), GlowLight));
        });
    }
}

/// A destroyed module's lights go out.
pub fn put_out_module_glow(mut commands: Commands, dead: Query<&Children, Added<DestroyedModule>>, lights: Query<(), With<GlowLight>>) {
    for children in &dead {
        for child in children.iter() {
            if lights.contains(child) {
                commands.entity(child).try_despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_pictures_never_overlap_and_stay_on_the_sheet() {
        let sizes: Vec<UVec2> = (0..60).map(|i| UVec2::new(80 + (i * 37) % 161, 120 + (i * 53) % 121)).collect();
        let (sheet, at) = pack(&sizes);
        assert!(sheet.x <= 2048);
        let rects: Vec<URect> = at.iter().zip(&sizes).map(|(a, s)| URect::from_corners(*a, *a + *s)).collect();
        for (i, a) in rects.iter().enumerate() {
            assert!(a.max.x < sheet.x && a.max.y < sheet.y, "{a:?} off a {sheet:?} sheet");
            for b in &rects[i + 1..] {
                // A gap of at least one clear pixel between any two.
                let apart = a.max.x < b.min.x || b.max.x < a.min.x || a.max.y < b.min.y || b.max.y < a.min.y;
                assert!(apart, "{a:?} touches {b:?}");
            }
        }
    }

    #[test]
    fn the_packed_sheet_holds_each_art_exactly() {
        let mut arts: Vec<String> = masked().iter().cloned().collect();
        arts.sort();
        assert!(!arts.is_empty() && arts.len() <= MAX_ARTS, "{} glow arts", arts.len());
        let (mut images, mut layouts) = (Assets::<Image>::default(), Assets::<TextureAtlasLayout>::default());
        for kind in ["glow", "lit", "core"] {
            let packed = pack_kind(&arts, kind, &mut images, &mut layouts).expect(kind);
            let sheet = images.get(&packed.image).unwrap();
            let layout = layouts.get(&packed.layout).unwrap();
            let w = sheet.size().x as usize;
            for (i, art) in arts.iter().enumerate() {
                let own = read_png(&format!("assets/sprites/module_glow/{art}_{kind}.png")).unwrap();
                let r = layout.textures[i];
                assert_eq!(r.size(), own.size(), "{art}_{kind}");
                let (src, dst) = (own.data.as_ref().unwrap(), sheet.data.as_ref().unwrap());
                let row = r.width() as usize * 4;
                for y in 0..r.height() as usize {
                    let to = ((r.min.y as usize + y) * w + r.min.x as usize) * 4;
                    assert_eq!(&dst[to..to + row], &src[y * row..(y + 1) * row], "{art}_{kind} row {y}");
                }
                let uv = packed.uv[i] * Vec4::new(sheet.size().x as f32, sheet.size().y as f32, sheet.size().x as f32, sheet.size().y as f32);
                assert_eq!(uv.round(), Vec4::new(r.min.x as f32, r.min.y as f32, r.width() as f32, r.height() as f32));
            }
        }
    }

    #[test]
    fn a_tag_keeps_art_colour_and_phase_apart() {
        let t = tag(127, 63, 7);
        assert_eq!((t & 0xff, (t >> 8) & 0xff, (t >> 16) & 7), (127, 63, 7));
    }
}
