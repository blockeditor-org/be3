use super::*;

#[test]
fn a_revision_preview_dropped_before_it_opens_is_let_go_once_it_opens() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (block, head) = saved_counter();
    let mut instances = placed_on(block, be_type_example_counter::CounterContent::CONTENT_TYPE);
    preview_revision(&mut instances, 40, None, block, head);
    instances.ended(40);
    let answers = answered_until(&mut instances, block, |_, previews| {
        previews.iter().any(|(_, released)| *released > 0)
    });
    assert!(answers.is_empty(), "nothing waits on it: {answers:?}");
    assert_eq!(
        previews_of(block)
            .iter()
            .map(|(_, released)| *released)
            .collect::<Vec<_>>(),
        [1],
        "the preview that opened after the editor let go is let go"
    );
}
