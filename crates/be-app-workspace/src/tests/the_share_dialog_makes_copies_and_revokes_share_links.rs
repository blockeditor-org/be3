use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::{
    AccessGrant, AccessLevel, AccessListing, BlockInfo, LinkAdmits, ShareLink, ShellDialog,
};
use be_sdk_test::HostCall;
use be_type_workspace::FolderContent;

use super::*;

#[test]
fn the_share_dialog_makes_copies_and_revokes_share_links() {
    let (mut fixture, block) = editor_sized(Some(Vec2::new(900.0, 1000.0)));
    let me = fixture.host.account_id();
    let store = fixture.test.store();
    let mut shared = BlockInfo::new(block, FolderContent::CONTENT_TYPE, BlockParent::Root);
    shared.set_name("Plans");
    store.add_block(shared);
    fixture.test.show_dialog(block, ShellDialog::Share);
    fixture.settle();

    let mut listing = None;
    let mut links = None;
    for call in fixture.test.take_requests() {
        match call {
            HostCall::ListAccess { reply, block: asked } if asked == block.into_bytes() => {
                listing = Some(reply)
            }
            HostCall::ListShareLinks { reply, block: asked } if asked == block.into_bytes() => {
                links = Some(reply)
            }
            _ => {}
        }
    }
    let grant = |account: Uuid, name: &str, effective: AccessLevel, through_link: bool| {
        AccessGrant {
            account: account.into_bytes(),
            email: format!("{}@example.org", name.to_lowercase()),
            display_name: name.to_owned(),
            administrator: false,
            author: account == me,
            granted: (account != me).then_some(effective),
            inherited: AccessLevel::None,
            effective,
            source: Some(block.into_bytes()),
            waiting: false,
            through_link,
        }
    };
    fixture.test.answer(
        listing.expect("opening the share dialog asks who can open the block"),
        Ok(AccessListing {
            grants: vec![
                grant(me, "Me", AccessLevel::Manage, false),
                grant(Uuid::from_u128(0x5a_0101), "Ada", AccessLevel::View, true),
            ],
            company: true,
            set_inside: 0,
            links_admit: Some(LinkAdmits::Anyone),
        }),
    );
    fixture.test.answer(
        links.expect("a manager's share dialog asks for the block's share links"),
        Ok(vec![ShareLink {
            id: Uuid::from_u128(0x11).to_string(),
            access: AccessLevel::View,
            admits: LinkAdmits::Members,
            url: "https://be3.example/#share/ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGH".to_owned(),
            maker_name: "Me".to_owned(),
        }]),
    );
    fixture.settle();

    assert!(
        fixture.says("Anyone in the organization with the link can view. Made by Me."),
        "a link says whom it admits and what it grants"
    );
    assert!(
        fixture.says("View, through a share link"),
        "someone who opened a link is listed as such"
    );
    assert!(fixture.says("Who can open it"), "a company that allows it chooses whom a link admits");
    fixture
        .test
        .snapshot("the_share_dialog_makes_copies_and_revokes_share_links");

    fixture.test.click("share.links.make");
    fixture.settle();
    let made = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::MakeShareLink {
                access, admits, ..
            } => Some((access, admits)),
            _ => None,
        });
    assert_eq!(
        made,
        Some((AccessLevel::View, LinkAdmits::Members)),
        "a link grants viewing to members unless the person chooses otherwise"
    );

    fixture.test.click("share.links.revoke");
    fixture.settle();
    let revoked = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::RevokeShareLink { link, .. } => Some(link),
            _ => None,
        });
    assert_eq!(revoked, Some(Uuid::from_u128(0x11).to_string()));
}
