use std::borrow::Cow;

use smithay::desktop::PopupKind;
use smithay::input::Seat;
use smithay::input::keyboard::{KeyboardTarget, KeysymHandle, ModifiersState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Serial};
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::X11Surface;

use crate::state::State;

#[derive(Clone, Debug, PartialEq)]
pub enum KeyboardFocus {
    Surface(WlSurface),
    X11(X11Surface, WlSurface),
}

impl KeyboardFocus {
    pub fn surface(&self) -> &WlSurface {
        match self {
            Self::Surface(surface) | Self::X11(_, surface) => surface,
        }
    }
}

impl From<WlSurface> for KeyboardFocus {
    fn from(surface: WlSurface) -> Self {
        Self::Surface(surface)
    }
}

impl From<PopupKind> for KeyboardFocus {
    fn from(popup: PopupKind) -> Self {
        Self::Surface(popup.wl_surface().clone())
    }
}

impl From<KeyboardFocus> for WlSurface {
    fn from(focus: KeyboardFocus) -> Self {
        match focus {
            KeyboardFocus::Surface(surface) | KeyboardFocus::X11(_, surface) => surface,
        }
    }
}

impl IsAlive for KeyboardFocus {
    fn alive(&self) -> bool {
        match self {
            Self::Surface(surface) => surface.alive(),
            Self::X11(window, surface) => window.alive() && surface.alive(),
        }
    }
}

impl WaylandFocus for KeyboardFocus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        Some(Cow::Borrowed(self.surface()))
    }
}

impl KeyboardTarget<State> for KeyboardFocus {
    fn enter(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        match self {
            Self::Surface(surface) => KeyboardTarget::enter(surface, seat, data, keys, serial),
            Self::X11(window, _) => KeyboardTarget::enter(window, seat, data, keys, serial),
        }
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial) {
        match self {
            Self::Surface(surface) => KeyboardTarget::leave(surface, seat, data, serial),
            Self::X11(window, _) => KeyboardTarget::leave(window, seat, data, serial),
        }
    }

    fn key(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        key: KeysymHandle<'_>,
        state: smithay::backend::input::KeyState,
        serial: Serial,
        time: u32,
    ) {
        match self {
            Self::Surface(surface) => {
                KeyboardTarget::key(surface, seat, data, key, state, serial, time);
            }
            Self::X11(window, _) => {
                KeyboardTarget::key(window, seat, data, key, state, serial, time);
            }
        }
    }

    fn modifiers(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        match self {
            Self::Surface(surface) => {
                KeyboardTarget::modifiers(surface, seat, data, modifiers, serial);
            }
            Self::X11(window, _) => {
                KeyboardTarget::modifiers(window, seat, data, modifiers, serial);
            }
        }
    }
}
