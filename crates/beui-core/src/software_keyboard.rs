use std::any::Any;

use crate::context::Context;
use crate::document::{Document, trim_bottom};
use crate::geometry::{Rect, pos2};
use crate::input::{Event, ImeArea, InputState, Keyboard, PointerId, PointerPhase};
use crate::window::WindowId;

pub const MAX_SHARE: f32 = 0.5;
pub const DEFAULT_SHARE: f32 = 0.35;

pub trait KeyboardHost: Any {
    fn show(&mut self, ctx: &Context, rect: Rect);

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
    window: WindowId,
    pressed: Vec<PointerId>,
    elsewhere: Vec<PointerId>,
    owns: bool,
    typed: Vec<crate::input::Input>,
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
            window: WindowId::MAIN,
            pressed: Vec::new(),
            elsewhere: Vec::new(),
            owns: false,
            typed: Vec::new(),
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

    pub(crate) fn trimmed(&self, window: WindowId, rect: Rect) -> Rect {
        match self
            .rect
            .filter(|_| self.window == window && !window.is_main())
        {
            Some(keyboard) => trim_bottom(rect, keyboard.height()).0,
            None => rect,
        }
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
        document: &Document,
        ctx: &Context,
        rect: Rect,
        host: Option<&mut Box<dyn KeyboardHost>>,
    ) -> Option<(WindowId, Rect, Rect)> {
        if self.opened == Opened::Closed || !self.enabled {
            return None;
        }
        host?;
        let window = typing_window(document, ctx);
        let target = match window.is_main() {
            true => rect,
            false => document.window_rect(window)?,
        };
        let height = target.height() * self.share;
        let keyboard =
            Rect::from_min_max(pos2(target.left(), target.bottom() - height), target.max);
        let content = match window.is_main() {
            true => trim_bottom(rect, height).0,
            false => rect,
        };
        Some((window, keyboard, content))
    }

    fn track(&mut self, input: &InputState, keyboard: Rect) -> bool {
        let window = self.window;
        let mut held = !self.pressed.is_empty();
        let mut away = !self.elsewhere.is_empty();
        for event in input
            .pointer_events
            .iter()
            .filter(|event| event.window == window)
        {
            match event.phase {
                PointerPhase::Down(_) => {
                    let known = self.pressed.contains(&event.pointer)
                        || self.elsewhere.contains(&event.pointer);
                    if known {
                        continue;
                    }
                    match self.elsewhere.is_empty() && keyboard.contains(event.pos) {
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
        let over = input.pointer.window == window
            && input.pointer.pos.is_some_and(|pos| keyboard.contains(pos));
        held || (!away && over)
    }
}

fn typing_window(document: &Document, ctx: &Context) -> WindowId {
    let hosted = |window: &WindowId| !window.is_main() && document.window_rect(*window).is_some();
    document
        .key_window()
        .filter(hosted)
        .or_else(|| match ctx.shows_main() {
            true => None,
            false => document.open_windows().into_iter().find(hosted),
        })
        .unwrap_or(WindowId::MAIN)
}

pub fn show_with_keyboard(
    document: &mut Document,
    ctx: &Context,
    rect: Rect,
    show: impl FnOnce(&mut Document, Rect),
) {
    let mut host = document.take_keyboard_host();
    let (placed, changed, moved) = ctx.keyboard(|state| {
        let changed = state.decide();
        let placed = state.place(document, ctx, rect, host.as_mut());
        let before = (state.window, state.rect);
        state.rect = placed.map(|(_, keyboard, _)| keyboard);
        state.window = placed.map_or(WindowId::MAIN, |(window, _, _)| window);
        let after = (state.window, state.rect);
        let moved = (before != after).then_some([before.0, after.0]);
        (placed, changed, moved)
    });
    for window in moved
        .into_iter()
        .flatten()
        .filter(|window| !window.is_main())
    {
        document.relayout_window(window);
    }
    if changed {
        ctx.request_repaint();
        if placed.is_none()
            && let Some(host) = host.as_mut()
        {
            host.closed();
        }
    }
    let Some((window, keyboard, content)) = placed else {
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
        ctx.with_input(
            |input| {
                let input = input
                    .seen_from(window)
                    .only(|event| !event.is_key() && !matches!(event, Event::Focus(_)));
                match owns {
                    true => input,
                    false => input.without_pointer(was),
                }
            },
            || ctx.draw_into(window, || host.show(ctx, keyboard)),
        );
        document.set_keyboard_host(host);
    }
}

fn remember(ctx: &Context) {
    let area = ctx.ime_area();
    ctx.keyboard(|state| state.area = area);
}
