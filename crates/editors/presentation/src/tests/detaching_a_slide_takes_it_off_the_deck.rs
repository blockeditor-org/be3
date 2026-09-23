use super::*;

#[test]
fn detaching_a_slide_takes_it_off_the_deck() {
    let (mut test, _editor) = editor(3);
    let ids = slide_ids(&test);
    test.run();

    test.click(&format!("presentation.slide.{}.remove", ids[1]));
    test.run();

    assert_eq!(slide_ids(&test), [ids[0], ids[2]]);
}
