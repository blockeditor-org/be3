use super::*;

#[test]
fn a_window_brought_forward_while_it_is_not_shown_is_reported_raised() {
    let mut harness = Harness::showing(None, |_| {
        view! {
            <Frame />
        }
    });
    let (_window, id) = harness.open();
    let windows = harness.compositor().windows();
    assert_eq!(windows.raised(id), 0);
    let revision = windows.revision();

    windows.activate(id);
    harness.settle();
    assert_eq!(
        windows.raised(id),
        1,
        "the window is reported raised, so the shell can show its tab"
    );
    assert!(windows.revision() > revision, "and the host hears of it");
}
