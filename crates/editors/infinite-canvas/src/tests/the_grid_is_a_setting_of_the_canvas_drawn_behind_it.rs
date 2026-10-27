use super::*;
use block_editor_beui::beui::{Key, Modifiers, Rect, pos2};
use infinite_canvas_types::{CanvasGrid, CanvasGridSpacing};

fn grid(editor: &BeuiTest<CanvasApp>) -> CanvasGrid {
    editor.content::<CanvasContent>(None).root().grid
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    }
}

fn run_from_palette(editor: &mut BeuiTest<CanvasApp>, typed: &str) {
    editor.key(
        Key::P,
        Modifiers {
            shift: true,
            ..ctrl()
        },
    );
    editor.run();
    editor.type_text(typed);
    editor.key(Key::Enter, Modifiers::NONE);
}

#[test]
fn the_grid_is_a_setting_of_the_canvas_drawn_behind_it() {
    let mut editor = editor(&[card()]);
    assert_eq!(
        grid(&editor).shown(),
        Some(CanvasGridSpacing::Medium),
        "a canvas starts with a medium grid"
    );
    let size = editor.rect().size();
    let view = Rect::from_min_size(pos2(0.0, 0.0), size);
    editor.set_view(view, 1.0);
    editor.run();
    editor.snapshot("the_grid_is_drawn_behind_the_canvas");

    editor.set_view(Rect::from_center_size(view.center(), size * 0.3), 0.3);
    editor.advance(std::time::Duration::from_millis(500));
    editor.run();
    editor.snapshot("a_zoomed_out_grid_keeps_its_dots_apart");

    editor.set_view(view, 1.0);
    editor.run();
    run_from_palette(&mut editor, "large grid");
    assert_eq!(grid(&editor).shown(), Some(CanvasGridSpacing::Large));

    let applied = editor.applied(None);
    run_from_palette(&mut editor, "show grid");
    assert_eq!(
        editor.applied(None),
        applied + 1,
        "turning the grid off is one edit of the canvas, which undo takes back"
    );
    assert_eq!(
        grid(&editor),
        CanvasGrid {
            hidden: true,
            spacing: CanvasGridSpacing::Large,
        },
        "turning the grid off keeps its spacing"
    );
    editor.snapshot("a_canvas_with_its_grid_off");
}
