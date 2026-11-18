//! The clip a program copies, with representations rendered on request.

use std::rc::Rc;

use be_lib_ui_types::clipboard::{ClipBody, ClipItem, formats};

/// Renders one representation of a clip when a paste asks for it; answers
/// `None` once it can no longer (the app that copied stopped).
pub type Render = Rc<dyn Fn(RenderAnswer)>;

/// Where a render answers: the data and the blocks it names (`ClipBody`).
pub type RenderAnswer = Box<dyn FnOnce(Option<ClipBody>)>;

#[derive(Clone)]
pub enum ClipData {
    Ready(Rc<ClipBody>),
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
            format: item.format.clone(),
            data: ClipData::Ready(Rc::new(item.into_body())),
        });
        self
    }

    /// A representation rendered only when a paste asks for it.
    pub fn with_lazy(
        mut self,
        format: impl Into<String>,
        render: impl Fn(RenderAnswer) + 'static,
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
            ClipData::Ready(body) if entry.format == format => Some(&body.data[..]),
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
                ClipData::Ready(body) => Some(ClipItem::of_body(
                    entry.format.clone(),
                    ClipBody::clone(body),
                )),
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
        ClipData::Ready(body) => answer(Some(ClipItem::of_body(
            entry.format,
            ClipBody::clone(&body),
        ))),
        ClipData::Lazy(render) => {
            let format = entry.format;
            render(Box::new(move |body| match body {
                Some(body) => answer(Some(ClipItem::of_body(format, body))),
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
    /// wrote: the check of a platform without a change counter. Line breaks
    /// are compared as `\n` on both sides, since a platform reads the text
    /// back through `text_from_system`.
    pub fn wrote(&self, now: Option<&str>) -> bool {
        let lf = |text: &str| text_from_system(text.to_owned());
        self.clip.is_some() && now.filter(|text| !text.is_empty()).map(lf) == self.written().map(lf)
    }

    pub fn drop_clip(&mut self) {
        self.clip = None;
        self.written = None;
    }

    pub fn holds(&self) -> bool {
        self.clip.is_some()
    }
}

/// The `items` whose costs fit within `budget`, in order, leaving out each one
/// that would overrun it: what a system clipboard with a size limit carries
/// of a clip, the rest staying in the held clip.
pub fn fitting<T>(
    items: impl IntoIterator<Item = T>,
    budget: usize,
    cost: impl Fn(&T) -> usize,
) -> Vec<T> {
    let mut left = budget;
    items
        .into_iter()
        .filter(|item| {
            let cost = cost(item);
            let fits = cost <= left;
            if fits {
                left -= cost;
            }
            fits
        })
        .collect()
}

/// `text` as a system clipboard with a size limit carries it: whole when it is
/// at most `units` UTF-16 code units (what an Android Parcel counts), otherwise
/// cut at a character boundary and ended with `notice`, the two together
/// within `units`. The whole text stays in the held clip for pastes in the
/// program.
pub fn cut_short<'a>(text: &'a str, units: usize, notice: &str) -> std::borrow::Cow<'a, str> {
    if utf16_units(text) <= units {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut left = units.saturating_sub(utf16_units(notice));
    let end = text
        .char_indices()
        .find(|(_, char)| {
            let fits = char.len_utf16() <= left;
            left = left.saturating_sub(char.len_utf16());
            !fits
        })
        .map_or(text.len(), |(at, _)| at);
    std::borrow::Cow::Owned(format!("{}{notice}", &text[..end]))
}

/// How many UTF-16 code units `text` takes.
pub fn utf16_units(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// Plain text read from a system clipboard, with its line breaks as beui keeps
/// them: `\n`. Windows programs write `\r\n`, and a `\r` alone ends a line
/// too (old Mac programs), so each becomes one `\n`. Every platform passes the
/// text it reads through this, so a paste never brings a `\r`.
pub fn text_from_system(text: String) -> String {
    if !text.contains('\r') {
        return text;
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Plain text with `\r\n` line breaks, as Windows programs expect on the
/// clipboard: what a copy writes there.
pub fn text_with_crlf(text: &str) -> String {
    text_from_system(text.to_owned()).replace('\n', "\r\n")
}

/// The link `text` is, when it is one http or https URL and nothing else, as a share sheet
/// takes it apart from text (the web's `ShareData.url`), so the target opens it as a link.
pub fn shared_link(text: &str) -> Option<&str> {
    let text = text.trim();
    let scheme = ["https://", "http://"].iter().any(|scheme| {
        text.get(..scheme.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
    });
    (scheme && !text.contains(char::is_whitespace)).then_some(text)
}

#[cfg(test)]
mod tests;
