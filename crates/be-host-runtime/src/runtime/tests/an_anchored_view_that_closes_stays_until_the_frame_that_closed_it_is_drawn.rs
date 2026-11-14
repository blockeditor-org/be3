use super::*;
use crate::EditorBlock;
use be_sdk_api::{AnchoredPlacement, PresentedFrame, ViewAnchor};

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const APP: &str = "test.lifted";

fn manifest() -> AppManifest {
    be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.lifted",
            "name": "Lifted",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e41c",
                "edits": "00000000-0000-4000-8000-0000000000d3",
                "display_name": "Lifted",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.lifted.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn layout(lifted: bool) -> be_sdk_api::ScreenLayout {
    let placement = |screen, view, surface| be_sdk_api::ScreenPlacement {
        screen,
        instance: INSTANCE,
        view,
        surface,
        width: 200,
        height: 40,
        scale_factor_millis: 1000,
        raster_millis: 1000,
    };
    be_sdk_api::ScreenLayout {
        screens: match lifted {
            true => vec![placement(ScreenId::opened(1), ViewId(1), 3)],
            false => Vec::new(),
        },
    }
}

fn report(anchored: Option<AnchoredPlacement>) -> be_sdk_api::FrameReport {
    be_sdk_api::FrameReport {
        screen: ScreenId::opened(1),
        content: None,
        painted: Vec::new(),
        floating: Vec::new(),
        floating_painted: Vec::new(),
        marks: Vec::new(),
        marks_painted: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        description: None,
        actions: None,
        anchored,
        work_area: None,
        backdrop: None,
    }
}

fn laid_out(serial: u64) -> Heard {
    Heard::LaidOut(Box::new(LayoutCommit {
        serial,
        sizes: Vec::new(),
        reports: Vec::new(),
        windows: Vec::new(),
        children: Vec::new(),
        window_requests: Vec::new(),
        docks: Vec::new(),
    }))
}

fn presented(serial: u64, sequence: Option<u64>) -> Heard {
    Heard::Presented(PresentCommit {
        serial,
        repaint_after_micros: None,
        animating: false,
        input_waiting: false,
        presented: sequence.map(|sequence| PresentedFrame {
            sequence,
            damage: Vec::new(),
        }),
    })
}

fn lifted_shown() -> bool {
    with(APP, |runtime| {
        runtime.layout.placement(ScreenId::opened(1)).is_some()
            && !runtime
                .instances
                .shown_anchored(INSTANCE, ViewId::MAIN)
                .is_empty()
    })
    .unwrap_or(false)
}

#[test]
fn an_anchored_view_that_closes_stays_until_the_frame_that_closed_it_is_drawn() {
    let app = manifest();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(app.identity.id.clone(), Runtime::new(&app, 0));
    });
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    with(APP, |runtime| {
        runtime.taking_in = true;
        runtime.instances.mount(
            INSTANCE,
            View::MAIN,
            Uuid::nil(),
            role,
            &Arc::new(crate::AppCatalog::default()),
        );
    });
    let rect = Rect::from_min_size(pos2(20.0, 30.0), vec2(300.0, 200.0));
    place_view(
        APP,
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
            anchor_bounds: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            fit: None,
        },
        None,
        None,
    );
    with(APP, |runtime| {
        runtime.laid_out(layout(true));
        runtime.commit(LayoutCommit {
            serial: 1,
            sizes: Vec::new(),
            reports: vec![report(Some(AnchoredPlacement {
                parent: ViewId::MAIN,
                anchor: ViewAnchor::Rect(Rect::from_min_size(pos2(40.0, 40.0), vec2(1.0, 1.0))),
                rect: Rect::from_min_size(pos2(40.0, 40.0), vec2(200.0, 40.0)),
                dismiss_on_outside_press: false,
                trigger: None,
                keyboard: false,
                blocks_owner: false,
            }))],
            windows: Vec::new(),
            children: Vec::new(),
            window_requests: Vec::new(),
            docks: Vec::new(),
        });
    });
    assert!(lifted_shown(), "the commit that opens the layer shows it");

    with(APP, |runtime| {
        runtime.hear(vec![Heard::Layout(layout(false)), laid_out(2)]);
    });
    assert!(
        lifted_shown(),
        "a layer closed in a frame stays while the frame that shows what it held elsewhere is unpainted"
    );
    with(APP, |runtime| runtime.hear(vec![presented(2, Some(1))]));
    assert!(
        lifted_shown(),
        "presented, it stays until that frame's pixels arrive"
    );
    with(APP, |runtime| {
        runtime.presents.damage_through(1);
        runtime.close_anchored();
    });
    assert!(!lifted_shown(), "and goes with them");

    with(APP, |runtime| {
        runtime.laid_out(layout(true));
        runtime.commit(LayoutCommit {
            serial: 3,
            sizes: Vec::new(),
            reports: vec![report(Some(AnchoredPlacement {
                parent: ViewId::MAIN,
                anchor: ViewAnchor::Rect(Rect::from_min_size(pos2(40.0, 40.0), vec2(1.0, 1.0))),
                rect: Rect::from_min_size(pos2(40.0, 40.0), vec2(200.0, 40.0)),
                dismiss_on_outside_press: false,
                trigger: None,
                keyboard: false,
                blocks_owner: false,
            }))],
            windows: Vec::new(),
            children: Vec::new(),
            window_requests: Vec::new(),
            docks: Vec::new(),
        });
        runtime.hear(vec![
            Heard::Layout(layout(false)),
            laid_out(4),
            presented(4, None),
        ]);
    });
    assert!(
        !lifted_shown(),
        "closed in a frame that paints nothing new, it goes with that frame's present"
    );
}
