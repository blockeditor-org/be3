use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use block_plugin_api::{FontRole, Fonts, Message};

#[derive(Clone)]
pub struct HostFont {
    pub role: FontRole,
    pub index: u32,
    pub data: Arc<[u8]>,
}

type Listener = Rc<dyn Fn(&[HostFont], bool)>;

#[derive(Default)]
struct State {
    faces: Vec<HostFont>,
    listeners: Vec<Listener>,
    missing: Vec<char>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn host_fonts() -> Vec<HostFont> {
    STATE.with(|state| state.borrow().faces.clone())
}

pub fn watch_fonts(listener: impl Fn(&[HostFont], bool) + 'static) {
    STATE.with(|state| state.borrow_mut().listeners.push(Rc::new(listener)));
}

pub fn report_missing(characters: impl IntoIterator<Item = char>) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        for character in characters {
            if !state.missing.contains(&character) {
                state.missing.push(character);
            }
        }
    });
}

pub(crate) fn receive(fonts: &Fonts) {
    let added: Vec<HostFont> = fonts
        .faces
        .iter()
        .map(|face| HostFont {
            role: face.role,
            index: face.index,
            data: face.data.as_slice().into(),
        })
        .collect();
    let listeners = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if fonts.replace {
            state.faces.clear();
        }
        state.faces.extend(added.iter().cloned());
        state.listeners.clone()
    });
    for listener in listeners {
        listener(&added, fonts.replace);
    }
}

pub(crate) fn take_missing() -> Option<Message> {
    let missing = STATE.with(|state| std::mem::take(&mut state.borrow_mut().missing));
    (!missing.is_empty()).then_some(Message::MissingCharacters(missing))
}
