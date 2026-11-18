use super::*;

use be_block_graph::Access;

/// A contact who accepted this account's link shares a block, sealing its key, before this
/// device finished the link: the key it fetched is not one it opens, so a submission's
/// `sealing_key` is refused. Once the device finishes its links and lists its contacts, the key
/// opens there, with no word from the server: that is keys arriving, told as
/// `Event::KeysChanged` and counted in `arrivals`, so a host reads again what the key unlocks
/// and submits again what it refused.
#[tokio::test]
async fn a_key_sealed_by_a_contact_verified_later_arrives_as_it_is_verified() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.member(&rosa, "guest@example.com").await;
    let carol = harness.member(&rosa, "carol@example.com").await;

    // The guest's link, which Carol accepts: the guest verifies her only once it finishes it.
    let code = crate::contacts::make_link(
        &guest.connection(),
        guest.keys().identity(),
        &crate::contacts::card(
            guest.account(),
            guest.keys().identity(),
            "guest@example.com",
            "guest@example.com",
        ),
        None,
    )
    .await
    .unwrap();
    crate::contacts::accept_link(
        &carol.connection(),
        carol.keys().identity(),
        &crate::contacts::card(
            carol.account(),
            carol.keys().identity(),
            "Carol",
            "carol@example.com",
        ),
        &code,
        crate::contacts::Expected::Contact,
        &Default::default(),
    )
    .await
    .unwrap();
    carol.refresh_contacts().await.unwrap();
    let identity = carol
        .verified_identity(guest.account())
        .expect("Carol verified the guest through the link");
    let block = workspace(&carol, "Plans").await;
    carol
        .share(block, guest.account(), Access::Edit, &identity)
        .await
        .unwrap();

    let listed = vec![guest.summary(block).await.unwrap()];
    guest.learn(&listed).await.unwrap();
    assert!(
        matches!(guest.sealing_key(block).await, Err(ClientError::Locked(_))),
        "the guest sealed with a key from a contact it had not verified"
    );

    let mut events = guest.connection().subscribe();
    let arrivals = guest.arrivals();
    guest.finish_links().await.unwrap();
    guest.refresh_contacts().await.unwrap();
    assert!(
        guest.arrivals() > arrivals,
        "the key opened by verifying its sealer was not counted as arriving"
    );
    // Told before `refresh_contacts` returned.
    assert!(
        told_keys_changed(&mut events),
        "the key opened by verifying its sealer was never told"
    );
    guest
        .sealing_key(block)
        .await
        .expect("the guest seals with the key once it verified its sealer");

    harness.stop().await;
}
