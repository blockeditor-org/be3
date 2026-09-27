use accesskit::Role;

use super::*;
use crate::input::CursorIcon;
use crate::reactive::view;
use crate::styled::{Button, ButtonVariant, ColorInput};

#[test]
fn a_click_outside_an_open_color_input_closes_it_and_lands_on_what_was_clicked() {
    let clicks = Rc::new(Cell::new(0));
    let clicked = clicks.clone();
    let (document, [button, input]) = toolbar_of(|| {
        [
            view! {
                <Button
                    label="Elsewhere"
                    variant=ButtonVariant::Secondary
                    on_click={move || clicked.set(clicked.get() + 1)}
                />
            },
            view! {
                <ColorInput value={Color32::from_rgb(0x11, 0x22, 0x33)} label="Fill" />
            },
        ]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let open = |harness: &Harness| {
        harness
            .accessible()
            .iter()
            .any(|node| node.role() == Role::Dialog && node.label() == Some("Choose Fill"))
    };
    let swatch = harness.rect(input);
    harness.click(pos2(swatch.left() + 8.0, swatch.center().y));
    harness.frame(Vec::new());
    assert!(open(&harness));

    let elsewhere = pos2(
        harness.rect(button).left() + 10.0,
        harness.rect(button).center().y,
    );
    let hovered = harness.frame(vec![Event::PointerMoved(elsewhere)]);
    assert_eq!(
        hovered.cursor_icon,
        CursorIcon::PointingHand,
        "the document beside an open popover still answers the pointer"
    );

    harness.click(elsewhere);
    harness.frame(Vec::new());

    assert!(!open(&harness), "the click closes the popover");
    assert_eq!(clicks.get(), 1, "and lands on the button it was aimed at");
}
