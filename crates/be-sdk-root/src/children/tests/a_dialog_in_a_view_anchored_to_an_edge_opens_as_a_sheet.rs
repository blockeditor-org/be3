use be_sdk_api::{AnchoredPlacement, ScreenId, ViewAnchor, ViewEdge};

use super::*;

/// A view hanging from editor 1's main view, anchored by `anchor`.
fn hanging(anchor: ViewAnchor) -> AnchoredPlacement {
    let rect = rect((0.0, 0.0), (200.0, 120.0));
    AnchoredPlacement {
        parent: ViewId::MAIN,
        anchor,
        rect,
        dismiss_on_outside_press: true,
        trigger: None,
        keyboard: true,
        blocks_owner: false,
    }
}

#[test]
fn a_dialog_in_a_view_anchored_to_an_edge_opens_as_a_sheet() {
    // Editor 1 anchors a sheet to an edge (view 10, screen 1) and a popover to a rect (view
    // 11, screen 2): only the sheet's view is one whose dialogs open as sheets, known from the
    // layout alone, before either view painted.
    let sheet = hanging(ViewAnchor::Edge(ViewEdge::Bottom));
    let popover = hanging(ViewAnchor::Rect(rect((10.0, 10.0), (20.0, 20.0))));
    let sheets = crate::embedded::sheet_views(
        [(ScreenId(1), &sheet), (ScreenId(2), &popover)].into_iter(),
        |screen| Some((EditorInstanceId(1), ViewId(9 + screen.0))),
    );
    assert_eq!(sheets, vec![(EditorInstanceId(1), ViewId(10))]);

    // A picker placed in each is wanted as a sheet only in the sheet's view.
    let picker = |view: u64| {
        let mut placement = placed(rect((0.0, 0.0), (200.0, 120.0)), ChildMode::Live);
        placement.content = ChildContent::Picker {
            filter: be_sdk_api::BlockFilter::default(),
            parent: be_sdk_api::BlockLocation::Root,
        };
        Child::placed(
            EditorInstanceId(1),
            ViewId(view),
            1.0,
            placement,
            Vec::new(),
            sheets.contains(&(EditorInstanceId(1), ViewId(view))),
        )
        .expect("a picker runs in an editor")
    };
    let sheet_of = |child: &Child| match &child.shown {
        Shown::Dialog(dialog) => dialog.sheet,
        Shown::Block(_) => panic!("a picker is a dialog"),
    };
    assert!(sheet_of(&picker(10)), "in the sheet's view it opens as a sheet");
    assert!(!sheet_of(&picker(11)), "in the popover's view it does not");
}
