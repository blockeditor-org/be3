use super::*;
use crate::reactive::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn the_screen_reader_follows_focus_that_tab_moves() {
    let (document, [_heading, _first, second]) = toolbar_of(|| {
        [
            view! {
                <Text style=TextStyle::HEADING>"Settings"</Text>
            },
            view! {
                <Checkbox label="Show timings" checked=false />
            },
            view! {
                <Checkbox label="Show damage" checked=false />
            },
        ]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());
    assert_eq!(harness.reading().as_deref(), Some("Settings, heading"));

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        harness.reading().as_deref(),
        Some("Show timings, check box, not checked")
    );

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        harness.reading().as_deref(),
        Some("Show damage, check box, not checked")
    );

    harness.key(Key::Space, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(second)
            .checked()
            .get()
    );
    assert_eq!(
        harness.reading().as_deref(),
        Some("Show damage, check box, checked")
    );
}
