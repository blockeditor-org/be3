use super::*;

#[test]
fn the_export_scale_setting_writes_the_artifact_draft() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let source = client.create_block(PixelArt::new());
    let target = client.create_block(Image::new());
    let artifacts = Artifacts::new(
        EditorHost::default(),
        Arc::clone(&client),
        Artifact {
            block_id: target.id(),
            block_type: Image::TYPE_ID,
        },
    );
    let data = artifact::descriptor(source.id()).data;
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
