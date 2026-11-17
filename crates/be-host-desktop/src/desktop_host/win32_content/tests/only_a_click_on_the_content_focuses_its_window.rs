use beui::PointerButton;

use super::*;

#[test]
fn only_a_click_on_the_content_focuses_its_window() {
    let mut shown = Shown::new();

    shown.stepped.hover("content");
    shown.settle();
    assert!(
        shown.heard.0.borrow().contains(&Heard::Hovered(true)),
        "a hover is heard, for the note that says to click"
    );
    assert!(shown.clicks().is_empty(), "but focuses nothing");

    shown.keyboard.notify();
    shown.settle();
    shown.keyboard.notify();
    shown.settle();
    assert!(
        shown.clicks().is_empty(),
        "the shell giving the window the keyboard (a press on its grab handle or space's chrome, \
         be-host coming back to the front) focuses nothing"
    );

    shown.stepped.leave();
    shown.settle();
    shown.stepped.hover("content");
    shown.settle();
    assert!(
        shown.clicks().is_empty(),
        "nor does the pointer coming back over the content"
    );

    let over = shown.content_point(vec2(100.0, 100.0));
    shown.stepped.down("grip", PointerButton::Primary);
    shown.settle();
    shown.stepped.hover(over.clone());
    shown.settle();
    shown.stepped.up(over, PointerButton::Primary);
    shown.settle();
    assert!(
        shown.clicks().is_empty(),
        "nor a press on the grab handle let go over the content"
    );

    let from = shown.content_point(vec2(100.0, 100.0));
    let to = shown.content_point(vec2(200.0, 100.0));
    shown.stepped.drag(from, to);
    shown.settle();
    assert!(
        shown.clicks().is_empty(),
        "nor a press on the content that moves into a drag"
    );

    let at = shown.content_point(vec2(120.0, 80.0));
    shown.stepped.click(at);
    shown.settle();
    assert_eq!(
        shown.clicks(),
        vec![Pos2::new(120.0, 80.0)],
        "a click on the content focuses its window, at the point clicked"
    );
}
