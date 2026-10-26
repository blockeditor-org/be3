use super::*;
use block_editor_beui::beui::{PenPhase, PointerButton, Rect};

fn shown_entities(editor: &BeuiTest<CanvasApp>) -> Vec<(Uuid, Rect)> {
    editor
        .test_ids()
        .iter()
        .filter_map(|(test_id, _)| {
            let id = test_id.strip_prefix("infinite-canvas.entity.")?;
            Some((Uuid::parse_str(id).ok()?, editor.rect_of(test_id)))
        })
        .collect()
}

#[test]
fn a_shape_is_drawn_while_dragged_as_it_lands() {
    let mut editor = editor(&[]);
    let canvas = editor.rect_of("infinite-canvas.canvas");

    for (tool, offset, shape) in [
        ("Ellipse", -220.0, CanvasShape::Ellipse),
        ("Diamond", -60.0, CanvasShape::Diamond),
        ("Arrow", 100.0, CanvasShape::Path),
    ] {
        editor.click(format!("infinite-canvas.dock.tool.{tool}"));
        editor.run();
        let landed: HashSet<Uuid> = entities(&editor).iter().map(|entity| entity.id).collect();
        let from = canvas.center() + Vec2::new(offset, -60.0);
        let to = from + Vec2::new(120.0, 80.0);
        editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
        editor.next_frame();
        editor.pen(PenPhase::Move, to, 0.5);
        editor.run();
        let dragged: Vec<(Uuid, Rect)> = shown_entities(&editor)
            .into_iter()
            .filter(|(id, _)| !landed.contains(id))
            .collect();
        let [(id, rect)] = dragged[..] else {
            panic!("the {tool} being dragged is drawn as the one entity it will be");
        };
        editor.snapshot(&format!(
            "a_shape_is_drawn_while_dragged_as_it_lands_{}",
            tool.to_lowercase()
        ));

        editor.pen(PenPhase::Up(PointerButton::Primary), to, 0.0);
        editor.run();
        let drawn = entities(&editor);
        let entity = drawn.last().expect("the shape lands");
        assert_eq!((entity.id, entity.shape), (id, shape));
        assert_eq!(
            editor.rect_of(&format!("infinite-canvas.entity.{id}")),
            rect,
            "the {tool} lands where and as it was drawn while dragged"
        );
    }
    editor.snapshot("a_shape_is_drawn_while_dragged_as_it_lands");
}
