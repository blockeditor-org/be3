use super::*;

/// Keeps a megabyte array alive on every turn of its loop, on V8's heap rather than in its
/// linear memory.
const HOARDS: &str = r#"(module
    (type $bytes (array (mut i8)))
    (type $node (struct (field (ref null $node)) (field (ref $bytes))))
    (global $head (mut (ref null $node)) (ref.null $node))
    (func (export "app_start"))
    (func (export "app_shutdown"))
    (func (export "app_step")
        (loop $hoard
            (global.set $head
                (struct.new $node (global.get $head) (array.new_default $bytes (i32.const 1048576))))
            (br $hoard)))
)"#;

#[test]
fn a_guest_that_declares_garbage_collected_types_is_refused() {
    let refused = host()
        .load_bytes(HOARDS.as_bytes())
        .err()
        .expect("a guest that could fill V8's heap is refused");
    assert!(refused.contains("garbage-collected"), "{refused}");
}
