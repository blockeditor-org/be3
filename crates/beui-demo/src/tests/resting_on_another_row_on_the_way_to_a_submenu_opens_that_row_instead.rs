use super::heading_for_a_submenu_across_another_row_keeps_it_open::{
    diagonal, open_card_menu, open_share,
};
use super::*;
use beui::unstyled::{SUBMENU_DELAY, SUBMENU_GRACE};
use beui_host::native::testing::Target;

#[test]
fn resting_on_another_row_on_the_way_to_a_submenu_opens_that_row_instead() {
    let mut test = alone(WIDE, Page::Menus);
    open_card_menu(&mut test);
    open_share(&mut test);

    let (points, export) = diagonal(&test, 12);
    for at in points {
        test.hover(Target::point(at));
        if export.contains(at) {
            break;
        }
    }
    test.pause(SUBMENU_GRACE / 2);
    assert!(
        test.shown("demo.menu.share.email") && !test.shown("demo.menu.export.pdf"),
        "a short rest on Export on the way keeps Share's submenu"
    );
    test.pause(SUBMENU_GRACE);
    assert!(
        test.shown("demo.menu.export.pdf"),
        "resting on Export opens its submenu"
    );
    assert!(!test.shown("demo.menu.share.email"), "and closes Share's");

    test.hover("demo.menu.delete");
    assert!(
        test.shown("demo.menu.export.pdf"),
        "moving straight down, away from the open submenu, waits a moment"
    );
    test.pause(SUBMENU_DELAY * 2);
    assert!(!test.shown("demo.menu.export.pdf"));
    assert!(focused(&test, "demo.menu.delete"));
}
