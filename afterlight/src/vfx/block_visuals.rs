use bevy::prelude::*;
use crate::components::*;
use crate::building::multiblock::components::*;

// ============================================================================
// BLOCK VISUALS — CLEAR & DISTINCT
// Rule: identify any block at first glance. Shape > color > detail.
// Max 3-4 child sprites per block. Strong silhouettes, high contrast.
// ============================================================================

#[derive(Component)]
pub struct HasBlockVisual;

pub fn attach_block_visuals(
    mut commands: Commands,
    module_query: Query<(Entity, &Module, Option<&MachineBlock>), Without<HasBlockVisual>>,
) {
    for (entity, module, _) in module_query.iter() {
        commands.entity(entity).insert(HasBlockVisual);
        build_visual(&mut commands, entity, module.module_type);
    }
}

fn s(commands: &mut Commands, parent: Entity, size: Vec2, color: Color, pos: Vec3) {
    let child = commands.spawn((Sprite { color, custom_size: Some(size), ..default() }, Transform::from_translation(pos))).id();
    commands.entity(parent).add_child(child);
}

fn build_visual(commands: &mut Commands, e: Entity, mt: ModuleType) {
    match mt {
        // ====== WEAPON CORES — each has a unique silhouette ======

        // Cannon: dark base + short wide barrel stub
        ModuleType::Cannon => {
            s(commands, e, Vec2::splat(44.0), Color::srgb(0.30, 0.22, 0.18), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(22.0, 10.0), Color::srgb(0.50, 0.35, 0.25), Vec3::new(16.0, 0.0, 0.02));
        }
        // Railgun: long narrow body with blue rails
        ModuleType::Railgun => {
            s(commands, e, Vec2::new(54.0, 28.0), Color::srgb(0.20, 0.22, 0.32), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(48.0, 3.0), Color::srgb(0.4, 0.5, 0.9), Vec3::new(0.0, 7.0, 0.02));
            s(commands, e, Vec2::new(48.0, 3.0), Color::srgb(0.4, 0.5, 0.9), Vec3::new(0.0, -7.0, 0.02));
        }
        // Coilgun: body with visible coil rings
        ModuleType::Coilgun => {
            s(commands, e, Vec2::new(46.0, 30.0), Color::srgb(0.22, 0.28, 0.38), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(4.0, 26.0), Color::srgb(0.45, 0.50, 0.75), Vec3::new(-8.0, 0.0, 0.02));
            s(commands, e, Vec2::new(4.0, 26.0), Color::srgb(0.45, 0.50, 0.75), Vec3::new(4.0, 0.0, 0.02));
            s(commands, e, Vec2::new(4.0, 26.0), Color::srgb(0.45, 0.50, 0.75), Vec3::new(16.0, 0.0, 0.02));
        }
        // Gatling: cluster of small barrels
        ModuleType::Gatling => {
            s(commands, e, Vec2::splat(40.0), Color::srgb(0.28, 0.22, 0.20), Vec3::new(0.0, 0.0, 0.01));
            for i in [-6.0, 0.0, 6.0] {
                s(commands, e, Vec2::new(18.0, 3.0), Color::srgb(0.55, 0.42, 0.30), Vec3::new(8.0, i, 0.02));
            }
        }
        // Laser: bright green lens in dark housing
        ModuleType::Laser => {
            s(commands, e, Vec2::new(42.0, 36.0), Color::srgb(0.15, 0.25, 0.18), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(18.0), Color::srgb(0.3, 0.8, 0.4), Vec3::new(6.0, 0.0, 0.02));
        }
        // Plasma: orange glow center
        ModuleType::PlasmaCaster => {
            s(commands, e, Vec2::new(42.0, 38.0), Color::srgb(0.28, 0.18, 0.12), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(20.0), Color::srgb(0.85, 0.50, 0.15), Vec3::new(0.0, 0.0, 0.02));
        }
        // Ion: purple glow rings
        ModuleType::IonDisruptor => {
            s(commands, e, Vec2::new(40.0, 36.0), Color::srgb(0.20, 0.15, 0.30), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(22.0), Color::srgb(0.50, 0.30, 0.75), Vec3::new(0.0, 0.0, 0.02));
        }
        // Torpedo/Missile: two dark tube openings
        ModuleType::HeavyMissile | ModuleType::GuidedMissile | ModuleType::ClusterRocket => {
            s(commands, e, Vec2::new(46.0, 38.0), Color::srgb(0.32, 0.20, 0.16), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(38.0, 10.0), Color::srgb(0.10, 0.08, 0.08), Vec3::new(2.0, 7.0, 0.02));
            s(commands, e, Vec2::new(38.0, 10.0), Color::srgb(0.10, 0.08, 0.08), Vec3::new(2.0, -7.0, 0.02));
        }
        // Mining drill: tapered point
        ModuleType::MiningDrill => {
            s(commands, e, Vec2::new(40.0, 30.0), Color::srgb(0.38, 0.32, 0.20), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(16.0, 12.0), Color::srgb(0.60, 0.50, 0.30), Vec3::new(18.0, 0.0, 0.02));
        }
        // Tractor beam: dish shape
        ModuleType::TractorBeam => {
            s(commands, e, Vec2::splat(38.0), Color::srgb(0.20, 0.32, 0.42), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(12.0), Color::srgb(0.40, 0.60, 0.80), Vec3::new(4.0, 0.0, 0.02));
        }
        // EMP: concentric rings
        ModuleType::EMPPulse => {
            s(commands, e, Vec2::splat(42.0), Color::srgb(0.22, 0.18, 0.32), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(24.0), Color::srgb(0.40, 0.28, 0.60), Vec3::new(0.0, 0.0, 0.02));
        }

        // ====== EXTENSION BLOCKS ======

        // Barrel: long thin tube — THE defining shape
        ModuleType::BarrelExtension => {
            s(commands, e, Vec2::new(54.0, 14.0), Color::srgb(0.38, 0.30, 0.24), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(50.0, 4.0), Color::srgb(0.15, 0.12, 0.10), Vec3::new(0.0, 0.0, 0.02));
        }
        // Ammo feed: box with dots (rounds)
        ModuleType::AmmoFeedUnit => {
            s(commands, e, Vec2::splat(40.0), Color::srgb(0.35, 0.28, 0.18), Vec3::new(0.0, 0.0, 0.01));
            for i in [-8.0, 0.0, 8.0] {
                s(commands, e, Vec2::splat(6.0), Color::srgb(0.65, 0.50, 0.20), Vec3::new(i, 0.0, 0.02));
            }
        }
        // Cooling: horizontal fins
        ModuleType::CoolingJacket | ModuleType::ReactorCooling => {
            s(commands, e, Vec2::splat(42.0), Color::srgb(0.15, 0.28, 0.40), Vec3::new(0.0, 0.0, 0.01));
            for i in [-12.0, -4.0, 4.0, 12.0] {
                s(commands, e, Vec2::new(36.0, 3.0), Color::srgb(0.25, 0.45, 0.65), Vec3::new(0.0, i, 0.02));
            }
        }
        // Muzzle brake: body with vent cuts
        ModuleType::MuzzleBrake => {
            s(commands, e, Vec2::new(36.0, 18.0), Color::srgb(0.35, 0.28, 0.24), Vec3::new(0.0, 0.0, 0.01));
            for i in [-8.0, 0.0, 8.0] {
                s(commands, e, Vec2::new(3.0, 14.0), Color::srgb(0.12, 0.10, 0.08), Vec3::new(i, 0.0, 0.02));
            }
        }
        // Magnetic accelerator: coil rings around bore
        ModuleType::MagneticAccelerator => {
            s(commands, e, Vec2::new(44.0, 28.0), Color::srgb(0.20, 0.22, 0.35), Vec3::new(0.0, 0.0, 0.01));
            for i in [-12.0, 0.0, 12.0] {
                s(commands, e, Vec2::new(4.0, 24.0), Color::srgb(0.40, 0.45, 0.75), Vec3::new(i, 0.0, 0.02));
            }
        }
        // Focusing array: bright lens
        ModuleType::FocusingArray => {
            s(commands, e, Vec2::splat(40.0), Color::srgb(0.16, 0.26, 0.22), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(20.0), Color::srgb(0.30, 0.60, 0.45), Vec3::new(0.0, 0.0, 0.02));
        }
        // Warhead bay: box with warning stripes
        ModuleType::WarheadBay => {
            s(commands, e, Vec2::splat(42.0), Color::srgb(0.32, 0.18, 0.14), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(36.0, 4.0), Color::srgb(0.80, 0.60, 0.10), Vec3::new(0.0, 10.0, 0.02));
            s(commands, e, Vec2::new(36.0, 4.0), Color::srgb(0.80, 0.60, 0.10), Vec3::new(0.0, -10.0, 0.02));
        }

        // ====== REACTORS — glowing center, size varies ======

        ModuleType::SmallReactor => {
            s(commands, e, Vec2::splat(44.0), Color::srgb(0.28, 0.26, 0.16), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(16.0), Color::srgb(0.60, 0.55, 0.12), Vec3::new(0.0, 0.0, 0.02));
        }
        ModuleType::StandardReactor => {
            s(commands, e, Vec2::splat(48.0), Color::srgb(0.30, 0.28, 0.16), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(22.0), Color::srgb(0.70, 0.62, 0.10), Vec3::new(0.0, 0.0, 0.02));
        }
        ModuleType::LargeReactor | ModuleType::FusionReactor => {
            s(commands, e, Vec2::splat(52.0), Color::srgb(0.32, 0.30, 0.16), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(28.0), Color::srgb(0.80, 0.70, 0.10), Vec3::new(0.0, 0.0, 0.02));
        }
        // Fuel rod: thin vertical glow
        ModuleType::ReactorFuelRod => {
            s(commands, e, Vec2::new(18.0, 44.0), Color::srgb(0.28, 0.26, 0.14), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(8.0, 38.0), Color::srgb(0.70, 0.65, 0.10), Vec3::new(0.0, 0.0, 0.02));
        }
        // Enrichment: spinning centrifuge look
        ModuleType::FuelEnrichmentUnit => {
            s(commands, e, Vec2::splat(42.0), Color::srgb(0.32, 0.28, 0.10), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(30.0, 3.0), Color::srgb(0.55, 0.50, 0.10), Vec3::new(0.0, 0.0, 0.02));
        }
        // Containment: blue ring
        ModuleType::ContainmentField => {
            s(commands, e, Vec2::splat(44.0), Color::srgb(0.18, 0.22, 0.38), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(28.0), Color::srgba(0.30, 0.40, 0.70, 0.5), Vec3::new(0.0, 0.0, 0.02));
        }
        // Emergency shutdown: big red button
        ModuleType::EmergencyShutdown => {
            s(commands, e, Vec2::splat(38.0), Color::srgb(0.28, 0.22, 0.20), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(20.0), Color::srgb(0.85, 0.18, 0.10), Vec3::new(0.0, 0.0, 0.02));
        }

        // ====== ENGINES — nozzle shape ======

        ModuleType::SmallEngine | ModuleType::StandardEngine | ModuleType::LargeEngine => {
            s(commands, e, Vec2::new(44.0, 34.0), Color::srgb(0.18, 0.28, 0.40), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(14.0, 28.0), Color::srgb(0.25, 0.35, 0.50), Vec3::new(-16.0, 0.0, 0.02));
        }
        // Nozzle extension: tapered
        ModuleType::EngineNozzle => {
            s(commands, e, Vec2::new(48.0, 26.0), Color::srgb(0.20, 0.30, 0.42), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(10.0, 20.0), Color::srgb(0.08, 0.10, 0.15), Vec3::new(-20.0, 0.0, 0.02));
        }
        // Afterburner: orange injection ports
        ModuleType::Afterburner => {
            s(commands, e, Vec2::new(42.0, 30.0), Color::srgb(0.30, 0.25, 0.16), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(6.0, 22.0), Color::srgb(0.80, 0.45, 0.10), Vec3::new(8.0, 0.0, 0.02));
        }
        // ====== DEFENSE — shield blue, armor gray ======

        ModuleType::ShieldEmitter => {
            s(commands, e, Vec2::splat(40.0), Color::srgb(0.18, 0.25, 0.40), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(34.0, 8.0), Color::srgb(0.30, 0.55, 0.85), Vec3::new(0.0, 10.0, 0.02));
        }
        ModuleType::AblativeArmor => {
            s(commands, e, Vec2::splat(48.0), Color::srgb(0.42, 0.40, 0.38), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(42.0, 2.0), Color::srgb(0.55, 0.52, 0.48), Vec3::new(0.0, 0.0, 0.02));
        }
        ModuleType::DecoyLauncher => {
            s(commands, e, Vec2::new(38.0, 38.0), Color::srgb(0.35, 0.30, 0.20), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(10.0), Color::srgb(0.70, 0.55, 0.15), Vec3::new(0.0, 0.0, 0.02));
        }

        // ====== UTILITY — unique per type ======

        ModuleType::GravityCompensator => {
            s(commands, e, Vec2::splat(42.0), Color::srgb(0.25, 0.20, 0.38), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::splat(18.0), Color::srgb(0.45, 0.35, 0.70), Vec3::new(0.0, 0.0, 0.02));
        }
        ModuleType::RadiationHardening => {
            s(commands, e, Vec2::splat(44.0), Color::srgb(0.30, 0.35, 0.22), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(38.0, 2.0), Color::srgb(0.45, 0.55, 0.30), Vec3::new(0.0, 6.0, 0.02));
            s(commands, e, Vec2::new(38.0, 2.0), Color::srgb(0.45, 0.55, 0.30), Vec3::new(0.0, -6.0, 0.02));
        }
        ModuleType::BlackBox => {
            s(commands, e, Vec2::splat(36.0), Color::srgb(0.08, 0.08, 0.10), Vec3::new(0.0, 0.0, 0.01));
        }
        ModuleType::EmergencyO2Cache => {
            s(commands, e, Vec2::splat(38.0), Color::srgb(0.18, 0.35, 0.35), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(10.0, 28.0), Color::srgb(0.25, 0.55, 0.55), Vec3::new(0.0, 0.0, 0.02));
        }

        // ====== STRUCTURAL — gray tones with subtle marks ======

        ModuleType::ReinforcedJoint | ModuleType::StructuralBrace => {
            s(commands, e, Vec2::splat(46.0), Color::srgb(0.38, 0.38, 0.40), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(3.0, 40.0), Color::srgb(0.50, 0.50, 0.52), Vec3::new(0.0, 0.0, 0.02));
            s(commands, e, Vec2::new(40.0, 3.0), Color::srgb(0.50, 0.50, 0.52), Vec3::new(0.0, 0.0, 0.02));
        }
        ModuleType::VibrationDamper => {
            s(commands, e, Vec2::splat(42.0), Color::srgb(0.35, 0.35, 0.38), Vec3::new(0.0, 0.0, 0.01));
            s(commands, e, Vec2::new(30.0, 6.0), Color::srgb(0.28, 0.28, 0.32), Vec3::new(0.0, 0.0, 0.02));
        }
        ModuleType::ThermalInsulator => {
            s(commands, e, Vec2::splat(42.0), Color::srgb(0.40, 0.34, 0.22), Vec3::new(0.0, 0.0, 0.01));
            for i in [-10.0, 0.0, 10.0] {
                s(commands, e, Vec2::new(36.0, 2.0), Color::srgb(0.52, 0.44, 0.28), Vec3::new(0.0, i, 0.02));
            }
        }
        // Corner armor plate: L-shaped silhouette — two bars meeting at a corner,
        // the fourth quadrant is deliberately left bare to read as the open notch.
        ModuleType::CornerArmorPlate => {
            s(commands, e, Vec2::new(64.0, 28.0), Color::srgb(0.42, 0.40, 0.38), Vec3::new(0.0, -18.0, 0.01));
            s(commands, e, Vec2::new(28.0, 64.0), Color::srgb(0.42, 0.40, 0.38), Vec3::new(-18.0, 0.0, 0.01));
            s(commands, e, Vec2::new(58.0, 2.0), Color::srgb(0.55, 0.52, 0.48), Vec3::new(0.0, -18.0, 0.02));
            s(commands, e, Vec2::new(2.0, 58.0), Color::srgb(0.55, 0.52, 0.48), Vec3::new(-18.0, 0.0, 0.02));
        }
        // Bridge Wing: T-shaped — wide window bar (the wings) plus a stem
        // (access corridor back into the hull).
        ModuleType::BridgeWing => {
            s(commands, e, Vec2::new(96.0, 26.0), Color::srgb(0.30, 0.24, 0.38), Vec3::new(0.0, -20.0, 0.01));
            s(commands, e, Vec2::new(20.0, 44.0), Color::srgb(0.30, 0.24, 0.38), Vec3::new(0.0, 12.0, 0.01));
            for i in [-32.0, 0.0, 32.0] {
                s(commands, e, Vec2::new(14.0, 8.0), Color::srgb(0.45, 0.70, 0.95), Vec3::new(i, -20.0, 0.02));
            }
        }
        // Surgical Bay: T-shaped — narrow triage stem into a wide treatment bay.
        ModuleType::SurgicalBay => {
            s(commands, e, Vec2::new(96.0, 26.0), Color::srgb(0.55, 0.56, 0.66), Vec3::new(0.0, -20.0, 0.01));
            s(commands, e, Vec2::new(20.0, 44.0), Color::srgb(0.55, 0.56, 0.66), Vec3::new(0.0, 12.0, 0.01));
            s(commands, e, Vec2::new(24.0, 8.0), Color::srgb(0.85, 0.30, 0.30), Vec3::new(0.0, -20.0, 0.02));
            s(commands, e, Vec2::new(8.0, 24.0), Color::srgb(0.85, 0.30, 0.30), Vec3::new(0.0, -20.0, 0.02));
        }
        // Galley Mess: L-shaped — long galley run + dining nook in the bend.
        ModuleType::GalleyMess => {
            s(commands, e, Vec2::new(64.0, 28.0), Color::srgb(0.30, 0.55, 0.34), Vec3::new(0.0, -18.0, 0.01));
            s(commands, e, Vec2::new(28.0, 64.0), Color::srgb(0.30, 0.55, 0.34), Vec3::new(18.0, 0.0, 0.01));
            s(commands, e, Vec2::new(18.0, 18.0), Color::srgb(0.75, 0.65, 0.35), Vec3::new(18.0, -18.0, 0.02));
        }
        // Bulk Cargo Hold: L-shaped — same footprint language as Galley Mess,
        // fills a hull corner instead of wasting it.
        ModuleType::BulkCargoHold => {
            s(commands, e, Vec2::new(64.0, 28.0), Color::srgb(0.55, 0.46, 0.20), Vec3::new(0.0, -18.0, 0.01));
            s(commands, e, Vec2::new(28.0, 64.0), Color::srgb(0.55, 0.46, 0.20), Vec3::new(18.0, 0.0, 0.01));
            s(commands, e, Vec2::new(20.0, 20.0), Color::srgb(0.35, 0.30, 0.15), Vec3::new(18.0, -18.0, 0.02));
        }
        // Docking Hub: plus-shaped — central hub with a port on every side.
        ModuleType::DockingHub => {
            s(commands, e, Vec2::splat(40.0), Color::srgb(0.32, 0.32, 0.42), Vec3::new(0.0, 0.0, 0.01));
            for (dx, dy) in [(0.0, 34.0), (0.0, -34.0), (34.0, 0.0), (-34.0, 0.0)] {
                s(commands, e, Vec2::new(if dx == 0.0 { 20.0 } else { 24.0 }, if dx == 0.0 { 24.0 } else { 20.0 }),
                    Color::srgb(0.45, 0.45, 0.58), Vec3::new(dx, dy, 0.02));
            }
        }
        // Wellness Hub: plus-shaped — same hub language as Docking Hub, softer
        // colors for a gym/common-area read.
        ModuleType::WellnessHub => {
            s(commands, e, Vec2::splat(40.0), Color::srgb(0.34, 0.34, 0.48), Vec3::new(0.0, 0.0, 0.01));
            for (dx, dy) in [(0.0, 34.0), (0.0, -34.0), (34.0, 0.0), (-34.0, 0.0)] {
                s(commands, e, Vec2::new(if dx == 0.0 { 20.0 } else { 24.0 }, if dx == 0.0 { 24.0 } else { 20.0 }),
                    Color::srgb(0.55, 0.75, 0.55), Vec3::new(dx, dy, 0.02));
            }
        }
        // Staggered Armor Plate: S-shaped — two offset bars, no straight seam
        // runs through the whole plate.
        ModuleType::StaggeredArmorPlate => {
            s(commands, e, Vec2::new(64.0, 28.0), Color::srgb(0.36, 0.36, 0.40), Vec3::new(-16.0, -18.0, 0.01));
            s(commands, e, Vec2::new(64.0, 28.0), Color::srgb(0.42, 0.42, 0.46), Vec3::new(16.0, 18.0, 0.01));
            s(commands, e, Vec2::new(58.0, 2.0), Color::srgb(0.55, 0.52, 0.48), Vec3::new(-16.0, -18.0, 0.02));
            s(commands, e, Vec2::new(58.0, 2.0), Color::srgb(0.55, 0.52, 0.48), Vec3::new(16.0, 18.0, 0.02));
        }
        // Angled Hull Plate / Angled Armor Plate: single-cell wedge — a
        // triangular silhouette instead of a filled square. The exposed
        // diagonal is the real armour face (Block::facing): material lies
        // along the two solid edges and the hypotenuse between them is what
        // rounds skip off, so the sprite shows which way the plate looks.
        ModuleType::AngledHullPlate => {
            s(commands, e, Vec2::new(46.0, 10.0), Color::srgb(0.42, 0.40, 0.36), Vec3::new(0.0, 16.0, 0.01));
            s(commands, e, Vec2::new(10.0, 46.0), Color::srgb(0.42, 0.40, 0.36), Vec3::new(16.0, 0.0, 0.01));
            s(commands, e, Vec2::new(30.0, 6.0), Color::srgb(0.52, 0.50, 0.46), Vec3::new(4.0, 4.0, 0.02));
        }
        ModuleType::AngledArmorPlate => {
            s(commands, e, Vec2::new(46.0, 10.0), Color::srgb(0.36, 0.36, 0.40), Vec3::new(0.0, 16.0, 0.01));
            s(commands, e, Vec2::new(10.0, 46.0), Color::srgb(0.36, 0.36, 0.40), Vec3::new(16.0, 0.0, 0.01));
            s(commands, e, Vec2::new(30.0, 6.0), Color::srgb(0.50, 0.48, 0.44), Vec3::new(4.0, 4.0, 0.02));
        }

        // ====== FALLBACK — category-colored panel ======
        _ => {
            let c = match mt.category() {
                ModuleCategory::Power => Color::srgb(0.38, 0.35, 0.14),
                ModuleCategory::Propulsion => Color::srgb(0.16, 0.28, 0.40),
                ModuleCategory::LifeSupport => Color::srgb(0.16, 0.38, 0.25),
                ModuleCategory::Weapons => Color::srgb(0.40, 0.18, 0.14),
                ModuleCategory::Guns => Color::srgb(0.40, 0.32, 0.18),
                ModuleCategory::Lasers => Color::srgb(0.40, 0.18, 0.18),
                ModuleCategory::Detection => Color::srgb(0.16, 0.35, 0.38),
                ModuleCategory::Storage => Color::srgb(0.32, 0.26, 0.16),
                ModuleCategory::Crew => Color::srgb(0.32, 0.22, 0.35),
                ModuleCategory::Utility => Color::srgb(0.26, 0.30, 0.32),
                ModuleCategory::Structural => Color::srgb(0.30, 0.30, 0.32),
                ModuleCategory::Control => Color::srgb(0.28, 0.28, 0.38),
            };
            s(commands, e, Vec2::splat(46.0), c, Vec3::new(0.0, 0.0, 0.01));
        }
    }
}
