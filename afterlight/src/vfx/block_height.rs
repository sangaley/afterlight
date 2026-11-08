//! Tall blocks stand up off the hull.
//!
//! The ship is drawn straight down, so a bridge tower, a gun turret and a
//! floor plate all sit flat on the same plane. A Star Destroyer reads as a
//! ship partly because its tower and batteries rise off the hull; here that
//! is faked with the cheapest cue there is, a shadow. Each tall block casts a
//! faint copy of its own silhouette toward the lower right, as if lit from
//! the upper left, and is drawn just above its neighbours so the shadow falls
//! across them. Taller things cast further.
//!
//! The shadow is its own sprite, a child of the block. The block itself never
//! moves, so muzzles, hit resolution and everything else that reads a
//! block's position are untouched.

use bevy::prelude::*;

use crate::components::{DestroyedModule, Module, ModuleCategory, ModuleType, TurretBarrel};

/// Where the light comes from, as the direction shadows fall: toward the
/// lower right of the screen. The camera never rotates, so world-fixed is
/// screen-fixed -- and a ship turning under it keeps its shadows on that side.
const SHADOW_DIR: Vec2 = Vec2::new(0.6, -0.8);

/// How dark each of a shadow's two layers is. Where they overlap, next to the
/// block, the shadow is darkest; its far edge is half that, so it reads as a
/// soft shadow rather than a second, darker copy of the ship.
const SHADOW_ALPHA: f32 = 0.24;

/// The two layers, as fractions of the block's height.
const SHADOW_LAYERS: [f32; 2] = [0.55, 1.0];

/// Tall blocks draw at least this high, above ordinary modules (0.2) and
/// directional parts (0.4), so their shadow lands on top of the neighbours.
const TALL_Z: f32 = 0.42;

/// Shadow depth under its block, in the block's local z.
const SHADOW_Z: f32 = -0.02;

/// How far a block stands off the hull, in world units of shadow offset
/// (one grid cell is 66). None for blocks that lie flat.
pub fn block_height(module: ModuleType) -> Option<f32> {
    use ModuleType::*;
    match module {
        // The command tower.
        BridgeWing | HelmStation => Some(24.0),
        // Masts, dishes and emitters.
        RadarArray | AdvancedRadar | PassiveRadar | DepthScanner | ThermalImager | CreatureScanner
        | MineralScanner | SignalBuoy | SignalJammer | ShieldEmitter | Floodlight | Searchlight
        | DeepFloodlight => Some(17.0),
        _ if module.category() == ModuleCategory::Weapons => Some(GUN_HEIGHT),
        _ => None,
    }
}

/// Guns, and the barrels that turn on top of them.
const GUN_HEIGHT: f32 = 13.0;

/// A turret deck stands on its ring, above the hull and everything on it:
/// its floor casts the shadow that shows it's a layer of its own.
const TURRET_DECK_HEIGHT: f32 = 18.0;
/// A gun house stands on its deck: its shadow falls on the deck around it.
const GUN_HOUSE_HEIGHT: f32 = 9.0;

/// The shadow sprite under a tall block or barrel.
#[derive(Component)]
pub struct HeightShadow {
    height: f32,
}

/// On a block or barrel that already has its shadow.
#[derive(Component)]
pub struct CastsShadow;

fn shadow_of(sprite: &Sprite, height: f32) -> (Sprite, Transform, HeightShadow) {
    (
        Sprite {
            image: sprite.image.clone(),
            color: Color::srgba(0.0, 0.0, 0.0, SHADOW_ALPHA),
            custom_size: sprite.custom_size,
            flip_x: sprite.flip_x,
            flip_y: sprite.flip_y,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, SHADOW_Z),
        HeightShadow { height },
    )
}

/// Give each new tall block, and each turret barrel, its shadow.
pub fn attach_height_shadows(
    mut commands: Commands,
    mut blocks: Query<(Entity, &Module, &Sprite, &mut Transform), (Without<CastsShadow>, Without<DestroyedModule>)>,
    barrels: Query<(Entity, &Sprite), (With<TurretBarrel>, Without<CastsShadow>)>,
    decks: Query<(Entity, &Sprite), (With<crate::combat::turret_deck::TurretPlate>, Without<CastsShadow>)>,
    // The gun house roof: blocks on a turret above its gun parts.
    roofs: Query<(Entity, &Module, &Sprite), (With<crate::combat::turret_deck::OnTurret>, Without<CastsShadow>, Without<DestroyedModule>)>,
) {
    for (entity, module, sprite, mut tf) in &mut blocks {
        let Some(height) = block_height(module.module_type) else { continue };
        // Wedge plates and other blocks drawn as NONE have no silhouette of
        // their own to cast.
        if sprite.color.alpha() == 0.0 {
            continue;
        }
        tf.translation.z = tf.translation.z.max(TALL_Z);
        cast(&mut commands, entity, sprite, height);
    }
    for (entity, sprite) in &barrels {
        cast(&mut commands, entity, sprite, GUN_HEIGHT);
    }
    // Each floor tile casts; a tile's shadow falls under its neighbours, so
    // only the deck's outer edge shows one, on the hull below.
    for (entity, sprite) in &decks {
        cast(&mut commands, entity, sprite, TURRET_DECK_HEIGHT);
    }
    // The gun house roof stands on the deck, a little higher again.
    for (entity, module, sprite) in &roofs {
        if crate::combat::gun_house::is_roof_block(module.module_type) && sprite.color.alpha() > 0.0 {
            cast(&mut commands, entity, sprite, GUN_HOUSE_HEIGHT);
        }
    }
}

fn cast(commands: &mut Commands, entity: Entity, sprite: &Sprite, height: f32) {
    let layers: Vec<Entity> = SHADOW_LAYERS
        .iter()
        .map(|share| commands.spawn(shadow_of(sprite, height * share)).id())
        .collect();
    commands.entity(entity).try_insert(CastsShadow).add_children(&layers);
}

/// Keep every shadow on the lit side of the screen.
///
/// A shadow is a child of its block, so it turns with the ship and with a
/// turret's aim. The offset is set in world terms and carried back into the
/// parent's own frame each frame, so it always falls the same way however
/// the ship is turned.
pub fn place_height_shadows(
    mut shadows: Query<(&HeightShadow, &ChildOf, &mut Transform, &mut Visibility)>,
    parents: Query<(&GlobalTransform, Has<DestroyedModule>)>,
) {
    for (shadow, parent, mut tf, mut vis) in &mut shadows {
        let Ok((parent_gt, destroyed)) = parents.get(parent.parent()) else { continue };
        // A wrecked block has fallen flat.
        let want = if destroyed { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
        let (scale, rotation, _) = parent_gt.to_scale_rotation_translation();
        let world = (SHADOW_DIR.normalize() * shadow.height).extend(0.0);
        let local = rotation.inverse() * world / scale.max(Vec3::splat(1e-3));
        tf.translation.x = local.x;
        tf.translation.y = local.y;
    }
}

#[cfg(test)]
mod block_height_tests {
    use super::*;

    #[test]
    fn towers_stand_tallest_and_floors_lie_flat() {
        let tower = block_height(ModuleType::BridgeWing).unwrap();
        let mast = block_height(ModuleType::RadarArray).unwrap();
        let gun = block_height(ModuleType::Cannon).unwrap();
        assert!(tower > mast && mast > gun, "{tower} {mast} {gun}");
        assert_eq!(block_height(ModuleType::Corridor), None);
        assert_eq!(block_height(ModuleType::ArmorPlate), None);
    }

    /// However the ship is turned, the shadow lands on the same side.
    #[test]
    fn a_shadow_falls_the_same_way_however_the_ship_turns() {
        let mut app = App::new();
        app.add_systems(Update, place_height_shadows);
        let mut seen = Vec::new();
        for turn in [0.0f32, 1.3, 3.1, -2.2] {
            let rotation = Quat::from_rotation_z(turn);
            let parent = app.world_mut().spawn(GlobalTransform::from(Transform::from_rotation(rotation))).id();
            let shadow = app.world_mut().spawn((
                HeightShadow { height: 10.0 },
                Transform::default(),
                Visibility::Inherited,
                ChildOf(parent),
            )).id();
            app.update();
            let local = app.world().get::<Transform>(shadow).unwrap().translation;
            seen.push((rotation * local).truncate());
        }
        for world in &seen {
            assert!((world.length() - 10.0).abs() < 1e-3, "{world}");
            assert!(world.x > 0.0 && world.y < 0.0, "lower right, got {world}");
        }
    }
}
