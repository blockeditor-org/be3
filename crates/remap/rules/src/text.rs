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
    app_id: Option<String>,
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
                app_id: window.app_id,
                title: window.title,
            });
            let rules = group
                .rules
                .into_iter()
                .enumerate()
                .map(|(number, rule)| {
                    rule_of(rule).map_err(|error| format!("{name}, rule {}: {error}", number + 1))
                })
                .collect::<Result<Vec<Rule>, String>>()?;
            Ok(Group {
                name,
                window,
                rules,
            })
        })
        .collect()
}

fn rule_of(rule: TextRule) -> Result<Rule, String> {
    let mut parts: Vec<&str> = rule.on.split('+').map(str::trim).collect();
    let last = parts.pop().unwrap_or_default();
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
            Action::Send(chord(&send)?)
        }
        (None, Some(tap), None) => Action::Steps {
            steps: vec![Step::Tap {
                chord: chord(&tap)?,
                hold_ms: rule.hold_ms.unwrap_or(0),
            }],
            blind: rule.blind,
        },
        (None, None, Some(steps)) => {
            if rule.hold_ms.is_some() {
                return Err("hold_ms goes inside each step".to_owned());
            }
            Action::Steps {
                steps: steps.into_iter().map(step).collect::<Result<_, _>>()?,
                blind: rule.blind,
            }
        }
        (None, None, None) => return Err("it needs one of send, tap or steps".to_owned()),
        _ => return Err("it has more than one of send, tap and steps".to_owned()),
    };
    Ok(Rule {
        on,
        held,
        not_held,
        keep: rule.keep,
        action,
    })
}

fn step(step: TextStep) -> Result<Step, String> {
    match (step.tap, step.wait_ms) {
        (Some(tap), None) => Ok(Step::Tap {
            chord: chord(&tap)?,
            hold_ms: step.hold_ms.unwrap_or(0),
        }),
        (None, Some(ms)) if step.hold_ms.is_none() => Ok(Step::Wait { ms }),
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

fn chord(text: &str) -> Result<Chord, String> {
    let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let key = source(parts.pop().unwrap_or_default())?;
    let modifiers = parts
        .into_iter()
        .map(source)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Chord { modifiers, key })
}
