use super::*;
use crate::reactive::*;

#[component]
fn Program(id: u64) -> dock::unstyled::DockNode<u64> {
    let key = format!("window.{id}");
    let at = 60.0 + 40.0 * id as f32;
    view! {
        <dock::unstyled::DockWindow
            id={key.clone()}
            rect={Rect::from_min_size(pos2(at, at), vec2(360.0, 240.0))}
        >
            <dock::unstyled::DockPane id={key}>
                <dock::unstyled::DockTab id={id} title={format!("Program {id}")}>
                    <Frame @test_id={format!("program.{id}")} />
                </dock::unstyled::DockTab>
            </dock::unstyled::DockPane>
        </dock::unstyled::DockWindow>
    }
}

fn drawn_bottom_up(harness: &Harness) -> Vec<u64> {
    let document = harness.document();
    let mut programs = vec![1, 2, 3];
    programs
        .sort_by_key(|program| document.paint_order(harness.find(&format!("program.{program}"))));
    programs
}

fn window_of(harness: &Harness, program: u64) -> NodeId {
    let document = harness.document();
    let mut node = harness.find(&format!("program.{program}"));
    while document
        .arena
        .kind_of::<crate::base::overlay::OverlayNode>(node)
        .is_none()
    {
        node = document
            .arena
            .parent(node)
            .expect("a floating window is an overlay");
    }
    node
}

#[test]
fn a_floating_window_the_dock_puts_on_top_is_drawn_on_top() {
    let _root = crate::reactive::RootScope::new();
    let layout = dock::unstyled::DockingLayout::<u64>::new();
    let document = build_node(move || {
        view! {
            <dock::styled::Docking layout>
                <dock::unstyled::DockPane
                    id="desktop"
                    empty={|| view! {
                        <Frame />
                    }}
                />
                <ForEach keys={vec![1u64, 2, 3]}>|id: u64|
                    <Program id />
                </ForEach>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    for program in [2, 3, 1] {
        layout.show(&program);
        harness.frame(Vec::new());
    }
    assert_eq!(
        drawn_bottom_up(&harness),
        vec![2, 3, 1],
        "showing a window raises it"
    );

    let covering = window_of(&harness, 3);
    harness.document_mut().raise_overlay(covering);
    harness.frame(Vec::new());
    assert_eq!(drawn_bottom_up(&harness), vec![2, 1, 3]);
    assert_eq!(
        layout.focused(),
        Some(1),
        "the covered window keeps the focus"
    );

    layout.show(&1);
    harness.frame(Vec::new());
    assert_eq!(
        drawn_bottom_up(&harness),
        vec![2, 3, 1],
        "showing the window that has the focus still brings it back on top"
    );
}
