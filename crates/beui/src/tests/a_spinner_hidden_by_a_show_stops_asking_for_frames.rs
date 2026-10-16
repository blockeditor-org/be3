use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_spinner_hidden_by_a_show_stops_asking_for_frames() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
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

    shown.set(false);
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);

    shown.set(true);
    assert!(harness.frame(Vec::new()).repaint_after <= Duration::from_millis(16));
}
