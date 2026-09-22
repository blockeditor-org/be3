use super::*;

#[test]
fn a_checklist_and_a_counter_are_held_by_one_peer() {
    let harness = Harness::start();
    harness.connect();
    let counter = Uuid::new_v4();
    let checklist = Uuid::new_v4();

    open(counter, CounterContent::CONTENT_TYPE);
    open(checklist, ChecklistContent::CONTENT_TYPE);
    add(counter, 3);
    operate(
        checklist,
        ChecklistContent::encode_operation(&ChecklistOp::add("buy milk")),
    );
    wait_for_count(counter, 3);
    let listed = |shared: &Shared| {
        let held = shared.blocks.get(&checklist)?;
        (held.content_type == ChecklistContent::CONTENT_TYPE)
            .then(|| ChecklistContent::decode(&held.bytes).ok())
            .flatten()
            .map(|content| {
                content
                    .items()
                    .iter()
                    .map(|item| item.text.clone())
                    .collect::<Vec<_>>()
            })
    };
    wait_until("listed the item", |shared| {
        listed(shared) == Some(vec!["buy milk".to_owned()])
    });
    flush();
    stop();

    harness.connect();
    open(checklist, ChecklistContent::CONTENT_TYPE);

    wait_until("read the checklist back", |shared| {
        listed(shared) == Some(vec!["buy milk".to_owned()])
    });
}
