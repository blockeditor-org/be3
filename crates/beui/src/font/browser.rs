use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen::JsCast;

use super::{
    Advance, FontFamily, FontSources, Galley, GalleyData, GalleyLine, Shaping, TextLayout,
    break_lines,
};
use crate::geometry::vec2;

const PROPORTIONAL: &str =
    "Ubuntu, system-ui, -apple-system, \"Segoe UI\", Roboto, \"Helvetica Neue\", Arial, sans-serif";
const MONOSPACE: &str = "Hack, ui-monospace, SFMono-Regular, Menlo, Consolas, \"DejaVu Sans Mono\", \"Liberation Mono\", monospace";
const ICONS: &str = "beui-icons";
const MEASURED_PREFIX_LIMIT: usize = 48;

thread_local! {
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

pub(crate) fn fonts_loaded() {
    GENERATION.with(|generation| generation.set(generation.get() + 1));
}

pub(crate) fn css_family(family: FontFamily) -> &'static str {
    match family {
        FontFamily::Proportional => PROPORTIONAL,
        FontFamily::Monospace => MONOSPACE,
        FontFamily::Icons => ICONS,
    }
}

fn css(family: FontFamily, bold: bool, italic: bool, size: f32) -> String {
    let style = match italic {
        true => "italic",
        false => "normal",
    };
    let weight = match (family, bold) {
        (FontFamily::Proportional, false) => 300,
        (_, false) => 400,
        (_, true) => 700,
    };
    format!("{style} {weight} {size}px {}", css_family(family))
}

#[derive(Clone, Copy)]
struct Step {
    cluster: usize,
    x_advance: f32,
}

impl Advance for Step {
    fn cluster(&self) -> usize {
        self.cluster
    }

    fn x_advance(&self) -> f32 {
        self.x_advance
    }
}

pub(crate) struct Faces {
    context: Option<web_sys::CanvasRenderingContext2d>,
    font: String,
    generation: u64,
    segments: HashMap<Rc<str>, HashMap<String, Rc<[f32]>>>,
    metrics: HashMap<Rc<str>, (f32, f32)>,
}

impl Faces {
    pub(crate) fn new(_sources: &FontSources) -> Self {
        let context = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.create_element("canvas").ok())
            .and_then(|canvas| canvas.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .and_then(|canvas| canvas.get_context("2d").ok().flatten())
            .and_then(|context| context.dyn_into::<web_sys::CanvasRenderingContext2d>().ok());
        Self {
            context,
            font: String::new(),
            generation: GENERATION.with(Cell::get),
            segments: HashMap::new(),
            metrics: HashMap::new(),
        }
    }

    pub(crate) fn generation(&mut self) -> u64 {
        let generation = GENERATION.with(Cell::get);
        if generation != self.generation {
            self.generation = generation;
            self.segments.clear();
            self.metrics.clear();
            self.font.clear();
        }
        generation
    }

    fn select(&mut self, css: &str) {
        if self.font != css {
            if let Some(context) = &self.context {
                context.set_font(css);
            }
            css.clone_into(&mut self.font);
        }
    }

    fn width(&mut self, css: &str, text: &str) -> f32 {
        self.select(css);
        self.context
            .as_ref()
            .and_then(|context| context.measure_text(text).ok())
            .map_or(0.0, |metrics| metrics.width() as f32)
    }

    fn metrics(&mut self, css: &Rc<str>, pixel_size: u32) -> (f32, f32) {
        if let Some(metrics) = self.metrics.get(css) {
            return *metrics;
        }
        self.select(css);
        let measured = self
            .context
            .as_ref()
            .and_then(|context| context.measure_text("Hg").ok())
            .map(|metrics| {
                (
                    metrics.font_bounding_box_ascent() as f32,
                    metrics.font_bounding_box_descent() as f32,
                )
            })
            .filter(|(ascent, descent)| *ascent > 0.0 && descent.is_finite())
            .unwrap_or((pixel_size as f32 * 0.8, pixel_size as f32 * 0.2));
        self.metrics.insert(css.clone(), measured);
        measured
    }

    fn prefixes(&mut self, css: &Rc<str>, segment: &str) -> Rc<[f32]> {
        if let Some(known) = self.segments.get(css).and_then(|known| known.get(segment)) {
            return known.clone();
        }
        let mut prefixes = vec![0.0];
        let count = segment.chars().count();
        let mut total = 0.0;
        for (index, character) in segment.char_indices() {
            let end = index + character.len_utf8();
            total = match count <= MEASURED_PREFIX_LIMIT {
                true => self.width(css, &segment[..end]),
                false => total + self.prefixes(css, &segment[index..end])[1],
            };
            prefixes.push(total);
        }
        let prefixes: Rc<[f32]> = prefixes.into();
        self.segments
            .entry(css.clone())
            .or_default()
            .insert(segment.to_owned(), prefixes.clone());
        prefixes
    }

    fn steps(&mut self, css: &Rc<str>, line: &str) -> Vec<Step> {
        let mut steps = Vec::new();
        let mut start = 0;
        while start < line.len() {
            let space = line[start..].starts_with(char::is_whitespace);
            let end = line[start..]
                .char_indices()
                .find(|(_, character)| character.is_whitespace() != space)
                .map_or(line.len(), |(index, _)| start + index);
            let segment = &line[start..end];
            let prefixes = self.prefixes(css, segment);
            for (position, (index, _)) in segment.char_indices().enumerate() {
                steps.push(Step {
                    cluster: start + index,
                    x_advance: prefixes[position + 1] - prefixes[position],
                });
            }
            start = end;
        }
        steps
    }

    pub(crate) fn build(
        &mut self,
        text: &str,
        family: FontFamily,
        pixel_size: u32,
        shape: Shaping,
        scale: f32,
    ) -> Galley {
        let measured: Rc<str> = css(family, shape.bold, shape.italic, pixel_size as f32).into();
        let shown: Rc<str> =
            css(family, shape.bold, shape.italic, pixel_size as f32 / scale).into();
        let (ascent, descent) = self.metrics(&measured, pixel_size);
        let line_height = (shape.line() * shape.spacing()).round().max(1.0);
        let baseline = ((line_height - (ascent + descent)) / 2.0).round() + ascent;
        let mut rows = Vec::new();
        let mut width = 0.0f32;
        let mut start = 0;

        for line in text.split('\n') {
            let steps = self.steps(&measured, line);
            let runs = break_lines(&steps, line, shape.wrap());
            let last = runs.len() - 1;
            for (index, run) in runs.into_iter().enumerate() {
                let end = start
                    + match steps.get(run.end) {
                        Some(step) if index < last => step.cluster,
                        _ => line.len(),
                    };
                let row = steps[run]
                    .iter()
                    .map(|step| Step {
                        cluster: start + step.cluster,
                        ..*step
                    })
                    .collect::<Vec<_>>();
                let advance = row.iter().map(|step| step.x_advance).sum::<f32>();
                width = width.max(advance);
                rows.push((row, end, advance));
            }
            start += line.len() + 1;
        }

        let mut lines = Vec::new();
        let mut cursor = 0.0;
        for (row, end, advance) in rows {
            let indent = TextLayout {
                align: shape.align,
                ..TextLayout::DEFAULT
            }
            .indent_of(width, advance);
            let mut pen = indent;
            let mut cursors = Vec::new();
            for step in &row {
                if cursors
                    .last()
                    .is_none_or(|(previous, _)| *previous != step.cluster)
                {
                    cursors.push((step.cluster, pen / scale));
                }
                pen += step.x_advance;
            }
            cursors.push((end, pen / scale));
            lines.push(GalleyLine {
                top: cursor / scale,
                range: cursors[0].0..end,
                cursors,
            });
            cursor += line_height;
        }

        Galley {
            inner: Rc::new(GalleyData {
                size: vec2(width.ceil() / scale, cursor.ceil() / scale),
                line_height: line_height / scale,
                baseline: baseline / scale,
                pixel_bounds: [0.0, 0.0, width.ceil(), cursor.ceil()],
                glyphs: Vec::new(),
                lines,
                blank: text.chars().all(char::is_whitespace),
                text: text.to_owned(),
                css: shown,
            }),
        }
    }
}
