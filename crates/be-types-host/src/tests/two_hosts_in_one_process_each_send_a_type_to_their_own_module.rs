use super::*;

#[test]
fn two_hosts_in_one_process_each_send_a_type_to_their_own_module() {
    let counter = counter_types::CounterContent::CONTENT_TYPE;
    let mut both = pollster::block_on(started(
        vec![
            in_process::<checklist_types::BlockTypes>("be3.checklist"),
            in_process::<counter_types::BlockTypes>("be3.counter"),
        ],
        be_client::Clock::real(),
    ));
    let mut alone = pollster::block_on(started(
        vec![in_process::<counter_types::BlockTypes>("be3.counter")],
        be_client::Clock::real(),
    ));
    assert_eq!(both.module_of(counter), Some(1));
    assert_eq!(alone.module_of(counter), Some(0));
    for modules in [&mut both, &mut alone] {
        let blank = pollster::block_on(modules.blank(counter));
        assert!(blank.is_ok(), "the counter's own module answers: {blank:?}");
    }
}
