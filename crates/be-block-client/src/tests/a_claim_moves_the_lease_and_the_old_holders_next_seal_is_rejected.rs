use be_block_session::Membership;
use be_type_text::{TextBlock, TextContent};

use super::*;
use crate::Event;

/// A claim takes the lease at once: every participant hears it in the session's order, the
/// claimant seals, and the old holder's next seal, made on the head it last saw, is rejected,
/// as is its trim with no new commit.
#[tokio::test]
async fn a_claim_moves_the_lease_and_the_old_holders_next_seal_is_rejected() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let note = phone
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    let head = phone
        .save(note, &TextBlock::of("first"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let laptop = harness.second(&phone).await;
    let mut on_phone = phone.connection().subscribe();
    let (holder, mut seen, ..) = phone.connection().join_session(phone.organization(), note, BUILD).await.unwrap();
    let (claimant, ..) = laptop.connection().join_session(laptop.organization(), note, BUILD).await.unwrap();
    submitted(&phone, note, 1).await;
    submitted(&phone, note, 2).await;
    assert!(seen.is_holder(holder));

    laptop.connection().claim(note).await.unwrap();
    while !seen.is_holder(claimant) {
        if let Event::Membership { block, entry } = on_phone.recv().await.unwrap()
            && block == note
        {
            seen.apply(&entry);
        }
    }

    let laptops = TextBlock::of("sealed by the claimant");
    let Saved::Published(sealed) = laptop
        .write(note, sealing(&laptops, head, 1))
        .await
        .unwrap()
    else {
        panic!("the claimant's seal is taken");
    };
    let phones = TextBlock::of("sealed by the old holder");
    let late = phone.write(note, sealing(&phones, head, 2)).await.unwrap();
    assert!(
        matches!(late, Saved::Rejected { .. }),
        "the old holder's seal on the head it saw is rejected: {late:?}"
    );
    let trimmed = phone
        .rebase_journal(
            note,
            BUILD,
            0,
            be_block_protocol::JournalBase {
                commit: Some(sealed),
                sequence: 2,
                state: Vec::new(),
            },
            true,
            false,
        )
        .await
        .unwrap()
        .expect("a trim goes to the server");
    trimmed.await.unwrap();
    assert_eq!(
        server_journal(&laptop, note).await,
        Some((Some(sealed), 1, vec![2])),
        "the claimant's seal holds the journal, which the old holder's trim left as it was"
    );
    harness.stop().await;
}
