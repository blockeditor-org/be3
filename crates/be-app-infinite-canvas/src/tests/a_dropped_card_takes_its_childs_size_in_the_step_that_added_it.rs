use super::*;
use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::beui::Pos2;
use be_sdk_beui::{ChildStatus, EditorCapabilities, InteractionMode, ResizeMode, Vec2, ViewId};

pub(super) fn report(editor: &mut BeuiTest<CanvasRoot>, size: Option<Vec2>) {
    editor.report_children(|placement| ChildStatus {
        instance: be_sdk_beui::EditorInstanceId(0),
        view: ViewId::MAIN,
        child: placement.child,
        available: true,
        intrinsic: size,
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
        title: None,
        icon: None,
        group: None,
        raised: 0,
        rung: 0,
        minimized: 0,
        fullscreen: false,
        notice: None,
        menu_asked: None,
    });
    editor.run();
}

/// A canvas with a block dropped on it, whose editor has yet to tell its size.
pub(super) fn dropped() -> BeuiTest<CanvasRoot> {
    let block = Uuid::new_v4();
    let mut editor = open(
        &Canvas::default(),
        false,
        &[BlockInfo::new(
            block,
            CanvasContent::CONTENT_TYPE,
            BlockParent::Root,
        )],
    );
    editor.drag_block(
        Pos2::new(300.0, 300.0),
        block,
        CanvasContent::CONTENT_TYPE,
        true,
    );
    editor.run();
    report(&mut editor, None);
    editor
}

fn shows(card: &CanvasEntity) -> Vec2 {
    let content = crate::geometry::direct_editor_layout(card)
        .expect("a direct editor has a layout")
        .content
        .size();
    Vec2::new(content.x / card.scale, content.y / card.scale)
}

fn redo(editor: &mut BeuiTest<CanvasRoot>) {
    let block = editor.block_id().expect("the canvas has a block");
    editor.host().redo(block);
    editor.run();
}

#[test]
fn a_dropped_card_takes_its_childs_size_in_the_step_that_added_it() {
    let mut editor = dropped();
    let [placeholder] = entities(&editor)
        .try_into()
        .expect("the drop added one card");

    // The size arrives frames after the drop, once the child's editor has drawn.
    report(&mut editor, Some(Vec2::new(440.0, 480.0)));
    let [card] = entities(&editor)
        .try_into()
        .expect("the drop added one card");
    assert_ne!(card, placeholder, "the card is fitted to its child");
    let size = shows(&card);
    assert!(
        (size.x - 440.0).abs() < 0.5 && (size.y - 480.0).abs() < 0.5,
        "the card takes the size its child told: {size:?}"
    );

    undo(&mut editor);
    assert_eq!(
        entities(&editor),
        [],
        "one undo takes the card away, fit and all"
    );
    redo(&mut editor);
    assert_eq!(entities(&editor), [card.clone()], "redo brings it back fitted");

    report(&mut editor, Some(Vec2::new(600.0, 200.0)));
    assert_eq!(
        entities(&editor),
        [card],
        "a size told after the card was fitted leaves it as it is"
    );
    undo(&mut editor);
    assert_eq!(entities(&editor), [], "and adds no undo step");
}
