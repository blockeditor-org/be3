use super::*;
use crate::reactive::view;
use crate::styled::Button;

#[test]
fn clicking_a_button_focuses_it_without_a_ring_until_a_key_is_pressed() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! {
                <Button label="First" variant=styled::ButtonVariant::Primary />
            },
            view! {
                <Button label="Second" variant=styled::ButtonVariant::Primary />
            },
        ]
    });
    let first_focused = unstyled::button_focused(&document, first);
    let second_focused = unstyled::button_focused(&document, second);
    let mut harness = Harness::new(document);
    let rings = |output: &crate::FrameOutput| {
        output.shapes().iter().filter(|shape| matches!(shape,
            crate::Shape::Rect { color, stroke_width, .. } if *color == styled::Theme::DARK.accent && *stroke_width == 2.0
        )).count()
    };
    let initial = harness.frame(vec![]);

    let center = harness.center(first);
    harness.click(center);
    let clicked = harness.frame(vec![]);
    assert!(first_focused.get());
    assert_eq!(rings(&clicked), rings(&initial));

    let tabbed = harness.frame(vec![key_event(Key::Tab, true, Modifiers::NONE)]);
    assert!(second_focused.get());
    assert!(rings(&tabbed) > rings(&initial));

    let center = harness.center(first);
    harness.click(center);
    let clicked_again = harness.frame(vec![]);
    assert!(first_focused.get());
    assert_eq!(rings(&clicked_again), rings(&initial));
}
