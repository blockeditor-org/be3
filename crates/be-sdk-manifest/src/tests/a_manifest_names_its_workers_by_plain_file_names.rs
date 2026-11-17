use super::*;

#[test]
fn a_manifest_names_its_workers_by_plain_file_names() {
    let document = r#"{
        "id": "be3.map",
        "name": "Map",
        "version": "0.1.0",
        "editors": {
            "map": {
                "id": "378d6c68-d52a-4781-addb-fde9a8aff2e3",
                "display_name": "Map",
                "icon": "x",
                "views": ["Main"]
            }
        },
        "entry_point": "map.wasm",
        "workers": WORKERS
    }"#;
    let read = |workers: &str| manifest_from_json(&document.replace("WORKERS", workers));

    assert_eq!(
        read(r#"["tiles.wasm"]"#).unwrap().workers,
        vec!["tiles.wasm".to_owned()]
    );
    assert!(read("[]").unwrap().workers.is_empty());
    for refused in [
        r#"["../tiles.wasm"]"#,
        r#"["workers/tiles.wasm"]"#,
        r#"["tiles"]"#,
        r#"[".wasm"]"#,
        r#"["map.wasm"]"#,
        r#"["tiles.wasm", "tiles.wasm"]"#,
    ] {
        assert_eq!(read(refused), Err(ManifestError::InvalidWorker), "{refused}");
    }
}
