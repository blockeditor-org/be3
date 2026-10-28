use super::*;
use beui::PointerButton;

fn button(pos: Pos2, pressed: bool) -> Input {
    Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
    }
}

#[test]
fn a_page_dropped_on_the_middle_of_a_pane_is_grouped_with_the_page_it_shows() {
    let mut test = demo(WIDE);
    open_beside(&mut test, Page::Buttons);
    open_beside(&mut test, Page::Text);
    let close = format!("dock.tab.{}.close", tab(Page::Docking));
    let from = test.rect_of(&close).center() - Vec2::new(50.0, 0.0);
    let catalog = WIDE.x * (1.0 - PAGE_SHARE);
    let to = pos2((catalog + WIDE.x) / 2.0, WIDE.y * 0.65);
    test.frame(vec![Input::PointerMoved(from)]);
    test.frame(vec![button(from, true)]);
    test.frame(vec![Input::PointerMoved(from + (to - from) / 2.0)]);
    let alone = pos2(catalog / 2.0, WIDE.y / 2.0);
    test.frame(vec![Input::PointerMoved(alone)]);
    test.frame(Vec::new());
    assert!(
        test.shows("dock.drop") && !test.shows("dock.drop.label"),
        "over a pane holding one tab the page would only join it, so nothing is named"
    );
    test.frame(vec![Input::PointerMoved(to)]);
    test.frame(Vec::new());

    let root = test.document().root().expect("the demo built a root");
    assert!(
        test.shows("dock.drop.label"),
        "the highlight over the middle of a pane says what the drop does"
    );
    assert_eq!(
        showing(test.document(), root, "Group with Text"),
        1,
        "it names the page shown, which the dragged one would be grouped with"
    );
    test.snapshot("docking_group_with_the_page_shown");

    test.frame(vec![button(to, false)]);
    test.frame(Vec::new());

    let root = test.document().root().expect("the demo built a root");
    assert_eq!(
        showing(test.document(), root, "Text, Docking"),
        1,
        "the two pages share one tab of a group"
    );
}
