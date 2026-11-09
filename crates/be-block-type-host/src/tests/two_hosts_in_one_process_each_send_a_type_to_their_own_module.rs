use super::*;

#[test]
fn two_hosts_in_one_process_each_send_a_type_to_their_own_module() {
    // Every test of the process names one build, as the server a test starts needs.
    be_lib_build_id::install("test");
    let counter = be_type_example_counter::CounterContent::CONTENT_TYPE;
    let mut both = pollster::block_on(started(
        vec![
            in_process::<be_type_checklist::BlockTypes>("be3.checklist"),
            in_process::<be_type_example_counter::BlockTypes>("be3.counter"),
        ],
        be_block_client::Clock::real(),
    ));
    let mut alone = pollster::block_on(started(
        vec![in_process::<be_type_example_counter::BlockTypes>(
            "be3.counter",
        )],
        be_block_client::Clock::real(),
    ));
    assert_eq!(both.module_of(counter), Some(1));
    assert_eq!(alone.module_of(counter), Some(0));
    for modules in [&mut both, &mut alone] {
        let blank = pollster::block_on(modules.blank(counter));
        assert!(blank.is_ok(), "the counter's own module answers: {blank:?}");
    }
}
