use beui::{PointerButton, PointerEvent, PointerId, WindowPlacement as Placed};
use block_plugin_api::{RegionRef, Size, WindowOpen, WindowOver};

use super::*;
use crate::display::shown::ShownDisplay;

const PLUGIN: &str = "test.desktop";
const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn display(id: &str, rect: Rect, scale: f32) -> ShownDisplay {
    ShownDisplay {
        id: id.to_owned(),
        name: id.to_owned(),
        rect,
        scale,
        placement: Placed::AtPointer,
    }
}

fn place(rect: Rect, displays: &[Rect], scale: f32) -> DesktopPlace {
    let displays = displays.to_vec();
    DesktopPlace {
        display: create_memo(move || rect),
        displays: create_memo(move || displays.clone()),
        scale: create_memo(move || scale),
    }
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
fn a_window_shows_on_the_display_holding_most_of_it_and_on_both_while_it_is_moved() {
    let _root = beui::reactive::RootScope::new();
    install();
    crate::display::create_handle();
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let right = Rect::from_min_size(pos2(800.0, 0.0), vec2(640.0, 480.0));
    crate::display::shown::offer(vec![display("left", left, 1.0), display("right", right, 2.0)]);
    crate::display::apply_shown();
    let on_left = shown_on(place(left, &[left, right], 1.0), PLUGIN.to_owned(), INSTANCE);
    let on_right = shown_on(place(right, &[left, right], 2.0), PLUGIN.to_owned(), INSTANCE);
    note_origin(PLUGIN, INSTANCE, EditorRegion::Frame, left.min);
    request(
        PLUGIN,
        vec![WindowRequest::Open(WindowOpen {
            window: 7,
            opener: RegionRef {
                instance: INSTANCE,
                region: EditorRegion::Frame,
            },
            placement: WindowPlacement::Over(WindowOver {
                region: RegionRef {
                    instance: INSTANCE,
                    region: EditorRegion::Frame,
                },
                at: pos2(100.0, 100.0),
            }),
            size: Size {
                width: 300.0,
                height: 200.0,
            },
        })],
    );
    apply();
    assert_eq!(
        (on_left.get_untracked(), on_right.get_untracked()),
        (vec![7], vec![]),
        "a window opens where the region asked, on the display holding it"
    );
    let key = FloatedKey {
        plugin: PLUGIN.to_owned(),
        instance: INSTANCE,
        window: 7,
    };
    let origin = left.min.to_vec2();
    follow(&key, mouse(PointerPhase::Down(PointerButton::Primary), pos2(150.0, 110.0)), origin);
    request(PLUGIN, vec![WindowRequest::BeginMove(7)]);
    apply();
    follow(&key, mouse(PointerPhase::Move, pos2(700.0, 110.0)), origin);
    assert_eq!(
        rect_of(key.clone()).get_untracked().map(|rect| rect.min),
        Some(pos2(650.0, 100.0)),
        "the window keeps the point it was pressed at under the pointer"
    );
    assert_eq!(
        (on_left.get_untracked(), on_right.get_untracked()),
        (vec![7], vec![7]),
        "moving across the seam, it shows on both displays"
    );
    follow(&key, mouse(PointerPhase::Move, pos2(1000.0, 110.0)), origin);
    assert_eq!(
        on_left.get_untracked(),
        vec![7],
        "it stays on the display the move began on, which holds the press, until it is let go"
    );
    follow(&key, mouse(PointerPhase::Up(PointerButton::Primary), pos2(1000.0, 110.0)), origin);
    assert_eq!(
        (on_left.get_untracked(), on_right.get_untracked()),
        (vec![], vec![7]),
        "let go, it shows only on the display holding most of it"
    );
    assert_eq!(
        placement_of(&key).as_deref(),
        Some("desktop 950 100"),
        "and its placement is where it was let go"
    );
}
