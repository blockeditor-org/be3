use super::*;

#[test]
fn a_stopped_plugins_view_keeps_the_place_it_is_shown_at() {
    let plugin = block_plugin_manifest::manifest_from_json(
        r#"{
            "id": "test.stopping",
            "name": "Stopping",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e418",
                "edits": "00000000-0000-4000-8000-0000000000c3",
                "display_name": "Stopping",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.stopping.wasm"
        }"#,
    )
    .expect("the manifest is valid");
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(plugin.identity.id.clone(), Runtime::new(&plugin, 0));
    });
    with("test.stopping", |_| -> u32 {
        panic!("the plugin went wrong")
    });

    let rect = Rect::from_min_size(pos2(497.0, 234.0), vec2(553.0, 388.0));
    let placed = EmbedPlacement {
        rect,
        clip: rect,
        radius: 0.0,
        raster: 1.0,
    };
    let view = placed_view(
        &plugin,
        EditorInstanceId(1),
        ViewId::MAIN,
        Some(placed),
        None,
    );
    assert!(view.error.is_some(), "the view shows the plugin stopped");
    assert_eq!(
        view.rect, rect,
        "a stopped view still says where it is, so input aimed through it lands in the right place"
    );
}
