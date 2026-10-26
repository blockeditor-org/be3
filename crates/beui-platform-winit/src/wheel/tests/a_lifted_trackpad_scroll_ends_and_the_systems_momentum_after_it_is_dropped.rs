use super::*;

#[test]
fn a_lifted_trackpad_scroll_ends_and_the_systems_momentum_after_it_is_dropped() {
    let mut wheel = Wheel::new(true);
    let start = Instant::now();
    let lifted = phases(
        &mut wheel,
        start,
        &[
            (0, TouchPhase::Started),
            (8, TouchPhase::Moved),
            (16, TouchPhase::Moved),
            (24, TouchPhase::Ended),
        ],
    );
    assert_eq!(
        lifted,
        [
            Wheeled::StartAndScroll,
            Wheeled::Scroll,
            Wheeled::Scroll,
            Wheeled::ScrollAndEnd
        ],
        "the fingers scroll, and lifting them ends the gesture so beui flings it"
    );

    let coasted = phases(
        &mut wheel,
        start,
        &[
            (25, TouchPhase::Started),
            (33, TouchPhase::Moved),
            (400, TouchPhase::Moved),
            (900, TouchPhase::Ended),
        ],
    );
    assert_eq!(
        coasted,
        [Wheeled::Ignore; 4],
        "the system's momentum would scroll past beui's fling, clamped at the edge"
    );

    let again = phases(
        &mut wheel,
        start,
        &[
            (950, TouchPhase::Started),
            (958, TouchPhase::Moved),
            (966, TouchPhase::Ended),
        ],
    );
    assert_eq!(
        again,
        [
            Wheeled::StartAndScroll,
            Wheeled::Scroll,
            Wheeled::ScrollAndEnd
        ],
        "the next scroll after the momentum is the fingers again"
    );
}
