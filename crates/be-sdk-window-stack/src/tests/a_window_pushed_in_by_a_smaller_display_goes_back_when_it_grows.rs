use super::*;

struct Kept {
    rect: Rect,
    wanted: Rect,
}

impl Stacked for Kept {
    fn view(&self) -> ViewId {
        ViewId(1)
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.wanted = rect;
    }

    fn wanted(&self) -> Rect {
        self.wanted
    }

    fn show_at(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn opened_over(&self, _: &Self) -> bool {
        false
    }
}

#[test]
fn a_window_pushed_in_by_a_smaller_display_goes_back_when_it_grows() {
    let large = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0));
    let small = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let banded = |display: Rect| {
        vec![(
            display,
            Rect::from_min_max(display.min, pos2(display.max.x, display.max.y - 40.0)),
        )]
    };
    let put = Rect::from_min_size(pos2(1000.0, 500.0), vec2(400.0, 300.0));
    let mut windows = [Kept {
        rect: put,
        wanted: put,
    }];

    assert!(keep_in_work_areas(&mut windows, None, |_| banded(small)));
    let pushed = windows[0].rect;
    assert!(
        pushed.min.x <= 800.0 - KEPT.x && pushed.max.y <= 560.0,
        "on the smaller display the window shows where its bar can be grabbed: {pushed:?}"
    );
    assert_eq!(
        windows[0].wanted, put,
        "but it is still wanted where it was put"
    );

    assert!(keep_in_work_areas(&mut windows, None, |_| banded(large)));
    assert_eq!(
        windows[0].rect, put,
        "once the display is large again, the window shows where it was put"
    );

    keep_in_work_areas(&mut windows, None, |_| banded(small));
    let moved = Rect::from_min_size(pos2(100.0, 100.0), put.size());
    let from = windows[0].rect;
    drag(&mut windows, 0, Gesture::Move, from, moved.min - from.min);
    keep_in_work_areas(&mut windows, None, |_| banded(large));
    assert_eq!(
        windows[0].rect, moved,
        "a window moved while the display was small stays where it was moved to"
    );
}
