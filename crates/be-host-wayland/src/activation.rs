use std::collections::HashMap;
use std::time::{Duration, Instant};

use smithay::delegate_xdg_activation;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{DisplayHandle, Resource};
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};

use crate::state::{ServerEvent, State, WindowId};

pub(crate) const TOKEN_LIFETIME: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug)]
enum Origin {
    Launch,
    Client { focus: Option<WindowId> },
}

#[derive(Clone, Copy, Debug)]
struct Grant {
    created: Instant,
    origin: Origin,
}

pub(crate) struct Activation {
    state: XdgActivationState,
    grants: HashMap<XdgActivationToken, Grant>,
    now: Option<Instant>,
}

impl Activation {
    pub(crate) fn new(handle: &DisplayHandle) -> Self {
        Self {
            state: XdgActivationState::new::<State>(handle),
            grants: HashMap::new(),
            now: None,
        }
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        self.now = Some(now);
        self.grants
            .retain(|_, grant| now.saturating_duration_since(grant.created) < TOKEN_LIFETIME);
        let grants = &self.grants;
        self.state
            .retain_tokens(|token, _| grants.contains_key(token));
    }

    pub(crate) fn focus_moved(&mut self) {
        self.grants
            .retain(|_, grant| matches!(grant.origin, Origin::Launch));
        let grants = &self.grants;
        self.state
            .retain_tokens(|token, _| grants.contains_key(token));
    }

    pub(crate) fn launch_token(&mut self, now: Instant) -> String {
        self.tick(now);
        let token = self.state.create_external_token(None).0.clone();
        self.grants.insert(
            token.clone(),
            Grant {
                created: now,
                origin: Origin::Launch,
            },
        );
        token.to_string()
    }

    fn take(&mut self, token: &XdgActivationToken) -> Option<Grant> {
        self.state.remove_token(token);
        self.grants.remove(token)
    }
}

impl State {
    fn interacting(&self, data: &XdgActivationTokenData) -> bool {
        let Some((serial, seat)) = &data.serial else {
            return false;
        };
        if !self.seat().owns(seat) {
            return false;
        }
        let keyboard = self.keyboard();
        let focused = keyboard
            .current_focus()
            .and_then(|focus| focus.surface().client())
            .map(|client| client.id());
        focused.is_some()
            && focused == data.client_id
            && keyboard
                .last_enter()
                .is_some_and(|entered| serial.is_no_older_than(&entered))
    }

    fn honoured(&self, grant: Grant) -> bool {
        let Some(now) = self.activation.now else {
            return false;
        };
        if now.saturating_duration_since(grant.created) >= TOKEN_LIFETIME {
            return false;
        }
        match grant.origin {
            Origin::Launch => true,
            Origin::Client { focus } => self.keyboard_window() == focus,
        }
    }
}

impl XdgActivationHandler for State {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.activation.state
    }

    fn token_created(&mut self, token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        let Some(now) = self.activation.now else {
            return false;
        };
        if !self.interacting(&data) {
            return false;
        }
        let focus = self.keyboard_window();
        self.activation.grants.insert(
            token,
            Grant {
                created: now,
                origin: Origin::Client { focus },
            },
        );
        true
    }

    fn request_activation(
        &mut self,
        token: XdgActivationToken,
        _data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        let Some(grant) = self.activation.take(&token) else {
            return;
        };
        if !self.honoured(grant) {
            return;
        }
        if let Some(id) = self.window_of_surface(&surface) {
            self.push_event(ServerEvent::Activated(id));
        }
    }
}

delegate_xdg_activation!(State);
