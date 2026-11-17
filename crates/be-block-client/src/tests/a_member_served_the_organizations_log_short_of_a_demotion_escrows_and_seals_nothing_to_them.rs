use super::*;

use be_block_protocol::{OrganizationRole, Unsealed, WrappedBy};

/// A member's device read the organization's log past an administrator's demotion, before any
/// new escrow epoch, then restarts, and the server serves the log short of the demotion. The
/// device takes nothing from that log: it judges by what it kept, so the escrow epoch from
/// before the demotion, which the demoted administrator holds, is wrapped under no more, and
/// nothing is sealed to them as an administrator (signed access 3d).
#[tokio::test]
async fn a_member_served_the_organizations_log_short_of_a_demotion_escrows_and_seals_nothing_to_them()
 {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    befriend(&member, &deputy).await;
    let old = member.keys().escrow().unwrap();
    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
    admin
        .set_role(deputy.account(), OrganizationRole::Editor)
        .await
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let before = harness.device(&member, &refs, &store).await;
    assert!(!before.keys().is_administrator(deputy.account()));
    assert_eq!(
        before.keys().escrow(),
        None,
        "an epoch from before the demotion is wrapped under"
    );
    drop(before);

    let organization = admin.organization();
    let withheld = withhold_newest(&harness, organization, None);
    // The server's rows as the short log has them, so it would take a seal to the deputy.
    harness
        .database()
        .execute(
            "UPDATE memberships SET role = 'administrator' WHERE account_id = ?1",
            [deputy.account().to_string()],
        )
        .unwrap();
    let restarted = harness.device(&member, &refs, &store).await;
    assert!(matches!(
        restarted.checked_organization().await,
        Err(ClientError::Withheld(_))
    ));
    let made = workspace(&restarted, "After").await;
    let scope = restarted.keys().scope_key(made).unwrap();
    assert!(
        !restarted
            .keys()
            .wrapped_by(scope)
            .contains(&WrappedBy::Key(old.id)),
        "a new block was escrowed under the epoch the demoted administrator holds"
    );
    let mut work = restarted.key_work().await.unwrap();
    work.rotations.clear();
    work.escrow_rotation = false;
    work.unsealed.retain(|unsealed| unsealed.key != scope);
    work.unsealed.push(Unsealed {
        key: scope,
        account: deputy.account(),
    });
    // Key work goes on with what it can, and reports the withheld log.
    let tended = restarted.tend_with(work).await;
    assert!(
        matches!(tended, Ok(()) | Err(ClientError::Withheld(_))),
        "{tended:?}"
    );
    assert_eq!(
        sealed_to(&harness, scope, deputy.account()),
        0,
        "the demoted administrator was sealed a key by a log short of their demotion"
    );
    withheld.serve(&harness);

    harness.stop().await;
}
