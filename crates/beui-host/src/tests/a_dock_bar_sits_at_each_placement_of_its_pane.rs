use super::*;
use crate::dock::unstyled::{DockBarPlacement, DockEdge};

#[test]
fn a_dock_bar_sits_at_each_placement_of_its_pane() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(DockBarPlacement::Over(DockEdge::Top), false);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let body = harness.rect(harness.find("content.1"));
    for edge in DockEdge::ALL {
        dock.layout.place_bar(&9, DockBarPlacement::Over(edge));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let bar = harness.rect(harness.find("bar.9"));
        let panel = harness.rect(harness.find("content.1"));
        assert_eq!(
            panel, body,
            "a bar floating over the {edge:?} edge leaves the body its room"
        );
        assert!(body.contains_rect(bar), "the bar floats inside the body");
        let (near, along) = match edge {
            DockEdge::Top => (bar.top() - body.top(), bar.center().x - body.center().x),
            DockEdge::Bottom => (
                body.bottom() - bar.bottom(),
                bar.center().x - body.center().x,
            ),
            DockEdge::Left => (bar.left() - body.left(), bar.center().y - body.center().y),
            DockEdge::Right => (body.right() - bar.right(), bar.center().y - body.center().y),
        };
        assert!(
            near > 0.0 && near < 16.0,
            "the bar keeps a margin from the {edge:?} edge, {near}"
        );
        assert!(
            along.abs() < 1.0,
            "the bar is centred along the {edge:?} edge"
        );
        let lying = bar.width() > bar.height();
        assert_eq!(
            lying,
            matches!(edge, DockEdge::Top | DockEdge::Bottom),
            "the bar lays itself out along the {edge:?} edge"
        );
    }
    for edge in DockEdge::ALL {
        dock.layout.place_bar(&9, DockBarPlacement::Beside(edge));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let bar = harness.rect(harness.find("bar.9"));
        let panel = harness.rect(harness.find("content.1"));
        let gave = match edge {
            DockEdge::Top => panel.top() >= bar.bottom() && bar.width() == body.width(),
            DockEdge::Bottom => panel.bottom() <= bar.top() && bar.width() == body.width(),
            DockEdge::Left => panel.left() >= bar.right() && bar.height() == body.height(),
            DockEdge::Right => panel.right() <= bar.left() && bar.height() == body.height(),
        };
        assert!(
            gave,
            "a bar beside the {edge:?} edge takes a band of the pane from the body"
        );
        assert!(body.contains_rect(bar), "the band is inside the pane");
    }
}
