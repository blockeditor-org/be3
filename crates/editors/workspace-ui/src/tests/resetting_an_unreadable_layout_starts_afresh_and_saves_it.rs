use super::*;

#[test]
fn resetting_an_unreadable_layout_starts_afresh_and_saves_it() {
    let (mut fixture, opened) = profiled(Some(unreadable_layout()));

    fixture.test.click("workspace.layout.reset");
    fixture.settle();

    assert!(
        !fixture.test.shown("workspace.layout.reset"),
        "the notice closes"
    );
    let layout = profile(&fixture)
        .state("layout")
        .cloned()
        .expect("a layout is saved");
    assert_ne!(layout, unreadable_layout(), "the fresh layout is saved over it");
    assert_eq!(layout.refs.len(), 1, "only the new files view is referenced");

    show(&mut fixture, opened, None);
    let (reopened, _) = profiled(profile(&fixture).state("layout").cloned());
    assert!(
        !reopened.test.shown("workspace.layout.reset"),
        "the saved layout reads back"
    );
    assert_eq!(reopened.open_tabs(), 1, "the tab opened after the reset comes back");
}
