use super::*;

#[test]
fn power_messages_round_trip() {
    let available = PowerAvailability {
        lock: true,
        suspend: true,
        restart: true,
        power_off: false,
        log_out: true,
        session: true,
        ending: None,
    };
    watched_round_trips::<PowerWatch>(available);
    for end in SessionEnd::ALL {
        watched_round_trips::<PowerWatch>(PowerAvailability {
            ending: Some(end),
            ..available
        });
    }
    for answer in EndingAnswer::ALL {
        round_trips(&answer);
    }
    for action in PowerAction::ALL {
        round_trips(&action);
    }
    assert!(available.allows(PowerAction::Restart));
    assert!(!available.allows(PowerAction::PowerOff));
}
