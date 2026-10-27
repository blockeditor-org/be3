use super::*;
use block_editor_beui::beui::icons::ICON_SHAPES;
use block_editor_beui::{
    BlockInfo, BlockParent, BlockQuery, BlockTypeDescriptor, ChildMode, ChildOperations,
    ChildPreview, ChildStatus, EditorCapabilities, EditorInstanceId, EditorRegion, InteractionMode,
    Press, ResizeMode,
};

const CANVAS: Uuid = Uuid::from_u128(0x0c);

#[test]
fn a_slide_with_no_picture_of_its_own_shows_its_name_in_the_filmstrip() {
    let block = Uuid::new_v4();
    let slides = [Uuid::from_u128(0x51), Uuid::from_u128(0x52)];
    let mut content = PresentationContent::default();
    for (index, slide) in slides.into_iter().enumerate() {
        let edit = content.root().insert(ObjectId::new(), index, slide);
        content.apply(&edit);
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let infos: Vec<BlockInfo> = slides
        .into_iter()
        .zip(["Opening", "Agenda"])
        .map(|(slide, name)| {
            let mut info = BlockInfo::new(slide, CANVAS, BlockParent::Block(block));
            info.set_intrinsic_name(name);
            info
        })
        .collect();
    let mut test = Harness::new(OwnedEditor::new(host.clone(), block));
    test.hold(None, content);
    test.block_types(vec![BlockTypeDescriptor {
        block_type: CANVAS.into_bytes(),
        display_name: "Canvas".to_owned(),
        color: None,
        icon_codepoint: ICON_SHAPES.as_str().to_owned(),
        children: ChildOperations::default(),
        press: Press::Open,
        plain_file: false,
    }]);
    host.set_blocks(BlockQuery::References(block), infos.clone());
    test.run();
    test.report_children(|placement| ChildStatus {
        instance: EditorInstanceId(0),
        region: EditorRegion::Frame,
        child: placement.child,
        available: true,
        intrinsic: None,
        aspect_ratio: None,
        active: false,
        interaction: InteractionMode::Live,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::None,
        error: None,
        menu: Vec::new(),
        creation: None,
        preview: match placement.mode {
            ChildMode::Preview => ChildPreview::Label,
            _ => ChildPreview::Hosted,
        },
        dialog: Default::default(),
        picked: None,
    });
    test.run();
    for info in infos {
        host.set_blocks(BlockQuery::Block(info.id), vec![info]);
    }
    test.run();
    test.snapshot("a_slide_with_no_picture_of_its_own_shows_its_name_in_the_filmstrip");
}
