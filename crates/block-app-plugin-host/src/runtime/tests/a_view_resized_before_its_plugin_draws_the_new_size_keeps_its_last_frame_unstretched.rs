use super::*;
use crate::EditorBlock;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const PLUGIN: &str = "test.resized";

fn manifest() -> PluginManifest {
    block_plugin_manifest::manifest_from_json(
        r#"{
            "id": "test.resized",
            "name": "Resized",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e41a",
                "edits": "00000000-0000-4000-8000-0000000000d1",
                "display_name": "Resized",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.resized.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn place(rect: Rect) {
    place_view(
        PLUGIN,
        INSTANCE,
        ViewId::MAIN,
        1,
        ViewPlace {
            rect,
            clip: rect,
            zoom: 1.0,
            raster: 1.0,
            windows: false,
            placement: None,
            origin: None,
            anchor_bounds: None,
        },
        None,
        None,
    );
}

#[test]
fn a_view_resized_before_its_plugin_draws_the_new_size_keeps_its_last_frame_unstretched() {
    let plugin = manifest();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(plugin.identity.id.clone(), Runtime::new(&plugin, 0));
    });
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    with(PLUGIN, |runtime| {
        runtime.instances.mount(
            INSTANCE,
            View::MAIN,
            Uuid::nil(),
            role,
            &Arc::new(crate::PluginCatalog::default()),
        )
    });
    place(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 200.0)));
    let screen = with(PLUGIN, |runtime| {
        runtime.instances.screen_id(INSTANCE, ViewId::MAIN)
    })
    .flatten()
    .expect("the view has a screen");
    with(PLUGIN, |runtime| {
        runtime.laid_out(block_plugin_api::ScreenLayout {
            screens: vec![block_plugin_api::ScreenPlacement {
                screen,
                instance: INSTANCE,
                view: ViewId::MAIN,
                surface: 3,
                width: 300,
                height: 200,
                scale_factor_millis: 1000,
                raster_millis: 1000,
            }],
        });
    });

    let wider = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0));
    place(wider);
    let template =
        with(PLUGIN, |runtime| runtime.template(screen, None)).expect("the runtime is running");
    let requested = template
        .requested
        .expect("the view asks for the size it is now laid out at");
    assert_eq!(
        requested,
        (400, 200),
        "the drawing knows the size the view was just given, before the plugin hears of it"
    );
    let (quad, _) = presenter::unstretched(Quad::upright(wider), UNIT, requested, (300, 200))
        .expect("the last frame still shows");
    assert_eq!(
        quad.rect,
        Rect::from_min_size(Pos2::ZERO, vec2(300.0, 200.0)),
        "the last frame keeps its own size at the view's top left rather than stretching to the new width"
    );
}
