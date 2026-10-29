use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_screen_reader_buttons_walk_the_document_and_activate_what_they_reach() {
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Show timings" checked=false />
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());

    harness.click(harness.screen_reader_control_center("next_control"));
    harness.frame(Vec::new());
    assert_eq!(
        harness.reading().as_deref(),
        Some("Show timings, check box, not checked")
    );

    harness.click(harness.screen_reader_control_center("activate"));
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(checkbox)
            .checked()
            .get()
    );
    assert_eq!(
        harness.reading().as_deref(),
        Some("Show timings, check box, checked")
    );
}
