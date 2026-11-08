use crate::stage::{Entry, apps, describe, layout, write};

#[test]
fn stage_lists_every_file_beside_an_executable() {
    let directory = std::env::temp_dir().join(format!("stage-lists-{}", std::process::id()));
    let staged = layout(&[
        "--executable=be-host=be-host".into(),
        "--file=info.json".into(),
        "--index".into(),
    ])
    .unwrap();
    assert_eq!(
        staged["program-files.txt"],
        Entry::Write(b"apps.json\nbe-host\ninfo.json\n".to_vec())
    );
    assert_eq!(apps(&[]), "[]\n");
    let described: serde_json::Value = serde_json::from_str(&describe(&staged)).unwrap();
    assert_eq!(
        described,
        serde_json::json!([
            {"path": "apps.json"},
            {"path": "be-host", "argument": 0, "part": 0, "within": ""},
            {"path": "info.json", "argument": 1, "part": 0, "within": ""},
            {"path": "program-files.txt"},
        ])
    );
    write(&directory, &staged).unwrap();
    assert_eq!(
        std::fs::read_to_string(directory.join("apps.json")).unwrap(),
        "[]\n"
    );
    assert!(!directory.join("be-host").exists());
    let _ = std::fs::remove_dir_all(directory);
}
