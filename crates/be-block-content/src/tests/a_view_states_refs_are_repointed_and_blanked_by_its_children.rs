use crate::{ChildChange, EditorView, LiveEdit, Root, ViewRef, ViewStateKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Tabs {
    shown: Vec<ViewRef>,
    focused: Option<ViewRef>,
    scrolled: u32,
}

impl ViewStateKind for Tabs {
    const ID: Uuid = Uuid::from_u128(1);
}

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}

#[test]
fn a_view_states_refs_are_repointed_and_blanked_by_its_children() {
    let (content, first, second, copy) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let start = EditorView::document(Some(content));
    let tabs = Tabs {
        shown: vec![ViewRef::new(first), ViewRef::new(second)],
        focused: Some(ViewRef::new(second)),
        scrolled: 7,
    };
    let view = edited(
        &start,
        start
            .root()
            .set_view_state(Some(&tabs), 10, Uuid::new_v4())
            .expect("encodes"),
    );
    assert_eq!(view.root().view_state::<Tabs>(), Some(Ok(tabs)));
    assert_eq!(
        view.root().stored(Tabs::ID).expect("kept").refs,
        [first, second],
        "a block named twice is one ref"
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

    let read = deleted
        .root()
        .view_state::<Tabs>()
        .expect("still kept")
        .expect("still readable");
    assert_eq!(
        read.shown
            .iter()
            .map(|shown| shown.block())
            .collect::<Vec<_>>(),
        [Some(copy), None]
    );
    assert_eq!(read.focused.and_then(ViewRef::block), None);
    assert_eq!(read.scrolled, 7);
    assert_eq!(deleted.root().references().len(), 2);
    assert!(deleted.root().state.stamp(&Tabs::ID).time() > 10);
}
