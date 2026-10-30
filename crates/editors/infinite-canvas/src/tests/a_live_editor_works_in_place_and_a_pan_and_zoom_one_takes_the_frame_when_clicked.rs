use super::*;
use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{
    ChildMode, ChildStatus, EditorCapabilities, InteractionMode, ResizeMode, ViewId,
};
use counter_types::CounterContent;

#[test]
fn a_live_editor_works_in_place_and_a_pan_and_zoom_one_takes_the_frame_when_clicked() {
    let counter = Uuid::new_v4();
    let mut placed = entity(Uuid::from_u128(1));
    placed.shape = CanvasShape::Editor;
    placed.block = Some(counter);
    placed.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(240.0, 180.0), 0.0);
    let mut editor = open(
        &Canvas::with_entities([placed]),
        false,
        &[BlockInfo::new(
            counter,
            CounterContent::CONTENT_TYPE,
            BlockParent::Root,
        )],
    );
    let live = |editor: &mut BeuiTest<CanvasApp>, pan_and_zoom: bool| {
        editor.report_children(|placement| ChildStatus {
            instance: block_editor_beui::EditorInstanceId(0),
            view: ViewId::MAIN,
            child: placement.child,
            available: true,
            intrinsic: None,
            aspect_ratio: None,
            active: false,
            interaction: InteractionMode::Live,
            capabilities: EditorCapabilities {
                pan_and_zoom,
                ..EditorCapabilities::default()
            },
            resize: ResizeMode::Both,
            error: None,
            menu: Vec::new(),
            creation: None,
            preview: Default::default(),
            dialog: Default::default(),
            picked: None,
            card: false,
        });
        editor.run();
    };
    live(&mut editor, false);
    live(&mut editor, false);
    let mode = |editor: &BeuiTest<CanvasApp>| {
        let children = editor.children();
        assert_eq!(children.len(), 1, "the canvas places its one editor");
        children[0].mode
    };
    assert_eq!(
        mode(&editor),
        ChildMode::InPlace,
        "an editor without a camera of its own takes input where it is"
    );

    live(&mut editor, true);
    live(&mut editor, true);
    assert_eq!(mode(&editor), ChildMode::Passive);
    let rect = editor.children()[0].rect;
    editor.click(rect.center());
    editor.run();
    live(&mut editor, true);

    assert_eq!(
        mode(&editor),
        ChildMode::Active,
        "clicking an editor with a camera hands it the frame"
    );
}
