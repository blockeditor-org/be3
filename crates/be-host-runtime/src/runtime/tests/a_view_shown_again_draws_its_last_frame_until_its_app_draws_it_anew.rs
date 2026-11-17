use super::*;
use crate::EditorBlock;
use be_sdk_api::ViewPlacement;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const APP: &str = "test.kept";
/// A dock bar's view: shown only while its tab is, so it is hidden and shown again with it.
const BAR: View = View {
    id: ViewId(7),
    placement: ViewPlacement::Dock,
};

fn manifest() -> AppManifest {
    be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.kept",
            "name": "Kept",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e41f",
                "edits": "00000000-0000-4000-8000-0000000000d7",
                "display_name": "Kept",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.kept.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn show(mount: u64) {
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
            BAR,
            Uuid::nil(),
            role,
            &Arc::new(crate::AppCatalog::default()),
        )
    });
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(300.0, 40.0));
    place_view(
        APP,
        INSTANCE,
        BAR.id,
        mount,
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
}

fn laid_out(screens: Vec<be_sdk_api::ScreenPlacement>) {
    with(APP, |runtime| {
        runtime.place(be_sdk_api::ScreenLayout { screens })
    });
}

fn on(screen: ScreenId, surface: u32) -> be_sdk_api::ScreenPlacement {
    be_sdk_api::ScreenPlacement {
        screen,
        instance: INSTANCE,
        view: BAR.id,
        surface,
        width: 300,
        height: 40,
        scale_factor_millis: 1000,
        raster_millis: 1000,
    }
}

/// The surface the view draws, if it draws one.
fn shown_from() -> Option<u32> {
    with(APP, |runtime| {
        let view = shown_view(runtime, INSTANCE, BAR.id, None, None);
        match view.loading || view.base.is_empty() {
            true => None,
            false => view_placed_in(runtime, INSTANCE, BAR.id).map(|[surface, _, _]| surface),
        }
    })
    .flatten()
}

fn retained() -> Vec<u32> {
    with(APP, |runtime| runtime.retained_surfaces()).expect("the runtime is running")
}

/// Why a dock bar (the text editor's format bar) went blank for a frame on a tab switch: hidden
/// with its tab, the app stops drawing its view and lets its surface go, and shown again it had
/// nothing to draw until the app's next frame arrived, which a host that does not wait for it
/// (its clock running) showed a frame late.
#[test]
fn a_view_shown_again_draws_its_last_frame_until_its_app_draws_it_anew() {
    let app = manifest();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(app.identity.id.clone(), Runtime::new(&app, 0));
    });
    show(1);
    let screen = with(APP, |runtime| runtime.instances.screen_id(INSTANCE, BAR.id))
        .flatten()
        .expect("the view has a screen");
    laid_out(vec![on(screen, 3)]);
    assert_eq!(
        shown_from(),
        Some(3),
        "the view draws the surface the app laid it out on"
    );

    unmount_view(APP, INSTANCE, BAR.id, 1);
    laid_out(Vec::new());
    assert!(
        retained().contains(&3),
        "the hidden view's last frame is kept while the view may be shown again"
    );

    show(2);
    assert_eq!(
        shown_from(),
        Some(3),
        "shown again before its app has drawn it, the view draws its last frame rather than nothing"
    );

    laid_out(vec![on(screen, 4)]);
    assert_eq!(
        shown_from(),
        Some(3),
        "the app's new surface for it has no pixels yet, so the last frame stays"
    );
    assert!(
        retained().contains(&3) && retained().contains(&4),
        "both are held while the new one is on its way"
    );

    with(APP, |runtime| {
        runtime.present(PresentCommit {
            serial: 0,
            repaint_after_micros: None,
            animating: false,
            input_waiting: false,
            presented: Some(PresentedFrame {
                sequence: 1,
                damage: Vec::new(),
                painted: Vec::new(),
            }),
        });
    });
    assert_eq!(
        shown_from(),
        Some(3),
        "the app has presented its frame, but until the host has drawn it the last frame stays"
    );
    with(APP, |runtime| runtime.show_frame(frame(1)));
    assert_eq!(
        shown_from(),
        Some(4),
        "once the app's frame for it is drawn, the view shows it"
    );
    assert_eq!(retained(), vec![4], "and the last frame is let go");

    unmount_view(APP, INSTANCE, BAR.id, 2);
    laid_out(Vec::new());
    assert_eq!(
        retained(),
        vec![4],
        "hidden again, its last frame is kept again"
    );
    close(APP, INSTANCE);
    assert!(
        retained().is_empty(),
        "a view that goes for good lets its last frame go with it"
    );
}
