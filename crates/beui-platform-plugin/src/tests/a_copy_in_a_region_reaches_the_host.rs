use super::*;

#[test]
fn a_copy_in_a_region_reaches_the_host() {
    let host = EditorHost::default();
    let mut surface = PluginSurface::launch(
        host.clone(),
        beui::context(),
        Recording {
            laid: Rc::default(),
            copy: Some("copied".to_owned()),
        },
    );
    let whole = Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 30.0));
    surface.update(&region(whole, [40, 30]));
    assert_eq!(host.take_copied_text(), vec!["copied".to_owned()]);
}
