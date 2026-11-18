use super::*;

#[test]
fn switching_to_vector_draws_the_next_shape_clean() {
    let mut editor = editor(&[]).with_top_bar(false);
    editor.pick_menu("canvas.profile.vector");
    editor.run();

    editor.click("infinite-canvas.dock.tool.Rectangle");
    let from = editor.rect_of("infinite-canvas.canvas").center();
    editor.drag(from, from + Vec2::new(120.0, 80.0));

    let drawn = entities(&editor);
    assert_eq!(drawn.len(), 1);
    assert_eq!(
        drawn[0].style.look,
        CanvasLook::Clean,
        "vector's shapes start in the clean look"
    );
}
