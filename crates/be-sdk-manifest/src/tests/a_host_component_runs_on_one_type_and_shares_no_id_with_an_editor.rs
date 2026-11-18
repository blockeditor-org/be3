use super::*;

/// A game's levels: Join, Host and Manage.
const LEVELS: &str = r#""levels": [
    { "access": "join", "name": "Join game", "line": "Connect to whoever hosts the world." },
    { "access": "edit", "name": "Host", "line": "Can run the world, and so read and change anything in it." },
    { "access": "manage", "name": "Manage", "line": "Can also share it." }
]"#;

fn with_hosts(hosts: &str) -> Result<AppManifest, ManifestError> {
    with_raw_hosts(&hosts.replace("LEVELS", LEVELS))
}

fn with_raw_hosts(hosts: &str) -> Result<AppManifest, ManifestError> {
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
            "block_types": {{
                "world": {{
                    "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000",
                    "display_name": "World",
                    "icon": "\ue0ef",
                    "module": "hosted_types.wasm"
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
            "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000", LEVELS
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
            levels: vec![
                OfferedLevel {
                    access: OfferedAccess::Join,
                    name: "Join game".into(),
                    line: "Connect to whoever hosts the world.".into(),
                },
                OfferedLevel {
                    access: OfferedAccess::Edit,
                    name: "Host".into(),
                    line: "Can run the world, and so read and change anything in it.".into(),
                },
                OfferedLevel {
                    access: OfferedAccess::Manage,
                    name: "Manage".into(),
                    line: "Can also share it.".into(),
                },
            ],
        }]
    );
    assert_eq!(
        with_hosts(
            r#"{ "world-host": {
                "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6071",
                "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000", LEVELS
            } }"#
        ),
        Err(ManifestError::DuplicateHost),
        "a host with an editor's id"
    );
    assert_eq!(
        with_hosts(
            r#"{
                "one": { "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6072", "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000", LEVELS },
                "two": { "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6073", "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000", LEVELS }
            }"#
        ),
        Err(ManifestError::DuplicateHost),
        "two hosts on one type"
    );
    for (levels, why) in [
        (
            r#"[{ "access": "edit", "name": "Host", "line": "Runs it." }]"#,
            "levels without Join",
        ),
        (
            r#"[{ "access": "join", "name": "Join", "line": "Plays." }, { "access": "join", "name": "Watch", "line": "Watches." }]"#,
            "a level twice",
        ),
    ] {
        assert_eq!(
            with_raw_hosts(&format!(
                r#"{{ "world-host": {{
                    "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6072",
                    "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6000",
                    "levels": {levels}
                }} }}"#
            )),
            Err(ManifestError::InvalidLevels),
            "{why}"
        );
    }
    assert_eq!(
        with_hosts(
            r#"{ "world-host": {
                "id": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6072",
                "edits": "8a7f3b5c-1d2e-4f60-9a1b-2c3d4e5f6999", LEVELS
            } }"#
        ),
        Err(ManifestError::HostOfAnotherType),
        "a host on a type its app does not declare, which could relabel that type's levels"
    );
}
