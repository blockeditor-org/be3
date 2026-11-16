use super::*;

fn with_hosts(hosts: &str) -> Result<AppManifest, ManifestError> {
    manifest_from_json(&format!(
        r#"{{
            "id": "test.hosted",
            "name": "Hosted",
            "version": "0.1.0",
            "editors": {{
                "world": {{
                    "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6071",
                    "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000",
                    "display_name": "World",
                    "icon": "",
                    "views": ["Main"]
                }}
            }},
            "hosts": {hosts},
            "entry_point": "hosted.wasm"
        }}"#
    ))
}

#[test]
fn a_host_component_runs_on_one_type_and_shares_no_id_with_an_editor() {
    let manifest = with_hosts(
        r#"{ "world-host": {
            "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6072",
            "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000"
        } }"#,
    )
    .unwrap();
    assert_eq!(
        manifest.hosts,
        [HostManifest {
            id: uuid::Uuid::parse_str("8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6072")
                .unwrap()
                .into_bytes(),
            name: "world-host".into(),
            edits: uuid::Uuid::parse_str("8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000")
                .unwrap()
                .into_bytes(),
        }]
    );
    assert_eq!(
        with_hosts(
            r#"{ "world-host": {
                "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6071",
                "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000"
            } }"#
        ),
        Err(ManifestError::DuplicateHost),
        "a host with an editor's id"
    );
    assert_eq!(
        with_hosts(
            r#"{
                "one": { "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6072", "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000" },
                "two": { "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6073", "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000" }
            }"#
        ),
        Err(ManifestError::DuplicateHost),
        "two hosts on one type"
    );
}
