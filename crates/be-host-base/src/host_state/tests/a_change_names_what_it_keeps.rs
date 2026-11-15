use super::*;

/// A change that could not be kept is shown by what it keeps, in the person's words.
#[test]
fn a_change_names_what_it_keeps() {
    let remote = account(
        ServerLocation::Remote("https://be.example".into()),
        "a@example",
    );
    assert_eq!(
        Write::SaveAccount(remote.clone()).what(),
        "signing in to https://be.example"
    );
    assert_eq!(
        Write::SetSetting("active_account".into(), remote.id.to_string()).what(),
        "switching accounts"
    );
    assert_eq!(
        Write::SetIdentity(remote, [0; 32]).what(),
        "your unlocked keys"
    );
    assert_eq!(Write::SetInputSettings(Vec::new()).what(), "a setting");
}
