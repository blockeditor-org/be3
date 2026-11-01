mod audio;
mod backlight;
mod players;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::rc::Rc;

use block_app_base::host::{self, Mailbox};

use block_plugin_api::{
    AudioOutput, MediaLevel, MediaLevels, MediaRequest, MediaWatch, PlayerCommand,
};

#[derive(Clone, Debug, PartialEq)]
pub enum AudioRequest {
    Step(f32),
    Set(f32),
    ToggleMute,
    SetMute(bool),
    Choose(String),
    ToggleMicMute,
}

pub trait Audio {
    fn request(&self, request: AudioRequest);
}

pub trait Backlight {
    fn step(&self, by: f32);
}

pub trait Players {
    fn request(&self, command: PlayerCommand);
}

#[derive(Clone)]
pub struct Backends {
    pub audio: Rc<dyn Audio>,
    pub backlight: Rc<dyn Backlight>,
    pub players: Rc<dyn Players>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioState {
    pub output: Option<MediaLevel>,
    pub input: Option<MediaLevel>,
    pub outputs: Vec<AudioOutput>,
    pub default_output: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MediaEvent {
    Audio(AudioState),
    Brightness(f32),
}

pub struct Media {
    backends: Backends,
    _events: Mailbox,
}

impl Media {
    pub fn start() -> Self {
        let levels = RefCell::new(MediaLevels::default());
        block_app_base::watches::publish::<MediaWatch>(&levels.borrow());
        let (events, post) = host::mailbox(move |event: MediaEvent| {
            let mut levels = levels.borrow_mut();
            heard(&mut levels, event);
            block_app_base::watches::publish::<MediaWatch>(&levels);
        });
        let backends = Backends {
            audio: Rc::new(audio::PulseAudio::start(post.clone())),
            backlight: Rc::new(backlight::LogindBacklight::start(post)),
            players: Rc::new(players::Mpris::start()),
        };
        Self::with(backends, events)
    }

    pub fn with(backends: Backends, events: Mailbox) -> Self {
        Self {
            backends,
            _events: events,
        }
    }

    pub fn request(&self, request: MediaRequest) {
        let backends = &self.backends;
        match request {
            MediaRequest::StepVolume(by) => {
                if let Some(by) = step(by) {
                    backends.audio.request(AudioRequest::Step(by));
                }
            }
            MediaRequest::SetVolume(level) if level.is_finite() => {
                backends
                    .audio
                    .request(AudioRequest::Set(level.clamp(0.0, 1.0)));
            }
            MediaRequest::SetVolume(_) => {}
            MediaRequest::ToggleMute => backends.audio.request(AudioRequest::ToggleMute),
            MediaRequest::SetMute(mute) => backends.audio.request(AudioRequest::SetMute(mute)),
            MediaRequest::ChooseOutput(id) if !id.is_empty() && !id.contains('\0') => {
                backends.audio.request(AudioRequest::Choose(id));
            }
            MediaRequest::ChooseOutput(_) => {}
            MediaRequest::ToggleMicMute => backends.audio.request(AudioRequest::ToggleMicMute),
            MediaRequest::StepBrightness(by) => {
                if let Some(by) = step(by) {
                    backends.backlight.step(by);
                }
            }
            MediaRequest::Player(command) => backends.players.request(command),
        }
    }
}

fn heard(levels: &mut MediaLevels, event: MediaEvent) {
    match event {
        MediaEvent::Audio(audio) => {
            levels.output = audio.output;
            levels.input = audio.input;
            levels.outputs = audio.outputs;
            levels.default_output = audio.default_output;
        }
        MediaEvent::Brightness(level) => levels.brightness = Some(level),
    }
}

fn step(by: f32) -> Option<f32> {
    by.is_finite()
        .then(|| by.clamp(-1.0, 1.0))
        .filter(|by| *by != 0.0)
}
