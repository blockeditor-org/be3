//! The chords the session's editor intercepts (`window-requests.intercepted-keys`), which it
//! takes ahead of the focus while the focus is on one of its children's views, as be-host's own
//! UI hands them to the session's view (be-host's `compositor::intercept`): the workspace's
//! Ctrl+Tab from a text open in one of its tabs. The router hands them to the root's own layer
//! with the root's own chords (`Windows::intercept`), and the root's layer hands them on: to the
//! session's screen as intercepted keys (`input.intercepted-key`), with the modifiers held until
//! each is let go; or, while the focus is on the session's screen itself or a surface of the
//! root's own (its palette, a dialog), to the focus, as if no one had intercepted them.

use be_lib_layers::{LayerId, Surface};
use be_lib_ui_types::input::{Input, Key, KeyPress, Modifiers};
use be_lib_ui_types::KeyChord;
use be_sdk_api::ScreenId;

use crate::{Delivery, Embedding, app_surface};

/// What the root's own layer takes ahead of the focus, and what it hands on.
#[derive(Debug, Default)]
pub(crate) struct Intercepting {
    /// The root's own chords (`Embedding::intercept`).
    pub(crate) own: Vec<KeyChord>,
    /// The screen that takes its chords ahead of the focus: the app's handle and the screen.
    screen: Option<(u64, ScreenId)>,
    /// The chords it intercepts, as its last layout said: those with a command modifier (Ctrl,
    /// Alt or Super), and media keys, as be-host takes them.
    pub(crate) chords: Vec<KeyChord>,
    /// The keys the root's layer handed on, in each window, and where: their repeats and
    /// releases go there too.
    handed: Vec<(Option<u64>, Key, Handed)>,
    /// The modifiers still held of those held as the screen took a chord, in its window, which
    /// it hears let go one by one.
    held: Option<(Option<u64>, LayerId, Surface, Modifiers)>,
    /// The app whose screen intercepts and how many times its windows' requests had changed
    /// when they were last read (`Embedded::window_changes`).
    seen: Option<(u64, u64)>,
}

/// Where the root's layer handed a key on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Handed {
    /// To the intercepting screen, as an intercepted key.
    Screen(LayerId, Surface),
    /// To the focus, as the key it is.
    Focus(LayerId, Surface),
}

impl Embedding {
    /// Has `screen` of the app embedded as the handle `.0` (the session's editor's) take the
    /// chords it intercepts ahead of the focus, as be-host's own UI has the session's view take
    /// them; none for none.
    pub fn intercept_for(&mut self, screen: Option<(u64, ScreenId)>) {
        if self.intercepting.screen == screen {
            return;
        }
        self.intercepting.screen = screen;
        self.intercepting.seen = None;
        self.follow_interceptor();
    }

    /// Takes the chords the intercepting screen asks for, read again once its app's layouts
    /// changed what its screens ask of their windows (`Embedded::window_changes`).
    pub(crate) fn follow_interceptor(&mut self) {
        let app = self.intercepting.screen.and_then(|(instance, screen)| {
            let app = self.apps.iter().find(|app| app.instance() == instance)?;
            Some((app, screen))
        });
        let seen = app.map(|(app, _)| (app.instance(), app.window_changes()));
        if seen.is_some() && seen == self.intercepting.seen {
            return;
        }
        let chords: Vec<KeyChord> = app
            .and_then(|(app, screen)| Some(app.screen_window(screen)?.1.intercepted_keys))
            .unwrap_or_default()
            .into_iter()
            .filter(|chord| !chord.tap && (chord.modifiers.command() || chord.key.is_media()))
            .collect();
        self.intercepting.seen = seen;
        if chords != self.intercepting.chords {
            self.intercepting.chords = chords;
            self.reintercept();
        }
    }

    /// Has every window's router hand the root's layer its own chords and the intercepting
    /// screen's.
    pub(crate) fn reintercept(&mut self) {
        let mut chords = self.intercepting.own.clone();
        for chord in &self.intercepting.chords {
            if !chords.contains(chord) {
                chords.push(*chord);
            }
        }
        self.windows.intercept(chords);
    }

    /// The layer and surface showing the intercepting screen in `window`, if it shows there.
    fn interceptor(&self, window: Option<u64>) -> Option<(LayerId, Surface)> {
        let (instance, screen) = self.intercepting.screen?;
        let app = self.apps.iter().find(|app| app.instance() == instance)?;
        let (surface, _) = app.screen_window(screen)?;
        self.windows
            .layers(window)
            .iter()
            .find(|layer| app_surface(layer.surface) == Some((instance, surface)))
            .map(|layer| (layer.id, layer.surface))
    }

    /// What `routed`, the deliveries one event of `window`'s made, become: what reached the
    /// root's own layer that the intercepting screen took and the root did not is handed on in
    /// its place. An event that changed the `modifiers` (with when) tells the screen holding a
    /// chord those it let go of after it.
    pub(crate) fn hand_intercepted(
        &mut self,
        window: Option<u64>,
        routed: Vec<Delivery>,
        modifiers: Option<(Modifiers, Option<u64>)>,
    ) -> Vec<Delivery> {
        let compositor = self.windows.compositor();
        let to = self.hand_to(window);
        self.intercepting
            .hand(window, compositor, to, routed, modifiers)
    }

    /// Where a chord the intercepting screen takes goes in `window`: to the screen, unless the
    /// focus is on it or on a surface of the root's own, or the screen does not show there,
    /// where it goes to the focus, as if no one had intercepted it; nowhere with no focus then.
    fn hand_to(&self, window: Option<u64>) -> Option<Handed> {
        let focused = self.windows.focused(window);
        let to_focus = || focused.map(|focused| Handed::Focus(focused.id, focused.surface));
        let Some((target, surface)) = self.interceptor(window) else {
            return to_focus();
        };
        match focused {
            Some(focused) if focused.id == target || matches!(focused.surface, Surface::Root(_)) => {
                to_focus()
            }
            _ => Some(Handed::Screen(target, surface)),
        }
    }
}

impl Intercepting {
    /// What `routed`, the deliveries one event of `window`'s made, become
    /// (`Embedding::hand_intercepted`), with the root's layer `compositor` and where a chord the
    /// screen takes goes now (`to`).
    pub(crate) fn hand(
        &mut self,
        window: Option<u64>,
        compositor: LayerId,
        to: Option<Handed>,
        routed: Vec<Delivery>,
        modifiers: Option<(Modifiers, Option<u64>)>,
    ) -> Vec<Delivery> {
        let mut out = Vec::with_capacity(routed.len());
        for delivery in routed {
            let Input::Key(stroke) = &delivery.input else {
                out.push(delivery);
                continue;
            };
            let Some(key) = stroke.key.filter(|_| delivery.target == compositor) else {
                out.push(delivery);
                continue;
            };
            let held = self
                .handed
                .iter()
                .position(|(at, held, _)| *at == window && *held == key);
            let to = match (stroke.pressed && !stroke.repeat, held) {
                // A press the root's own chords do not take, that the screen's do.
                (true, _) => {
                    let press = KeyPress {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers: delivery.modifiers,
                    };
                    let own = self.own.iter().any(|chord| chord.matches(press));
                    let asked = self.chords.iter().any(|chord| chord.matches(press));
                    if let Some(at) = held {
                        self.handed.remove(at);
                    }
                    match to.filter(|_| !own && asked) {
                        Some(to) => {
                            self.handed.push((window, key, to));
                            to
                        }
                        None => {
                            out.push(delivery);
                            continue;
                        }
                    }
                }
                (false, Some(at)) if stroke.pressed => self.handed[at].2,
                (false, Some(at)) => self.handed.remove(at).2,
                (false, None) => {
                    out.push(delivery);
                    continue;
                }
            };
            match to {
                Handed::Screen(target, surface) => {
                    if stroke.pressed && !stroke.repeat {
                        self.held = Some((window, target, surface, delivery.modifiers));
                    }
                    for input in [
                        Input::Modifiers(delivery.modifiers),
                        Input::InterceptedKey {
                            key,
                            pressed: stroke.pressed,
                            repeat: stroke.repeat,
                        },
                    ] {
                        out.push(Delivery {
                            target,
                            surface,
                            input,
                            ..delivery.clone()
                        });
                    }
                }
                Handed::Focus(target, surface) => out.push(Delivery {
                    target,
                    surface,
                    ..delivery
                }),
            }
        }
        // The screen holding a chord hears its modifiers let go, as be-host's `intercept_keys`
        // tells it, until none of them is held: only changes after its press, in order.
        if let Some((now, time)) = modifiers
            && let Some((at, target, surface, was)) = self.held
            && at == window
        {
            let still = Modifiers {
                alt: was.alt && now.alt,
                ctrl: was.ctrl && now.ctrl,
                shift: was.shift && now.shift,
                logo: was.logo && now.logo,
            };
            self.held =
                (still != Modifiers::NONE).then_some((at, target, surface, still));
            if still != was {
                out.push(Delivery {
                    target,
                    surface,
                    input: Input::Modifiers(still),
                    time,
                    modifiers: now,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
