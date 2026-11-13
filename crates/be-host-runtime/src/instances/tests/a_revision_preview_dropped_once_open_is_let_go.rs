use super::*;

#[test]
fn a_revision_preview_dropped_once_open_is_let_go() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (block, head) = saved_counter();
    let mut instances = placed_on(block, be_type_example_counter::CounterContent::CONTENT_TYPE);
    preview_revision(&mut instances, 40, Some(41), block, head);
    let answers = answered_until(&mut instances, block, |answers, _| {
        answers.iter().any(|(op, _)| *op == 41)
    });
    let [(_, Outcome::Ok(_))] = answers.as_slice() else {
        panic!("the preview opens: {answers:?}");
    };
    assert_eq!(
        previews_of(block).iter().map(|(_, released)| *released).collect::<Vec<_>>(),
        [0],
        "open while it is held"
    );
    instances.ended(40);
    instances.ended(40);
    assert_eq!(
        previews_of(block).iter().map(|(_, released)| *released).collect::<Vec<_>>(),
        [1],
        "dropping it lets it go, once"
    );
}
