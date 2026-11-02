use super::*;

#[test]
fn power_messages_round_trip() {
    let available = PowerAvailability {
        lock: true,
        suspend: true,
        restart: true,
        power_off: false,
        log_out: true,
        ending: false,
    };
    watched_round_trips::<PowerWatch>(available);
    for action in PowerAction::ALL {
        round_trips(&action);
    }
    assert!(available.allows(PowerAction::Restart));
    assert!(!available.allows(PowerAction::PowerOff));
}
