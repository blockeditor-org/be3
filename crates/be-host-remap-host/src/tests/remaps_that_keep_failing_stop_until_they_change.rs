use super::*;

#[test]
fn remaps_that_keep_failing_stop_until_they_change() {
    let mut remapper = hostile("(unreachable)");

    for second in 0..FAILURES_BEFORE_STOPPING as u64 {
        let now = NOW + second * 1_000_000;
        assert_eq!(remapper.input(press("A"), now), vec![press("A")]);
        assert_eq!(remapper.input(release("A"), now), vec![release("A")]);
    }
    let problems = remapper.take_problems();
    assert!(
        problems
            .last()
            .is_some_and(|problem| problem.contains("stopped")),
        "{problems:?}"
    );
    assert!(!remapper.running());
    assert_eq!(remapper.input(press("B"), NOW), vec![press("B")]);
    assert!(
        remapper.take_problems().is_empty(),
        "stopped remaps are not called"
    );

    remapper.configure(Some(vec![1]));
    assert!(remapper.running(), "new remaps get a new start");
}
