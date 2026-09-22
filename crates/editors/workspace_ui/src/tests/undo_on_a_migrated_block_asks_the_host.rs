use super::*;

use block_client::blocks::calendar::Calendar;
use block_editor_plugin::BlockCommand;
use block_editor_plugin::BlockHistory;

#[test]
fn undo_on_a_migrated_block_asks_the_host() {
    let (mut fixture, _) = editor();
    let calendar = Uuid::new_v4();
    fixture
        .host
        .show_block(calendar, <Calendar as Block>::TYPE_ID, None, None);
    fixture.settle();
    let _ = fixture.host.take_block_commands();

    fixture.test.click("workspace.undo");
    fixture.settle();
    assert!(
        fixture.host.take_block_commands().is_empty(),
        "undo asked the host with nothing to undo"
    );

    fixture.host.set_histories([(
        calendar,
        BlockHistory {
            can_undo: true,
            can_redo: false,
        },
    )]);
    fixture.settle();
    fixture.test.click("workspace.undo");
    fixture.settle();

    assert_eq!(
        fixture.host.take_block_commands(),
        [(calendar, BlockCommand::Undo)]
    );
}
