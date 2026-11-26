use super::*;

use be_block_keys::Admits;
use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

/// An account in the personal organization, where every account is a member and there is no
/// organization log: a key reaches someone who is no contact only by the block's own log.
pub(super) async fn personal(harness: &Harness, email: &str) -> Peer<MemoryStore> {
    let token = harness.server.register(email, email).await;
    let (connection, _) = Connection::connect(
        &harness.url,
        &Clock::real(),
        &Credentials::Token(token.clone()),
    )
    .await
    .unwrap();
    let organizations = be_block_protocol::read::<Vec<be_block_protocol::Organization>, _>(
        &connection,
        None,
        be_block_protocol::Query::Organizations,
    )
    .await
    .unwrap()
    .unwrap();
    let personal = organizations
        .iter()
        .find(|organization| organization.kind == be_block_protocol::OrganizationKind::Personal)
        .expect("every account is in the personal organization")
        .id;
    Peer::connect(
        PeerConfig::new(&harness.url, identity(email), Credentials::Token(token))
            .organization(Some(personal))
            .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap()
}

/// A View link lets an account no grant names read the block, signing its own grant with the
/// key the code derives. The server holds the link's record, what it hands its opener and every
/// message the opening sent, and with them opens nothing and grants no one.
#[tokio::test]
async fn a_share_link_grants_view_to_an_account_no_one_granted() {
    let harness = Harness::start().await;
    let maker = personal(&harness, "maker@example.com").await;
    let opener = personal(&harness, "opener@example.com").await;
    let stranger = personal(&harness, "stranger@example.com").await;
    let team = workspace(&maker, "Plans").await;
    let note = maker
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    maker
        .save(note, &TextBlock::of("shared by link"), None)
        .await
        .unwrap();
    let code = maker
        .make_share_link(team, Access::View, Admits::Members)
        .await
        .unwrap();
    assert!(
        opener.summary(note).await.is_err(),
        "no one granted the opener anything"
    );

    // What the server holds: the link's row, and what it hands its opener sealed with the code.
    let link = crate::share_links::link_id(&code);
    let sealed: Vec<u8> = harness
        .database()
        .query_row(
            "SELECT sealed FROM share_links WHERE id = ?1",
            [link.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    let scope = maker.keys().scope_key(team).unwrap();
    assert!(
        be_block_keys::InviteCode::generate()
            .open_share(team.into_bytes(), &sealed)
            .is_err(),
        "what a link hands opens only with its code"
    );
    let plain = be_block_keys::CarriedKey::decode(&maker.keys().carried_plain(scope).unwrap())
        .unwrap()
        .key;
    assert!(
        !sealed.windows(plain.len()).any(|window| window == plain),
        "the stored link holds the scope key's bytes only sealed"
    );
    // Whoever holds only what the server holds (the link's id and its record) signs no grant:
    // the record's key derives from the code.
    let read = crate::share_links::read_share_link(&stranger.connection(), &code)
        .await
        .unwrap();
    let forged = be_block_keys::Entry::opened(
        &be_block_keys::InviteCode::generate(),
        team.into_bytes(),
        read.head,
        stranger.account().into_bytes(),
        stranger.keys().identity(),
        be_block_keys::Level::View,
    );
    let refused = be_block_protocol::write::<(), _>(
        &stranger.connection(),
        None,
        be_block_protocol::Op::OpenShareLink {
            link,
            signed: forged.encode(),
            key: Some(scope),
            sealed: Vec::new(),
        },
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(refused.code, ErrorCode::InvalidRequest, "{refused:?}");
    assert!(stranger.summary(note).await.is_err());

    let (opened, _) = crate::share_links::open_share_link(
        &opener.connection(),
        opener.keys().identity(),
        opener.account(),
        &code,
        None,
    )
    .await
    .unwrap();
    assert_eq!((opened.block, opened.access), (team, Access::View));
    assert_eq!(text(&opener, note).await, "shared by link");
    assert_eq!(
        text(&harness.second(&opener).await, note).await,
        "shared by link",
        "another device of the opener reads it too"
    );
    let listed = maker.list_access(team).await.unwrap();
    assert!(
        listed
            .entries
            .iter()
            .any(|entry| entry.account == opener.account() && entry.through_link),
        "the access list says the opener joined through the link"
    );
    let saved = opener
        .save(note, &TextBlock::of("changed by a viewer"), None)
        .await;
    assert!(saved.is_err(), "a View link grants no editing");

    harness.stop().await;
}
