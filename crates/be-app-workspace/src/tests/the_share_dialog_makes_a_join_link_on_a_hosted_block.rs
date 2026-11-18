use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, BlockInfo, LinkAdmits, ShellDialog};
use be_sdk_test::HostCall;
use be_type_workspace::FolderContent;

use super::*;

/// On a hosted block a share link grants Join unless the person chooses more
/// (plans/hosted_blocks.md, "The share dialog").
#[test]
fn the_share_dialog_makes_a_join_link_on_a_hosted_block() {
    let (mut fixture, block) = editor_sized(Some(Vec2::new(900.0, 800.0)));
    let me = fixture.host.account_id();
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
                    }],
                    company: false,
                    set_inside: 0,
                    links_admit: Some(LinkAdmits::Members),
                    hosted: true,
                }),
            ),
            HostCall::ListShareLinks { reply, .. } => fixture.test.answer(reply, Ok(Vec::new())),
            _ => {}
        }
    }
    fixture.settle();
    assert!(
        fixture.says(AccessLevel::Join.label()),
        "a hosted block's link offers Join first"
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
}
