use super::*;

/// Whether the exporter's settings `draft` say to rename the component with its grid: the
/// default, when it says nothing.
fn renames(draft: &[u8]) -> bool {
    draft.is_empty()
        || serde_json::from_slice::<serde_json::Value>(draft).expect("the settings are JSON")
            ["rename_with_source"]
            == serde_json::Value::Bool(true)
}

#[test]
fn the_rename_setting_writes_the_artifact_draft() {
    let source = Uuid::new_v4();
    let artifacts = Artifacts::new(
        EditorHost::default(),
        Artifact {
            block_id: Uuid::new_v4(),
            block_type: CompiledLogicContent::CONTENT_TYPE,
            source,
            exporter: "component".to_owned(),
        },
    );
    let mut settings = BeuiTest::<LogicGridRoot>::settings(artifacts, Vec::new());
    assert!(
        renames(settings.draft()),
        "a compiled component is named after its grid by default"
    );

    settings.click("logic-grid.rename-with-source");

    assert!(
        !renames(settings.draft()),
        "unticked, the draft keeps the component's own name"
    );
}
