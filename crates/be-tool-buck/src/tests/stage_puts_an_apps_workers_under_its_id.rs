use crate::stage::layout;

/// An app's worker modules are staged under workers/<id>/, and an app staged with its module but
/// without a worker its manifest names, or with one it does not name, is refused.
#[test]
fn stage_puts_an_apps_workers_under_its_id() {
    let directory = std::env::temp_dir().join(format!("stage-workers-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let manifest = directory.join("manifest.json");
    std::fs::write(
        &manifest,
        r#"{"id": "be3.map", "entry_point": "map.wasm", "workers": ["tiles.wasm"]}"#,
    )
    .unwrap();
    let app = format!("{}=map.wasm", manifest.display());
    let staged = layout(&[app.clone(), "--worker=out/tiles.wasm".to_owned()]).unwrap();
    let paths: Vec<&str> = staged.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        ["be3.map.app.json", "map.wasm", "workers/be3.map/tiles.wasm"]
    );

    let error = layout(std::slice::from_ref(&app)).unwrap_err();
    assert!(error.contains("staged without it"), "{error}");
    let error = layout(&[app, "--worker=out/other.wasm".to_owned()]).unwrap_err();
    assert!(error.contains("names no worker other.wasm"), "{error}");
    let _ = std::fs::remove_dir_all(directory);
}
