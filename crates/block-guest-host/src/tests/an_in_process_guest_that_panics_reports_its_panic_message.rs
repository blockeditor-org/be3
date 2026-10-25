use super::*;

#[test]
fn an_in_process_guest_that_panics_reports_its_panic_message() {
    let (mut worker, _) = echo();
    assert_eq!(worker.queue(vec![block_guest_echo::PANIC.to_vec()]), None);
    let failure = polled(&mut worker, 3).unwrap_err();
    assert_eq!(failure, block_guest_echo::PANIC_MESSAGE);
}
