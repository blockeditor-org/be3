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
        .frame(
            FrameRequest {
                time: 1,
                wall_clock: 0,
                utc_offset: 0,
                window: window(),
                describe: false,
            },
            Instant::now(),
        )
        .expect("the root draws");
    assert_eq!(
        shown(&early).len(),
        3,
        "the root's surface, its text and its late marker, with no screen yet"
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
        .frame(
            FrameRequest {
                time: 2,
                wall_clock: 0,
                utc_offset: 0,
                window: window(),
                describe: false,
            },
            Instant::now() + PATIENT,
        )
        .expect("the root draws");
    assert_eq!(
        shown(&shown_at_last).len(),
        3,
        "the root's surface, its text and the editor's screen, the frame having waited for the \
         app"
    );
    assert!(
        woken.load(Ordering::Relaxed) > 0,
        "what the app sent rang the host as well as the root"
    );
    // The triangle asked for a frame more as it started, which the root asks for in turn. Past
    // its app's wants, the root wants only the frame its own document's timer is due at
    // (be-sdk-root-testing's `LATER`), which it does not owe.
    let timer = u64::try_from(be_sdk_root_testing::LATER.as_nanos()).expect("in range") + 1;
    let mut time = 2;
    let mut next = shown_at_last.next;
    let mut owed = shown_at_last.owed;
    while let Some(at) = next.filter(|at| *at < timer) {
        assert!(time < 5, "the root keeps wanting frames from {at}");
        time += 1;
        let again = root
            .frame(
                FrameRequest {
                    time,
                    wall_clock: 0,
                    utc_offset: 0,
                    window: window(),
                    describe: false,
                },
                Instant::now() + PATIENT,
            )
            .expect("the root draws");
        assert_eq!(shown(&again).len(), 3, "the screen stays shown");
        next = again.next;
        owed = again.owed;
        assert!(
            next.is_some() || !owed,
            "a root that wants nothing owes nothing"
        );
    }
    assert_eq!(
        (next, owed),
        (Some(timer), false),
        "once its app wants nothing more, the root wants only its document's timer, and owes it nothing"
    );
}
