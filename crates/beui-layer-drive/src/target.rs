use std::rc::Rc;

use beui_core::app::accessibility_dump::Line;
use beui_core::geometry::{Pos2, Rect, Vec2, pos2};

pub(crate) fn scaled(rect: Rect, scale: f32) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x * scale, rect.min.y * scale),
        pos2(rect.max.x * scale, rect.max.y * scale),
    )
}

pub(crate) fn describe(rect: Rect) -> String {
    format!(
        "at {},{} size {}x{}",
        rect.min.x.round(),
        rect.min.y.round(),
        rect.width().round(),
        rect.height().round()
    )
}

pub(crate) fn locate(target: &str, lines: &[Rc<Line>]) -> Result<Rect, String> {
    if let Some(id) = target.strip_prefix('#') {
        if let Some((id, offset)) = id.rsplit_once('+')
            && let Some([x, y]) = numbers(offset).as_deref()
        {
            let rect = locate(&format!("#{id}"), lines)?;
            return Ok(Rect::from_min_size(
                rect.min + Vec2::new(*x, *y),
                Vec2::ZERO,
            ));
        }
        let named: Vec<&Line> = lines
            .iter()
            .filter(|line| line.test_ids.iter().any(|named| named == id))
            .map(|line| &**line)
            .collect();
        let line = match named.as_slice() {
            [] => {
                return Err(format!(
                    "no node on screen has the test id {id}; `ids` lists them"
                ));
            }
            [line] => *line,
            _ => {
                return Err(format!(
                    "the test id {id} names {} nodes on screen",
                    named.len()
                ));
            }
        };
        return placed(line);
    }
    match numbers(target).as_deref() {
        Some([x, y]) => return Ok(Rect::from_min_size(pos2(*x, *y), Vec2::ZERO)),
        Some([x, y, width, height]) => {
            return Ok(Rect::from_min_size(
                pos2(*x, *y),
                Vec2::new(*width, *height),
            ));
        }
        _ => {}
    }
    let found: Vec<usize> = (0..lines.len())
        .filter(|index| lines[*index].text.contains(target))
        .collect();
    let line = match pick(lines, &found) {
        Some(index) => &*lines[index],
        None if found.is_empty() => {
            return Err(format!(
                "no line of the tree contains {target:?}; `tree` prints it"
            ));
        }
        None => {
            let mut message = format!("{} lines of the tree contain {target:?}:\n", found.len());
            for index in &found {
                message.push_str("  ");
                message.push_str(&lines[*index].text);
                message.push('\n');
            }
            message.push_str("name one of them more closely, or use #TEST_ID or X,Y");
            return Err(message);
        }
    };
    placed(line)
}

pub(crate) fn coordinates(target: &str) -> Option<Rect> {
    match numbers(target).as_deref() {
        Some([x, y]) => Some(Rect::from_min_size(pos2(*x, *y), Vec2::ZERO)),
        Some([x, y, width, height]) => Some(Rect::from_min_size(pos2(*x, *y), Vec2::new(*width, *height))),
        _ => None,
    }
}

fn numbers(text: &str) -> Option<Vec<f32>> {
    text.split(',')
        .map(|number| number.trim().parse::<f32>().ok())
        .collect()
}

fn placed(line: &Line) -> Result<Rect, String> {
    let rect = line
        .bounds
        .ok_or_else(|| format!("{:?} has no place on screen", line.text))?;
    if !line.visible {
        return Err(format!("{:?} is scrolled out of view", line.text));
    }
    Ok(rect)
}

fn line_with<'a>(target: &str, lines: &'a [Rc<Line>]) -> Result<&'a Line, String> {
    let found: Vec<usize> = (0..lines.len())
        .filter(|index| lines[*index].text.contains(target))
        .collect();
    match pick(lines, &found) {
        Some(index) => Ok(&*lines[index]),
        None if found.is_empty() => Err(format!(
            "no line of the tree contains {target:?}; `tree` prints it"
        )),
        None => {
            let mut message = format!("{} lines of the tree contain {target:?}:\n", found.len());
            for index in &found {
                message.push_str("  ");
                message.push_str(&lines[*index].text);
                message.push('\n');
            }
            message.push_str("name one of them more closely, or use #TEST_ID or X,Y");
            Err(message)
        }
    }
}

pub(crate) fn node<'a>(target: &str, lines: &'a [Rc<Line>]) -> Result<&'a Line, String> {
    let named = !target.starts_with('#')
        && target
            .split(',')
            .any(|number| number.trim().parse::<f32>().is_err());
    if named {
        return line_with(target, lines);
    }
    let center = locate(target, lines)?.center();
    let under: Vec<&Line> = lines
        .iter()
        .filter(|line| line.visible && line.bounds.is_some_and(|bounds| bounds.contains(center)))
        .map(|line| &**line)
        .collect();
    under
        .iter()
        .rev()
        .find(|line| !line.actions.is_empty())
        .or_else(|| under.last())
        .copied()
        .ok_or_else(|| format!("no node of the tree is at {target}"))
}

fn pick(lines: &[Rc<Line>], found: &[usize]) -> Option<usize> {
    if let [first, rest @ ..] = found
        && rest.iter().all(|index| {
            lines[*index].text == lines[*first].text && lines[*index].bounds == lines[*first].bounds
        })
    {
        return Some(*first);
    }
    let within = |candidates: &[usize]| {
        let first = *candidates.first()?;
        let depth = lines[first].depth;
        let end = lines[first + 1..]
            .iter()
            .position(|line| line.depth <= depth)
            .map_or(lines.len(), |offset| first + 1 + offset);
        candidates.iter().all(|index| *index < end).then_some(first)
    };
    within(found).or_else(|| {
        let shown: Vec<usize> = found
            .iter()
            .copied()
            .filter(|index| lines[*index].visible)
            .collect();
        within(&shown)
    })
}

pub(crate) fn point(
    target: &str,
    lines: &[Rc<Line>],
    pixels_per_point: f32,
) -> Result<Pos2, String> {
    let center = locate(target, lines)?.center();
    Ok(pos2(
        center.x / pixels_per_point,
        center.y / pixels_per_point,
    ))
}
