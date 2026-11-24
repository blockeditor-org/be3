use super::*;

/// The host menu lists each block this device keeps hosting with no editor open
/// (plans/hosted_blocks.md, "Hosting"), saying whether it hosts it now or stands by while
/// another device does, and picking one stops hosting it.
#[test]
fn the_menu_lists_what_this_device_keeps_hosting_with_stop() {
    let world = uuid::Uuid::from_u128(7);
    let farm = uuid::Uuid::from_u128(8);
    let mut shown = Shown::status(StatusView {
        sync: SyncState::Saved,
        organization: "Acme".to_owned(),
        signed_in_as: "Signed in as Ada".to_owned(),
        hosting: vec![
            KeptHost {
                block: farm,
                name: "Farm".to_owned(),
                now: false,
            },
            KeptHost {
                block: world,
                name: "Island world".to_owned(),
                now: true,
            },
        ],
        ..Default::default()
    });
    // Each block keeps one id, whether it hosts now or stands by: its detail says which.
    let id = format!("host.menu.hosting.{world}");
    assert!(shown.stepped.document().find_test_id(&id).is_some());
    assert!(
        shown
            .stepped
            .document()
            .find_test_id(&format!("host.menu.hosting.{farm}"))
            .is_some(),
        "one kept but hosted elsewhere is listed too"
    );
    shown.painted("the_menu_lists_what_this_device_keeps_hosting_with_stop");
    crate::ui::take_commands();
    shown
        .stepped
        .click(beui_host::native::testing::Target::id(&id));
    assert!(
        crate::ui::take_commands()
            .iter()
            .any(|command| matches!(command, UiCommand::StopHosting(block) if *block == world)),
        "Stop hosting stops it"
    );
}
