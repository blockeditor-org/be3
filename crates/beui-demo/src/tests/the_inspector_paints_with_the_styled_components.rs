use super::*;

#[test]
fn the_inspector_paints_with_the_styled_components() {
    let mut test = demo(WIDE);
    test.click("demo.bar.pin.demo.inspector");
    test.frame(Vec::new());
    assert!(
        test.document().find_test_id("inspector.close").is_none(),
        "the inspector paints in its own document beside the demo"
    );
    test.leave();
    test.snapshot("inspector");
}
