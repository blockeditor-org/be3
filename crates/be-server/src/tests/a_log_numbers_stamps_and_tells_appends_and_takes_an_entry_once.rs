use super::*;
use be_block_graph::Access;
use be_block_keys::merkle;

/// A block's signed log numbers each entry next and stamps it with the server's clock, in
/// order, whoever appends; a follower is told each append with the proofs that tie it to the
/// head it held, and a page read anywhere verifies against the head with its proof alone. An
/// entry sent again under its id is answered where it went, not appended twice.
#[tokio::test]
async fn a_log_numbers_stamps_and_tells_appends_and_takes_an_entry_once() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.member("guest@example.com").await;
    let guest_id = guest.id();
    guest.personal().await;
    let link = owner.link().await;
    let _ = guest.accept(link, &[]).await;
    let chat = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(chat, guest_id, Access::Edit).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");

    guest.listen().await;
    let empty = guest.log_page(chat, 0, None, true).await.unwrap();
    assert_eq!(empty.head.size, 0);
    assert_eq!(*empty.head.root.as_bytes(), merkle::empty_root());

    let first = log_entry(b"hello");
    let before = be_block_commit::now_milliseconds();
    let logged = owner
        .append(chat, vec![first.clone(), log_entry(b"how are you")])
        .await
        .unwrap();
    assert_eq!(
        logged.iter().map(|logged| logged.seq).collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(logged[0].arrival >= before && logged[1].arrival >= logged[0].arrival);

    let told = guest.appended().await;
    let first_head = told.head;
    assert_eq!((told.block, told.previous, told.head.size), (chat, 0, 2));
    assert_eq!(told.entries.len(), 2);
    let mut data = Vec::new();
    data.extend_from_slice(&told.entries[0].seq.to_be_bytes());
    data.extend_from_slice(&told.entries[0].arrival.to_be_bytes());
    data.extend_from_slice(told.entries[0].author.as_bytes());
    data.extend_from_slice(told.entries[0].id.as_bytes());
    data.extend_from_slice(told.entries[0].payload_hash.as_bytes());
    data.extend_from_slice(&told.entries[0].signature);
    assert_eq!(
        told.entries[0].leaf(),
        merkle::leaf_hash(&data),
        "an entry's leaf is RFC 6962's, as the tree hashes leaves"
    );
    assert_eq!(told.entries[0].author, owner.id());
    assert_eq!(told.entries[0].arrival, logged[0].arrival);
    assert_eq!(
        told.entries[1].payload.as_deref(),
        Some(&b"how are you"[..])
    );
    assert!(verifies(
        &be_block_protocol::LogPage {
            head: told.head,
            entries: told.entries.clone(),
            proof: told.proof.clone(),
            consistency: told.consistency.clone(),
            known_root: None,
            redactions: 0,
            redacted: Vec::new(),
        },
        0
    ));

    let reply = guest.append(chat, vec![log_entry(b"fine")]).await.unwrap();
    assert_eq!(reply[0].seq, 2);
    let told = guest.appended().await;
    assert_eq!((told.previous, told.head.size), (2, 3));
    assert_eq!(told.entries[0].author, guest_id);
    let held = |hashes: &[Hash]| {
        hashes
            .iter()
            .map(|hash| *hash.as_bytes())
            .collect::<Vec<_>>()
    };
    let kept = *first_head.root.as_bytes();
    assert!(
        merkle::consistent(
            2,
            &kept,
            3,
            told.head.root.as_bytes(),
            &held(&told.consistency)
        ),
        "the append's head extends the one before it"
    );

    // Sent again, an entry is answered where it went, and nothing is appended or told.
    let again = owner.append(chat, vec![first.clone()]).await.unwrap();
    assert_eq!(again, logged[..1]);
    // Another entry under its id, or the same one from someone else, is refused.
    let mut other = first.clone();
    other.payload = b"something else".to_vec();
    let refused = owner.append(chat, vec![other]).await;
    assert!(
        matches!(&refused, Err(error) if error.code == ErrorCode::InvalidRequest),
        "{refused:?}"
    );
    let refused = guest.append(chat, vec![first]).await;
    assert!(
        matches!(&refused, Err(error) if error.code == ErrorCode::InvalidRequest),
        "{refused:?}"
    );
    let page = guest.log_page(chat, 1, Some(2), false).await.unwrap();
    assert_eq!(page.head.size, 3);
    assert_eq!(page.entries.len(), 2);
    assert!(verifies(&page, 1));
    assert!(merkle::consistent(
        2,
        &kept,
        3,
        page.head.root.as_bytes(),
        &held(&page.consistency)
    ));
    guest.round_trip().await;
    assert!(
        !guest
            .notifications
            .iter()
            .any(|notice| matches!(notice, Notice::Appended(_))),
        "an entry sent again was told again"
    );

    // An unfollowed log tells nothing more.
    let unfollowed = guest
        .call(|peer| peer.unfollow(Topic::BlockLog(chat)))
        .await;
    assert!(unfollowed.is_ok(), "{unfollowed:?}");
    owner.append(chat, vec![log_entry(b"bye")]).await.unwrap();
    guest.round_trip().await;
    guest.round_trip().await;
    assert!(
        !guest
            .notifications
            .iter()
            .any(|notice| matches!(notice, Notice::Appended(_))),
        "an unfollowed log was told"
    );
    harness.stop().await;
}
