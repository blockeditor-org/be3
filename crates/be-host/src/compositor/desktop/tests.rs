use std::time::Duration;

use be_sdk_api::{Size, WindowOver, WindowRequest};
use beui::native::{Context, WindowSize, enter};
use beui::{PointerButton, PointerEvent, PointerId};
use beui_host::native::testing::{Showing, Stepped};
use beui_host::native::{FreetypeFonts, build};

use super::*;
mod a_closed_window_takes_the_windows_it_opened_with_it;
mod a_desktop_window_keeps_its_grip_in_the_work_area_in_a_window_and_off_an_unplugged_display;
mod a_desktop_window_moved_past_the_foot_of_its_display_hangs_off_it_and_springs_back_to_its_grip;
mod a_moved_window_brings_the_windows_opened_over_it_along;
mod a_raised_window_brings_the_windows_opened_over_it_along;
mod a_window_a_program_asks_to_move_from_a_finger_follows_that_finger;
mod a_window_a_program_asks_to_move_or_resize_follows_the_pointer;
mod a_window_held_past_the_edge_of_the_host_stretches_and_springs_back_when_let_go;
mod a_window_moved_off_the_bottom_of_the_host_keeps_its_size_and_its_top_in_view;
mod a_window_opened_from_a_dock_tab_is_hosted_by_that_tab;
mod a_window_shown_in_the_host_is_kept_inside_it_and_shrinks_rather_than_covering_its_opener;
mod a_window_shows_on_the_display_holding_most_of_it_and_on_both_while_it_is_moved;

const HOST: Vec2 = vec2(800.0, 600.0);
const HELD: Vec2 = vec2(50.0, 10.0);
const STEP: Duration = Duration::from_millis(16);

struct Carrying {
    stepped: Stepped,
    shown: Memo<Rect>,
    key: FloatedKey,
}

impl Carrying {
    /// A window carried in be-host's own window (`carried`).
    fn open(at: Pos2, size: Vec2) -> Self {
        Self::opened(at, size, None)
    }

    /// A window on a desktop of one display the size of `HOST`, whose work area is `work`.
    fn on_desktop(at: Pos2, size: Vec2, work: Rect) -> Self {
        Self::opened(at, size, Some(work))
    }

    fn opened(at: Pos2, size: Vec2, desktop: Option<Rect>) -> Self {
        let key = FloatedKey {
            app: "test.carry".to_owned(),
            instance: EditorInstanceId(1),
            window: ViewId(7),
        };
        let mut shown = None;
        let document = build(|| {
            install();
            crate::display::create_handle();
            shown = Some(match desktop {
                None => carried(key.clone(), create_memo(|| HOST)),
                Some(work) => {
                    hold_window_display(create_memo(|| Rect::from_min_size(Pos2::ZERO, HOST)));
                    note_work_area((key.app.clone(), key.instance, ViewId::MAIN), Some(work));
                    let shown = rect_of(key.clone());
                    create_memo(move || shown.get().unwrap_or(Rect::ZERO))
                }
            });
            view! {
                <beui::styled::Root>
                    <beui::styled::Frame />
                </beui::styled::Root>
            }
        });
        let window = WindowSize {
            size: HOST,
            scale: 1.0,
        };
        let mut stepped = Stepped::new(
            Context::new(FreetypeFonts::default()),
            Showing(document),
            window,
            None,
        );
        stepped.frame(Vec::new());
        let mut carrying = Self {
            stepped,
            shown: shown.expect("the document was built"),
            key,
        };
        let main = ViewRef {
            instance: carrying.key.instance,
            view: ViewId::MAIN,
        };
        let opened = WindowRequest::Open(WindowOpen {
            view: carrying.key.window,
            screen: be_sdk_api::ScreenId::opened(7),
            opener: main,
            placement: WindowPlacement::Over(WindowOver { view: main, at }),
            size: Size {
                width: size.x,
                height: size.y,
            },
            level: be_sdk_api::WindowLevel::Normal,
        });
        carrying.inside(|key| {
            note_origin(&key.app, key.instance, ViewId::MAIN, Pos2::ZERO);
            be_host_runtime::request_windows(&key.app, vec![opened]);
            apply();
        });
        carrying
    }

    fn inside<R>(&mut self, run: impl FnOnce(&FloatedKey) -> R) -> R {
        let key = self.key.clone();
        enter(self.stepped.document_mut(), || run(&key))
    }

    fn pointer(&mut self, phase: PointerPhase, at: Pos2) {
        self.pointer_by(PointerId::Mouse, phase, at);
    }

    fn pointer_by(&mut self, pointer: PointerId, phase: PointerPhase, at: Pos2) {
        self.inside(|key| {
            let event = PointerEvent {
                pointer,
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
            };
            follow(
                key,
                ForwardedInput {
                    events: vec![Forwarded::Pointer { event, over: None }],
                    rect: Rect::ZERO,
                    hovered: true,
                    focused: true,
                    pointer: None,
                    modifiers: Default::default(),
                },
                Vec2::ZERO,
            );
        });
        self.stepped.frame(Vec::new());
    }

    fn press(&mut self, at: Pos2) {
        self.pointer(PointerPhase::Down(PointerButton::Primary), at);
        self.inside(|key| {
            be_host_runtime::request_windows(&key.app, vec![WindowRequest::BeginMove(key.window)]);
            apply();
        });
        self.stepped.frame(Vec::new());
    }

    fn drag_to(&mut self, to: Pos2) {
        self.stepped.advance(STEP);
        self.pointer(PointerPhase::Move, to);
    }

    fn release(&mut self, at: Pos2) {
        self.pointer(PointerPhase::Up(PointerButton::Primary), at);
    }

    fn shown(&mut self) -> Rect {
        let shown = self.shown;
        self.inside(|_| shown.get_untracked())
    }

    fn placed(&mut self) -> Rect {
        self.inside(|key| rect_of(key.clone()).get_untracked())
            .expect("the window is open")
    }

    fn frames_to_settle(&mut self) -> usize {
        for frames in 0..600 {
            let before = self.shown();
            self.stepped.advance(STEP);
            self.stepped.frame(Vec::new());
            if self.shown() == before {
                return frames;
            }
        }
        panic!("the window is still moving after 600 frames");
    }
}
