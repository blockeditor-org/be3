use super::*;

/// The host menu lists each block this device keeps hosting with no editor open
/// (plans/hosted_blocks.md, "Hosting"), and picking one stops hosting it.
#[test]
fn the_menu_lists_what_this_device_keeps_hosting_with_stop() {
    let world = uuid::Uuid::from_u128(7);
    let mut shown = Shown::status(StatusView {
        sync: SyncState::Saved,
        organization: "Acme".to_owned(),
        signed_in_as: "Signed in as Ada".to_owned(),
        hosting: vec![(world, "Island world".to_owned())],
        ..Default::default()
    });
    let id = format!("host.menu.hosting.{world}");
    assert!(shown.stepped.document().find_test_id(&id).is_some());
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
