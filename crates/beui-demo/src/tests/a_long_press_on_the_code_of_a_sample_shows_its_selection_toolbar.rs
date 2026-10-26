use super::*;
use beui::{TouchId, TouchPhase};

const HOLD_FRAMES: usize = 40;

fn finger(phase: TouchPhase, pos: Pos2) -> Input {
    Input::Touch {
        id: TouchId {
            device: 1,
            finger: 1,
        },
        phase,
        pos,
        force: None,
    }
}

#[test]
fn a_long_press_on_the_code_of_a_sample_shows_its_selection_toolbar() {
    let mut test = demo(WIDE);
    test.click("demo.code.The dock around this page");
    test.frame(Vec::new());

    let code = test.rect_of("demo.code");
    let on_word = pos2(code.left() + 24.0, code.top() + 8.0);
    test.frame(vec![finger(TouchPhase::Start, on_word)]);
    for _ in 0..HOLD_FRAMES {
        test.frame(Vec::new());
    }
    test.frame(vec![finger(TouchPhase::End, on_word)]);
    test.frame(Vec::new());

    let region = test
        .document()
        .find_test_id("demo.code")
        .expect("the code is shown");
    let selected = selected_text(&mut test, region);
    assert!(
        !selected.is_empty() && !selected.contains(' '),
        "a long press selects one word, not {selected:?}"
    );
    assert!(
        test.document()
            .api::<beui::unstyled::ContextMenuApi>(region)
            .toolbar(test.document())
            .is_some(),
        "and shows the toolbar above it"
    );
    assert_eq!(
        test.document().floating_rects(beui::WindowId::MAIN).len(),
        1,
        "the toolbar is laid out though the end of the code is scrolled away"
    );
    test.snapshot("code_toolbar");
}
