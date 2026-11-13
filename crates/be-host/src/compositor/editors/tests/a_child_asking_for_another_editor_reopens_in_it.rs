use super::*;

#[test]
fn a_child_asking_for_another_editor_reopens_in_it() {
    let block_type = Uuid::new_v4();
    let built_in = app(
        "test.built-in",
        vec![
            editor("main", Edits::Type(block_type.into_bytes())),
            editor("outline", Edits::Type(block_type.into_bytes())),
        ],
    );
    let debug = app("test.debug", vec![editor("debug", Edits::Any)]);
    let (built_in_id, outline_id, debug_id) = (
        Uuid::from_bytes(built_in.editors[0].id),
        Uuid::from_bytes(built_in.editors[1].id),
        Uuid::from_bytes(debug.editors[0].id),
    );
    let registry = Rc::new(EditorRegistry::from_manifests(vec![built_in, debug]));
    let editors = OwnedEditors::install(registry, Uuid::new_v4());
    let (id, parent) = (Uuid::new_v4(), Uuid::new_v4());
    let place = (parent, ViewId::MAIN, ChildId(1));
    let opened =
        |editors: &OwnedEditors| editors.with(|open| open.get(&id).and_then(AppEditor::editor_id));

    editors.place(place, id, block_type, None, None);
    assert_eq!(
        opened(&editors),
        Some(built_in_id),
        "a block opens in the first editor of its type"
    );

    editors.place(place, id, block_type, None, Some(outline_id));
    assert_eq!(
        opened(&editors),
        Some(outline_id),
        "a second editor of the same type in the same app is opened by its id"
    );

    editors.place(place, id, block_type, None, Some(debug_id));
    assert_eq!(
        opened(&editors),
        Some(debug_id),
        "asking for an editor of every type opens it"
    );

    editors.place(place, id, block_type, None, None);
    assert_eq!(
        opened(&editors),
        Some(debug_id),
        "a child that names no editor keeps the open one"
    );

    editors.place(place, id, block_type, None, Some(Uuid::new_v4()));
    assert_eq!(
        opened(&editors),
        Some(debug_id),
        "an editor nobody provides is not opened"
    );

    editors.place(place, id, block_type, None, Some(built_in_id));
    assert_eq!(opened(&editors), Some(built_in_id));

    drop(editors);
}
