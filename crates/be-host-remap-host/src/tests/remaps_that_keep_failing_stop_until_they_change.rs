use super::*;

#[test]
fn remaps_that_keep_failing_stop_until_they_change() {
    // They trap on `Start`, every time they are started again.
    let mut remapper = hostile("(unreachable)");

    assert_eq!(
        remapper.input(press("A"), NOW),
        vec![press("A")],
        "input passes while they start"
    );
    assert_eq!(remapper.input(release("A"), NOW), vec![release("A")]);
    remapper.started();
    let problems = remapper.take_problems();
    assert_eq!(problems.len(), FAILURES_BEFORE_STOPPING, "{problems:?}");
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
