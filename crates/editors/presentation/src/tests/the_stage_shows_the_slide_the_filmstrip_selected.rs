use super::*;

#[test]
fn the_stage_shows_the_slide_the_filmstrip_selected() {
    let (mut test, _editor, block) = editor(3);
    let ids = slide_ids(&block);
    test.run();

    test.click(&format!("presentation.slide.{}", ids[2]));
    test.run();

    assert_eq!(detail(&test, "presentation.position"), "Slide 3 of 3");
}
