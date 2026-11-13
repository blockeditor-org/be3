use super::*;

#[test]
fn an_editor_closes_only_itself_and_the_children_it_placed() {
    let (shell, editor) = (Uuid::new_v4(), Uuid::new_v4());
    assert!(
        may_close(Some(shell), Some(shell), Some(editor)),
        "the shell closes any tab"
    );
    assert!(
        may_close(Some(editor), Some(shell), Some(editor)),
        "an editor closes a child it placed"
    );
    assert!(
        !may_close(Some(editor), Some(shell), Some(shell)),
        "an editor never closes another's"
    );
    assert!(
        !may_close(Some(editor), Some(shell), None),
        "nor one nobody placed"
    );
    assert!(
        !may_close(None, Some(shell), None),
        "nor does an ask from no editor"
    );
}
