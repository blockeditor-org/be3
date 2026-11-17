use super::*;
use beui::unstyled::SUBMENU_DELAY;
use beui::{PointerButton, Rect};
use beui_host::native::testing::Target;

pub(super) fn open_card_menu(test: &mut DocumentTest) {
    reveal(test, "demo.menu.card");
    test.click_with("demo.menu.card", PointerButton::Secondary);
    test.settle();
}

pub(super) fn open_share(test: &mut DocumentTest) {
    test.hover("demo.menu.share");
    assert!(
        !test.shown("demo.menu.share.email"),
        "a submenu waits for the pointer to rest on its row"
    );
    test.pause(SUBMENU_DELAY * 2);
    assert!(
        test.shown("demo.menu.share.email"),
        "resting on Share opens its submenu"
    );
}

/// The points of a straight move from Share to Message, its submenu's last row,
/// and the rect of Export, the row below Share it crosses.
pub(super) fn diagonal(test: &DocumentTest, steps: usize) -> (Vec<Pos2>, Rect) {
    let from = test.point_of("demo.menu.share");
    let to = test.point_of("demo.menu.share.message");
    let points = (1..=steps)
        .map(|step| from + (to - from) * (step as f32 / steps as f32))
        .collect();
    (points, test.rect_of("demo.menu.export"))
}

#[test]
fn heading_for_a_submenu_across_another_row_keeps_it_open() {
    let mut test = alone(WIDE, Page::Menus);
    open_card_menu(&mut test);
    open_share(&mut test);
    test.record();

    let (points, export) = diagonal(&test, 12);
    let mut crossed = 0;
    for at in points {
        test.hover(Target::point(at));
        if export.contains(at) {
            crossed += 1;
            assert!(
                test.shown("demo.menu.share.email") && !test.shown("demo.menu.export.pdf"),
                "crossing Export on the way keeps Share's submenu open"
            );
            if crossed == 2 {
                test.record();
            }
        }
    }
    assert!(crossed >= 2, "the path crosses Export");
    test.pause(SUBMENU_DELAY * 4);
    assert!(test.shown("demo.menu.share.message"));
    assert!(!test.shown("demo.menu.export.pdf"));
    assert!(focused(&test, "demo.menu.share.message"));
    test.record();
    test.snapshot("heading_for_a_submenu_across_another_row_keeps_it_open");
}
