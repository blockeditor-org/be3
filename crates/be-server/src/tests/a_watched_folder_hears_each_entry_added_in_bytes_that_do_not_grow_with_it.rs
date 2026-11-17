use be_block_protocol::{ReferenceChange, encoded_len, references_digest};

use super::*;

/// The entries the folder holds before the watcher reads it.
const HELD: usize = 5_000;
/// Entries then added to the folder one publish at a time.
const ADDED: usize = 50;
/// What one announcement of a folder with no metadata may take, however many entries it holds:
/// its summary without them, and the one entry added. Every entry is 16 bytes, so the folder's
/// whole list would take 80 KB.
const ANNOUNCEMENT_BYTES: usize = 1024;

/// A folder's change is announced to a connection told its references as what the change did
/// to them, not the whole list again, so adding entries one at a time sends each watcher bytes
/// linear in them however many the folder holds; applying each change to what the watcher held
/// gives the server's references.
#[tokio::test]
async fn a_watched_folder_hears_each_entry_added_in_bytes_that_do_not_grow_with_it() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("filer@example.com").await;
    let organization = client.organization("files").await;
    let author = Author::new(account);
    let folder = client.create_block(BlockParent::Root).await;

    let mut entries = Vec::new();
    let mut head = None;
    let mut time = 1_000;
    let mut publish = async |client: &mut TestClient, entries: &mut Vec<Uuid>, adding: usize| {
        let before = entries.clone();
        entries.extend((0..adding).map(|_| Uuid::new_v4()));
        time += 1;
        let (commit, chunks) = author.compose(format!("{time}").as_bytes(), time, head);
        client
            .upload(&author.store, &author.objects_of(commit, &chunks))
            .await;
        let objects = objects_change(&author.held_by(head), chunks);
        let references = ReferenceChange::between(&before, entries);
        let expected = head;
        let response = client
            .call(|peer| {
                client::blocks::publish(
                    peer,
                    folder,
                    commit,
                    expected,
                    expected.into_iter().collect(),
                    objects,
                    time,
                    false,
                    references,
                    Vec::new(),
                    Vec::new(),
                    None,
                )
            })
            .await;
        assert!(
            matches!(
                response,
                Ok(be_block_protocol::PublishOutcome::Published(_))
            ),
            "{response:?}"
        );
        head = Some(commit);
    };
    publish(&mut client, &mut entries, HELD).await;

    let mut watcher = harness
        .second_connection(&client.token.clone(), organization)
        .await;
    let mut held = watcher.read_block(folder).await.references;
    assert_eq!(held, entries);

    let mut sent = 0;
    for entry in 0..ADDED {
        publish(&mut client, &mut entries, 1).await;
        let Notice::BlockChanged { block, moved, .. } = watcher.graph_change().await else {
            panic!("the watcher heard something other than the folder's change");
        };
        assert_eq!(block.id, folder);
        let bytes = encoded_len(&block) + encoded_len(&moved);
        assert!(
            bytes <= ANNOUNCEMENT_BYTES,
            "entry {entry}'s announcement took {bytes} bytes"
        );
        sent += bytes;
        let moved = moved.expect("a watcher told the references hears what changed in them");
        assert!(block.references.is_empty(), "{:?}", block.references);
        held = moved.change.apply(&held).expect("the change applies");
        assert_eq!(references_digest(&held), moved.digest);
    }
    assert_eq!(held, entries);
    assert!(sent <= ADDED * ANNOUNCEMENT_BYTES);

    harness.stop().await;
}
