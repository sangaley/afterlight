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

/// A halo's light, added onto what's under it (assets/shaders/module_glow.wgsl).
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
        let add = BlendState {
            color: BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
            alpha: BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(add);
            }
        }
        Ok(())
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
    mut materials: ResMut<Assets<GlowMaterial>>,
    modules: Query<(Entity, &Module, &Sprite), (Without<Glowing>, Without<DestroyedModule>)>,
    mut quads: Local<HashMap<(u32, u32), Handle<Mesh>>>,
    mut made: Local<HashMap<(String, ModuleType, u8), Handle<GlowMaterial>>>,
) {
    let masks = masked();
    for (entity, module, sprite) in &modules {
        let Some(size) = sprite.custom_size else { continue };
        let Some(path) = assets.get_path(sprite.image.id()) else { continue };
        let Some(stem) = path.path().file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
        commands.entity(entity).try_insert(Glowing);
        let Some((rgb, strength)) = glow_colour(module.module_type) else { continue };
        if !masks.contains(&stem) {
            continue;
        }
        let colour = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
        // The lights themselves dimmer on the quieter systems: a strong
        // colour drawn flat over a bright core reads as a bare bulb.
        let lit = {
            let l = colour.to_linear() * (0.25 + 0.75 * strength);
            Color::LinearRgba(LinearRgba { alpha: 1.0, ..l })
        };
        let big = size * (1.0 + 2.0 * PAD);
        let quad = quads.entry((big.x.to_bits(), big.y.to_bits())).or_insert_with(|| meshes.add(Rectangle::from_size(big))).clone();
        // Eight phases, so a ship's lights don't breathe together.
        let bucket = (entity.index_u32().wrapping_mul(2_654_435_761) >> 29) as u8;
        let halo = made
            .entry((stem.clone(), module.module_type, bucket))
            .or_insert_with(|| {
                let l = colour.to_linear() * (strength * halo_strength());
                materials.add(GlowMaterial {
                    colour: l,
                    breath: Vec4::new(bucket as f32 * 0.785, 0.22, 1.3, 0.0),
                    mask: assets.load(format!("sprites/module_glow/{stem}_glow.png")),
                })
            })
            .clone();
        let flip = |s: Sprite| Sprite { flip_x: sprite.flip_x, flip_y: sprite.flip_y, ..s };
        commands.entity(entity).with_children(|c| {
            // The lights recoloured...
            c.spawn((
                flip(Sprite { image: assets.load(format!("sprites/module_glow/{stem}_lit.png")), color: lit.with_alpha(0.85), custom_size: Some(big), ..default() }),
                Transform::from_xyz(0.0, 0.0, 0.003),
                GlowLight,
            ));
            // ...their light on the deck round them...
            let mut t = Transform::from_xyz(0.0, 0.0, 0.004);
            if sprite.flip_x {
                t.scale.x = -1.0;
            }
            if sprite.flip_y {
                t.scale.y = -1.0;
            }
            c.spawn((Mesh2d(quad), MeshMaterial2d(halo), t, GlowLight));
            // ...and the hottest of them white.
            c.spawn((
                flip(Sprite { image: assets.load(format!("sprites/module_glow/{stem}_core.png")), color: Color::srgb(1.0, 0.98, 0.93).with_alpha(0.7 * strength * strength), custom_size: Some(big), ..default() }),
                Transform::from_xyz(0.0, 0.0, 0.005),
                GlowLight,
            ));
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
