//! The basic schema's `text/html`: written from a fragment's marks for other
//! programs, and read back from what a browser or another program copied,
//! which may be anything. Only what maps to the schema's marks is read; the
//! rest is kept as text.

use std::collections::BTreeMap;
use std::ops::Range;

use super::{
    BOLD, BULLET, CHECK, CHECKED, CODE, CODE_BLOCK, HEADING, INDENT, ITALIC, LINK, LIST, MENTION,
    NUMBER, QUOTE, STRIKETHROUGH, UNDERLINE,
};
use crate::core::{MarkValue, TextChunk, TextFragment, TextMark, TextShape};
use crate::html::{Token, escape, tokens};
use crate::rich::VOID;

pub(super) const NAME: &str = "basic";

const INLINE_ORDER: [(&str, &str); 5] = [
    (BOLD, "b"),
    (ITALIC, "i"),
    (UNDERLINE, "u"),
    (STRIKETHROUGH, "s"),
    (CODE, "code"),
];

/// Where a line goes when written.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    Pre,
    List(bool),
    Quote,
}

pub(super) fn write(fragment: &TextFragment) -> Option<String> {
    if fragment.is_empty() {
        return None;
    }
    let TextChunk { bytes, marks } = fragment.joined();
    let mut html = String::new();
    let mut container: Option<Container> = None;
    let mut start = 0;
    while start <= bytes.len() {
        let end = bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |at| start + at);
        let props: BTreeMap<&str, &MarkValue> = marks
            .iter()
            .filter(|mark| mark.block && mark.range.start == start)
            .map(|mark| (mark.key.as_str(), &mark.value))
            .collect();
        let list = props.get(LIST).and_then(|kind| match kind {
            MarkValue::Text(kind) => Some(kind.as_str()),
            _ => None,
        });
        let wanted = if props.contains_key(CODE_BLOCK) {
            Some(Container::Pre)
        } else if let Some(kind) = list {
            Some(Container::List(kind == NUMBER))
        } else if props
            .get(QUOTE)
            .is_some_and(|quote| **quote == MarkValue::Bool(true))
        {
            Some(Container::Quote)
        } else {
            None
        };
        if wanted != container {
            close(&mut html, container);
            match wanted {
                Some(Container::Pre) => html.push_str("<pre><code>"),
                Some(Container::List(true)) => html.push_str("<ol>"),
                Some(Container::List(false)) => html.push_str("<ul>"),
                Some(Container::Quote) => html.push_str("<blockquote>"),
                None => {}
            }
            container = wanted;
        } else if container == Some(Container::Pre) {
            html.push('\n');
        }
        let line = start..end;
        match container {
            Some(Container::Pre) => {
                html.push_str(&escape(
                    &String::from_utf8_lossy(&bytes[line]).replace(VOID, ""),
                ));
            }
            Some(Container::List(_)) => {
                html.push_str("<li>");
                if list == Some(CHECK) {
                    let checked = props
                        .get(CHECKED)
                        .is_some_and(|on| **on == MarkValue::Bool(true));
                    html.push_str(match checked {
                        true => "<input type=\"checkbox\" checked disabled> ",
                        false => "<input type=\"checkbox\" disabled> ",
                    });
                }
                inline(&mut html, &bytes, &marks, line);
                html.push_str("</li>");
            }
            _ => {
                let heading = match props.get(HEADING) {
                    Some(MarkValue::Int(level)) if (1..=6).contains(level) => Some(*level),
                    _ => None,
                };
                match heading {
                    Some(level) => html.push_str(&format!("<h{level}>")),
                    None => html.push_str("<p>"),
                }
                if line.is_empty() {
                    html.push_str("<br>");
                }
                inline(&mut html, &bytes, &marks, line);
                match heading {
                    Some(level) => html.push_str(&format!("</h{level}>")),
                    None => html.push_str("</p>"),
                }
            }
        }
        if end == bytes.len() {
            break;
        }
        start = end + 1;
        if start == bytes.len() {
            break;
        }
    }
    close(&mut html, container);
    Some(html)
}

fn close(html: &mut String, container: Option<Container>) {
    html.push_str(match container {
        Some(Container::Pre) => "</code></pre>",
        Some(Container::List(true)) => "</ol>",
        Some(Container::List(false)) => "</ul>",
        Some(Container::Quote) => "</blockquote>",
        None => "",
    });
}

/// A line's text, cut where its inline marks start and end, each piece in the
/// tags of its marks.
fn inline(html: &mut String, bytes: &[u8], marks: &[TextMark], line: Range<usize>) {
    let within: Vec<&TextMark> = marks
        .iter()
        .filter(|mark| !mark.block && mark.range.start < line.end && mark.range.end > line.start)
        .collect();
    let mut cuts: Vec<usize> = within
        .iter()
        .flat_map(|mark| [mark.range.start, mark.range.end])
        .map(|at| at.clamp(line.start, line.end))
        .chain([line.start, line.end])
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    for piece in cuts.windows(2) {
        let (start, end) = (piece[0], piece[1]);
        let on: BTreeMap<&str, &MarkValue> = within
            .iter()
            .filter(|mark| mark.range.start <= start && mark.range.end >= end)
            .map(|mark| (mark.key.as_str(), &mark.value))
            .collect();
        let mut text = String::from_utf8_lossy(&bytes[start..end]).into_owned();
        if text.contains(VOID) {
            let shown = match on.get(MENTION) {
                Some(MarkValue::Text(name)) => format!("@{name}"),
                _ => String::new(),
            };
            text = text.replace(VOID, &shown);
        }
        if text.is_empty() {
            continue;
        }
        let link = match on.get(LINK) {
            Some(MarkValue::Text(href)) => Some(href),
            _ => None,
        };
        if let Some(href) = link {
            html.push_str(&format!("<a href=\"{}\">", escape(href)));
        }
        let tags: Vec<&str> = INLINE_ORDER
            .iter()
            .filter(|(key, _)| on.get(key).is_some_and(|on| **on == MarkValue::Bool(true)))
            .map(|(_, tag)| *tag)
            .collect();
        for tag in &tags {
            html.push_str(&format!("<{tag}>"));
        }
        html.push_str(&escape(&text));
        for tag in tags.iter().rev() {
            html.push_str(&format!("</{tag}>"));
        }
        if link.is_some() {
            html.push_str("</a>");
        }
    }
}

const VOID_ELEMENTS: [&str; 13] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

const BLOCK_ELEMENTS: [&str; 30] = [
    "address",
    "article",
    "aside",
    "blockquote",
    "dd",
    "details",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
];

/// What an element changed, put back when it closes.
struct Frame {
    name: String,
    marks: Vec<(&'static str, Option<MarkValue>)>,
    heading: Option<Option<i64>>,
    list: bool,
    quote: bool,
    pre: bool,
    checked: Option<Option<bool>>,
}

#[derive(Default)]
struct Reader {
    bytes: Vec<u8>,
    marks: Vec<TextMark>,
    /// The inline mark of each key that ends where the text does.
    open: BTreeMap<&'static str, usize>,
    inline: BTreeMap<&'static str, MarkValue>,
    frames: Vec<Frame>,
    names: BTreeMap<String, usize>,
    heading: Option<i64>,
    lists: Vec<&'static str>,
    quote: usize,
    pre: usize,
    language: Option<String>,
    checked: Option<bool>,
    /// Where the line being read starts, once it has text.
    line: Option<usize>,
}

pub(super) fn read(html: &str) -> TextFragment {
    let mut reader = Reader::default();
    for token in tokens(html) {
        match token {
            Token::Text(text) => reader.text(&text),
            Token::Open {
                name,
                attributes,
                closed,
            } => reader.open(name, &attributes, closed),
            Token::Close { name } => reader.close(&name),
        }
    }
    reader.end_line();
    while reader.bytes.last() == Some(&b'\n') {
        reader.bytes.pop();
    }
    reader.trimmed();
    TextFragment {
        chunks: vec![TextChunk {
            bytes: reader.bytes,
            marks: reader.marks,
        }],
        shape: TextShape::Chars,
        schema: Some(NAME.to_owned()),
    }
}

impl Reader {
    fn text(&mut self, text: &str) {
        if self.pre > 0 {
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
            for (index, line) in text.split('\n').enumerate() {
                if index > 0 {
                    self.start_line();
                    self.end_line();
                }
                self.push(&line.replace(VOID, " "));
            }
            return;
        }
        let mut collapsed = String::with_capacity(text.len());
        let mut space = self
            .bytes
            .last()
            .is_none_or(|byte| matches!(byte, b' ' | b'\n' | b'\t'));
        for letter in text.chars() {
            if letter.is_ascii_whitespace() {
                if !space {
                    collapsed.push(' ');
                }
                space = true;
            } else {
                collapsed.push(match letter {
                    '\u{a0}' | VOID_CHAR => ' ',
                    letter => letter,
                });
                space = false;
            }
        }
        self.push(&collapsed);
    }

    /// Appends text with the inline marks open now, starting a line first.
    fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.start_line();
        let start = self.bytes.len();
        self.bytes.extend_from_slice(text.as_bytes());
        let end = self.bytes.len();
        for (key, value) in &self.inline {
            let held = self.open.get(key).filter(|at| {
                self.marks[**at].range.end == start && self.marks[**at].value == *value
            });
            match held {
                Some(at) => self.marks[*at].range.end = end,
                None => {
                    self.open.insert(key, self.marks.len());
                    self.marks.push(TextMark {
                        range: start..end,
                        block: false,
                        key: (*key).to_owned(),
                        value: value.clone(),
                    });
                }
            }
        }
    }

    fn start_line(&mut self) {
        if self.line.is_none() {
            self.line = Some(self.bytes.len());
            self.props();
        }
    }

    /// The props of the line just started, as block marks over it; their
    /// ends are set when it ends.
    fn props(&mut self) {
        let Some(start) = self.line else {
            return;
        };
        let mut props: Vec<(&str, MarkValue)> = Vec::new();
        if self.pre > 0 {
            let language = self.language.clone().unwrap_or_else(|| "text".to_owned());
            props.push((CODE_BLOCK, MarkValue::Text(language)));
        } else if let Some(kind) = self.lists.last() {
            let kind = match self.checked {
                Some(_) => CHECK,
                None => kind,
            };
            props.push((LIST, MarkValue::Text(kind.to_owned())));
            if self.checked == Some(true) {
                props.push((CHECKED, MarkValue::Bool(true)));
            }
            let depth = self.lists.len() as i64 - 1;
            if depth > 0 {
                props.push((INDENT, MarkValue::Int(depth.min(4))));
            }
        } else if let Some(level) = self.heading {
            props.push((HEADING, MarkValue::Int(level)));
        }
        if self.quote > 0 && self.pre == 0 {
            props.push((QUOTE, MarkValue::Bool(true)));
        }
        for (key, value) in props {
            self.marks.push(TextMark {
                range: start..start,
                block: true,
                key: key.to_owned(),
                value,
            });
        }
    }

    /// Ends the line being read, if it has text, so what follows starts one.
    fn end_line(&mut self) {
        let Some(start) = self.line.take() else {
            return;
        };
        if self.pre == 0 {
            while self.bytes.len() > start && self.bytes.last() == Some(&b' ') {
                self.bytes.pop();
            }
        }
        let end = self.bytes.len();
        for mark in self.marks.iter_mut().rev() {
            if mark.range.start < start {
                break;
            }
            mark.range.end = match mark.block {
                true => end,
                false => mark.range.end.min(end),
            };
        }
        while self
            .marks
            .last()
            .is_some_and(|mark| !mark.block && mark.range.start == mark.range.end)
        {
            self.marks.pop();
        }
        let kept = self.marks.len();
        self.open.retain(|_, at| *at < kept);
        self.bytes.push(b'\n');
    }

    /// Marks cut back to the text, after its last line breaks went.
    fn trimmed(&mut self) {
        let len = self.bytes.len();
        for mark in &mut self.marks {
            mark.range.start = mark.range.start.min(len);
            mark.range.end = mark.range.end.min(len);
        }
        self.marks.retain(|mark| match mark.block {
            true => mark.range.start < len,
            false => mark.range.start < mark.range.end,
        });
        self.open.clear();
    }

    fn open(&mut self, name: String, attributes: &[(String, String)], closed: bool) {
        let block = BLOCK_ELEMENTS.contains(&name.as_str());
        if block || matches!(name.as_str(), "tr" | "ul" | "ol") {
            self.end_line();
        }
        match name.as_str() {
            "br" => {
                self.start_line();
                self.end_line();
            }
            "td" | "th" if self.line.is_some() => self.push("\t"),
            "input" if self.lists.last().is_some() && self.line.is_none() => {
                if Token::attribute(attributes, "type")
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("checkbox"))
                {
                    self.checked = Some(Token::attribute(attributes, "checked").is_some());
                }
            }
            _ => {}
        }
        if closed || VOID_ELEMENTS.contains(&name.as_str()) {
            return;
        }
        let mut frame = Frame {
            name: name.clone(),
            marks: Vec::new(),
            heading: None,
            list: false,
            quote: false,
            pre: false,
            checked: None,
        };
        let mut set: Vec<(&'static str, Option<MarkValue>)> = Vec::new();
        let on = Some(MarkValue::Bool(true));
        match name.as_str() {
            "b" | "strong" => set.push((BOLD, on.clone())),
            "i" | "em" | "cite" | "var" => set.push((ITALIC, on.clone())),
            "u" | "ins" => set.push((UNDERLINE, on.clone())),
            "s" | "strike" | "del" => set.push((STRIKETHROUGH, on.clone())),
            "code" | "kbd" | "samp" | "tt" if self.pre == 0 => set.push((CODE, on.clone())),
            "code" => {
                if let Some(language) = Token::attribute(attributes, "class").and_then(|class| {
                    class
                        .split_whitespace()
                        .find_map(|class| class.strip_prefix("language-"))
                }) {
                    self.language = Some(language.to_ascii_lowercase());
                }
            }
            "a" => {
                if let Some(href) = Token::attribute(attributes, "href").filter(|href| safe(href)) {
                    set.push((LINK, Some(MarkValue::Text(href.trim().to_owned()))));
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                frame.heading = Some(self.heading);
                self.heading = name[1..].parse().ok();
            }
            "ul" | "ol" => {
                frame.list = true;
                self.lists.push(match name.as_str() {
                    "ol" => NUMBER,
                    _ => BULLET,
                });
            }
            "li" => {
                frame.checked = Some(self.checked);
                self.checked = None;
            }
            "blockquote" => {
                frame.quote = true;
                self.quote += 1;
            }
            "pre" => {
                frame.pre = true;
                self.pre += 1;
                self.language = None;
            }
            _ => {}
        }
        if let Some(style) = Token::attribute(attributes, "style") {
            set.extend(styled(style));
        }
        for (key, value) in set {
            let before = match value {
                Some(value) => self.inline.insert(key, value),
                None => self.inline.remove(key),
            };
            frame.marks.push((key, before));
        }
        *self.names.entry(name).or_default() += 1;
        self.frames.push(frame);
    }

    fn close(&mut self, name: &str) {
        if self.names.get(name).is_none_or(|open| *open == 0) {
            return;
        }
        while let Some(frame) = self.frames.pop() {
            let closed = frame.name == name;
            self.restore(frame);
            if closed {
                break;
            }
        }
    }

    fn restore(&mut self, frame: Frame) {
        let block = BLOCK_ELEMENTS.contains(&frame.name.as_str());
        if block || matches!(frame.name.as_str(), "tr" | "ul" | "ol") {
            self.end_line();
        }
        if let Some(open) = self.names.get_mut(&frame.name) {
            *open -= 1;
        }
        for (key, before) in frame.marks.into_iter().rev() {
            match before {
                Some(value) => self.inline.insert(key, value),
                None => self.inline.remove(key),
            };
        }
        if let Some(heading) = frame.heading {
            self.heading = heading;
        }
        if frame.list {
            self.lists.pop();
        }
        if frame.quote {
            self.quote -= 1;
        }
        if frame.pre {
            self.pre -= 1;
        }
        if let Some(checked) = frame.checked {
            self.checked = checked;
        }
    }
}

const VOID_CHAR: char = '\u{fffc}';

/// Whether a link may be followed: only the web and mail, never a script.
pub(super) fn safe(href: &str) -> bool {
    let href = href.trim().to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| href.starts_with(scheme))
}

/// The marks an inline `style` sets or clears, as browsers copy computed
/// styles: bold from `font-weight`, italic from `font-style`, underline and
/// strikethrough from `text-decoration`.
fn styled(style: &str) -> Vec<(&'static str, Option<MarkValue>)> {
    let on = |set: bool| set.then_some(MarkValue::Bool(true));
    let mut marks = Vec::new();
    for declaration in style.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        match property.as_str() {
            "font-weight" => {
                let bold = match value.parse::<u32>() {
                    Ok(weight) => weight >= 600,
                    Err(_) => matches!(value.as_str(), "bold" | "bolder"),
                };
                marks.push((BOLD, on(bold)));
            }
            "font-style" => marks.push((
                ITALIC,
                on(value.starts_with("italic") || value == "oblique"),
            )),
            "text-decoration" | "text-decoration-line" => {
                marks.push((UNDERLINE, on(value.contains("underline"))));
                marks.push((STRIKETHROUGH, on(value.contains("line-through"))));
            }
            _ => {}
        }
    }
    marks
}
