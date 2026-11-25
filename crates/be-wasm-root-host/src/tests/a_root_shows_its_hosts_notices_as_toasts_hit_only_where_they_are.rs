use be_lib_geometry::pos2;
use be_lib_layers::{Area, Layer};
use be_lib_ui_types::input::{Input, PointerButton, encode};
use be_sdk_root_testing::{TEXT_LAYER, TOAST_AREA, TOAST_LAYER, TOAST_SURFACE};
use be_wasm_root_api::{Notice, NoticeAction};

use super::*;

/// Well past the time an ordinary toast shows for (beui's `TOAST_DURATION`), in nanoseconds.
const PAST_ITS_TIME: u64 = 60_000_000_000;
/// Long enough a frame, in nanoseconds, that a few of them see a toast slide all the way in
/// or out.
const FRAME: u64 = 100_000_000;
/// When the toasts have slid in and rest, well before an ordinary one's time is up.
const SETTLED: u64 = 1_000_000_000;

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
    (2..12)
        .map(|time| draw(&mut root, time))
        .find(|done| toasts(done).is_some())
        .expect("the root shows its host's notices within a few frames");
    // They slide in from the side; once they rest, they are hit where they are drawn.
    let drawn = draw(&mut root, SETTLED);
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
    // As the root's router hits its layers: the strip of text under the toasts' surface takes
    // a press beside them, and the toasts one on them.
    let windows = LayerList::decode(&drawn.layers).expect("the frame is a layer list");
    let layers = &windows.windows[0].layers;
    let under = |x: f32, y: f32| {
        layers
            .iter()
            .rev()
            .find(|layer| layer.shown && layer.takes(pos2(x, y)))
            .map(|layer| layer.id)
    };
    assert_eq!(
        under(10.0, 10.0),
        Some(TEXT_LAYER),
        "a press beside the toasts reaches the strip of text under them"
    );
    assert_eq!(under(width as f32 - 40.0, 40.0), Some(TOAST_LAYER));
    // The first toast, the failure, is at the top right: its close button at its top right,
    // its action under it, where the accepted painting shows them.
    let close = pos2(width as f32 - 40.0, 36.0);
    let copy = pos2(width as f32 - 84.0, 72.0);

    // What beui drew: the failure as an alert with its action, the refusal under it.
    let painting = own_paintings(&mut root, SETTLED + 1)
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

    // Pressed through the root's router: the failure's action, then its close button.
    let mut time = SETTLED + 2;
    let mut press = |root: &mut Root, at| {
        root.input(
            None,
            encode(&[
                Input::PointerMoved(at).into(),
                Input::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed: true,
                }
                .into(),
                Input::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed: false,
                }
                .into(),
            ]),
        )
        .expect("the root takes input");
        (0..5)
            .flat_map(|_| {
                time += FRAME;
                draw(root, time);
                root.take_answers()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        press(&mut root, copy),
        vec![(1, Some("copy-report".to_owned()))],
        "pressing the failure's action tells the host which, for it to do"
    );
    assert_eq!(
        press(&mut root, close),
        vec![(1, None)],
        "pressing its close button dismisses it"
    );

    // The refusal goes when its time is up, which the root tells its host; the failure stays
    // until the person answers it.
    let answers: Vec<_> = (0..10)
        .flat_map(|step| {
            draw(&mut root, PAST_ITS_TIME + step * FRAME);
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
            utc_offset: 0,
            window: window(),
            describe: false,
        },
        Instant::now() + PATIENT,
    )
    .expect("the root draws")
}
