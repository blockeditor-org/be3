use super::*;

use crate::Event;

/// The server tells a connection that read a block what a change did to its references, not
/// the whole list again (`moved-references`); the connection applies it to what this device
/// holds, so whoever listens hears the summary with every reference.
#[tokio::test]
async fn a_change_announced_as_what_moved_in_its_references_arrives_with_them_all() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let mut slides = Vec::new();
    for _ in 0..3 {
        slides.push(
            phone
                .create::<ImageContent>(BlockParent::Root)
                .await
                .unwrap(),
        );
    }
    let deck = phone.create::<Link>(BlockParent::Root).await.unwrap();
    let first = phone
        .save(
            deck,
            &Link {
                targets: vec![slides[0], slides[1]],
            },
            None,
        )
        .await
        .unwrap()
        .published()
        .unwrap();
    let laptop = harness.second(&phone).await;
    let mut on_laptop = laptop.connection().subscribe();
    assert_eq!(
        laptop.summary(deck).await.unwrap().references,
        vec![slides[0], slides[1]]
    );

    let mut head = first;
    for targets in [
        vec![slides[1], slides[2], slides[0]],
        vec![slides[2], slides[0]],
    ] {
        head = phone
            .save(
                deck,
                &Link {
                    targets: targets.clone(),
                },
                Some(head),
            )
            .await
            .unwrap()
            .published()
            .unwrap();
        let references = loop {
            if let Event::BlockChanged { block, .. } = on_laptop.recv().await.unwrap()
                && block.id == deck
                && block.head == Some(head)
            {
                break block.references;
            }
        };
        assert_eq!(references, targets);
    }

    harness.stop().await;
}
