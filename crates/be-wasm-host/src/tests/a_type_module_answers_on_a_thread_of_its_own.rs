use super::*;

#[test]
fn a_type_module_answers_on_a_thread_of_its_own() {
    assert!(!counter_answers([3; 32]).is_empty());
}
