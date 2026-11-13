use super::*;
use be_sdk_beui::beui::clipboard::{CanvasItems, formats};
use be_sdk_beui::beui::{Input, Key, Modifiers};
use be_type_database::database::RowRef;

#[test]
fn a_canvas_copy_names_its_blocks_only_in_refs() {
    let [shown, schema, value, related, linked] =
        [0xa1, 0xa2, 0xa3, 0xa4, 0xa5].map(|n: u128| Uuid::from_u128(n << 64 | n));
    let row = Uuid::from_u128(0xb1);
    let mut copied = card();
    copied.block = Some(shown);
    copied.components.push(CanvasComponent {
        schema_id: schema,
        values: BTreeMap::from([
            (Uuid::from_u128(1), DatabaseValue::Block(value)),
            (
                Uuid::from_u128(2),
                DatabaseValue::Relation(RowRef {
                    database: related,
                    row,
                }),
            ),
            (
                Uuid::from_u128(3),
                DatabaseValue::Links(vec![RowRef {
                    database: linked,
                    row,
                }]),
            ),
        ]),
    });
    let mut source = editor(std::slice::from_ref(&copied));
    source.click(source.rect_of("infinite-canvas.canvas").center());
    source.run();
    source.key(Key::C, Modifiers::CTRL);
    source.run();
    let [item] = &source.take_copied(formats::CANVAS_ITEMS)[..] else {
        panic!("Copy puts the card on the clipboard as canvas items");
    };
    let item = item.clone();
    let items = item.decode::<CanvasItems>().expect("the canvas items decode");
    assert_eq!(items.blocks, [shown, schema, value, related, linked]);
    let json = String::from_utf8_lossy(&items.entities);
    for id in [shown, schema, value, related, linked] {
        assert!(
            !json.contains(&id.to_string()) && !item.data.windows(16).any(|w| w == id.as_bytes()),
            "the copied bytes hold no block id, only the refs do: {id} in {json}"
        );
    }
    assert!(json.contains(&row.to_string()), "a row id is not a block's and stays");

    let mut pasted = editor(&[]);
    pasted.click(pasted.rect_of("infinite-canvas.canvas").center());
    pasted.run();
    let mut translated = item.clone();
    let swap = [shown, schema, value, related, linked].map(|id| (id, Uuid::from_u128(id.as_u128() + 1)));
    for held in &mut translated.refs {
        if let Some((_, to)) = swap.iter().find(|(from, _)| from == held) {
            *held = *to;
        }
    }
    pasted.step(vec![Input::Paste(translated)]);
    pasted.run();
    let asked = pasted
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            be_sdk_test::HostCall::PasteBlocks { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("the paste asks the host about the block the card shows");
    pasted.answer(
        asked,
        Ok(vec![be_sdk_beui::PastedBlock {
            block_id: swap[0].1.into_bytes(),
            block_type: Uuid::from_u128(77).into_bytes(),
            moved: false,
            cut_from: None,
        }]),
    );
    pasted.run();
    let [placed] = &entities(&pasted)[..] else {
        panic!("the card is placed");
    };
    let mut expected = copied.clone();
    for block in expected.blocks_mut() {
        *block = swap.iter().find(|(from, _)| from == block).expect("named").1;
    }
    assert_eq!(placed.block, expected.block);
    assert_eq!(placed.components, expected.components, "every id comes from the refs");
}
