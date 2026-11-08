use crate::stage::layout;

#[test]
fn stage_refuses_a_module_its_manifest_does_not_name() {
    let directory = std::env::temp_dir().join(format!("stage-refuses-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let manifest = directory.join("manifest.json");
    std::fs::write(
        &manifest,
        r#"{"id": "be3.notes", "entry_point": "notes.wasm"}"#,
    )
    .unwrap();
    let error = layout(&[format!("{}=other.wasm", manifest.display())]).unwrap_err();
    assert!(
        error.contains("names notes.wasm as its entry point"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(directory);
}
