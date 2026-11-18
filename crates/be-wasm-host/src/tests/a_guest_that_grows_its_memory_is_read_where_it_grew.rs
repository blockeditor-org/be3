use super::*;

/// The host reads a guest's memory from where it was made (engine::memories_stay_put), so a call
/// after the guest grew it must see the new pages: here the buffer's descriptor is copied into
/// them and read from there.
#[test]
fn a_guest_that_grows_its_memory_is_read_where_it_grew() {
    let source = guest(
        "(if (i32.ne (memory.grow (i32.const 2)) (i32.const 1)) (then unreachable))
         (memory.copy (i32.const 131072) (i32.const 0) (global.get $length))
         (i32.store (i32.const 1024) (call $create_buffer (i32.const 131072) (global.get $length)))
         (call $host_send (i32.const 1024) (i32.const 4))",
    );
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    app.start().unwrap();
    app.step().unwrap();

    let outbound = app.take_outbound();
    assert_eq!(outbound.len(), 1);
    let handle = u32::from_le_bytes(outbound[0][..4].try_into().unwrap());
    assert_ne!(handle, 0, "the descriptor in the grown pages should have been read");
    assert!(
        engine::memories_stay_put(),
        "V8's trap handler should be on here, which keeps the gpu calls fast"
    );
}
