use std::rc::Rc;

use crate::{Rect, Vec2, pos2};

pub trait TreeLine {
    fn depth(&self) -> usize;
    fn text(&self) -> &str;
    fn test_ids(&self) -> &[String];
    fn bounds(&self) -> Option<Rect>;
    fn visible(&self) -> bool;
    fn shown(&self) -> Option<Rect>;
}

impl<T: TreeLine> TreeLine for Rc<T> {
    fn depth(&self) -> usize {
        (**self).depth()
    }
    fn text(&self) -> &str {
        (**self).text()
    }
    fn test_ids(&self) -> &[String] {
        (**self).test_ids()
    }
    fn bounds(&self) -> Option<Rect> {
        (**self).bounds()
    }
    fn visible(&self) -> bool {
        (**self).visible()
    }
    fn shown(&self) -> Option<Rect> {
        (**self).shown()
    }
}

pub fn place(rect: Rect) -> String {
    format!(
        "at {},{} size {}x{}",
        rect.min.x.round(),
        rect.min.y.round(),
        rect.width().round(),
        rect.height().round()
    )
}

enum Found<'a, L> {
    Line(&'a L),
    Inside(&'a L, Vec2),
    Area(Rect),
}

fn found<'a, L: TreeLine>(target: &str, lines: &'a [L]) -> Result<Found<'a, L>, String> {
    if let Some(id) = target.strip_prefix('#') {
        if let Some((id, offset)) = id.rsplit_once('+')
            && let Some([x, y]) = numbers(offset).as_deref()
        {
            return match found(&format!("#{id}"), lines)? {
                Found::Line(line) => Ok(Found::Inside(line, Vec2::new(*x, *y))),
                _ => unreachable!("a test id finds a line"),
            };
        }
        let named: Vec<&L> = lines
            .iter()
            .filter(|line| line.test_ids().iter().any(|named| named == id))
            .collect();
        return match named.as_slice() {
            [] => Err(format!(
                "no node on screen has the test id {id}; `ids` lists them"
            )),
            [line] => Ok(Found::Line(line)),
            _ => Err(format!(
                "the test id {id} names {} nodes on screen",
                named.len()
            )),
        };
    }
    if let Some(rect) = coordinates(target) {
        return Ok(Found::Area(rect));
    }
    line_with(target, lines).map(Found::Line)
}

pub fn locate<L: TreeLine>(target: &str, lines: &[L]) -> Result<Rect, String> {
    match found(target, lines)? {
        Found::Line(line) => placed(line),
        Found::Inside(line, offset) => {
            Ok(Rect::from_min_size(placed(line)?.min + offset, Vec2::ZERO))
        }
        Found::Area(rect) => Ok(rect),
    }
}

pub fn hidden<'a, L: TreeLine>(target: &str, lines: &'a [L]) -> Option<&'a L> {
    let line = match found(target, lines).ok()? {
        Found::Line(line) | Found::Inside(line, _) => line,
        Found::Area(_) => return None,
    };
    (line.bounds().is_some() && line.shown() != line.bounds()).then_some(line)
}

pub fn shown_at<L: TreeLine>(target: &str, lines: &[L]) -> Result<Rect, String> {
    match found(target, lines)? {
        Found::Line(line) => {
            let rect = placed(line)?;
            Ok(line
                .shown()
                .filter(|shown| shown.is_positive())
                .unwrap_or(rect))
        }
        _ => locate(target, lines),
    }
}

pub fn coordinates(target: &str) -> Option<Rect> {
    match numbers(target).as_deref() {
        Some([x, y]) => Some(Rect::from_min_size(pos2(*x, *y), Vec2::ZERO)),
        Some([x, y, width, height]) => Some(Rect::from_min_size(
            pos2(*x, *y),
            Vec2::new(*width, *height),
        )),
        _ => None,
    }
}

fn numbers(text: &str) -> Option<Vec<f32>> {
    text.split(',')
        .map(|number| number.trim().parse::<f32>().ok())
        .collect()
}

fn placed<L: TreeLine>(line: &L) -> Result<Rect, String> {
    let rect = line
        .bounds()
        .ok_or_else(|| format!("{:?} has no place on screen", line.text()))?;
    if !line.visible() {
        return Err(format!("{:?} is scrolled out of view", line.text()));
    }
    Ok(rect)
}

pub fn line_with<'a, L: TreeLine>(target: &str, lines: &'a [L]) -> Result<&'a L, String> {
    let found: Vec<usize> = (0..lines.len())
        .filter(|index| lines[*index].text().contains(target))
        .collect();
    match pick(lines, &found) {
        Some(index) => Ok(&lines[index]),
        None if found.is_empty() => Err(format!(
            "no line of the tree contains {target:?}; `tree` prints it"
        )),
        None => {
            let mut message = format!("{} lines of the tree contain {target:?}:\n", found.len());
            for index in &found {
                message.push_str("  ");
                message.push_str(lines[*index].text());
                message.push('\n');
            }
            message.push_str("name one of them more closely, or use #TEST_ID or X,Y");
            Err(message)
        }
    }
}

fn pick<L: TreeLine>(lines: &[L], found: &[usize]) -> Option<usize> {
    if let [first, rest @ ..] = found
        && rest.iter().all(|index| {
            lines[*index].text() == lines[*first].text()
                && lines[*index].bounds() == lines[*first].bounds()
        })
    {
        return Some(*first);
    }
    let within = |candidates: &[usize]| {
        let first = *candidates.first()?;
        let depth = lines[first].depth();
        let end = lines[first + 1..]
            .iter()
            .position(|line| line.depth() <= depth)
            .map_or(lines.len(), |offset| first + 1 + offset);
        candidates.iter().all(|index| *index < end).then_some(first)
    };
    within(found).or_else(|| {
        let shown: Vec<usize> = found
            .iter()
            .copied()
            .filter(|index| lines[*index].visible())
            .collect();
        within(&shown)
    })
}
