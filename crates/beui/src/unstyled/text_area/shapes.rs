use std::ops::Range;

use text_editor_core::{CollapsibleSection, SynHlColorScope};

use crate::color::Color32;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2};
use crate::icons::{ICON_CHECK, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT};
use crate::page::{Page, PageShape};
use crate::reactive::layout_text;

use super::RemoteTextCursor;
use super::colors::TextAreaColors;
use super::layout::{BytePosition, DocumentLayout, INLINE_WIDGET_ICON_INSET, LineLayout, Run};
use super::state::{MarkdownCheckbox, Snapshot};

pub(crate) const PADDING: Vec2 = Vec2::new(12.0, 8.0);
pub(crate) const GUTTER_TEXT_SIZE: f32 = 12.0;
pub(crate) const GUTTER_PADDING_LEFT: f32 = 10.0;
pub(crate) const GUTTER_PADDING_RIGHT: f32 = 10.0;
pub(crate) const GUTTER_ARROW_SIZE: f32 = 14.0;
pub(crate) const TOUCH_HANDLE_RADIUS: f32 = 9.0;
pub(crate) const TOUCH_HANDLE_GAP: f32 = 4.0;
pub(crate) const TOUCH_HANDLE_HIT_RADIUS: f32 = 24.0;

const COLLAPSED_ELLIPSIS_GAP: f32 = 6.0;
pub(crate) const CARET_WIDTH: f32 = 2.0;
const EMPTY_BYTE_WIDTH: f32 = 8.0;
const CHECKBOX_RADIUS: f32 = 3.0;
const CHECKBOX_OUTLINE: f32 = 1.5;
const INLINE_WIDGET_RADIUS: f32 = 5.0;
const REMOTE_SELECTION_ALPHA: u8 = 70;
const REMOTE_FLAG: Vec2 = Vec2::new(6.0, 4.0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectionHandle {
    Start,
    End,
    Caret,
}

pub(crate) fn digit_width() -> f32 {
    layout_text(
        "0",
        FontId::monospace(GUTTER_TEXT_SIZE),
        TextLayout::DEFAULT,
    )
    .map_or(GUTTER_TEXT_SIZE * 0.6, |galley| galley.size().x)
}

fn gutter_digits(line_count: usize) -> usize {
    let mut digits = 1;
    let mut value = line_count.max(1);
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

pub(crate) fn gutter_width(line_count: usize) -> f32 {
    GUTTER_PADDING_LEFT
        + GUTTER_ARROW_SIZE
        + digit_width() * gutter_digits(line_count) as f32
        + GUTTER_PADDING_RIGHT
}

pub(crate) fn origin(gutter_width: f32, padding: Vec2) -> Vec2 {
    Vec2::new(gutter_width + padding.x, padding.y)
}

fn text(origin: Pos2, string: &str, font: FontId, color: Color32) -> Option<PageShape> {
    let galley = layout_text(string, font, TextLayout::DEFAULT)?;
    Some(PageShape::Text {
        origin,
        galley,
        color,
    })
}

fn centered(rect: Rect, string: &str, font: FontId, color: Color32) -> Option<PageShape> {
    let galley = layout_text(string, font, TextLayout::DEFAULT)?;
    let size = galley.size();
    Some(PageShape::Text {
        origin: Pos2::new(
            rect.center().x - size.x / 2.0,
            rect.center().y - size.y / 2.0,
        ),
        galley,
        color,
    })
}

fn line_section<'a>(
    line: &LineLayout,
    sections: &'a [CollapsibleSection],
) -> Option<&'a CollapsibleSection> {
    sections
        .iter()
        .find(|section| section.line_start == line.start)
}

pub(crate) fn gutter_arrow_rect(origin: Vec2, line: &LineLayout) -> Rect {
    Rect::from_min_size(
        Pos2::new(
            GUTTER_PADDING_LEFT,
            origin.y + line.y + (line.height - GUTTER_ARROW_SIZE) / 2.0,
        ),
        Vec2::splat(GUTTER_ARROW_SIZE),
    )
}

pub(crate) fn gutter_arrow_at(
    layout: &DocumentLayout,
    sections: &[CollapsibleSection],
    gutter_width: f32,
    origin: Vec2,
    point: Pos2,
) -> Option<usize> {
    if point.x < 0.0 || point.x > gutter_width {
        return None;
    }
    layout.lines.iter().find_map(|line| {
        let section = line_section(line, sections)?;
        gutter_arrow_rect(origin, line)
            .contains(point)
            .then_some(section.line_start)
    })
}

fn byte_span(layout: &DocumentLayout, byte: usize) -> Option<(usize, f32, f32)> {
    let left = layout.positions.get(byte).copied().flatten()?;
    let right = layout
        .positions
        .get(byte + 1)
        .copied()
        .flatten()
        .filter(|right| right.line == left.line)
        .unwrap_or(BytePosition {
            line: left.line,
            x: left.x + EMPTY_BYTE_WIDTH,
        });
    Some((left.line, left.x.min(right.x), left.x.max(right.x)))
}

fn selection_rects(layout: &DocumentLayout, range: &Range<usize>, origin: Vec2) -> Vec<Rect> {
    let mut rects = Vec::new();
    for byte in range.clone() {
        if layout
            .widgets
            .iter()
            .any(|widget| !widget.block && widget.range.contains(&byte))
        {
            continue;
        }
        let Some((line, left, right)) = byte_span(layout, byte) else {
            continue;
        };
        let Some(line) = layout.lines.get(line) else {
            continue;
        };
        rects.push(Rect::from_min_max(
            Pos2::new(origin.x + left, origin.y + line.y),
            Pos2::new(
                origin.x + right.max(left + 1.0),
                origin.y + line.y + line.height,
            ),
        ));
    }
    rects
}

pub(crate) fn caret_rect(layout: &DocumentLayout, byte: usize, origin: Vec2) -> Option<Rect> {
    let position = layout.positions.get(byte).copied().flatten()?;
    let line = layout.lines.get(position.line)?;
    Some(Rect::from_min_size(
        Pos2::new(origin.x + position.x, origin.y + line.y),
        Vec2::new(CARET_WIDTH, line.height),
    ))
}

pub(crate) fn checkbox_marker_rect(
    layout: &DocumentLayout,
    checkbox: &MarkdownCheckbox,
) -> Option<Rect> {
    let left = layout
        .positions
        .get(checkbox.marker.start)
        .copied()
        .flatten()?;
    let right = layout
        .positions
        .get(checkbox.marker.end)
        .copied()
        .flatten()?;
    (left.line == right.line).then(|| {
        let line = &layout.lines[left.line];
        Rect::from_min_max(
            Pos2::new(left.x, line.y),
            Pos2::new(right.x, line.y + line.height),
        )
    })
}

fn checkbox_rect(layout: &DocumentLayout, checkbox: &MarkdownCheckbox) -> Option<Rect> {
    let marker = checkbox_marker_rect(layout, checkbox)?;
    let size = marker.width().min(marker.height());
    let center = marker.center();
    Some(Rect::from_min_size(
        Pos2::new(center.x - size / 2.0, center.y - size / 2.0),
        Vec2::splat(size),
    ))
}

pub(crate) fn checkbox_at<'a>(
    layout: &DocumentLayout,
    checkboxes: &'a [MarkdownCheckbox],
    point: Pos2,
) -> Option<&'a MarkdownCheckbox> {
    checkboxes.iter().find(|checkbox| {
        checkbox_marker_rect(layout, checkbox).is_some_and(|rect| rect.contains(point))
    })
}

pub(crate) fn touch_handle_anchor(layout: &DocumentLayout, byte: usize) -> Option<Vec2> {
    let position = layout.positions.get(byte).copied().flatten()?;
    let line = layout.lines.get(position.line)?;
    Some(Vec2::new(
        position.x,
        line.y + line.height + TOUCH_HANDLE_GAP,
    ))
}

pub(crate) fn hit_test_anchor(layout: &DocumentLayout, byte: usize) -> Option<Vec2> {
    let position = layout.positions.get(byte).copied().flatten()?;
    let line = layout.lines.get(position.line)?;
    Some(Vec2::new(position.x, line.y))
}

pub(crate) fn touch_handle_center(anchor: Vec2, handle: SelectionHandle) -> Vec2 {
    match handle {
        SelectionHandle::Start => anchor + Vec2::new(-TOUCH_HANDLE_RADIUS, TOUCH_HANDLE_RADIUS),
        SelectionHandle::End => anchor + Vec2::new(TOUCH_HANDLE_RADIUS, TOUCH_HANDLE_RADIUS),
        SelectionHandle::Caret => anchor + Vec2::new(0.0, TOUCH_HANDLE_RADIUS),
    }
}

fn touch_handle_shapes(anchor: Vec2, handle: SelectionHandle, color: Color32) -> [PageShape; 2] {
    let center = touch_handle_center(anchor, handle);
    let round = Rect::from_min_size(
        Pos2::new(
            center.x - TOUCH_HANDLE_RADIUS,
            center.y - TOUCH_HANDLE_RADIUS,
        ),
        Vec2::splat(TOUCH_HANDLE_RADIUS * 2.0),
    );
    let corner = match handle {
        SelectionHandle::Start => Rect::from_min_size(
            Pos2::new(anchor.x - TOUCH_HANDLE_RADIUS, anchor.y),
            Vec2::splat(TOUCH_HANDLE_RADIUS),
        ),
        SelectionHandle::End => Rect::from_min_size(
            Pos2::new(anchor.x, anchor.y),
            Vec2::splat(TOUCH_HANDLE_RADIUS),
        ),
        SelectionHandle::Caret => Rect::from_min_size(
            Pos2::new(anchor.x - TOUCH_HANDLE_RADIUS / 2.0, anchor.y),
            Vec2::splat(TOUCH_HANDLE_RADIUS),
        ),
    };
    [
        PageShape::Rect {
            rect: round,
            corner_radius: TOUCH_HANDLE_RADIUS,
            color,
        },
        PageShape::Rect {
            rect: corner,
            corner_radius: 0.0,
            color,
        },
    ]
}

pub(crate) fn background(
    layout: &DocumentLayout,
    snapshot: &Snapshot,
    colors: &TextAreaColors,
    size: Vec2,
    gutter_width: f32,
    origin: Vec2,
) -> Page {
    let mut shapes = vec![PageShape::Rect {
        rect: Rect::from_min_size(Pos2::ZERO, size),
        corner_radius: 0.0,
        color: colors.surface,
    }];
    if gutter_width > 0.0 {
        shapes.push(PageShape::Rect {
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(gutter_width, size.y)),
            corner_radius: 0.0,
            color: colors.gutter,
        });
        shapes.push(PageShape::Rect {
            rect: Rect::from_min_size(Pos2::new(gutter_width - 1.0, 0.0), Vec2::new(1.0, size.y)),
            corner_radius: 0.0,
            color: colors.gutter_border,
        });
    }

    for section in snapshot.sections.iter().filter(|section| section.revealed) {
        let hidden = layout
            .lines
            .iter()
            .filter(|line| line.start > section.line_end && line.start <= section.content_end);
        let Some(top) = hidden.clone().map(|line| line.y).reduce(f32::min) else {
            continue;
        };
        let bottom = hidden
            .map(|line| line.y + line.height)
            .reduce(f32::max)
            .unwrap_or(top);
        shapes.push(PageShape::Rect {
            rect: Rect::from_min_max(
                Pos2::new(0.0, origin.y + top),
                Pos2::new(size.x, origin.y + bottom),
            ),
            corner_radius: 0.0,
            color: colors.revealed_background,
        });
    }

    let number_x = gutter_width - GUTTER_PADDING_RIGHT;
    for line in &layout.lines {
        if line.show_line_number && gutter_width > 0.0 {
            let number = (line.document_line + 1).to_string();
            if let Some(galley) = layout_text(
                &number,
                FontId::monospace(GUTTER_TEXT_SIZE),
                TextLayout::DEFAULT,
            ) {
                let size = galley.size();
                shapes.push(PageShape::Text {
                    origin: Pos2::new(
                        number_x - size.x,
                        origin.y + line.y + (line.height - size.y) / 2.0,
                    ),
                    galley,
                    color: colors.gutter_text,
                });
            }
        }
        if let Some(section) = line_section(line, &snapshot.sections) {
            let glyph = if section.collapsed || section.revealed {
                ICON_KEYBOARD_ARROW_RIGHT
            } else {
                ICON_KEYBOARD_ARROW_DOWN
            };
            shapes.extend(centered(
                gutter_arrow_rect(origin, line),
                glyph,
                FontId::icons(GUTTER_ARROW_SIZE),
                colors.gutter_arrow,
            ));
        }
    }

    for widget in layout.widgets.iter().filter(|widget| !widget.block) {
        let rect = widget.rect.translate(origin);
        shapes.push(PageShape::Rect {
            rect,
            corner_radius: INLINE_WIDGET_RADIUS,
            color: match widget.broken {
                true => colors.broken_widget,
                false => colors.widget,
            },
        });
        if let Some(icon) = widget.icon {
            shapes.extend(centered(
                Rect::from_min_size(
                    Pos2::new(rect.min.x, rect.min.y),
                    Vec2::new(INLINE_WIDGET_ICON_INSET * 2.0, rect.height()),
                ),
                icon,
                FontId::icons(16.0),
                Color32::WHITE,
            ));
        }
    }

    let highlight = snapshot.highlight();
    for line in &layout.lines {
        let mut start = None;
        for byte in line.start..=line.end {
            let is_code =
                byte < line.end && highlight.style_at(byte).color == SynHlColorScope::MarkdownCode;
            if is_code {
                start.get_or_insert(byte);
                continue;
            }
            let Some(run_start) = start.take() else {
                continue;
            };
            let Some(left) = layout.positions.get(run_start).copied().flatten() else {
                continue;
            };
            let right = layout
                .positions
                .get(byte)
                .copied()
                .flatten()
                .filter(|right| right.line == left.line)
                .unwrap_or(BytePosition {
                    line: left.line,
                    x: line.width,
                });
            shapes.push(PageShape::Rect {
                rect: Rect::from_min_max(
                    Pos2::new(origin.x + left.x - 3.0, origin.y + line.y + 1.0),
                    Pos2::new(
                        origin.x + right.x + 3.0,
                        origin.y + line.y + line.height - 1.0,
                    ),
                ),
                corner_radius: 3.0,
                color: colors.code_background,
            });
        }
    }

    Page::new(shapes)
}

pub(crate) fn selection(
    layout: &DocumentLayout,
    ranges: &[Range<usize>],
    colors: &TextAreaColors,
    origin: Vec2,
) -> Page {
    let mut shapes = Vec::new();
    for range in ranges {
        for rect in selection_rects(layout, range, origin) {
            shapes.push(PageShape::Rect {
                rect,
                corner_radius: 0.0,
                color: colors.selection,
            });
        }
    }
    Page::new(shapes)
}

fn run_shapes(
    line: &LineLayout,
    run: &Run,
    colors: &TextAreaColors,
    origin: Vec2,
    shapes: &mut Vec<PageShape>,
) {
    let Some(galley) = &run.galley else {
        return;
    };
    let color = match run.invisible {
        true => colors.syntax.scope(SynHlColorScope::Invisible),
        false => colors.syntax.scope(run.style.color),
    };
    let top = origin.y + line.y + line.baseline - galley.baseline();
    let left = origin.x + run.x;
    shapes.push(PageShape::Text {
        origin: Pos2::new(left, top),
        galley: galley.clone(),
        color,
    });
    let font_size = run.font_size;
    let baseline = origin.y + line.y + line.baseline;
    let thickness = (font_size / 16.0).max(1.0);
    if run.style.underline {
        let y = baseline + (font_size * 0.12).max(1.0);
        shapes.push(PageShape::Rect {
            rect: Rect::from_min_size(Pos2::new(left, y), Vec2::new(run.width, thickness)),
            corner_radius: 0.0,
            color,
        });
    }
    if run.style.strikethrough {
        let y = baseline - font_size * 0.32;
        shapes.push(PageShape::Rect {
            rect: Rect::from_min_size(Pos2::new(left, y), Vec2::new(run.width, thickness)),
            corner_radius: 0.0,
            color,
        });
    }
}

pub(crate) fn placeholder(
    layout: &DocumentLayout,
    placeholder: &str,
    font: FontId,
    colors: &TextAreaColors,
    origin: Vec2,
) -> Option<PageShape> {
    let line = layout.lines.first()?;
    let galley = layout_text(placeholder, font, TextLayout::DEFAULT)?;
    Some(PageShape::Text {
        origin: Pos2::new(
            origin.x,
            origin.y + line.y + line.baseline - galley.baseline(),
        ),
        galley,
        color: colors.placeholder,
    })
}

pub(crate) fn content(
    layout: &DocumentLayout,
    snapshot: &Snapshot,
    colors: &TextAreaColors,
    origin: Vec2,
    placeholder: Option<PageShape>,
) -> Page {
    let mut shapes = Vec::from_iter(placeholder);
    for line in &layout.lines {
        for run in &line.runs {
            if run.invisible && !run.show_when_trailing {
                continue;
            }
            run_shapes(line, run, colors, origin, &mut shapes);
        }
    }

    for checkbox in &snapshot.checkboxes {
        let Some(rect) = checkbox_rect(layout, checkbox) else {
            continue;
        };
        let rect = rect.translate(origin);
        if checkbox.checked {
            shapes.push(PageShape::Rect {
                rect,
                corner_radius: CHECKBOX_RADIUS,
                color: colors.widget,
            });
        }
        shapes.push(PageShape::Outline {
            rect,
            corner_radius: CHECKBOX_RADIUS,
            width: CHECKBOX_OUTLINE,
            color: colors.gutter_arrow,
        });
        if checkbox.checked {
            shapes.extend(centered(
                rect,
                ICON_CHECK,
                FontId::icons(14.0),
                Color32::WHITE,
            ));
        }
    }

    for section in snapshot.sections.iter().filter(|section| section.collapsed) {
        let Some(line) = layout
            .lines
            .iter()
            .find(|line| line.start == section.line_start)
        else {
            continue;
        };
        shapes.extend(text(
            Pos2::new(
                origin.x + line.width + COLLAPSED_ELLIPSIS_GAP,
                origin.y + line.y + (line.height - GUTTER_TEXT_SIZE) / 2.0,
            ),
            "...",
            FontId::monospace(GUTTER_TEXT_SIZE),
            colors.gutter_arrow,
        ));
    }

    Page::new(shapes)
}

pub(crate) struct Overlay<'a> {
    pub layout: &'a DocumentLayout,
    pub colors: &'a TextAreaColors,
    pub origin: Vec2,
    pub selection: &'a [Range<usize>],
    pub remote: &'a [RemoteTextCursor],
    pub drop_caret: Option<usize>,
}

pub(crate) fn overlay(state: Overlay<'_>) -> Page {
    let Overlay {
        layout,
        colors,
        origin,
        selection,
        remote,
        drop_caret,
    } = state;
    let mut shapes = Vec::new();

    for line in &layout.lines {
        for run in &line.runs {
            if !run.invisible || run.show_when_trailing {
                continue;
            }
            if !selection
                .iter()
                .any(|range| range.start < run.range.end && range.end > run.range.start)
            {
                continue;
            }
            run_shapes(line, run, colors, origin, &mut shapes);
        }
    }

    for cursor in remote {
        let color = cursor.color;
        let fill = Color32::from_rgba_unmultiplied(
            color.to_array()[0],
            color.to_array()[1],
            color.to_array()[2],
            REMOTE_SELECTION_ALPHA,
        );
        for rect in selection_rects(layout, &cursor.selection, origin) {
            shapes.push(PageShape::Rect {
                rect,
                corner_radius: 0.0,
                color: fill,
            });
        }
        let Some(rect) = caret_rect(layout, cursor.caret, origin) else {
            continue;
        };
        shapes.push(PageShape::Rect {
            rect,
            corner_radius: 0.0,
            color,
        });
        shapes.push(PageShape::Rect {
            rect: Rect::from_min_size(
                Pos2::new(rect.min.x, rect.min.y - REMOTE_FLAG.y),
                REMOTE_FLAG,
            ),
            corner_radius: 0.0,
            color,
        });
    }

    if let Some(byte) = drop_caret
        && let Some(rect) = caret_rect(layout, byte, origin)
    {
        shapes.push(PageShape::Rect {
            rect,
            corner_radius: 0.0,
            color: colors.caret,
        });
    }

    Page::new(shapes)
}

pub(crate) fn carets(
    layout: &DocumentLayout,
    carets: &[usize],
    color: Color32,
    origin: Vec2,
) -> Page {
    Page::new(
        carets
            .iter()
            .filter_map(|caret| caret_rect(layout, *caret, origin))
            .map(|rect| PageShape::Rect {
                rect,
                corner_radius: 0.0,
                color,
            })
            .collect(),
    )
}

pub(crate) fn handles(
    layout: &DocumentLayout,
    handles: &[(SelectionHandle, usize)],
    color: Color32,
    origin: Vec2,
) -> Page {
    let mut shapes = Vec::new();
    for (handle, byte) in handles {
        if let Some(anchor) = touch_handle_anchor(layout, *byte) {
            shapes.extend(touch_handle_shapes(anchor + origin, *handle, color));
        }
    }
    Page::new(shapes)
}

fn preedit_galley(
    layout: &DocumentLayout,
    byte: usize,
    preedit: &str,
    font: FontId,
    origin: Vec2,
) -> Option<(Rect, f32, Galley)> {
    let caret = caret_rect(layout, byte, origin)?;
    let position = layout.positions.get(byte).copied().flatten()?;
    let line = layout.lines.get(position.line)?;
    let galley = layout_text(preedit, font, TextLayout::DEFAULT)?;
    Some((caret, origin.y + line.y + line.baseline, galley))
}

pub(crate) fn preedit_caret(
    layout: &DocumentLayout,
    byte: usize,
    preedit: &str,
    font: FontId,
    origin: Vec2,
) -> Option<Rect> {
    if preedit.is_empty() {
        return caret_rect(layout, byte, origin);
    }
    let (caret, _, galley) = preedit_galley(layout, byte, preedit, font, origin)?;
    Some(caret.translate(Vec2::new(galley.size().x, 0.0)))
}

pub(crate) fn preedit(
    layout: &DocumentLayout,
    byte: usize,
    preedit: &str,
    font: FontId,
    colors: &TextAreaColors,
    origin: Vec2,
) -> Page {
    let Some((caret, baseline, galley)) = preedit_galley(layout, byte, preedit, font, origin)
    else {
        return Page::new(Vec::new());
    };
    let width = galley.size().x;
    let font_size = font.size;
    let color = colors.syntax.scope(SynHlColorScope::Unstyled);
    let underline = baseline + (font_size * 0.12).max(1.0);
    Page::new(vec![
        PageShape::Rect {
            rect: Rect::from_min_size(caret.min, Vec2::new(width, caret.height())),
            corner_radius: 0.0,
            color: colors.surface,
        },
        PageShape::Text {
            origin: Pos2::new(caret.min.x, baseline - galley.baseline()),
            galley,
            color,
        },
        PageShape::Rect {
            rect: Rect::from_min_size(
                Pos2::new(caret.min.x, underline),
                Vec2::new(width, (font_size / 16.0).max(1.0)),
            ),
            corner_radius: 0.0,
            color,
        },
        PageShape::Rect {
            rect: caret.translate(Vec2::new(width, 0.0)),
            corner_radius: 0.0,
            color: colors.caret,
        },
    ])
}
