use super::*;

#[test]
fn display_confirmation_messages_round_trip() {
    host_value_round_trips::<DisplayConfirmation>(Some(PendingDisplayChange {
        round: 3,
        timeout: std::time::Duration::from_secs(15),
    }));
    host_value_round_trips::<DisplayConfirmation>(None);
    host_action_round_trips(DisplayAnswer::Keep(3));
    host_action_round_trips(DisplayAnswer::Revert(3));
}
