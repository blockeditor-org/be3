use super::*;
use block_editor_beui::beui::Rect;

#[test]
fn a_view_the_host_sets_is_not_fitted_back() {
    let mut editor = editor(&[card()]);
    assert_eq!(editor.label("infinite-canvas.pill.zoom"), "100%");
    let band = editor.content_rect().expect("the canvas reports its stage");

    editor.set_view(
        Rect::from_center_size(band.center() + Vec2::new(60.0, -30.0), band.size() * 2.0),
        2.0,
    );
    editor.settle();
    assert_eq!(
        editor.label("infinite-canvas.pill.zoom"),
        "200%",
        "a view the host moved to is the person's, not something to fit back to the origin"
    );

    editor.set_size(Vec2::new(1300.0, 860.0));
    editor.settle();
    assert_eq!(editor.label("infinite-canvas.pill.zoom"), "200%");
}
