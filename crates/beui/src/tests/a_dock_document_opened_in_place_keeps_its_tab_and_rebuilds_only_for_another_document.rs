use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::reactive::*;

type Doc = (u64, u32);

#[test]
fn a_dock_document_opened_in_place_keeps_its_tab_and_rebuilds_only_for_another_document() {
    let held = Rc::new(Cell::new(None));
    let owned = OwnedSignal::new(Vec::new());
    let built = owned.handle();
    let document = {
        let held = held.clone();
        build_node(move || {
            let layout = unstyled::DockingLayout::new();
            let documents =
                unstyled::DockDocuments::new(layout, |slot| 100 + slot.value(), |doc: &Doc| doc.0);
            held.set(Some(documents));
            view! {
                <styled::Docking layout>
                    <unstyled::DockPane id="tabs">
                        <unstyled::DockTab id=1u64 title="Fixed">
                            <Frame />
                        </unstyled::DockTab>
                        <unstyled::DockDocumentTabs
                            documents
                        >|document: unstyled::DockDocument<u64, Doc, u64>|
                            <unstyled::DockTab
                                id={document.key()}
                                title="Document"
                                on_close={move || document.close()}
                            >
                                <unstyled::DockDocumentBody document>|content: ReadSignal<Doc>| {
                                    built.update(|built| built.push(content.get_untracked().0));
                                    view! {
                                        <Frame />
                                    }
                                }</unstyled::DockDocumentBody>
                            </unstyled::DockTab>
                        </unstyled::DockDocumentTabs>
                    </unstyled::DockPane>
                </styled::Docking>
            }
        })
    };
    let documents = held.get().expect("the dock was built");
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let first = documents.open((1, 0), false).slot();
    harness.frame(Vec::new());
    harness.find("dock.tab.101.close");
    assert_eq!(
        built.get_untracked(),
        [1],
        "an opened document builds its body"
    );

    let opened = documents.open((2, 0), false);
    harness.frame(Vec::new());
    assert_eq!(
        opened,
        unstyled::DockOpened::Replaced(first, (1, 0)),
        "another document takes the place of the one shown"
    );
    harness.find("dock.tab.101.close");
    assert_eq!(
        built.get_untracked(),
        [1, 2],
        "and its tab rebuilds its body for the new document"
    );

    documents.refresh((2, 5));
    harness.frame(Vec::new());
    assert_eq!(
        built.get_untracked(),
        [1, 2],
        "new content for the same document reaches the body without rebuilding it"
    );
    assert_eq!(
        documents
            .content(first)
            .map(|content| content.get_untracked()),
        Some((2, 5))
    );

    let beside = documents.open((3, 0), true);
    harness.frame(Vec::new());
    assert!(
        matches!(beside, unstyled::DockOpened::Added(_)),
        "a document opened beside gets a tab of its own"
    );
    harness.find("dock.tab.101.close");
    harness.find("dock.tab.102.close");

    assert_eq!(
        documents.open((2, 0), false),
        unstyled::DockOpened::Shown(first),
        "a document that is open is shown where it is"
    );
    harness.frame(Vec::new());
    assert_eq!(built.get_untracked(), [1, 2, 3]);

    let close = harness.center(harness.find("dock.tab.101.close"));
    harness.click(close);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.101.close")
            .is_none(),
        "a document's tab closes from its close button"
    );
    assert_eq!(untrack(|| documents.find(&2)), None);
}
