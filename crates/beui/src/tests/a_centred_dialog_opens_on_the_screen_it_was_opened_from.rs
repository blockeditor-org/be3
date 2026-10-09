use super::*;
use crate::Screen;
use crate::base::Justify;
use crate::reactive::{NodeRef, Text, build, create_signal, view, with_reactive_scope};
use crate::styled::{Button, ButtonVariant, Dialog};

#[test]
fn a_centred_dialog_opens_on_the_screen_it_was_opened_from() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(700.0, 500.0));
    let right = Rect::from_min_size(pos2(700.0, 0.0), vec2(500.0, 400.0));
    let (open, set_open) = create_signal(false);
    let body = NodeRef::new();
    let document = build({
        let body = body.clone();
        let (opening, reopening) = (set_open.clone(), set_open.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <List direction=Direction::Horizontal justify=Justify::SpaceBetween spacing=0.0>
                        <Button
                            label="Left"
                            variant=ButtonVariant::Secondary
                            @test_id={"left"}
                            on_click={move || opening.set(true)}
                        />
                        <Button
                            label="Right"
                            variant=ButtonVariant::Secondary
                            @test_id={"right"}
                            on_click={move || reopening.set(true)}
                        />
                    </List>
                    <Dialog open={open} title="About" width=300.0 on_dismiss={|| {}}>
                        <Text string="Body" font_size=14.0 color=Color32::WHITE @node_ref={&body} />
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(1200.0, 500.0));
    harness.context().set_screens(vec![
        Screen::new("left", "Left", left),
        Screen::new("right", "Right", right),
    ]);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().screens(),
        vec![
            Screen::new("left", "Left", left),
            Screen::new("right", "Right", right),
        ],
        "the document lists the screens it is shown on"
    );

    let button = harness.center(harness.find("right"));
    harness.click(button);
    harness.frame(Vec::new());
    let shown = harness.rect(body.get());
    assert!(
        right.contains_rect(shown),
        "a dialog opened from the right screen opens there, not across the seam: {shown:?}"
    );
    assert!(
        (shown.center().x - right.center().x).abs() < 2.0,
        "the dialog is centred on its screen"
    );

    with_reactive_scope(harness.document_mut(), {
        let set_open = set_open.clone();
        move || set_open.set(false)
    });
    harness.frame(Vec::new());
    let button = harness.center(harness.find("left"));
    harness.click(button);
    harness.frame(Vec::new());
    let shown = harness.rect(body.get());
    assert!(
        left.contains_rect(shown),
        "a dialog opened from the left screen opens there: {shown:?}"
    );
    assert!((shown.center().x - left.center().x).abs() < 2.0);
}
