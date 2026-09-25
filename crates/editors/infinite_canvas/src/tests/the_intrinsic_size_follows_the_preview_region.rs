use super::*;
use block_editor_plugin::beui::Vec2;

#[test]
fn the_intrinsic_size_follows_the_preview_region() {
    let mut editor = editor(&[]);
    apply(
        &mut editor,
        InfiniteCanvasOperation::SetPreviewRegion {
            region: Some(CanvasPreviewRegion::new(
                CanvasPoint::default(),
                CanvasPoint::new(960.0, 540.0),
            )),
        },
    );
    editor.run();

    assert_eq!(editor.intrinsic_size(), Some(Vec2::new(960.0, 540.0)));

    editor.resize(Vec2::new(480.0, 270.0));
    editor.run();

    assert_eq!(editor.intrinsic_size(), Some(Vec2::new(480.0, 270.0)));
}
