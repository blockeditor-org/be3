use be_sdk_api::{Size, ViewRef, WindowOver};

use super::*;
use crate::display::shown::ShownDisplay;

const APP: &str = "test.work-area";
const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const WINDOW: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1100.0, 720.0));
const WORK: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1100.0, 679.0));

fn open(window: u64, at: Pos2, size: Vec2) {
    let main = ViewRef {
        instance: INSTANCE,
        view: ViewId::MAIN,
    };
    be_host_runtime::request_windows(
        APP,
        vec![WindowRequest::Open(WindowOpen {
            view: ViewId(window),
            screen: be_sdk_api::ScreenId::opened(window),
            opener: main,
            placement: WindowPlacement::Over(WindowOver { view: main, at }),
            size: Size {
                width: size.x,
                height: size.y,
            },
            level: be_sdk_api::WindowLevel::Normal,
        })],
    );
    apply();
}

fn rect(window: u64) -> Rect {
    rect_of(FloatedKey {
        app: APP.to_owned(),
        instance: INSTANCE,
        window: ViewId(window),
    })
    .get_untracked()
    .expect("the window is open")
}

fn display(id: &str, rect: Rect) -> ShownDisplay {
    ShownDisplay {
        id: id.to_owned(),
        name: id.to_owned(),
        rect,
        scale: 1.0,
        placement: beui::WindowPlacement::AtPointer,
    }
}

#[test]
fn a_desktop_window_is_kept_in_the_work_area_in_a_window_and_off_an_unplugged_display() {
    let _root = beui::reactive::RootScope::new();
    install();
    crate::display::create_handle();
    hold_window_display(create_memo(|| WINDOW));
    note_origin(APP, INSTANCE, ViewId::MAIN, Pos2::ZERO);
    note_work_area((APP.to_owned(), INSTANCE, ViewId::MAIN), Some(WORK));
    beui::reactive::flush();

    open(1, pos2(80.0, 560.0), vec2(400.0, 300.0));
    assert_eq!(
        rect(1),
        Rect::from_min_size(pos2(80.0, 379.0), vec2(400.0, 300.0)),
        "in be-host's own window a new window is lifted clear of the band at its foot"
    );
    open(2, pos2(80.0, 210.0), vec2(800.0, 700.0));
    assert_eq!(
        rect(2),
        Rect::from_min_size(pos2(80.0, 0.0), vec2(800.0, 679.0)),
        "one taller than the work area shrinks to it"
    );

    let second = Rect::from_min_size(pos2(1100.0, 0.0), vec2(1100.0, 720.0));
    crate::display::shown().set(vec![display("one", WINDOW), display("two", second)]);
    beui::reactive::flush();
    open(3, pos2(1500.0, 200.0), vec2(400.0, 300.0));
    assert_eq!(
        rect(3).min,
        pos2(1500.0, 200.0),
        "a window on the second display stays there"
    );
    crate::display::shown().set(vec![display("one", WINDOW)]);
    beui::reactive::flush();
    assert_eq!(
        rect(3),
        Rect::from_min_size(pos2(700.0, 200.0), vec2(400.0, 300.0)),
        "unplugging its display brings it wholly onto the one left"
    );
}
