use super::*;
use beui_reactive::RootScope;

#[test]
fn a_timer_that_restarts_itself_at_zero_fires_once_a_frame() {
    let _root = RootScope::new();
    let fired = Rc::new(Cell::new(0));
    let counted = Rc::clone(&fired);
    let ticking = create_timer(move || {
        counted.set(counted.get() + 1);
        Some(Duration::ZERO)
    });
    start();
    freeze_clock(true);
    ticking.start(Duration::ZERO);
    frame();
    assert_eq!(fired.get(), 1, "a frame fires each timer at most once");
    frame();
    assert_eq!(fired.get(), 2, "and the next frame fires it again");
}
