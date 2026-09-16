use super::*;

#[test]
fn dragging_a_slide_onto_another_reorders_the_deck() {
    let (mut test, _editor, block) = editor(3);
    let ids = slide_ids(&block);
    test.run();

    let from = test
        .rect_of(&format!("presentation.slide.{}", ids[0]))
        .center();
    let onto = test
        .rect_of(&format!("presentation.slide.{}", ids[2]))
        .center();
    test.drag(from, onto);
    test.run();

    assert_eq!(slide_ids(&block), [ids[1], ids[2], ids[0]]);
}
