use std::rc::Rc;

use beui_core::app::accessibility_dump::Line;
use beui_core::geometry::{Pos2, Rect, pos2};
use beui_types::target::{line_with, shown_at};

pub(crate) use beui_types::target::{coordinates, locate, place};

pub(crate) fn scaled(rect: Rect, scale: f32) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x * scale, rect.min.y * scale),
        pos2(rect.max.x * scale, rect.max.y * scale),
    )
}

pub(crate) fn hidden(target: &str, lines: &[Rc<Line>]) -> Option<accesskit::NodeId> {
    beui_types::target::hidden(target, lines).map(|line| line.id)
}

pub(crate) fn node<'a>(target: &str, lines: &'a [Rc<Line>]) -> Result<&'a Line, String> {
    let named = !target.starts_with('#')
        && target
            .split(',')
            .any(|number| number.trim().parse::<f32>().is_err());
    if named {
        return line_with(target, lines).map(|line| &**line);
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

pub(crate) fn point(
    target: &str,
    lines: &[Rc<Line>],
    pixels_per_point: f32,
) -> Result<Pos2, String> {
    let center = shown_at(target, lines)?.center();
    Ok(pos2(
        center.x / pixels_per_point,
        center.y / pixels_per_point,
    ))
}
