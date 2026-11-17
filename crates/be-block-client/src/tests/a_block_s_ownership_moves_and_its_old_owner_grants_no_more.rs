use super::*;

use be_block_keys::Owner;

/// Ownership of a block moves by an owner record its owner signs (guides/blocks/keys.md,
/// "Owner records"): the new owner manages it and reads it, and the old owner keeps Edit but grants
/// nothing more, whether signed at the head now or at one from before the transfer.
#[tokio::test]
async fn a_block_s_ownership_moves_and_its_old_owner_grants_no_more() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let alice = harness.member(&admin, "alice@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    befriend(&alice, &bob).await;
    befriend(&alice, &carol).await;
    befriend(&bob, &carol).await;
    let plans = workspace(&alice, "Plans").await;
    let note = alice
        .create::<TextContent>(BlockParent::Block(plans))
        .await
        .unwrap();
    alice
        .save(note, &TextBlock::of("the plan"), None)
        .await
        .unwrap();
    let carols = alice.verified_identity(carol.account()).unwrap();
    let grant_to_carol = Record::Grant(Grant {
        grantee: carol.account().into_bytes(),
        identity: carols,
        level: Level::View,
        authority: Authority::Log,
    });
    // Signed while Alice owned the block, and sent only after she gave it away, as a grant
    // made on a device that had not heard of the transfer would be.
    let (_, before) = alice.checked_access(plans).await.unwrap();
    let stale = before
        .next(
            alice.keys().identity(),
            alice.account().into_bytes(),
            plans.into_bytes(),
            grant_to_carol.clone(),
        )
        .unwrap();

    alice.transfer(plans, bob.account()).await.unwrap();

    bob.refresh_keys(None).await.unwrap();
    assert_eq!(text(&bob, note).await, "the plan", "the new owner reads it");
    let (_, checked) = bob.checked_access(plans).await.unwrap();
    assert_eq!(checked.owner, Some(bob.account().into_bytes()));
    assert_eq!(
        checked
            .grants
            .get(&alice.account().into_bytes())
            .map(|(level, _)| *level),
        Some(Level::Edit),
        "the old owner keeps Edit"
    );
    assert_eq!(
        bob.summary(plans).await.unwrap().access,
        Access::Manage,
        "the new owner manages it"
    );

    let refused = alice
        .share(plans, carol.account(), Access::View, &carols)
        .await;
    assert!(
        matches!(
            &refused,
            Err(ClientError::Refused(ErrorCode::PermissionDenied, _))
        ),
        "the old owner grants no more: {refused:?}"
    );
    let (_, now) = alice.checked_access(plans).await.unwrap();
    let forced = Entry::sign(
        alice.keys().identity(),
        alice.account().into_bytes(),
        plans.into_bytes(),
        Some(now.head),
        false,
        grant_to_carol,
    );
    for (what, entry) in [("at the head now", forced), ("from before", stale)] {
        let sent = crate::ask(be_block_protocol::write::<(), _>(
            &alice.connection(),
            Some(alice.organization()),
            be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Grant {
                block: plans,
                account: carol.account(),
                access: Access::View,
                signed: Some(entry.encode()),
            }),
        ))
        .await;
        assert!(
            matches!(
                &sent,
                Err(ClientError::Refused(ErrorCode::PermissionDenied, _))
            ),
            "a grant the old owner signed {what} is taken: {sent:?}"
        );
    }
    let given_back = Entry::sign(
        alice.keys().identity(),
        alice.account().into_bytes(),
        plans.into_bytes(),
        Some(now.head),
        true,
        Record::Owner(Owner {
            account: carol.account().into_bytes(),
            identity: carols,
        }),
    );
    let sent = crate::ask(be_block_protocol::write::<(), _>(
        &alice.connection(),
        Some(alice.organization()),
        be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Grant {
            block: plans,
            account: carol.account(),
            access: Access::Manage,
            signed: Some(given_back.encode()),
        }),
    ))
    .await;
    assert!(
        matches!(
            &sent,
            Err(ClientError::Refused(ErrorCode::PermissionDenied, _))
        ),
        "the old owner gives the block away again: {sent:?}"
    );
    assert!(matches!(
        alice.transfer(plans, carol.account()).await,
        Err(ClientError::Refused(ErrorCode::PermissionDenied, _))
    ));

    let bobs = bob.verified_identity(carol.account()).unwrap();
    bob.share(plans, carol.account(), Access::View, &bobs)
        .await
        .unwrap();
    carol.refresh_keys(None).await.unwrap();
    assert_eq!(text(&carol, note).await, "the plan", "the new owner grants");

    // A server that appends a grant the old owner signs at the head anyway, chained at its
    // right place, has every device that checks the log refuse it for the signer's authority.
    let (_, head) = bob.checked_access(plans).await.unwrap();
    let appended = Entry::sign(
        alice.keys().identity(),
        alice.account().into_bytes(),
        plans.into_bytes(),
        Some(head.head),
        false,
        Record::Grant(Grant {
            grantee: admin.account().into_bytes(),
            identity: admin.keys().identity().identity(),
            level: Level::Manage,
            authority: Authority::Log,
        }),
    );
    harness
        .database()
        .execute(
            "INSERT INTO access_log (organization_id, block_id, position, entry)
             SELECT organization_id, block_id, MAX(position) + 1, ?3 FROM access_log
             WHERE organization_id = ?1 AND block_id = ?2",
            rusqlite::params![
                alice.organization().to_string(),
                plans.to_string(),
                appended.encode()
            ],
        )
        .unwrap();
    assert!(
        matches!(bob.checked_access(plans).await, Err(ClientError::Forged(_))),
        "a log with the old owner's grant after the transfer checks"
    );

    harness.stop().await;
}
