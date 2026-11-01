use super::*;

#[test]
fn a_pure_guest_is_loaded_from_its_precompiled_module() {
    let directory = std::env::temp_dir().join(format!("pure-guest-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let wasm = directory.join("echo.wasm");
    std::fs::write(&wasm, ECHO).unwrap();
    precompile(std::slice::from_ref(&wasm), None).unwrap();
    let compiled = std::fs::read(directory.join("echo.cwasm")).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    let mut guest = Host::pure(None)
        .unwrap()
        .load_from("echo.wasm", |name| match name {
            "echo.cwasm" => Ok(Stored::Bytes(compiled.clone())),
            _ => Err(format!("{name} is not here")),
        })
        .expect("the precompiled module is used, without the module it was compiled from");
    guest.start().unwrap();
    assert_eq!(
        guest.take_outbound(),
        vec![block_guest_echo::STARTED.to_vec()]
    );
}
