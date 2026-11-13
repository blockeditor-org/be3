use super::*;
use beui_reactive::RootScope;

/// How an animation steps: its timer's work starts it again at zero and touches no signal.
#[test]
fn a_timer_that_starts_itself_again_at_zero_is_given_the_next_frame() {
    let _root = RootScope::new();
    let fired = Rc::new(Cell::new(0));
    let counted = Rc::clone(&fired);
    let held: Rc<Cell<Option<Timer>>> = Rc::default();
    let again = Rc::clone(&held);
    let stepping = create_timer(move || {
        counted.set(counted.get() + 1);
        if let Some(timer) = again.get() {
            timer.start(Duration::ZERO);
        }
        None
    });
    held.set(Some(stepping));
    start();
    stepping.start(Duration::ZERO);
    frame();
    assert_eq!(fired.get(), 1, "a frame fires each timer at most once");
    assert!(busy(), "the step it started again asks for the next frame");
    frame();
    assert_eq!(fired.get(), 2, "and the next frame fires it again");
}
