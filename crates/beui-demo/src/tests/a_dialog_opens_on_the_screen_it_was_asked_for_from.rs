use beui::{Justify, Rect, Screen};

use super::*;

#[test]
fn a_dialog_opens_on_the_screen_it_was_asked_for_from() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(640.0, 480.0));
    let right = Rect::from_min_size(pos2(640.0, 0.0), Vec2::new(460.0, 320.0));
    let document = build(move || {
        let open = create_signal(false);
        view! {
            <Root>
                <List direction=beui::Direction::Horizontal justify=Justify::End spacing=Space::None>
                    <Button
                        label="About"
                        variant=ButtonVariant::Secondary
                        @test_id={"test.about"}
                        on_click={move || open.set(true)}
                    />
                    <Dialog
                        open={open}
                        title="About"
                        width=Size::Popover
                        on_dismiss={move || open.set(false)}
                    >
                        <Text style=TextStyle::PARAGRAPH @test_id={"test.body"}>
                            "Shown on the screen it was asked for from."
                        </Text>
                    </Dialog>
                </List>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(1100.0, 480.0));
    test.set_screens(vec![
        Screen::new("left", "Left", left),
        Screen::new("right", "Right", right),
    ]);
    test.click("test.about");
    test.frame(Vec::new());
    let body = test.rect_of("test.body");
    assert!(
        right.contains_rect(body),
        "the dialog stays on the screen it was asked for from: {body:?}"
    );
    test.snapshot("dialog_on_two_screens");
}
