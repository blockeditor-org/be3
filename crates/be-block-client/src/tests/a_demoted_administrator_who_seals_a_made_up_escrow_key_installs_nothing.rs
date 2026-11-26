use super::*;

use be_block_keys::{CarriedKey, EscrowSecret, Heads, Party};
use be_block_protocol::{EscrowKey, OrganizationRole, WrappedBy};

/// A demoted administrator, with the server, makes up the next escrow epoch (its record
/// unsigned, naming no head the log holds) and seals its secret to an administrator who is
/// their contact. That device holds the secret but takes neither it nor the epoch: an escrow
/// key is taken from a contact only while the organization's log names them an administrator
/// at its head, and an epoch someone else made only through the signed chain, so nothing new
/// is escrowed under it.
#[tokio::test]
async fn a_demoted_administrator_who_seals_a_made_up_escrow_key_installs_nothing() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    befriend(&member, &deputy).await;
    for (account, role) in [
        (member.account(), OrganizationRole::Administrator),
        (deputy.account(), OrganizationRole::Administrator),
        (deputy.account(), OrganizationRole::Editor),
    ] {
        admin.set_role(account, role).await.unwrap();
    }
    let organization = admin.organization();
    let latest = admin
        .keys()
        .escrows()
        .into_iter()
        .max_by_key(|escrow| escrow.epoch)
        .unwrap();

    let secret = EscrowSecret::generate();
    let forged = EscrowKey {
        id: Uuid::new_v4(),
        epoch: latest.epoch + 1,
        public: secret.public().0,
        head: [0; 32],
        maker: deputy.account(),
        signature: vec![0; 64],
    };
    put_escrow(&harness, organization, &forged);
    let carried = CarriedKey {
        key: secret.to_bytes(),
        label: crate::tests::planted(forged.id, "escrow", forged.epoch),
        block: organization.into_bytes(),
        root: None,
        heads: Heads::default(),
    };
    let sealed = deputy
        .keys()
        .identity()
        .seal_for(
            deputy.account().into_bytes(),
            &Party {
                account: member.account().into_bytes(),
                public: member.keys().identity().public(),
            },
            forged.id.as_bytes(),
            &carried.encode(),
        )
        .unwrap();
    harness
        .database()
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                forged.id.to_string(),
                format!("account:{}", member.account()),
                deputy.account().to_string(),
                sealed
            ],
        )
        .unwrap();

    let device = harness.second(&member).await;
    assert!(
        !device.keys().trusted(forged.id),
        "an escrow key a demoted administrator sealed was taken"
    );
    assert_ne!(device.keys().escrow(), Some(forged.clone()));
    let made = workspace(&device, "After").await;
    let scope = device.keys().scope_key(made).unwrap();
    assert!(
        !device
            .keys()
            .wrapped_by(scope)
            .contains(&WrappedBy::Key(forged.id)),
        "a key was escrowed under the made-up epoch"
    );

    harness.stop().await;
}
