use serde::{Deserialize, Serialize};

use crate::names::{Modifier, Source};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Group {
    pub name: String,
    pub window: Option<WindowMatch>,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct WindowMatch {
    pub exe: Option<String>,
    pub app_id: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Window {
    pub app_id: String,
    pub title: String,
    pub exe: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Rule {
    pub on: Trigger,
    pub held: Vec<Held>,
    pub not_held: Vec<Held>,
    pub keep: bool,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Trigger {
    Press(Source),
    Wheel(Wheel),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Wheel {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Held {
    Source(Source),
    Modifier(Modifier),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Action {
    Send(Chord),
    Steps { steps: Vec<Step>, blind: bool },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Step {
    Tap { chord: Chord, hold_ms: u32 },
    Wait { ms: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Chord {
    pub modifiers: Vec<Source>,
    pub key: Source,
}

impl Chord {
    pub fn sources(&self) -> impl Iterator<Item = Source> + '_ {
        self.modifiers
            .iter()
            .copied()
            .chain(std::iter::once(self.key))
    }
}

impl Wheel {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Up => "WheelUp",
            Self::Down => "WheelDown",
            Self::Left => "WheelLeft",
            Self::Right => "WheelRight",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        [Self::Up, Self::Down, Self::Left, Self::Right]
            .into_iter()
            .find(|wheel| wheel.name().eq_ignore_ascii_case(name.trim()))
    }
}

impl WindowMatch {
    pub fn matches(&self, window: &Window) -> bool {
        let field = |pattern: &Option<String>, value: &str| {
            pattern
                .as_deref()
                .is_none_or(|pattern| glob(pattern, value))
        };
        field(&self.exe, &window.exe)
            && field(&self.app_id, &window.app_id)
            && field(&self.title, &window.title)
    }
}

pub fn glob(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let value: Vec<char> = value.to_lowercase().chars().collect();
    let (mut p, mut v) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while v < value.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, v));
                p += 1;
            }
            Some(&c) if c == '?' || c == value[v] => {
                p += 1;
                v += 1;
            }
            _ => match star {
                Some((star_p, star_v)) => {
                    p = star_p + 1;
                    v = star_v + 1;
                    star = Some((star_p, star_v + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

pub fn exe_name(command: &[String]) -> String {
    command
        .first()
        .and_then(|first| first.rsplit(['/', '\\']).next())
        .unwrap_or_default()
        .to_owned()
}
