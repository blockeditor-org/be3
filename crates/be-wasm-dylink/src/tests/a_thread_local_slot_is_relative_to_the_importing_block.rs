use super::*;

#[test]
fn a_thread_local_slot_is_relative_to_the_importing_block() {
    assert_eq!(thread_local_slot(5000, 1000), 4000);
    assert_eq!(thread_local_slot(1000, 5000), -4000);
}
