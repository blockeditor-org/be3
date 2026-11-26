use super::*;

/// A member's workspace reaches an administrator only through its escrow wrap: the key opens
/// once `settle_escrowed` finds the member may edit it (`KeyBook::take_escrowed`), with no word
/// from the server. That is keys arriving, as any key that opens here is
/// (`KeyBook::on_opened`): counted in `arrivals` and told as `Event::KeysChanged`, so a host
/// reads again what the key unlocks.
#[tokio::test]
async fn a_key_opened_from_escrow_arrives_and_unlocks_its_block() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let team = workspace(&member, "Team").await;

    let listed = vec![admin.summary(team).await.unwrap()];
    let [summary] = listed.as_slice() else {
        panic!("the administrator lists {listed:?}");
    };
    let key = summary.key.expect("the workspace has a key");
    assert!(
        !admin.keys().has(key),
        "the administrator held the workspace's key before it asked for it"
    );

    let mut events = admin.connection().subscribe();
    let arrivals = admin.arrivals();
    admin.learn(&listed).await.unwrap();
    assert!(admin.keys().has(key), "the escrow wrap did not open");
    assert!(
        !admin.keys().trusted(key),
        "the key opened some other way than its escrow wrap"
    );
    assert!(
        admin.arrivals() > arrivals,
        "the key opened from escrow was not counted as arriving"
    );
    assert!(
        told_keys_changed(&mut events),
        "the key opened from escrow was never told"
    );
    assert_eq!(
        admin.metadata(summary).unwrap().name().as_deref(),
        Some("Team"),
        "the key opened from escrow did not unlock the workspace"
    );

    harness.stop().await;
}
