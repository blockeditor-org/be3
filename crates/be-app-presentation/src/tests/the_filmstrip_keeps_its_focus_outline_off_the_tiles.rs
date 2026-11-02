use super::*;

const GAP: f32 = 2.0;

#[test]
fn the_filmstrip_keeps_its_focus_outline_off_the_tiles() {
    let (mut test, _editor) = editor(3);
    test.run();

    let strip = test.rect_of("presentation.filmstrip");
    for id in slide_ids(&test) {
        let tile = test.rect_of(&format!("presentation.slide.{id}"));
        assert!(
            tile.left() - strip.left() >= GAP && strip.right() - tile.right() >= GAP,
            "slide {id} sits under the filmstrip's focus outline"
        );
    }
}
