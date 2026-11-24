use super::*;

#[test]
fn the_wire_round_trips() {
    let commit = "c".repeat(40);
    round_trips(
        Status {
            product: "android".to_owned(),
            commit: commit.clone(),
            state: State::Queued,
            position: Some(2),
            manifest: None,
            error: None,
        },
        json!({"product": "android", "commit": commit, "state": "queued", "position": 2}),
    );
    round_trips(
        Status {
            product: "android".to_owned(),
            commit: commit.clone(),
            state: State::Succeeded,
            position: None,
            manifest: Some("https://cdn/builds/android/c.json".to_owned()),
            error: None,
        },
        json!({
            "product": "android",
            "commit": commit,
            "state": "succeeded",
            "manifest": "https://cdn/builds/android/c.json",
        }),
    );
    for state in [State::Queued, State::Running, State::Succeeded, State::Failed] {
        assert_eq!(serde_json::to_value(state).unwrap(), state.name());
    }
    round_trips(Refusal::default(), json!({}));
    round_trips(
        Refusal {
            error: Some("not on a branch".to_owned()),
        },
        json!({"error": "not on a branch"}),
    );
    round_trips(
        Manifest {
            product: "native-linux_x86_64".to_owned(),
            commit: commit.clone(),
            files: vec![ManifestFile {
                path: "bin/be-host".to_owned(),
                sha256: "a".repeat(64),
                size: 3,
                executable: true,
            }],
        },
        json!({
            "product": "native-linux_x86_64",
            "commit": commit,
            "files": [{"path": "bin/be-host", "sha256": "a".repeat(64), "size": 3, "executable": true}],
        }),
    );
}
