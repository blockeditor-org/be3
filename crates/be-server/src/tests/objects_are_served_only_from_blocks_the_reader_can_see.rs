use super::*;

async fn read(client: &mut TestClient, hash: Hash) -> Option<Vec<u8>> {
    let response = client
        .call(|peer| client::objects::get_object(peer, hash))
        .await;
    let Ok(bytes) = response else {
        panic!("reading an object failed: {response:?}");
    };
    bytes
}

async fn join(owner: &mut TestClient, guest: &mut TestClient, organization: Uuid, email: &str) {
    let response = owner
        .call(|peer| {
            client::organizations::invite(
                peer,
                Uuid::from_u128(1),
                organization,
                email.into(),
                OrganizationRole::Editor,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let _ = guest
        .call(|peer| {
            client::organizations::accept_invitation(
                peer,
                Uuid::from_u128(1),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
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
        matches!(response, Ok(be_protocol::PublishOutcome::Published(_))),
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
    join(&mut owner, &mut guest, organization, "guest@example.com").await;
    assert!(read(&mut guest, commit.hash()).await.is_none());

    let token = guest.token.clone();
    let response = guest
        .call(|peer| client::account::authenticate(peer, Some(token)))
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
        .call(|peer| client::blocks::set_access(peer, block, guest_account, be_graph::Access::View))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert!(read(&mut guest, commit.hash()).await.is_some());

    harness.stop().await;
}
