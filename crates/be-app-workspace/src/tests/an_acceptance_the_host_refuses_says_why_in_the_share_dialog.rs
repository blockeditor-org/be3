use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, Failure, ShellDialog};
use be_sdk_test::HostCall;

use super::*;

/// What the host answers an acceptance of ownership it could not make shows in the share dialog,
/// and the offer stands: here that the account holds as many blocks as it may, in the host's
/// words (be-host-stack's `TOO_MANY_TO_ACCEPT`).
#[test]
fn an_acceptance_the_host_refuses_says_why_in_the_share_dialog() {
    let grace = Uuid::from_u128(0x3a_00c1);
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
    let offered = AccessListing {
        grants: vec![grant(grace, "Grace", true), grant(me, "Me", false)],
        company: false,
        set_inside: 0,
        levels: Vec::new(),
        host: None,
        offered: Some(me.into_bytes()),
    };
    fixture.test.answer(listing, Ok(offered.clone()));
    fixture.settle();
    fixture.test.click("share.accept");
    fixture.settle();
    let accepted = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::AcceptOwnership { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("accepting asks the host");
    let said = "You already have as many blocks as one account can hold here, so you can't \
                take this one yet. The offer stands until you make room.";
    fixture
        .test
        .answer(accepted, Err(Failure::Failed(said.to_owned())));
    fixture.settle();
    // Listed again, the offer still stands.
    if let Some(listing) = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ListAccess { reply, .. } => Some(reply),
            _ => None,
        })
    {
        fixture.test.answer(listing, Ok(offered));
        fixture.settle();
    }
    assert!(fixture.says(said), "the dialog says why it was refused");
    assert!(
        fixture.test.shown("share.accept"),
        "the offer stands, to accept again"
    );
    fixture
        .test
        .snapshot("an_acceptance_the_host_refuses_says_why_in_the_share_dialog");
}
