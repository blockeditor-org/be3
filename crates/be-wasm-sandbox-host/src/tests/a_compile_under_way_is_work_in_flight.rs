use super::*;

/// A module the guest asked to compile, until `poll` answers it, is work the server has yet to
/// finish.
#[test]
fn a_compile_under_way_is_work_in_flight() {
    let mut server = idle_server();
    assert!(server.settled(), "a server with nothing asked of it is settled");
    let compiling = be_wasm_host::sandbox::compile(ECHO.to_vec(), Arc::new(|| {}));
    server
        .compiling
        .push((compiling, Box::new(|_| (1, Outcome::Ok(Vec::new())))));
    assert!(!server.settled(), "a compile under way is not settled");
}
