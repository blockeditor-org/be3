use super::*;
use beui::PointerButton;
use beui_host::native::testing::Target;

#[test]
fn a_tab_let_go_on_a_tool_bar_band_goes_home() {
    let mut test = alone(WIDE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let home = text_x(&test, "Layers");
    let band = test.rect_of("dock.bar.band.bottom");
    let status = test.rect_of("bars.status");
    let pages = test.rect_of("bars.pages");
    let between = Target::point(pos2(
        (status.right() + pages.left()) / 2.0,
        band.center().y,
    ));

    test.down(Target::text("Tab \"Layers\""), PointerButton::Primary);
    test.hover(Target::point(band.center() - Vec2::new(0.0, 120.0)));
    test.hover(between.clone());
    test.frame(Vec::new());
    assert!(
        test.shown("dock.preview") && !test.shown("dock.drop"),
        "over the band between its bars, the tab is carried but offered no drop"
    );
    test.up(between, PointerButton::Primary);
    test.settle();
    assert!(
        (text_x(&test, "Layers") - home).abs() < 0.5,
        "let go there, the tab goes back to its bar rather than floating into a window"
    );
    assert!(!test.shown("dock.preview"), "and nothing is carried any more");
}
