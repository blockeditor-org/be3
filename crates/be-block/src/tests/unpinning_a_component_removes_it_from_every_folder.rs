use super::*;
use crate::hotbar::{Hotbar, HotbarContent, HotbarSlot};
use crate::{ChildChange, Root};
use uuid::Uuid;

fn pinned(hotbar: &HotbarContent) -> Vec<Option<Uuid>> {
    let mut found = Vec::new();
    for slot in hotbar.root().slots.iter() {
        found.push(slot.compiled());
        found.extend(slot.slots.iter().map(|inner| inner.compiled()));
    }
    found
}

#[test]
fn unpinning_a_component_removes_it_from_every_folder() {
    let (adder, latch, counter) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut hotbar = HotbarContent::new(&Hotbar {
        slots: [
            HotbarSlot::component("Adder", adder),
            HotbarSlot::folder(
                "Memory",
                [
                    HotbarSlot::component("Latch", latch),
                    HotbarSlot::component("Adder", adder),
                ],
            ),
        ]
        .into_iter()
        .collect(),
    });
    assert_eq!(BlockContent::references(&hotbar), [adder, latch]);

    let edit = hotbar.root().unpin(adder);
    hotbar.apply(&edit);
    assert_eq!(pinned(&hotbar), [None, Some(latch)]);

    let edit = hotbar
        .root()
        .child_edit(ChildChange::Replace {
            old: latch,
            new: counter,
        })
        .expect("a hotbar follows its components");
    hotbar.apply(&edit);
    assert_eq!(pinned(&hotbar), [None, Some(counter)]);

    let edit = hotbar.root().replace_all([
        HotbarSlot::component("Adder", adder),
        HotbarSlot::folder("Empty", []),
    ]);
    hotbar.apply(&edit);
    assert_eq!(pinned(&hotbar), [Some(adder), None]);
}
