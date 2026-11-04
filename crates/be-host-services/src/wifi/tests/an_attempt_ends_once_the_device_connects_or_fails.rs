use super::*;

const DEACTIVATING: u32 = 110;
const CONFIG: u32 = 50;

#[test]
fn an_attempt_ends_once_the_device_connects_or_fails() {
    let mut attempts = Attempts::default();
    assert_eq!(
        attempts.state(FAILED, None),
        None,
        "with no attempt nothing ends"
    );

    attempts.start("Cafe".to_owned());
    assert_eq!(attempts.trying(), Some("Cafe"));
    for state in [ACTIVATED, DEACTIVATING, DISCONNECTED] {
        assert_eq!(
            attempts.state(state, Some("Home")),
            None,
            "the network being left going down is not the attempt failing"
        );
    }
    assert_eq!(attempts.state(PREPARE, None), None);
    assert_eq!(attempts.state(CONFIG, None), None);
    assert_eq!(attempts.state(ACTIVATED, None), Some(Outcome::Connected));
    assert_eq!(attempts.trying(), None);
    assert_eq!(attempts.failed, None);

    attempts.start("Home".to_owned());
    assert_eq!(
        attempts.state(ACTIVATED, Some("Home")),
        Some(Outcome::Connected),
        "a connection read once it is up still ends the attempt"
    );

    attempts.start("Flat".to_owned());
    attempts.state(PREPARE, None);
    assert_eq!(attempts.state(DISCONNECTED, None), Some(Outcome::Failed));
    assert_eq!(attempts.failed.as_deref(), Some("Flat"));

    attempts.start("Flat".to_owned());
    assert_eq!(attempts.failed, None, "trying again clears the failure");
    assert_eq!(
        attempts.state(FAILED, None),
        None,
        "the device still failed from the last attempt is not this one failing"
    );
    attempts.state(PREPARE, None);
    assert_eq!(attempts.state(FAILED, None), Some(Outcome::Failed));
    attempts.cancel();
    assert_eq!(attempts.failed, None);
    attempts.refused("Office".to_owned());
    assert_eq!(attempts.failed.as_deref(), Some("Office"));
}
