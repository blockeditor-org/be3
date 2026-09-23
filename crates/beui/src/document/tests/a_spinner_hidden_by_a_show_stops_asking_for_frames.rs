use super::*;
use crate::reactive::{Show, with_reactive_scope};
use crate::styled::Spinner;

#[test]
fn a_spinner_hidden_by_a_show_stops_asking_for_frames() {
    let (shown, set_shown) = create_signal(true);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Show condition={shown}>
                    <Spinner />
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(harness.frame(Vec::new()).repaint_after <= Duration::from_millis(16));

    let hide = set_shown.clone();
    with_reactive_scope(harness.document_mut(), move || hide.set(false));
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);

    with_reactive_scope(harness.document_mut(), move || set_shown.set(true));
    assert!(harness.frame(Vec::new()).repaint_after <= Duration::from_millis(16));
}
