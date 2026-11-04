use super::*;
use crate::dock::{DockPane, DockTab, DockingLayout, TabId};
use crate::reactive::*;

const FILES: u64 = 1;

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

fn tap(harness: &mut Harness, test_id: &str) {
    let at = harness.center(harness.find(test_id));
    harness.click(at);
    harness.settle();
}

#[test]
fn a_stacked_dock_goes_back_from_a_tab_back_does_not_close_and_leaves_it_open() {
    let _root = crate::reactive::RootScope::new();
    let closed = create_signal(Vec::new());
    let open = create_signal(Vec::<u64>::new());
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking layout focus=FILES>
                    <DockPane id="tabs">
                        <DockTab id=FILES title="Files">
                            <Frame @test_id="content.1" />
                        </DockTab>
                        <ForEach keys={open}>|id: u64|
                            <DockTab
                                id
                                title={format!("Window {id}")}
                                on_close={move || {
                                    closed.update(|closed| closed.push(TabId::new(id)));
                                    open.update(|open| open.retain(|other| *other != id));
                                }}
                                back_closes=false
                            >
                                <Frame @test_id={format!("content.{id}")} />
                            </DockTab>
                        </ForEach>
                    </DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, VIEWPORT);
    harness.settle();

    open.update(|open| open.push(2));
    harness.settle();
    assert!(laid_out(&harness, "content.2"), "the window opens on show");
    assert!(
        laid_out(&harness, "dock.back"),
        "a tab opened from another can go back to it"
    );

    tap(&mut harness, "dock.back");
    assert!(
        laid_out(&harness, "content.1"),
        "back returns to the tab the window was opened from"
    );
    assert_eq!(
        closed.get_untracked(),
        Vec::<TabId>::new(),
        "and leaves the window open, as closing it would end its program"
    );
    assert!(
        !laid_out(&harness, "dock.back"),
        "the visit back undid is gone"
    );

    tap(&mut harness, "dock.switch");
    assert!(
        laid_out(&harness, "dock.switcher.tab.2"),
        "the window is still one of the dock's tabs"
    );
}
