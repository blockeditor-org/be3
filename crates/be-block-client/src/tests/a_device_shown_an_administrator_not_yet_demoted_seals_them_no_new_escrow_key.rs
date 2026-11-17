use super::*;

use be_block_protocol::{OrganizationRole, Unsealed};

/// Demoting an administrator calls for a new escrow epoch, which names the head of the
/// organization's log it was made under. The server withholds the demotion from another of the
/// other administrator's devices and lists the demoted one as due the new epoch: that device,
/// shown them still administering, seals it to them only once it holds the log up to the
/// epoch's head, which demotes them, so never (signed access 3d).
#[tokio::test]
async fn a_device_shown_an_administrator_not_yet_demoted_seals_them_no_new_escrow_key() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
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
        work.unsealed.push(Unsealed {
            key: escrow.id,
            account: deputy.account(),
        });
        work
    };
    device.tend_with(work().await).await.unwrap();
    assert_eq!(
        sealed_to(&harness, escrow.id, deputy.account()),
        0,
        "the demoted administrator was sealed the epoch made after their demotion"
    );

    withheld.serve(&harness);
    device.tend_with(work().await).await.unwrap();
    assert_eq!(sealed_to(&harness, escrow.id, deputy.account()), 0);

    harness.stop().await;
}
