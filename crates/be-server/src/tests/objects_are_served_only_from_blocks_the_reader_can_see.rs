use super::*;

async fn read(client: &mut TestClient, hash: Hash) -> Option<Vec<u8>> {
    let response = client.download(hash).await;
    let Ok(bytes) = response else {
        panic!("reading an object failed: {response:?}");
    };
    bytes
}

async fn join(owner: &mut TestClient, guest: &mut TestClient, organization: Uuid) {
    let response = owner
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::CreateLink { link: Uuid::from_u128(1), card: Vec::new(), code: Vec::new(), kind: be_block_protocol::LinkKind::Invitation(be_block_protocol::Invitation {
                    organization,
                    escrow: Vec::new(),
                }) })
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let _ = guest
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::AcceptLink { link: Uuid::from_u128(1), card: Vec::new(), contact: Vec::new(), key: Vec::new() })
        })
        .await;
    guest.open(organization).await;
}

#[tokio::test]
async fn objects_are_served_only_from_blocks_the_reader_can_see() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    let organization = owner.organization("private").await;
    let author = Author::new(account);
    let block = owner.create_block(BlockParent::Root).await;
    let (commit, chunks) = author.compose(b"only for the owner\n", 1_000, None);
    let response = owner
        .publish(&author, block, commit, chunks.clone(), None, 1_000)
        .await;
    assert!(
        matches!(
            response,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{response:?}"
    );
    let (unpublished, loose) = author.compose(b"never published\n", 2_000, Some(commit));
    owner
        .upload(&author.store, &author.objects_of(unpublished, &loose))
        .await;

    assert!(read(&mut owner, commit.hash()).await.is_some());
    assert!(read(&mut owner, chunks[0]).await.is_some());
    assert!(read(&mut owner, unpublished.hash()).await.is_none());

    let mut outsider = harness.member("outsider@example.com").await;
    outsider.organization("elsewhere").await;
    assert!(read(&mut outsider, commit.hash()).await.is_none());
    assert!(read(&mut outsider, chunks[0]).await.is_none());

    let mut guest = harness.member("guest@example.com").await;
    join(&mut owner, &mut guest, organization).await;
    assert!(read(&mut guest, commit.hash()).await.is_none());

    let token = guest.token.clone();
    let response = guest
        .call(|peer| be_block_protocol::client::server::authenticate(peer, Some(token)))
        .await;
    let Ok(Authenticated {
        account: guest_account,
        ..
    }) = response
    else {
        panic!("the guest could not re-authenticate: {response:?}");
    };
    guest.open(organization).await;
    let response = owner
        .grant(block, guest_account, be_block_graph::Access::View)
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert!(read(&mut guest, commit.hash()).await.is_some());

    harness.stop().await;
}
