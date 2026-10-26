use super::*;

#[test]
fn only_the_default_desktop_takes_input() {
    assert_eq!(InputDesktop::named(Some("Default")), InputDesktop::Default);
    assert_eq!(InputDesktop::named(Some("default")), InputDesktop::Default);
    assert_eq!(InputDesktop::named(Some("Winlogon")), InputDesktop::Secure);
    assert_eq!(InputDesktop::named(None), InputDesktop::Secure);
    assert_eq!(InputDesktop::Default.reachable(), Ok(()));
    assert_eq!(
        InputDesktop::Secure.reachable(),
        Err(InjectError::SecureDesktop)
    );
}
