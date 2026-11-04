use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[cfg(feature = "text")]
use crate::engine::Engine;
use crate::engine::{Input, Output};
use crate::names::Source;
use crate::rules::Window;

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
    pub problem: Option<String>,
}

pub fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    bincode::serialize(value).unwrap_or_default()
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    bincode::deserialize(bytes).map_err(|error| error.to_string())
}

#[cfg(feature = "text")]
#[derive(Default)]
pub struct Module {
    engine: Option<Engine>,
}

#[cfg(feature = "text")]
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
                let groups = String::from_utf8(config)
                    .map_err(|_| "the remaps are not text".to_owned())
                    .and_then(|text| crate::text::parse(&text));
                match groups {
                    Ok(groups) => self.engine = Some(Engine::new(groups, window, &held, &pressed)),
                    Err(problem) => {
                        return Reply {
                            problem: Some(problem),
                            ..Reply::default()
                        };
                    }
                }
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
            problem: None,
        }
    }

    fn engine_mut(&mut self) -> &mut Engine {
        match &mut self.engine {
            Some(engine) => engine,
            None => panic!("the host asked for remaps before starting them"),
        }
    }
}
