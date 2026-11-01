use std::time::Duration;

use beui::native::{Context, WindowSize, enter};
use beui::{PointerButton, PointerEvent, PointerId};
use beui_host::native::testing::{Showing, Stepped};
use beui_host::native::{FreetypeFonts, build};
use block_plugin_api::{Size, WindowOver, WindowRequest};

use super::*;
mod a_closed_window_takes_the_windows_it_opened_with_it;
mod a_moved_window_brings_the_windows_opened_over_it_along;
mod a_raised_window_brings_the_windows_opened_over_it_along;
mod a_window_held_past_the_edge_of_the_app_stretches_and_springs_back_when_let_go;
mod a_window_moved_off_the_bottom_of_the_app_keeps_its_size_and_its_top_in_view;
mod a_window_opened_from_a_dock_tab_is_hosted_by_that_tab;
mod a_window_shown_in_the_app_is_kept_inside_it_and_shrinks_rather_than_covering_its_opener;
mod a_window_shows_on_the_display_holding_most_of_it_and_on_both_while_it_is_moved;

const APP: Vec2 = vec2(800.0, 600.0);
const HELD: Vec2 = vec2(50.0, 10.0);
const STEP: Duration = Duration::from_millis(16);

struct Carrying {
    stepped: Stepped,
    shown: Memo<Rect>,
    key: FloatedKey,
}

impl Carrying {
    fn open(at: Pos2, size: Vec2) -> Self {
        let key = FloatedKey {
            plugin: "test.carry".to_owned(),
            instance: EditorInstanceId(1),
            window: ViewId(7),
        };
        let mut shown = None;
        let document = build(|| {
            install();
            crate::display::create_handle();
            shown = Some(carried(key.clone(), create_memo(|| APP)));
            view! {
                <beui::styled::Root>
                    <beui::styled::Frame />
                </beui::styled::Root>
            }
        });
        let window = WindowSize {
            size: APP,
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
            screen: block_plugin_api::ScreenId::opened(7),
            opener: main,
            placement: WindowPlacement::Over(WindowOver { view: main, at }),
            size: Size {
                width: size.x,
                height: size.y,
            },
        });
        carrying.inside(|key| {
            note_origin(&key.plugin, key.instance, ViewId::MAIN, Pos2::ZERO);
            block_app_plugin_host::request_windows(&key.plugin, vec![opened]);
            apply();
        });
        carrying
    }

    fn inside<R>(&mut self, run: impl FnOnce(&FloatedKey) -> R) -> R {
        let key = self.key.clone();
        enter(self.stepped.document_mut(), || run(&key))
    }

    fn pointer(&mut self, phase: PointerPhase, at: Pos2) {
        self.inside(|key| {
            let event = PointerEvent {
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
            block_app_plugin_host::request_windows(
                &key.plugin,
                vec![WindowRequest::BeginMove(key.window)],
            );
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
