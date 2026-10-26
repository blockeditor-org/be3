use super::*;
use crate::plugin_host::EditorBlock;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: EditorRegion = EditorRegion::Frame;
const PLUGIN: &str = "test.rastered";

fn manifest() -> PluginManifest {
    block_plugin_api::manifest_from_json(
        r#"{
            "id": "test.rastered",
            "name": "Rastered",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e418",
                "edits": "00000000-0000-4000-8000-0000000000d1",
                "display_name": "Rastered",
                "icon": "x",
                "regions": ["Frame"]
            }},
            "entry_point": "test.rastered.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn sent() -> Option<(Vec2, [u32; 2], f32, f32)> {
    with(PLUGIN, |runtime| {
        runtime
            .instances
            .statuses(&runtime.layout, 0)
            .into_iter()
            .flat_map(|instance| instance.screens)
            .find(|screen| screen.region == REGION)
            .map(|screen| {
                (
                    screen.logical,
                    screen.pixels,
                    screen.scale_factor,
                    screen.raster,
                )
            })
    })
    .flatten()
}

#[test]
fn a_region_drawn_finer_than_its_window_is_sent_its_raster_beside_its_scale() {
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
            &Arc::new(crate::editors::PluginCatalog::default()),
        )
    });
    let rect = Rect::from_min_size(pos2(20.0, 30.0), vec2(300.0, 200.0));
    let place = |raster: f32| {
        place_region(
            PLUGIN,
            INSTANCE,
            REGION,
            1,
            RegionPlacement {
                rect,
                clip: rect,
                zoom: 1.0,
                raster,
            },
            None,
            None,
        );
    };

    place(1.0);
    assert_eq!(sent(), Some((rect.size(), [300, 200], 1.0, 1.0)));

    place(2.5);
    assert_eq!(
        sent(),
        Some((rect.size(), [750, 500], 1.0, 2.5)),
        "a region whose window is drawn 2.5 times as finely keeps its layout and scale, and is asked for 2.5 times the pixels"
    );
}
