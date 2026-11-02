use super::*;

#[test]
fn a_preview_is_not_drawn_through_the_frames_view() {
    let host = EditorHost::default();
    host.set_view(
        be_lib_geometry::Rect::from_min_size(
            be_lib_geometry::pos2(30.0, 20.0),
            be_lib_geometry::vec2(100.0, 50.0),
        ),
        2.0,
    );

    host.in_view(be_sdk_api::View::PREVIEW, || {
        assert_eq!(host.view(), None);
        assert_eq!(host.view_scale(), None);
    });

    host.in_view(be_sdk_api::View::MAIN, || {
        assert_eq!(host.view_scale(), Some(2.0));
    });
}
