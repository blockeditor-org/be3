#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(target_os = "linux")]
mod audio;
#[cfg(target_os = "linux")]
mod backlight;
#[cfg(target_os = "linux")]
mod players;
#[cfg(all(test, target_os = "linux"))]
mod tests;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use beui::icons::{
    ICON_BRIGHTNESS_HIGH, ICON_BRIGHTNESS_LOW, ICON_MIC, ICON_MIC_OFF, ICON_PLAY_PAUSE,
    ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS, ICON_STOP, ICON_VOLUME_DOWN, ICON_VOLUME_OFF,
    ICON_VOLUME_UP,
};
use beui::reactive::{
    Action, Chord, Frame, Memo, NodeRef, Prop, ReadSignal, WriteSignal, component, create_memo,
    create_signal, view,
};
use beui::styled::{LevelOsd, OsdLevel};
use beui::{Key, NodeId, Rect};

pub(crate) const VOLUME_STEP: f32 = 0.05;
pub(crate) const BRIGHTNESS_STEP: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MediaKey {
    VolumeUp,
    VolumeDown,
    Mute,
    MicMute,
    BrightnessUp,
    BrightnessDown,
    PlayPause,
    Next,
    Previous,
    Stop,
}

pub(crate) const BINDINGS: [(Key, MediaKey); 10] = [
    (Key::VolumeUp, MediaKey::VolumeUp),
    (Key::VolumeDown, MediaKey::VolumeDown),
    (Key::VolumeMute, MediaKey::Mute),
    (Key::MicMute, MediaKey::MicMute),
    (Key::BrightnessUp, MediaKey::BrightnessUp),
    (Key::BrightnessDown, MediaKey::BrightnessDown),
    (Key::MediaPlayPause, MediaKey::PlayPause),
    (Key::MediaNext, MediaKey::Next),
    (Key::MediaPrevious, MediaKey::Previous),
    (Key::MediaStop, MediaKey::Stop),
];

impl MediaKey {
    fn id(self) -> &'static str {
        match self {
            Self::VolumeUp => "media.volume_up",
            Self::VolumeDown => "media.volume_down",
            Self::Mute => "media.mute",
            Self::MicMute => "media.mic_mute",
            Self::BrightnessUp => "media.brightness_up",
            Self::BrightnessDown => "media.brightness_down",
            Self::PlayPause => "media.play_pause",
            Self::Next => "media.next",
            Self::Previous => "media.previous",
            Self::Stop => "media.stop",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::VolumeUp => "Turn the volume up",
            Self::VolumeDown => "Turn the volume down",
            Self::Mute => "Mute or unmute the sound",
            Self::MicMute => "Mute or unmute the microphone",
            Self::BrightnessUp => "Brighten the screen",
            Self::BrightnessDown => "Dim the screen",
            Self::PlayPause => "Play or pause",
            Self::Next => "Next track",
            Self::Previous => "Previous track",
            Self::Stop => "Stop playing",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::VolumeUp => ICON_VOLUME_UP,
            Self::VolumeDown => ICON_VOLUME_DOWN,
            Self::Mute => ICON_VOLUME_OFF,
            Self::MicMute => ICON_MIC_OFF,
            Self::BrightnessUp => ICON_BRIGHTNESS_HIGH,
            Self::BrightnessDown => ICON_BRIGHTNESS_LOW,
            Self::PlayPause => ICON_PLAY_PAUSE,
            Self::Next => ICON_SKIP_NEXT,
            Self::Previous => ICON_SKIP_PREVIOUS,
            Self::Stop => ICON_STOP,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AudioRequest {
    Volume(f32),
    ToggleMute,
    ToggleMicMute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlayerRequest {
    PlayPause,
    Next,
    Previous,
    Stop,
}

impl PlayerRequest {
    fn method(self) -> &'static str {
        match self {
            Self::PlayPause => "PlayPause",
            Self::Next => "Next",
            Self::Previous => "Previous",
            Self::Stop => "Stop",
        }
    }
}

pub(crate) trait Audio {
    fn request(&self, request: AudioRequest);
}

pub(crate) trait Backlight {
    fn step(&self, by: f32);
}

pub(crate) trait Players {
    fn request(&self, request: PlayerRequest);
}

#[derive(Clone)]
pub(crate) struct Backends {
    pub(crate) audio: Rc<dyn Audio>,
    pub(crate) backlight: Rc<dyn Backlight>,
    pub(crate) players: Rc<dyn Players>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Level {
    pub(crate) volume: f32,
    pub(crate) muted: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct AudioLevels {
    pub(crate) output: Option<Level>,
    pub(crate) input: Option<Level>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MediaEvent {
    Audio(AudioLevels),
    Brightness(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shown {
    Volume,
    Microphone,
    Brightness,
}

#[derive(Clone)]
pub(crate) struct MediaModel {
    pub(crate) audio: ReadSignal<AudioLevels>,
    set_audio: WriteSignal<AudioLevels>,
    pub(crate) brightness: ReadSignal<Option<f32>>,
    set_brightness: WriteSignal<Option<f32>>,
    shown: ReadSignal<Option<Shown>>,
    set_shown: WriteSignal<Option<Shown>>,
    shows: ReadSignal<u64>,
    set_shows: WriteSignal<u64>,
}

impl MediaModel {
    pub(crate) fn new() -> Self {
        let (audio, set_audio) = create_signal(AudioLevels::default());
        let (brightness, set_brightness) = create_signal(None);
        let (shown, set_shown) = create_signal(None);
        let (shows, set_shows) = create_signal(0);
        Self {
            audio,
            set_audio,
            brightness,
            set_brightness,
            shown,
            set_shown,
            shows,
            set_shows,
        }
    }

    pub(crate) fn apply(&self, event: MediaEvent) {
        match event {
            MediaEvent::Audio(levels) if self.audio.get_untracked() != levels => {
                self.set_audio.set(levels);
            }
            MediaEvent::Brightness(level) if self.brightness.get_untracked() != Some(level) => {
                self.set_brightness.set(Some(level));
            }
            MediaEvent::Audio(_) | MediaEvent::Brightness(_) => {}
        }
    }

    fn show(&self, what: Shown) {
        if self.shown.get_untracked() != Some(what) {
            self.set_shown.set(Some(what));
        }
        self.set_shows.update(|shows| *shows += 1);
    }

    pub(crate) fn osd(&self) -> Memo<Option<OsdLevel>> {
        let model = self.clone();
        create_memo(move || match model.shown.get()? {
            Shown::Volume => model
                .audio
                .with(|audio| audio.output)
                .map(|level| OsdLevel {
                    glyph: volume_glyph(level).to_owned(),
                    label: "Volume".to_owned(),
                    level: level.volume,
                    muted: level.muted,
                }),
            Shown::Microphone => model.audio.with(|audio| audio.input).map(|level| OsdLevel {
                glyph: match level.muted {
                    true => ICON_MIC_OFF,
                    false => ICON_MIC,
                }
                .to_owned(),
                label: "Microphone".to_owned(),
                level: level.volume,
                muted: level.muted,
            }),
            Shown::Brightness => model.brightness.get().map(|level| OsdLevel {
                glyph: match level < 0.5 {
                    true => ICON_BRIGHTNESS_LOW,
                    false => ICON_BRIGHTNESS_HIGH,
                }
                .to_owned(),
                label: "Brightness".to_owned(),
                level,
                muted: false,
            }),
        })
    }

    pub(crate) fn shows(&self) -> ReadSignal<u64> {
        self.shows.clone()
    }
}

fn volume_glyph(level: Level) -> &'static str {
    match level.volume {
        _ if level.muted || level.volume <= 0.0 => ICON_VOLUME_OFF,
        volume if volume < 0.5 => ICON_VOLUME_DOWN,
        _ => ICON_VOLUME_UP,
    }
}

pub(crate) fn register(model: &MediaModel, backends: &Backends) -> Vec<Action> {
    BINDINGS
        .iter()
        .map(|&(key, media_key)| {
            let (model, backends) = (model.clone(), backends.clone());
            Action::new(media_key.id(), media_key.label(), move || {
                press(media_key, &model, &backends);
            })
            .glyph(media_key.glyph())
            .shortcut(Chord::key(key))
            .intercepts()
            .register()
        })
        .collect()
}

fn press(key: MediaKey, model: &MediaModel, backends: &Backends) {
    let shown = match key {
        MediaKey::VolumeUp => {
            backends.audio.request(AudioRequest::Volume(VOLUME_STEP));
            Shown::Volume
        }
        MediaKey::VolumeDown => {
            backends.audio.request(AudioRequest::Volume(-VOLUME_STEP));
            Shown::Volume
        }
        MediaKey::Mute => {
            backends.audio.request(AudioRequest::ToggleMute);
            Shown::Volume
        }
        MediaKey::MicMute => {
            backends.audio.request(AudioRequest::ToggleMicMute);
            Shown::Microphone
        }
        MediaKey::BrightnessUp => {
            backends.backlight.step(BRIGHTNESS_STEP);
            Shown::Brightness
        }
        MediaKey::BrightnessDown => {
            backends.backlight.step(-BRIGHTNESS_STEP);
            Shown::Brightness
        }
        MediaKey::PlayPause => return backends.players.request(PlayerRequest::PlayPause),
        MediaKey::Next => return backends.players.request(PlayerRequest::Next),
        MediaKey::Previous => return backends.players.request(PlayerRequest::Previous),
        MediaKey::Stop => return backends.players.request(PlayerRequest::Stop),
    };
    model.show(shown);
}

struct Media {
    model: MediaModel,
    events: Option<Receiver<MediaEvent>>,
}

thread_local! {
    static MEDIA: RefCell<Option<Media>> = const { RefCell::new(None) };
}

pub(crate) fn install(desktop: bool) {
    let model = MediaModel::new();
    let events = desktop.then(|| start(&model)).flatten();
    MEDIA.with(|media| *media.borrow_mut() = Some(Media { model, events }));
}

#[cfg(target_os = "linux")]
fn start(model: &MediaModel) -> Option<Receiver<MediaEvent>> {
    let (sender, events) = crate::host::waking_channel();
    let backends = Backends {
        audio: Rc::new(audio::PulseAudio::start(sender.clone())),
        backlight: Rc::new(backlight::LogindBacklight::start(sender)),
        players: Rc::new(players::Mpris::start()),
    };
    register(model, &backends);
    Some(events)
}

#[cfg(not(target_os = "linux"))]
fn start(_model: &MediaModel) -> Option<Receiver<MediaEvent>> {
    None
}

pub(crate) fn model() -> Option<MediaModel> {
    MEDIA.with(|media| media.borrow().as_ref().map(|media| media.model.clone()))
}

pub(crate) fn frame() {
    let received: Vec<MediaEvent> = MEDIA.with(|media| {
        media
            .borrow()
            .as_ref()
            .and_then(|media| media.events.as_ref())
            .map(|events| events.try_iter().collect())
            .unwrap_or_default()
    });
    if let Some(model) = model() {
        for event in received {
            model.apply(event);
        }
    }
}

#[component]
pub(crate) fn Osd(anchor: NodeRef, screens: Prop<Vec<Rect>>) -> NodeId {
    let Some(model) = model() else {
        return view! {
            <Frame />
        };
    };
    view! {
        <LevelOsd anchor level={model.osd()} shown={model.shows()} screens id="media.osd" />
    }
}
