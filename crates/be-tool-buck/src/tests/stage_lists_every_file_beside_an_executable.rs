use crate::stage::{Entry, apps, layout, write};

#[test]
fn stage_lists_every_file_beside_an_executable() {
    use std::os::unix::fs::PermissionsExt;
    let directory = std::env::temp_dir().join(format!("stage-lists-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = |name: &str| directory.join(name).to_string_lossy().into_owned();
    std::fs::write(path("host"), "binary").unwrap();
    std::fs::write(path("info.json"), "{}").unwrap();
    let staged = layout(&[
        format!("--executable={}=be-host", path("host")),
        format!("--file={}", path("info.json")),
        "--index".into(),
    ])
    .unwrap();
    assert_eq!(
        staged["program-files.txt"],
        Entry::Write(b"apps.json\nbe-host\ninfo.json\n".to_vec())
    );
    assert_eq!(apps(&[]), "[]\n");
    let out = directory.join("out");
    write(&out, &staged).unwrap();
    let mode = std::fs::metadata(out.join("be-host")).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111);
    assert_eq!(std::fs::read_to_string(out.join("apps.json")).unwrap(), "[]\n");
    let _ = std::fs::remove_dir_all(directory);
}
