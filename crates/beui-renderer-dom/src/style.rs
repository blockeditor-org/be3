use std::fmt::Write as _;

use beui_core::color::Color32;
use beui_core::fade::Fade;
use beui_core::font::Galley;
use beui_core::geometry::{Pos2, Rect, Rotation, vec2};
use beui_core::painter::{Entry, Shape};
use beui_font_browser::css_font;

const REACH: f64 = 1_000_000.0;

#[derive(Debug, PartialEq)]
pub enum Look {
    Box(String),
    Text { css: String, lines: Vec<Line> },
}

#[derive(Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub css: String,
}

pub fn look(shape: &Shape, factor: f32, top: bool, image: Option<&str>) -> Option<Look> {
    let mut look = match shape {
        Shape::Rect {
            rect,
            corner_radius,
            stroke_width,
            color,
            rotation,
            clip,
        } => {
            let rect = snapped(*rect, factor);
            let mut css = placed(rect, *clip, *rotation)?;
            match corner_radius.to_array() {
                [top_left, top_right, bottom_right, bottom_left]
                    if top_left == top_right
                        && top_left == bottom_right
                        && top_left == bottom_left =>
                {
                    let _ = write!(css, "border-radius:{top_left}px;");
                }
                [top_left, top_right, bottom_right, bottom_left] => {
                    let _ = write!(
                        css,
                        "border-radius:{top_left}px {top_right}px {bottom_right}px {bottom_left}px;"
                    );
                }
            }
            match *stroke_width > 0.0 {
                true => {
                    let width = (stroke_width * factor).round().max(1.0) / factor;
                    let _ = write!(css, "border:{width}px solid {};", rgba(*color));
                }
                false => {
                    let _ = write!(css, "background:{};", rgba(*color));
                }
            }
            Look::Box(css)
        }
        Shape::Text {
            origin,
            galley,
            color,
            rotation,
            clip,
        } => text(*origin, galley, *color, *rotation, *clip, factor)?,
        Shape::Image {
            rect,
            source,
            tint,
            corner_radius,
            smooth,
            rotation,
            clip,
            ..
        } => {
            let url = image?;
            let rect = snapped(*rect, factor);
            let mut css = placed(rect, *clip, *rotation)?;
            let span = vec2(
                source.width().max(f32::EPSILON),
                source.height().max(f32::EPSILON),
            );
            let size = vec2(rect.width() / span.x, rect.height() / span.y);
            let _ = write!(
                css,
                "background-image:url(\"{url}\");background-repeat:no-repeat;\
                 background-size:{}px {}px;background-position:{}px {}px;\
                 border-radius:{corner_radius}px;opacity:{};",
                size.x,
                size.y,
                -source.min.x * size.x,
                -source.min.y * size.y,
                f32::from(tint.to_array()[3]) / 255.0,
            );
            if !smooth {
                css.push_str("image-rendering:pixelated;");
            }
            Look::Box(css)
        }
        Shape::Line {
            from,
            to,
            width,
            color,
            clip,
        } => {
            let width = (width * factor).max(1.0) / factor;
            let bounds = Rect::from_points(&[*from, *to]).expand(width);
            if !bounds.intersects(*clip) {
                return None;
            }
            let span = *to - *from;
            let angle = span.y.atan2(span.x);
            Look::Box(format!(
                "left:{}px;top:{}px;width:{}px;height:{width}px;border-radius:{}px;\
                 transform-origin:{}px 50%;transform:rotate({angle}rad);background:{};",
                from.x - width / 2.0,
                from.y - width / 2.0,
                span.length() + width,
                width / 2.0,
                width / 2.0,
                rgba(*color),
            ))
        }
        Shape::Punch { .. } | Shape::Drawing { .. } => return None,
    };
    if top {
        match &mut look {
            Look::Box(css) | Look::Text { css, .. } => css.push_str("z-index:1;"),
        }
    }
    Some(look)
}

fn text(
    origin: Pos2,
    galley: &Galley,
    color: Color32,
    rotation: Rotation,
    clip: Rect,
    factor: f32,
) -> Option<Look> {
    let origin = Pos2::new(snap(origin.x, factor), snap(origin.y, factor));
    let rect = Rect::from_min_size(origin, galley.size());
    let mut css = placed(rect, clip, rotation)?;
    let _ = write!(
        css,
        "font:{};color:{};",
        css_font(galley.font()),
        rgba(color)
    );
    let height = galley.line_height();
    let lines = galley
        .lines()
        .iter()
        .filter_map(|line| {
            let text = galley.text().get(line.range.clone())?;
            if text.trim().is_empty() {
                return None;
            }
            let left = line.cursors.first().map_or(0.0, |(_, x)| *x);
            Some(Line {
                text: text.trim_end_matches(['\n', '\r']).replace('\t', " "),
                css: format!(
                    "left:{left}px;top:{}px;height:{height}px;line-height:{height}px;",
                    line.top
                ),
            })
        })
        .collect();
    Some(Look::Text { css, lines })
}

fn placed(rect: Rect, clip: Rect, rotation: Rotation) -> Option<String> {
    let turned = rotation.turns();
    let bounds = match turned {
        true => rotation.bounds(rect),
        false => rect,
    };
    if !bounds.intersects(clip) {
        return None;
    }
    let mut css = format!(
        "left:{}px;top:{}px;width:{}px;height:{}px;",
        rect.min.x,
        rect.min.y,
        rect.width(),
        rect.height()
    );
    if turned {
        let _ = write!(
            css,
            "transform-origin:{}px {}px;transform:rotate({}rad);",
            rotation.pivot.x - rect.min.x,
            rotation.pivot.y - rect.min.y,
            rotation.angle
        );
    } else if !clip.contains_rect(rect) {
        let _ = write!(
            css,
            "clip-path:inset({}px {}px {}px {}px);",
            (clip.min.y - rect.min.y).max(0.0),
            (rect.max.x - clip.max.x).max(0.0),
            (rect.max.y - clip.max.y).max(0.0),
            (clip.min.x - rect.min.x).max(0.0),
        );
    }
    Some(css)
}

pub fn placement(entry: Entry, factor: f32) -> (String, String) {
    let x = span(entry.clip.min.x, entry.clip.max.x, factor);
    let y = span(entry.clip.min.y, entry.clip.max.y, factor);
    let content = format!(
        "left:{}px;top:{}px;",
        f64::from(snap(entry.translation.x, factor)) - x.map_or(0.0, |(min, _)| min),
        f64::from(snap(entry.translation.y, factor)) - y.map_or(0.0, |(min, _)| min),
    );
    let mut frame = clipped(x, y);
    if let (Some(x), Some(y)) = (x, y) {
        frame.push_str(&masked(entry.fade, (x.0, y.0)));
    }
    (frame, content)
}

fn masked(fade: Fade, origin: (f64, f64)) -> String {
    if fade.is_none() {
        return String::new();
    }
    let [left, top, right, bottom] = fade.widths.map(f64::from);
    let rect = fade.rect;
    let gradients: Vec<String> = [
        ("right", rect.min.x, rect.max.x, left, right, origin.0),
        ("bottom", rect.min.y, rect.max.y, top, bottom, origin.1),
    ]
    .into_iter()
    .filter(|(.., near, far, _)| *near > 0.0 || *far > 0.0)
    .map(|(towards, start, end, near, far, origin)| {
        let start = f64::from(start).clamp(-REACH, REACH) - origin;
        let end = f64::from(end).clamp(-REACH, REACH) - origin;
        let mut stops = Vec::new();
        match near > 0.0 {
            true => {
                stops.push(format!("transparent {start}px"));
                stops.push(format!("#000 {}px", start + near));
            }
            false => stops.push("#000 0px".to_owned()),
        }
        match far > 0.0 {
            true => {
                stops.push(format!("#000 {}px", end - far));
                stops.push(format!("transparent {end}px"));
            }
            false => stops.push("#000 100%".to_owned()),
        }
        format!("linear-gradient(to {towards},{})", stops.join(","))
    })
    .collect();
    let images = gradients.join(",");
    let mut css = format!("mask-image:{images};-webkit-mask-image:{images};");
    if gradients.len() > 1 {
        css.push_str("mask-composite:intersect;-webkit-mask-composite:source-in;");
    }
    css
}

pub fn layer(clip: Rect, scale: f32, factor: f32) -> (String, String) {
    let x = span(clip.min.x, clip.max.x, factor);
    let y = span(clip.min.y, clip.max.y, factor);
    let mut scaled = format!(
        "left:{}px;top:{}px;",
        -x.map_or(0.0, |(min, _)| min),
        -y.map_or(0.0, |(min, _)| min),
    );
    if scale != 1.0 {
        let _ = write!(scaled, "transform:scale({scale});");
    }
    (clipped(x, y), scaled)
}

fn clipped(x: Option<(f64, f64)>, y: Option<(f64, f64)>) -> String {
    let mut css = String::new();
    for (axis, start, extent, overflow) in [
        (x, "left", "width", "overflow-x"),
        (y, "top", "height", "overflow-y"),
    ] {
        if let Some((min, max)) = axis {
            let _ = write!(
                css,
                "{start}:{min}px;{extent}:{}px;{overflow}:clip;",
                max - min
            );
        }
    }
    css
}

fn span(min: f32, max: f32, factor: f32) -> Option<(f64, f64)> {
    if !min.is_finite() && !max.is_finite() {
        return None;
    }
    let bound = |value: f32| {
        let value = f64::from(value).clamp(-REACH, REACH);
        let factor = f64::from(factor);
        (value * factor).round() / factor
    };
    let min = bound(min);
    Some((min, bound(max).max(min)))
}

fn snap(value: f32, factor: f32) -> f32 {
    (value * factor).round() / factor
}

fn snapped(rect: Rect, factor: f32) -> Rect {
    let min = Pos2::new(snap(rect.min.x, factor), snap(rect.min.y, factor));
    let max = Pos2::new(
        snap(rect.max.x, factor).max(min.x + 1.0 / factor),
        snap(rect.max.y, factor).max(min.y + 1.0 / factor),
    );
    Rect::from_min_max(min, max)
}

pub fn rgba(color: Color32) -> String {
    let [red, green, blue, alpha] = color.to_array();
    format!("rgba({red},{green},{blue},{})", f32::from(alpha) / 255.0)
}
