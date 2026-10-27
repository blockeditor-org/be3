use super::*;
use crate::native::Waker;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn a_signal_written_between_frames_wakes_the_app_and_lands_on_the_next_frame() {
    let _root = RootScope::new();
    let count = create_signal(0i64);
    let (document, [value]) = toolbar_of(|| {
        [view! {
            <Text>{create_memo(move || count.get().to_string())}</Text>
        }]
    });
    let mut harness = Harness::new(document);
    let woken = Arc::new(AtomicUsize::new(0));
    harness.context().set_waker(Waker::new({
        let woken = Arc::clone(&woken);
        move || {
            woken.fetch_add(1, Ordering::Relaxed);
        }
    }));
    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), value), "0");

    count.set(1);
    count.set(2);
    assert_eq!(woken.load(Ordering::Relaxed), 1);
    assert_eq!(text_of(harness.document(), value), "0");

    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), value), "2");

    count.set(3);
    assert_eq!(woken.load(Ordering::Relaxed), 2);
}
