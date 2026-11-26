use super::*;

/// Taken in late (`drive present late`), what an app sends from its first present on is held
/// across settling's pumps, then heard in the order it came at the poll; the frame it presented
/// stays with the worker until then.
#[test]
fn presents_taken_in_late_are_heard_in_order_at_the_poll() {
    let mut runtime = Runtime::new(&manifest(), 0);
    runtime.take_in(vec![presented(1, 1)], false);
    runtime.show_presented(|_| Some(frame(1)));
    assert_eq!(
        runtime.frames, 1,
        "taken in at once, a frame is shown at once"
    );

    runtime.take_in(vec![presented(2, 2)], true);
    runtime.take_in(vec![placing(3, &[1])], true);
    assert!(
        matches!(
            runtime.late.as_slice(),
            [Heard::Presented(_), Heard::LaidOut(_)]
        ),
        "what came after a late present is held behind it, in order"
    );
    let mut asked = false;
    runtime.show_presented(|_| {
        asked = true;
        Some(frame(2))
    });
    assert!(
        !asked && runtime.frames == 1,
        "the frame of a held present stays with the worker"
    );

    runtime.taking_in = true;
    runtime.take_in(Vec::new(), false);
    runtime.show_presented(|_| Some(frame(2)));
    assert!(runtime.late.is_empty(), "the poll hears what was held");
    assert_eq!(runtime.frames, 2, "the poll shows the held present's frame");
}
