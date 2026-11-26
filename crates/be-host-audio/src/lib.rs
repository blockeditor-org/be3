//! Apps' audio output (guides/apps/editor.md, "Holding the pointer and sound"): be-host hands
//! `open` to `be_host_runtime::install`, which opens a player with it for each editor
//! instance that plays. `be`, which runs host components headless, does not link this crate,
//! so it needs no audio library (ALSA on Linux) to start.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Arc, time::Duration};

use be_sdk_api::{AudioSource, AudioStatus};

/// A player for one editor instance, which calls `changed` whenever its status changes.
pub fn open(changed: Box<dyn Fn() + Send + Sync>) -> Box<dyn be_host_runtime::AudioPlayer> {
    Box::new(AudioPlayer::new(Changed(Arc::from(changed))))
}

struct Source {
    media_type: String,
    bytes: Arc<[u8]>,
}

impl From<AudioSource> for Source {
    fn from(source: AudioSource) -> Self {
        Self {
            media_type: source.media_type,
            bytes: source.bytes.into(),
        }
    }
}

#[derive(Clone)]
struct Changed(Arc<dyn Fn() + Send + Sync>);

impl Changed {
    fn mark(&self) {
        (self.0)();
    }
}

impl be_host_runtime::AudioPlayer for AudioPlayer {
    fn load(&mut self, source: AudioSource) {
        self.reset();
        self.source = Some(source.into());
    }

    fn toggle(&mut self) {
        AudioPlayer::toggle(self);
    }

    fn reset(&mut self) {
        AudioPlayer::reset(self);
    }

    fn status(&self) -> AudioStatus {
        AudioStatus {
            playing: self.is_playing(),
            position_micros: self.position().as_micros() as u64,
            duration_micros: self.duration().map(|duration| duration.as_micros() as u64),
            error: self.error().map(str::to_owned),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct AudioPlayer {
    source: Option<Source>,
    stream: Option<(rodio::OutputStream, rodio::OutputStreamHandle)>,
    sink: Option<rodio::Sink>,
    duration: Option<Duration>,
    error: Option<String>,
    finished: Arc<AtomicBool>,
    changed: Changed,
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioPlayer {
    fn new(changed: Changed) -> Self {
        Self {
            source: None,
            stream: None,
            sink: None,
            duration: None,
            error: None,
            finished: Arc::default(),
            changed,
        }
    }

    fn is_playing(&self) -> bool {
        !self.finished.load(Ordering::Acquire)
            && self
                .sink
                .as_ref()
                .is_some_and(|sink| !sink.is_paused() && !sink.empty())
    }

    fn position(&self) -> Duration {
        let position = self
            .sink
            .as_ref()
            .map_or(Duration::ZERO, rodio::Sink::get_pos);
        match (self.finished.load(Ordering::Acquire), self.duration) {
            (true, Some(duration)) => duration,
            _ => position,
        }
    }

    fn duration(&self) -> Option<Duration> {
        self.duration
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn reset(&mut self) {
        self.source = None;
        self.sink = None;
        self.duration = None;
        self.changed.mark();
    }

    fn ensure_stream(&mut self) -> bool {
        if self.stream.is_none() {
            match rodio::OutputStream::try_default() {
                Ok(stream) => self.stream = Some(stream),
                Err(error) => {
                    self.error = Some(format!("Could not open audio output: {error}"));
                    return false;
                }
            }
        }
        true
    }

    fn toggle(&mut self) {
        self.changed.mark();
        if let Some(sink) = &self.sink
            && !self.finished.load(Ordering::Acquire)
        {
            if sink.is_paused() {
                sink.play();
            } else {
                sink.pause();
            }
            return;
        }
        self.sink = None;
        self.error = None;
        let Some(source) = &self.source else {
            return;
        };
        let (media_type, bytes) = (source.media_type.clone(), Arc::clone(&source.bytes));
        if !self.ensure_stream() {
            return;
        }
        let handle = &self.stream.as_ref().expect("just ensured").1;
        let sink = match rodio::Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(error) => {
                self.error = Some(format!("Could not start playback: {error}"));
                return;
            }
        };
        let cursor = std::io::Cursor::new(bytes);
        let source = match rodio::Decoder::new(cursor) {
            Ok(source) => source,
            Err(error) => {
                self.error = Some(format!("Could not decode {media_type}: {error}"));
                return;
            }
        };
        self.duration = rodio::Source::total_duration(&source);
        let finished = Arc::new(AtomicBool::new(false));
        self.finished = Arc::clone(&finished);
        let changed = self.changed.clone();
        sink.append(source);
        sink.append(rodio::source::EmptyCallback::<f32>::new(Box::new(
            move || {
                finished.store(true, Ordering::Release);
                changed.mark();
            },
        )));
        self.sink = Some(sink);
    }
}

#[cfg(target_arch = "wasm32")]
struct Element {
    element: web_sys::HtmlAudioElement,
    url: String,
    listeners: Vec<Listener>,
}

#[cfg(target_arch = "wasm32")]
type Listener = (&'static str, wasm_bindgen::closure::Closure<dyn FnMut()>);

#[cfg(target_arch = "wasm32")]
impl Drop for Element {
    fn drop(&mut self) {
        use wasm_bindgen::JsCast;

        for (event, listener) in &self.listeners {
            let _ = self
                .element
                .remove_event_listener_with_callback(event, listener.as_ref().unchecked_ref());
        }
    }
}

#[cfg(target_arch = "wasm32")]
struct AudioPlayer {
    source: Option<Source>,
    element: Option<Element>,
    error: Option<String>,
    changed: Changed,
}

#[cfg(target_arch = "wasm32")]
impl AudioPlayer {
    fn new(changed: Changed) -> Self {
        Self {
            source: None,
            element: None,
            error: None,
            changed,
        }
    }

    fn is_playing(&self) -> bool {
        self.element
            .as_ref()
            .is_some_and(|held| !held.element.paused())
    }

    fn position(&self) -> Duration {
        self.element.as_ref().map_or(Duration::ZERO, |held| {
            duration_from_seconds(held.element.current_time())
        })
    }

    fn duration(&self) -> Option<Duration> {
        let seconds = self.element.as_ref()?.element.duration();
        seconds.is_finite().then(|| duration_from_seconds(seconds))
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn reset(&mut self) {
        self.source = None;
        if let Some(held) = self.element.take() {
            let _ = held.element.pause();
            let _ = web_sys::Url::revoke_object_url(&held.url);
        }
        self.changed.mark();
    }

    fn toggle(&mut self) {
        self.changed.mark();
        if let Some(held) = &self.element {
            let result = if held.element.paused() {
                held.element.play().map(|_| ())
            } else {
                held.element.pause()
            };
            if result.is_err() {
                self.error = Some("Could not control playback".into());
            }
            return;
        }
        self.error = None;
        let Some(source) = &self.source else {
            return;
        };
        match create_element(source, &self.changed) {
            Ok(held) => {
                let _ = held.element.play();
                self.element = Some(held);
            }
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn duration_from_seconds(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds).unwrap_or(Duration::ZERO)
}

#[cfg(target_arch = "wasm32")]
fn create_element(source: &Source, changed: &Changed) -> Result<Element, String> {
    use wasm_bindgen::{JsCast, closure::Closure};

    let bytes = js_sys::Uint8Array::from(&source.bytes[..]);
    let parts = js_sys::Array::new();
    parts.push(&bytes.buffer());

    let properties = web_sys::BlobPropertyBag::new();
    properties.set_type(&source.media_type);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &properties)
        .map_err(|_| "Could not create an audio blob".to_owned())?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "Could not create an object URL for the audio".to_owned())?;
    let element = web_sys::HtmlAudioElement::new_with_src(&url).map_err(|_| {
        let _ = web_sys::Url::revoke_object_url(&url);
        "Could not create an audio element".to_owned()
    })?;
    let mut listeners = Vec::new();
    for event in [
        "play",
        "pause",
        "ended",
        "seeked",
        "durationchange",
        "error",
    ] {
        let changed = changed.clone();
        let listener = Closure::<dyn FnMut()>::new(move || changed.mark());
        let _ = element.add_event_listener_with_callback(event, listener.as_ref().unchecked_ref());
        listeners.push((event, listener));
    }
    Ok(Element {
        element,
        url,
        listeners,
    })
}
