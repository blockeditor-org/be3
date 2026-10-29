use super::*;

struct Unreadable;

impl BlockContent for Unreadable {
    const CONTENT_TYPE: Uuid = CounterContent::CONTENT_TYPE;

    fn encode(&self) -> Vec<u8> {
        b"saved by another build".to_vec()
    }

    fn decode(_: &[u8]) -> Result<Self, be_block::ContentError> {
        Ok(Self)
    }
}

#[test]
fn a_block_another_build_saved_is_offered_for_reset_and_a_held_one_is_reset() {
    let harness = Harness::start();
    let (shown, held) = (Uuid::new_v4(), Uuid::new_v4());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts");
    let _outside = runtime.block_on(async {
        let outside = harness.outside_peer().await;
        for block in [shown, held] {
            outside
                .ensure::<Unreadable>(block, be_graph::BlockParent::Root)
                .await
                .expect("the block is created");
            outside
                .save(block, &Unreadable, None)
                .await
                .expect("another build's content is saved");
        }
        outside
    });

    harness.connect();
    wait_until("loaded the graph", |shared| {
        shared.graph.ready(crate::be::Query::Roots)
    });
    open(shown, CounterContent::CONTENT_TYPE);
    wait_until("found the block unreadable", |shared| {
        shared.unreadable.contains_key(&shown)
    });
    assert!(matches!(staleness(shown), Some(Stale::Unreadable(_))));

    try_anyway(shown);
    assert!(
        matches!(staleness(shown), Some(Stale::Unreadable(_))),
        "an unreadable block was shown anyway, with nothing to show"
    );

    reset(shown);
    wait_for_count(shown, 0);
    wait_until("forgot the block was unreadable", |shared| {
        !shared.unreadable.contains_key(&shown)
    });
    wait_until("restamped the reset block", |shared| {
        shared
            .graph
            .get(shown)
            .and_then(|node| node.metadata.get(&be_block::SCHEMA))
            == Some(be_block::schema_fingerprint::<CounterContent>())
    });
    assert_eq!(staleness(shown), None);

    hold(held, CounterContent::CONTENT_TYPE);
    wait_for_count(held, 0);
    assert!(
        block_app_base::notices::shown()
            .iter()
            .any(|notice| notice.message.contains("were reset")),
        "resetting a held block said nothing"
    );
}
