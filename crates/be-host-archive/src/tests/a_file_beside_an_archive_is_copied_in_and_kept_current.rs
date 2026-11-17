use super::*;

#[test]
fn a_file_beside_an_archive_is_copied_in_and_kept_current() {
    let scratch = std::env::temp_dir().join(format!("program-archive-beside-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let staged = scratch.join("staged");
    fs::create_dir_all(&staged).unwrap();
    fs::write(staged.join("app.wasm"), b"app").unwrap();
    let archive = scratch.join("staged.programz");
    pack(&staged, &archive).unwrap();
    let info = scratch.join("build-info.json");
    fs::write(&info, b"{\"id\":\"one\"}").unwrap();
    let dir = unpacked(&archive, std::slice::from_ref(&info)).unwrap();
    assert_eq!(fs::read(dir.join("build-info.json")).unwrap(), b"{\"id\":\"one\"}");
    fs::write(&info, b"{\"id\":\"two\"}").unwrap();
    assert_eq!(unpacked(&archive, std::slice::from_ref(&info)).unwrap(), dir);
    assert_eq!(fs::read(dir.join("build-info.json")).unwrap(), b"{\"id\":\"two\"}");
    assert_eq!(fs::read(dir.join("app.wasm")).unwrap(), b"app");
    fs::remove_dir_all(&scratch).unwrap();
}
