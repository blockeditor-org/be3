use super::*;

use crate::EscrowWarning;
use be_keys::EscrowSecret;
use be_protocol::{EscrowKey, WrappedBy};

fn substitute(harness: &Harness, organization: Uuid, epoch: u64, previous: Uuid) -> EscrowKey {
    let secret = EscrowSecret::generate();
    let mut escrow = EscrowKey {
        id: Uuid::new_v4(),
        epoch,
        public: secret.public().0,
        signing: secret.signing(),
        signature: Vec::new(),
    };
    escrow.signature = secret.sign(&escrow.chained(organization, previous));
    harness
        .database()
        .execute(
            "INSERT INTO keys
                (organization_id, id, block_id, kind, epoch, created, public, signing, signature)
             VALUES (?1, ?2, ?1, 'escrow', ?3, 0, ?4, ?5, ?6)",
            rusqlite::params![
                organization.to_string(),
                escrow.id.to_string(),
                i64::try_from(epoch).unwrap(),
                escrow.public.to_vec(),
                escrow.signing.to_vec(),
                escrow.signature
            ],
        )
        .unwrap();
    escrow
}

#[tokio::test]
async fn an_escrow_key_the_server_puts_in_place_is_refused() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let pinned = admin
        .keys()
        .escrow()
        .expect("the creator pins its own escrow key");
    let member = harness.member(&admin, "member@example.com").await;
    assert_eq!(member.keys().escrow(), Some(pinned.clone()));
    assert_eq!(member.keys().escrow_warning(), None);

    let unpinned = harness
        .member_pinning(&admin, "unpinned@example.com", None)
        .await;
    assert_eq!(unpinned.keys().escrow(), None);
    assert_eq!(
        unpinned.keys().escrow_warning(),
        Some(EscrowWarning::Unverified)
    );
    let unescrowed = workspace(&unpinned, "Mine").await;
    let scope = unpinned.keys().scope_key(unescrowed).unwrap();
    assert!(
        !unpinned
            .keys()
            .wrapped_by(scope)
            .iter()
            .any(|by| matches!(by, WrappedBy::Key(_))),
        "a device wrapped a key under an escrow key it never verified"
    );

    let organization = admin.organization();
    let forged = substitute(&harness, organization, pinned.epoch + 1, pinned.id);
    for device in [harness.second(&member).await, harness.second(&admin).await] {
        assert_eq!(device.keys().escrow(), Some(pinned.clone()));
        assert_eq!(
            device.keys().escrow_warning(),
            Some(EscrowWarning::Substituted)
        );
        let made = workspace(&device, "After").await;
        let scope = device.keys().scope_key(made).unwrap();
        let wrapped = device.keys().wrapped_by(scope);
        assert!(
            !wrapped.contains(&WrappedBy::Key(forged.id)),
            "a new scope key was escrowed under the server's key"
        );
        assert!(wrapped.contains(&WrappedBy::Key(pinned.id)), "{wrapped:?}");
        device.tend().await.unwrap();
        assert_eq!(device.keys().pinned(), Some(pinned.clone()));
    }

    harness.stop().await;
}
