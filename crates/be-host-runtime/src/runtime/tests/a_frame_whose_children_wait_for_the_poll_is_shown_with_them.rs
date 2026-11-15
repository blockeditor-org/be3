use super::*;

/// A frame that switches the children it places (a stacked dock's bar going from the files to a
/// document) is presented between polls, but its children change only at the poll. Shown at
/// once, the new bar would sit a frame over the old children; it is shown at that poll instead,
/// with them.
#[test]
fn a_frame_whose_children_wait_for_the_poll_is_shown_with_them() {
    let mut runtime = Runtime::new(&manifest(), 0);

    runtime.hear(vec![presented(1, 1)]);
    runtime.received(frame(1));
    assert_eq!(
        runtime.frames, 1,
        "a frame that brings no children is shown at once"
    );

    runtime.hear(vec![placing(2, &[1])]);
    runtime.received(frame(2));
    assert_eq!(
        (runtime.held.len(), runtime.frames),
        (1, 1),
        "between polls, the children wait, and so does the frame drawn after their layout, even \
         before its present is heard"
    );
    runtime.hear(vec![presented(2, 2)]);
    assert_eq!(
        runtime.frames, 1,
        "its present heard, it still waits for the poll"
    );

    runtime.taking_in = true;
    runtime.hear(Vec::new());
    assert_eq!(
        (runtime.held.len(), runtime.frames),
        (0, 2),
        "the poll applies the children and shows the frame with them"
    );

    runtime.received(frame(3));
    assert_eq!(
        runtime.frames, 3,
        "with nothing held, a frame is shown at once"
    );

    runtime.taking_in = false;
    runtime.hear(vec![placing(3, &[1]), unpainted(3)]);
    runtime.received(frame(4));
    assert_eq!(
        runtime.frames, 3,
        "a present that painted nothing still holds the next frame"
    );
    runtime.taking_in = true;
    runtime.hear(Vec::new());
    assert_eq!(
        (runtime.held.len(), runtime.frames),
        (0, 4),
        "and the poll that applies the children shows it"
    );
}
