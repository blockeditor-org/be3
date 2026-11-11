use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// A watch's value reaches the editor from a task, between frames, so nothing but the editor's
/// waker tells the runtime to draw a frame that shows it.
#[test]
fn a_watched_value_that_changes_wakes_the_editor_to_draw_it() {
    let woken = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&woken);
    let host = EditorHost::new(Waker::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    }));
    woken.store(0, Ordering::SeqCst);

    host.set_watched::<be_sdk_api::AdministratorWatch>(&true);
    assert_eq!(
        woken.load(Ordering::SeqCst),
        1,
        "a value that changed wakes the editor"
    );

    host.set_watched::<be_sdk_api::AdministratorWatch>(&true);
    assert_eq!(
        woken.load(Ordering::SeqCst),
        1,
        "the same value again changes nothing to draw"
    );
}
