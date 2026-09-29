use super::*;
use crate::editor_view::{EditorView, ViewState};
use crate::{ChildChange, Root};
use uuid::Uuid;

#[test]
fn a_deleted_child_of_a_view_is_blanked_in_the_state_that_referenced_it() {
    let (content, first, second, copy) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let start = EditorView::document(Uuid::new_v4(), Some(content));
    let client = Uuid::new_v4();
    let view = edited(
        &start,
        [start.root().set_state(
            "layout",
            Some(&ViewState::new(&7u32, vec![first, second])),
            10,
            client,
        )],
    );
    assert_eq!(view.root().references().len(), 3);

    let replaced = edited(
        &view,
        view.root().child_edit(ChildChange::Replace {
            old: first,
            new: copy,
        }),
    );
    let deleted = edited(
        &replaced,
        replaced.root().child_edit(ChildChange::Delete(second)),
    );

    let layout = deleted.root().state("layout").cloned().expect("still held");
    assert_eq!(layout.refs, [copy, Uuid::nil()], "positions are kept");
    assert_eq!(layout.value::<u32>(), Some(7));
    assert_eq!(deleted.root().references().len(), 2);
    assert!(deleted.root().state.stamp(&"layout".to_owned()).time > 10);
}
