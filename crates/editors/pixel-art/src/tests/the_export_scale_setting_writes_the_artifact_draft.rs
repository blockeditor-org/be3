use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn the_export_scale_setting_writes_the_artifact_draft() {
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let artifacts = Artifacts::new(
        EditorHost::default(),
        Artifact {
            block_id: target,
            block_type: ImageContent::CONTENT_TYPE,
            source,
        },
    );
    let data = artifact::descriptor(source).settings;
    let mut settings = BeuiTest::<PixelArtApp>::settings(artifacts, data);

    assert_eq!(
        settings.label("pixel-art.export-summary"),
        "PNG export at the original size"
    );

    settings.click("pixel-art.export-scale");
    settings.type_text("3");
    settings.key(Key::Enter, Modifiers::NONE);

    assert_eq!(
        artifact::describe(settings.draft()).unwrap(),
        "PNG export at 3x"
    );
}
