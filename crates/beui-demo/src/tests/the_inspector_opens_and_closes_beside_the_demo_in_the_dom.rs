use super::*;

#[test]
fn the_inspector_opens_and_closes_beside_the_demo_in_the_dom() {
    let mut test = demo(WIDE);
    test.render_dom();
    test.click("demo.inspector");
    test.frame(Vec::new());
    test.snapshot_dom("inspector_dom");

    let chord = beui::Modifiers {
        ctrl: true,
        shift: true,
        ..beui::Modifiers::NONE
    };
    press_key_with(&mut test, beui::Key::I, chord);
    test.snapshot_dom("inspector_closed_dom");
}
