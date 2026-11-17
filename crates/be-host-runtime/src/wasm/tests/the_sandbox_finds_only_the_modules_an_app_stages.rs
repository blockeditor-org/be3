/// An app's modules are its entry point and its workers, under workers/<id>/; any other name, a
/// path out of that directory or another app's module, is refused.
#[test]
fn the_sandbox_finds_only_the_modules_an_app_stages() {
    let _installed = be_host_stack::discovery::install_for_testing(&[
        (
            "be3.map.app.json",
            r#"{"id": "be3.map", "name": "Map", "version": "0.1.0", "entry_point": "map.wasm",
                "workers": ["tiles.wasm"], "editors": {"map": {
                    "id": "00000000-0000-4000-8000-00000000a001",
                    "display_name": "Map", "icon": "x", "views": ["Main"]}}}"#,
        ),
        (
            "be3.notes.app.json",
            r#"{"id": "be3.notes", "name": "Notes", "version": "0.1.0", "entry_point": "notes.wasm",
                "editors": {"notes": {
                    "id": "00000000-0000-4000-8000-00000000a002",
                    "display_name": "Notes", "icon": "x", "views": ["Main"]}}}"#,
        ),
    ]);
    let entry = |module: &str| crate::staged_app("be3.map", module).map(|staged| staged.entry().to_owned());
    assert_eq!(entry("map.wasm"), Ok("map.wasm".to_owned()));
    assert_eq!(entry("tiles.wasm"), Ok("workers/be3.map/tiles.wasm".to_owned()));
    for refused in ["notes.wasm", "../notes.wasm", "workers/be3.map/tiles.wasm", "lib.wasm", ""] {
        assert_eq!(
            entry(refused),
            Err(format!("be3.map stages no module named {refused:?}")),
        );
    }
    assert_eq!(
        crate::staged_app("be3.nowhere", "map.wasm").err(),
        Some("no staged app has the id be3.nowhere".to_owned())
    );
}
