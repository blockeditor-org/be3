use beui::native::{Context, WindowSize};
use beui_host::native::testing::{Showing, Stepped};
use beui_host::native::{FreetypeFonts, build};

use super::*;

#[test]
fn a_child_moving_out_of_a_view_during_layout_leaves_no_removed_node_to_paint() {
    let _root = RootScope::new();
    let in_window = Rect::from_min_size(pos2(10.0, 10.0), Vec2::new(40.0, 20.0));
    let main = create_signal(placed(Vec::new()));
    let window = create_signal(placed(vec![child(in_window)]));
    let built: Rc<Cell<Option<NodeId>>> = Rc::default();
    let document = build({
        let built = Rc::clone(&built);
        move || {
            let child_view: ChildView = store(Box::new(
                move |_: ChildId, _: Memo<Option<HostChild>>, _: Memo<Rect>| {
                    let node = view! {
                        <Interactive focusable=true>
                            <Frame width={Size::Custom(40.0)} height={Size::Custom(20.0)} />
                        </Interactive>
                    };
                    built.set(Some(node));
                    node
                },
            ));
            let joined = move |mount: u64, state: Signal<PlacedView>| Joined {
                mount,
                state: create_memo(move || state.get()),
                rect: create_memo(|| Rect::ZERO),
                child_view,
                anchors: create_memo(|| None),
                encloses: create_memo(|| None),
            };
            let outer = ChildNodes::joining(None, joined(1, main));
            let inner = ChildNodes::joining(Some(outer), joined(2, window));
            let shown = |state: Signal<PlacedView>| create_memo(move || state.get());
            let rect = create_memo(|| Rect::ZERO);
            view! {
                <Root title="Block">
                    <Layers>
                        <ViewChildren state={shown(main)} rect nodes={outer} below=false />
                        <ViewChildren state={shown(window)} rect nodes={inner} below=false />
                    </Layers>
                </Root>
            }
        }
    });
    let mut stepped = Stepped::new(
        Context::new(FreetypeFonts::default()),
        Showing(document),
        WindowSize {
            size: Vec2::new(400.0, 300.0),
            scale: 1.0,
        },
        None,
    );
    stepped.frame(Vec::new());
    let node = built.get().expect("the window's view built its child");
    stepped.document_mut().focus_focusable(node);
    stepped.frame(Vec::new());

    let moving = Rc::new(Cell::new(true));
    stepped.document_mut().on_laid_out(move || {
        if moving.replace(false) {
            let mut anchored = child(in_window);
            anchored.anchored = Some(ViewId(9));
            main.set(placed(vec![anchored]));
            window.set(placed(Vec::new()));
        }
    });
    stepped.frame(Vec::new());
    stepped.frame(Vec::new());

    assert_eq!(
        built.get(),
        Some(node),
        "the child moved into the main view's anchored views, so it is kept, not built again"
    );
    assert!(
        stepped.document().contains(node),
        "the window's view let go of the child without removing it"
    );
}
