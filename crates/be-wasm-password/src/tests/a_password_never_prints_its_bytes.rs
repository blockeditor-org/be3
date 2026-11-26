use super::*;

#[test]
fn a_password_never_prints_its_bytes() {
    let password = Password::from("hunter2".to_owned());
    assert_eq!(format!("{password:?}"), "Password(..)");
    assert_eq!(password.as_str(), Some("hunter2"));
}
