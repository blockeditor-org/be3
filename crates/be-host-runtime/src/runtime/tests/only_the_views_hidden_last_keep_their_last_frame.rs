use super::*;
use crate::EditorBlock;
use be_sdk_api::ViewPlacement;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const APP: &str = "test.capped";

fn bar(index: u64) -> View {
    View {
        id: ViewId(100 + index),
        placement: ViewPlacement::Dock,
    }
}

fn manifest() -> AppManifest {
    be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.capped",
            "name": "Kept",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e420",
                "edits": "00000000-0000-4000-8000-0000000000d7",
                "display_name": "Kept",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.capped.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn show(view: View) -> ScreenId {
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    with(APP, |runtime| {
        runtime.instances.mount(
            INSTANCE,
            view,
            Uuid::nil(),
            role,
            &Arc::new(crate::AppCatalog::default()),
        )
    });
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(300.0, 40.0));
    place_view(
        APP,
        INSTANCE,
        view.id,
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
            fit: None,
        },
        None,
        None,
    );
    with(APP, |runtime| {
        runtime.instances.screen_id(INSTANCE, view.id)
    })
    .flatten()
    .expect("the view has a screen")
}

fn on(screen: ScreenId, view: View, surface: u32) -> be_sdk_api::ScreenPlacement {
    be_sdk_api::ScreenPlacement {
        screen,
        instance: INSTANCE,
        view: view.id,
        surface,
        width: 300,
        height: 40,
        scale_factor_millis: 1000,
        raster_millis: 1000,
    }
}

fn retained() -> Vec<u32> {
    let mut held =
        with(APP, |runtime| runtime.shown.retained_surfaces(&runtime.layout)).expect("the runtime is running");
    held.sort_unstable();
    held
}

/// Each hidden view keeping its last frame for as long as its instance lives would hold a texture
/// for every tab it ever showed; only the views hidden last keep theirs.
#[test]
fn only_the_views_hidden_last_keep_their_last_frame() {
    let app = manifest();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(app.identity.id.clone(), Runtime::new(&app, 0));
    });
    let views: Vec<(View, ScreenId)> = (0..9).map(|index| (bar(index), show(bar(index)))).collect();
    let all: Vec<be_sdk_api::ScreenPlacement> = views
        .iter()
        .enumerate()
        .map(|(index, (view, screen))| on(*screen, *view, 10 + index as u32))
        .collect();
    with(APP, |runtime| {
        runtime.place(be_sdk_api::ScreenLayout {
            screens: all.clone(),
        })
    });

    for (index, (view, _)) in views.iter().enumerate() {
        unmount_view(APP, INSTANCE, view.id, 1);
        let left: Vec<_> = all[index + 1..].to_vec();
        with(APP, |runtime| {
            runtime.place(be_sdk_api::ScreenLayout { screens: left })
        });
    }
    assert_eq!(
        retained(),
        (11..19).collect::<Vec<u32>>(),
        "of nine views hidden, the eight hidden last keep their frame and the first lets its go"
    );

    // Show again the view hidden longest that kept its frame, in the layout that hides one more.
    let (back, back_screen) = views[1];
    show(back);
    let tenth = bar(9);
    let tenth_screen = show(tenth);
    with(APP, |runtime| {
        runtime.place(be_sdk_api::ScreenLayout {
            screens: vec![on(tenth_screen, tenth, 30)],
        })
    });
    unmount_view(APP, INSTANCE, tenth.id, 1);
    with(APP, |runtime| {
        runtime.place(be_sdk_api::ScreenLayout {
            screens: vec![on(back_screen, back, 31)],
        })
    });
    assert!(
        retained().contains(&11),
        "a view the layout shows again keeps its last frame though more were hidden since"
    );
    close(APP, INSTANCE);
    assert!(
        !retained().iter().any(|surface| (10..31).contains(surface)),
        "and every kept frame goes with its instance"
    );
}
