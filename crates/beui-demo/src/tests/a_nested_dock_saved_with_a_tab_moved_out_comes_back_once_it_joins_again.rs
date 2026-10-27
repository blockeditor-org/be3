use super::*;
use beui::PointerButton;
use beui::dock::unstyled::DockingSnapshot;
use beui::testing::Target;
use std::cell::Cell;

thread_local! {
    static LAYOUT: Cell<Option<DockingLayout<u64>>> = const { Cell::new(None) };
}

const BAR_Y: f32 = 20.0;
const SIZE: Vec2 = Vec2::new(900.0, 500.0);

#[component]
fn Inner() -> NodeId {
    let layout = DockingLayout::<u64>::new();
    view! {
        <Docking layout>
            <DockPane id="inner">
                <DockTab id=10u64 title="Alpha">
                    <Text>"Alpha body"</Text>
                </DockTab>
                <DockTab id=11u64 title="Beta">
                    <Text>"Beta body"</Text>
                </DockTab>
            </DockPane>
        </Docking>
    }
}

fn nested(restored: Option<DockingSnapshot<u64>>) -> DocumentTest {
    let document = build(move || {
        let layout = DockingLayout::<u64>::new();
        if let Some(restored) = restored.clone() {
            layout.restore(restored);
        }
        LAYOUT.with(|held| held.set(Some(layout)));
        view! {
            <Root>
                <Docking layout>
                    <DockSplit id="outer">
                        <DockPane id="left">
                            <DockTab id=1u64 title="Host">
                                <Inner />
                            </DockTab>
                        </DockPane>
                        <DockPane id="right">
                            <DockTab id=2u64 title="Other">
                                <Text>"Other body"</Text>
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                </Docking>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, SIZE);
    test.frame(Vec::new());
    test.settle();
    test
}

fn text_x(test: &DocumentTest, text: &str) -> f32 {
    let document = test.document();
    let root = document.root().expect("the test built a root");
    find_text(document, root, text)
        .and_then(|node| document.node_rect(node))
        .unwrap_or_else(|| panic!("{text} is shown"))
        .center()
        .x
}

fn find_text(document: &Document, id: NodeId, text: &str) -> Option<NodeId> {
    let here = document.node_rect(id).is_some()
        && document
            .arena
            .kind_of(id)
            .is_some_and(|node| document.text(node) == text);
    if here {
        return Some(id);
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| find_text(document, child, text))
}

fn menu_shows(test: &DocumentTest, text: &str) -> bool {
    let document = test.document();
    document
        .layers()
        .iter()
        .filter_map(|layer| layer.content)
        .any(|content| showing(document, content, text) > 0)
}

#[test]
fn a_nested_dock_saved_with_a_tab_moved_out_comes_back_once_it_joins_again() {
    let mut test = nested(None);
    assert!(
        text_x(&test, "Beta") < SIZE.x / 2.0,
        "the inner tabs join the outer dock where the host tab was"
    );
    test.click_with(Target::text("Beta"), PointerButton::Secondary);
    test.frame(Vec::new());
    test.click(Target::text("Unlock"));
    test.frame(Vec::new());
    let bar = pos2(text_x(&test, "Other") + 60.0, BAR_Y);
    test.drag(Target::text("Beta"), Target::point(bar));
    test.frame(Vec::new());
    test.settle();
    assert!(
        text_x(&test, "Beta") > SIZE.x / 2.0,
        "the unlocked inner tab moves into the outer dock's other pane"
    );

    let layout = LAYOUT.with(Cell::get).expect("the outer dock was built");
    let saved = with_reactive_scope(test.document_mut(), || layout.snapshot());

    let mut reopened = nested(Some(saved));
    assert!(
        text_x(&reopened, "Beta") > SIZE.x / 2.0,
        "the moved tab comes back in the other pane once the inner dock declares it again"
    );
    assert!(
        text_x(&reopened, "Alpha") < SIZE.x / 2.0,
        "the tab left in the group comes back in the group"
    );
    let beta = Target::point(pos2(text_x(&reopened, "Beta"), BAR_Y));
    reopened.click(beta.clone());
    reopened.frame(Vec::new());
    assert!(
        text_x(&reopened, "Beta body") > SIZE.x / 2.0,
        "the inner dock places the moved tab's body in the other pane"
    );
    reopened.click_with(beta, PointerButton::Secondary);
    reopened.frame(Vec::new());
    assert!(
        menu_shows(&reopened, "Lock"),
        "the moved tab is still unlocked and can lock again once home"
    );
    reopened.key(beui::Key::Escape, beui::Modifiers::NONE);
    reopened.frame(Vec::new());
    reopened.snapshot("a_nested_dock_saved_with_a_tab_moved_out_comes_back_once_it_joins_again");
}
