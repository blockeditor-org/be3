use super::*;
use beui::{Event, Modifiers, PointerButton};

fn button(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    }
}

fn drag(test: &mut DocumentTest, from: Pos2, to: Pos2, modifiers: Modifiers) {
    test.frame(vec![Event::PointerMoved(from), Event::Modifiers(modifiers)]);
    test.frame(vec![button(from, true, modifiers)]);
    test.frame(vec![Event::PointerMoved(from + (to - from) / 2.0)]);
    test.frame(vec![Event::PointerMoved(to)]);
}

fn release(test: &mut DocumentTest, at: Pos2, modifiers: Modifiers) {
    test.frame(vec![button(at, false, modifiers)]);
    test.frame(vec![Event::Modifiers(Modifiers::NONE)]);
    test.frame(Vec::new());
}

#[test]
fn holding_alt_moves_a_floating_demo_page_by_its_content() {
    let mut test = demo(WIDE);
    let close = format!("dock.tab.{}.close", Page::Docking.tab().value());
    let tab = test.rect_of(&close).center() - Vec2::new(50.0, 0.0);
    let dropped = pos2(550.0, 300.0);
    drag(&mut test, tab, dropped, Modifiers::ALT);
    release(&mut test, dropped, Modifiers::ALT);

    let inside = pos2(650.0, 450.0);
    for _ in 0..REVEAL_TICKS {
        if test.shows("demo.drag_modifier") {
            let center = test.rect_of("demo.drag_modifier").center();
            if (340.0..540.0).contains(&center.y) {
                break;
            }
        }
        test.scroll_at(inside, Vec2::new(0.0, -REVEAL_STEP / 4.0));
    }
    test.click("demo.drag_modifier");
    let before = test.rect_of("demo.drag_modifier");

    let from = pos2(700.0, 560.0);
    let to = from - Vec2::new(300.0, 150.0);
    drag(&mut test, from, to, Modifiers::ALT);
    test.snapshot("docking_window_moved_by_content");
    release(&mut test, to, Modifiers::ALT);

    assert_eq!(
        test.rect_of("demo.drag_modifier"),
        before.translate(to - from),
        "the window holding the page moved with the pointer"
    );
}
