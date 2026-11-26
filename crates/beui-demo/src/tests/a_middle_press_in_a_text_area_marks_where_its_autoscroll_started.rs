use super::*;

const MARKER: &str = "scroll.autoscroll-marker";

#[test]
fn a_middle_press_in_a_text_area_marks_where_its_autoscroll_started() {
    let mut test = alone(WIDE, Page::Editor);
    reveal(&mut test, "demo.markdown-editor");
    assert!(!test.shown(MARKER), "no marker shows before autoscroll");
    let at = test.rect_of("demo.markdown-editor").center();
    test.frame(vec![
        Input::PointerMoved(at),
        Input::PointerButton {
            pos: at,
            button: beui::PointerButton::Middle,
            pressed: true,
        },
    ]);
    test.frame(Vec::new());
    assert!(
        test.shown(MARKER),
        "a middle press in the text area starts autoscroll and marks where"
    );
    assert!(
        test.rect_of(MARKER).contains(at),
        "the marker sits where the press was"
    );
    test.snapshot("a_middle_press_in_a_text_area_marks_where_its_autoscroll_started");
}
