use super::*;

#[test]
fn a_joined_docks_body_is_its_owners_dock_view_framed_where_it_holds_the_content() {
    let mut body = placed(rect((20.0, 40.0), (320.0, 240.0)), ChildMode::Live);
    body.content = ChildContent::DockTab(be_sdk_api::DockTabPath {
        via: vec![ChildId(7)],
        tab: 4,
    });
    body.anchor = Some(rect((20.0, 20.0), (120.0, 40.0)));
    let ask = TabAsk {
        editor: EditorInstanceId(1),
        view: ViewId::MAIN,
        body: ChildId(7),
        via: vec![ChildId(7)],
        tab: 4,
        placement: body,
        raster: 2.0,
        space: None,
    };
    // The owner's view of that tab, placed by the dock, the size of the body's place.
    let screen = tab_screen(ScreenId(2), EditorInstanceId(9), &ask, false);
    assert_eq!(screen.screen, ScreenId(2));
    assert_eq!(screen.instance, EditorInstanceId(9));
    assert_eq!(screen.view, ViewId(4));
    assert_eq!(screen.placement, ViewPlacement::Dock);
    assert_eq!(
        (screen.metrics.pixel_width, screen.metrics.pixel_height),
        (600, 400)
    );
    // A bar, or a tab holding none of the editor's content, has no frame.
    assert_eq!(screen.frame, None);

    // The tab holding it has its chrome, and points what it opens at the tab.
    let frame = tab_screen(ScreenId(2), EditorInstanceId(9), &ask, true)
        .frame
        .expect("the view holding the content is framed");
    assert_eq!(frame.chrome, FrameChrome::Drawn);
    assert_eq!(frame.anchor, Some(rect((0.0, -20.0), (100.0, 0.0))));
}
