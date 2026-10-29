use super::*;
use crate::EditorBlock;
use block_plugin_api::{Layer, LayerParent, LayerPlacement};

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: EditorRegion = EditorRegion::Frame;
const PLUGIN: &str = "test.layered";

fn manifest() -> PluginManifest {
    block_plugin_manifest::manifest_from_json(
        r#"{
            "id": "test.layered",
            "name": "Layered",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e419",
                "edits": "00000000-0000-4000-8000-0000000000d1",
                "display_name": "Layered",
                "icon": "x",
                "regions": ["Frame"]
            }},
            "entry_point": "test.layered.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn sheet(blocks_owner: bool, keyboard: bool) -> Layer {
    let rect = Rect::from_min_size(pos2(0.0, 400.0), vec2(800.0, 200.0));
    Layer {
        id: LayerId(1),
        parent: LayerParent::Region,
        placement: LayerPlacement::Edge(block_plugin_api::LayerEdge::Bottom),
        rect,
        surface: 3,
        painted: vec![rect],
        floating: vec![rect],
        scrims: Vec::new(),
        dismiss_on_outside_press: false,
        keyboard,
        blocks_owner,
    }
}

fn commit(serial: u64, layers: Vec<Layer>) {
    with(PLUGIN, |runtime| {
        let screen = runtime
            .instances
            .screen_id(INSTANCE, REGION)
            .expect("the region has a screen");
        runtime.commit(FrameCommit {
            serial,
            sizes: Vec::new(),
            reports: vec![block_plugin_api::FrameReport {
                screen,
                content: block_plugin_api::Insets::default(),
                painted: Vec::new(),
                floating: Vec::new(),
                floating_painted: Vec::new(),
                scrims: Vec::new(),
                drawing: Vec::new(),
                every_sample: false,
                claims: Vec::new(),
                description: None,
                layers,
            }],
            windows: Vec::new(),
            children: Vec::new(),
            repaint_after_micros: None,
            animating: false,
            presented: None,
            window_requests: Vec::new(),
            docks: Vec::new(),
        });
    });
}

#[test]
fn a_region_hears_where_its_layers_go_and_a_layer_can_block_it_or_take_its_keyboard() {
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
            REGION,
            Uuid::nil(),
            role,
            &Arc::new(crate::PluginCatalog::default()),
        )
    });
    let rect = Rect::from_min_size(pos2(20.0, 30.0), vec2(300.0, 200.0));
    let window = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    place_region(
        PLUGIN,
        INSTANCE,
        REGION,
        1,
        RegionPlacement {
            rect,
            clip: rect,
            zoom: 2.0,
            raster: 1.0,
            windows: false,
            placement: None,
            layer_bounds: Some(window),
        },
        None,
        None,
    );

    let requested = with(PLUGIN, |runtime| {
        runtime
            .instances
            .next_screens(0)
            .screens
            .into_iter()
            .find(|request| request.region == REGION)
            .and_then(|request| request.layers)
    })
    .flatten();
    assert_eq!(
        requested,
        Some(LayerSpace {
            to_host: HostTransform {
                origin: rect.min,
                scale: vec2(2.0, 2.0),
            },
            bounds: window,
        }),
        "a region shown at twice its size maps its points to the window at twice their size, and may open layers anywhere in the window"
    );

    let view = || {
        with(PLUGIN, |runtime| placed_view(runtime, INSTANCE, REGION, None, None))
            .expect("the runtime is running")
    };
    assert!(view().layers.is_empty());
    assert!(!view().blocks_owner && !view().wants_keyboard);

    commit(1, vec![sheet(true, false)]);
    let shown = view();
    assert_eq!(shown.layers.len(), 1, "the commit that opens a layer shows it");
    assert!(
        shown.blocks_owner,
        "a layer that blocks its owner keeps presses from the region's content"
    );
    assert!(!shown.wants_keyboard);

    commit(2, vec![sheet(false, true)]);
    let shown = view();
    assert!(!shown.blocks_owner);
    assert!(
        shown.wants_keyboard,
        "a layer that takes the keyboard gives its owner the keyboard"
    );
}
