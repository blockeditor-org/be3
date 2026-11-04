use super::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn flicking_across_the_screen_reader_reads_the_next_item() {
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

    harness.touch(TouchPhase::Start, pos2(100.0, 40.0));
    harness.touch(TouchPhase::End, pos2(240.0, 40.0));
    harness.frame(Vec::new());

    assert_eq!(harness.reading().as_deref(), Some("Pick a theme, text"));
}
