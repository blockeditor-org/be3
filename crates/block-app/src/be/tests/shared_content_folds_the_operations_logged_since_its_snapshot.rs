use super::*;

fn value(content: &Content) -> i64 {
    CounterContent::decode(content.bytes())
        .expect("the counter decodes")
        .root()
        .value()
}

#[test]
fn shared_content_folds_the_operations_logged_since_its_snapshot() {
    let mut content = Content::new(
        CounterContent::CONTENT_TYPE,
        CounterContent::default().encode(),
        Vec::new(),
        None,
    );
    for _ in 0..3 {
        content.record(worker::Logged::Operation {
            bytes: CounterContent::encode_operation(&Counter::add(2)),
            origin: None,
        });
    }
    assert_eq!(content.unfolded(), 3);
    assert_eq!(value(&content), 6, "reading folds the logged operations in");

    let mut settled = CounterContent::default();
    settled.apply(&Counter::add(10));
    content.settle(settled.encode(), Vec::new());
    assert_eq!(content.unfolded(), 0);
    assert_eq!(value(&content), 10);

    content.record(worker::Logged::Operation {
        bytes: CounterContent::encode_operation(&Counter::add(1)),
        origin: None,
    });
    assert_eq!(
        value(&content),
        11,
        "a later operation is folded onto the new snapshot"
    );
}
