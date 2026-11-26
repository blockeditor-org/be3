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
    let by = NOW + 20_000;
    assert_eq!(
        remapper.deadline(),
        Some(by),
        "the host's timer comes when the start must have been answered"
    );
    assert_eq!(
        remapper.input(press("A"), NOW),
        vec![press("A")],
        "input passes as it is while the remaps start"
    );
    assert!(remapper.timer(by - 1).is_empty());
    assert_eq!(remapper.deadline(), Some(by), "an early timer comes again");
    assert!(remapper.take_problems().is_empty());

    assert_eq!(remapper.timer(by), vec![]);
    assert_eq!(
        remapper.take_problems(),
        vec!["The input remaps failed (they did not answer in time) and start again."]
    );
    assert_eq!(
        remapper.input(release("A"), by),
        vec![release("A")],
        "a key pressed while they started is let go as it is"
    );
}
