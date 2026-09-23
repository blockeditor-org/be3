use super::*;

#[test]
fn the_rename_setting_writes_the_artifact_draft() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let source = client.create_block(LogicGrid::new());
    let artifacts = Artifacts::new(
        EditorHost::default(),
        Arc::clone(&client),
        Artifact {
            block_id: Uuid::new_v4(),
            block_type: CompiledLogic::TYPE_ID,
        },
    );
    let data = crate::app::descriptor_data(source.id());
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
