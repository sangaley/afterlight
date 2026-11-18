//! The inside of a ship, lit.
//!
//! The block art -- hull floors, hallways, every module -- is drawn dark
//! (its average is about 60 of 255), so zoomed in a ship read as a dim slate
//! box beside the bright painted shell it wears zoomed out. Every ship
//! block's base colour is lifted here, once, by `gain()`. It goes into
//! `BaseSpriteColor`, the colour damage darkening and wreck greying blend
//! from, so they keep working on top of it.
//!
//! AFTERLIGHT_INTERIOR_GAIN=<x> sets the lift (linear light; 1 = the art as
//! drawn), for tuning.

use bevy::prelude::*;

use crate::components::{BaseSpriteColor, HullSegment, Module};

/// How much the inside is lifted, in linear light: 2.4 takes the art's
/// middle grey from about 60 to about 90 of 255.
pub fn gain() -> f32 {
    static G: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *G.get_or_init(|| std::env::var("AFTERLIGHT_INTERIOR_GAIN").ok().and_then(|v| v.parse().ok()).unwrap_or(2.4))
}

/// A block's base colour as this lifted it: a base colour set anew (a wreck
/// greyed, a gun painted) is lifted again; one already lifted is left.
#[derive(Component)]
pub struct Lifted(Color);

fn lift(c: Color, g: f32) -> Color {
    let l = c.to_linear();
    Color::LinearRgba(LinearRgba::new(l.red * g, l.green * g, l.blue * g, l.alpha))
}

#[allow(clippy::type_complexity)]
pub fn light_interiors(
    mut commands: Commands,
    mut blocks: Query<(Entity, &mut BaseSpriteColor, &mut Sprite, Option<&Lifted>), (Changed<BaseSpriteColor>, Or<(With<HullSegment>, With<Module>)>)>,
) {
    let g = gain();
    if (g - 1.0).abs() < 1e-3 {
        return;
    }
    for (entity, mut base, mut sprite, lifted) in &mut blocks {
        if lifted.is_some_and(|l| l.0 == base.0) {
            continue;
        }
        let new = lift(base.0, g);
        base.0 = new;
        sprite.color = lift(sprite.color, g);
        commands.entity(entity).try_insert(Lifted(new));
    }
}
