use super::*;

#[test]
fn a_window_is_shown_until_hidden_and_forgotten_once_it_closes() {
    let mut shown = Shown::default();
    let (terminal, notes) = (HostWindowId(1), HostWindowId(2));
    assert!(shown.shown(terminal), "a window opens shown");
    assert!(!shown.set(terminal, true), "showing a shown window changes nothing");
    assert_eq!(shown.changes(), 0);

    assert!(shown.set(terminal, false));
    assert!(!shown.shown(terminal));
    assert!(shown.shown(notes));
    assert!(!shown.set(terminal, false), "hiding it again changes nothing");
    assert_eq!(shown.changes(), 1);

    shown.keep_only(&[terminal, notes]);
    assert!(!shown.shown(terminal), "a listed window stays hidden");
    shown.keep_only(&[notes]);
    assert!(
        shown.shown(HostWindowId(1)),
        "a window that closed is forgotten, so a new one with its id opens shown"
    );

    assert!(shown.set(notes, false));
    assert!(shown.set(notes, true));
    assert!(shown.shown(notes));
    assert_eq!(shown.changes(), 3);
}
