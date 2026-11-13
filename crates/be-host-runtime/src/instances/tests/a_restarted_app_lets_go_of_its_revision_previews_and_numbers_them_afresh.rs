use super::*;

#[test]
fn a_restarted_app_lets_go_of_its_revision_previews_and_numbers_them_afresh() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (block, head) = saved_counter();
    let mut instances = placed_on(block, be_type_example_counter::CounterContent::CONTENT_TYPE);
    preview_revision(&mut instances, 40, Some(41), block, head);
    answered_until(&mut instances, block, |answers, _| {
        answers.iter().any(|(op, _)| *op == 41)
    });
    let (first, _) = previews_of(block)[0];

    instances.reopen();
    assert!(
        previews_of(block).contains(&(first, 1)),
        "the old app's preview is let go with it"
    );

    preview_revision(&mut instances, 40, Some(41), block, head);
    let answers = answered_until(&mut instances, block, |answers, _| {
        answers.iter().any(|(op, _)| *op == 41)
    });
    assert!(
        matches!(answers.as_slice(), [(41, Outcome::Ok(_))]),
        "the new app's preview, under the same number, opens: {answers:?}"
    );
    instances.ended(40);
    let previews = previews_of(block);
    assert_eq!(previews.len(), 2, "{previews:?}");
    assert!(
        previews.iter().all(|(_, released)| *released == 1),
        "ending the new one lets go of it alone, and the old one stays let go once: {previews:?}"
    );
}
