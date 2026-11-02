use super::*;

#[test]
fn a_guest_that_reaches_the_gpu_without_a_device_is_told_why() {
    let host =
        Host::on_demand(|| Err("no graphics adapter is available".to_owned()), None).unwrap();
    let source = guest("(drop (call $create_buffer (i32.const 0) (global.get $length)))");
    let mut app = host.load_bytes(source.as_bytes()).unwrap();
    app.start().unwrap();

    let error = app.step().unwrap_err();

    assert!(
        error.contains("no graphics adapter is available"),
        "{error}"
    );
}
