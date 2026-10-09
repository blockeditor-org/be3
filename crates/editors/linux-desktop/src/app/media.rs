use block_editor_beui::beui::icons::{
    ICON_BRIGHTNESS_HIGH, ICON_BRIGHTNESS_LOW, ICON_MIC, ICON_MIC_OFF, ICON_PLAY_PAUSE,
    ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS, ICON_STOP, ICON_VOLUME_DOWN, ICON_VOLUME_OFF,
    ICON_VOLUME_UP,
};
use block_editor_beui::beui::reactive::{
    Action, Chord, component, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{LevelOsd, OsdLevel};
use block_editor_beui::beui::{Key, NodeId};
use block_editor_beui::{Editor, MediaLevel, MediaLevels, MediaRequest, PlayerCommand};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    Volume,
    Microphone,
    Brightness,
}

impl MediaKey {
    fn id(self) -> &'static str {
        match self {
            Self::VolumeUp => "desktop.media.volume_up",
            Self::VolumeDown => "desktop.media.volume_down",
            Self::Mute => "desktop.media.mute",
            Self::MicMute => "desktop.media.mic_mute",
            Self::BrightnessUp => "desktop.media.brightness_up",
            Self::BrightnessDown => "desktop.media.brightness_down",
            Self::PlayPause => "desktop.media.play_pause",
            Self::Next => "desktop.media.next",
            Self::Previous => "desktop.media.previous",
            Self::Stop => "desktop.media.stop",
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

    fn request(self) -> (MediaRequest, Option<Shown>) {
        match self {
            Self::VolumeUp => (MediaRequest::StepVolume(VOLUME_STEP), Some(Shown::Volume)),
            Self::VolumeDown => (MediaRequest::StepVolume(-VOLUME_STEP), Some(Shown::Volume)),
            Self::Mute => (MediaRequest::ToggleMute, Some(Shown::Volume)),
            Self::MicMute => (MediaRequest::ToggleMicMute, Some(Shown::Microphone)),
            Self::BrightnessUp => (
                MediaRequest::StepBrightness(BRIGHTNESS_STEP),
                Some(Shown::Brightness),
            ),
            Self::BrightnessDown => (
                MediaRequest::StepBrightness(-BRIGHTNESS_STEP),
                Some(Shown::Brightness),
            ),
            Self::PlayPause => (MediaRequest::Player(PlayerCommand::PlayPause), None),
            Self::Next => (MediaRequest::Player(PlayerCommand::Next), None),
            Self::Previous => (MediaRequest::Player(PlayerCommand::Previous), None),
            Self::Stop => (MediaRequest::Player(PlayerCommand::Stop), None),
        }
    }
}

fn volume_glyph(level: MediaLevel) -> &'static str {
    match level.level {
        _ if level.muted || level.level <= 0.0 => ICON_VOLUME_OFF,
        volume if volume < 0.5 => ICON_VOLUME_DOWN,
        _ => ICON_VOLUME_UP,
    }
}

fn osd_level(shown: Shown, levels: MediaLevels) -> Option<OsdLevel> {
    match shown {
        Shown::Volume => levels.output.map(|level| OsdLevel {
            glyph: volume_glyph(level).to_owned(),
            label: "Volume".to_owned(),
            level: level.level,
            muted: level.muted,
        }),
        Shown::Microphone => levels.input.map(|level| OsdLevel {
            glyph: match level.muted {
                true => ICON_MIC_OFF,
                false => ICON_MIC,
            }
            .to_owned(),
            label: "Microphone".to_owned(),
            level: level.level,
            muted: level.muted,
        }),
        Shown::Brightness => levels.brightness.map(|level| OsdLevel {
            glyph: match level < 0.5 {
                true => ICON_BRIGHTNESS_LOW,
                false => ICON_BRIGHTNESS_HIGH,
            }
            .to_owned(),
            label: "Brightness".to_owned(),
            level,
            muted: false,
        }),
    }
}

#[component]
pub(super) fn MediaKeys(editor: Editor) -> NodeId {
    let levels = editor.media();
    let (shown, set_shown) = create_signal(None);
    let (shows, set_shows) = create_signal(0_u64);
    for (key, media_key) in BINDINGS {
        let editor = editor.clone();
        let (set_shown, set_shows) = (set_shown.clone(), set_shows.clone());
        Action::new(media_key.id(), media_key.label(), move || {
            let (request, showing) = media_key.request();
            editor.request_media(request);
            if let Some(showing) = showing {
                set_shown.set(Some(showing));
                set_shows.update(|shows| *shows += 1);
            }
        })
        .glyph(media_key.glyph())
        .shortcut(Chord::key(key))
        .intercepts()
        .register();
    }
    let level = create_memo(move || osd_level(shown.get()?, levels.get()));
    view! {
        <LevelOsd level={level} shown={shows} id="desktop.osd" />
    }
}
