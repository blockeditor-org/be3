use super::*;

#[test]
fn another_change_while_asking_asks_again_with_the_full_time() {
    let start = Duration::from_secs(5);
    let mut guard = showing(gaming_monitor(), saved(Some(slow())));
    guard.take(saved(Some(small())));
    guard.prompt(start);

    let later = start + Duration::from_secs(10);
    guard.take(saved(Some(fast())));
    assert_eq!(
        guard.prompt(later),
        Prompt::Asking {
            round: 2,
            left: ANSWER_WITHIN
        }
    );

    guard.revert();
    assert_eq!(
        guard.applied().mode(MONITOR),
        Some(slow()),
        "reverting goes back to the mode before either change"
    );
}
