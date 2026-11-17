use std::time::Duration;

use super::*;

#[test]
fn a_module_that_never_starts_holds_up_no_input() {
    let start = Duration::from_millis(20);
    let mut remapper = hostile_within(
        "(loop $forever (br $forever)) (unreachable)",
        Limits {
            start,
            ..Limits::default()
        },
    );

    assert!(remapper.poll(NOW).is_empty());
    assert!(remapper.starting());
    assert_eq!(
        remapper.input(press("A"), NOW),
        vec![press("A")],
        "input passes as it is while the remaps start"
    );
    // Nothing wakes the host for a start that never answers: the next input after its limit
    // finds it failed.
    let by = Deadline::after(start);
    while let Some(left) = by.remaining() {
        let _ = remapper.woken.recv_timeout(left);
    }
    assert_eq!(remapper.input(press("B"), NOW), vec![press("B")]);
    assert_eq!(
        remapper.take_problems(),
        vec![
            "The input remaps failed (they did not answer in time) and start again with the next key or button."
        ]
    );
    assert_eq!(
        remapper.input(release("A"), NOW),
        vec![release("A")],
        "a key pressed while they started is let go as it is"
    );
}
