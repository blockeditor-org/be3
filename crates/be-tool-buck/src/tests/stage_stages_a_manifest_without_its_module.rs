use crate::stage::{Entry, layout};

/// A MANIFEST= with no module stages the manifest and its type module, and lists it in
/// apps.json, without the app's module: what `be` reads block types from.
#[test]
fn stage_stages_a_manifest_without_its_module() {
    let directory = std::env::temp_dir().join(format!("stage-manifest-alone-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let manifest = directory.join("manifest.json");
    std::fs::write(
        &manifest,
        r#"{"id": "be3.notes", "entry_point": "notes.wasm",
            "block_types": {"notes": {"module": "notes_types.wasm"}}}"#,
    )
    .unwrap();
    let types = directory.join("notes_types.wasm");
    std::fs::write(&types, b"\0asm").unwrap();
    let staged = layout(&[
        "--index".to_owned(),
        format!("{}=", manifest.display()),
        format!("--app-type={}", types.display()),
    ])
    .unwrap();
    let paths: Vec<&str> = staged.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        ["apps.json", "be3.notes.app.json", "notes_types.sha256", "notes_types.wasm"]
    );
    assert!(matches!(&staged["apps.json"], Entry::Write(listed) if String::from_utf8_lossy(listed).contains("be3.notes.app.json")));
    let _ = std::fs::remove_dir_all(directory);
}
