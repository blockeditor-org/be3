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
fn a_tapped_popover_opens_in_a_sheet() {
    let mut test = alone(PHONE, Page::Overlays);
    reveal(&mut test, "demo.filters");
    let trigger = test.rect_of("demo.filters").left_top() + Vec2::new(24.0, 12.0);
    test.click(trigger);
    test.settle();
    assert!(
        !test.shows("sheet.handle"),
        "a click opens the popover under its button"
    );
    press_key(&mut test, beui::Key::Escape);
    test.settle();
    tap(&mut test, trigger);
    test.settle();
    assert!(test.shows("sheet.handle"), "a tap opens the popover in a sheet");
    test.snapshot("a_popover_in_a_sheet");
}
