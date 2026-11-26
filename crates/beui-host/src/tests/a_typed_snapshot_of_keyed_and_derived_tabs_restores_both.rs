use super::*;
use crate::dock::{DockPane, DockTab, Docking, DockingLayout};

fn dock(layout: DockingLayout<&'static str>) -> Document {
    build_node(move || {
        view! {
            <dock_host::styled::DockHost>
                <Docking layout>
                    <DockPane id="tabs">
                        <DockTab id="files" title="Files">
                            <Frame @test_id="content.files" />
                        </DockTab>
                        <DockTab title="Notes">
                            <Frame @test_id="content.notes" />
                        </DockTab>
                    </DockPane>
                </Docking>
            </dock_host::styled::DockHost>
        }
    })
}

/// A typed snapshot holds only the keyed tabs, and leaves the derived ones out of its state too,
/// so a restore never meets a tab id it holds no key for; the derived tab is declared again.
#[test]
fn a_typed_snapshot_of_keyed_and_derived_tabs_restores_both() {
    let _root = crate::reactive::RootScope::new();
    let first = DockingLayout::<&'static str>::new();
    let mut harness = Harness::sized(dock(first), WIDE_VIEWPORT);
    harness.frame(Vec::new());
    assert_eq!(first.state().get_untracked().all_tabs().len(), 2);
    let snapshot = first.snapshot();
    assert_eq!(snapshot.keys().copied().collect::<Vec<_>>(), vec!["files"]);
    assert_eq!(
        snapshot.state().all_tabs(),
        vec![first.tab_of(&"files").expect("files is keyed")],
        "the derived tab is left out of the snapshot's state"
    );

    let second = DockingLayout::<&'static str>::new();
    second.restore(snapshot);
    let mut restored = Harness::sized(dock(second), WIDE_VIEWPORT);
    restored.frame(Vec::new());
    restored.frame(Vec::new());
    let tabs = second.state().get_untracked().all_tabs();
    assert_eq!(tabs.len(), 2, "both tabs are open again: {tabs:?}");
    assert_eq!(second.tab_of(&"files"), first.tab_of(&"files"));
    restored.find("content.notes");
}
