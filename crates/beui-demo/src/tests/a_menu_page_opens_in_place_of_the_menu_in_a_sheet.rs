use super::*;
use beui::{TouchId, TouchPhase};

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

fn tap(test: &mut DocumentTest, test_id: &str) {
    let pos = test.rect_of(test_id).center();
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
fn a_menu_page_opens_in_place_of_the_menu_in_a_sheet() {
    let mut test = alone(PHONE, Page::Menus);
    reveal(&mut test, "demo.new");
    tap(&mut test, "demo.new");
    test.settle();
    tap(&mut test, "demo.new.location");
    test.settle();
    assert!(
        test.shown("demo.new.location.back") && test.shown("demo.new.folder.1"),
        "the sheet shows the page of folders"
    );
    test.snapshot("a_menu_page_in_a_sheet");
    tap(&mut test, "demo.new.location.back");
    test.settle();
    assert!(
        test.shown("demo.new.location"),
        "the back row returns to the menu"
    );
}
