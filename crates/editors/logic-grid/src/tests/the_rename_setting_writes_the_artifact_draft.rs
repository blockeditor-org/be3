use super::*;

#[test]
fn the_rename_setting_writes_the_artifact_draft() {
    let source = Uuid::new_v4();
    let artifacts = Artifacts::new(
        EditorHost::default(),
        Artifact {
            block_id: Uuid::new_v4(),
            block_type: CompiledLogicContent::CONTENT_TYPE,
        },
    );
    let data = crate::app::descriptor_data(source);
    let mut settings = BeuiTest::<LogicGridApp>::settings(artifacts, data);
    assert_eq!(
        crate::app::artifact_summary(settings.draft()),
        "Compiled component, named after its grid"
    );

    settings.click("logic-grid.rename-with-source");
    settings.run();

    assert_eq!(
        crate::app::artifact_summary(settings.draft()),
        "Compiled component"
    );
}
