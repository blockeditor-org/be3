use super::*;

use be_block_protocol::{OrganizationRole, Unsealed};

/// Demoting an administrator calls for a new escrow epoch, which names the head of the
/// organization's log it was made under. The server withholds the demotion from another of the
/// other administrator's devices and lists the demoted one as due the new epoch: that device,
/// shown them still administering, seals it to no one until it holds the log up to the epoch's
/// head, which demotes them; then to the other administrator alone (signed access 3d).
#[tokio::test]
async fn a_device_shown_an_administrator_not_yet_demoted_seals_them_no_new_escrow_key() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let other = harness.member(&admin, "other@example.com").await;
    for administrator in [&deputy, &other] {
        admin
            .set_role(administrator.account(), OrganizationRole::Administrator)
            .await
            .unwrap();
    }
    admin
        .set_role(deputy.account(), OrganizationRole::Editor)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    let escrow = admin.keys().escrow().unwrap();
    let withheld = withhold_newest(&harness, admin.organization(), None);
    assert_eq!(
        escrow.head,
        withheld.hash(),
        "the new escrow epoch names a head other than the demotion's"
    );
    assert_eq!(admin.keys().heads(escrow.id).organization, Some(escrow.head));
    let database = harness.database();
    // As though no device had sealed it to the other administrator yet, and with the server's
    // rows as the withheld log has them, so the server would take a seal to the deputy and only
    // the device holds it back.
    database
        .execute(
            "DELETE FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
            [escrow.id.to_string(), format!("account:{}", other.account())],
        )
        .unwrap();
    database
        .execute(
            "UPDATE memberships SET role = 'administrator' WHERE account_id = ?1",
            [deputy.account().to_string()],
        )
        .unwrap();

    let device = harness.second(&admin).await;
    assert_eq!(device.keys().escrow(), Some(escrow.clone()));
    assert!(
        device.keys().is_administrator(deputy.account()),
        "the server withheld no demotion"
    );
    let work = || async {
        let mut work = device.key_work().await.unwrap();
        work.rotations.clear();
        work.escrow_rotation = false;
        work.unsealed.retain(|unsealed| unsealed.key != escrow.id);
        for account in [deputy.account(), other.account()] {
            work.unsealed.push(Unsealed {
                key: escrow.id,
                account,
            });
        }
        work
    };
    device.tend_with(work().await).await.unwrap();
    assert_eq!(
        sealed_to(&harness, escrow.id, deputy.account()),
        0,
        "the demoted administrator was sealed the epoch made after their demotion"
    );
    assert_eq!(
        sealed_to(&harness, escrow.id, other.account()),
        0,
        "the epoch was sealed by a log short of its head"
    );

    withheld.serve(&harness);
    device.tend_with(work().await).await.unwrap();
    assert_eq!(sealed_to(&harness, escrow.id, other.account()), 1);
    assert_eq!(sealed_to(&harness, escrow.id, deputy.account()), 0);

    harness.stop().await;
}
