use crate::{EditorView, ViewState, ViewStateKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Scrolled(u32);

impl ViewStateKind for Scrolled {
    const ID: Uuid = Uuid::from_u128(1);
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Zoomed(String);

impl ViewStateKind for Zoomed {
    const ID: Uuid = Uuid::from_u128(2);
}

#[test]
fn a_view_state_is_written_read_and_skipped_when_unchanged() {
    let client = Uuid::new_v4();
    let mut view = EditorView::document(None);
    assert_eq!(view.root().view_state::<Scrolled>(), None);

    let edit = view.root().set_view_state(Some(&Scrolled(3)), 1, client);
    view.apply(&edit.expect("encodes").expect("a change"));
    let edit = view
        .root()
        .set_view_state(Some(&Zoomed("docs".into())), 1, client);
    view.apply(&edit.expect("encodes").expect("a change"));
    assert_eq!(view.root().view_state::<Scrolled>(), Some(Ok(Scrolled(3))));
    assert_eq!(
        view.root().view_state::<Zoomed>(),
        Some(Ok(Zoomed("docs".into()))),
        "each kind is its own entry"
    );
    assert_eq!(
        view.root().set_view_state(Some(&Scrolled(3)), 2, client),
        Ok(None),
        "the value kept already is not written again"
    );

    view.apply(
        &view
            .root()
            .set_view_state::<Scrolled>(None, 2, client)
            .expect("encodes")
            .expect("a change"),
    );
    assert_eq!(view.root().view_state::<Scrolled>(), None);

    let unreadable = ViewState {
        bytes: vec![0xff, 0xff, 0xff],
        refs: Vec::new(),
    };
    view.apply(
        &view
            .root()
            .put_stored(Zoomed::ID, Some(&unreadable), 3, client),
    );
    assert!(
        matches!(view.root().view_state::<Zoomed>(), Some(Err(_))),
        "what cannot be read is an error, not nothing"
    );
}
