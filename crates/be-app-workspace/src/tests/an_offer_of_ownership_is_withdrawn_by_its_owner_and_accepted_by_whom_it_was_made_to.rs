use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, ShellDialog};
use be_sdk_test::HostCall;

use super::*;

/// An offer of ownership standing on a block shows in its share dialog: to its owner, who may
/// withdraw it, and to the member it was made to, who may accept it (guides/blocks/keys.md,
/// "Owner records").
#[test]
fn an_offer_of_ownership_is_withdrawn_by_its_owner_and_accepted_by_whom_it_was_made_to() {
    let ada = Uuid::from_u128(0x3a_00c0);
    let grace = Uuid::from_u128(0x3a_00c1);
    for owner in [true, false] {
        let (mut fixture, block) = editor();
        let me = fixture.host.account_id();
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
        let grant = |account: Uuid, name: &str, author: bool| AccessGrant {
            account: account.into_bytes(),
            email: format!("{}@example.org", name.to_lowercase()),
            display_name: name.to_owned(),
            administrator: false,
            author,
            granted: (!author).then_some(AccessLevel::Edit),
            inherited: AccessLevel::None,
            effective: match author {
                true => AccessLevel::Manage,
                false => AccessLevel::Edit,
            },
            source: Some(block.into_bytes()),
            waiting: false,
            through_link: false,
            guest: false,
        };
        // Owning it, the person offered it to Ada; else Grace, its owner, offered it to them.
        let (grants, offered) = match owner {
            true => (vec![grant(me, "Me", true), grant(ada, "Ada", false)], ada),
            false => (
                vec![grant(grace, "Grace", true), grant(me, "Me", false)],
                me,
            ),
        };
        fixture.test.answer(
            listing,
            Ok(AccessListing {
                grants,
                company: false,
                set_inside: 0,
                levels: Vec::new(),
                host: None,
                offered: Some(offered.into_bytes()),
            }),
        );
        fixture.settle();
        if owner {
            assert!(fixture.says("Ownership offered to Ada"));
            assert!(!fixture.test.shown("share.accept"));
            fixture.test.click("share.withdraw");
            fixture.settle();
            assert_eq!(
                fixture.test.take_ownership_transfers(),
                vec![(block, me)],
                "an owner withdraws an offer by offering the block to themselves"
            );
        } else {
            assert!(fixture.says("Grace offered you ownership"));
            assert!(!fixture.test.shown("share.withdraw"));
            fixture.test.snapshot(
                "an_offer_of_ownership_is_withdrawn_by_its_owner_and_accepted_by_whom_it_was_made_to",
            );
            fixture.test.click("share.accept");
            fixture.settle();
            assert_eq!(fixture.test.take_ownership_acceptances(), vec![block]);
        }
    }
}
