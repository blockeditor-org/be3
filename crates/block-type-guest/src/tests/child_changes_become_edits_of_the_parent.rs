use super::*;
use block_type_api::{ChildChange, ReplaceChild};

#[test]
fn child_changes_become_edits_of_the_parent() {
    let mut sim = Sim::<FolderContent>::new(2, folder_module);
    sim.open(0);
    sim.open(1);
    let child = Uuid::from_u128(0xc41d);
    let other = Uuid::from_u128(0x07e5);
    assert!(sim.change(1, Change::Child(ChildChange::Add(child))));
    assert_eq!(sim.content(0).references(), [child]);
    assert_eq!(
        sim.devices[1]
            .described
            .as_ref()
            .map(|description| description.references.clone()),
        Some(vec![child])
    );
    sim.change(
        0,
        Change::Child(ChildChange::Replace(ReplaceChild {
            old: child,
            new: other,
        })),
    );
    for device in 0..2 {
        assert_eq!(sim.content(device).references(), [other]);
    }
    sim.change(0, Change::Child(ChildChange::Delete(other)));
    sim.advance(crate::SEAL_INTERVAL);
    assert!(sim.head_content().references().is_empty());
}
