use crate::stage::{Entry, layout};

#[test]
fn stage_names_libraries_by_their_sha256_and_apps_by_their_id() {
    let directory = std::env::temp_dir().join(format!("stage-names-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("data/files")).unwrap();
    let path = |name: &str| directory.join(name).to_string_lossy().into_owned();
    std::fs::write(path("library.wasm"), "abc").unwrap();
    std::fs::write(
        path("manifest.json"),
        r#"{"id": "be3.notes", "block_types": {"note": {"module": "notes_types.wasm"}}, "entry_point": "notes.wasm"}"#,
    )
    .unwrap();
    for name in ["notes.wasm", "notes.cwasm", "notes_types.wasm", "data/files/a.txt"] {
        std::fs::write(path(name), name).unwrap();
    }
    let staged = layout(&[
        format!("--library={}", path("library.wasm")),
        format!("{}={}", path("manifest.json"), path("notes.wasm")),
        format!("--data={}", path("data")),
        format!("--artifact={}", path("notes.cwasm")),
        format!("--app-type={}", path("notes_types.wasm")),
        "--index".into(),
    ])
    .unwrap();
    assert_eq!(
        staged.keys().collect::<Vec<_>>(),
        [
            "apps.json",
            "be3.notes.app.json",
            "data/be3.notes/files/a.txt",
            "lib-ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad.wasm",
            "notes.cwasm",
            "notes.wasm",
            "notes_types.sha256",
            "notes_types.wasm",
        ]
    );
    assert_eq!(
        staged["apps.json"],
        Entry::Write(b"[\n  \"be3.notes.app.json\"\n]\n".to_vec())
    );
    let _ = std::fs::remove_dir_all(directory);
}
