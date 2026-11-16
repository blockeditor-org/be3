use super::*;

use crate::EscrowWarning;
use be_block_keys::EscrowSecret;
use be_block_protocol::{EscrowKey, OrganizationRole};

/// An escrow key `maker` signs chained to `previous`, put in the chain by the server.
fn made_by(harness: &Harness, maker: &Peer<MemoryStore>, epoch: u64, previous: Uuid) -> EscrowKey {
    let organization = maker.organization();
    let secret = EscrowSecret::generate();
    let mut escrow = EscrowKey {
        id: Uuid::new_v4(),
        epoch,
        public: secret.public().0,
        signing: secret.signing(),
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
/// device refuses, so members never escrow to the demoted administrator (crypto audit 10).
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

    let made = made_by(&harness, &deputy, pinned.epoch + 1, pinned.id);
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
    let forged = made_by(&harness, &deputy, made.epoch + 1, made.id);
    let device = harness.second(&member).await;
    assert_eq!(
        device.keys().escrow(),
        Some(made),
        "a demoted administrator's escrow key was taken"
    );
    assert_eq!(
        device.keys().escrow_warning(),
        Some(EscrowWarning::Substituted)
    );
    let mine = workspace(&device, "After").await;
    let scope = device.keys().scope_key(mine).unwrap();
    assert!(
        !device
            .keys()
            .wrapped_by(scope)
            .contains(&be_block_protocol::WrappedBy::Key(forged.id)),
        "a key was escrowed to the demoted administrator"
    );

    harness.stop().await;
}
