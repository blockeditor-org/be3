use be_sdk_beui::{AccessListing, ShellDialog};
use be_sdk_test::HostCall;

use super::*;

#[test]
fn an_editor_cannot_swap_the_block_of_an_open_share_dialog() {
    let (mut fixture, shared) = editor();
    let other = Uuid::new_v4();
    fixture.test.show_dialog(shared, ShellDialog::Share);
    fixture.settle();
    let listing = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ListAccess { reply, block } if block == shared.into_bytes() => Some(reply),
            _ => None,
        })
        .expect("opening the share dialog lists who can open the block");
    fixture.test.answer(
        listing,
        Ok(AccessListing {
            grants: Vec::new(),
            company: false,
            set_inside: 0,
        }),
    );
    fixture.settle();

    fixture.test.show_dialog(other, ShellDialog::Share);
    fixture.settle();
    fixture.test.click("share.invite");
    fixture.settle();

    let asked: Vec<[u8; 16]> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::ListAccess { block, .. }
            | HostCall::SetAccess { block, .. }
            | HostCall::MakeContactLink { block, .. } => Some(block),
            _ => None,
        })
        .collect();
    assert!(
        !asked.contains(&other.into_bytes()),
        "nothing in the open dialog reaches the block an editor asked for later: {asked:?}"
    );
    assert!(
        asked.contains(&shared.into_bytes()),
        "the invite is for the block the dialog shows: {asked:?}"
    );
}
