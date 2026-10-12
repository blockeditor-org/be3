use super::*;
use block_editor_beui::be_block::{BlockContent, CounterContent};
use block_editor_beui::beui::Pos2;
use block_editor_beui::{
    ChildMode, ChildStatus, EditorCapabilities, EditorRegion, InteractionMode, ResizeMode,
};

#[test]
fn clicking_a_live_editor_hands_it_the_frame() {
    let counter = Uuid::new_v4();
    let mut placed = entity(Uuid::from_u128(1));
    placed.kind = CanvasEntityKind::DirectEditor {
        block_id: counter,
        scale: 1.0,
    };
    placed.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(240.0, 180.0), 0.0);
    let mut editor = open(
        &Canvas::with_entities([placed]),
        false,
        &[BlockInfo::new(
            counter,
            CounterContent::CONTENT_TYPE,
            BlockParent::Detached,
        )],
    );
    let live = |editor: &mut BeuiTest<CanvasApp>| {
        editor.report_children(|placement| ChildStatus {
            instance: block_editor_beui::EditorInstanceId(0),
            region: EditorRegion::Frame,
            child: placement.child,
            available: true,
            intrinsic: None,
            aspect_ratio: None,
            active: false,
            interaction: InteractionMode::Live,
            capabilities: EditorCapabilities::default(),
            resize: ResizeMode::Both,
            error: None,
            menu: Vec::new(),
            creation: None,
            settings: None,
        });
        editor.run();
    };
    live(&mut editor);
    live(&mut editor);
    let mode = |editor: &BeuiTest<CanvasApp>| {
        let children = editor.children();
        assert_eq!(children.len(), 1, "the canvas places its one editor");
        children[0].mode
    };
    assert_ne!(mode(&editor), ChildMode::Active);

    let rect = editor.children()[0].rect;
    editor.click_at(Pos2::new(
        rect.x + rect.width / 2.0,
        rect.y + rect.height / 2.0,
    ));
    editor.run();
    live(&mut editor);

    assert_eq!(mode(&editor), ChildMode::Active);
}
