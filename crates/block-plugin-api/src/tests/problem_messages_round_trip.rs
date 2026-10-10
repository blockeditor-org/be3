use super::*;

#[test]
fn problem_messages_round_trip() {
    host_value_round_trips::<Problems>(vec![HostProblem {
        id: 2,
        message: "foot did not start: not found".into(),
    }]);
    host_action_round_trips(ProblemAction::Dismiss(2));
}
