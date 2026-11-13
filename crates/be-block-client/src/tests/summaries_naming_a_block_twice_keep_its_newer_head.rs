use super::*;

use be_type_text::{TextBlock, TextContent};

#[tokio::test]
async fn summaries_naming_a_block_twice_keep_its_newer_head() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let mut heads = Vec::new();
    let mut summaries = Vec::new();
    for text in ["one", "two", "three"] {
        let head = owner
            .save(note, &TextBlock::of(text), heads.last().copied())
            .await
            .unwrap()
            .published()
            .unwrap();
        heads.push(head);
        summaries.push(owner.summary(note).await.unwrap());
    }
    let [one, _, three] = heads[..] else {
        unreachable!()
    };
    let (two, newer) = (summaries[1].clone(), summaries[2].clone());
    assert!(two.revision < newer.revision);
    let mut same = two.clone();
    same.revision = newer.revision;

    // A device that knows the first head hears of the second and the third in one answer: each
    // is news to it, but the second is behind the third.
    for (case, answer) in [
        (
            "an older summary after a newer one",
            [newer.clone(), two.clone()],
        ),
        (
            "one revision, the older head after",
            [newer.clone(), same.clone()],
        ),
        (
            "one revision, the older head before",
            [same.clone(), newer.clone()],
        ),
    ] {
        let refs = scoped(&refs::RefsDb::memory());
        refs.set_remote(note, one, owner.behind()).unwrap();
        refs.observe(&answer, owner.behind()).unwrap();
        assert_eq!(refs.remote(note), Some(three), "{case}");
    }

    harness.stop().await;
}
