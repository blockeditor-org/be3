use super::*;

#[test]
fn an_editor_closing_lets_go_of_its_revision_previews_once_each() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (block, head) = saved_counter();
    let mut instances = placed_on(block, be_type_example_counter::CounterContent::CONTENT_TYPE);
    preview_revision(&mut instances, 40, Some(41), block, head);
    answered_until(&mut instances, block, |answers, _| {
        answers.iter().any(|(op, _)| *op == 41)
    });
    preview_revision(&mut instances, 42, None, block, head);
    instances.remove(INSTANCE);
    answered_until(&mut instances, block, |_, previews| {
        previews.len() == 2 && previews.iter().all(|(_, released)| *released > 0)
    });
    assert_eq!(
        previews_of(block)
            .iter()
            .map(|(_, released)| *released)
            .collect::<Vec<_>>(),
        [1, 1],
        "the open preview and the one still opening are each let go once"
    );
}
