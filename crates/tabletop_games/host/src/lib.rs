use std::fmt;

use uuid::Uuid;
use wasmi::errors::HostError;
use wasmi::{
    Caller, Config, Engine, Error, Extern, Instance, Linker, Memory, Module, Store, TypedFunc,
    TypedResumableCall, TypedResumableCallHostTrap, Val,
};

pub use game_api::{Command, GameAction, GameActionOption, GameScreen, Turn};

const TURN_FUEL: u64 = 10_000_000;

pub struct Game {
    engine: Engine,
    module: Module,
    linker: Linker<Presented>,
    name: String,
}

#[derive(Default)]
struct Presented {
    screen: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug)]
struct Waiting {
    buffer: u32,
    capacity: u32,
}

impl fmt::Display for Waiting {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the game is waiting for the host")
    }
}

impl HostError for Waiting {}

impl Game {
    pub fn load(module: &[u8]) -> Result<Self, String> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, module)
            .map_err(|error| format!("this is not a game module: {error}"))?;
        let mut linker = Linker::<Presented>::new(&engine);
        linker
            .func_wrap("game", "next", |buffer: u32, capacity: u32| -> Result<u64, Error> {
                Err(Error::host(Waiting { buffer, capacity }))
            })
            .and_then(|linker| {
                linker.func_wrap(
                    "game",
                    "present",
                    |mut caller: Caller<'_, Presented>, pointer: u32, length: u32| {
                        let memory = caller
                            .get_export("memory")
                            .and_then(Extern::into_memory)
                            .ok_or_else(|| Error::new("this game exports no memory"))?;
                        let mut screen = vec![0; length as usize];
                        memory.read(&caller, pointer as usize, &mut screen)?;
                        caller.data_mut().screen = Some(screen);
                        Ok(())
                    },
                )
            })
            .map_err(|error| format!("the game host could not be set up: {error}"))?;
        let mut game = Self {
            engine,
            module,
            linker,
            name: String::new(),
        };
        game.name = game.ask_name()?;
        Ok(game)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start(&self) -> Result<Session, String> {
        let (mut store, instance, memory) = self.instantiate()?;
        let play: TypedFunc<(), ()> = export(&store, &instance, "play")?;
        let started = play.call_resumable(&mut store, ());
        let mut session = Session {
            store,
            memory,
            paused: None,
            played: Vec::new(),
            failure: None,
        };
        session.paused = Some(session.settle(started, "while setting up")?);
        Ok(session)
    }

    pub fn show(&self, actions: &[GameAction], player: Uuid) -> Result<GameScreen, String> {
        let mut session = self.start()?;
        for action in actions {
            session.play(action)?;
        }
        session.show(player)
    }

    fn ask_name(&self) -> Result<String, String> {
        let (mut store, instance, memory) = self.instantiate()?;
        let name: TypedFunc<(), u64> = export(&store, &instance, "name")?;
        let answer = name
            .call(&mut store, ())
            .map_err(|error| format!("this game would not name itself: {error}"))?;
        String::from_utf8(read(&store, &memory, answer)?)
            .map_err(|_| "this game's name is not text".to_owned())
    }

    fn instantiate(&self) -> Result<(Store<Presented>, Instance, Memory), String> {
        let mut store = Store::new(&self.engine, Presented::default());
        store
            .set_fuel(TURN_FUEL)
            .map_err(|error| format!("this game could not be given fuel: {error}"))?;
        let instance = self
            .linker
            .instantiate_and_start(&mut store, &self.module)
            .map_err(|error| format!("this game would not start: {error}"))?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| "this game module exports no memory".to_owned())?;
        Ok((store, instance, memory))
    }
}

pub struct Session {
    store: Store<Presented>,
    memory: Memory,
    paused: Option<TypedResumableCallHostTrap<()>>,
    played: Vec<GameAction>,
    failure: Option<String>,
}

impl Session {
    pub fn played(&self) -> &[GameAction] {
        &self.played
    }

    pub fn play(&mut self, action: &GameAction) -> Result<(), String> {
        let during = format!("on move {}", self.played.len() + 1);
        self.send(&Command::Play(action.clone()), &during)?;
        self.played.push(action.clone());
        Ok(())
    }

    pub fn show(&mut self, player: Uuid) -> Result<GameScreen, String> {
        self.store.data_mut().screen = None;
        self.send(&Command::Show(player), "while drawing the screen")?;
        let screen = self
            .store
            .data_mut()
            .screen
            .take()
            .ok_or_else(|| "this game answered without a screen".to_owned())?;
        bincode::deserialize(&screen)
            .map_err(|error| format!("this game answered with nothing readable: {error}"))
    }

    fn send(&mut self, command: &Command, during: &str) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let sent = self.deliver(command, during);
        if let Err(failure) = &sent {
            self.failure = Some(failure.clone());
        }
        sent
    }

    fn deliver(&mut self, command: &Command, during: &str) -> Result<(), String> {
        let bytes = bincode::serialize(command)
            .map_err(|error| format!("the command could not be encoded: {error}"))?;
        let length = u32::try_from(bytes.len())
            .map_err(|_| "the command is too long for a game module".to_owned())?;
        self.store
            .set_fuel(TURN_FUEL)
            .map_err(|error| format!("this game could not be given fuel: {error}"))?;
        loop {
            let paused = self
                .paused
                .take()
                .ok_or_else(|| "this game is not waiting for a move".to_owned())?;
            let waiting = *paused
                .host_error()
                .downcast_ref::<Waiting>()
                .ok_or_else(|| format!("this game stopped: {}", paused.host_error()))?;
            let fits = length <= waiting.capacity;
            if fits {
                self.memory
                    .write(&mut self.store, waiting.buffer as usize, &bytes)
                    .map_err(|error| format!("this game could not take a command: {error}"))?;
            }
            let resumed = paused.resume(&mut self.store, &[Val::I64(i64::from(length))]);
            self.paused = Some(self.settle(resumed, during)?);
            if fits {
                return Ok(());
            }
        }
    }

    fn settle(
        &mut self,
        resumed: Result<TypedResumableCall<()>, Error>,
        during: &str,
    ) -> Result<TypedResumableCallHostTrap<()>, String> {
        match resumed {
            Ok(TypedResumableCall::HostTrap(paused)) => Ok(paused),
            Ok(TypedResumableCall::OutOfFuel(_)) => {
                Err(format!("this game ran out of fuel {during}"))
            }
            Ok(TypedResumableCall::Finished(())) => {
                Err("this game stopped taking moves".to_owned())
            }
            Err(error) => Err(format!("this game stopped: {error}")),
        }
    }
}

fn export<Parameters: wasmi::WasmParams, Results: wasmi::WasmResults>(
    store: &Store<Presented>,
    instance: &Instance,
    name: &str,
) -> Result<TypedFunc<Parameters, Results>, String> {
    instance
        .get_typed_func(store, name)
        .map_err(|error| format!("this game module has no usable {name}: {error}"))
}

fn read(store: &Store<Presented>, memory: &Memory, answer: u64) -> Result<Vec<u8>, String> {
    let pointer = (answer >> 32) as usize;
    let length = (answer & u64::from(u32::MAX)) as usize;
    let end = pointer
        .checked_add(length)
        .ok_or_else(|| "this game answered outside its own memory".to_owned())?;
    memory
        .data(store)
        .get(pointer..end)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| "this game answered outside its own memory".to_owned())
}

#[cfg(test)]
mod tests;
