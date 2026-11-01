use beui_types::{BackEdge, Key, PointerButton, TouchPhase};
use beui_types::{Pos2, vec2};

use crate::USAGE;
use crate::gestures::{Frames, Gestures};

pub fn plan(
    gestures: &mut Gestures,
    command: &str,
    arguments: &[String],
    pixels_per_point: f32,
    at: &dyn Fn(&str) -> Result<Pos2, String>,
) -> Result<Option<Frames>, String> {
    let argument = |index: usize| -> Result<&str, String> {
        arguments
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| format!("{command} needs more arguments\n{USAGE}"))
    };
    let pixels = |text: &str, what: &str| -> Result<f32, String> {
        text.parse::<f32>()
            .map(|value| value / pixels_per_point)
            .map_err(|_| format!("{text} is not a {what}"))
    };
    Ok(Some(match command {
        "click" => {
            let at = at(argument(0)?)?;
            let (button, count) = match arguments.get(1).map(String::as_str) {
                None => (PointerButton::Primary, 1),
                Some("right") => (PointerButton::Secondary, 1),
                Some("middle") => (PointerButton::Middle, 1),
                Some("double") => (PointerButton::Primary, 2),
                Some(other) => {
                    return Err(format!("click takes right, middle or double, not {other}"));
                }
            };
            gestures.click(at, button, count)
        }
        "move" => gestures.hover(at(argument(0)?)?),
        "down" | "up" => {
            let (target, button) = match (arguments.first(), arguments.get(1)) {
                (Some(first), _) if button_named(first).is_some() => (None, button_named(first)),
                (target, button) => (target, button.and_then(|name| button_named(name))),
            };
            let button = button.unwrap_or(PointerButton::Primary);
            let target = target.map(|target| at(target)).transpose()?;
            match (command == "down", target) {
                (true, Some(at)) => gestures.down(at, button),
                (true, None) => return Err("down needs a TARGET".to_owned()),
                (false, at) => gestures.up(at, button),
            }
        }
        "drag" => {
            let from = at(argument(0)?)?;
            let to = at(argument(1)?)?;
            let (mut steps, mut button) = (2, PointerButton::Primary);
            for extra in arguments.iter().skip(2) {
                match (button_named(extra), extra.parse::<usize>()) {
                    (Some(named), _) => button = named,
                    (None, Ok(count)) => steps = count,
                    (None, Err(_)) => {
                        return Err(format!(
                            "drag takes a number of steps and a button, not {extra}"
                        ));
                    }
                }
            }
            gestures.drag(from, to, steps, button)
        }
        "wheel" => {
            let at = at(argument(0)?)?;
            let ticks = |index: usize| -> Result<i32, String> {
                arguments.get(index).map_or(Ok(0), |ticks| {
                    ticks
                        .parse()
                        .map_err(|_| format!("{ticks} is not a number of ticks"))
                })
            };
            gestures.wheel(at, ticks(1)?, ticks(2)?)
        }
        "zoom" => {
            let at = at(argument(0)?)?;
            gestures.zoom(at, factor(argument(1)?)?)
        }
        "pinch" => {
            let center = at(argument(0)?)?;
            gestures.pinch(center, factor(argument(1)?)?)
        }
        "scroll" => {
            let at = at(argument(0)?)?;
            let number = |index: usize| match arguments.get(index) {
                None => Ok(0.0),
                Some(text) => pixels(text, "number"),
            };
            gestures.scroll(at, vec2(number(2)?, number(1)?))
        }
        "leave" => gestures.leave(),
        "motion" => {
            let by = vec2(
                pixels(argument(0)?, "number of pixels")?,
                pixels(argument(1)?, "number of pixels")?,
            );
            gestures.motion(by)
        }
        "tap" => {
            let at = at(argument(0)?)?;
            gestures.tap(at, finger(arguments.get(1))?)
        }
        "swipe" => {
            let from = at(argument(0)?)?;
            let to = at(argument(1)?)?;
            gestures.swipe(from, to, finger(arguments.get(2))?)
        }
        "pen" => {
            let pressure = |index: usize| {
                arguments
                    .get(index)
                    .map(|text| {
                        text.parse::<f32>()
                            .ok()
                            .filter(|pressure| (0.0..=1.0).contains(pressure))
                            .ok_or_else(|| format!("{text} is not a pressure from 0 to 1"))
                    })
                    .transpose()
            };
            let count = |index: usize, default: usize, what: &str| match arguments.get(index) {
                Some(text) => text
                    .parse::<usize>()
                    .ok()
                    .filter(|count| *count > 0)
                    .ok_or_else(|| format!("{text} is not a number of {what} above 0")),
                None => Ok(default),
            };
            match argument(0)? {
                "move" => gestures.pen_move(
                    at(argument(1)?)?,
                    pressure(2)?,
                    count(3, 1, "steps")?,
                    count(4, 1, "samples a frame")?,
                ),
                "down" => gestures.pen_down(at(argument(1)?)?, pressure(2)?)?,
                "up" => {
                    let to = arguments.get(1).map(|target| at(target)).transpose()?;
                    gestures.pen_up(to)?
                }
                "draw" => {
                    let from = at(argument(1)?)?;
                    let to = at(argument(2)?)?;
                    let steps = count(3, 8, "steps")?;
                    let per_frame = count(4, 1, "samples a frame")?;
                    gestures.pen_draw(from, to, steps, per_frame)?
                }
                "leave" => gestures.pen_leave()?,
                other => {
                    return Err(format!(
                        "pen takes move, down, up, draw or leave, not {other}"
                    ));
                }
            }
        }
        "touch" => {
            let phase = match argument(0)? {
                "down" => TouchPhase::Start,
                "move" => TouchPhase::Move,
                "up" => TouchPhase::End,
                "cancel" => TouchPhase::Cancel,
                other => {
                    return Err(format!("touch takes down, move, up or cancel, not {other}"));
                }
            };
            let fingers = arguments[1..]
                .iter()
                .map(|spec| {
                    let (finger, target) = finger_target(spec);
                    Ok((finger, target.map(at).transpose()?))
                })
                .collect::<Result<Vec<_>, String>>()?;
            gestures.touch(phase, &fingers)?
        }
        "back" => match arguments.first().map(String::as_str) {
            None => gestures.back(),
            Some("left") => gestures.back_from(BackEdge::Left)?,
            Some("right") => gestures.back_from(BackEdge::Right)?,
            Some("commit") => gestures.back_end(true)?,
            Some("cancel") => gestures.back_end(false)?,
            Some(progress) => {
                let progress: f32 = progress
                    .parse()
                    .ok()
                    .filter(|progress: &f32| (0.0..=1.0).contains(progress))
                    .ok_or("back takes left, right, a progress from 0 to 1, commit or cancel")?;
                gestures.back_to(progress)
            }
        },
        "type" => gestures.type_text(&arguments.join(" ")),
        "key" => {
            if arguments.is_empty() {
                return Err(format!("key needs a chord, such as ctrl+z\n{USAGE}"));
            }
            gestures.chords(arguments)?
        }
        "keydown" | "keyup" => {
            if arguments.is_empty() {
                return Err(format!("{command} needs a KEY"));
            }
            let mut events = Vec::new();
            for name in arguments {
                let key = Key::from_name(name)
                    .ok_or_else(|| format!("{name:?} is not a key beui knows"))?;
                events.extend(
                    gestures
                        .key(key, command == "keydown")?
                        .into_iter()
                        .flatten(),
                );
            }
            vec![events]
        }
        "ime" => match argument(0)? {
            "compose" => gestures.compose(arguments[1..].join(" ")),
            "commit" => gestures.commit((arguments.len() > 1).then(|| arguments[1..].join(" "))),
            "cancel" => gestures.cancel_composing(),
            other => {
                return Err(format!("ime takes compose, commit or cancel, not {other}"));
            }
        },
        "focus" | "blur" => gestures.focus(command == "focus"),
        _ => return Ok(None),
    }))
}

fn button_named(name: &str) -> Option<PointerButton> {
    match name {
        "left" => Some(PointerButton::Primary),
        "right" => Some(PointerButton::Secondary),
        "middle" => Some(PointerButton::Middle),
        "back" => Some(PointerButton::Back),
        "forward" => Some(PointerButton::Forward),
        _ => {
            let code = name.strip_prefix("button:")?;
            let code = match code.strip_prefix("0x") {
                Some(hex) => u16::from_str_radix(hex, 16).ok()?,
                None => code.parse().ok()?,
            };
            Some(PointerButton::Other(code))
        }
    }
}

fn factor(text: &str) -> Result<f32, String> {
    text.parse::<f32>()
        .ok()
        .filter(|factor| *factor > 0.0)
        .ok_or_else(|| format!("{text} is not a factor above 0"))
}

fn finger_target(spec: &str) -> (u64, Option<&str>) {
    if let Some((finger, target)) = spec.split_once('=')
        && let Ok(finger) = finger.parse()
    {
        return (finger, Some(target));
    }
    match spec.parse() {
        Ok(finger) => (finger, None),
        Err(_) => (0, Some(spec)),
    }
}

fn finger(argument: Option<&String>) -> Result<u64, String> {
    argument.map_or(Ok(0), |finger| {
        finger
            .parse()
            .map_err(|_| format!("{finger} is not a finger number"))
    })
}
