use super::*;
use beui::reactive::*;
use beui::styled;
use beui_inspector::{InspectorTools, scale};

#[component]
fn Greeting() -> NodeId {
    view! {
        <styled::Frame>
            <styled::Text string="Hi" />
        </styled::Frame>
    }
}

#[test]
fn the_inspector_paints_the_component_shadow_tree() {
    let document = build(|| {
        view! {
            <Greeting />
        }
    });
    let mut test = DocumentTest::new(document, WIDE);
    test.frame(vec![beui::Event::key(beui::KeyPress {
        key: beui::Key::I,
        pressed: true,
        modifiers: beui::Modifiers {
            ctrl: true,
            shift: true,
            ..beui::Modifiers::NONE
        },
        repeat: false,
    })]);
    test.frame(Vec::new());
    let inspector = test
        .document()
        .tools::<InspectorTools>()
        .unwrap()
        .inspector
        .as_ref()
        .unwrap();
    let tab = inspector.option_node("inspector.tabs", 1);
    let center = inspector
        .document
        .node_rect(tab)
        .unwrap()
        .scaled(scale(test.context()))
        .center();
    test.click_at(center);
    test.frame(Vec::new());
    test.snapshot("inspector_component_shadow_tree");
}
