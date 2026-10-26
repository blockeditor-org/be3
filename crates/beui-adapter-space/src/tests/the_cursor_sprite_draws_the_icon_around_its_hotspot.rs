use beui_core::painter::Shape;

use super::*;
use crate::cursor::{HOTSPOT, SIZE, Sprite};

#[test]
fn the_cursor_sprite_draws_the_icon_around_its_hotspot() {
    let mut sprite = Sprite::new();
    assert!(sprite.changed(CursorIcon::Text, 1.0));
    let beam = sprite.paint(CursorIcon::Text, 1.0);
    assert!(!sprite.changed(CursorIcon::Text, 1.0));
    assert_eq!((beam.id, beam.size), (sprite.window, SIZE));
    let upright = beam.shapes().iter().any(|shape| {
        matches!(shape, Shape::Line { from, to, .. }
            if from.x == HOTSPOT.x && to.x == HOTSPOT.x && from.y < HOTSPOT.y && to.y > HOTSPOT.y)
    });
    assert!(upright, "the text beam stands on the hotspot");

    assert!(sprite.changed(CursorIcon::Default, 1.0));
    let arrow = sprite.paint(CursorIcon::Default, 2.0);
    let glyphs: Vec<Pos2> = arrow
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text { origin, .. } => Some(*origin),
            _ => None,
        })
        .collect();
    assert!(!glyphs.is_empty(), "the arrow is the icon font's pointer");
    assert!(
        glyphs
            .iter()
            .all(|origin| origin.x < HOTSPOT.x && origin.y < HOTSPOT.y),
        "the arrow's tip, not its corner, is on the hotspot"
    );

    assert!(
        sprite.paint(CursorIcon::None, 1.0).shapes().is_empty(),
        "a hidden cursor draws nothing"
    );
}
