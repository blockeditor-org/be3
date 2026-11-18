use super::*;

use be_block_protocol::KeyKind;
use be_type_text::{TextBlock, TextContent};

/// What a key is wrapped as names its own row (`be_block_keys::Label`), so a server that serves
/// a wrap of an old data key as the newest epoch of the block (decisions/a_wrapped_key_names_its_own_row.md) has
/// the device hold nothing under the new row, and the block is never written under the old key
/// as if it were a new one.
#[tokio::test]
async fn an_old_key_the_server_relabels_as_a_new_epoch_opens_nothing() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let organization = owner.organization();
    let note = owner
        .create::<TextContent>(BlockParent::Block(workspace(&owner, "Mine").await))
        .await
        .unwrap();
    owner
        .save(note, &TextBlock::of("first"), None)
        .await
        .unwrap();
    let old = owner.keys().keys_of(note, KeyKind::Data)[0];

    // The server copies the old key's wrap under the scope to a row of its making, epoch 99.
    let relabelled = Uuid::new_v4();
    {
        let database = harness.database();
        database
            .execute(
                "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
                 VALUES (?1, ?2, ?3, 'data', 99, 0)",
                rusqlite::params![
                    organization.to_string(),
                    relabelled.to_string(),
                    note.to_string()
                ],
            )
            .unwrap();
        let copied = database
            .execute(
                "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
                 SELECT organization_id, ?2, wrapped_by, sender, sealed FROM wrapped_keys
                 WHERE organization_id = ?1 AND key_id = ?3 AND wrapped_by NOT LIKE 'account:%'",
                rusqlite::params![
                    organization.to_string(),
                    relabelled.to_string(),
                    old.id.to_string()
                ],
            )
            .unwrap();
        assert!(copied > 0, "the old key is wrapped under its scope");
    }

    let device = harness.second(&owner).await;
    device.refresh_keys(None).await.unwrap();
    assert!(
        device.keys().has(old.id),
        "the old key's own wrap still opens"
    );
    assert!(
        !device.keys().has(relabelled),
        "a wrap of an old key opened as a new epoch of it"
    );
    harness.stop().await;
}
