use super::*;
use beui::PointerButton;
use beui_host::native::testing::Target;

#[test]
fn the_nested_docks_outline_keeps_the_other_tabs_out_until_their_group_is_unlocked() {
    let mut test = demo(WIDE);
    open(&mut test, Page::NestedDocks);
    test.frame(Vec::new());
    let outline = test.rect_of(&format!(
        "dock.grip.joined:{}/outline",
        tab(Page::NestedDocks)
    ));
    let views = format!("dock.grip.joined:{}/views", tab(Page::NestedDocks));
    let beside = test.rect_of(&views).left();
    let into_outline = Target::point(outline.center() + Vec2::new(40.0, 200.0));

    test.drag(Target::text("Preview"), into_outline.clone());
    test.settle();
    assert!(
        text_x(&test, "Preview") > beside,
        "the Outline's locked area does not take the group's other tabs"
    );
    test.down(Target::text("Preview"), PointerButton::Primary);
    test.hover(Target::point(outline.center() + Vec2::new(120.0, 180.0)));
    test.hover(into_outline.clone());
    test.frame(Vec::new());
    assert!(
        test.shows("dock.refused"),
        "dragged over the locked area, the tab shows it is refused"
    );
    test.snapshot("the_nested_docks_outline_refuses_a_tab_dragged_over_it");
    test.up(into_outline.clone(), PointerButton::Primary);
    test.settle();

    test.click_with(views.as_str(), PointerButton::Secondary);
    test.frame(Vec::new());
    test.click(Target::text("MenuItemCheckBox \"Locked\""));
    test.frame(Vec::new());
    test.drag(Target::text("Preview"), into_outline);
    test.settle();
    assert!(
        text_x(&test, "Preview") < beside,
        "once the group is unlocked, its tab may join the Outline"
    );
}
