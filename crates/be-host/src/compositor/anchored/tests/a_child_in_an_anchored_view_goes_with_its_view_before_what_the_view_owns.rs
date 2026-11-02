use super::*;

#[test]
fn a_child_in_an_anchored_view_goes_with_its_view_before_what_the_view_owns() {
    let _root = beui_reactive::RootScope::new();
    let region = beui_reactive::Scope::new();
    let (owned, context) = region.run(|| (create_signal(0u32), beui_reactive::current_scope()));
    let context = context.expect("the region's scope is alive");
    let seen = Rc::new(Cell::new(None));
    let item = beui_reactive::Scope::new();
    item.run(|| {
        let child = closed_with(&context).expect("the region is alive");
        let sink = Rc::clone(&seen);
        child.run(move || {
            on_cleanup(move || sink.set(Some(owned.is_alive().then(|| owned.get_untracked()))));
        });
        on_cleanup(move || child.dispose());
    });

    region.dispose();
    assert_eq!(
        seen.get(),
        Some(Some(0)),
        "the child is gone with its region, and still reads what the region owns as it goes"
    );
    drop(item);
}
