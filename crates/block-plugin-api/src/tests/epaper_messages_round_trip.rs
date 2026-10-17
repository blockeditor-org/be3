use super::*;

#[test]
fn epaper_messages_round_trip() {
    host_value_round_trips::<Epaper>(true);
    host_action_round_trips(EpaperRequest::Refresh);
}
