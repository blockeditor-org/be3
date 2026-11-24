use super::*;
use beui::TouchPhase;

/// Frames a finger rests before letting go: past the speed's window, so it lets go at rest.
const RESTING_FRAMES: usize = 10;
const RECORDED_FRAMES: usize = 4;

#[test]
fn a_toast_swiped_far_or_fast_flies_off_and_one_let_go_short_springs_back() {
    let mut test = alone(WIDE, Page::Overlays);
    // The lowest toast in view shows the whole stack.
    reveal(&mut test, "toast.3");
    let start = test.rect_of("toast.2").center();
    let at = |x: f32| start + Vec2::new(x, 0.0);

    // Carried a short way and let go at rest, it springs back.
    test.fingers(TouchPhase::Start, &[(1, start)]);
    for step in 1..=6 {
        test.fingers(TouchPhase::Move, &[(1, at(step as f32 * 10.0))]);
    }
    for _ in 0..RESTING_FRAMES {
        test.fingers(TouchPhase::Move, &[(1, at(60.0))]);
    }
    test.snapshot("a_toast_held_part_way_across");
    test.fingers(TouchPhase::End, &[(1, at(60.0))]);
    test.settle();
    assert!(
        test.shown("toast.2"),
        "let go short of the distance and at rest, the toast springs back"
    );

    // Thrown the same short way, it flies off at the speed it was let go at.
    test.fingers(TouchPhase::Start, &[(1, start)]);
    for step in 1..=4 {
        test.fingers(TouchPhase::Move, &[(1, at(step as f32 * 20.0))]);
    }
    test.fingers(TouchPhase::End, &[(1, at(80.0))]);
    for _ in 0..RECORDED_FRAMES {
        test.record();
        test.frame(Vec::new());
    }
    test.snapshot("a_toast_thrown_flies_off");
    test.settle();
    assert!(
        !test.shown("toast.2"),
        "a toast thrown away is dismissed once it is off"
    );
    assert!(
        test.shown("toast.1") && test.shown("toast.3"),
        "and the others stay"
    );
}
