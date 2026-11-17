use super::*;

use be_block_graph::{Access, PREFIXES_TYPE};
use be_block_protocol::Unsealed;

/// A kind its author opened to anyone inside, closed, and then gave a new epoch (it stays a
/// scope by a grant of its own): the server
/// withholds the closing from another of the author's devices and lists a contact viewing
/// inside as due the epoch. That device, shown the kind still open, offers the epoch to no
/// contact until it holds the log up to the epoch's head, which closes it (signed access 3d).
#[tokio::test]
async fn a_device_shown_a_kind_still_open_seals_the_new_epoch_to_no_contact() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let team = workspace(&admin, "Team").await;
    admin
        .grant(team, member.account(), Access::View)
        .await
        .unwrap();
    let kind = admin
        .create_block(
            Uuid::now_v7(),
            PREFIXES_TYPE,
            BlockParent::Block(team),
            &be_block_content::BlockMetadata::named("Prefixes"),
            false,
        )
        .await
        .unwrap()
        .id;
    // A grant of its own keeps it a scope, with epochs of its own, once it is closed.
    admin
        .grant(kind, reader.account(), Access::View)
        .await
        .unwrap();
    admin.open_inside(kind, Access::View).await.unwrap();
    admin.open_inside(kind, Access::None).await.unwrap();
    let mut work = admin.key_work().await.unwrap();
    work.rotations = vec![kind];
    admin.tend_with(work).await.unwrap();
    let epoch = admin.keys().scope_key(kind).unwrap();
    let withheld = withhold_newest(&harness, admin.organization(), Some(kind));
    assert_eq!(admin.keys().heads(epoch).block, Some(withheld.hash()));

    let device = harness.second(&admin).await;
    let (_, shown) = device.checked_access(kind).await.unwrap();
    assert_eq!(
        shown.open_inside,
        be_block_keys::Level::View,
        "the server withheld no closing"
    );
    let work = || async {
        let mut work = device.key_work().await.unwrap();
        work.rotations.clear();
        work.unsealed.retain(|unsealed| unsealed.key != epoch);
        work.unsealed.push(Unsealed {
            key: epoch,
            account: member.account(),
        });
        work
    };
    device.tend_with(work().await).await.unwrap();
    assert_eq!(
        sealed_to(&harness, epoch, member.account()),
        0,
        "an epoch made after the closing was offered by a log short of its head"
    );

    withheld.serve(&harness);
    device.tend_with(work().await).await.unwrap();
    assert_eq!(sealed_to(&harness, epoch, member.account()), 0);

    harness.stop().await;
}
