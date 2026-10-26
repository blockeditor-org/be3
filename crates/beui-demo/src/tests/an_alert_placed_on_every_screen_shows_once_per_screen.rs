use beui::{Rect, Screen};

use super::*;

#[test]
fn an_alert_placed_on_every_screen_shows_once_per_screen() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(640.0, 480.0));
    let right = Rect::from_min_size(pos2(640.0, 0.0), Vec2::new(460.0, 320.0));
    let document = build(move || {
        view! {
            <Root>
                <Modal open=true placement=ModalPlacement::AllScreens motion=ModalMotion::Still>
                    <EveryScreen>|slot: ScreenSlot| {
                        let id = format!("alert.{}", slot.index);
                        let body_id = id.clone();
                        view! {
                            <DialogCard @test_id={id}>
                                <DialogBody
                                    kind=DialogKind::Alert
                                    title="Keep these display settings?"
                                    id={body_id}
                                    actions={vec![
                                        DialogAction::primary("Keep", || {}),
                                        DialogAction::cancel("Revert"),
                                    ]}
                                    focuses={slot.index == 0}
                                >
                                    <Text style=TextStyle::PARAGRAPH>
                                        "Reverting in 15 seconds."
                                    </Text>
                                </DialogBody>
                            </DialogCard>
                        }
                    }</EveryScreen>
                </Modal>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(1100.0, 480.0));
    test.set_screens(vec![
        Screen::new("left", "Left", left),
        Screen::new("right", "Right", right),
    ]);
    assert!(left.contains(test.rect_of("alert.0").center()));
    assert!(right.contains(test.rect_of("alert.1").center()));
    test.snapshot("an_alert_on_two_screens");
}
