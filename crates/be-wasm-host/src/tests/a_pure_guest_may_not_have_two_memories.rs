use super::*;

#[test]
fn a_pure_guest_may_not_have_two_memories() {
    let refused = Host::pure()
        .unwrap()
        .load_bytes(
            br#"(module
                (memory 1) (memory 1)
                (func (export "app_start")) (func (export "app_step")) (func (export "app_shutdown")))"#,
        )
        .err()
        .expect("a guest with two memories is refused");
    assert!(refused.contains("may not"), "{refused}");
}
