use super::*;

#[tokio::test]
async fn a_reordered_reference_list_keeps_its_new_order() {
    let harness = Harness::start().await;
    let author = harness.owner("presenter@example.com").await;
    let mut slides = Vec::new();
    for _ in 0..3 {
        slides.push(
            author
                .create::<ImageContent>(BlockParent::Root)
                .await
                .unwrap(),
        );
    }
    slides.sort_unstable();
    let deck = author.create::<Link>(BlockParent::Root).await.unwrap();

    let mut head = None;
    for targets in [
        vec![slides[0], slides[1], slides[2]],
        vec![slides[2], slides[0], slides[1]],
    ] {
        head = Some(
            author
                .save(
                    deck,
                    &Link {
                        targets: targets.clone(),
                    },
                    head,
                )
                .await
                .unwrap()
                .published()
                .unwrap(),
        );
        assert_eq!(
            author.load_commit(head.unwrap()).await.unwrap().references,
            targets,
            "the commit did not keep the order the content listed"
        );
        assert_eq!(
            author.summary(deck).await.unwrap().references,
            targets,
            "the graph did not keep the order the content listed"
        );
    }

    harness.stop().await;
}
