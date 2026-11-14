use super::*;
use beui_reactive::RootScope;

#[test]
fn a_zero_delay_timer_started_during_a_frame_fires_in_that_frame() {
    let _root = RootScope::new();
    let second_ran = Rc::new(Cell::new(false));
    let ran = Rc::clone(&second_ran);
    let second = create_timer(move || {
        ran.set(true);
        None
    });
    let first = create_timer(move || {
        second.start(Duration::ZERO);
        None
    });
    start();
    freeze_clock(true);
    first.start(Duration::ZERO);
    frame();
    assert!(
        second_ran.get(),
        "work due now that the frame's own timers started is done in the frame, not left to a \
         next frame that a stopped clock never draws"
    );
    assert!(!busy(), "nothing is left for another frame");
}
