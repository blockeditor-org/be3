use stable_hash::HashMap;
use std::cell::Cell;
use std::error::Error;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use beui_core::font::{FontBackend, Galley, GalleyLine, Shaping};
use beui_types::vec2;
use beui_types::{FontFamily, FontId};

use crate::{ICONS, css_font};

const MEASURED_PREFIX_LIMIT: usize = 48;

thread_local! {
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

fn fonts_loaded() {
    GENERATION.with(|generation| generation.set(generation.get() + 1));
}

pub fn watch(icons_url: Option<&str>, loaded: impl Fn() + 'static) -> Result<(), Box<dyn Error>> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("no browser document is available")?;
    if let Some(url) = icons_url {
        let style = document
            .create_element("style")
            .map_err(|_| "could not create a stylesheet")?;
        style.set_text_content(Some(&format!(
            "@font-face{{font-family:{ICONS};src:url(\"{url}\");font-display:block}}"
        )));
        document
            .head()
            .ok_or("the page has no head")?
            .append_child(&style)
            .map_err(|_| "could not add the icon font to the page")?;
    }
    let loaded = Rc::new(loaded);
    let fonts = document.fonts();
    let done = Closure::<dyn FnMut(web_sys::Event)>::new({
        let loaded = loaded.clone();
        move |_event: web_sys::Event| {
            fonts_loaded();
            loaded();
        }
    });
    fonts
        .add_event_listener_with_callback("loadingdone", done.as_ref().unchecked_ref())
        .map_err(|_| "could not watch the page's fonts")?;
    done.forget();
    if icons_url.is_some() {
        let icons = Closure::<dyn FnMut(wasm_bindgen::JsValue)>::new(move |_| {
            fonts_loaded();
            loaded();
        });
        let _ = fonts.load(&format!("24px {ICONS}")).then(&icons);
        icons.forget();
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Step {
    cluster: usize,
    x_advance: f32,
}

pub struct BrowserFonts {
    context: Option<web_sys::CanvasRenderingContext2d>,
    font: String,
    generation: u64,
    segments: HashMap<Rc<str>, HashMap<String, Rc<[f32]>>>,
    metrics: HashMap<Rc<str>, (f32, f32)>,
}

impl Default for BrowserFonts {
    fn default() -> Self {
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
            segments: HashMap::default(),
            metrics: HashMap::default(),
        }
    }
}

impl BrowserFonts {
    fn refresh(&mut self) {
        let generation = GENERATION.with(Cell::get);
        if generation != self.generation {
            self.generation = generation;
            self.segments.clear();
            self.metrics.clear();
            self.font.clear();
        }
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
}

impl FontBackend for BrowserFonts {
    fn shape(
        &mut self,
        text: &str,
        family: FontFamily,
        pixel_size: u32,
        shape: Shaping,
        scale: f32,
    ) -> Galley {
        self.refresh();
        let font = FontId {
            size: pixel_size as f32,
            family,
            bold: shape.bold,
            italic: shape.italic,
        };
        let measured: Rc<str> = css_font(font).into();
        let (ascent, descent) = self.metrics(&measured, pixel_size);
        let line_height = shape.line();
        let baseline = ((line_height - (ascent + descent)) / 2.0).round() + ascent;
        let mut pen = 0.0;
        let mut cursors = Vec::new();
        for step in self.steps(&measured, text) {
            if cursors
                .last()
                .is_none_or(|(previous, _)| *previous != step.cluster)
            {
                cursors.push((step.cluster, pen / scale));
            }
            pen += step.x_advance;
        }
        cursors.push((text.len(), pen / scale));
        Galley::new(
            text,
            FontId {
                size: pixel_size as f32 / scale,
                ..font
            },
            vec2(pen.ceil() / scale, line_height / scale),
            line_height / scale,
            baseline / scale,
            Vec::new(),
            vec![GalleyLine {
                top: 0.0,
                range: 0..text.len(),
                cursors,
            }],
            scale,
        )
    }

    fn generation(&self) -> u64 {
        GENERATION.with(Cell::get)
    }
}
