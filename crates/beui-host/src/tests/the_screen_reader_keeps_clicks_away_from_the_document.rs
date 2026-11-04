use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_screen_reader_keeps_clicks_away_from_the_document() {
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Show timings" checked=false />
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());

    let at = harness.center(checkbox);
    harness.click(at);
    harness.frame(Vec::new());

    assert!(
        !harness
            .document()
            .api::<styled::ToggleApi>(checkbox)
            .checked()
            .get()
    );
}
