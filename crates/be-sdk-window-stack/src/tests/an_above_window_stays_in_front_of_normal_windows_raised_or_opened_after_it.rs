use super::*;

#[derive(Debug)]
struct Shown {
    view: ViewId,
    rect: Rect,
    above: bool,
    parent: Option<ViewId>,
}

impl Stacked for Shown {
    fn view(&self) -> ViewId {
        self.view
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn wanted(&self) -> Rect {
        self.rect
    }

    fn show_at(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn opened_over(&self, other: &Self) -> bool {
        self.parent == Some(other.view)
    }

    fn above(&self) -> bool {
        self.above
    }
}

struct Desk;

impl Host<Shown> for Desk {
    fn mine(&self, _: &Shown) -> bool {
        true
    }

    fn pointer(&self) -> Pos2 {
        pos2(200.0, 200.0)
    }

    fn over(&self, _: ViewRef, at: Pos2) -> Pos2 {
        at
    }

    fn open(
        &mut self,
        open: &WindowOpen,
        rect: Rect,
        _: Option<&Shown>,
        opener: Option<&Shown>,
    ) -> Shown {
        Shown {
            view: open.view,
            rect,
            above: open.level == be_sdk_api::WindowLevel::Above,
            parent: opener.map(|opener| opener.view),
        }
    }
}

fn open(view: u64, opener: u64, level: be_sdk_api::WindowLevel) -> WindowRequest {
    WindowRequest::Open(WindowOpen {
        view: ViewId(view),
        screen: be_sdk_api::ScreenId(view),
        opener: ViewRef {
            instance: be_sdk_api::EditorInstanceId(1),
            view: ViewId(opener),
        },
        placement: WindowPlacement::AtPointer,
        size: be_sdk_api::Size {
            width: 100.0,
            height: 80.0,
        },
        level,
    })
}

fn order(windows: &[Shown]) -> Vec<u64> {
    windows.iter().map(|window| window.view.0).collect()
}

#[test]
fn an_above_window_stays_in_front_of_normal_windows_raised_or_opened_after_it() {
    use be_sdk_api::WindowLevel::{Above, Normal};
    let mut windows = Vec::new();
    ask(&mut windows, &mut Desk, open(1, 0, Normal));
    ask(&mut windows, &mut Desk, open(2, 0, Above));
    ask(&mut windows, &mut Desk, open(3, 0, Normal));
    assert_eq!(
        order(&windows),
        [1, 3, 2],
        "a normal window opened after the bar opens beneath it"
    );
    ask(&mut windows, &mut Desk, WindowRequest::Raise(ViewId(1)));
    assert_eq!(
        order(&windows),
        [3, 1, 2],
        "a raised normal window comes in front of the other normal windows only"
    );
    ask(&mut windows, &mut Desk, open(4, 2, Normal));
    assert_eq!(
        order(&windows),
        [3, 1, 2, 4],
        "a window opened over the bar stays with it, in front"
    );
    ask(&mut windows, &mut Desk, open(2, 0, Normal));
    ask(&mut windows, &mut Desk, WindowRequest::Raise(ViewId(3)));
    assert_eq!(
        order(&windows),
        [1, 2, 4, 3],
        "opened again at the normal level, the bar is covered by a raised window"
    );
}
