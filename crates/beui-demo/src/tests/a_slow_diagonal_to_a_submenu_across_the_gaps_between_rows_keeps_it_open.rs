use super::heading_for_a_submenu_across_another_row_keeps_it_open::{
    diagonal, open_card_menu, open_share,
};
use super::*;
use beui_host::native::testing::Target;

#[test]
fn a_slow_diagonal_to_a_submenu_across_the_gaps_between_rows_keeps_it_open() {
    let mut test = alone(WIDE, Page::Menus);
    open_card_menu(&mut test);
    open_share(&mut test);

    let share = test.rect_of("demo.menu.share");
    let (points, export) = diagonal(&test, 40);
    let mut in_gap = false;
    for at in points {
        test.hover(Target::point(at));
        in_gap |= at.y > share.bottom() && at.y < export.top() && at.x < export.right();
        assert!(
            !test.shown("demo.menu.export.pdf"),
            "a slow path across the gap between Share and Export still heads for Share's submenu"
        );
    }
    assert!(in_gap, "the path stops in the gap between the rows");
    assert!(test.shown("demo.menu.share.message"));
    assert!(focused(&test, "demo.menu.share.message"));
}
