use super::*;
use crate::plugin_host::EditorBlock;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: EditorRegion = EditorRegion::Preview;

fn manifest() -> PluginManifest {
    block_plugin_api::manifest_from_json(
        r#"{
            "id": "test.previewed",
            "name": "Previewed",
            "version": "0.1.0",
            "editors": [{
                "block_type": "00000000-0000-4000-8000-0000000000d1",
                "display_name": "Previewed",
                "icon": "x",
                "regions": ["Frame", "Preview"]
            }],
            "entry_point": "test.previewed.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn placement(rect: Rect, clip: Rect) -> RegionPlacement {
    RegionPlacement {
        rect,
        clip,
        zoom: 1.0,
        screens: Vec::new(),
    }
}

fn chosen() -> Option<Rect> {
    with("test.previewed", |runtime| {
        runtime
            .placements
            .get(&(INSTANCE, REGION))
            .map(|placed| placed.rect)
    })
    .flatten()
}

fn pieces(rect: Rect, clip: Rect) -> Vec<(Rect, Rect)> {
    with("test.previewed", |runtime| {
        placed_view(
            runtime,
            INSTANCE,
            REGION,
            Some(EmbedPlacement { rect, clip }),
            None,
        )
        .base
        .into_iter()
        .map(|piece| (piece.local, piece.source))
        .collect()
    })
    .unwrap_or_default()
}

#[test]
fn a_preview_shown_twice_is_drawn_at_its_larger_place_and_each_shows_its_own_part() {
    let plugin = manifest();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(plugin.identity.id.clone(), Runtime::new(&plugin, 0));
    });
    let block_types = Arc::new(block_plugin_api::Catalog::default());
    let role = InstanceRole::Editor(EditorBlock {
        id: Uuid::nil(),
        block_type: Uuid::nil(),
        view_block: None,
    });
    for _ in 0..2 {
        with("test.previewed", |runtime| {
            runtime
                .instances
                .mount(INSTANCE, REGION, Uuid::nil(), role, &block_types)
        });
    }
    let large = Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 200.0));
    let shown = Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 100.0));
    let small = Rect::from_min_size(pos2(10.0, 10.0), vec2(60.0, 40.0));

    place_region(
        "test.previewed",
        INSTANCE,
        REGION,
        1,
        placement(large, shown),
        None,
        None,
    );
    place_region(
        "test.previewed",
        INSTANCE,
        REGION,
        2,
        placement(small, small),
        None,
        None,
    );

    assert_eq!(
        chosen(),
        Some(large),
        "the preview is drawn at the larger of the places it is shown, not the last one placed"
    );
    let top = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 0.5));
    assert_eq!(
        pieces(large, shown),
        vec![(top, top)],
        "the clipped place shows only its top half, from the top half of the drawing"
    );
    assert_eq!(
        pieces(small, small),
        vec![(UNIT, UNIT)],
        "the place shown whole shows the whole drawing, which was drawn for both"
    );

    unplace_region("test.previewed", INSTANCE, REGION, 1);

    assert_eq!(
        chosen(),
        Some(small),
        "once the larger place goes, the preview is drawn for the one left"
    );
}
