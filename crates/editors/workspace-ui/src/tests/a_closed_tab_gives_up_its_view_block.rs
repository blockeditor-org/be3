use super::*;

#[test]
fn a_closed_tab_gives_up_its_view_block() {
    let (mut fixture, opened) = profiled(None);
    show(&mut fixture, opened, None);
    let view = profile(&fixture).state("layout").expect("saved").refs[1];

    fixture.close_active_tab();

    let info = fixture
        .test
        .store()
        .block(view)
        .expect("the view was a block");
    assert_eq!(info.parent, BlockParent::Detached, "the view is deleted");
    let layout = profile(&fixture).state("layout").cloned().expect("saved");
    assert_eq!(layout.refs.len(), 1, "only the files view is referenced");
}
