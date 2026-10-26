use std::any::Any;

use crate::context::Context;
use crate::document::{Document, trim_bottom};
use crate::geometry::{Pos2, Rect, pos2};
use crate::input::{Event, ImeArea, InputState, Keyboard, PointerId, PointerPhase};
use crate::screens::{self, Screen};
use crate::window::{WindowId, WindowRequest};

pub const MAX_SHARE: f32 = 0.5;
pub const DEFAULT_SHARE: f32 = 0.35;

pub trait KeyboardHost: Any {
    fn show(&mut self, ctx: &Context, rect: Rect, window: Option<WindowId>);

    fn closed(&mut self) {}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Opened {
    #[default]
    Closed,
    Asked,
    Manual,
}

pub(crate) struct KeyboardState {
    enabled: bool,
    attached: bool,
    share: f32,
    opened: Opened,
    toggles: u32,
    applied_toggles: u32,
    asks: Option<u32>,
    area: Option<ImeArea>,
    rect: Option<Rect>,
    pressed: Vec<PointerId>,
    elsewhere: Vec<PointerId>,
    owns: bool,
    typed: Vec<crate::input::Input>,
    window: Option<WindowId>,
    shown_in: Option<WindowId>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Spot {
    Screen(Rect),
    Window(WindowId),
}

impl Spot {
    fn takes(self, window: WindowId, pos: Pos2) -> bool {
        match self {
            Self::Screen(rect) => rect.contains(pos),
            Self::Window(own) => own == window,
        }
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            enabled: false,
            attached: true,
            share: DEFAULT_SHARE,
            opened: Opened::Closed,
            toggles: 0,
            applied_toggles: 0,
            asks: None,
            area: None,
            rect: None,
            pressed: Vec::new(),
            elsewhere: Vec::new(),
            owns: false,
            typed: Vec::new(),
            window: None,
            shown_in: None,
        }
    }
}

impl KeyboardState {
    pub(crate) fn enable(&mut self) {
        self.enabled = true;
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn attached(&self) -> bool {
        self.attached
    }

    pub(crate) fn set_attached(&mut self, attached: bool) {
        self.attached = attached;
    }

    pub(crate) fn window(&self) -> Option<WindowId> {
        self.window
    }

    pub(crate) fn share(&self) -> f32 {
        self.share
    }

    pub(crate) fn set_share(&mut self, share: f32) -> bool {
        let share = match share.is_finite() && share > 0.0 {
            true => share.min(MAX_SHARE),
            false => DEFAULT_SHARE,
        };
        std::mem::replace(&mut self.share, share) != share
    }

    pub(crate) fn opened(&self) -> Opened {
        self.opened
    }

    pub(crate) fn toggle(&mut self) {
        self.toggles = self.toggles.wrapping_add(1);
    }

    pub(crate) fn toggles(&self) -> u32 {
        self.toggles
    }

    pub(crate) fn close(&mut self) {
        self.opened = Opened::Closed;
    }

    pub(crate) fn rect(&self) -> Option<Rect> {
        self.rect
    }

    pub(crate) fn type_events(&mut self, events: impl IntoIterator<Item = crate::input::Input>) {
        self.typed.extend(events);
    }

    pub(crate) fn take_typed(&mut self) -> Vec<crate::input::Input> {
        std::mem::take(&mut self.typed)
    }

    fn decide(&mut self) -> bool {
        let before = self.opened;
        if self.toggles != self.applied_toggles {
            self.applied_toggles = self.toggles;
            self.opened = match self.opened {
                Opened::Closed => Opened::Manual,
                _ => Opened::Closed,
            };
        }
        match self.area.as_ref().map(|area| area.keyboard) {
            Some(Keyboard::Shown { asks }) => {
                if !self.attached && self.asks != Some(asks) && self.opened == Opened::Closed {
                    self.opened = Opened::Asked;
                }
                self.asks = Some(asks);
            }
            _ => {
                self.asks = None;
                if self.opened == Opened::Asked {
                    self.opened = Opened::Closed;
                }
            }
        }
        self.opened != before
    }

    fn place(
        &mut self,
        ctx: &Context,
        rect: Rect,
        host: Option<&mut Box<dyn KeyboardHost>>,
    ) -> Option<(Spot, Rect)> {
        if self.opened == Opened::Closed || !self.enabled {
            return None;
        }
        host?;
        if ctx.opens_windows() {
            let window = *self.window.get_or_insert_with(WindowId::next);
            return Some((Spot::Window(window), rect));
        }
        let shown = screens::shown_in(&ctx.screens(), rect);
        let target = self
            .area
            .as_ref()
            .and_then(|area| screens::under(&shown, area.rect))
            .or_else(|| shown.first())
            .map_or(rect, |screen| screen.rect);
        let height = target.height() * self.share;
        let keyboard =
            Rect::from_min_max(pos2(target.left(), target.bottom() - height), target.max);
        let content = match target == rect {
            true => trim_bottom(rect, height).0,
            false => rect,
        };
        Some((Spot::Screen(keyboard), content))
    }

    fn track(&mut self, input: &InputState, keyboard: Spot) -> bool {
        let mut held = !self.pressed.is_empty();
        let mut away = !self.elsewhere.is_empty();
        for event in &input.pointer_events {
            match event.phase {
                PointerPhase::Down(_) => {
                    let known = self.pressed.contains(&event.pointer)
                        || self.elsewhere.contains(&event.pointer);
                    if known {
                        continue;
                    }
                    match self.elsewhere.is_empty() && keyboard.takes(event.window, event.pos) {
                        true => {
                            held = true;
                            self.pressed.push(event.pointer);
                        }
                        false => {
                            away = true;
                            self.elsewhere.push(event.pointer);
                        }
                    }
                }
                PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone => {
                    self.pressed.retain(|pointer| *pointer != event.pointer);
                    self.elsewhere.retain(|pointer| *pointer != event.pointer);
                }
                _ => {}
            }
        }
        let over = input
            .pointer
            .pos
            .is_some_and(|pos| keyboard.takes(input.pointer.window, pos));
        held || (!away && over)
    }
}

pub(crate) fn usable_screens(ctx: &Context, screens: Vec<Screen>) -> Vec<Screen> {
    let Some(keyboard) = ctx.keyboard_rect() else {
        return screens;
    };
    screens
        .into_iter()
        .map(|screen| {
            let covered = screen.rect.intersect(keyboard);
            match covered.is_positive() && covered.width() >= screen.rect.width() {
                true => Screen {
                    rect: Rect::from_min_max(
                        screen.rect.min,
                        pos2(screen.rect.right(), covered.top().max(screen.rect.top())),
                    ),
                    ..screen
                },
                false => screen,
            }
        })
        .collect()
}

pub fn show_with_keyboard(
    document: &mut Document,
    ctx: &Context,
    rect: Rect,
    show: impl FnOnce(&mut Document, Rect),
) {
    let mut host = document.take_keyboard_host();
    let (placed, changed, left) = ctx.keyboard(|state| {
        let changed = state.decide();
        let placed = state.place(ctx, rect, host.as_mut());
        let (rect, window) = match placed {
            Some((Spot::Screen(keyboard), _)) => (Some(keyboard), None),
            Some((Spot::Window(window), _)) => (None, Some(window)),
            None => (None, None),
        };
        state.rect = rect;
        let left = std::mem::replace(&mut state.shown_in, window).filter(|_| window.is_none());
        (placed, changed, left)
    });
    if let Some(window) = left {
        ctx.request_window(WindowRequest::Close(window));
    }
    if changed {
        ctx.request_repaint();
        if placed.is_none()
            && let Some(host) = host.as_mut()
        {
            host.closed();
        }
    }
    let Some((keyboard, content)) = placed else {
        ctx.keyboard(|state| {
            state.owns = false;
            state.pressed.clear();
            state.elsewhere.clear();
        });
        show(document, rect);
        remember(ctx);
        if let Some(host) = host {
            document.set_keyboard_host(host);
        }
        return;
    };
    let (was, owns) = ctx.keyboard(|state| {
        let owns = ctx.input(|input| state.track(input, keyboard));
        (std::mem::replace(&mut state.owns, owns), owns)
    });
    ctx.with_input(
        |input| match owns {
            true => input.without_pointer(!was),
            false => input.clone(),
        },
        || show(document, content),
    );
    remember(ctx);
    if let Some(mut host) = host {
        let (shown, window) = match keyboard {
            Spot::Screen(keyboard) => (keyboard, None),
            Spot::Window(window) => (content, Some(window)),
        };
        let typing_in = ctx.keyboard_window();
        ctx.with_input(
            |input| {
                let input =
                    input.only(|event| !event.is_key() && !matches!(event, Event::Focus(_)));
                match owns {
                    true => input,
                    false => input.without_pointer(was),
                }
            },
            || host.show(ctx, shown, window),
        );
        ctx.set_keyboard_window(typing_in);
        document.set_keyboard_host(host);
    }
}

fn remember(ctx: &Context) {
    let area = ctx.ime_area();
    ctx.keyboard(|state| state.area = area);
}
