use super::*;

#[test]
fn power_messages_round_trip() {
    let available = PowerAvailability {
        lock: true,
        suspend: true,
        restart: true,
        power_off: false,
        log_out: true,
    };
    host_value_round_trips::<Power>(available);
    for action in PowerAction::ALL {
        host_action_round_trips(action);
    }
    assert!(available.allows(PowerAction::Restart));
    assert!(!available.allows(PowerAction::PowerOff));
}
