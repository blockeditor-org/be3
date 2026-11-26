use be_lib_geometry::{Rect, pos2, vec2};
use be_sdk_api::{
    EditorInstanceId, ParentView, ScreenId, ScreenRequest, ViewId, ViewPlacement, ViewRef,
    WindowEdge, WindowLevel, WindowOpen, WindowOver, WindowPlace, WindowPlacement, WindowRequest,
    WindowResize, WindowResizeBegin,
};

use super::*;

mod a_window_closed_leaves_the_windows_opened_over_it;
mod a_window_is_kept_within_its_openers_work_area_but_while_it_moves;
mod a_window_opened_at_the_pointer_opens_under_it;
mod a_window_opened_from_a_view_asked_without_windows_is_not_floated;
mod a_window_opens_over_its_opener_and_is_asked_for_at_its_size_until_it_closes;
mod a_window_placed_and_resized_is_asked_for_where_it_went_and_at_its_new_size;
mod a_window_pressed_and_asked_to_move_follows_the_pointer_from_the_press;
mod raising_a_window_puts_it_on_top;

const EDITOR: EditorInstanceId = EditorInstanceId(3);

/// The editor's main view, asked for filling a window of 800 by 600 points at 2 pixels a point,
/// with windows as `windows` says, anchoring views in the window's points from `origin`.
fn main_screen(windows: bool) -> ScreenRequest {
    ScreenRequest {
        screen: ScreenId(1),
        instance: EDITOR,
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: None,
        metrics: ViewportMetrics {
            logical: vec2(800.0, 600.0),
            visible: Pos2::ZERO,
            pixel_width: 1600,
            pixel_height: 1200,
            scale_factor: 2.0,
            raster: 2.0,
        },
        windows,
        host_place: None,
        docked: false,
        anchor_space: Some(AnchorSpace {
            to_host: HostTransform {
                origin: pos2(0.0, 0.0),
                scale: vec2(1.0, 1.0),
            },
            bounds: Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0)),
        }),
        fit: false,
        origin: None,
    }
}

/// The editor opening its view `view` as a window of 200 by 100 points, opened with screen
/// `screen`, with its corner at `at` in its main view.
fn open(view: u64, screen: u64, at: Pos2) -> WindowRequest {
    WindowRequest::Open(WindowOpen {
        view: ViewId(view),
        screen: ScreenId::opened(screen),
        opener: ViewRef {
            instance: EDITOR,
            view: ViewId::MAIN,
        },
        placement: WindowPlacement::Over(WindowOver {
            view: ViewRef {
                instance: EDITOR,
                view: ViewId::MAIN,
            },
            at,
        }),
        size: vec2(200.0, 100.0),
        level: WindowLevel::Normal,
    })
}
