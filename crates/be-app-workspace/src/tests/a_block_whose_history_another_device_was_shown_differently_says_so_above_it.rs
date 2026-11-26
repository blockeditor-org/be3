use super::*;

use be_sdk_beui::{BlockCommand, BlockInfo, LogDoubt, LogDoubtKind, LogDoubtSource};

/// A block whose signed log this device found forked (guides/blocks/keys.md, "Forks") says so
/// in a bar above it, which still lets the person write: Details says where the other history
/// came from, and Dismiss hides the bar on this device, the verdict kept.
#[test]
fn a_block_whose_history_another_device_was_shown_differently_says_so_above_it() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1100.0, 600.0)));
    let store = fixture.test.store();
    let mut info = BlockInfo::new(opened, SHOWN_TYPE, BlockParent::Root);
    info.set_name("Plans");
    info.log_doubt = Some(LogDoubt {
        kind: LogDoubtKind::Forked,
        source: LogDoubtSource::Account,
        account: None,
        ours: 4,
        theirs: 3,
    });
    store.add_block(info);

    show(&mut fixture, opened, None);
    // The block is open and writable beneath the bar.
    fixture.test.available_children();
    fixture.settle();

    assert!(
        fixture.says(
            "Another device was shown a different history of this block. The server may be showing people different versions."
        ),
        "the bar says the block's history was shown differently"
    );
    fixture.click_text("Details");
    assert!(
        fixture.says(
            "Found comparing with another of your devices. This device holds 4 entries of the block's history, the other 3."
        ),
        "Details says where the other history came from"
    );
    fixture
        .test
        .snapshot("a_block_whose_history_another_device_was_shown_differently_says_so_above_it");

    fixture.test.take_block_commands();
    fixture.click_text("Dismiss");
    assert!(
        fixture
            .test
            .take_block_commands()
            .contains(&(opened, BlockCommand::DismissLogDoubt)),
        "dismissing asks the host to hide the doubt on this device"
    );
}
