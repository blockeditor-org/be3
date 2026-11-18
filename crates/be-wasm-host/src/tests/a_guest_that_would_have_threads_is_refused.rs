use super::*;

#[test]
fn a_guest_that_would_have_threads_is_refused() {
    let entries = r#"(func (export "app_start")) (func (export "app_step")) (func (export "app_shutdown"))"#;
    for (module, why) in [
        (
            format!(r#"(module (import "env" "memory" (memory 1 4 shared)) {entries})"#),
            "a shared memory",
        ),
        (
            format!(r#"(module (memory (export "memory") 1 4 shared) {entries})"#),
            "a shared memory",
        ),
        (
            format!(
                r#"(module (import "wasi" "thread-spawn" (func (param i32) (result i32))) {entries})"#
            ),
            "asks to start threads",
        ),
    ] {
        let Err(error) = host().load_bytes(module.as_bytes()) else {
            panic!("{module} loaded");
        };
        assert!(
            error.contains(why) && error.contains("single-threaded"),
            "{module}: {error}"
        );
    }
}
