use super::*;

#[test]
fn keys_pasted_on_two_devices_for_two_accounts_are_both_kept() {
    let base = BoatSettingsContent::default();
    let ours = edited(&base, [BoatSettings::set_key("", Some(&key("boat_own")))]);
    let theirs = edited(
        &base,
        [BoatSettings::set_key("acme", Some(&key("boat_acme")))],
    );

    let both = merged(&base, &ours, &theirs).root();

    assert_eq!(
        both.key("").map(|key| key.secret.as_str()),
        Some("boat_own")
    );
    assert_eq!(
        both.key("acme").map(|key| key.secret.as_str()),
        Some("boat_acme")
    );

    let forgotten = edited(
        &BoatSettingsContent::new(&both),
        [BoatSettings::set_key("acme", None)],
    );
    assert_eq!(forgotten.root().key("acme"), None);
    assert_eq!(
        BoatSettingsContent::decode(&forgotten.encode()),
        Ok(forgotten)
    );
}
