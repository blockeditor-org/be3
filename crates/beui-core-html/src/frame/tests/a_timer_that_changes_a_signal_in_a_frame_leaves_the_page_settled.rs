use super::*;
use reactive::{RootScope, create_effect, create_signal};

#[test]
fn a_timer_that_changes_a_signal_in_a_frame_leaves_the_page_settled() {
    let _root = RootScope::new();
    let seen = Rc::new(Cell::new(0));
    let phase = create_signal(0u32);
    let shown = Rc::clone(&seen);
    create_effect(move || shown.set(phase.get()));
    let ticking = create_timer(move || {
        phase.set(phase.get_untracked() + 1);
        Some(Duration::from_millis(16))
    });
    start();
    freeze_clock(true);
    ticking.start(Duration::from_millis(16));
    frame();
    for tick in 1..=3 {
        advance_clock(Duration::from_millis(16));
        assert!(busy(), "advancing the clock asks for a frame now");
        frame();
        assert_eq!(seen.get(), tick, "the frame flushed what its timer changed");
        assert!(
            !busy(),
            "tick {tick}: the timer's change asked for another frame now"
        );
    }
}
