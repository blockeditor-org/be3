use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, BlockInfo, OfferedLevel, ShellDialog};
use be_sdk_test::HostCall;
use be_type_workspace::FolderContent;

use super::*;

/// A hosted block's dialog offers its type's levels by their names, each said in a line, and
/// a share link grants Join unless the person chooses more (plans/hosted_blocks.md, "The share
/// dialog"): a game's Join game, Host and Manage. It says who is hosting the world now.
/// Someone invited by a contact link starts at Join too.
#[test]
fn the_share_dialog_makes_a_join_link_on_a_hosted_block() {
    let (mut fixture, block) = editor_sized(Some(Vec2::new(900.0, 800.0)));
    let me = fixture.host.account_id();
    let ada = Uuid::from_u128(0x5a_0301);
    let store = fixture.test.store();
    let mut world = BlockInfo::new(block, FolderContent::CONTENT_TYPE, BlockParent::Root);
    world.set_name("World");
    store.add_block(world);
    fixture.test.show_dialog(block, ShellDialog::Share);
    fixture.settle();
    for call in fixture.test.take_requests() {
        match call {
            HostCall::ListAccess { reply, .. } => fixture.test.answer(
                reply,
                Ok(AccessListing {
                    grants: vec![AccessGrant {
                        account: me.into_bytes(),
                        email: "me@example.org".to_owned(),
                        display_name: "Me".to_owned(),
                        administrator: false,
                        author: true,
                        granted: None,
                        inherited: AccessLevel::Manage,
                        effective: AccessLevel::Manage,
                        source: Some(block.into_bytes()),
                        waiting: false,
                        through_link: false,
                        guest: false,
                    }, AccessGrant {
                        account: ada.into_bytes(),
                        email: "ada@example.org".to_owned(),
                        display_name: "Ada".to_owned(),
                        administrator: false,
                        author: false,
                        granted: Some(AccessLevel::Edit),
                        inherited: AccessLevel::None,
                        effective: AccessLevel::Edit,
                        source: Some(block.into_bytes()),
                        waiting: false,
                        through_link: false,
                        guest: false,
                    }],
                    company: false,
                    set_inside: 0,
                    levels: vec![
                        OfferedLevel {
                            access: AccessLevel::Join,
                            name: "Join game".to_owned(),
                            line: "Connect to whoever hosts the world.".to_owned(),
                        },
                        OfferedLevel {
                            access: AccessLevel::Edit,
                            name: "Host".to_owned(),
                            line: "Can run the world, and so read and change anything in it, outside the game's rules.".to_owned(),
                        },
                        OfferedLevel {
                            access: AccessLevel::Manage,
                            name: "Manage".to_owned(),
                            line: "Can also share the world.".to_owned(),
                        },
                    ],
                    host: Some(ada.into_bytes()),
                    offered: None,
                }),
            ),
            HostCall::ListShareLinks { reply, .. } => fixture.test.answer(reply, Ok(Vec::new())),
            _ => {}
        }
    }
    fixture.settle();
    assert!(
        fixture.says("Join game"),
        "a hosted block's link offers its type's Join first, by its name"
    );
    assert!(
        fixture.says("Connect to whoever hosts the world."),
        "with the line saying what it means"
    );
    assert!(
        !fixture.says(AccessLevel::Join.label()),
        "never by the core name"
    );
    assert!(
        fixture.says("Host, set here"),
        "a member's access is named by the type too: Ada hosts"
    );
    assert!(
        fixture.says("Ada is hosting it now."),
        "the dialog says who hosts the world now"
    );
    fixture
        .test
        .snapshot("the_share_dialog_makes_a_join_link_on_a_hosted_block");

    fixture.test.click("share.links.make");
    fixture.settle();
    let made = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::MakeShareLink { access, .. } => Some(access),
            _ => None,
        });
    assert_eq!(made, Some(AccessLevel::Join));

    fixture.test.click("share.invite");
    fixture.settle();
    let invited = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::MakeContactLink { access, .. } => Some(access),
            _ => None,
        });
    assert_eq!(
        invited,
        Some(AccessLevel::Join),
        "someone invited to a hosted block starts at Join"
    );
}
