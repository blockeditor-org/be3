use std::sync::Mutex;

use smithay::reexports::wayland_protocols::wp::text_input::zv3::server::zwp_text_input_manager_v3::{
    self, ZwpTextInputManagerV3,
};
use smithay::reexports::wayland_protocols::wp::text_input::zv3::server::zwp_text_input_v3::{
    self, ZwpTextInputV3,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};

use crate::state::State;

const VERSION: u32 = 1;

pub(crate) fn create(handle: &DisplayHandle) {
    handle.create_global::<State, ZwpTextInputManagerV3, _>(VERSION, ());
}

#[derive(Default)]
pub(crate) struct TextInputs {
    inputs: Vec<ZwpTextInputV3>,
    focus: Option<WlSurface>,
    asks: u64,
}

#[derive(Default)]
pub struct InputData(Mutex<Input>);

#[derive(Default)]
struct Input {
    entered: Option<WlSurface>,
    pending: Option<bool>,
    enabled: bool,
    commits: u32,
}

fn input(text_input: &ZwpTextInputV3) -> Option<&InputData> {
    text_input.data::<InputData>()
}

impl TextInputs {
    pub(crate) fn focus(&mut self, surface: Option<&WlSurface>) {
        if self.focus.as_ref() == surface {
            return;
        }
        self.focus = surface.cloned();
        for text_input in &self.inputs {
            enter(text_input, self.focus.as_ref());
        }
    }

    fn add(&mut self, text_input: ZwpTextInputV3) {
        enter(&text_input, self.focus.as_ref());
        self.inputs.push(text_input);
    }

    fn typing(&self) -> impl Iterator<Item = &ZwpTextInputV3> {
        self.inputs.iter().filter(|text_input| {
            input(text_input).is_some_and(|data| {
                let data = data.0.lock().unwrap();
                data.enabled && data.entered.is_some() && data.entered == self.focus
            })
        })
    }

    pub(crate) fn asks(&self) -> Option<u64> {
        self.typing().next().map(|_| self.asks)
    }

    pub(crate) fn commit_text(&self, text: &str) {
        for text_input in self.typing() {
            let commits = input(text_input).map_or(0, |data| data.0.lock().unwrap().commits);
            text_input.commit_string(Some(text.to_owned()));
            text_input.done(commits);
        }
    }
}

fn enter(text_input: &ZwpTextInputV3, focus: Option<&WlSurface>) {
    let Some(data) = input(text_input) else {
        return;
    };
    let mut data = data.0.lock().unwrap();
    let wanted = focus.filter(|surface| surface.client() == text_input.client());
    if data.entered.as_ref() == wanted {
        return;
    }
    if let Some(left) = data.entered.take() {
        text_input.leave(&left);
        data.enabled = false;
        data.pending = None;
    }
    if let Some(surface) = wanted {
        text_input.enter(surface);
        data.entered = Some(surface.clone());
    }
}

impl GlobalDispatch<ZwpTextInputManagerV3, ()> for State {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ZwpTextInputManagerV3>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwpTextInputManagerV3, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _manager: &ZwpTextInputManagerV3,
        request: zwp_text_input_manager_v3::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwp_text_input_manager_v3::Request::GetTextInput { id, .. } = request {
            let text_input = data_init.init(id, InputData::default());
            state.text_inputs.add(text_input);
        }
    }
}

impl Dispatch<ZwpTextInputV3, InputData> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _text_input: &ZwpTextInputV3,
        request: zwp_text_input_v3::Request,
        data: &InputData,
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let mut data = data.0.lock().unwrap();
        match request {
            zwp_text_input_v3::Request::Enable => data.pending = Some(true),
            zwp_text_input_v3::Request::Disable => data.pending = Some(false),
            zwp_text_input_v3::Request::Commit => {
                data.commits = data.commits.wrapping_add(1);
                match data.pending.take() {
                    Some(true) => {
                        data.enabled = data.entered.is_some();
                        state.text_inputs.asks += 1;
                    }
                    Some(false) => data.enabled = false,
                    None => {}
                }
            }
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        text_input: &ZwpTextInputV3,
        _data: &InputData,
    ) {
        state
            .text_inputs
            .inputs
            .retain(|held| held.id() != text_input.id());
    }
}
