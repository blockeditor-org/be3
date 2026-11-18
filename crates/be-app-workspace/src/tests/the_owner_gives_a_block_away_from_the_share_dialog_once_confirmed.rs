use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, ShellDialog};
use be_sdk_test::HostCall;

use super::*;

/// The block's owner offers ownership to someone with access from their row, and gives it only
/// once they confirm; no one else is offered it (guides/blocks/keys.md, "Owner records").
#[test]
fn the_owner_gives_a_block_away_from_the_share_dialog_once_confirmed() {
    let (mut fixture, block) = editor();
    let me = fixture.host.account_id();
    let colleague = Uuid::from_u128(0x3a_00c0);
    let stranger = Uuid::from_u128(0x3a_00c1);
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
    let grant = |account: Uuid, name: &str, granted: AccessLevel| AccessGrant {
        account: account.into_bytes(),
        email: format!("{}@example.org", name.to_lowercase()),
        display_name: name.to_owned(),
        administrator: false,
        author: false,
        granted: Some(granted),
        inherited: AccessLevel::None,
        effective: granted,
        source: Some(block.into_bytes()),
        waiting: false,
        through_link: false,
        guest: false,
    };
    fixture.test.answer(
        listing,
        Ok(AccessListing {
            grants: vec![
                AccessGrant {
                    author: true,
                    granted: None,
                    effective: AccessLevel::Manage,
                    ..grant(me, "Me", AccessLevel::Manage)
                },
                grant(colleague, "Ada", AccessLevel::Edit),
                AccessGrant {
                    waiting: true,
                    through_link: false,
                    guest: false,
                    ..grant(stranger, "Grace", AccessLevel::View)
                },
            ],
            company: false,
            set_inside: 0,
            hosted: false,
        }),
    );
    fixture.settle();
    assert!(
        !fixture.test.shown(&format!("share.owner.{stranger}")),
        "someone who is not yet a confirmed contact is not offered ownership"
    );

    fixture
        .test
        .click(format!("share.owner.{colleague}").as_str());
    fixture.settle();
    assert!(
        fixture.says("Make Ada the owner?"),
        "giving ownership away asks first"
    );
    assert!(
        fixture.test.take_ownership_transfers().is_empty(),
        "nothing is given before the owner confirms"
    );
    fixture
        .test
        .snapshot("the_owner_gives_a_block_away_from_the_share_dialog_once_confirmed");

    fixture.test.click("share.transfer");
    fixture.settle();
    assert_eq!(
        fixture.test.take_ownership_transfers(),
        vec![(block, colleague)]
    );
    assert!(
        !fixture.says("Make Ada the owner?"),
        "the question goes once it is answered"
    );
}
