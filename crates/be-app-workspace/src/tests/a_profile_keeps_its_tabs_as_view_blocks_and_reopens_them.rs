use super::*;

#[test]
fn a_profile_keeps_its_tabs_as_view_blocks_and_reopens_them() {
    let (mut fixture, opened) = profiled(None);
    let second = Uuid::new_v4();
    show(&mut fixture, opened, None);
    show(&mut fixture, second, None);

    let layout = profile(&fixture)
        .state("layout")
        .cloned()
        .expect("the layout is saved in the profile");
    let [first_view, second_view] = layout.refs[..] else {
        panic!("the layout references one view per tab");
    };
    let files_state = profile(&fixture)
        .state("files")
        .cloned()
        .expect("the files view is saved in the profile");
    let files = files_state.refs[0];
    let store = fixture.test.store();
    for (view, content) in [
        (files, None),
        (first_view, Some(opened)),
        (second_view, Some(second)),
    ] {
        let info = store.block(view).expect("each view is a block");
        assert_eq!(
            info.parent,
            Some(BlockParent::Block(fixture.test.block_id().unwrap()))
        );
        let held = store.content::<EditorViewContent>(Some(view)).root();
        assert_eq!(held.content, content);
    }
    let placement = fixture
        .test
        .children()
        .last()
        .expect("the shown tab is placed");
    let ChildContent::Block { view_block, .. } = placement.content else {
        panic!("the shown tab is a block");
    };
    assert_eq!(view_block, Some(second_view.into_bytes()));

    let mut saved = EditorView::document(None);
    for (key, state) in [("layout", layout), ("files", files_state)] {
        let edit = saved.root().set_state(key, Some(&state), 1, Uuid::nil());
        saved.apply(&edit);
    }
    let (mut reopened, _) = open_editor(None, Some(saved));
    reopened.settle();

    assert_eq!(reopened.open_tabs(), 2, "both tabs come back");
    assert_eq!(
        reopened.shown(),
        vec![files, second],
        "the files and the focused tab show"
    );
}
