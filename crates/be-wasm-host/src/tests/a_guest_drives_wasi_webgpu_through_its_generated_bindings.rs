use super::*;

/// be-wasm-guest-webgpu runs the generated guest half of a local interface on V8: tickets
/// settled through `[poll]`, a fallible call's error through `[returned]`, and options of lists
/// passed flat. It panics, naming what went wrong, unless all of it answers as it should.
#[test]
fn a_guest_drives_wasi_webgpu_through_its_generated_bindings() {
    let mut app = host().load_bytes(WEBGPU).unwrap();
    app.start().unwrap();
    app.step().unwrap();
    assert_eq!(
        app.take_outbound(),
        vec![be_wasm_guest_webgpu::DONE.to_vec()]
    );
}
