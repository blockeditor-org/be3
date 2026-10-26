use super::*;

#[test]
fn a_manifest_grants_only_extensions_the_host_knows() {
    let document = r#"{
        "id": "be3.desktop",
        "name": "Desktop",
        "version": "0.1.0",
        "editors": {
            "desktop": {
                "id": "378d6c68-d52a-4781-addb-fde9a8aff2e2",
                "display_name": "Desktop",
                "icon": "x",
                "regions": ["Frame"]
            }
        },
        "entry_point": "desktop.wasm",
        "extensions": EXTENSIONS
    }"#;
    let read = |extensions: &str| manifest_from_json(&document.replace("EXTENSIONS", extensions));

    let granted = read(r#"["be3:desktop"]"#).unwrap();
    assert!(granted.grants("be3:desktop"));
    assert_eq!(
        extension_of("be3:desktop/power@0.1.0"),
        Some("be3:desktop")
    );
    assert_eq!(
        extension_of("be3:plugin/account@0.1.0"),
        None,
        "core is no extension"
    );
    assert!(!read("[]").unwrap().grants("be3:desktop"));

    for refused in [
        r#"["be3:plugin"]"#,
        r#"["be3:desktop", "be3:desktop"]"#,
    ] {
        assert_eq!(
            read(refused),
            Err(ManifestError::UnknownExtension),
            "{refused}"
        );
    }
}
