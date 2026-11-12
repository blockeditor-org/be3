use be_sdk_beui::beui::{Key, Modifiers};
use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, Failure, ShellDialog};
use be_sdk_test::HostCall;

use super::*;

const OFFLINE: &str =
    "You're offline. Sharing changes need a connection; try again once you're back online.";

#[test]
fn a_share_refused_offline_says_why_in_the_dialog() {
    let (mut fixture, block) = editor();
    let me = fixture.host.account_id();
    let friend = Uuid::new_v4();
    fixture.test.show_dialog(block, ShellDialog::Share);
    fixture.settle();
    let listing = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ListAccess { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("opening the share dialog asks who can open the block");
    let grant = |account: Uuid, name: &str, granted: Option<AccessLevel>| AccessGrant {
        account: account.into_bytes(),
        email: format!("{name}@example.org"),
        display_name: name.to_owned(),
        administrator: false,
        author: false,
        granted,
        inherited: AccessLevel::None,
        effective: granted.unwrap_or(AccessLevel::None),
        source: granted.map(|_| block.into_bytes()),
        waiting: false,
    };
    fixture.test.answer(
        listing,
        Ok(AccessListing {
            grants: vec![
                grant(me, "Me", Some(AccessLevel::Manage)),
                grant(friend, "Friend", None),
            ],
            company: false,
            set_inside: 0,
        }),
    );
    fixture.settle();

    fixture.test.click("share.query");
    fixture.settle();
    fixture.test.type_text("fri");
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    fixture.test.click("share.add");
    fixture.settle();
    let reply = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::SetAccess { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("adding someone asks the host to grant them");
    fixture
        .test
        .answer(reply, Err(Failure::Failed(OFFLINE.to_owned())));
    fixture.settle();
    assert!(
        fixture.says(OFFLINE),
        "the dialog where the share was made says why it did not take"
    );
    fixture.test.snapshot("a_share_refused_offline_says_why_in_the_dialog");
}
