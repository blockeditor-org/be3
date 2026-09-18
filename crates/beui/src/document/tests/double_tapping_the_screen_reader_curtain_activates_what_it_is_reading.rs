use super::*;
use crate::reactive::view;
use crate::styled::Checkbox;

#[test]
fn double_tapping_the_screen_reader_curtain_activates_what_it_is_reading() {
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Show timings" checked=false />
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());

    let at = harness.reader_item_center(0);
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    assert_eq!(
        harness.reading().as_deref(),
        Some("Show timings, check box, not checked")
    );
    assert!(!styled::checkbox_checked(harness.document(), checkbox));

    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(styled::checkbox_checked(harness.document(), checkbox));
    assert_eq!(
        harness.transcript().last().map(String::as_str),
        Some("Show timings, check box, checked")
    );
}
