use super::*;

#[test]
fn a_disabled_button_says_why_in_its_tooltip() {
    let mut test = alone(WIDE, Page::Inputs);
    reveal(&mut test, "demo.locked.save");
    let at = test.rect_of("demo.locked.save").center();
    test.frame(vec![Input::PointerMoved(at)]);
    test.advance(std::time::Duration::from_secs(2));
    test.frame(Vec::new());
    test.settle();
    assert!(test.shows("tooltip"), "a disabled button still shows its tooltip");
    let save = test.document().find_test_id("demo.locked.save").unwrap();
    assert_eq!(
        test.document()
            .accessibility
            .get(&save)
            .and_then(|node| node.description()),
        Some("Unlock to save"),
        "the reason is the button's description"
    );
    test.snapshot("a_disabled_button_says_why_in_its_tooltip");
}
