use block_editor_beui::beui::{Key, Modifiers};
use block_editor_beui::{
    AccessGrant, AccessLevel, AccessListing, HostReply, HostRequest, ShellDialog,
};

use super::*;

#[test]
fn sharing_a_block_grants_the_people_added_and_lists_them_again() {
    let (mut fixture, block) = editor();
    let me = fixture.host.account_id();
    let friend = Uuid::new_v4();
    let maker = Uuid::new_v4();
    fixture.test.show_dialog(block, ShellDialog::Share);
    fixture.settle();

    let listing = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|(request, asked)| {
            (asked == HostRequest::ListAccess(block.into_bytes())).then_some(request)
        })
        .expect("opening the share dialog asks who can open the block");
    let grant = |account: Uuid, name: &str, granted: Option<AccessLevel>| AccessGrant {
        account: account.into_bytes(),
        email: format!("{name}@example.org"),
        display_name: name.to_owned(),
        administrator: false,
        author: false,
        granted,
        effective: granted.unwrap_or(AccessLevel::None),
        source: granted.map(|_| block.into_bytes()),
        waiting: false,
    };
    fixture.test.reply(
        listing,
        HostReply::AccessListed(AccessListing::Listed {
            grants: vec![
                grant(me, "Me", Some(AccessLevel::Edit)),
                grant(friend, "Friend", None),
                AccessGrant {
                    author: true,
                    effective: AccessLevel::Manage,
                    source: Some(block.into_bytes()),
                    ..grant(maker, "Maker", None)
                },
            ],
            company: false,
            set_inside: 0,
        }),
    );
    fixture.settle();
    assert!(fixture.says("This is you"));
    assert!(
        fixture.says("Made this block") && !fixture.says("Can manage, set here"),
        "the block's author is listed as its author, not as inheriting access"
    );
    assert!(
        !fixture.says("Friend"),
        "people without access are only offered once searched for"
    );

    fixture.test.click("share.query");
    fixture.settle();
    fixture.test.type_text("fri");
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    fixture.test.click("share.add");
    fixture.settle();

    assert_eq!(
        fixture.test.take_access_changes(),
        vec![(block, friend, AccessLevel::Edit)]
    );
    assert!(
        fixture
            .test
            .take_requests()
            .iter()
            .any(|(_, asked)| *asked == HostRequest::ListAccess(block.into_bytes())),
        "the members are listed again after a grant"
    );
}
