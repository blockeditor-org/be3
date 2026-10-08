use super::*;

use be_block::ChecklistItem;
use be_block::be_model::Anchor;

#[test]
fn a_snapshot_carries_where_removed_items_were() {
    let host = EditorHost::default();
    let mut content = ChecklistContent::default();
    let mut removed = ObjectId::ROOT;
    for text in ["milk", "eggs", "bread"] {
        let (id, add) = Checklist::add(text);
        if text == "eggs" {
            removed = id;
        }
        content.apply(&add);
    }
    content.apply(&Checklist::remove(removed));
    host.set_block_content(HostContent::of(&content));
    let projection = ContentProjection::<ChecklistContent>::new(host.clone(), None);
    projection.pump();
    let (_, insert) = Checklist::ITEMS.insert(
        ObjectId::ROOT,
        Anchor::After(removed),
        &ChecklistItem {
            text: "flour".into(),
            done: false,
        },
    );

    host.push_content_operations(vec![(
        ChecklistContent::encode_operation(&insert.into()),
        false,
    )]);
    projection.pump();

    let texts = projection
        .read(|content| {
            content
                .root()
                .items
                .iter()
                .map(|item| item.text.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert_eq!(texts, ["milk", "flour", "bread"]);
}
