use super::*;

#[test]
fn a_preview_is_not_drawn_through_the_frames_view() {
    let host = EditorHost::default();
    host.set_view(
        geometry::Rect::from_min_size(geometry::pos2(30.0, 20.0), geometry::vec2(100.0, 50.0)),
        2.0,
    );

    host.in_region(EditorRegion::Preview, || {
        assert_eq!(host.view(), None);
        assert_eq!(host.view_scale(), None);
    });

    host.in_region(EditorRegion::Frame, || {
        assert_eq!(host.view_scale(), Some(2.0));
    });
}
