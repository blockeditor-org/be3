use be_sdk_api::{
    AnchoredPlacement, EditorInstanceId, Pos2 as ViewPos2, ScreenId, ScreenRequest,
    Vec2 as ViewVec2, ViewAnchor, ViewId, ViewPlacement, ViewportMetrics,
};

use super::*;

/// The view `view` of the editor `editor`, anchored from its view `parent`.
fn hung(editor: u64, view: u64, parent: u64) -> AnchoredView {
    let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(40.0, 20.0));
    AnchoredView {
        layer: shown_at((0.0, 0.0), 1.0),
        placement: AnchoredPlacement {
            parent: ViewId(parent),
            anchor: ViewAnchor::Rect(rect),
            rect,
            dismiss_on_outside_press: true,
            trigger: None,
            keyboard: true,
            blocks_owner: false,
        },
        screen: ScreenId(view),
        editor: EditorInstanceId(editor),
        view: ViewId(view),
        request: ScreenRequest {
            screen: ScreenId(view),
            instance: EditorInstanceId(editor),
            view: ViewId(view),
            placement: ViewPlacement::Anchored,
            frame: None,
            metrics: ViewportMetrics {
                logical: ViewVec2 { x: 40.0, y: 20.0 },
                visible: ViewPos2::ZERO,
                pixel_width: 40,
                pixel_height: 20,
                scale_factor: 1.0,
                raster: 1.0,
            },
            windows: false,
            host_place: None,
            docked: false,
            anchor_space: None,
            fit: false,
            origin: None,
        },
    }
}

#[test]
fn anchored_views_hang_in_chains_from_their_editors_views() {
    // A menu hangs from the editor's main view and its submenu from the menu; another editor's
    // popover hangs from that editor's own main view, whose id is the same as the first's.
    let chains = anchored_chains(vec![hung(1, 10, 0), hung(1, 11, 10), hung(2, 12, 0)]);
    let shown: Vec<(EditorInstanceId, ViewId, Vec<ViewId>)> = chains
        .iter()
        .map(|chain| {
            (
                chain.editor,
                chain.parent,
                chain.views.iter().map(|view| view.view).collect(),
            )
        })
        .collect();
    assert_eq!(
        shown,
        vec![
            (EditorInstanceId(1), ViewId(0), vec![ViewId(10), ViewId(11)]),
            (EditorInstanceId(2), ViewId(0), vec![ViewId(12)]),
        ]
    );
}
