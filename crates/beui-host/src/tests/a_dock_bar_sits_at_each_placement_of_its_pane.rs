use super::*;
use crate::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_dock_bar_sits_at_each_placement_of_its_pane() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(DockBarPlacement::Over(DockEdge::Top, DockAlign::Center), false);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let body = harness.rect(harness.find("content.1"));
    for edge in DockEdge::ALL {
        for align in DockAlign::ALL {
            dock.layout.place_bar(&9, DockBarPlacement::Over(edge, align));
            harness.frame(Vec::new());
            harness.frame(Vec::new());
            let bar = harness.rect(harness.find("bar.9"));
            let panel = harness.rect(harness.find("content.1"));
            assert_eq!(
                panel, body,
                "a bar floating over the {edge:?} edge leaves the body its room"
            );
            assert!(body.contains_rect(bar), "the bar floats inside the body");
            let near = match edge {
                DockEdge::Top => bar.top() - body.top(),
                DockEdge::Bottom => body.bottom() - bar.bottom(),
                DockEdge::Left => bar.left() - body.left(),
                DockEdge::Right => body.right() - bar.right(),
            };
            assert!(
                near > 0.0 && near < 16.0,
                "the bar keeps a margin from the {edge:?} edge, {near}"
            );
            let (start, centre, end) = match edge {
                DockEdge::Top | DockEdge::Bottom => (
                    bar.left() - body.left(),
                    bar.center().x - body.center().x,
                    body.right() - bar.right(),
                ),
                DockEdge::Left | DockEdge::Right => (
                    bar.top() - body.top(),
                    bar.center().y - body.center().y,
                    body.bottom() - bar.bottom(),
                ),
            };
            let placed = match align {
                DockAlign::Start => start > 0.0 && start < 16.0,
                DockAlign::Center => centre.abs() < 1.0,
                DockAlign::End => end > 0.0 && end < 16.0,
            };
            assert!(
                placed,
                "the bar sits at the {align:?} of the {edge:?} edge, at {bar:?} in {body:?}"
            );
            let lying = bar.width() > bar.height();
            assert_eq!(
                lying,
                matches!(edge, DockEdge::Top | DockEdge::Bottom),
                "the bar lays itself out along the {edge:?} edge"
            );
        }
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
