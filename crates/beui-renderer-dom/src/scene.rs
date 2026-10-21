use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::rc::Rc;

use wasm_bindgen::JsCast;

use beui_core::color::Color32;
use beui_core::context::{FrameOutput, RendererInfo};
use beui_core::display::{Display, Group, Layer, Part};
use beui_core::geometry::Vec2;
use beui_core::image::{Image, ImageId};
use beui_core::node::NodeId;
use beui_core::painter::Shape;
use beui_core::renderer::Renderer;

use crate::style::{self, Look};

const STYLE: &str = "\
.beui-root{position:relative;overflow:hidden;user-select:none;-webkit-user-select:none;touch-action:none;outline:none}\
.beui-stage{position:absolute;left:0;top:0;width:0;height:0;transform-origin:0 0;pointer-events:none}\
.beui-stage div{position:absolute;left:0;top:0;box-sizing:border-box;white-space:pre;pointer-events:none;font-variant-ligatures:none}\
.beui-stage .beui-layer{isolation:isolate}\
.beui-stage .beui-scaled{transform-origin:0 0}";
const IMAGE_LIMIT: usize = 512;

#[derive(Clone)]
struct Child {
    id: u64,
    element: web_sys::HtmlElement,
}

struct Grouped {
    group: Group,
    factor: f32,
    outer: Child,
    inner: web_sys::HtmlElement,
    children: Vec<Child>,
}

struct Node {
    frame: Child,
    content: web_sys::HtmlElement,
    display: Option<Rc<Display>>,
    factor: f32,
    parent: Option<NodeId>,
    placed: Option<(Vec2, f32)>,
    painted: Vec<Option<Child>>,
    groups: Vec<Option<Grouped>>,
    children: Vec<Child>,
    kids: Vec<NodeId>,
}

struct Root {
    wrap: Child,
    holds: Option<u64>,
}

pub struct DomRenderer {
    document: web_sys::Document,
    root: web_sys::HtmlElement,
    stage: web_sys::HtmlElement,
    next: u64,
    layers: Vec<Child>,
    roots: HashMap<NodeId, Root>,
    groups: Vec<Option<Grouped>>,
    loose: Vec<Option<Child>>,
    nodes: HashMap<NodeId, Node>,
    images: HashMap<ImageId, Rc<str>>,
    prepared: Option<(f32, f32, Color32)>,
    size: Option<(u32, u32)>,
}

struct Shown {
    loose: usize,
    groups: usize,
    roots: HashSet<NodeId>,
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
            next: 0,
            layers: Vec::new(),
            roots: HashMap::new(),
            groups: Vec::new(),
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
        let mut shown = Shown {
            loose: 0,
            groups: 0,
            roots: HashSet::new(),
        };
        let mut groups = std::mem::take(&mut self.groups);
        let desired = self.frame_layers(&output.layers, scale, &mut groups, &mut shown);
        groups.truncate(shown.groups);
        self.groups = groups;
        self.loose.truncate(shown.loose);
        reconcile(&self.stage, &mut self.layers, desired);
        let gone: Vec<NodeId> = self
            .roots
            .keys()
            .filter(|key| !shown.roots.contains(key))
            .copied()
            .collect();
        for key in gone {
            self.roots.remove(&key);
            self.release(key, None);
        }
    }

    fn frame_layers(
        &mut self,
        layers: &[Layer],
        factor: f32,
        groups: &mut Vec<Option<Grouped>>,
        shown: &mut Shown,
    ) -> Vec<Child> {
        let mut desired = Vec::new();
        for layer in layers {
            match layer {
                Layer::Shape(shape) => {
                    if let Some(child) = self.loose(shown.loose, shape, factor) {
                        desired.push(child);
                    }
                    shown.loose += 1;
                }
                Layer::Display(at, display) => {
                    if let Some(wrap) = self.layer(display, *at, factor) {
                        shown.roots.insert(display.node);
                        desired.push(wrap);
                    }
                }
                Layer::Group(group, inner) => {
                    let index = shown.groups;
                    shown.groups += 1;
                    let Some(mut grouped) = self.grouped(groups, index, *group, factor) else {
                        continue;
                    };
                    let within = inner_factor(*group, factor);
                    let wanted = self.frame_layers(inner, within, groups, shown);
                    reconcile(&grouped.inner, &mut grouped.children, wanted);
                    desired.push(grouped.outer.clone());
                    put(groups, index, grouped);
                }
            }
        }
        desired
    }

    fn grouped(
        &mut self,
        groups: &mut [Option<Grouped>],
        index: usize,
        group: Group,
        factor: f32,
    ) -> Option<Grouped> {
        let held = groups.get_mut(index).and_then(Option::take);
        let mut grouped = match held {
            Some(grouped) => grouped,
            None => {
                let outer = self.child()?;
                outer.element.set_class_name("beui-group");
                let inner = div(&self.document).ok()?;
                let _ = outer.element.append_child(&inner);
                Grouped {
                    group,
                    factor: 0.0,
                    outer,
                    inner,
                    children: Vec::new(),
                }
            }
        };
        if grouped.factor != factor || grouped.group != group {
            let (outer, inner) = style::group(group, factor);
            grouped.outer.element.style().set_css_text(&outer);
            grouped.inner.style().set_css_text(&inner);
            let scaled = matches!(group, Group::Transform { scale, .. } if scale != 1.0);
            grouped.inner.set_class_name(match scaled {
                true => "beui-scaled",
                false => "",
            });
            grouped.group = group;
            grouped.factor = factor;
        }
        Some(grouped)
    }

    fn clear(&mut self) {
        self.stage.set_text_content(None);
        self.layers.clear();
        self.roots.clear();
        self.groups.clear();
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
        let reused = self.loose.get_mut(index).and_then(Option::take);
        let child = self.paint(reused, shape, factor);
        match index < self.loose.len() {
            true => self.loose[index] = child.clone(),
            false => self.loose.push(child.clone()),
        }
        child
    }

    fn layer(&mut self, display: &Rc<Display>, at: Vec2, factor: f32) -> Option<Child> {
        let frame = self.visit(display, factor, None)?;
        self.place(display.node, at, factor);
        if !self.roots.contains_key(&display.node) {
            let wrap = self.child()?;
            wrap.element.set_class_name("beui-layer");
            self.roots.insert(display.node, Root { wrap, holds: None });
        }
        let root = self.roots.get_mut(&display.node)?;
        if root.holds != Some(frame.id) {
            root.wrap.element.set_text_content(None);
            let _ = root.wrap.element.append_child(&frame.element);
            root.holds = Some(frame.id);
        }
        Some(root.wrap.clone())
    }

    fn visit(
        &mut self,
        display: &Rc<Display>,
        factor: f32,
        parent: Option<NodeId>,
    ) -> Option<Child> {
        let key = display.node;
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
                    groups: Vec::new(),
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
        let kept = node
            .display
            .as_ref()
            .is_some_and(|held| node.factor == factor && Rc::ptr_eq(&held.shapes, &display.shapes));
        let mut old = std::mem::take(&mut node.painted);
        let mut painted = Vec::with_capacity(display.shapes.len());
        for (index, shape) in display.shapes.iter().enumerate() {
            let previous = old.get_mut(index).and_then(Option::take);
            match previous {
                Some(previous) if kept => painted.push(Some(previous)),
                previous => painted.push(self.paint(previous, shape, factor)),
            }
        }
        let mut kids = Vec::new();
        let mut groups = std::mem::take(&mut node.groups);
        let mut index = 0;
        let desired = self.fill(
            &display.parts,
            display.node,
            factor,
            &painted,
            &mut groups,
            &mut index,
            &mut kids,
        );
        groups.truncate(index);
        node.groups = groups;
        reconcile(&node.content, &mut node.children, desired);
        let shown: HashSet<NodeId> = kids.iter().copied().collect();
        for kid in std::mem::replace(&mut node.kids, kids) {
            if !shown.contains(&kid) {
                self.release(kid, Some(display.node));
            }
        }
        node.painted = painted;
        node.display = Some(Rc::clone(display));
        node.factor = factor;
    }

    #[allow(clippy::too_many_arguments)]
    fn fill(
        &mut self,
        parts: &[Part],
        key: NodeId,
        factor: f32,
        painted: &[Option<Child>],
        groups: &mut Vec<Option<Grouped>>,
        index: &mut usize,
        kids: &mut Vec<NodeId>,
    ) -> Vec<Child> {
        let mut desired = Vec::new();
        for part in parts {
            match part {
                Part::Shapes { start, end, .. } => desired.extend(
                    painted[*start as usize..*end as usize]
                        .iter()
                        .flatten()
                        .cloned(),
                ),
                Part::Child(id, at, child) => {
                    if let Some(frame) = self.visit(child, factor, Some(key)) {
                        self.place(*id, *at, factor);
                        kids.push(*id);
                        desired.push(frame);
                    }
                }
                Part::Group(group, inner) => {
                    let at = *index;
                    *index += 1;
                    let Some(mut grouped) = self.grouped(groups, at, *group, factor) else {
                        continue;
                    };
                    let within = inner_factor(*group, factor);
                    let wanted = self.fill(inner, key, within, painted, groups, index, kids);
                    reconcile(&grouped.inner, &mut grouped.children, wanted);
                    desired.push(grouped.outer.clone());
                    put(groups, at, grouped);
                }
            }
        }
        desired
    }

    fn place(&mut self, key: NodeId, at: Vec2, factor: f32) {
        let Some(node) = self.nodes.get_mut(&key) else {
            return;
        };
        if node.placed == Some((at, factor)) {
            return;
        }
        node.placed = Some((at, factor));
        node.frame
            .element
            .style()
            .set_css_text(&style::placement(at, factor));
    }

    fn release(&mut self, key: NodeId, parent: Option<NodeId>) {
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
                pending.extend(node.kids.into_iter().map(|kid| (kid, Some(key))));
            }
        }
    }

    fn paint(&mut self, reused: Option<Child>, shape: &Shape, factor: f32) -> Option<Child> {
        let image = match shape {
            Shape::Image { image, .. } => self.image(image),
            _ => None,
        };
        let look = style::look(shape, factor, image.as_deref())?;
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

fn inner_factor(group: Group, factor: f32) -> f32 {
    match group {
        Group::Transform { scale, .. } => factor * scale,
        _ => factor,
    }
}

fn put(groups: &mut Vec<Option<Grouped>>, index: usize, grouped: Grouped) {
    if groups.len() <= index {
        groups.resize_with(index + 1, || None);
    }
    groups[index] = Some(grouped);
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
    let pixels = js_sys::Uint8ClampedArray::new_with_length(image.pixels().len() as u32);
    pixels.copy_from(image.pixels());
    let data = web_sys::ImageData::new_with_js_u8_clamped_array_and_sh(
        &pixels,
        image.width(),
        image.height(),
    )
    .ok()?;
    context.put_image_data(&data, 0.0, 0.0).ok()?;
    canvas.to_data_url().ok()
}
