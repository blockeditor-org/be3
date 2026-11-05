//! The clip a program copies, with representations rendered on request.

use std::rc::Rc;

use be_lib_ui_types::clipboard::{ClipItem, formats};

/// Renders one representation of a clip when a paste asks for it; answers
/// `None` once it can no longer (the app that copied stopped).
pub type Render = Rc<dyn Fn(Box<dyn FnOnce(Option<Vec<u8>>)>)>;

#[derive(Clone)]
pub enum ClipData {
    Ready(Rc<[u8]>),
    Lazy(Render),
}

#[derive(Clone)]
pub struct ClipEntry {
    pub format: String,
    pub data: ClipData,
}

/// What a copy puts on the clipboard: representations of the same content,
/// richest first. A paste brings the first of the formats the receiver takes
/// that the clip holds, rendering a lazy one only then.
#[derive(Clone, Default)]
pub struct Clip {
    pub entries: Vec<ClipEntry>,
}

impl Clip {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new().with(ClipItem::text(text))
    }

    pub fn png(png: Vec<u8>) -> Self {
        Self::new().with(ClipItem::png(png))
    }

    pub fn items(items: impl IntoIterator<Item = ClipItem>) -> Self {
        items.into_iter().fold(Self::new(), Self::with)
    }

    pub fn with(mut self, item: ClipItem) -> Self {
        self.entries.push(ClipEntry {
            format: item.format,
            data: ClipData::Ready(item.data.into()),
        });
        self
    }

    /// A representation rendered only when a paste asks for it.
    pub fn with_lazy(
        mut self,
        format: impl Into<String>,
        render: impl Fn(Box<dyn FnOnce(Option<Vec<u8>>)>) + 'static,
    ) -> Self {
        self.entries.push(ClipEntry {
            format: format.into(),
            data: ClipData::Lazy(Rc::new(render)),
        });
        self
    }

    pub fn formats(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.format.as_str())
    }

    pub fn holds(&self, format: &str) -> bool {
        self.formats().any(|held| held == format)
    }

    /// The representation of `format` if it is held and already rendered.
    pub fn ready(&self, format: &str) -> Option<&[u8]> {
        self.entries.iter().find_map(|entry| match &entry.data {
            ClipData::Ready(data) if entry.format == format => Some(&data[..]),
            _ => None,
        })
    }

    /// The plain text of the clip, when it holds it ready.
    pub fn plain_text(&self) -> Option<String> {
        self.ready(formats::TEXT)
            .map(|data| String::from_utf8_lossy(data).into_owned())
    }

    /// Only the representations already rendered, as a platform that cannot
    /// render on request puts them on the system clipboard.
    pub fn ready_items(&self) -> Vec<ClipItem> {
        self.entries
            .iter()
            .filter_map(|entry| match &entry.data {
                ClipData::Ready(data) => Some(ClipItem::new(entry.format.clone(), data.to_vec())),
                ClipData::Lazy(_) => None,
            })
            .collect()
    }

    /// Answers with the first of `wanted` the clip holds, rendering it if it
    /// is lazy, and trying the next when a render answers nothing.
    pub fn resolve(&self, wanted: &[String], answer: impl FnOnce(Option<ClipItem>) + 'static) {
        let chosen = wanted
            .iter()
            .filter_map(|format| {
                self.entries
                    .iter()
                    .find(|entry| &entry.format == format)
                    .cloned()
            })
            .collect::<Vec<_>>();
        resolve_in_order(chosen, Box::new(answer));
    }
}

fn resolve_in_order(mut chosen: Vec<ClipEntry>, answer: Box<dyn FnOnce(Option<ClipItem>)>) {
    if chosen.is_empty() {
        return answer(None);
    }
    let entry = chosen.remove(0);
    match entry.data {
        ClipData::Ready(data) => answer(Some(ClipItem::new(entry.format, data.to_vec()))),
        ClipData::Lazy(render) => {
            let format = entry.format;
            render(Box::new(move |data| match data {
                Some(data) => answer(Some(ClipItem::new(format, data))),
                None => resolve_in_order(chosen, answer),
            }));
        }
    }
}

impl std::fmt::Debug for Clip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.formats()).finish()
    }
}

/// The clip a platform last copied, kept to answer pastes with what the system
/// clipboard does not carry (lazy representations, be3 formats) until another
/// program copies. How a platform knows that differs: a change counter where
/// the system has one, otherwise the plain text it wrote, compared with the
/// text there now.
#[derive(Default)]
pub struct HeldClip {
    clip: Option<Clip>,
    written: Option<String>,
}

impl HeldClip {
    pub fn copy(&mut self, clip: Clip) {
        self.written = clip.plain_text();
        self.clip = Some(clip);
    }

    /// The plain text the held clip put on the system clipboard.
    pub fn written(&self) -> Option<&str> {
        self.written.as_deref()
    }

    /// The held clip if it is still the clipboard's (`ours`); dropped otherwise.
    pub fn current(&mut self, ours: bool) -> Option<&Clip> {
        if !ours {
            self.drop_clip();
        }
        self.clip.as_ref()
    }

    /// Whether the system clipboard's text `now` is still what the held clip
    /// wrote: the check of a platform without a change counter.
    pub fn wrote(&self, now: Option<&str>) -> bool {
        self.clip.is_some() && now.filter(|text| !text.is_empty()) == self.written()
    }

    pub fn drop_clip(&mut self) {
        self.clip = None;
        self.written = None;
    }

    pub fn holds(&self) -> bool {
        self.clip.is_some()
    }
}
