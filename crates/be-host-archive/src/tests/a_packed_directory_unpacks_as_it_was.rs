use super::*;

#[test]
fn a_packed_directory_unpacks_as_it_was() {
    let scratch = std::env::temp_dir().join(format!("app-archive-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let staged = scratch.join("staged");
    fs::create_dir_all(staged.join("data/be3.settings")).unwrap();
    fs::write(staged.join("app"), b"#!/bin/sh\n").unwrap();
    make_executable(&staged.join("app")).unwrap();
    fs::write(staged.join("plugin.wasm"), vec![7; 300_000]).unwrap();
    fs::write(staged.join("data/be3.settings/empty"), b"").unwrap();
    let archive = scratch.join("staged.appz");
    pack(&staged, &archive).unwrap();
    assert!(fs::metadata(&archive).unwrap().len() < 10_000);
    let out = scratch.join("out");
    unpack(&archive, &out).unwrap();
    assert_eq!(fs::read(out.join("app")).unwrap(), b"#!/bin/sh\n");
    assert_eq!(fs::read(out.join("plugin.wasm")).unwrap(), vec![7; 300_000]);
    assert_eq!(fs::read(out.join("data/be3.settings/empty")).unwrap(), b"");
    assert!(executable(&fs::metadata(out.join("app")).unwrap()));
    assert!(!executable(&fs::metadata(out.join("plugin.wasm")).unwrap()));
    let again = scratch.join("again.appz");
    pack(&staged, &again).unwrap();
    assert_eq!(fs::read(&archive).unwrap(), fs::read(&again).unwrap());
    let cached = unpacked(&archive).unwrap();
    assert_eq!(cached, unpacked(&archive).unwrap());
    assert_eq!(
        fs::read(cached.join("plugin.wasm")).unwrap(),
        vec![7; 300_000]
    );
    fs::remove_dir_all(&scratch).unwrap();
}
