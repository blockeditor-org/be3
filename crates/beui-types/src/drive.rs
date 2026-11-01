use std::fmt::Write as _;
use std::time::Duration;

use crate::target::{TreeLine, place};
use crate::{Pos2, Rect};

pub struct Description<'a> {
    pub role: &'a str,
    pub label: Option<&'a str>,
    pub value: Option<&'a str>,
    pub toggled: Option<&'a str>,
    pub disabled: bool,
    pub selected: bool,
    pub focused: bool,
    pub test_ids: Vec<String>,
    pub bounds: Option<Rect>,
    pub clip: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line<N> {
    pub depth: usize,
    pub text: String,
    pub test_ids: Vec<String>,
    pub bounds: Option<Rect>,
    pub visible: bool,
    pub shown: Option<Rect>,
    pub node: N,
}

impl<N> TreeLine for Line<N> {
    fn depth(&self) -> usize {
        self.depth
    }
    fn text(&self) -> &str {
        &self.text
    }
    fn test_ids(&self) -> &[String] {
        &self.test_ids
    }
    fn bounds(&self) -> Option<Rect> {
        self.bounds
    }
    fn visible(&self) -> bool {
        self.visible
    }
    fn shown(&self) -> Option<Rect> {
        self.shown
    }
}

pub fn describe<N>(depth: usize, description: Description, node: N) -> Line<N> {
    let mut text = description.role.to_owned();
    if let Some(label) = description.label.filter(|label| !label.is_empty()) {
        let _ = write!(text, " {label:?}");
    }
    if let Some(value) = description.value.filter(|value| !value.is_empty()) {
        let _ = write!(text, " value={value:?}");
    }
    if let Some(toggled) = description.toggled {
        let _ = write!(text, " toggled={toggled}");
    }
    if description.disabled {
        text.push_str(" disabled");
    }
    if description.selected {
        text.push_str(" selected");
    }
    if description.focused {
        text.push_str(" focused");
    }
    for test_id in &description.test_ids {
        let _ = write!(text, " #{test_id}");
    }
    let mut visible = true;
    let mut shown = description.bounds;
    if let Some(rect) = description.bounds {
        let _ = write!(text, " {}", place(rect));
        let part = description.clip.intersect(rect);
        if area(part) <= 0.0 && area(rect) > 0.0 {
            text.push_str(" offscreen");
            visible = false;
            shown = None;
        } else if area(part) < area(rect) {
            text.push_str(" partly offscreen");
            shown = Some(part);
        }
    }
    Line {
        depth,
        text,
        test_ids: description.test_ids,
        bounds: description.bounds,
        visible,
        shown,
        node,
    }
}

fn area(rect: Rect) -> f32 {
    rect.width().max(0.0) * rect.height().max(0.0)
}

pub fn render<L: TreeLine>(lines: &[L]) -> String {
    let mut text = String::new();
    for line in lines {
        for _ in 0..line.depth() {
            text.push_str("  ");
        }
        text.push_str(line.text());
        text.push('\n');
    }
    text
}

pub fn ids<L: TreeLine>(lines: &[L]) -> String {
    let mut named: Vec<(&str, &L)> = lines
        .iter()
        .flat_map(|line| line.test_ids().iter().map(move |id| (id.as_str(), line)))
        .collect();
    named.sort_by(|a, b| a.0.cmp(b.0));
    named
        .into_iter()
        .map(|(id, line)| {
            let place = line.bounds().map(place).unwrap_or_default();
            let hidden = if line.visible() { "" } else { " offscreen" };
            format!("#{id} {place}{hidden}\n")
        })
        .collect()
}

pub fn waited<'a, L: TreeLine>(lines: &'a [L], text: &str, over: Option<Pos2>) -> Option<&'a L> {
    lines.iter().find(|line| {
        let matches = match text.strip_prefix('#') {
            Some(id) => line.test_ids().iter().any(|named| named == id),
            None => line.text().contains(text),
        };
        matches && over.is_none_or(|over| line.bounds().is_some_and(|bounds| bounds.contains(over)))
    })
}

pub enum Command {
    Tree,
    Ids,
    Find(String),
    Reveal(String),
    Shown(String),
    Wait {
        text: String,
        over: Option<String>,
        present: bool,
    },
    Settle,
    Pause(Duration),
    Clock(bool),
    Help,
}

pub fn parse(command: &str, arguments: &[String]) -> Option<Result<Command, String>> {
    let named = |what: &str| -> Result<String, String> {
        match arguments.is_empty() {
            true => Err(format!("{command} needs {what}")),
            false => Ok(arguments.join(" ")),
        }
    };
    let wait = |present: bool| -> Result<Command, String> {
        let (text, over) = match arguments {
            [text @ .., over, at] if !text.is_empty() && over == "over" => (text, Some(at.clone())),
            text => (text, None),
        };
        match text.is_empty() {
            true => Err(format!("{command} needs TEXT or #TEST_ID")),
            false => Ok(Command::Wait {
                text: text.join(" "),
                over,
                present,
            }),
        }
    };
    Some(match command {
        "tree" => Ok(Command::Tree),
        "ids" => Ok(Command::Ids),
        "find" => named("a TARGET").map(Command::Find),
        "reveal" => named("a TARGET").map(Command::Reveal),
        "shown" => named("a TARGET").map(Command::Shown),
        "wait" => wait(true),
        "gone" => wait(false),
        "settle" => Ok(Command::Settle),
        "help" => Ok(Command::Help),
        "pause" => named("MILLISECONDS").and_then(|milliseconds| {
            milliseconds
                .parse()
                .map(|milliseconds| Command::Pause(Duration::from_millis(milliseconds)))
                .map_err(|_| format!("{milliseconds} is not a number of milliseconds"))
        }),
        "clock" => match arguments.join(" ").as_str() {
            "stop" => Ok(Command::Clock(false)),
            "run" => Ok(Command::Clock(true)),
            other => Err(format!("clock takes stop or run, not {other:?}")),
        },
        _ => return None,
    })
}
