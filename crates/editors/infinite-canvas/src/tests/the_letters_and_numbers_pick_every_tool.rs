use super::*;
use block_editor_beui::beui::{Key, Modifiers, Vec2};

#[test]
fn the_letters_and_numbers_pick_every_tool() {
    let mut editor = editor(&[]);
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let origin = canvas.min + Vec2::new(40.0, 40.0);
    let tools = [
        (Key::R, CanvasShape::Rectangle),
        (Key::Two, CanvasShape::Rectangle),
        (Key::D, CanvasShape::Diamond),
        (Key::Three, CanvasShape::Diamond),
        (Key::O, CanvasShape::Ellipse),
        (Key::Four, CanvasShape::Ellipse),
        (Key::A, CanvasShape::Path),
        (Key::Five, CanvasShape::Path),
        (Key::L, CanvasShape::Path),
        (Key::Six, CanvasShape::Path),
        (Key::P, CanvasShape::Freehand),
        (Key::Seven, CanvasShape::Freehand),
        (Key::T, CanvasShape::Text),
        (Key::Eight, CanvasShape::Text),
        (Key::F, CanvasShape::Frame),
    ];
    for (index, (key, shape)) in tools.into_iter().enumerate() {
        let from = origin + Vec2::new((index % 5) as f32 * 120.0, (index / 5) as f32 * 120.0);
        editor.key(Key::Escape, Modifiers::NONE);
        editor.run();
        let before: HashSet<Uuid> = entities(&editor).iter().map(|entity| entity.id).collect();
        editor.key(key, Modifiers::NONE);
        editor.run();
        editor.drag_along(&[
            from,
            from + Vec2::new(30.0, 20.0),
            from + Vec2::new(60.0, 40.0),
        ]);
        editor.run();
        let drawn: Vec<CanvasEntity> = entities(&editor)
            .into_iter()
            .filter(|entity| !before.contains(&entity.id))
            .collect();
        assert_eq!(drawn.len(), 1, "{key:?} draws one shape");
        assert_eq!(
            drawn[0].shape, shape,
            "{key:?} picks the tool for {shape:?}"
        );
        assert_eq!(
            drawn[0].style.end_arrowhead == CanvasArrowhead::Arrow,
            matches!(key, Key::A | Key::Five),
            "{key:?} draws an arrow only for the arrow tool"
        );
    }

    let count = entities(&editor).len();
    for key in [Key::V, Key::One] {
        editor.key(Key::R, Modifiers::NONE);
        editor.key(key, Modifiers::NONE);
        editor.run();
        let from = origin + Vec2::new(620.0, 0.0);
        editor.drag(from, from + Vec2::new(60.0, 40.0));
        editor.run();
        assert_eq!(entities(&editor).len(), count, "{key:?} selects");
    }
    editor.key(Key::R, Modifiers::NONE);
    editor.key(
        Key::V,
        Modifiers {
            shift: true,
            ..Modifiers::NONE
        },
    );
    editor.run();
    let from = origin + Vec2::new(620.0, 120.0);
    editor.drag(from, from + Vec2::new(60.0, 40.0));
    editor.run();
    assert_eq!(
        entities(&editor).len(),
        count,
        "Shift+V lassoes rather than draws"
    );
}
