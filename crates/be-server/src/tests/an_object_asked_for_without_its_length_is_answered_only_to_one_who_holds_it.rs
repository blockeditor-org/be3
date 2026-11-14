use be_block_protocol::ObjectLength;

use super::*;

/// An object asked for with no length (a part the caller reuses by its ref alone) is answered
/// none to one who holds it, and refused as not found to anyone else, the same whether another
/// organization stored it or no one did: nothing is reserved for it, so a real uploader of it is
/// never refused, and the answer tells nothing of its size or existence.
#[tokio::test]
async fn an_object_asked_for_without_its_length_is_answered_only_to_one_who_holds_it() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    owner.personal().await;
    let author = Author::new(account);
    let block = owner.create_block(BlockParent::Root).await;
    let (commit, chunks) = author.compose(b"the owner's words\n", 1_000, None);
    let published = owner
        .publish(&author, block, commit, chunks.clone(), None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    let stored = author.objects_of(commit, &chunks)[0];
    let unknown = Hash::of(b"stored by no one");
    let asked = |hash| vec![ObjectLength { hash, length: None }];

    let held = owner
        .call(|peer| client::objects::upload_urls(peer, asked(stored)))
        .await;
    assert!(
        matches!(held.as_deref(), Ok([None])),
        "the owner holds it: {held:?}"
    );

    let sizes = || {
        metadata_database(&harness.directory)
            .query_row("SELECT COUNT(*) FROM object_sizes", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
    };
    let before = sizes();
    let mut other = harness.client().await;
    other.register("other@example.com").await;
    other.organization("elsewhere").await;
    let mut refusals = Vec::new();
    for hash in [stored, unknown] {
        let answer = other
            .call(|peer| client::objects::upload_urls(peer, asked(hash)))
            .await;
        let Err(error) = answer else {
            panic!("{hash} was answered to an account that does not hold it: {answer:?}");
        };
        assert_eq!(error.code, ErrorCode::ObjectNotFound, "{error:?}");
        refusals.push(error.message.replace(&hash.to_string(), "HASH"));
    }
    assert_eq!(
        refusals[0], refusals[1],
        "another organization's object is refused as one no one stored"
    );
    assert_eq!(sizes(), before, "nothing was reserved for either");

    // So the next uploader of the unknown object is not held to a length no one gave.
    let bytes = b"stored by no one".to_vec();
    other
        .put(vec![bytes.clone()])
        .await
        .expect("the real upload goes through");

    harness.stop().await;
}
