use super::*;

#[test]
fn a_reopened_block_gets_back_the_view_it_was_closed_with() {
    let (mut fixture, opened) = profiled(None);
    show(&mut fixture, opened, None);
    let view = profile(&fixture).state("layout").expect("saved").refs[0];

    fixture.close_active_tab();

    let info = fixture
        .test
        .store()
        .block(view)
        .expect("the view was a block");
    assert!(
        matches!(info.parent, Some(BlockParent::Block(_))),
        "a closed tab's view stays in the session, holding what its editor kept there, such as \
         where a text was scrolled to"
    );
    let layout = profile(&fixture).state("layout").cloned().expect("saved");
    assert!(layout.refs.is_empty(), "no tab's view is referenced");
    let closed = profile(&fixture)
        .state("closed-views")
        .cloned()
        .expect("the closed views are saved");
    assert_eq!(
        closed.refs,
        vec![view],
        "the session lists it among its closed views"
    );

    show(&mut fixture, opened, None);
    let layout = profile(&fixture).state("layout").cloned().expect("saved");
    assert_eq!(
        layout.refs,
        vec![view],
        "opening the block again, as from the files list after going back, uses that view"
    );
    let closed = profile(&fixture)
        .state("closed-views")
        .cloned()
        .expect("saved");
    assert!(closed.refs.is_empty(), "an open view is not a closed one");
}
