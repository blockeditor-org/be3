use be_sdk_beui::be_block::organization_settings;

use super::*;

#[test]
fn settings_only_their_owner_reads_are_lost_with_the_recovery_phrase() {
    let personal = editor();
    assert!(
        personal
            .document()
            .find_test_id("settings.only-yours")
            .is_none(),
        "with no administrators to recover anything, there is nothing to tell apart"
    );

    let organization = Uuid::from_u128(0x5e77_0f0f);
    let mut company = editor_in::<SettingsRoot>(Uuid::nil(), organization);
    company.set_size(Vec2::new(480.0, 900.0));
    company.store().add_block(BlockInfo::new(
        organization_settings(organization),
        SettingsContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    company.run();
    assert!(
        company
            .document()
            .find_test_id("settings.only-yours")
            .is_some()
    );
    company.snapshot("settings_only_their_owner_reads_are_lost_with_the_recovery_phrase");
}
