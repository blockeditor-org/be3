use be_lib_geometry::pos2;
use be_lib_layers::{Area, Layer};
use be_sdk_root_testing::{TOAST_AREA, TOAST_LAYER, TOAST_SURFACE};
use be_wasm_root_api::{Notice, NoticeAction};

use super::*;

/// Well past the time an ordinary toast shows for (beui's `TOAST_DURATION`), in nanoseconds.
const PAST_ITS_TIME: u64 = 60_000_000_000;

#[test]
fn a_root_shows_its_hosts_notices_as_toasts_hit_only_where_they_are() {
    let mut root = embedding();
    let toasts = |done: &FrameDone| -> Option<Layer> {
        LayerList::decode(&done.layers)
            .expect("the frame is a layer list")
            .windows
            .into_iter()
            .flat_map(|window| window.layers)
            .find(|layer| layer.id == TOAST_LAYER)
    };
    let first = draw(&mut root, 1);
    assert!(
        toasts(&first).is_none(),
        "with no notice, no toasts' layer shows or takes input"
    );

    // The host shows a failure, which offers its report, and a refusal.
    root.show_notices(vec![
        Notice {
            id: 1,
            message: "This device could not save the draft.".to_owned(),
            failure: true,
            actions: vec![NoticeAction {
                key: "copy-report".to_owned(),
                label: "Copy report".to_owned(),
            }],
        },
        Notice {
            id: 2,
            message: "Only the owner can rename this folder.".to_owned(),
            failure: false,
            actions: Vec::new(),
        },
    ]);
    // The root hears them as it steps, and draws them in a frame it then asks for.
    let drawn = (2..12)
        .map(|time| draw(&mut root, time))
        .find(|done| toasts(done).is_some())
        .expect("the root shows its host's notices within a few frames");
    let layer = toasts(&drawn).expect("the toasts' layer shows");
    assert_eq!(
        shown(&drawn).last(),
        Some(&TOAST_LAYER),
        "the toasts show over everything else"
    );
    assert!(
        !layer.input.focuses,
        "a press on a toast never takes the focus"
    );
    let Area::Region(rects) = &layer.input.hit else {
        panic!(
            "the toasts' layer is hit only where a toast is: {:?}",
            layer.input.hit
        );
    };
    let hit = |x: f32, y: f32| rects.iter().any(|rect| rect.contains(pos2(x, y)));
    let (width, _) = TOAST_AREA;
    assert!(
        hit(width as f32 - 40.0, 40.0),
        "a toast at the top right takes a press there: {rects:?}"
    );
    assert!(
        !hit(10.0, 10.0) && !hit(width as f32 - 40.0, 230.0),
        "past the toasts, a press reaches what lies under them: {rects:?}"
    );

    // What beui drew: the failure as an alert with its action, the refusal under it.
    let painting = own_paintings(&mut root, 11)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(TOAST_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its toasts");
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_notices_as_toasts_hit_only_where_they_are",
        painting,
    );
    assert!(
        root.take_answers().is_empty(),
        "nothing answered the notices yet"
    );

    // The refusal goes when its time is up, which the root tells its host; the failure stays
    // until the person answers it.
    let answers: Vec<_> = (0..10)
        .flat_map(|step| {
            draw(&mut root, PAST_ITS_TIME + step);
            root.take_answers()
        })
        .collect();
    assert_eq!(
        answers,
        vec![(2, None)],
        "the refusal is dismissed once its time is up, and the failure is not"
    );
}

/// Draws a frame for the frame clock's `time`.
fn draw(root: &mut Root, time: u64) -> FrameDone {
    root.frame(
        FrameRequest {
            time,
            wall_clock: 0,
        },
        Instant::now() + PATIENT,
    )
    .expect("the root draws")
}
