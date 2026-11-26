use be_block_client::Event;
use be_block_content::LiveEdit;
use be_sdk_type_guest::wire::{Entry, Submitted};

use super::*;

/// An entry of the order sealed with a key this device does not hold yet (another device made
/// the block, and its key has not reached this one) waits, with what follows it, for a search
/// for that key, and is handed to the replica once the key arrives, not passed over as one it
/// heard already: the replica applies the order in its numbers, and one it never got would hold
/// back every entry after it, its own edits among them, for good.
#[test]
fn an_entry_sealed_with_a_key_not_yet_here_reaches_the_replica_once_it_arrives() {
    with_another_device(|peer, phone, shared, mut replicas| async move {
        let block = phone
            .create::<Sections>(BlockParent::Root)
            .await
            .expect("the other device makes a block");
        let key = phone.current_key(block).expect("the block has a key");
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let through = replicas.heard_through_of(block).unwrap_or(0);

        // The phone's edits, numbered next by the order, each sealed with the block's key.
        let entry = |sequence: u64, text: &str| {
            let operation = Sections::encode_operation(&sections::SetSection {
                index: 0,
                bytes: text.as_bytes().to_vec(),
                part: None,
            });
            let payload = Submitted {
                through: sequence - through,
                entry: Entry::Operation(be_block_session::Payload::Inline(operation)),
            }
            .encode();
            be_block_protocol::JournalEntry {
                sequence,
                origin: 7,
                counter: sequence,
                base: sequence - 1,
                floor: 0,
                received: Some(0),
                payload: phone
                    .commits()
                    .vault()
                    .seal(key, &payload)
                    .expect("the phone seals with its key"),
            }
        };
        for (sequence, text) in [(through + 1, "first"), (through + 2, "second")] {
            replicas.heard(
                (&peer, &shared),
                &Event::Sequenced {
                    block,
                    entry: entry(sequence, text),
                },
            );
        }
        assert!(
            replicas.waits_for_keys(block),
            "the entries wait for the key: {:?}",
            shared.lock().unwrap().errors
        );
        while replicas.waits_for_keys(block) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "the entries were opened: {errors:?}");
        assert_eq!(
            replicas.entries_given(block),
            [through + 1, through + 2],
            "the replica was handed both entries, the first included"
        );
    });
}
