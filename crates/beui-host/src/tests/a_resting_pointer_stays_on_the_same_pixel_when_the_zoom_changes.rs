use super::*;
use beui_runner::Incoming;

#[test]
fn a_resting_pointer_stays_on_the_same_pixel_when_the_zoom_changes() {
    let mut intaken = Intaken::new();
    intaken.frame(
        1.0,
        [Incoming::logical(Input::PointerMoved(pos2(40.0, 20.0)))],
    );
    let mouse = |intaken: &Intaken| {
        intaken
            .runner
            .context()
            .input(|input| input.pointer.mouse())
    };
    assert_eq!(mouse(&intaken), Some(pos2(40.0, 20.0)));
    intaken.runner.context().set_zoom_factor(2.0);
    intaken.frame(1.0, []);
    assert_eq!(
        mouse(&intaken),
        Some(pos2(20.0, 10.0)),
        "the pointer has not moved on the screen, so at twice the zoom it rests at half the points"
    );
}
