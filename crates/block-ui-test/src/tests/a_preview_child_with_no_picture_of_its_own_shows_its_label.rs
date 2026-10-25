use super::*;
use block_editor_beui::{
    ChildPreview, ChildStatus, EditorCapabilities, EditorRegion, InteractionMode, ResizeMode,
};

#[test]
fn a_preview_child_with_no_picture_of_its_own_shows_its_label() {
    let mut test = editor().with_size(beui::Vec2::new(360.0, 220.0));
    test.run();
    test.report_children(|placement| ChildStatus {
        instance: block_editor_beui::EditorInstanceId(0),
        region: EditorRegion::Frame,
        child: placement.child,
        available: true,
        intrinsic: None,
        aspect_ratio: None,
        active: false,
        interaction: InteractionMode::Preview,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::None,
        error: None,
        menu: Vec::new(),
        creation: None,
        settings: None,
        preview: ChildPreview::Label,
    });
    test.run();
    assert_eq!(status(&test), "available");
    test.snapshot("a_preview_child_with_no_picture_of_its_own_shows_its_label");
}
