use super::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn dragging_slowly_with_the_screen_reader_reads_what_the_finger_is_on() {
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
    let from = harness.reader_item_center(0);
    let to = harness.reader_item_center(1);

    harness.touch(TouchPhase::Start, from);
    harness.advance(Duration::from_millis(500));
    harness.touch(TouchPhase::Move, to);
    assert_eq!(harness.reading().as_deref(), Some("Pick a theme, text"));

    harness.touch(TouchPhase::End, to);
    harness.frame(Vec::new());
    assert_eq!(harness.reading().as_deref(), Some("Pick a theme, text"));
}
