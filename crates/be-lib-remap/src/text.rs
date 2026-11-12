use serde::Deserialize;

use crate::names::{Modifier, Source};
use crate::rules::{Action, Chord, Group, Held, Rule, Step, Trigger, Wheel, WindowMatch};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextFile {
    #[serde(default)]
    group: Vec<TextGroup>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextGroup {
    name: Option<String>,
    window: Option<TextWindow>,
    #[serde(default)]
    rules: Vec<TextRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextWindow {
    exe: Option<String>,
    program_id: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextRule {
    on: String,
    #[serde(default)]
    held: Vec<String>,
    #[serde(default)]
    not_held: Vec<String>,
    #[serde(default)]
    keep: bool,
    send: Option<String>,
    tap: Option<String>,
    hold_ms: Option<u32>,
    steps: Option<Vec<TextStep>>,
    #[serde(default)]
    blind: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextStep {
    tap: Option<String>,
    hold_ms: Option<u32>,
    wait_ms: Option<u32>,
}

pub fn parse(text: &str) -> Result<Vec<Group>, String> {
    let file: TextFile = toml::from_str(text).map_err(|error| error.to_string())?;
    file.group
        .into_iter()
        .enumerate()
        .map(|(index, group)| {
            let name = group
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| format!("Group {}", index + 1));
            let window = group.window.map(|window| WindowMatch {
                exe: window.exe,
                program_id: window.program_id,
                title: window.title,
            });
            let rules = group
                .rules
                .into_iter()
                .enumerate()
                .map(|(number, rule)| {
                    rule_of(rule).map_err(|error| format!("{name}, rule {}: {error}", number + 1))
                })
                .collect::<Result<Vec<Option<Rule>>, String>>()?
                .into_iter()
                .flatten()
                .collect();
            Ok(Group {
                name,
                window,
                rules,
            })
        })
        .collect()
}

/// A rule, or none while what is pressed is not chosen yet (`on = ""`): an
/// unfinished rule reads, and does nothing.
fn rule_of(rule: TextRule) -> Result<Option<Rule>, String> {
    let mut parts: Vec<&str> = rule.on.split('+').map(str::trim).collect();
    let last = parts.pop().unwrap_or_default();
    if last.is_empty() {
        return Ok(None);
    }
    let on = match Wheel::named(last) {
        Some(wheel) => Trigger::Wheel(wheel),
        None => Trigger::Press(source(last)?),
    };
    let mut held = parts
        .into_iter()
        .map(held_of)
        .collect::<Result<Vec<_>, _>>()?;
    for name in &rule.held {
        held.push(held_of(name)?);
    }
    let not_held = rule
        .not_held
        .iter()
        .map(|name| held_of(name))
        .collect::<Result<Vec<_>, _>>()?;
    let action = match (rule.send, rule.tap, rule.steps) {
        (Some(send), None, None) => {
            if rule.hold_ms.is_some() {
                return Err("hold_ms goes with tap, not send".to_owned());
            }
            match chord(&send)? {
                Some(chord) => Action::Send(chord),
                None => Action::NOTHING,
            }
        }
        (None, Some(tap), None) => match chord(&tap)? {
            Some(chord) => Action::Steps {
                steps: vec![Step::Tap {
                    chord,
                    hold_ms: rule.hold_ms.unwrap_or(0),
                }],
                blind: rule.blind,
            },
            None => Action::NOTHING,
        },
        (None, None, Some(steps)) => {
            if rule.hold_ms.is_some() {
                return Err("hold_ms goes inside each step".to_owned());
            }
            Action::Steps {
                steps: steps
                    .into_iter()
                    .map(step)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect(),
                blind: rule.blind,
            }
        }
        (None, None, None) => return Err("it needs one of send, tap or steps".to_owned()),
        _ => return Err("it has more than one of send, tap and steps".to_owned()),
    };
    Ok(Some(Rule {
        on,
        held,
        not_held,
        keep: rule.keep,
        action,
    }))
}

/// A step, or none for a tap of nothing (`tap = ""`).
fn step(step: TextStep) -> Result<Option<Step>, String> {
    match (step.tap, step.wait_ms) {
        (Some(tap), None) => Ok(chord(&tap)?.map(|chord| Step::Tap {
            chord,
            hold_ms: step.hold_ms.unwrap_or(0),
        })),
        (None, Some(ms)) if step.hold_ms.is_none() => Ok(Some(Step::Wait { ms })),
        _ => Err("each step is either a tap (with an optional hold_ms) or a wait_ms".to_owned()),
    }
}

fn source(name: &str) -> Result<Source, String> {
    Source::named(name).ok_or_else(|| format!("no key or button is named {name:?}"))
}

fn held_of(name: &str) -> Result<Held, String> {
    match Modifier::named(name.trim()) {
        Some(modifier) => Ok(Held::Modifier(modifier)),
        None => source(name).map(Held::Source),
    }
}

/// A chord, or none for nothing (`""`), which disables what is pressed.
fn chord(text: &str) -> Result<Option<Chord>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let last = parts.pop().unwrap_or_default();
    if last.is_empty() {
        return Err(format!("{text:?} holds {} with no key", parts.join("+")));
    }
    let key = source(last)?;
    let modifiers = parts
        .into_iter()
        .map(source)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(Chord { modifiers, key }))
}
