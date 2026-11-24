use super::*;
use be_sdk_beui::be_block_content::{ARTIFACT, ARTIFACT_MADE, BlockContent};
use be_sdk_beui::{Artifact, Artifacts};
use be_type_image::ImageContent;

/// How many times the artifact instance `keeping` replaced the content of `image`.
fn remade(keeping: &BeuiTest<CanvasRoot>, image: Uuid) -> usize {
    keeping
        .seeded()
        .into_iter()
        .filter(|seeded| seeded.block == image && seeded.replace)
        .count()
}

#[test]
fn an_artifact_made_again_writes_only_its_content_until_it_is_unlinked() {
    let card = card();
    let mut editor = editor(std::slice::from_ref(&card));
    let canvas = editor.block_id().expect("the editor has a block");
    let mut dialog = export_dialog(&mut editor, "png");
    dialog.click("export.workspace");
    dialog.run();
    let store = dialog.store();
    let image = store
        .created()
        .into_iter()
        .filter_map(|block| store.block(block))
        .find(|info| info.block_type == ImageContent::CONTENT_TYPE)
        .expect("exporting to the workspace makes an image block");
    let artifact = image.metadata.get(&ARTIFACT).expect("an artifact");
    let made = dialog.content::<ImageContent>(Some(image.id));

    let mut keeping = BeuiTest::<CanvasRoot>::settings(
        Artifacts::new(
            EditorHost::default(),
            Artifact {
                block_id: image.id,
                block_type: ImageContent::CONTENT_TYPE,
                source: canvas,
                exporter: "png".to_owned(),
            },
        ),
        artifact.settings.clone(),
    );
    hold_canvas(&editor, &mut keeping, canvas);
    keeping.store().add_block(image.clone());
    keeping.hold(Some(image.id), made.clone());
    keeping.run();
    assert_eq!(
        remade(&keeping, image.id),
        0,
        "what was made already is not written again"
    );

    // Applied in the settings dialog: the host stores the new settings, then asks for a remake,
    // which may run before what the instance reads of the block says so.
    keeping.regenerate(br#"{"scale":2.0,"background":false,"items":[]}"#.to_vec());
    assert_eq!(remade(&keeping, image.id), 1, "new settings make a new image");
    assert!(
        keeping.take_definitions(ARTIFACT.id).is_empty(),
        "making it again never writes what it is made from, which the person sets"
    );
    assert_eq!(
        keeping.take_definitions(ARTIFACT_MADE.id).len(),
        1,
        "it notes what it made"
    );

    // A canvas with nothing on it makes no image: the failure stands until it makes one again.
    let emptied = Canvas::edit_in(
        &keeping.content::<CanvasContent>(Some(canvas)),
        &InfiniteCanvasOperation::Remove {
            ids: vec![card.id],
        },
    );
    keeping.edit::<CanvasContent>(Some(canvas), &emptied);
    keeping.run();
    assert_eq!(
        keeping.take_artifact_failures(),
        vec![Some("There is nothing to export.".to_owned())],
        "a remake that fails as the source changes says why, though no one asked for it"
    );
    let restored = Canvas::edit_in(
        &keeping.content::<CanvasContent>(Some(canvas)),
        &InfiniteCanvasOperation::Add {
            entity: Box::new(card.clone()),
            pixels: None,
        },
    );
    keeping.edit::<CanvasContent>(Some(canvas), &restored);
    keeping.run();
    assert_eq!(keeping.take_artifact_failures(), vec![None]);
    let before = remade(&keeping, image.id);
    let mut unlinked = keeping.store().block(image.id).expect("the image is known");
    unlinked.metadata.remove(&ARTIFACT);
    keeping.store().add_block(unlinked);
    keeping.run();
    let mut recoloured = card.clone();
    recoloured.style.fill_color = CanvasColor::Rgba {
        red: 200,
        green: 40,
        blue: 40,
        alpha: 255,
    };
    let edit = Canvas::edit_in(
        &keeping.content::<CanvasContent>(Some(canvas)),
        &InfiniteCanvasOperation::Update {
            entities: vec![recoloured],
        },
    );
    keeping.edit::<CanvasContent>(Some(canvas), &edit);
    keeping.run();
    assert_eq!(
        remade(&keeping, image.id),
        before,
        "an unlinked block keeps what was made, whatever its source does"
    );
    assert!(keeping.take_definitions(ARTIFACT.id).is_empty());
}
