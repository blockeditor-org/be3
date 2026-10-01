use super::*;
use crate::DemoShell;

#[test]
fn the_code_of_a_sample_can_be_selected() {
    let mut test = demo(WIDE);
    test.click("demo.code.The dock around this page");
    test.frame(Vec::new());

    let code = test.rect_of("demo.code");
    let line = code.top() + 8.0;
    test.drag(
        pos2(code.left() + 1.0, line),
        pos2(code.right() - 1.0, line),
    );
    test.frame(Vec::new());

    let region = test
        .document()
        .find_test_id("demo.code")
        .expect("the code is shown");
    let selected = beui::unstyled::selectable_text(test.document(), region);
    assert_eq!(
        selected,
        DemoShell::SOURCE
            .lines()
            .next()
            .expect("the source has a line"),
        "dragging across the first line of the code selects it"
    );
}
