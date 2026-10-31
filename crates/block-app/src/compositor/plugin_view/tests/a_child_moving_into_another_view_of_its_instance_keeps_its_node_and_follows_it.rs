use super::*;
#[test]
fn a_child_moving_into_another_view_of_its_instance_keeps_its_node_and_follows_it() {
    let _root = RootScope::new();
    let built: Rc<RefCell<Vec<Memo<Option<HostChild>>>>> = Rc::default();
    let sink = Rc::clone(&built);
    let child_view: ChildView = store(Box::new(
        move |_: ChildId, child: Memo<Option<HostChild>>, _: Memo<Rect>| {
            sink.borrow_mut().push(child);
            NodeId::new(sink.borrow().len() as u32, 0)
        },
    ));
    let in_window = Rect::from_min_size(pos2(10.0, 10.0), Vec2::new(200.0, 40.0));
    let lifted = Rect::from_min_size(pos2(300.0, 200.0), Vec2::new(200.0, 40.0));
    let main = create_signal(placed(Vec::new()));
    let window = create_signal(placed(vec![child(in_window)]));
    let joined = move |mount: u64, state: Signal<PlacedView>| Joined {
        mount,
        state: create_memo(move || state.get()),
        rect: create_memo(|| Rect::ZERO),
        child_view,
        anchors: create_memo(|| None),
        encloses: create_memo(|| None),
    };
    let view = reactive::Scope::new();
    let outer = view.run(|| ChildNodes::joining(None, joined(1, main)));
    let opened = reactive::Scope::new();
    let inner = opened.run(|| ChildNodes::joining(Some(outer), joined(2, window)));

    let node = inner.node(ChildId(7));
    assert_eq!(built.borrow().len(), 1);
    let shown = built.borrow()[0];
    assert_eq!(shown.get().map(|child| child.clip), Some(in_window));

    main.set(placed(vec![child(lifted)]));
    window.set(placed(Vec::new()));
    opened.dispose();
    reactive::flush();
    assert_eq!(
        outer.node(ChildId(7)),
        node,
        "the window's view is gone, and the child it showed goes on in the view it moved into"
    );
    assert_eq!(built.borrow().len(), 1, "the child is not built again");
    assert_eq!(
        shown.get().map(|child| child.clip),
        Some(lifted),
        "the child follows its place in the view that shows it now"
    );

    main.set(placed(Vec::new()));
    reactive::flush();
    outer.node(ChildId(7));
    assert_eq!(
        built.borrow().len(),
        2,
        "a child no view of the instance shows any more is let go, and built afresh when shown again"
    );
    drop(view);
}
