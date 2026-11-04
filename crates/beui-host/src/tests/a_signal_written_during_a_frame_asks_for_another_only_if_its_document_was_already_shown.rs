use super::*;
use crate::native::Waker;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn a_signal_written_during_a_frame_asks_for_another_only_if_its_document_was_already_shown() {
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
    harness.settle();
    let context = harness.context().clone();
    let rect = Rect::from_min_size(Pos2::ZERO, VIEWPORT);

    let before_show = context.run(|context| {
        count.update(|count| *count += 1);
        harness.document_mut().show(context, rect);
    });
    assert!(!before_show.repaint);
    assert_eq!(text_of(harness.document(), value), "1");

    let after_show = context.run(|context| {
        harness.document_mut().show(context, rect);
        count.update(|count| *count += 1);
    });
    assert!(after_show.repaint);
    assert_eq!(text_of(harness.document(), value), "1");

    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), value), "2");
    assert_eq!(woken.load(Ordering::Relaxed), 0);
}
