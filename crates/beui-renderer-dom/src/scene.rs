use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::rc::Rc;

use wasm_bindgen::JsCast;

use beui_core::color::Color32;
use beui_core::context::{FrameOutput, RendererInfo};
use beui_core::display::{Display, Layer, Part};
use beui_core::geometry::Vec2;
use beui_core::image::{Image, ImageId};
use beui_core::painter::{Entry, Shape};
use beui_core::renderer::Renderer;

use crate::style::{self, Look};

const STYLE: &str = "\
.beui-root{position:relative;overflow:hidden;user-select:none;-webkit-user-select:none;touch-action:none;outline:none}\
.beui-stage{position:absolute;left:0;top:0;width:0;height:0;transform-origin:0 0;pointer-events:none}\
.beui-stage div{position:absolute;left:0;top:0;box-sizing:border-box;white-space:pre;pointer-events:none;font-variant-ligatures:none}\
.beui-stage .beui-layer{isolation:isolate}\
.beui-stage .beui-scaled{transform-origin:0 0}";
const ROOT: u64 = 0;
const IMAGE_LIMIT: usize = 512;

#[derive(Clone)]
struct Child {
    id: u64,
    element: web_sys::HtmlElement,
}

struct Painted {
    child: Child,
    top: bool,
}

struct Node {
    frame: Child,
    content: web_sys::HtmlElement,
    display: Option<Rc<Display>>,
    factor: f32,
    parent: u64,
    placed: Option<Entry>,
    painted: Vec<Option<Painted>>,
    children: Vec<Child>,
    kids: Vec<u64>,
}

struct Root {
    wrap: Child,
    scaled: web_sys::HtmlElement,
    holds: Option<u64>,
    placed: Option<(beui_core::geometry::Rect, f32)>,
}

struct Loose {
    shape: Shape,
    child: Option<Child>,
}

pub struct DomRenderer {
    document: web_sys::Document,
    root: web_sys::HtmlElement,
    stage: web_sys::HtmlElement,
    next: u64,
    layers: Vec<Child>,
    roots: HashMap<u64, Root>,
    loose: Vec<Loose>,
    nodes: HashMap<u64, Node>,
    images: HashMap<ImageId, Rc<str>>,
    prepared: Option<(f32, f32, Color32)>,
    size: Option<(u32, u32)>,
}

impl DomRenderer {
    pub fn new(root: web_sys::HtmlElement) -> Result<Self, Box<dyn Error>> {
        let document = root
            .owner_document()
            .ok_or("the element is not in a document")?;
        let style = document
            .create_element("style")
            .map_err(|_| "could not create a stylesheet")?;
        style.set_text_content(Some(STYLE));
        document
            .head()
            .ok_or("the page has no head")?
            .append_child(&style)
            .map_err(|_| "could not add the stylesheet to the page")?;
        root.class_list()
            .add_1("beui-root")
            .map_err(|_| "could not style the root element")?;
        let stage = div(&document)?;
        stage.set_class_name("beui-stage");
        root.append_child(&stage)
            .map_err(|_| "could not add the stage to the root element")?;
        Ok(Self {
            document,
            root,
            stage,
            next: ROOT + 1,
            layers: Vec::new(),
            roots: HashMap::new(),
            loose: Vec::new(),
            nodes: HashMap::new(),
            images: HashMap::new(),
            prepared: None,
            size: None,
        })
    }

    fn show(&mut self, output: &FrameOutput, scale: f32, background: Color32) {
        let ratio = web_sys::window().map_or(1.0, |window| window.device_pixel_ratio() as f32);
        let zoom = scale / ratio;
        let prepared = Some((scale, zoom, background));
        if !output.changed && self.prepared == prepared {
            return;
        }
        let held = self.prepared.replace((scale, zoom, background));
        if held.is_none_or(|(_, _, old)| old != background) {
            let _ = self
                .root
                .style()
                .set_property("background", &style::rgba(background));
        }
        if held.is_none_or(|(_, old, _)| old != zoom) {
            let _ = self.stage.style().set_property(
                "transform",
                &match zoom == 1.0 {
                    true => String::new(),
                    false => format!("scale({zoom})"),
                },
            );
        }
        if held.is_some_and(|(old, _, _)| old != scale) {
            self.clear();
        }
        let mut desired = Vec::new();
        let mut loose = 0;
        let mut shown = HashSet::new();
        for layer in output.layers.iter() {
            match layer {
                Layer::Shape(shape) => {
                    if let Some(child) = self.loose(loose, shape, scale) {
                        desired.push(child);
                    }
                    loose += 1;
                }
                Layer::Display {
                    display,
                    entry,
                    scale: by,
                    clip,
                } => {
                    if let Some(wrap) = self.layer(display, *entry, *by, *clip, scale) {
                        shown.insert(display.key);
                        desired.push(wrap);
                    }
                }
            }
        }
        self.loose.truncate(loose);
        reconcile(&self.stage, &mut self.layers, desired);
        let gone: Vec<u64> = self
            .roots
            .keys()
            .filter(|key| !shown.contains(key))
            .copied()
            .collect();
        for key in gone {
            self.roots.remove(&key);
            self.release(key, ROOT);
        }
    }

    fn clear(&mut self) {
        self.stage.set_text_content(None);
        self.layers.clear();
        self.roots.clear();
        self.loose.clear();
        self.nodes.clear();
    }

    fn child(&mut self) -> Option<Child> {
        let element = div(&self.document).ok()?;
        self.next += 1;
        Some(Child {
            id: self.next,
            element,
        })
    }

    fn loose(&mut self, index: usize, shape: &Shape, factor: f32) -> Option<Child> {
        if let Some(held) = self.loose.get(index)
            && held.shape == *shape
        {
            return held.child.clone();
        }
        let reused = self.loose.get_mut(index).and_then(|held| held.child.take());
        let child = self.paint(reused, shape, factor, false);
        let held = Loose {
            shape: shape.clone(),
            child: child.clone(),
        };
        match index < self.loose.len() {
            true => self.loose[index] = held,
            false => self.loose.push(held),
        }
        child
    }

    fn layer(
        &mut self,
        display: &Rc<Display>,
        entry: Entry,
        by: f32,
        clip: beui_core::geometry::Rect,
        scale: f32,
    ) -> Option<Child> {
        let frame = self.visit(display, scale * by, ROOT)?;
        self.place(display.key, entry, scale * by);
        if !self.roots.contains_key(&display.key) {
            let wrap = self.child()?;
            wrap.element.set_class_name("beui-layer");
            let scaled = div(&self.document).ok()?;
            scaled.set_class_name("beui-scaled");
            let _ = wrap.element.append_child(&scaled);
            self.roots.insert(
                display.key,
                Root {
                    wrap,
                    scaled,
                    holds: None,
                    placed: None,
                },
            );
        }
        let root = self.roots.get_mut(&display.key)?;
        if root.holds != Some(frame.id) {
            root.scaled.set_text_content(None);
            let _ = root.scaled.append_child(&frame.element);
            root.holds = Some(frame.id);
        }
        if root.placed != Some((clip, by)) {
            root.placed = Some((clip, by));
            let (wrap, scaled) = style::layer(clip, by, scale);
            root.wrap.element.style().set_css_text(&wrap);
            root.scaled.style().set_css_text(&scaled);
        }
        Some(root.wrap.clone())
    }

    fn visit(&mut self, display: &Rc<Display>, factor: f32, parent: u64) -> Option<Child> {
        let key = display.key;
        let mut node = match self.nodes.remove(&key) {
            Some(node) => node,
            None => {
                let frame = self.child()?;
                frame.element.set_class_name("beui-frame");
                let content = div(&self.document).ok()?;
                let _ = frame.element.append_child(&content);
                Node {
                    frame,
                    content,
                    display: None,
                    factor,
                    parent,
                    placed: None,
                    painted: Vec::new(),
                    children: Vec::new(),
                    kids: Vec::new(),
                }
            }
        };
        node.parent = parent;
        let unchanged = node.factor == factor
            && node
                .display
                .as_ref()
                .is_some_and(|held| Rc::ptr_eq(held, display));
        if !unchanged {
            self.update(&mut node, display, factor);
        }
        let frame = node.frame.clone();
        self.nodes.insert(key, node);
        Some(frame)
    }

    fn update(&mut self, node: &mut Node, display: &Rc<Display>, factor: f32) {
        let mut tops = vec![false; display.shapes.len()];
        for part in display.parts.iter() {
            if let Part::Shapes {
                top: true,
                start,
                end,
            } = part
            {
                tops[*start as usize..*end as usize].fill(true);
            }
        }
        let held = node
            .display
            .as_ref()
            .filter(|_| node.factor == factor)
            .map(|held| Rc::clone(&held.shapes));
        let same_shapes = held
            .as_ref()
            .is_some_and(|held| Rc::ptr_eq(held, &display.shapes));
        let mut old = std::mem::take(&mut node.painted);
        let mut painted = Vec::with_capacity(display.shapes.len());
        for (index, shape) in display.shapes.iter().enumerate() {
            let top = tops[index];
            let previous = old.get_mut(index).and_then(Option::take);
            let same = held
                .as_ref()
                .is_some_and(|held| same_shapes || held.get(index) == Some(shape));
            match previous {
                Some(previous) if same && previous.top == top => painted.push(Some(previous)),
                previous => {
                    let reused = previous.map(|previous| previous.child);
                    painted.push(
                        self.paint(reused, shape, factor, top)
                            .map(|child| Painted { child, top }),
                    );
                }
            }
        }
        let mut desired = Vec::new();
        let mut kids = Vec::new();
        for part in display.parts.iter() {
            match part {
                Part::Shapes { start, end, .. } => desired.extend(
                    painted[*start as usize..*end as usize]
                        .iter()
                        .flatten()
                        .map(|painted| painted.child.clone()),
                ),
                Part::Child(_, entry, child) => {
                    if let Some(frame) = self.visit(child, factor, display.key) {
                        self.place(child.key, *entry, factor);
                        kids.push(child.key);
                        desired.push(frame);
                    }
                }
            }
        }
        reconcile(&node.content, &mut node.children, desired);
        let kept: HashSet<u64> = kids.iter().copied().collect();
        for kid in std::mem::replace(&mut node.kids, kids) {
            if !kept.contains(&kid) {
                self.release(kid, display.key);
            }
        }
        node.painted = painted;
        node.display = Some(Rc::clone(display));
        node.factor = factor;
    }

    fn place(&mut self, key: u64, entry: Entry, factor: f32) {
        let Some(node) = self.nodes.get_mut(&key) else {
            return;
        };
        if node.placed == Some(entry) {
            return;
        }
        node.placed = Some(entry);
        let (frame, content) = style::placement(entry, factor);
        node.frame.element.style().set_css_text(&frame);
        node.content.style().set_css_text(&content);
    }

    fn release(&mut self, key: u64, parent: u64) {
        let mut pending = vec![(key, parent)];
        while let Some((key, parent)) = pending.pop() {
            if self
                .nodes
                .get(&key)
                .is_none_or(|node| node.parent != parent)
            {
                continue;
            }
            if let Some(node) = self.nodes.remove(&key) {
                pending.extend(node.kids.into_iter().map(|kid| (kid, key)));
            }
        }
    }

    fn paint(
        &mut self,
        reused: Option<Child>,
        shape: &Shape,
        factor: f32,
        top: bool,
    ) -> Option<Child> {
        let image = match shape {
            Shape::Image { image, .. } => self.image(image),
            _ => None,
        };
        let look = style::look(shape, factor, top, image.as_deref())?;
        let child = match reused {
            Some(child) => child,
            None => self.child()?,
        };
        child.element.set_text_content(None);
        match look {
            Look::Box(css) => child.element.style().set_css_text(&css),
            Look::Text { css, lines } => {
                child.element.style().set_css_text(&css);
                for line in lines {
                    let Ok(row) = div(&self.document) else {
                        continue;
                    };
                    row.style().set_css_text(&line.css);
                    row.set_text_content(Some(&line.text));
                    let _ = child.element.append_child(&row);
                }
            }
        }
        Some(child)
    }

    fn image(&mut self, image: &Image) -> Option<Rc<str>> {
        if let Some(url) = self.images.get(&image.id()) {
            return Some(Rc::clone(url));
        }
        if self.images.len() >= IMAGE_LIMIT {
            self.images.clear();
        }
        let url: Rc<str> = data_url(&self.document, image)?.into();
        self.images.insert(image.id(), Rc::clone(&url));
        Some(url)
    }
}

impl Renderer for DomRenderer {
    fn name(&self) -> &'static str {
        "DOM"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo {
            rows: vec![("Renderer", "DOM".to_owned())],
        }
    }

    fn set_active(&mut self, active: bool) {
        let display = match active {
            true => "",
            false => "none",
        };
        let _ = self.stage.style().set_property("display", display);
        if !active {
            self.prepared = None;
            self.clear();
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = Some((width, height));
    }

    fn physical(&self) -> Option<Vec2> {
        self.size
            .map(|(width, height)| Vec2::new(width as f32, height as f32))
    }

    fn prepare(&mut self, output: &FrameOutput, scale: f32, background: Color32) -> bool {
        self.show(output, scale, background);
        false
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

fn div(document: &web_sys::Document) -> Result<web_sys::HtmlElement, Box<dyn Error>> {
    Ok(document
        .create_element("div")
        .map_err(|_| "could not create an element")?
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| "could not create an element")?)
}

fn reconcile(parent: &web_sys::HtmlElement, current: &mut Vec<Child>, desired: Vec<Child>) {
    if current.len() == desired.len()
        && current
            .iter()
            .zip(&desired)
            .all(|(held, wanted)| held.id == wanted.id)
    {
        return;
    }
    let wanted: HashSet<u64> = desired.iter().map(|child| child.id).collect();
    let mut positions = HashMap::new();
    for held in current.iter() {
        match wanted.contains(&held.id) {
            true => {
                positions.insert(held.id, positions.len());
            }
            false => {
                let owner: &web_sys::Node = parent.as_ref();
                if held
                    .element
                    .parent_node()
                    .is_some_and(|node| node.is_same_node(Some(owner)))
                {
                    held.element.remove();
                }
            }
        }
    }
    let assigned: Vec<Option<usize>> = desired
        .iter()
        .map(|child| positions.get(&child.id).copied())
        .collect();
    let kept = crate::order::longest_increasing(&assigned);
    let mut next: Option<web_sys::Node> = None;
    for (index, child) in desired.iter().enumerate().rev() {
        if !kept[index] {
            let _ = parent.insert_before(&child.element, next.as_ref());
        }
        next = Some(child.element.clone().into());
    }
    *current = desired;
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
