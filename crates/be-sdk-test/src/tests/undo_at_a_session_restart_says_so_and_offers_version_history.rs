use super::*;

use be_sdk_beui::{BlockCommand, BlockHistory};
use beui::{Key, Modifiers};

#[test]
fn undo_at_a_session_restart_says_so_and_offers_version_history() {
    let block = Uuid::new_v4();
    let restarted = [7; 32];
    let mut test = BeuiTest::<ChildRoot>::editing(block).with_top_bar(false);
    test.set_histories([(
        block,
        BlockHistory {
            restarted_at: Some(restarted),
            ..Default::default()
        },
    )]);
    test.run();
    assert!(
        test.menu_entry("editor.undo").enabled,
        "undo stays offered, to say why it stops"
    );

    test.key(Key::Z, Modifiers::CTRL);
    test.run();
    assert!(
        test.take_block_commands().is_empty(),
        "undo asked the host for a step past the restart"
    );
    test.snapshot("undo_at_a_session_restart_says_so_and_offers_version_history");

    test.click("toast.1.action.history");
    test.run();
    assert_eq!(
        test.take_block_commands(),
        [(block, BlockCommand::History(Some(restarted)))]
    );
}
