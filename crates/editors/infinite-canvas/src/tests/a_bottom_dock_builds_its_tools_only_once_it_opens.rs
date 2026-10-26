use super::*;
use std::cell::Cell;
use std::rc::Rc;

use block_editor_beui::BottomDock;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

#[component]
fn Counted(built: Rc<Cell<u32>>) -> NodeId {
    built.set(built.get() + 1);
    view! {
        <Frame width=Size::Md height=Size::Md />
    }
}

#[test]
fn a_bottom_dock_builds_its_tools_only_once_it_opens() {
    let host = EditorHost::default();
    let owner = OwnedEditor::new(host, Uuid::new_v4());
    let built = Rc::new(Cell::new(0));
    let open = Rc::new(Cell::new(None::<Signal<bool>>));
    let (counted, held) = (Rc::clone(&built), Rc::clone(&open));
    let mut test = BeuiTest::<CanvasApp>::with_view(owner, move || {
        let anchor = NodeRef::new();
        let open = create_signal(false);
        held.set(Some(open));
        let (above, tool) = (Rc::clone(&counted), counted);
        view! {
            <Root>
                <Frame @node_ref=&anchor width={Size::Custom(400.0)} height={Size::Custom(300.0)}>
                    <BottomDock
                        anchor={anchor}
                        open={open}
                        name="dock"
                        above={move || view! {
                            <Counted built={above} />
                        }}
                    >
                        <Counted built={tool} />
                    </BottomDock>
                </Frame>
            </Root>
        }
    });
    test.run();
    assert_eq!(
        built.get(),
        0,
        "a closed dock builds neither its tools nor what sits above them"
    );

    open.get().expect("the view was built").set(true);
    test.run();
    assert_eq!(built.get(), 2, "an opened dock builds both");
    assert!(test.document().find_test_id("dock").is_some());
}
