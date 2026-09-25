use super::*;

#[test]
fn the_export_scale_setting_writes_the_artifact_draft() {
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let artifacts = Artifacts::new(
        EditorHost::default(),
        Artifact {
            block_id: target,
            block_type: ImageContent::CONTENT_TYPE,
        },
    );
    let data = artifact::descriptor(source).data;
    let mut settings = BeuiTest::<PixelArtApp>::settings(artifacts, data);

    assert_eq!(
        settings.label("pixel-art.export-summary"),
        "PNG export at the original size"
    );

    settings.click("pixel-art.export-scale");
    settings.run();
    settings.key_press_modifiers(Modifiers::CTRL, Key::A);
    settings.text("3");
    settings.run();

    assert_eq!(
        artifact::describe(settings.draft()).unwrap().summary,
        "PNG export at 3x"
    );
}
