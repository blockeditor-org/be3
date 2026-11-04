use super::*;

#[test]
fn a_guest_creates_a_buffer_and_reports_its_handle() {
    let source = guest(
        "(i32.store (i32.const 1024) (call $create_buffer (i32.const 0) (global.get $length)))
         (call $host_send (i32.const 1024) (i32.const 4))",
    );
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    app.start().unwrap();
    app.step().unwrap();
    let outbound = app.take_outbound();
    assert_eq!(outbound.len(), 1);
    let handle = u32::from_le_bytes(outbound[0][..4].try_into().unwrap());
    assert_ne!(handle, 0);
}
