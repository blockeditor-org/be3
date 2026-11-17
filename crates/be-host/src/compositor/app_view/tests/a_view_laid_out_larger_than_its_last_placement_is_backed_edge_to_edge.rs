use be_sdk_test::DocumentTest;
use be_sdk_test::prelude::Snapshots;
use beui_host::native::build;

use super::*;

#[test]
fn a_view_laid_out_larger_than_its_last_placement_is_backed_edge_to_edge() {
    let _root = RootScope::new();
    // The view's last placement is smaller than the box it is laid out in now, as it is for a
    // frame while a window grows: the backdrop still fills the whole box, so the space beside
    // the app's last painting (shown at its own size) is the app's color, never what is behind.
    let state = create_signal(PlacedView {
        rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(180.0, 100.0)),
        clip: Rect::from_min_size(Pos2::ZERO, Vec2::new(180.0, 100.0)),
        backdrop: Some(beui::color::Color32::from_rgb(40, 90, 160)),
        reported: true,
        ..placed(Vec::new())
    });
    let document = build(move || {
        let state = create_memo(move || state.get());
        let radius = create_memo(|| 0.0);
        let upright = create_memo(|| true);
        view! {
            <Root clear_color=ColorRole::Danger>
                <Backdrop state radius upright />
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(300.0, 200.0));
    test.frame(Vec::new());
    test.snapshot("a_view_laid_out_larger_than_its_last_placement_is_backed_edge_to_edge");
}
