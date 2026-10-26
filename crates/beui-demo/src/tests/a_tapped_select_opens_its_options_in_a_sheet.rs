use super::*;
use beui::{TouchId, TouchPhase};

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

fn tap(test: &mut DocumentTest, pos: Pos2) {
    for phase in [TouchPhase::Start, TouchPhase::End] {
        test.frame(vec![Input::Touch {
            id: TouchId {
                device: 1,
                finger: 1,
            },
            phase,
            pos,
            force: None,
        }]);
    }
    test.frame(Vec::new());
}

#[test]
fn a_tapped_select_opens_its_options_in_a_sheet() {
    let mut test = alone(PHONE, Page::Choices);
    reveal(&mut test, "demo.fruit");
    let trigger = test.rect_of("demo.fruit").left_top() + Vec2::new(24.0, 12.0);
    test.click(trigger);
    test.settle();
    assert!(
        !test.shows("sheet.handle"),
        "a click opens the options under the select"
    );
    press_key(&mut test, beui::Key::Escape);
    test.settle();
    tap(&mut test, trigger);
    test.settle();
    assert!(
        test.shows("sheet.handle"),
        "a tap opens the options in a sheet"
    );
    test.snapshot("a_select_in_a_sheet");
}
