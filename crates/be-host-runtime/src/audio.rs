//! Apps' audio (guides/apps/editor.md, "Holding the pointer and sound"). The players live in a
//! crate of their own (be-host-audio), which be-host hands `install`: the runtime links no
//! audio library, so `be`, which runs it headless, starts where there is none (ALSA on Linux).

use std::sync::OnceLock;

use be_sdk_api::{AudioSource, AudioStatus};

/// One editor instance's player, which an `AudioOutput` opens on its first `PlayAudio`.
pub trait AudioPlayer {
    /// Holds `source`, stopping what played before.
    fn load(&mut self, source: AudioSource);
    /// Plays the source from the start, or pauses or resumes it.
    fn toggle(&mut self);
    /// Stops and forgets the source.
    fn reset(&mut self);
    fn status(&self) -> AudioStatus;
}

/// Opens a player that calls its argument whenever its status changes, from any thread.
pub type AudioOutput = fn(Box<dyn Fn() + Send + Sync>) -> Box<dyn AudioPlayer>;

static OUTPUT: OnceLock<AudioOutput> = OnceLock::new();

pub(super) fn set_output(output: AudioOutput) {
    let _ = OUTPUT.set(output);
}

pub(super) fn open(changed: Box<dyn Fn() + Send + Sync>) -> Box<dyn AudioPlayer> {
    match OUTPUT.get() {
        Some(output) => output(changed),
        None => Box::new(Silent {
            changed,
            loaded: false,
            refused: false,
        }),
    }
}

/// The player of a runtime installed with no audio output (`install_headless`), which says so
/// when asked to play.
struct Silent {
    changed: Box<dyn Fn() + Send + Sync>,
    loaded: bool,
    refused: bool,
}

impl AudioPlayer for Silent {
    fn load(&mut self, _: AudioSource) {
        self.loaded = true;
        self.refused = false;
        (self.changed)();
    }

    fn toggle(&mut self) {
        self.refused = self.loaded;
        (self.changed)();
    }

    fn reset(&mut self) {
        self.loaded = false;
        self.refused = false;
        (self.changed)();
    }

    fn status(&self) -> AudioStatus {
        AudioStatus {
            playing: false,
            position_micros: 0,
            duration_micros: None,
            error: self
                .refused
                .then(|| "This host has no audio output".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests;
