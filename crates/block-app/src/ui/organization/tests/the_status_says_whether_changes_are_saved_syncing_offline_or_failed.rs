use super::*;

#[test]
fn the_status_says_whether_changes_are_saved_syncing_offline_or_failed() {
    for (sync, unsent, name) in [
        (SyncState::Saved, 0, "the_status_says_changes_are_saved"),
        (SyncState::Syncing, 0, "the_status_says_changes_are_syncing"),
        (SyncState::Connecting, 0, "the_status_says_it_is_connecting"),
        (
            SyncState::Offline { waiting: 3 },
            1,
            "the_status_says_it_is_offline_with_changes_waiting",
        ),
        (
            SyncState::Error("the server refused the change".to_owned()),
            0,
            "the_status_says_changes_could_not_sync",
        ),
    ] {
        let offline = matches!(sync, SyncState::Offline { .. });
        let mut shown = Shown::menu(sync, unsent);
        assert!(
            shown
                .stepped
                .document()
                .find_test_id("app.menu.status")
                .is_some()
        );
        assert_eq!(
            shown
                .stepped
                .document()
                .find_test_id("app.menu.unsent")
                .is_some(),
            unsent > 0,
            "unsent edits are offered from the menu only while there are some"
        );
        shown.painted(name);
        if offline {
            shown
                .stepped
                .click(beui::testing::Target::id("app.menu.unsent"));
            assert!(
                crate::ui::take_commands()
                    .iter()
                    .any(|command| matches!(command, UiCommand::Unsent(true))),
                "the menu opens the unsent edits"
            );
        }
    }
}
