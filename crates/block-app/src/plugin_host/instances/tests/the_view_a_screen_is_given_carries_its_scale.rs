use super::*;

#[test]
fn the_view_a_screen_is_given_carries_its_scale() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_view(
        INSTANCE,
        EditorView {
            rect: Rect::from_min_size(pos2(4.0, 6.0), vec2(200.0, 100.0)),
            scale: 0.5,
        },
    );

    let opened = instances.next_screens(PASS).opened;

    assert!(opened.iter().any(|message| matches!(
        message,
        Outbound::Editor(instance, EditorCall::ViewChanged { view, scale }) if *instance == INSTANCE
            && *view == Rect::from_min_size(pos2(4.0, 6.0), vec2(200.0, 100.0))
            && *scale == 0.5
    )));
    assert_eq!(
        instances.configure(&opened),
        Some(PluginCall::Configure(1)),
        "a view is host state a plugin lays out against, so sending one starts a new serial"
    );
}
