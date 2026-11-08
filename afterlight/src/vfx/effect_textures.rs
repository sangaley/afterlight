use bevy::prelude::*;
use rand::Rng;

// ============================================================================
// EFFECT TEXTURES
//
// Handles for the soft effect art, loaded once at startup and passed to the
// spawn helpers. Modelled on `audio::GameAudio`, which does the same thing for
// the ~43 sound clips: one resource of handles, filled by a Startup system
// with literal paths. There is no asset manifest or folder scan in this
// project, so this is where "which PNGs are effects" is written down.
//
// The textures are pure white with the shape carried in alpha, because a Bevy
// `Sprite.color` MULTIPLIES its texture. Every spawn site already picks a
// meaningful colour — the missile trail rolls a grey per puff, `Blast` lerps
// hot to cool across an explosion's life, projectiles tint per ammo type — so
// baking colour into the art would apply it twice and quietly break the
// readability those colours exist to provide.
// ============================================================================

/// Soft smoke sprites for trails and explosions.
#[derive(Resource)]
pub struct EffectTextures {
    /// Three variants harvested from different frames of one Mantaflow bake.
    /// They are normalised to the same footprint, so they differ in structure
    /// and density but not in size and can be swapped freely.
    smoke: [Handle<Image>; 3],
    /// Six irregular scrap shards. Unlike the smoke these keep their render's
    /// shading rather than being flattened to white-plus-alpha: smoke is
    /// shapeless so its silhouette is the whole texture, while a chunk is a
    /// solid object and its facet shading is what stops it reading as a flat
    /// sticker. They are authored deliberately light because `spawn_chunks`
    /// tints them by the block's colour already darkened to 0.55, and
    /// `Sprite.color` multiplies.
    debris: [Handle<Image>; 6],
    /// The hot core of a detonation, for the `Blast` fireball layer.
    pub fireball: Handle<Image>,
    /// Flame variants. Three of them so a burning module can flicker between
    /// them on a timer rather than pulsing one shape, which reads as a
    /// throbbing sticker rather than as fire.
    flame: [Handle<Image>; 3],
    /// Directional gun flash, authored pointing along +X like every other
    /// rotated sprite in the project.
    pub muzzle: Handle<Image>,
    /// A spark: bright head, tapering tail, authored along +X. Anisotropic on
    /// purpose -- the radial puff stretched onto a long thin quad gives a
    /// symmetric smear with no sense of which way the spark is going, and for
    /// a ricochet that direction is the entire message.
    pub spark: Handle<Image>,
    /// An annulus for the blast's shock ring. The fireball texture is a filled
    /// ball, so it cannot serve here: over the ring layer it just draws a
    /// second, fainter fireball inside the first.
    pub ring: Handle<Image>,
}

impl EffectTextures {
    /// A random smoke puff.
    ///
    /// Randomising here rather than at the call site keeps every emitter
    /// varied without each one having to know how many variants exist. A
    /// trail that drew the same puff every time reads as a repeating stamp.
    pub fn puff(&self) -> Handle<Image> {
        let i = rand::thread_rng().gen_range(0..self.smoke.len());
        self.smoke[i].clone()
    }

    /// A random scrap shard, for the same reason as `puff`: two chunks off the
    /// same block should not be the same chunk.
    pub fn chunk(&self) -> Handle<Image> {
        let i = rand::thread_rng().gen_range(0..self.debris.len());
        self.debris[i].clone()
    }

    /// A random flame.
    pub fn flame(&self) -> Handle<Image> {
        let i = rand::thread_rng().gen_range(0..self.flame.len());
        self.flame[i].clone()
    }

    /// Flame `i`, wrapping. For the burning overlay, which steps through them
    /// in order so the flicker is a cycle rather than a random stutter.
    pub fn flame_at(&self, i: usize) -> Handle<Image> {
        self.flame[i % self.flame.len()].clone()
    }
}

pub fn load_effect_textures(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(EffectTextures {
        smoke: [
            assets.load("sprites/effects/smoke_puff_a.png"),
            assets.load("sprites/effects/smoke_puff_b.png"),
            assets.load("sprites/effects/smoke_puff_c.png"),
        ],
        debris: [
            assets.load("sprites/effects/debris_01.png"),
            assets.load("sprites/effects/debris_02.png"),
            assets.load("sprites/effects/debris_03.png"),
            assets.load("sprites/effects/debris_04.png"),
            assets.load("sprites/effects/debris_05.png"),
            assets.load("sprites/effects/debris_06.png"),
        ],
        fireball: assets.load("sprites/effects/fireball.png"),
        flame: [
            assets.load("sprites/effects/flame_a.png"),
            assets.load("sprites/effects/flame_b.png"),
            assets.load("sprites/effects/flame_c.png"),
        ],
        muzzle: assets.load("sprites/effects/muzzle_flash.png"),
        spark: assets.load("sprites/effects/spark_streak.png"),
        ring: assets.load("sprites/effects/shock_ring.png"),
    });
}

#[cfg(test)]
impl EffectTextures {
    /// Empty handles, for headless tests of systems that spawn effects. Test
    /// builds only: in the game an empty handle is an invisible effect, and
    /// nothing outside a test should ever want one.
    pub(crate) fn empty() -> Self {
        Self {
            smoke: Default::default(),
            debris: Default::default(),
            fireball: Default::default(),
            flame: Default::default(),
            muzzle: Default::default(),
            spark: Default::default(),
            ring: Default::default(),
        }
    }
}
