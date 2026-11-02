use super::*;

#[test]
fn a_change_no_one_answers_expires_once_the_time_to_answer_is_up() {
    let start = Duration::from_secs(100);
    let mut guard = showing(gaming_monitor(), saved(Some(slow())));
    assert_eq!(guard.prompt(start), Prompt::Settled);

    guard.take(saved(Some(small())));
    assert_eq!(
        guard.prompt(start),
        Prompt::Asking {
            round: 1,
            left: ANSWER_WITHIN
        },
        "the time to answer starts when the change is first seen"
    );
    let later = start + ANSWER_WITHIN - Duration::from_secs(1);
    assert_eq!(
        guard.prompt(later),
        Prompt::Asking {
            round: 1,
            left: Duration::from_secs(1)
        }
    );
    assert_eq!(guard.prompt(start + ANSWER_WITHIN), Prompt::Expired);

    guard.revert();
    assert_eq!(guard.prompt(start + ANSWER_WITHIN), Prompt::Settled);
    assert_eq!(guard.applied().mode(MONITOR), Some(slow()));
}
