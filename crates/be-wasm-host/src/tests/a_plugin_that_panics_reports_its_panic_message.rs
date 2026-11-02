use super::*;

#[test]
fn a_plugin_that_panics_reports_its_panic_message() {
    let message = "the count overflowed\n(at src/counter.rs:12:9)";
    let source = format!(
        r#"(module
            (import "env" "memory" (memory 1 4 shared))
            (import "be3_host" "host_panic" (func $host_panic (param i32 i32)))
            (data (i32.const 64) "{text}")
            (func (export "plugin_start"))
            (func (export "plugin_shutdown"))
            (func (export "plugin_step")
                (call $host_panic (i32.const 64) (i32.const {length}))
                unreachable)
        )"#,
        text = escaped(message.as_bytes()),
        length = message.len(),
    );
    let mut plugin = host().load_bytes(source.as_bytes()).unwrap();
    plugin.start().unwrap();

    let failure = plugin
        .step()
        .expect_err("a panicking plugin fails its step");

    assert_eq!(failure, message);
    assert_eq!(
        plugin.step(),
        Ok(()),
        "a plugin that panicked is not run again"
    );
}
