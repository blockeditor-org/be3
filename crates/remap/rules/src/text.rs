use std::fmt::Write as _;

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

pub fn print(groups: &[Group]) -> String {
    let mut text = String::new();
    for (index, group) in groups.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str("[[group]]\n");
        let _ = writeln!(text, "name = {}", quoted(&group.name));
        if let Some(window) = &group.window {
            let fields: Vec<String> = [
                ("exe", &window.exe),
                ("app_id", &window.app_id),
                ("title", &window.title),
            ]
            .into_iter()
            .filter_map(|(key, value)| {
                value
                    .as_ref()
                    .map(|value| format!("{key} = {}", quoted(value)))
            })
            .collect();
            let _ = writeln!(text, "window = {{ {} }}", fields.join(", "));
        }
        text.push_str("rules = [\n");
        for rule in &group.rules {
            let _ = writeln!(text, "  {{ {} }},", rule_fields(rule).join(", "));
        }
        text.push_str("]\n");
    }
    text
}

fn rule_fields(rule: &Rule) -> Vec<String> {
    let mut on: Vec<String> = rule.held.iter().map(|held| held_name(*held)).collect();
    on.push(match rule.on {
        Trigger::Press(source) => source.name(),
        Trigger::Wheel(wheel) => wheel.name().to_owned(),
    });
    let mut fields = vec![format!("on = {}", quoted(&on.join("+")))];
    if !rule.not_held.is_empty() {
        let names: Vec<String> = rule
            .not_held
            .iter()
            .map(|held| quoted(&held_name(*held)))
            .collect();
        fields.push(format!("not_held = [{}]", names.join(", ")));
    }
    match &rule.action {
        Action::Send(chord) => fields.push(format!("send = {}", quoted(&chord_name(chord)))),
        Action::Steps { steps, blind } => {
            match steps.as_slice() {
                [Step::Tap { chord, hold_ms }] => {
                    fields.push(format!("tap = {}", quoted(&chord_name(chord))));
                    if *hold_ms > 0 {
                        fields.push(format!("hold_ms = {hold_ms}"));
                    }
                }
                steps => {
                    let steps: Vec<String> = steps
                        .iter()
                        .map(|step| match step {
                            Step::Tap { chord, hold_ms: 0 } => {
                                format!("{{ tap = {} }}", quoted(&chord_name(chord)))
                            }
                            Step::Tap { chord, hold_ms } => format!(
                                "{{ tap = {}, hold_ms = {hold_ms} }}",
                                quoted(&chord_name(chord))
                            ),
                            Step::Wait { ms } => format!("{{ wait_ms = {ms} }}"),
                        })
                        .collect();
                    fields.push(format!("steps = [{}]", steps.join(", ")));
                }
            }
            if *blind {
                fields.push("blind = true".to_owned());
            }
        }
    }
    if rule.keep {
        fields.push("keep = true".to_owned());
    }
    fields
}

fn held_name(held: Held) -> String {
    match held {
        Held::Source(source) => source.name(),
        Held::Modifier(modifier) => modifier.name().to_owned(),
    }
}

fn chord_name(chord: &Chord) -> String {
    let names: Vec<String> = chord.sources().map(Source::name).collect();
    names.join("+")
}

fn quoted(value: &str) -> String {
    let mut quoted = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(quoted, "\\u{:04X}", u32::from(c));
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}
