use crate::{EditorView, LiveEdit, ViewStateKind};
use be_block_model::Edit;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Scrolled(u32);

impl ViewStateKind for Scrolled {
    const ID: Uuid = Uuid::from_u128(1);
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Caret(u32);

impl ViewStateKind for Caret {
    const ID: Uuid = Uuid::from_u128(2);
}

/// Writes are folded into the unsent operation before them (`LiveEdit::absorb_operation`), as
/// the replica's outbox does; a burst of writes to one kind leaves only the last.
#[test]
fn a_burst_of_view_state_writes_is_one_change() {
    let client = Uuid::new_v4();
    let mut view = EditorView::document(None);
    let mut unsent: Option<Edit> = None;
    for line in 0..200u32 {
        let edits = [
            view.root()
                .set_view_state(Some(&Scrolled(line)), u64::from(line), client),
            view.root()
                .set_view_state(Some(&Caret(line / 2)), u64::from(line), client),
        ];
        for edit in edits.into_iter().flatten() {
            view.apply(&edit);
            match &mut unsent {
                Some(held) => assert!(
                    <crate::EditorViewContent as LiveEdit>::absorb_operation(held, edit).is_none()
                ),
                None => unsent = Some(edit),
            }
        }
    }
    let unsent = unsent.expect("written");
    assert_eq!(unsent.0.len(), 2, "one put for each kind");

    let mut elsewhere = EditorView::document(None);
    elsewhere.apply(&unsent);
    assert_eq!(
        elsewhere.root().view_state::<Scrolled>(),
        Some(Ok(Scrolled(199)))
    );
    assert_eq!(elsewhere.root().view_state::<Caret>(), Some(Ok(Caret(99))));
}
