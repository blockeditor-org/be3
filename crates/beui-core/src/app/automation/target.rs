use std::collections::HashMap;

use crate::app::accessibility_dump::Line;
use crate::geometry::{Pos2, Rect, Vec2, pos2};

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

pub(crate) fn locate(
    target: &str,
    lines: &[Line],
    test_ids: &HashMap<String, Rect>,
    pixels_per_point: f32,
) -> Result<Rect, String> {
    if let Some(id) = target.strip_prefix('#') {
        return test_ids
            .get(id)
            .map(|rect| scaled(*rect, pixels_per_point))
            .ok_or_else(|| format!("no node on screen has the test id {id}; `ids` lists them"));
    }
    let numbers: Option<Vec<f32>> = target
        .split(',')
        .map(|number| number.trim().parse::<f32>().ok())
        .collect();
    match numbers.as_deref() {
        Some([x, y]) => return Ok(Rect::from_min_size(pos2(*x, *y), Vec2::ZERO)),
        Some([x, y, width, height]) => {
            return Ok(Rect::from_min_size(pos2(*x, *y), Vec2::new(*width, *height)));
        }
        _ => {}
    }
    let found: Vec<usize> = (0..lines.len())
        .filter(|index| lines[*index].text.contains(target))
        .collect();
    let line = match pick(lines, &found) {
        Some(index) => &lines[index],
        None if found.is_empty() => {
            return Err(format!("no line of the tree contains {target:?}; `tree` prints it"));
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
    let rect = line
        .bounds
        .ok_or_else(|| format!("{:?} has no place on screen", line.text))?;
    if !line.visible {
        return Err(format!("{:?} is scrolled out of view", line.text));
    }
    Ok(rect)
}

fn pick(lines: &[Line], found: &[usize]) -> Option<usize> {
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
        candidates
            .iter()
            .all(|index| *index < end)
            .then_some(first)
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
    lines: &[Line],
    test_ids: &HashMap<String, Rect>,
    pixels_per_point: f32,
) -> Result<Pos2, String> {
    let center = locate(target, lines, test_ids, pixels_per_point)?.center();
    Ok(pos2(center.x / pixels_per_point, center.y / pixels_per_point))
}
