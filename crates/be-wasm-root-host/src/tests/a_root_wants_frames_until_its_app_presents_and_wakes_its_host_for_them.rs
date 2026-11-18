use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

#[test]
fn a_root_wants_frames_until_its_app_presents_and_wakes_its_host_for_them() {
    let mut root = embedding();
    let woken = Arc::new(AtomicUsize::new(0));
    let rung = Arc::clone(&woken);
    root.on_wake(move || {
        rung.fetch_add(1, Ordering::Relaxed);
    });
    // Past its deadline at once: the app has not even said hello.
    let early = root
        .frame(FrameRequest { time: 1, wall_clock: 0 }, Instant::now())
        .expect("the root draws");
    assert_eq!(
        shown(&early).len(),
        2,
        "the root's surface and its late marker, with no screen yet"
    );
    assert!(
        early.next.is_some_and(|next| next > 1),
        "while its app starts, the root wants a frame by the time the app's frame times out, \
         unless the app wakes it first: {:?}",
        early.next
    );
    assert!(
        early.owed,
        "an app starting owes its hello, so the root owes work"
    );

    let shown_at_last = root
        .frame(FrameRequest { time: 2, wall_clock: 0 }, Instant::now() + PATIENT)
        .expect("the root draws");
    assert_eq!(
        shown(&shown_at_last).len(),
        2,
        "the root's surface and the editor's screen, the frame having waited for the app"
    );
    assert!(
        woken.load(Ordering::Relaxed) > 0,
        "what the app sent rang the host as well as the root"
    );
    // The triangle asked for a frame more as it started, which the root asks for in turn.
    let mut time = 2;
    let mut next = shown_at_last.next;
    while let Some(at) = next {
        assert!(time < 5, "the root keeps wanting frames from {at}");
        time += 1;
        let again = root
            .frame(FrameRequest { time, wall_clock: 0 }, Instant::now() + PATIENT)
            .expect("the root draws");
        assert_eq!(shown(&again).len(), 2, "the screen stays shown");
        next = again.next;
        assert!(
            next.is_some() || !again.owed,
            "a root that wants nothing owes nothing"
        );
    }
}
