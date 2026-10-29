use std::cell::RefCell;
use std::rc::Rc;

use super::*;

mod brightness_steps_stop_at_the_ends_of_the_backlight;
mod media_keys_go_to_the_player_that_played_last;
mod requests_from_the_desktop_reach_their_backends_within_bounds;
mod the_backlight_is_the_firmware_device_when_there_are_several;
mod the_levels_are_reported_once_they_change;
mod volume_steps_stop_at_full_and_at_silence;

const VOLUME_STEP: f32 = 0.05;
const BRIGHTNESS_STEP: f32 = 0.05;

#[derive(Clone, Debug, PartialEq)]
enum Sent {
    Audio(AudioRequest),
    Brightness(f32),
    Player(PlayerCommand),
}

#[derive(Clone, Default)]
struct Recorder(Rc<RefCell<Vec<Sent>>>);

impl Audio for Recorder {
    fn request(&self, request: AudioRequest) {
        self.0.borrow_mut().push(Sent::Audio(request));
    }
}

impl Backlight for Recorder {
    fn step(&self, by: f32) {
        self.0.borrow_mut().push(Sent::Brightness(by));
    }
}

impl Players for Recorder {
    fn request(&self, command: PlayerCommand) {
        self.0.borrow_mut().push(Sent::Player(command));
    }
}

fn recorded() -> (Media, Recorder, std::sync::mpsc::Sender<MediaEvent>) {
    let recorder = Recorder::default();
    let backends = Backends {
        audio: Rc::new(recorder.clone()),
        backlight: Rc::new(recorder.clone()),
        players: Rc::new(recorder.clone()),
    };
    let (sender, events) = std::sync::mpsc::channel();
    (Media::with(backends, events), recorder, sender)
}

impl Recorder {
    fn sent(&self) -> Vec<Sent> {
        self.0.borrow_mut().drain(..).collect()
    }
}
