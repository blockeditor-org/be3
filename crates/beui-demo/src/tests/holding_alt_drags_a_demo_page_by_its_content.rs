use super::*;
use beui::{Event, Modifiers, PointerButton};

fn pressed(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::ALT,
    }
}

#[test]
fn holding_alt_drags_a_demo_page_by_its_content() {
    let mut test = demo(WIDE);
    let root = test.document().root().expect("the demo built a root");
    test.click("demo.drag_modifier");
    let from = test.rect_of("demo.drag_modifier").center() + Vec2::new(0.0, 120.0);
    let to = pos2(CATALOG_X * 2.0, WIDE.y / 2.0);

    test.frame(vec![
        Event::PointerMoved(from),
        Event::Modifiers(Modifiers::ALT),
    ]);
    test.frame(vec![pressed(from, true)]);
    test.frame(vec![Event::PointerMoved(from + (to - from) / 2.0)]);
    test.frame(vec![Event::PointerMoved(to)]);
    test.snapshot("docking_drag_by_content");

    test.frame(vec![pressed(to, false)]);
    test.frame(vec![Event::Modifiers(Modifiers::NONE)]);
    test.frame(Vec::new());
    assert_eq!(
        showing(test.document(), root, "Nothing open"),
        1,
        "the page was carried out of its pane into the catalog's, which says nothing is open"
    );
}
