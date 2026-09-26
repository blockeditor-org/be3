use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write as _;
use std::hash::{DefaultHasher, Hash, Hasher};

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use super::browser::{self, Host, Screen};
use super::{App, RunOptions, Setup};
use crate::color::Color32;
use crate::context::{Context, FrameOutput};
use crate::font::{Galley, fonts_loaded};
use crate::geometry::{Pos2, Rect, Rotation, Vec2, vec2};
use crate::image::{Image, ImageId};
use crate::painter::Shape;

const STYLE: &str = "\
.beui-root{position:relative;overflow:hidden;user-select:none;-webkit-user-select:none;touch-action:none;outline:none}\
.beui-layer{position:absolute;left:0;top:0;width:0;height:0;transform-origin:0 0;pointer-events:none;contain:layout style}\
.beui-layer div{position:absolute;box-sizing:border-box;white-space:pre;pointer-events:none;font-variant-ligatures:none}";

struct Runner {
    host: Host,
    root: web_sys::HtmlElement,
    scene: Scene,
    prepared: Option<(Vec2, f32, Color32)>,
}

impl Screen for Runner {
    fn host(&self) -> &Host {
        &self.host
    }

    fn frame(&mut self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let ratio = window.device_pixel_ratio() as f32;
        let bounds = self.root.get_bounding_client_rect();
        let physical = vec2(
            ((bounds.width() as f32) * ratio).round().max(1.0),
            ((bounds.height() as f32) * ratio).round().max(1.0),
        );
        let step = self.host.step(&window, ratio, physical);
        let background = self.host.app.clear_color();
        let stale = self.prepared != Some((physical, step.scale, background));
        if step.output.changed || stale {
            if self.prepared.is_none_or(|(_, _, old)| old != background) {
                let _ = self
                    .root
                    .style()
                    .set_property("background", &rgba(background));
            }
            self.scene.show(&step.output, step.scale, ratio);
            self.prepared = Some((physical, step.scale, background));
        }
        self.host.finish(&step);
    }
}

pub async fn run_dom(
    root_id: &str,
    options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let window = web_sys::window().ok_or("no browser window is available")?;
    let document = window
        .document()
        .ok_or("no browser document is available")?;
    let root = document
        .get_element_by_id(root_id)
        .ok_or_else(|| format!("no element has the id {root_id}"))?
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| format!("the element {root_id} is not an html element"))?;
    document.set_title(&options.title);
    install_style(&document, options.icons_font.as_deref())?;
    watch_fonts(&document)?;
    root.class_list()
        .add_1("beui-root")
        .map_err(|_| "could not style the root element")?;
    let layer = create_div(&document)?;
    layer.set_class_name("beui-layer");
    root.append_child(&layer)
        .map_err(|_| "could not add the layer to the root element")?;
    let agent = browser::text_agent(&document)?;

    let mut app: Box<dyn App> = Box::new(app);
    app.setup(&Setup {
        waker: browser::waker(),
    });
    let host = Host::new(app, Context::new(), root.clone(), agent, options);
    browser::start(Runner {
        host,
        root,
        scene: Scene {
            document,
            layer,
            entries: Vec::new(),
            images: HashMap::new(),
            zoom: 1.0,
        },
        prepared: None,
    })
}

fn install_style(document: &web_sys::Document, icons: Option<&str>) -> Result<(), Box<dyn Error>> {
    let style = document
        .create_element("style")
        .map_err(|_| "could not create a stylesheet")?;
    let mut css = STYLE.to_owned();
    if let Some(url) = icons {
        let _ = write!(
            css,
            "@font-face{{font-family:{};src:url(\"{url}\");font-display:block}}",
            crate::font::css_family(crate::font::FontFamily::Icons)
        );
    }
    style.set_text_content(Some(&css));
    document
        .head()
        .ok_or("the page has no head")?
        .append_child(&style)
        .map_err(|_| "could not add the stylesheet to the page")?;
    Ok(())
}

fn watch_fonts(document: &web_sys::Document) -> Result<(), Box<dyn Error>> {
    let fonts = document.fonts();
    browser::on(fonts.as_ref(), "loadingdone", |_event: web_sys::Event| {
        fonts_loaded();
        browser::schedule();
    })?;
    let icons = format!(
        "24px {}",
        crate::font::css_family(crate::font::FontFamily::Icons)
    );
    let loaded = Closure::<dyn FnMut(wasm_bindgen::JsValue)>::new(|_| {
        fonts_loaded();
        browser::schedule();
    });
    let _ = fonts.load(&icons).then(&loaded);
    loaded.forget();
    Ok(())
}

fn create_div(document: &web_sys::Document) -> Result<web_sys::HtmlElement, Box<dyn Error>> {
    Ok(document
        .create_element("div")
        .map_err(|_| "could not create an element")?
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| "could not create an element")?)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Box,
    Text,
}

struct Entry {
    key: u64,
    kind: Kind,
    element: web_sys::HtmlElement,
}

struct Look {
    kind: Kind,
    css: String,
    lines: Vec<(String, String)>,
}

impl Look {
    fn key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.kind.hash(&mut hasher);
        self.css.hash(&mut hasher);
        self.lines.hash(&mut hasher);
        hasher.finish()
    }
}

struct Scene {
    document: web_sys::Document,
    layer: web_sys::HtmlElement,
    entries: Vec<Entry>,
    images: HashMap<ImageId, String>,
    zoom: f32,
}

impl Scene {
    fn show(&mut self, output: &FrameOutput, scale: f32, ratio: f32) {
        let zoom = scale / ratio;
        if zoom != self.zoom {
            self.zoom = zoom;
            let _ = self.layer.style().set_property(
                "transform",
                &match zoom == 1.0 {
                    true => String::new(),
                    false => format!("scale({zoom})"),
                },
            );
        }
        let mut images = HashMap::new();
        let looks = output
            .shapes()
            .iter()
            .filter_map(|shape| self.look(shape, scale, &mut images))
            .map(|look| (look.key(), look))
            .collect::<Vec<_>>();
        self.images = images;
        self.reconcile(looks);
    }

    fn reconcile(&mut self, looks: Vec<(u64, Look)>) {
        let old = std::mem::take(&mut self.entries);
        let mut by_key: HashMap<u64, Vec<usize>> = HashMap::new();
        for (index, entry) in old.iter().enumerate().rev() {
            by_key.entry(entry.key).or_default().push(index);
        }
        let mut used = vec![false; old.len()];
        let mut assigned = vec![None; looks.len()];
        for (index, (key, _)) in looks.iter().enumerate() {
            if let Some(found) = by_key.get_mut(key).and_then(Vec::pop) {
                assigned[index] = Some(found);
                used[found] = true;
            }
        }
        let mut pool: HashMap<Kind, Vec<usize>> = HashMap::new();
        for (index, entry) in old.iter().enumerate().rev() {
            if !used[index] {
                pool.entry(entry.kind).or_default().push(index);
            }
        }
        let mut rewritten = vec![false; looks.len()];
        for (index, (_, look)) in looks.iter().enumerate() {
            if assigned[index].is_some() {
                continue;
            }
            if let Some(found) = pool.get_mut(&look.kind).and_then(Vec::pop) {
                assigned[index] = Some(found);
                used[found] = true;
                rewritten[index] = true;
            }
        }
        for (index, entry) in old.iter().enumerate() {
            if !used[index] {
                entry.element.remove();
            }
        }
        let kept = longest_increasing(&assigned);
        let mut next: Option<web_sys::HtmlElement> = None;
        let mut entries = Vec::with_capacity(looks.len());
        for (index, (key, look)) in looks.into_iter().enumerate().rev() {
            let element = match assigned[index] {
                Some(found) => old[found].element.clone(),
                None => match create_div(&self.document) {
                    Ok(element) => element,
                    Err(_) => continue,
                },
            };
            if assigned[index].is_none() || rewritten[index] {
                self.write(&element, &look);
            }
            if !kept[index] {
                let _ = self
                    .layer
                    .insert_before(&element, next.as_ref().map(AsRef::as_ref));
            }
            next = Some(element.clone());
            entries.push(Entry {
                key,
                kind: look.kind,
                element,
            });
        }
        entries.reverse();
        self.entries = entries;
    }

    fn write(&self, element: &web_sys::HtmlElement, look: &Look) {
        element.style().set_css_text(&look.css);
        if look.kind != Kind::Text {
            return;
        }
        element.set_text_content(None);
        for (text, css) in &look.lines {
            let Ok(line) = create_div(&self.document) else {
                continue;
            };
            line.style().set_css_text(css);
            line.set_text_content(Some(text));
            let _ = element.append_child(&line);
        }
    }

    fn look(
        &self,
        shape: &Shape,
        scale: f32,
        images: &mut HashMap<ImageId, String>,
    ) -> Option<Look> {
        match shape {
            Shape::Rect {
                rect,
                corner_radius,
                stroke_width,
                color,
                rotation,
                clip,
            } => {
                let rect = snapped(*rect, scale);
                let mut css = placed(rect, *clip, *rotation)?;
                let _ = write!(css, "border-radius:{}px;", corner_radius);
                match *stroke_width > 0.0 {
                    true => {
                        let width = ((stroke_width * scale).round().max(1.0)) / scale;
                        let _ = write!(css, "border:{width}px solid {};", rgba(*color));
                    }
                    false => {
                        let _ = write!(css, "background:{};", rgba(*color));
                    }
                }
                Some(Look {
                    kind: Kind::Box,
                    css,
                    lines: Vec::new(),
                })
            }
            Shape::Text {
                origin,
                galley,
                color,
                rotation,
                clip,
            } => text(*origin, galley, *color, *rotation, *clip, scale),
            Shape::Image {
                rect,
                source,
                image,
                tint,
                corner_radius,
                smooth,
                rotation,
                clip,
            } => {
                let url = match self.images.get(&image.id()) {
                    Some(url) => url.clone(),
                    None => data_url(&self.document, image)?,
                };
                images.insert(image.id(), url.clone());
                let rect = snapped(*rect, scale);
                let mut css = placed(rect, *clip, *rotation)?;
                let span = vec2(
                    (source.width()).max(f32::EPSILON),
                    (source.height()).max(f32::EPSILON),
                );
                let size = vec2(rect.width() / span.x, rect.height() / span.y);
                let _ = write!(
                    css,
                    "background-image:url(\"{url}\");background-repeat:no-repeat;\
                     background-size:{}px {}px;background-position:{}px {}px;border-radius:{}px;opacity:{};",
                    size.x,
                    size.y,
                    -source.min.x * size.x,
                    -source.min.y * size.y,
                    corner_radius,
                    f32::from(tint.alpha()) / 255.0,
                );
                if !smooth {
                    css.push_str("image-rendering:pixelated;");
                }
                Some(Look {
                    kind: Kind::Box,
                    css,
                    lines: Vec::new(),
                })
            }
            Shape::Line {
                from,
                to,
                width,
                color,
                clip,
            } => {
                let width = (width * scale).max(1.0) / scale;
                let bounds = Rect::from_points(&[*from, *to]).expand(width);
                if !bounds.intersects(*clip) {
                    return None;
                }
                let span = *to - *from;
                let length = span.length();
                let angle = span.y.atan2(span.x);
                let css = format!(
                    "left:{}px;top:{}px;width:{}px;height:{width}px;border-radius:{}px;\
                     transform-origin:{}px 50%;transform:rotate({angle}rad);background:{};",
                    from.x - width / 2.0,
                    from.y - width / 2.0,
                    length + width,
                    width / 2.0,
                    width / 2.0,
                    rgba(*color),
                );
                Some(Look {
                    kind: Kind::Box,
                    css,
                    lines: Vec::new(),
                })
            }
            Shape::Punch { .. } | Shape::Drawing { .. } => None,
        }
    }
}

fn text(
    origin: Pos2,
    galley: &Galley,
    color: Color32,
    rotation: Rotation,
    clip: Rect,
    scale: f32,
) -> Option<Look> {
    let origin = Pos2::new(
        (origin.x * scale).round() / scale,
        (origin.y * scale).round() / scale,
    );
    let rect = Rect::from_min_size(origin, galley.size());
    let mut css = placed(rect, clip, rotation)?;
    let _ = write!(css, "font:{};color:{};", galley.css(), rgba(color));
    let line_height = galley.line_height();
    let lines = galley
        .text_lines()
        .filter(|(text, _)| !text.trim().is_empty())
        .map(|(text, at)| {
            (
                text.trim_end_matches(['\n', '\r']).replace('\t', " "),
                format!(
                    "left:{}px;top:{}px;height:{line_height}px;line-height:{line_height}px;",
                    at.x, at.y
                ),
            )
        })
        .collect();
    Some(Look {
        kind: Kind::Text,
        css,
        lines,
    })
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

fn snapped(rect: Rect, scale: f32) -> Rect {
    let snap = |value: f32| (value * scale).round() / scale;
    let min = Pos2::new(snap(rect.min.x), snap(rect.min.y));
    let max = Pos2::new(
        snap(rect.max.x).max(min.x + 1.0 / scale),
        snap(rect.max.y).max(min.y + 1.0 / scale),
    );
    Rect::from_min_max(min, max)
}

fn rgba(color: Color32) -> String {
    let [red, green, blue, alpha] = color.to_array();
    format!("rgba({red},{green},{blue},{})", f32::from(alpha) / 255.0)
}

fn data_url(document: &web_sys::Document, image: &Image) -> Option<String> {
    let canvas = document
        .create_element("canvas")
        .ok()?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .ok()?;
    canvas.set_width(image.width());
    canvas.set_height(image.height());
    let context = canvas
        .get_context("2d")
        .ok()??
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .ok()?;
    let data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(image.pixels()),
        image.width(),
        image.height(),
    )
    .ok()?;
    context.put_image_data(&data, 0.0, 0.0).ok()?;
    canvas.to_data_url().ok()
}

fn longest_increasing(assigned: &[Option<usize>]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![None; assigned.len()];
    for (index, value) in assigned.iter().enumerate() {
        let Some(value) = *value else {
            continue;
        };
        let at = tails
            .partition_point(|&tail| assigned[tail].is_some_and(|candidate| candidate < value));
        if at > 0 {
            previous[index] = Some(tails[at - 1]);
        }
        match at == tails.len() {
            true => tails.push(index),
            false => tails[at] = index,
        }
    }
    let mut kept = vec![false; assigned.len()];
    let mut current = tails.last().copied();
    while let Some(index) = current {
        kept[index] = true;
        current = previous[index];
    }
    kept
}
