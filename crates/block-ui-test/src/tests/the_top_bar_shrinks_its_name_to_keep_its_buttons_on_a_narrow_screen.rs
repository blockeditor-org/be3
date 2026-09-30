use super::*;

#[test]
fn the_top_bar_shrinks_its_name_to_keep_its_buttons_on_a_narrow_screen() {
    let mut test = editor()
        .with_size(beui::Vec2::new(320.0, 640.0))
        .with_top_bar(true);
    test.run();

    for button in ["editor.share", "editor.close"] {
        let right = test.rect_of(button).right();
        assert!(
            right <= 320.0,
            "{button} ends at {right}, past the edge of a screen 320 wide"
        );
    }
    assert!(
        test.rect_of("editor.name").width() > 0.0,
        "the name field keeps what room is left"
    );
}
