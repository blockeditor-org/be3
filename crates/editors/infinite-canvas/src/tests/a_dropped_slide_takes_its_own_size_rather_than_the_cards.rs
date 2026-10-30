use super::*;
use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Pos2;
use block_editor_beui::{
    ChildStatus, EditorCapabilities, InteractionMode, ResizeMode, Size, ViewId,
};

#[test]
fn a_dropped_slide_takes_its_own_size_rather_than_the_cards() {
    let slide = Uuid::new_v4();
    let mut editor = open(
        &Canvas::default(),
        false,
        &[BlockInfo::new(
            slide,
            CanvasContent::CONTENT_TYPE,
            BlockParent::Root,
        )],
    );
    editor.drag_block(
        Pos2::new(300.0, 300.0),
        slide,
        CanvasContent::CONTENT_TYPE,
        true,
    );
    editor.run();
    let [placeholder] = entities(&editor)
        .try_into()
        .expect("the drop added one card");
    let report = |editor: &mut BeuiTest<CanvasApp>| {
        editor.report_children(|placement| ChildStatus {
            instance: block_editor_beui::EditorInstanceId(0),
            view: ViewId::MAIN,
            child: placement.child,
            available: true,
            intrinsic: Some(Size {
                width: 960.0,
                height: 540.0,
            }),
            aspect_ratio: None,
            active: false,
            interaction: InteractionMode::Live,
            capabilities: EditorCapabilities::default(),
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
    report(&mut editor);
    report(&mut editor);

    let [card] = entities(&editor)
        .try_into()
        .expect("the drop added one card");
    assert_eq!(
        card.shape,
        CanvasShape::Editor,
        "the dropped block is a direct editor"
    );
    let scale = card.scale;
    let content = crate::geometry::direct_editor_layout(&card)
        .expect("a direct editor has a layout")
        .content
        .size();
    assert!(
        (content.x / scale - 960.0).abs() < 0.5 && (content.y / scale - 540.0).abs() < 0.5,
        "the card shows the slide at its own size, scaled: {content:?} at {scale}"
    );
    let assigned = editor.children()[0].intrinsic;
    assert!(
        assigned.is_none_or(
            |size| (size.width - 960.0).abs() < 0.5 && (size.height - 540.0).abs() < 0.5
        ),
        "the card would resize the slide to {assigned:?}"
    );
    editor.snapshot("a_dropped_slide_takes_its_own_size_rather_than_the_cards");

    apply(
        &mut editor,
        InfiniteCanvasOperation::Update {
            entities: vec![placeholder.clone()],
        },
    );
    report(&mut editor);
    report(&mut editor);

    assert_eq!(
        entities(&editor),
        [placeholder],
        "undoing the fit leaves the placeholder rather than fitting it again"
    );
    assert_eq!(
        editor.children()[0].intrinsic,
        None,
        "a card back at its placeholder does not resize the slide to it"
    );
}
