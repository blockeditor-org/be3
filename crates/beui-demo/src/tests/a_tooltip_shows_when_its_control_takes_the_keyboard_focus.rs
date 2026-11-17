use super::*;
use beui::Key;

const GAP: f32 = 4.0;

#[test]
fn a_tooltip_shows_when_its_control_takes_the_keyboard_focus() {
    let mut test = alone(WIDE, Page::Buttons);
    reveal(&mut test, "demo.tooltip.focus");
    let on_button = |test: &DocumentTest| {
        let button = test.rect_of("demo.tooltip.focus");
        test.document()
            .focused_node()
            .and_then(|node| test.document().node_rect(node))
            .is_some_and(|focused| button.contains(focused.center()))
    };
    for _ in 0..200 {
        if on_button(&test) {
            break;
        }
        test.frame(
            [true, false]
                .into_iter()
                .map(|pressed| {
                    Input::Key(beui::Keystroke {
                        key: Some(Key::Tab),
                        pressed,
                        ..Default::default()
                    })
                })
                .collect(),
        );
        test.frame(Vec::new());
    }
    assert!(on_button(&test), "tabbing reaches the button");
    test.settle();
    let button = test.rect_of("demo.tooltip.focus");
    let tooltip = test.rect_of("tooltip");
    assert_eq!(
        tooltip.top(),
        button.bottom() + GAP,
        "the tooltip opens below its button, its placement's gap off it"
    );
    test.snapshot("a_tooltip_shows_when_its_control_takes_the_keyboard_focus");
}
