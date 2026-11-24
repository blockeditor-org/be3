use super::*;
use be_sdk_beui::be_block_content::{ARTIFACT, BlockContent};
use be_sdk_beui::{Artifact, Artifacts, DialogAnswer, SavedFile};
use be_sdk_test::HostCall;
use be_type_image::ImageContent;

fn centre(png: &[u8]) -> [u8; 4] {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("the export is a readable PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    image.get_pixel(width / 2, height / 2).0
}

#[test]
fn a_png_exported_to_the_workspace_follows_the_canvas() {
    let card = card();
    let mut editor = editor(std::slice::from_ref(&card));
    let canvas = editor.block_id().expect("the editor has a block");

    let mut dialog = export_dialog(&mut editor, "png");
    dialog.click("export.workspace");
    dialog.run();
    assert_eq!(dialog.take_dialog_closes(), vec![DialogAnswer::Accepted]);
    let store = dialog.store();
    let made: Vec<BlockInfo> = store
        .created()
        .into_iter()
        .filter_map(|block| store.block(block))
        .filter(|info| info.block_type == ImageContent::CONTENT_TYPE)
        .collect();
    let [image] = made.as_slice() else {
        panic!("exporting to the workspace makes one image block, made {made:?}");
    };
    let artifact = image
        .metadata
        .get(&ARTIFACT)
        .expect("the image is an artifact of the canvas");
    assert_eq!(*artifact.source, canvas);
    assert_eq!(artifact.exporter, "png");
    let first = dialog.content::<ImageContent>(Some(image.id));
    assert_eq!(centre(first.data()), [60, 110, 90, 255]);

    let mut saving = export_dialog(&mut editor, "png");
    saving.click("export.save");
    saving.run();
    let saved: Vec<SavedFile> = saving
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::SaveFile { file, .. } => Some(file),
            _ => None,
        })
        .collect();
    let [file] = saved.as_slice() else {
        panic!(
            "saving asks the host to save one file, asked {}",
            saved.len()
        );
    };
    let form = store
        .file_of(ImageContent::CONTENT_TYPE, first.encode(), "")
        .expect("an image block has a file form");
    assert_eq!(
        file.data, form.data,
        "the file saved is the image block's own file form"
    );

    let artifacts = Artifacts::new(
        EditorHost::default(),
        Artifact {
            block_id: image.id,
            block_type: ImageContent::CONTENT_TYPE,
            source: canvas,
            exporter: "png".to_owned(),
        },
    );
    let mut keeping = BeuiTest::<CanvasRoot>::settings(artifacts, artifact.settings.clone());
    hold_canvas(&editor, &mut keeping, canvas);
    keeping.store().add_block(image.clone());
    keeping.hold(Some(image.id), first.clone());
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
    let written: Vec<ImageContent> = keeping
        .seeded()
        .into_iter()
        .filter(|seeded| seeded.block == image.id && seeded.replace)
        .map(|seeded| ImageContent::decode(&seeded.bytes).expect("an image's content"))
        .collect();
    let [followed] = written.as_slice() else {
        panic!(
            "the canvas changed once, so the image is made again once, made {}",
            written.len()
        );
    };
    assert_eq!(
        centre(followed.data()),
        [200, 40, 40, 255],
        "the image is made again as the canvas changes"
    );
}
