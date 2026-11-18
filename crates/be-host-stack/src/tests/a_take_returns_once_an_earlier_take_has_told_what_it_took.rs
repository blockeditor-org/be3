use super::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

/// In a test that runs no frames a thread takes each update as it lands (`start`). Here it takes
/// the export's answer and runs its callback, which holds until this thread's own take is waiting
/// on it: the take must not return before the callback, which the earlier take was telling, has
/// run.
#[test]
fn a_take_returns_once_an_earlier_take_has_told_what_it_took() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    let (entered, entering) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let told = Arc::new(AtomicBool::new(false));
    export(block, {
        let told = Arc::clone(&told);
        move |_| {
            let _ = entered.send(());
            let _ = released.recv();
            told.store(true, Ordering::SeqCst);
        }
    });
    entering
        .recv()
        .expect("the export's callback runs on the thread that takes the updates");
    let releasing = std::thread::spawn(move || {
        crate::telling::await_a_waiting_take();
        let _ = release.send(());
    });

    take_updates();

    assert!(
        told.load(Ordering::SeqCst),
        "the take returned before the earlier take told the answer it took"
    );
    releasing.join().expect("the callback was let go");
}
