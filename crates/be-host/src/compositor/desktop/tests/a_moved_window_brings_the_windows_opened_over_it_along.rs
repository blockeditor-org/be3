use be_sdk_api::{Size, ViewRef, WindowOpen, WindowOver, WindowPlace};
use beui::{PointerButton, PointerEvent, PointerId};

use super::*;

const APP: &str = "test.move";
const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn view(window: u64) -> ViewRef {
    ViewRef {
        instance: INSTANCE,
        view: match window {
            0 => ViewId::MAIN,
            window => ViewId(window),
        },
    }
}

fn open(window: u64, opener: u64) -> WindowRequest {
    WindowRequest::Open(WindowOpen {
        view: ViewId(window),
        screen: be_sdk_api::ScreenId::opened(window),
        opener: view(opener),
        placement: WindowPlacement::Over(WindowOver {
            view: view(opener),
            at: pos2(10.0, 10.0),
        }),
        size: Size {
            width: 80.0,
            height: 40.0,
        },
        level: be_sdk_api::WindowLevel::Normal,
    })
}

fn key(window: u64) -> FloatedKey {
    FloatedKey {
        app: APP.to_owned(),
        instance: INSTANCE,
        window: ViewId(window),
    }
}

fn corners() -> Vec<Pos2> {
    (1..=4)
        .map(|window| {
            rect_of(key(window))
                .get_untracked()
                .expect("the window is open")
                .min
        })
        .collect()
}

fn mouse(phase: PointerPhase, at: Pos2) -> ForwardedInput {
    ForwardedInput {
        events: vec![Forwarded::Pointer {
            event: PointerEvent {
                pointer: PointerId::Mouse,
                window: beui::WindowId::MAIN,
                phase,
                pos: at,
                modifiers: Default::default(),
                clicks: 1,
                force: None,
                pen: None,
                predicted: None,
                time: beui::reactive::now(),
                source: 0,
                rested: false,
            },
            over: None,
        }],
        rect: Rect::ZERO,
        hovered: true,
        focused: true,
        pointer: None,
        modifiers: Default::default(),
    }
}

#[test]
fn a_moved_window_brings_the_windows_opened_over_it_along() {
    let _root = beui::reactive::RootScope::new();
    install();
    crate::display::create_handle();
    note_origin(APP, INSTANCE, ViewId::MAIN, Pos2::ZERO);
    be_host_runtime::request_windows(APP, vec![open(1, 0), open(2, 1), open(3, 2), open(4, 0)]);
    apply();
    let before = corners();

    be_host_runtime::request_windows(
        APP,
        vec![WindowRequest::Place(WindowPlace {
            view: ViewId(1),
            placement: WindowPlacement::Over(WindowOver {
                view: view(0),
                at: before[0] + vec2(200.0, 100.0),
            }),
        })],
    );
    apply();
    let moved = vec2(200.0, 100.0);
    assert_eq!(
        corners(),
        [
            before[0] + moved,
            before[1] + moved,
            before[2] + moved,
            before[3]
        ],
        "a window placed elsewhere takes its dialog and the dialog's popup along, not a window opened beside it"
    );

    let placed = corners();
    let pressed = placed[0] + vec2(20.0, 5.0);
    follow(
        &key(1),
        mouse(PointerPhase::Down(PointerButton::Primary), pressed),
        Vec2::ZERO,
    );
    be_host_runtime::request_windows(APP, vec![WindowRequest::BeginMove(ViewId(1))]);
    apply();
    let dragged = vec2(-50.0, 30.0);
    follow(
        &key(1),
        mouse(PointerPhase::Move, pressed + dragged),
        Vec2::ZERO,
    );
    follow(
        &key(1),
        mouse(PointerPhase::Up(PointerButton::Primary), pressed + dragged),
        Vec2::ZERO,
    );
    assert_eq!(
        corners(),
        [
            placed[0] + dragged,
            placed[1] + dragged,
            placed[2] + dragged,
            placed[3]
        ],
        "dragged, it takes them along too"
    );
}
