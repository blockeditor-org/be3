use super::*;

use crate::EscrowWarning;
use be_block_keys::EscrowSecret;
use be_block_protocol::{EscrowKey, OrganizationRole};

/// An escrow key `maker` signs chained to `previous`, under `head` of the organization's log,
/// put in the chain by the server.
fn made_by(
    harness: &Harness,
    maker: &Peer<MemoryStore>,
    epoch: u64,
    previous: Uuid,
    head: be_block_keys::EntryHash,
) -> EscrowKey {
    let organization = maker.organization();
    let secret = EscrowSecret::generate();
    let mut escrow = EscrowKey {
        id: Uuid::new_v4(),
        epoch,
        public: secret.public().0,
        signing: secret.signing(),
        head,
        maker: maker.account(),
        signature: Vec::new(),
    };
    escrow.signature = maker
        .keys()
        .identity()
        .sign_escrow(&escrow.chained(organization, previous));
    put_escrow(harness, organization, &escrow);
    escrow
}

/// A device takes a new escrow key only from an administrator the organization's log names as
/// of its head. A demoted administrator still holds the identity key that signed their epochs,
/// and the server will put whatever it likes in the chain; together they make an epoch the
/// device refuses, so members never escrow to the demoted administrator (crypto audit 10); and
/// a device that holds the demotion wraps under no epoch from before it (signed access 3d).
#[tokio::test]
async fn an_escrow_key_a_demoted_administrator_makes_is_refused() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let pinned = member
        .keys()
        .escrow()
        .expect("the member pins the escrow key");
    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();

    let head = admin.checked_organization().await.unwrap().unwrap().head;
    let made = made_by(&harness, &deputy, pinned.epoch + 1, pinned.id, head);
    let device = harness.second(&member).await;
    assert_eq!(
        device.keys().escrow(),
        Some(made.clone()),
        "an administrator's escrow key is taken"
    );
    assert_eq!(device.keys().escrow_warning(), None);

    admin
        .set_role(deputy.account(), OrganizationRole::Editor)
        .await
        .unwrap();
    // Under the head that demotes them, as a device that holds it takes no epoch from before.
    let head = admin.checked_organization().await.unwrap().unwrap().head;
    let forged = made_by(&harness, &deputy, made.epoch + 1, made.id, head);
    let device = harness.second(&member).await;
    // Nor, holding the demotion, does it wrap under the epochs from before it, which the
    // demoted administrator may hold: keys wait unescrowed for the next epoch.
    assert_eq!(
        device.keys().escrow(),
        None,
        "a demoted administrator's escrow key, or one made before their demotion, was taken"
    );
    assert_eq!(
        device.keys().escrow_warning(),
        Some(EscrowWarning::Unverified)
    );
    let mine = workspace(&device, "After").await;
    let scope = device.keys().scope_key(mine).unwrap();
    let wrapped = device.keys().wrapped_by(scope);
    assert!(
        !wrapped.contains(&be_block_protocol::WrappedBy::Key(forged.id))
            && !wrapped.contains(&be_block_protocol::WrappedBy::Key(made.id)),
        "a key was escrowed to the demoted administrator"
    );

    harness.stop().await;
}
