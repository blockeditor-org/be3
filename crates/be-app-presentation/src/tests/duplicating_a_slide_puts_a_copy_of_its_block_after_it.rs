use super::*;
use be_sdk_beui::beui::Modifiers;
use be_sdk_beui::{BlockInfo, BlockLocation, BlockParent, BlockQuery, ImportedBlock};
use be_sdk_test::HostCall;
use be_type_presentation::Presentation;

const CANVAS: Uuid = Uuid::from_u128(0x0c);
const TEXT: Uuid = crate::root::TEXT_TYPE;

fn copied(test: &mut Harness, deck: Uuid, source: Uuid, copy: Uuid, block_type: Uuid) {
    let asked: Vec<_> = test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::CopyBlocks {
                reply,
                source,
                parent,
            } => Some((reply, Uuid::from_bytes(source), parent)),
            _ => None,
        })
        .collect();
    let [(reply, asked, parent)]: [_; 1] = asked
        .try_into()
        .unwrap_or_else(|asked: Vec<_>| panic!("duplicating asks for one copy, asked {asked:?}"));
    assert_eq!(asked, source, "the slide's own block is copied");
    assert_eq!(
        parent,
        BlockLocation::Block(deck.into_bytes()),
        "into the deck"
    );
    test.answer(
        reply,
        Ok(Some(ImportedBlock {
            block_id: copy.into_bytes(),
            block_type: block_type.into_bytes(),
        })),
    );
    test.run();
}

fn notes(test: &Harness) -> Vec<Option<Uuid>> {
    test.content::<PresentationContent>(None)
        .root()
        .slides
        .iter()
        .map(|slide| slide.notes)
        .collect()
}

fn blocks(test: &Harness) -> Vec<Option<Uuid>> {
    test.content::<PresentationContent>(None)
        .root()
        .slides
        .iter()
        .map(|slide| slide.block)
        .collect()
}

#[test]
fn duplicating_a_slide_puts_a_copy_of_its_block_after_it() {
    let deck = Uuid::new_v4();
    let [canvas, text, canvas_copy, text_copy] = [0x51, 0x52, 0x53, 0x54].map(Uuid::from_u128);
    let [written, written_copy, late_copy] = [0x61, 0x62, 0x63].map(Uuid::from_u128);
    let mut content = PresentationContent::default();
    for (index, slide) in [canvas, text].into_iter().enumerate() {
        let edit = content.root().insert(ObjectId::new(), index, slide);
        content.apply(&edit);
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = Harness::new(OwnedEditor::new(host.clone(), deck));
    test.hold(None, content);
    let info = |id: Uuid, block_type: Uuid, name: &str| {
        let mut info = BlockInfo::new(id, block_type, BlockParent::Block(deck));
        info.set_intrinsic_name(name);
        info
    };
    host.set_blocks(
        BlockQuery::References(deck),
        vec![info(canvas, CANVAS, "Opening"), info(text, TEXT, "Agenda")],
    );
    test.run();
    test.take_requests();

    test.key(Key::D, Modifiers::CTRL);
    test.run();
    copied(&mut test, deck, canvas, canvas_copy, CANVAS);
    assert_eq!(
        blocks(&test),
        [canvas, canvas_copy, text].map(Some),
        "the copy is a new block after the original"
    );
    assert_eq!(detail(&test, "presentation.position"), "Slide 2 of 3");

    let ids = slide_ids(&test);
    test.edit::<PresentationContent>(
        None,
        &Presentation::set_notes(ObjectId::from_uuid(ids[2]), Some(written)),
    );
    test.run();
    test.click(format!("presentation.slide.{}.more", ids[2]));
    test.run();
    test.click(format!("presentation.slide.{}.duplicate", ids[2]));
    test.run();
    copied(&mut test, deck, text, text_copy, TEXT);
    assert_eq!(
        blocks(&test),
        [canvas, canvas_copy, text, text_copy].map(Some),
        "the tile's menu duplicates its own slide"
    );
    assert_eq!(detail(&test, "presentation.position"), "Slide 4 of 4");
    copied(&mut test, deck, written, written_copy, TEXT);
    assert_eq!(
        notes(&test),
        [None, None, Some(written), Some(written_copy)],
        "the copy has a copy of the slide's notes of its own"
    );

    let ids = slide_ids(&test);
    test.click(format!("presentation.slide.{}", ids[0]));
    test.run();
    test.key(Key::D, Modifiers::CTRL);
    test.run();
    test.click(format!("presentation.slide.{}", ids[3]));
    test.run();
    copied(&mut test, deck, canvas, late_copy, CANVAS);
    assert_eq!(slide_ids(&test).len(), 5);
    assert_eq!(
        detail(&test, "presentation.position"),
        "Slide 5 of 5",
        "a copy that lands after the selection moved on leaves the selection alone"
    );

    host.set_blocks(
        BlockQuery::References(deck),
        vec![
            info(canvas, CANVAS, "Opening"),
            info(canvas_copy, CANVAS, "Opening"),
            info(text, TEXT, "Agenda"),
            info(text_copy, TEXT, "Agenda"),
            info(late_copy, CANVAS, "Opening"),
        ],
    );
    test.run();
    test.available_children();
    test.settle();
    test.snapshot("duplicating_a_slide_puts_a_copy_of_its_block_after_it");
}
