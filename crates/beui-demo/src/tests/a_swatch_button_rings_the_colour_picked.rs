use super::*;

#[test]
fn a_swatch_button_rings_the_colour_picked() {
    let mut test = alone(WIDE, Page::Colors);
    let root = test.document().root().expect("the demo built a root");
    reveal(&mut test, "swatches.Blue");
    test.click("swatches.Blue");
    test.frame(Vec::new());
    assert_eq!(showing(test.document(), root, "Drawing in Blue"), 1);
    test.snapshot("a_swatch_button_rings_the_colour_picked");
}
