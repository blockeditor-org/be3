use super::*;

/// Each commit names the head of the block's signed log its device had verified
/// (`Commit::log_through`), sealed with it, and the server takes it only as a head its log had.
/// A device that loads the commit as the block's head compares that head with its own
/// (guides/blocks/keys.md, "Forks"): the laptop saved on the log before the server rewrote it,
/// and the tablet, shown the rewritten log, finds the fork from the commit alone.
#[tokio::test]
async fn a_commit_names_the_log_head_it_was_made_on_and_a_reader_compares_it() {
    let harness = Harness::start().await;
    let laptop = harness.owner("owner@example.com").await;
    let block = note(&laptop).await;
    for text in ["one", "two"] {
        laptop
            .append(
                block,
                "test.app",
                text.as_bytes(),
                be_block_content::Along::default(),
            )
            .await
            .unwrap();
    }
    let kept = laptop.read_log(block, 0, 10).await.unwrap().head;
    let Saved::Published(head) = laptop
        .save(block, &TextBlock::of("made on two entries"), None)
        .await
        .unwrap()
    else {
        panic!("the server refused a commit naming a head its log had");
    };
    let made = laptop.commits().get(head).unwrap();
    let named = made
        .log_through
        .clone()
        .expect("the commit names the head it was made on");
    assert_eq!((named.size, named.root), (kept.size, *kept.root.as_bytes()));
    assert!(
        laptop.keys().identity().signing().verifies_log_head(
            block.into_bytes(),
            named.size,
            &named.root,
            &named.signature
        ),
        "the head a commit names is signed by its author"
    );

    // A device on the same log loads it and finds nothing amiss.
    let phone = another(&harness, &laptop).await;
    phone.read_log(block, 0, 10).await.unwrap();
    phone.load_head(block, head).await.unwrap();
    assert_eq!(phone.log_fork(block), None);

    {
        let database = harness.database();
        be_server::log::forget(&database, laptop.organization(), block).unwrap();
        let entries = (0..3)
            .map(|index| be_block_protocol::NewEntry {
                id: Uuid::new_v4(),
                payload: format!("made up {index}").into_bytes(),
                signature: vec![1; 64],
            })
            .collect();
        be_server::log::append(
            &database,
            laptop.organization(),
            block,
            laptop.account(),
            entries,
            be_block_commit::now_milliseconds(),
        )
        .unwrap();
    }
    let tablet = another(&harness, &laptop).await;
    tablet.read_log(block, 0, 10).await.unwrap();
    tablet.load_head(block, head).await.unwrap();
    let fork = tablet
        .log_fork(block)
        .expect("the tablet took the commit's head of another log as agreeing");
    assert_eq!(
        (fork.ours.size, fork.theirs.size, fork.from),
        (
            3,
            2,
            HeadFrom::Commit {
                author: laptop.account(),
                signature: named.signature.clone(),
            }
        )
    );
    harness.stop().await;
}
