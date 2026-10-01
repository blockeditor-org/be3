#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Arc, time::Duration};

use be_block::AudioContent as Audio;
use block_plugin_api::AudioStatus;

#[derive(Clone)]
struct Changed(Arc<dyn Fn() + Send + Sync>);

impl Changed {
    fn new(notify: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(notify))
    }

    fn mark(&self) {
        (self.0)();
    }
}

impl AudioPlayer {
    pub(super) fn status(&self) -> AudioStatus {
        AudioStatus {
            playing: self.is_playing(),
            position_micros: self.position().as_micros() as u64,
            duration_micros: self.duration().map(|duration| duration.as_micros() as u64),
            error: self.error().map(str::to_owned),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct AudioPlayer {
    stream: Option<(rodio::OutputStream, rodio::OutputStreamHandle)>,
    sink: Option<rodio::Sink>,
    duration: Option<Duration>,
    error: Option<String>,
    finished: Arc<AtomicBool>,
    changed: Changed,
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioPlayer {
    pub(super) fn new(notify: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            stream: None,
            sink: None,
            duration: None,
            error: None,
            finished: Arc::default(),
            changed: Changed::new(notify),
        }
    }

    pub(super) fn is_playing(&self) -> bool {
        !self.finished.load(Ordering::Acquire)
            && self
                .sink
                .as_ref()
                .is_some_and(|sink| !sink.is_paused() && !sink.empty())
    }

    pub(super) fn position(&self) -> Duration {
        let position = self
            .sink
            .as_ref()
            .map_or(Duration::ZERO, rodio::Sink::get_pos);
        match (self.finished.load(Ordering::Acquire), self.duration) {
            (true, Some(duration)) => duration,
            _ => position,
        }
    }

    pub(super) fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(super) fn reset(&mut self) {
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

    pub(super) fn toggle(&mut self, audio: &Audio) {
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
        let cursor = std::io::Cursor::new(audio.data().to_vec());
        let source = match rodio::Decoder::new(cursor) {
            Ok(source) => source,
            Err(error) => {
                self.error = Some(format!("Could not decode audio: {error}"));
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
    listeners: Vec<(&'static str, wasm_bindgen::closure::Closure<dyn FnMut()>)>,
}

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
pub(super) struct AudioPlayer {
    element: Option<Element>,
    error: Option<String>,
    changed: Changed,
}

#[cfg(target_arch = "wasm32")]
impl AudioPlayer {
    pub(super) fn new(notify: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            element: None,
            error: None,
            changed: Changed::new(notify),
        }
    }

    pub(super) fn is_playing(&self) -> bool {
        self.element
            .as_ref()
            .is_some_and(|held| !held.element.paused())
    }

    pub(super) fn position(&self) -> Duration {
        self.element.as_ref().map_or(Duration::ZERO, |held| {
            duration_from_seconds(held.element.current_time())
        })
    }

    pub(super) fn duration(&self) -> Option<Duration> {
        let seconds = self.element.as_ref()?.element.duration();
        seconds.is_finite().then(|| duration_from_seconds(seconds))
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(super) fn reset(&mut self) {
        if let Some(held) = self.element.take() {
            let _ = held.element.pause();
            let _ = web_sys::Url::revoke_object_url(&held.url);
        }
        self.changed.mark();
    }

    pub(super) fn toggle(&mut self, audio: &Audio) {
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
        match create_element(audio, &self.changed) {
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
fn create_element(audio: &Audio, changed: &Changed) -> Result<Element, String> {
    use wasm_bindgen::{JsCast, closure::Closure};

    let bytes = js_sys::Uint8Array::from(audio.data());
    let parts = js_sys::Array::new();
    parts.push(&bytes.buffer());

    let properties = web_sys::BlobPropertyBag::new();
    properties.set_type(&audio.header().media_type);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &properties)
        .map_err(|_| "Could not create an audio blob".to_owned())?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "Could not create an object URL for the audio".to_owned())?;
    let element = web_sys::HtmlAudioElement::new_with_src(&url).map_err(|_| {
        let _ = web_sys::Url::revoke_object_url(&url);
        "Could not create an audio element".to_owned()
    })?;
    let mut listeners = Vec::new();
    for event in ["play", "pause", "ended", "seeked", "durationchange", "error"] {
        let changed = changed.clone();
        let listener = Closure::<dyn FnMut()>::new(move || changed.mark());
        let _ =
            element.add_event_listener_with_callback(event, listener.as_ref().unchecked_ref());
        listeners.push((event, listener));
    }
    Ok(Element {
        element,
        url,
        listeners,
    })
}
