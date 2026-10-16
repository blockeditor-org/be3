use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::engine::{Engine, Input, Output};
use crate::names::Source;
use crate::rules::{Group, Window};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Request {
    Start {
        config: Vec<u8>,
        window: Option<Window>,
        held: Vec<Source>,
        pressed: Vec<Source>,
    },
    Focus(Option<Window>),
    Input {
        input: Input,
        now: u64,
    },
    Timer {
        now: u64,
    },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Reply {
    pub outputs: Vec<Output>,
    pub deadline: Option<u64>,
}

pub fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    bincode::serialize(value).unwrap_or_default()
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    bincode::deserialize(bytes).map_err(|error| error.to_string())
}

pub fn config(groups: &[Group]) -> Vec<u8> {
    encode(&groups)
}

#[derive(Default)]
pub struct Module {
    engine: Option<Engine>,
}

impl Module {
    pub fn handle(&mut self, request: &[u8]) -> Vec<u8> {
        let request: Request = match decode(request) {
            Ok(request) => request,
            Err(error) => panic!("the host sent a request this module cannot read: {error}"),
        };
        encode(&self.answer(request))
    }

    pub fn answer(&mut self, request: Request) -> Reply {
        let outputs = match request {
            Request::Start {
                config,
                window,
                held,
                pressed,
            } => {
                let groups: Vec<Group> = match decode(&config) {
                    Ok(groups) => groups,
                    Err(error) => panic!("the remaps are not readable: {error}"),
                };
                self.engine = Some(Engine::new(groups, window, &held, &pressed));
                Vec::new()
            }
            Request::Focus(window) => {
                self.engine_mut().focus(window);
                Vec::new()
            }
            Request::Input { input, now } => self.engine_mut().input(input, now),
            Request::Timer { now } => self.engine_mut().timer(now),
        };
        Reply {
            outputs,
            deadline: self.engine.as_ref().and_then(Engine::deadline),
        }
    }

    fn engine_mut(&mut self) -> &mut Engine {
        match &mut self.engine {
            Some(engine) => engine,
            None => panic!("the host asked for remaps before starting them"),
        }
    }
}
