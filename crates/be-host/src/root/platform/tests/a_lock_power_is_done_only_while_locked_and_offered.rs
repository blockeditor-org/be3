use super::*;

#[test]
fn a_lock_power_is_done_only_while_locked_and_offered() {
    let offered = [
        be_sdk_api::PowerAction::Suspend,
        be_sdk_api::PowerAction::PowerOff,
    ];
    let asked = || {
        vec![
            PowerAction::Suspend,
            PowerAction::Restart,
            PowerAction::PowerOff,
        ]
    };
    assert_eq!(
        powers_to_do(true, &offered, asked()),
        vec![
            be_sdk_api::PowerAction::Suspend,
            be_sdk_api::PowerAction::PowerOff
        ],
        "while locked, what the lock screen offers is done, and nothing else"
    );
    assert!(
        powers_to_do(false, &offered, asked()).is_empty(),
        "nothing is done while the session is not locked"
    );
    assert!(
        powers_to_do(true, &[], asked()).is_empty(),
        "nothing is done when the host offers nothing"
    );
}
