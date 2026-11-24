use be_sdk_beui::be_block::{
    Allowance, LinkSharing, OrganizationPolicies, OrganizationPoliciesContent, Settings,
    SettingsContent, organization_settings,
};
use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::{
    AccessGrant, AccessLevel, AccessListing, BlockInfo, LinkAdmits, ShareLink, ShellDialog,
};
use be_sdk_test::HostCall;
use be_type_workspace::FolderContent;

use super::*;

/// Answers the share dialog's listings of `block` among the host calls waiting, as a host with
/// one link and Ada, who opened it, would; answers the other calls.
fn answer_listings(fixture: &mut Fixture, block: Uuid) -> Vec<HostCall> {
    let me = fixture.host.account_id();
    let grant =
        |account: Uuid, name: &str, effective: AccessLevel, through_link: bool| AccessGrant {
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
            guest: false,
        };
    let mut others = Vec::new();
    for call in fixture.test.take_requests() {
        match call {
            HostCall::ListAccess {
                reply,
                block: asked,
            } if asked == block.into_bytes() => {
                fixture.test.answer(
                    reply,
                    Ok(AccessListing {
                        grants: vec![
                            grant(me, "Me", AccessLevel::Manage, false),
                            grant(Uuid::from_u128(0x5a_0101), "Ada", AccessLevel::View, true),
                        ],
                        company: true,
                        set_inside: 0,
                        levels: Vec::new(),
                        host: None,
                        offered: None,
                    }),
                );
            }
            HostCall::ListShareLinks {
                reply,
                block: asked,
            } if asked == block.into_bytes() => {
                fixture.test.answer(
                    reply,
                    Ok(vec![ShareLink {
                        id: Uuid::from_u128(0x11).to_string(),
                        access: AccessLevel::View,
                        admits: LinkAdmits::Members,
                        url: "https://be3.example/#share/ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGH"
                            .to_owned(),
                        maker: me.into_bytes(),
                        maker_name: "Me".to_owned(),
                    }]),
                );
            }
            call => others.push(call),
        }
    }
    fixture.settle();
    others
}

/// Holds the organization's settings with its policies, which let links admit only members, as
/// every member reads them; returns the policies' block.
fn hold_policies(fixture: &mut Fixture) -> Uuid {
    let settings = organization_settings(fixture.host.organization_id());
    let policies = Uuid::from_u128(0x5a_0201);
    let store = fixture.test.store();
    store.add_block(BlockInfo::new(
        settings,
        SettingsContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    store.add_block(BlockInfo::new(
        policies,
        OrganizationPoliciesContent::CONTENT_TYPE,
        BlockParent::Block(settings),
    ));
    let mut held = SettingsContent::default();
    held.apply(&Settings::set_policy(
        OrganizationPoliciesContent::CONTENT_TYPE,
        policies,
    ));
    store.hold(Some(settings), held);
    let mut set = OrganizationPoliciesContent::default();
    set.apply(&OrganizationPolicies::set_outside_sharing(Some(
        Allowance::Allowed,
    )));
    store.hold(Some(policies), set);
    policies
}

#[test]
fn the_share_dialog_makes_copies_and_revokes_share_links() {
    let (mut fixture, block) = editor_sized(Some(Vec2::new(900.0, 1000.0)));
    let store = fixture.test.store();
    let mut shared = BlockInfo::new(block, FolderContent::CONTENT_TYPE, BlockParent::Root);
    shared.set_name("Plans");
    store.add_block(shared);
    let policies = hold_policies(&mut fixture);
    fixture.test.show_dialog(block, ShellDialog::Share);
    fixture.settle();
    answer_listings(&mut fixture, block);
    assert!(
        !fixture.says("People in the organization"),
        "while the policy lets links admit only members, there is no one else to choose"
    );

    // An administrator lets links admit anyone: the open dialog follows, listing nothing again.
    fixture.test.edit::<OrganizationPoliciesContent>(
        Some(policies),
        &OrganizationPolicies::set_link_sharing(Some(LinkSharing::Anyone)),
    );
    fixture.settle();
    assert!(
        fixture.test.take_requests().iter().all(|call| !matches!(
            call,
            HostCall::ListAccess { .. } | HostCall::ListShareLinks { .. }
        )),
        "the policy reaches the dialog without listing again"
    );

    assert!(
        fixture.says("Anyone in the organization with the link can view. Made by you."),
        "a link says whom it admits and what it grants"
    );
    assert!(
        fixture.says("Can view, through a share link"),
        "someone who opened a link is listed as such"
    );
    assert!(
        fixture.says("People in the organization"),
        "a company that allows it chooses whom a link admits"
    );
    fixture
        .test
        .snapshot("the_share_dialog_makes_copies_and_revokes_share_links");

    fixture.test.click("share.links.make");
    fixture.settle();
    let made = answer_listings(&mut fixture, block)
        .into_iter()
        .find_map(|call| match call {
            HostCall::MakeShareLink {
                reply,
                access,
                admits,
                ..
            } => Some((reply, access, admits)),
            _ => None,
        })
        .expect("Make link asks the host for a link");
    assert_eq!(
        (made.1, made.2),
        (AccessLevel::View, LinkAdmits::Members),
        "a link grants viewing to members unless the person chooses otherwise"
    );
    fixture
        .test
        .answer(made.0, Ok("https://be3.example/#share/NEW".to_owned()));
    fixture.settle();
    answer_listings(&mut fixture, block);

    fixture.test.click("share.links.revoke");
    fixture.settle();
    assert!(
        answer_listings(&mut fixture, block)
            .iter()
            .all(|call| !matches!(call, HostCall::RevokeShareLink { .. })),
        "revoking asks to confirm first"
    );
    assert!(fixture.says("Revoke this link?"));
    fixture.test.click("share.links.revoke.confirm");
    fixture.settle();
    let revoked = answer_listings(&mut fixture, block)
        .into_iter()
        .find_map(|call| match call {
            HostCall::RevokeShareLink { link, .. } => Some(link),
            _ => None,
        });
    assert_eq!(revoked, Some(Uuid::from_u128(0x11).to_string()));
}
