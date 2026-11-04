use super::*;

#[test]
fn the_filmstrip_places_a_child_editor_for_every_slide() {
    let (mut test, _editor) = editor(3);
    test.run();

    for id in slide_ids(&test) {
        assert!(
            test.shown(&format!("presentation.slide.{id}")),
            "slide {id} has no tile in the filmstrip"
        );
    }
    assert_eq!(detail(&test, "presentation.position"), "Slide 1 of 3");
}
