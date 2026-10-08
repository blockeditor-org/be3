use super::*;

#[test]
fn a_launcher_closed_by_a_click_outside_within_one_frame_opens_again() {
    reopens_after(|harness| {
        let pos = Pos2::new(2.0, 2.0);
        harness.frame(vec![Event::PointerMoved(pos)]);
        harness.frame(
            [true, false]
                .into_iter()
                .map(|pressed| Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                })
                .collect(),
        );
    });
}
