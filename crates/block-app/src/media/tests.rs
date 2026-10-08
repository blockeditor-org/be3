use std::cell::RefCell;
use std::rc::Rc;

use beui::reactive::{Frame, build, view, with_reactive_scope};
use beui::{Document, Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};

use super::*;

mod a_held_volume_key_keeps_turning_the_volume;
mod brightness_steps_stop_at_the_ends_of_the_backlight;
mod media_keys_go_to_the_player_that_played_last;
mod the_backlight_is_the_firmware_device_when_there_are_several;
mod the_media_keys_send_their_requests_and_reach_past_apps;
mod the_osd_shows_the_level_that_changed;
mod volume_steps_stop_at_full_and_at_silence;

#[derive(Clone, Debug, PartialEq)]
enum Sent {
    Audio(AudioRequest),
    Brightness(f32),
    Player(PlayerRequest),
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
    fn request(&self, request: PlayerRequest) {
        self.0.borrow_mut().push(Sent::Player(request));
    }
}

struct Harness {
    document: Document,
    context: beui::Context,
    recorder: Recorder,
    model: MediaModel,
    osd: Memo<Option<OsdLevel>>,
    actions: Vec<Action>,
}

impl Harness {
    fn new() -> Self {
        let recorder = Recorder::default();
        let mut made = None;
        let backends = Backends {
            audio: Rc::new(recorder.clone()),
            backlight: Rc::new(recorder.clone()),
            players: Rc::new(recorder.clone()),
        };
        let document = build(|| {
            let model = MediaModel::new();
            let actions = register(&model, &backends);
            made = Some((model.osd(), model, actions));
            view! {
                <Frame />
            }
        });
        let (osd, model, actions) = made.expect("the media keys are registered while building");
        let mut harness = Self {
            document,
            context: beui::context(),
            recorder,
            model,
            osd,
            actions,
        };
        harness.frame(Vec::new());
        harness
    }

    fn frame(&mut self, events: Vec<Event>) {
        let document = &mut self.document;
        self.context.run(RawInput { events }, |context| {
            document.show(
                context,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)),
            );
        });
    }

    fn press(&mut self, key: Key) {
        self.frame(vec![
            self::key(key, true, false),
            self::key(key, false, false),
        ]);
    }

    fn apply(&mut self, event: MediaEvent) {
        let model = self.model.clone();
        with_reactive_scope(&mut self.document, move || model.apply(event));
    }

    fn sent(&self) -> Vec<Sent> {
        self.recorder.0.borrow_mut().drain(..).collect()
    }

    fn osd(&self) -> Option<OsdLevel> {
        self.osd.get_untracked()
    }

    fn shows(&self) -> u64 {
        self.model.shows().get_untracked()
    }
}

fn key(key: Key, pressed: bool, repeat: bool) -> Event {
    Event::Key {
        key,
        pressed,
        repeat,
        modifiers: Modifiers::NONE,
    }
}
