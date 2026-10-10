use block_editor_beui::{HostProblem, ProblemAction, Problems};

use super::*;

#[test]
fn a_problem_the_host_reports_shows_until_it_is_dismissed() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_host_value::<Problems>(&vec![HostProblem {
        id: 3,
        message: "Terminal did not start: No such file or directory".to_owned(),
    }]);
    fixture.settle();
    assert!(fixture.says("Terminal did not start"));
    fixture.test.snapshot("a_problem_toast");

    fixture.test.click("toast.3.dismiss");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<ProblemAction>(),
        vec![ProblemAction::Dismiss(3)]
    );
}
