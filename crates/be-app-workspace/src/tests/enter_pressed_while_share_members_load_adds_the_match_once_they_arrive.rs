use be_sdk_beui::beui::{Key, Modifiers};
use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, ShellDialog};
use be_sdk_test::HostCall;

use super::*;

#[test]
fn enter_pressed_while_share_members_load_adds_the_match_once_they_arrive() {
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
            HostCall::ListAccess {
                reply,
                block: asked,
            } if asked == block.into_bytes() => Some(reply),
            _ => None,
        })
        .expect("opening the share dialog asks who can open the block");
    assert!(fixture.says("Loading members…"));

    fixture.test.click("share.query");
    fixture.settle();
    fixture.test.type_text("fri");
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert!(fixture.says("Loading members…"));

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
                grant(me, "Me", Some(AccessLevel::Edit)),
                grant(friend, "Friend", None),
            ],
            company: false,
            set_inside: 0,
        }),
    );
    fixture.settle();
    assert!(
        fixture.says("Friend"),
        "the Enter pressed while loading adds the match once the members arrive"
    );

    fixture.test.click("share.add");
    fixture.settle();
    assert_eq!(
        fixture.test.take_access_changes(),
        vec![(block, friend, AccessLevel::Edit)]
    );
}
