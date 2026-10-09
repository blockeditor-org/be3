mod audio;
mod backlight;
mod players;
#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::mpsc::Receiver;

use block_plugin_api::{MediaLevel, MediaLevels, MediaRequest, PlayerCommand};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AudioRequest {
    Step(f32),
    Set(f32),
    ToggleMute,
    ToggleMicMute,
}

pub(crate) trait Audio {
    fn request(&self, request: AudioRequest);
}

pub(crate) trait Backlight {
    fn step(&self, by: f32);
}

pub(crate) trait Players {
    fn request(&self, command: PlayerCommand);
}

#[derive(Clone)]
pub(crate) struct Backends {
    pub(crate) audio: Rc<dyn Audio>,
    pub(crate) backlight: Rc<dyn Backlight>,
    pub(crate) players: Rc<dyn Players>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MediaEvent {
    Audio {
        output: Option<MediaLevel>,
        input: Option<MediaLevel>,
    },
    Brightness(f32),
}

pub(crate) struct Media {
    backends: Backends,
    events: Receiver<MediaEvent>,
    levels: MediaLevels,
}

impl Media {
    pub(crate) fn start() -> Self {
        let (sender, events) = crate::host::waking_channel();
        let backends = Backends {
            audio: Rc::new(audio::PulseAudio::start(sender.clone())),
            backlight: Rc::new(backlight::LogindBacklight::start(sender)),
            players: Rc::new(players::Mpris::start()),
        };
        Self::with(backends, events)
    }

    pub(crate) fn with(backends: Backends, events: Receiver<MediaEvent>) -> Self {
        Self {
            backends,
            events,
            levels: MediaLevels::default(),
        }
    }

    pub(crate) fn request(&self, request: MediaRequest) {
        let backends = &self.backends;
        match request {
            MediaRequest::StepVolume(by) => {
                if let Some(by) = step(by) {
                    backends.audio.request(AudioRequest::Step(by));
                }
            }
            MediaRequest::SetVolume(level) if level.is_finite() => {
                backends.audio.request(AudioRequest::Set(level.clamp(0.0, 1.0)));
            }
            MediaRequest::SetVolume(_) => {}
            MediaRequest::ToggleMute => backends.audio.request(AudioRequest::ToggleMute),
            MediaRequest::ToggleMicMute => backends.audio.request(AudioRequest::ToggleMicMute),
            MediaRequest::StepBrightness(by) => {
                if let Some(by) = step(by) {
                    backends.backlight.step(by);
                }
            }
            MediaRequest::Player(command) => backends.players.request(command),
        }
    }

    pub(crate) fn frame(&mut self) -> Option<MediaLevels> {
        let before = self.levels;
        for event in self.events.try_iter() {
            match event {
                MediaEvent::Audio { output, input } => {
                    self.levels.output = output;
                    self.levels.input = input;
                }
                MediaEvent::Brightness(level) => self.levels.brightness = Some(level),
            }
        }
        (self.levels != before).then_some(self.levels)
    }
}

fn step(by: f32) -> Option<f32> {
    by.is_finite()
        .then(|| by.clamp(-1.0, 1.0))
        .filter(|by| *by != 0.0)
}
