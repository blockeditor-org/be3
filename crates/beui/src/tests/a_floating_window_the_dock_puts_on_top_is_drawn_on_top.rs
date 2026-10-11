use super::*;
use crate::reactive::{ForEach, NodeRef, build, view, with_reactive_scope};

#[component]
fn Program(id: u64) -> unstyled::DockNode<u64> {
    let key = format!("window.{id}");
    let at = 60.0 + 40.0 * id as f32;
    view! {
        <unstyled::DockWindow
            id={key.clone()}
            rect={Rect::from_min_size(pos2(at, at), vec2(360.0, 240.0))}
        >
            <unstyled::DockPane id={key}>
                <unstyled::DockTab id={id} title={format!("Program {id}")}>
                    <Frame @test_id={format!("program.{id}")} />
                </unstyled::DockTab>
            </unstyled::DockPane>
        </unstyled::DockWindow>
    }
}

fn drawn_bottom_up(harness: &Harness) -> Vec<u64> {
    let document = harness.document();
    let mut programs = vec![1, 2, 3];
    programs
        .sort_by_key(|program| document.paint_order(harness.find(&format!("program.{program}"))));
    programs
}

fn stacked_bottom_up(
    harness: &Harness,
    dock: NodeId,
    layout: &unstyled::DockingLayout<u64>,
) -> Vec<u64> {
    let state = unstyled::dock_state(harness.document(), dock);
    state
        .windows()
        .iter()
        .filter_map(|surface| layout.key_of(*state.surface_tabs(*surface).first()?))
        .collect()
}

fn edit(harness: &mut Harness, change: impl FnOnce() + 'static) {
    with_reactive_scope(harness.document_mut(), change);
    harness.frame(Vec::new());
}

#[test]
fn a_floating_window_the_dock_puts_on_top_is_drawn_on_top() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let layout = unstyled::DockingLayout::<u64>::new();
    let shown = layout.clone();
    let document = build(move || {
        view! {
            <styled::Docking @node_ref=&built layout={shown}>
                <unstyled::DockPane
                    id="desktop"
                    empty={|| view! {
                        <Frame />
                    }}
                />
                <ForEach keys={vec![1u64, 2, 3]}>
                    {move |id: u64| view! {
                        <Program id />
                    }}
                </ForEach>
            </styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let dock = dock.get();
    for program in [2, 3, 1] {
        let layout = layout.clone();
        edit(&mut harness, move || layout.show(&program));
    }
    assert_eq!(stacked_bottom_up(&harness, dock, &layout), vec![2, 3, 1]);
    assert_eq!(
        drawn_bottom_up(&harness),
        vec![2, 3, 1],
        "focusing a window raises it"
    );
    let saved = layout.snapshot();

    for program in [2, 1] {
        let layout = layout.clone();
        edit(&mut harness, move || layout.show(&program));
    }
    assert_eq!(stacked_bottom_up(&harness, dock, &layout), vec![3, 2, 1]);
    assert_eq!(drawn_bottom_up(&harness), vec![3, 2, 1]);

    let restoring = layout.clone();
    edit(&mut harness, move || restoring.restore(saved));
    assert_eq!(layout.focused(), Some(1), "the focus stayed where it was");
    assert_eq!(stacked_bottom_up(&harness, dock, &layout), vec![2, 3, 1]);
    assert_eq!(
        drawn_bottom_up(&harness),
        vec![2, 3, 1],
        "the windows are drawn in the order the dock stacks them, though the focus never moved"
    );
}
