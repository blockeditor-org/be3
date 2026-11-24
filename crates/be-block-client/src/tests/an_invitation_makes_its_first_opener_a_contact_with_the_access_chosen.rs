use be_block::{LinkSharing, OrganizationPolicies};
use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

fn card(peer: &Peer<MemoryStore>, name: &str) -> be_block_keys::ContactCard {
    crate::contacts::card(peer.account(), peer.keys().identity(), name, name)
}

async fn accept(peer: &Peer<MemoryStore>, name: &str, code: &be_block_keys::InviteCode) -> Result<(), ClientError> {
    crate::contacts::accept_link(
        &peer.connection(),
        peer.keys().identity(),
        &card(peer, name),
        code,
        crate::contacts::Expected::Contact,
        &Default::default(),
    )
    .await
    .map(|_| ())
}

async fn open(peer: &Peer<MemoryStore>, code: &be_block_keys::InviteCode) -> Result<be_block_protocol::ShareLink, ClientError> {
    crate::share_links::open_share_link(
        &peer.connection(),
        peer.keys().identity(),
        peer.account(),
        code,
        None,
    )
    .await
    .map(|(link, _)| link)
}

/// An invitation (plans/invitations_without_pending_grants.md) is a contact link and a share
/// link that admits one, with one code. A new person who opens it becomes a contact through the
/// contact link and gains the access chosen through the share link, which they sign
/// themselves: the server applies no grant. A second person who opens it becomes nobody's
/// contact and gains nothing.
#[tokio::test]
async fn an_invitation_makes_its_first_opener_a_contact_with_the_access_chosen() {
    let harness = Harness::start().await;
    let maker = harness.owner("maker@example.com").await;
    let invited = harness.owner("invited@example.com").await;
    let second = harness.owner("second@example.com").await;
    set_policies(
        &maker,
        &[OrganizationPolicies::set_link_sharing(Some(
            LinkSharing::Anyone,
        ))],
    )
    .await;
    let team = workspace(&maker, "Team").await;
    let note = maker
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    maker
        .save(note, &TextBlock::of("for the invited"), None)
        .await
        .unwrap();

    let code = maker
        .make_invitation(&card(&maker, "maker@example.com"), team, Access::Edit)
        .await
        .unwrap();
    let grant: String = harness
        .database()
        .query_row(
            "SELECT grant_block FROM links WHERE id = ?1",
            [crate::share_links::link_id(&code).to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(grant, "", "the contact link carries no grant for the server to apply");

    accept(&invited, "invited@example.com", &code).await.unwrap();
    invited.refresh_contacts().await.unwrap();
    assert!(
        invited.verified_identity(maker.account()).is_some(),
        "the invited person is the maker's contact"
    );
    let opened = open(&invited, &code).await.unwrap();
    assert_eq!((opened.block, opened.access), (team, Access::Edit));
    let guest = Peer::connect(
        PeerConfig::new(
            &harness.url,
            invited.keys().identity().clone(),
            Credentials::Token(invited.token()),
        )
        .organization(Some(maker.organization()))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    assert_eq!(text(&guest, note).await, "for the invited", "with the access chosen");

    let refused = accept(&second, "second@example.com", &code).await;
    assert!(refused.is_err(), "the contact half took one person: {refused:?}");
    second.refresh_contacts().await.unwrap();
    assert!(
        second.verified_identity(maker.account()).is_none(),
        "a second person is nobody's contact"
    );
    let refused = open(&second, &code).await.unwrap_err();
    assert!(
        matches!(refused, ClientError::Refused(ErrorCode::PermissionDenied, _)),
        "and gains no access: {refused}"
    );

    harness.stop().await;
}
