use super::*;

#[test]
fn a_reopened_tab_shows_its_saved_title_before_its_block_is_known() {
    let (mut fixture, opened) = profiled(None);
    let mut info = block_editor_beui::BlockInfo::new(opened, SHOWN_TYPE, BlockParent::Root);
    info.set_name("Issues");
    fixture.test.store().add_block(info);
    show(&mut fixture, opened, None);
    assert!(fixture.says("Issues"));

    let saved = profile(&fixture);
    let mut reopened_profile = EditorView::document(None);
    for key in ["layout", "titles"] {
        let state = saved.state(key).cloned().expect("the profile keeps it");
        let edit = reopened_profile
            .root()
            .set_state(key, Some(&state), 1, Uuid::nil());
        reopened_profile.apply(&edit);
    }
    let (mut reopened, _) = open_editor(None, Some(reopened_profile));
    reopened.settle();

    assert_eq!(reopened.open_tabs(), 1);
    assert!(
        reopened.says("Issues"),
        "the tab keeps its name while its block is still unknown"
    );
}
