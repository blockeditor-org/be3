use super::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn swiping_left_with_the_screen_reader_does_not_read_what_the_finger_crosses() {
    let (document, [_heading, _body]) = toolbar_of(|| {
        [
            view! {
                <Text style=TextStyle::HEADING>"Settings"</Text>
            },
            view! {
                <Text>"Pick a theme"</Text>
            },
        ]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());
    let start = harness.reading();
    let at = harness.reader_item_center(1);

    harness.touch(TouchPhase::Start, at);
    assert_eq!(harness.reading(), start);
    harness.touch(TouchPhase::Move, at - vec2(20.0, 0.0));
    assert_eq!(harness.reading(), start);
    harness.touch(TouchPhase::End, at - vec2(40.0, 0.0));
    harness.frame(Vec::new());

    assert_eq!(harness.reading(), start);
}
