use be_sdk_test::DocumentTest;
use be_sdk_test::prelude::Snapshots;
use beui_host::native::build;

use super::*;

/// The view is laid out larger than the placement its app last drew (it grows before the app
/// reports the new size): its app's clear colour fills the whole laid box, not only the old
/// placement, so nothing behind the view shows beyond it.
#[test]
fn a_view_grown_past_its_last_placement_shows_its_backdrop_there() {
    let _root = RootScope::new();
    let state = create_memo(|| PlacedView {
        rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 60.0)),
        clip: Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 60.0)),
        backdrop: Some(beui::color::Color32::from_rgb(200, 40, 40)),
        ..placed(Vec::new())
    });
    let document = build(move || {
        view! {
            <Root>
                <Frame width={Size::Custom(200.0)} height={Size::Custom(120.0)}>
                    <Layers>
                        <Backdrop
                            state
                            radius={create_memo(|| 0.0)}
                            upright={create_memo(|| true)}
                        />
                    </Layers>
                </Frame>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(240.0, 160.0));
    test.frame(Vec::new());
    test.snapshot("a_view_grown_past_its_last_placement_shows_its_backdrop_there");
}
