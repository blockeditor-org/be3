//! The content of a captured entry (a Windows window, display or popup) and the pointer on it,
//! with the platform behind [`ContentInput`], so the one rule that matters is tested on Linux:
//! focus mode, which foregrounds the real window and locks the cursor to it, starts only from a
//! click on the content. A hover, a press that becomes a drag, a press that began elsewhere (a
//! grab handle, space's chrome) and the shell giving the window the keyboard never start it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::reactive::*;
use beui::styled::*;
use beui::{Forwarded, ForwardedInput, NodeId, PointerButton, PointerPhase, Pos2, Vec2};

use super::win32_map::ContentPress;

/// What a controller's laser does on the content, injected since a laser has no cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Laser {
    Press(PointerButton),
    Release(PointerButton),
    Move,
}

/// The platform behind a captured entry's content. Points are in the content's own points, with
/// the panel's size beside them.
pub trait ContentInput {
    /// A click on the content: the only way focus mode starts.
    fn clicked(&self, at: Pos2, panel: Vec2);
    /// A controller's laser on the content.
    fn laser(&self, _at: Pos2, _laser: Laser, _panel: Vec2) {}
    /// The pointer came over the content or left it.
    fn hovered(&self, _over: bool) {}
    /// The content's node took or lost beui's keyboard focus, as when the shell gives the window
    /// the keyboard; it never starts focus mode.
    fn keyboard(&self, _held: bool) {}
}

/// Takes the pointer on a captured entry's content and hands it to `input`. `focus` is the
/// shell giving the window the keyboard: it focuses this node, as on every platform, and never
/// starts focus mode. A `popup` takes no keyboard focus and no lasers.
#[component]
pub fn CapturedContent(
    input: Rc<dyn ContentInput>,
    focus: Option<Trigger>,
    tag: String,
    #[prop(default = false)] popup: bool,
    children: Child,
) -> NodeId {
    let node = NodeRef::new();
    if let Some(focus) = focus {
        create_effect_on_change(
            move || focus.track(),
            move |_, _| {
                if let Some(node) = node.try_get() {
                    node.focus();
                }
            },
        );
    }
    let press = RefCell::new(ContentPress::default());
    let held = Cell::new(false);
    let hover = input.clone();
    view! {
        <Interactive
            @node_ref=&node
            focusable=true
            press_focus={!popup}
            tab_stop={!popup}
            @test_id={tag.clone()}
            on_hover_change={move |over: bool| hover.hovered(over)}
            on_forward={move |forwarded: ForwardedInput| route(&press, &held, &forwarded, &*input, !popup)}
        >
            {children}
        </Interactive>
    }
}

fn route(
    press: &RefCell<ContentPress>,
    held: &Cell<bool>,
    forwarded: &ForwardedInput,
    input: &dyn ContentInput,
    lasers: bool,
) {
    let panel = forwarded.rect.size();
    for event in &forwarded.events {
        let (event, over) = match event {
            Forwarded::Pointer { event, over } => (event, over),
            Forwarded::Focus(held) => {
                input.keyboard(*held);
                continue;
            }
            _ => continue,
        };
        let at = over.map_or(event.pos - forwarded.rect.min.to_vec2(), |over| over.local);
        if event.pointer.is_controller() {
            if !lasers {
                continue;
            }
            let laser = match event.phase {
                PointerPhase::Down(button) => {
                    held.set(true);
                    Laser::Press(button)
                }
                PointerPhase::Up(button) => {
                    held.set(false);
                    Laser::Release(button)
                }
                PointerPhase::Move if held.get() => Laser::Move,
                PointerPhase::Cancel | PointerPhase::Gone => {
                    held.set(false);
                    continue;
                }
                _ => continue,
            };
            input.laser(at, laser, panel);
            continue;
        }
        let clicked = {
            let mut press = press.borrow_mut();
            match event.phase {
                PointerPhase::Down(_) => {
                    press.down(event.pointer, at);
                    None
                }
                PointerPhase::Move => {
                    press.moved(event.pointer, at);
                    None
                }
                PointerPhase::Up(_) => press.up(event.pointer, at, panel),
                PointerPhase::Cancel | PointerPhase::Gone => {
                    press.cancel();
                    None
                }
                _ => None,
            }
        };
        if let Some(at) = clicked {
            input.clicked(at, panel);
        }
    }
}

#[cfg(test)]
mod tests;
