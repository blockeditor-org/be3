#[cfg(test)]
extern crate beui_view as beui;

use be_lib_stable_hash::HashMap;
use std::rc::Rc;

use be_lib_layers::LayerId;
use beui_core::context::{Moved, WindowOutput};
use beui_core::display::{Display, Group, Layer, Part, bounds_of};
use beui_core::draw::{Quad, Turn, push_quads};
use beui_core::drawing::Drawing;
use beui_core::fade::Fade;
use beui_core::filter::Filter;
use beui_core::font::{GlyphId, GlyphImage};
use beui_core::painter::{Shape, placed_shape};
use beui_draw::compose::Placed;
use beui_draw::{Draw, DrawAt};
use beui_types::Color32;
use beui_types::geometry::Region;
use beui_types::{ImageId, PathStyle};
use beui_types::{Pos2, Rect, Vec2};
use beui_vello::vello_common::kurbo::Affine;

mod composite;
pub mod filter;
mod group;
pub mod present;
mod resources;
mod scene;

pub use resources::{Lost, Resources};
use scene::{Item, Painted};

#[cfg(all(target_arch = "wasm32", feature = "web"))]
pub mod canvas;
#[cfg(not(target_arch = "wasm32"))]
pub mod offscreen;
#[cfg(not(target_arch = "wasm32"))]
pub mod window;

use beui_core::context::RendererInfo;

static RENDERERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn renderer_info(adapter: &wgpu::AdapterInfo, format: wgpu::TextureFormat) -> RendererInfo {
    let mut rows = vec![
        ("Backend", format!("{:?}", adapter.backend)),
        ("Adapter", adapter.name.clone()),
        ("Device type", format!("{:?}", adapter.device_type)),
        ("Driver", or_unknown(&adapter.driver)),
        ("Driver info", or_unknown(&adapter.driver_info)),
        (
            "Vendor / device",
            format!("{:#06x} / {:#06x}", adapter.vendor, adapter.device),
        ),
    ];
    if !adapter.device_pci_bus_id.is_empty() {
        rows.push(("PCI bus", adapter.device_pci_bus_id.clone()));
    }
    rows.push(("Surface format", format!("{format:?}")));
    RendererInfo { rows }
}

fn or_unknown(value: &str) -> String {
    if value.is_empty() {
        "unknown".to_owned()
    } else {
        value.to_owned()
    }
}

const ATLAS_SIZE: u32 = 2048;
const GLYPH_PADDING: u32 = 1;

/// The formats a texture beui draws on must also be viewed as: Vello writes sRGB-encoded values
/// itself, so it draws into the non-sRGB view of an sRGB texture. Give these as the texture's
/// (or surface configuration's) `view_formats`.
pub fn view_formats(format: wgpu::TextureFormat) -> &'static [wgpu::TextureFormat] {
    match format {
        wgpu::TextureFormat::Rgba8UnormSrgb => &[wgpu::TextureFormat::Rgba8Unorm],
        wgpu::TextureFormat::Bgra8UnormSrgb => &[wgpu::TextureFormat::Bgra8Unorm],
        _ => &[],
    }
}

/// The view of `view`'s texture that Vello draws into: the texture's non-sRGB format, which
/// its `view_formats` must list (`view_formats`). A target is always the first level and layer
/// of its texture (a Space panel's mips are drawn from it afterwards).
pub(crate) fn drawn_view(view: &wgpu::TextureView) -> wgpu::TextureView {
    let texture = view.texture();
    let format = texture.format();
    match format.is_srgb() {
        false => view.clone(),
        true => texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("beui drawn view"),
            format: Some(format.remove_srgb_suffix()),
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(1),
            ..Default::default()
        }),
    }
}

/// Glyphs uploaded as Vello samples them: premultiplied RGBA8, a coverage glyph white with its
/// coverage in every channel.
pub(crate) struct Atlas {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    entries: HashMap<GlyphId, [u32; 4]>,
    row_y: u32,
    row_height: u32,
    cursor_x: u32,
    pub(crate) full: bool,
}

impl Atlas {
    pub(crate) fn new(device: &wgpu::Device, label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            entries: HashMap::default(),
            row_y: 0,
            row_height: 0,
            cursor_x: 0,
            full: false,
        }
    }

    /// Where the glyph lies, in texels (left, top, width, height), uploading it if it is new.
    pub(crate) fn insert(
        &mut self,
        queue: &wgpu::Queue,
        id: GlyphId,
        image: &GlyphImage,
    ) -> Option<[u32; 4]> {
        if let Some(region) = self.entries.get(&id) {
            return Some(*region);
        }
        let texel = match image.color {
            true => 4,
            false => 1,
        };
        let texels = image.width as usize * image.height as usize;
        if image.pixels.len() < texels * texel {
            return None;
        }
        let width = image.width + GLYPH_PADDING;
        let height = image.height + GLYPH_PADDING;
        if width > ATLAS_SIZE || height > ATLAS_SIZE {
            return None;
        }
        if self.cursor_x + width > ATLAS_SIZE {
            self.row_y += self.row_height;
            self.row_height = 0;
            self.cursor_x = 0;
        }
        if self.row_y + height > ATLAS_SIZE {
            self.full = true;
            return None;
        }

        let x = self.cursor_x;
        let y = self.row_y;
        self.cursor_x += width;
        self.row_height = self.row_height.max(height);

        let pixels = match image.color {
            true => resources::premultiplied(&image.pixels[..texels * 4]),
            false => image.pixels[..texels]
                .iter()
                .flat_map(|coverage| [*coverage; 4])
                .collect(),
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
        );
        let region = [x, y, image.width, image.height];
        self.entries.insert(id, region);
        Some(region)
    }
}

#[derive(Clone, Copy)]
pub enum Repaint {
    Everything,
    Region { region: Region, background: Color32 },
}

impl Repaint {
    pub fn union(self, other: Self) -> Self {
        match (self, other) {
            (
                Self::Region { region, .. },
                Self::Region {
                    region: added,
                    background,
                },
            ) => Self::Region {
                region: region.union(added),
                background,
            },
            _ => Self::Everything,
        }
    }
}

pub trait Repainting {
    fn repaint(&self, background: Color32) -> Repaint;

    fn repaint_moving(&self, background: Color32) -> (Repaint, Option<Moved>);
}

impl Repainting for WindowOutput {
    fn repaint(&self, background: Color32) -> Repaint {
        let region = self.unmoved();
        match region.is_empty() {
            true => Repaint::Everything,
            false => Repaint::Region { region, background },
        }
    }

    fn repaint_moving(&self, background: Color32) -> (Repaint, Option<Moved>) {
        match (self.moved, self.damage.is_empty()) {
            (None, _) | (Some(_), true) => (self.repaint(background), None),
            (Some(moved), false) => (
                Repaint::Region {
                    region: self.damage,
                    background,
                },
                Some(moved),
            ),
        }
    }
}

/// Where a run of items is placed: its pixels' translation in the document, the bound it is
/// clipped to, the fade over it, and how strongly a punch in it erases.
#[derive(Clone, Copy)]
pub(crate) struct Space {
    translation: [f32; 2],
    opacity: f32,
    clip: [f32; 4],
    fade: [f32; 4],
    widths: [f32; 4],
}

const OPEN: [f32; 4] = [-1.0e9, -1.0e9, 1.0e9, 1.0e9];
const LISTED: usize = 4096;
const REACH: f32 = 2.0;
/// Frames drawn with no group before the group layers are freed.
const IDLE_GROUP_FRAMES: u32 = 120;

pub struct Renderer {
    id: u64,
    resources: Resources,
    format: wgpu::TextureFormat,
    lists: HashMap<ListKey, Encoded>,
    drawn: be_lib_stable_hash::HashSet<ListKey>,
    frames: u64,
    live: usize,
    pub encoded: usize,
    steps: Vec<Step>,
    spaces: Vec<Space>,
    scissors: Option<Vec<[u32; 4]>>,
    origin: Vec2,
    held: be_lib_stable_hash::HashSet<ImageId>,
    using: be_lib_stable_hash::HashSet<ImageId>,
    /// The image levels this frame's items sample.
    levels: Vec<(ImageId, u32)>,
    textures: scene::Textures,
    filters: Vec<filter::Prepared>,
    effects: Vec<filter::Effects>,
    opacities: Vec<f32>,
    groups: Option<group::Groups>,
    idle_groups: u32,
    target: (u32, u32),
    bounds: Option<[u32; 4]>,
    whole: bool,
    scratch: Option<wgpu::Texture>,
    /// Whether this renderer draws a window, which is composited while the program composites
    /// windows (`beui_draw::compose`).
    window: bool,
    composition: Option<composite::Composition>,
    /// Runs that erase each damaged region, which clear the upper root there.
    cleared: Vec<Run>,
    vello: Option<Vello>,
    /// The filter scene the scene being drawn blurs, while it is drawn.
    filtered: Option<wgpu::TextureView>,
}

/// Vello's renderer, made for the format it draws on when it first draws.
struct Vello {
    renderer: Option<(vello_gpu::Renderer, vello_gpu::Resources)>,
    format: wgpu::TextureFormat,
    scene: vello_gpu::Scene,
    /// How many scenes it rendered since the encoder they went into was submitted: Vello's
    /// buffers hold one scene, so a second is rendered only after the first was submitted.
    rendered: usize,
}

#[derive(Clone)]
enum Run {
    Items { space: u32, items: Vec<Item> },
    Drawing(Drawing, DrawAt),
}

struct Emitted {
    background: Vec<Run>,
    cleared: Vec<Run>,
    filters: usize,
}

enum Step {
    Runs(Vec<Run>),
    Filter {
        index: usize,
        bottom: bool,
        steps: Vec<Step>,
    },
    Group {
        index: usize,
        /// Where the group can paint, in physical pixels of the document.
        bounds: [f32; 4],
        steps: Vec<Step>,
    },
}

impl Step {
    fn depth(steps: &[Self]) -> usize {
        steps
            .iter()
            .map(|step| match step {
                Self::Runs(_) => 0,
                Self::Filter { steps, .. } => Self::depth(steps),
                Self::Group { steps, .. } => 1 + Self::depth(steps),
            })
            .max()
            .unwrap_or(0)
    }

    /// Whether the steps hold what a Vello layer cannot: a drawing, painted in a pass of its
    /// own, or a filter, which reads what is drawn beneath it.
    fn splits(steps: &[Self]) -> bool {
        steps.iter().any(|step| match step {
            Self::Runs(runs) => runs.iter().any(|run| matches!(run, Run::Drawing(..))),
            Self::Filter { .. } => true,
            Self::Group { steps, .. } => Self::splits(steps),
        })
    }
}

fn push_items(steps: &mut Vec<Step>, space: u32, items: &[Item]) {
    if items.is_empty() {
        return;
    }
    if let Some(Step::Runs(runs)) = steps.last_mut() {
        Run::extend(runs, space, items);
        return;
    }
    steps.push(Step::Runs(vec![Run::Items {
        space,
        items: items.to_vec(),
    }]));
}

fn push_run(steps: &mut Vec<Step>, run: Run) {
    match run {
        Run::Items { space, items } => push_items(steps, space, &items),
        run => match steps.last_mut() {
            Some(Step::Runs(runs)) => runs.push(run),
            _ => steps.push(Step::Runs(vec![run])),
        },
    }
}

impl Run {
    fn extend(runs: &mut Vec<Self>, space: u32, items: &[Item]) {
        if let Some(Self::Items {
            space: held,
            items: holding,
        }) = runs.last_mut()
            && *held == space
        {
            holding.extend_from_slice(items);
            return;
        }
        runs.push(Self::Items {
            space,
            items: items.to_vec(),
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ListKey {
    display: u64,
    factor: u32,
    offset: [u32; 2],
    clip: [u32; 4],
}

struct Encoded {
    items: Vec<Item>,
    runs: Vec<ListRun>,
    parts: Vec<(usize, usize)>,
    atlas: u64,
    seen: u64,
}

enum ListRun {
    Items {
        start: usize,
        count: usize,
    },
    Drawing {
        drawing: Drawing,
        rect: [f32; 4],
        clip: [f32; 4],
    },
}

enum Encoding {
    Item(Item),
    Drawing {
        drawing: Drawing,
        rect: [f32; 4],
        clip: [f32; 4],
    },
}

#[derive(Clone, Copy)]
struct Walk {
    factor: f32,
    space: u32,
    origin: [f32; 2],
    bound: [f32; 4],
    offset: Vec2,
    clip: Rect,
    fade: Fade,
}

impl Walk {
    fn key(&self, display: &Display) -> ListKey {
        ListKey {
            display: display.key,
            factor: self.factor.to_bits(),
            offset: [self.offset.x.to_bits(), self.offset.y.to_bits()],
            clip: [
                self.clip.min.x.to_bits(),
                self.clip.min.y.to_bits(),
                self.clip.max.x.to_bits(),
                self.clip.max.y.to_bits(),
            ],
        }
    }

    fn reach(&self, display: &Display) -> [f32; 4] {
        self.reach_of(display.bounds)
    }

    fn reach_of(&self, bounds: Rect) -> [f32; 4] {
        let visible = bounds.translate(self.offset).intersect(self.clip);
        let [left, top, right, bottom] = pixels(visible, self.factor, self.origin);
        overlap(
            [left - REACH, top - REACH, right + REACH, bottom + REACH],
            self.bound,
        )
    }

    fn child(&self, at: Vec2) -> Self {
        Self {
            offset: self.offset + at,
            ..*self
        }
    }

    fn group(
        &self,
        group: &Group,
        space: &mut impl FnMut([f32; 2], [f32; 4], Fade) -> u32,
    ) -> Self {
        match *group {
            Group::Clip(clip) => Self {
                clip: self.clip.intersect(clip.translate(self.offset)),
                ..*self
            },
            Group::Scroll {
                viewport,
                offset,
                scale,
                ..
            } => {
                let clip = self.clip.intersect(viewport.translate(self.offset));
                let at = self.offset + offset;
                let origin = [
                    self.origin[0] + (at.x * self.factor).round(),
                    self.origin[1] + (at.y * self.factor).round(),
                ];
                let bound = overlap(pixels(clip, self.factor, self.origin), self.bound);
                let scale = match scale > 0.0 && scale.is_finite() {
                    true => scale,
                    false => 1.0,
                };
                Self {
                    factor: self.factor * scale,
                    space: space(origin, bound, self.fade),
                    origin,
                    bound,
                    offset: Vec2::ZERO,
                    clip: Rect::EVERYTHING,
                    fade: self.fade,
                }
            }
            Group::Mask { fade, .. } => {
                let fade = self.fade.within(
                    fade.translate(self.offset)
                        .scaled(self.factor, Vec2::new(self.origin[0], self.origin[1])),
                );
                Self {
                    space: space(self.origin, self.bound, fade),
                    fade,
                    ..*self
                }
            }
            Group::Transform { offset, scale } if scale > 0.0 && scale.is_finite() => Self {
                factor: self.factor * scale,
                offset: (self.offset + offset) * scale.recip(),
                clip: self.clip.scaled(scale.recip()),
                ..*self
            },
            Group::Transform { .. } | Group::Filter(_) => *self,
        }
    }
}

struct Frame<'a> {
    queue: &'a wgpu::Queue,
    damaged: Option<&'a [[f32; 4]]>,
    screen: Vec2,
    pixels_per_point: f32,
    spaces: Vec<Space>,
    slots: HashMap<[u32; 15], u32>,
    drawings: Vec<(Drawing, DrawAt)>,
    opacities: Vec<f32>,
    /// Whether the frame is composited (`DrawAt::composited`).
    composited: bool,
    /// What is visited damaged or not.
    forced: &'a Forced,
}

/// What a composited frame visits whether or not it is damaged: each drawing that placed a
/// composited layer (an anchored view) last frame, and the displays that hold one, so the roots
/// split where it paints.
#[derive(Default)]
pub(crate) struct Forced {
    displays: be_lib_stable_hash::HashSet<u64>,
    drawings: be_lib_stable_hash::HashSet<usize>,
}

impl Forced {
    fn drawing(&self, drawing: &Drawing) -> bool {
        !self.drawings.is_empty() && self.drawings.contains(&drawing.identity().0)
    }
}

impl Frame<'_> {
    fn space(&mut self, origin: [f32; 2], bound: [f32; 4], fade: Fade) -> u32 {
        self.faded_space(origin, bound, fade, 1.0)
    }

    fn faded_space(&mut self, origin: [f32; 2], bound: [f32; 4], fade: Fade, opacity: f32) -> u32 {
        let bound = bound.map(|value| value.clamp(OPEN[0], OPEN[2]));
        let rect = [
            fade.rect.min.x,
            fade.rect.min.y,
            fade.rect.max.x,
            fade.rect.max.y,
        ]
        .map(|value| value.clamp(OPEN[0], OPEN[2]));
        let mut key = [0; 15];
        for (slot, value) in key.iter_mut().zip(
            origin
                .iter()
                .chain(&bound)
                .chain(&rect)
                .chain(&fade.widths)
                .chain([&opacity]),
        ) {
            *slot = value.to_bits();
        }
        if let Some(slot) = self.slots.get(&key) {
            return *slot;
        }
        let slot = self.spaces.len() as u32;
        self.spaces.push(Space {
            translation: origin,
            opacity,
            clip: bound,
            fade: rect,
            widths: fade.widths,
        });
        self.slots.insert(key, slot);
        slot
    }

    /// Wraps steps drawn inside a group at `opacity` over `bounds`, unless it has
    /// nothing to draw. A punch in the group erases its layer, and also the target
    /// beneath at the group's opacity, as it would if the group were drawn in place.
    fn group(&mut self, opacity: f32, bounds: [f32; 4], steps: Vec<Step>, into: &mut Vec<Step>) {
        if steps.is_empty() || opacity <= 0.0 {
            return;
        }
        if opacity >= 1.0 {
            into.extend(steps);
            return;
        }
        for step in &steps {
            let Step::Runs(runs) = step else {
                continue;
            };
            for run in runs {
                let Run::Items { space, items } = run else {
                    continue;
                };
                let punches: Vec<Item> = items
                    .iter()
                    .filter(|item| matches!(item, Item::Punch { .. }))
                    .cloned()
                    .collect();
                if punches.is_empty() {
                    continue;
                }
                let held = self.spaces[*space as usize];
                let fade = Fade::new(
                    Rect::from_min_max(
                        Pos2::new(held.fade[0], held.fade[1]),
                        Pos2::new(held.fade[2], held.fade[3]),
                    ),
                    held.widths,
                );
                let space =
                    self.faded_space(held.translation, held.clip, fade, held.opacity * opacity);
                push_items(into, space, &punches);
            }
        }
        into.push(Step::Group {
            index: self.opacities.len(),
            bounds,
            steps,
        });
        self.opacities.push(opacity);
    }

    fn damages(&self, reach: [f32; 4]) -> bool {
        reach[0] < reach[2]
            && reach[1] < reach[3]
            && self
                .damaged
                .is_none_or(|damaged| damaged.iter().any(|damaged| overlaps(*damaged, reach)))
    }
}

fn pixels(rect: Rect, factor: f32, origin: [f32; 2]) -> [f32; 4] {
    [
        (rect.min.x * factor).round() + origin[0],
        (rect.min.y * factor).round() + origin[1],
        (rect.max.x * factor).round() + origin[0],
        (rect.max.y * factor).round() + origin[1],
    ]
}

fn overlap(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    [
        left[0].max(right[0]),
        left[1].max(right[1]),
        left[2].min(right[2]),
        left[3].min(right[3]),
    ]
}

fn shows(reach: [f32; 4]) -> bool {
    reach[0] < reach[2] && reach[1] < reach[3]
}

fn overlaps(left: [f32; 4], right: [f32; 4]) -> bool {
    let [x0, y0, x1, y1] = overlap(left, right);
    x0 < x1 && y0 < y1
}

fn at(texture: &wgpu::Texture, x: f32, y: f32) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d {
            x: x as u32,
            y: y as u32,
            z: 0,
        },
        aspect: wgpu::TextureAspect::All,
    }
}

fn translated(rect: [f32; 4], origin: [f32; 2]) -> [f32; 4] {
    [
        rect[0] + origin[0],
        rect[1] + origin[1],
        rect[2] + origin[0],
        rect[3] + origin[1],
    ]
}

impl Renderer {
    pub fn new(resources: &Resources) -> Self {
        let format = resources.0.borrow().format;
        Self {
            id: RENDERERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            resources: resources.clone(),
            format,
            lists: HashMap::default(),
            drawn: be_lib_stable_hash::HashSet::default(),
            frames: 0,
            live: 0,
            encoded: 0,
            steps: Vec::new(),
            spaces: Vec::new(),
            scissors: None,
            origin: Vec2::ZERO,
            held: be_lib_stable_hash::HashSet::default(),
            using: be_lib_stable_hash::HashSet::default(),
            levels: Vec::new(),
            textures: scene::Textures::default(),
            filters: Vec::new(),
            effects: Vec::new(),
            opacities: Vec::new(),
            groups: None,
            idle_groups: 0,
            target: (0, 0),
            bounds: None,
            whole: true,
            scratch: None,
            window: false,
            composition: None,
            cleared: Vec::new(),
            vello: None,
            filtered: None,
        }
    }

    /// Marks this renderer as drawing a window, which is composited while the program
    /// composites windows (`beui_draw::compose::composite_windows`).
    pub fn set_window(&mut self, window: bool) {
        self.window = window;
    }

    /// The layers this renderer composited its last frame from, in the window's points, when it
    /// composites.
    pub fn layers(&self) -> Option<&be_lib_layers::WindowLayers> {
        Some(self.composition.as_ref()?.list())
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }


    fn upload(&mut self, queue: &wgpu::Queue, item: &Item) {
        let Some((image, level)) = item.picture() else {
            return;
        };
        self.resources.0.borrow_mut().upload(queue, image);
        self.using.insert(image.id());
        if !self.levels.contains(&(image.id(), level)) {
            self.levels.push((image.id(), level));
        }
    }

    fn settle_pictures(&mut self) {
        let mut shared = self.resources.0.borrow_mut();
        for id in self.using.difference(&self.held) {
            shared.hold(*id);
        }
        for id in self.held.difference(&self.using) {
            shared.release(*id);
        }
        self.textures = scene::Textures::of(&shared, &self.levels);
        drop(shared);
        self.levels.clear();
        self.held = std::mem::take(&mut self.using);
    }

    pub fn shift(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        moved: Moved,
        pixels_per_point: f32,
    ) {
        let size = texture.size();
        let whole = [0.0, 0.0, size.width as f32, size.height as f32];
        let by = [
            (moved.by.x * pixels_per_point).round(),
            (moved.by.y * pixels_per_point).round(),
        ];
        let mut from = overlap(
            translated(
                pixels(moved.from, pixels_per_point, [0.0; 2]),
                [-self.origin.x, -self.origin.y],
            ),
            whole,
        );
        from = overlap(from, translated(whole, [-by[0], -by[1]]));
        if let Some([left, top, width, height]) = self.bounds {
            let bounds = [
                left as f32,
                top as f32,
                (left + width) as f32,
                (top + height) as f32,
            ];
            from = overlap(overlap(from, bounds), translated(bounds, [-by[0], -by[1]]));
        }
        let [left, top, right, bottom] = from;
        if left >= right || top >= bottom {
            return;
        }
        let extent = wgpu::Extent3d {
            width: (right - left) as u32,
            height: (bottom - top) as u32,
            depth_or_array_layers: 1,
        };
        let scratch = match &self.scratch {
            Some(scratch)
                if scratch.format() == texture.format()
                    && scratch.width() >= extent.width
                    && scratch.height() >= extent.height =>
            {
                scratch
            }
            _ => self
                .scratch
                .insert(device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("beui moved region"),
                    size: wgpu::Extent3d {
                        width: size.width,
                        height: size.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: texture.format(),
                    usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                })),
        };
        // A composited window's roots hold what its target shows, so they move with it.
        let roots = self
            .composition
            .iter()
            .flat_map(|composition| composition.roots());
        for texture in std::iter::once(texture).chain(roots) {
            encoder.copy_texture_to_texture(at(texture, left, top), at(scratch, 0.0, 0.0), extent);
            encoder.copy_texture_to_texture(
                at(scratch, 0.0, 0.0),
                at(texture, left + by[0], top + by[1]),
                extent,
            );
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn set_origin(&mut self, origin: Vec2) {
        self.origin = origin;
    }

    pub fn set_bounds(&mut self, bounds: Option<[u32; 4]>) {
        self.bounds = bounds;
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &WindowOutput,
        screen: Vec2,
        pixels_per_point: f32,
        repaint: Repaint,
    ) -> Repaint {
        let composited = self.window && beui_draw::compose::compositing();
        let mut repaint = repaint;
        match composited {
            true => {
                let size = (screen.x.max(0.0) as u32, screen.y.max(0.0) as u32);
                if self
                    .composition
                    .as_ref()
                    .is_none_or(|composition| !composition.fits(self.format, size))
                {
                    self.composition = Some(composite::Composition::new(
                        device,
                        queue,
                        self.format,
                        size,
                    ));
                    repaint = Repaint::Everything;
                }
            }
            false => self.composition = None,
        }
        {
            let mut shared = self.resources.0.borrow_mut();
            if shared.full() {
                shared.renew();
            }
        }

        let prepared: Vec<filter::Prepared> = output
            .filters()
            .iter()
            .map(|filter: &Filter| filter::Prepared::new(filter, pixels_per_point, screen))
            .collect();
        let mut restyled = Vec::new();
        for index in 0..prepared.len().max(self.filters.len()) {
            let (now, was) = (prepared.get(index), self.filters.get(index));
            if now != was {
                let origin = [self.origin.x, self.origin.y];
                restyled.extend(
                    [now, was]
                        .into_iter()
                        .flatten()
                        .map(|filter| translated(filter.region(), origin)),
                );
            }
        }
        let damaged: Option<Vec<[f32; 4]>> = match repaint {
            Repaint::Everything => None,
            Repaint::Region { region, .. } => {
                let regions = region
                    .rects()
                    .iter()
                    .map(|rect| physical(*rect, self.origin, screen, pixels_per_point))
                    .chain(restyled);
                Some(
                    regions
                        .map(|region: [f32; 4]| {
                            prepared
                                .iter()
                                .fold(region, |region, prepared| prepared.widen(region))
                        })
                        .collect(),
                )
            }
        };
        // A composited window lists every drawing it shows, from what they placed last frame
        // where nothing changed (`place`), and visits those that placed an anchored view.
        let (found, forced) = match composited {
            true => self.found(output, screen, pixels_per_point),
            false => (Vec::new(), Forced::default()),
        };
        let mut frame = Frame {
            queue,
            damaged: damaged.as_deref(),
            screen,
            pixels_per_point,
            spaces: Vec::new(),
            slots: HashMap::default(),
            drawings: Vec::new(),
            opacities: Vec::new(),
            composited,
            forced: &forced,
        };
        frame.space([0.0; 2], OPEN, Fade::NONE);
        let mut steps = self.emit(&mut frame, output, repaint);
        self.record_drawings(device, queue, &frame.drawings);
        let placed = match composited {
            true => self.place(
                device,
                queue,
                found,
                &frame.drawings,
                &mut steps,
                damaged.is_none(),
            ),
            false => Some(Vec::new()),
        };
        let Some(placed) = placed else {
            // The first anchored view, where the upper root begins, is another than last
            // frame's, so what the UI paints after it moves between the roots: the frame is
            // prepared again, painted whole. That frame splits wherever it finds, so this
            // happens at most once a frame.
            self.release_drawings();
            return self.prepare(
                device,
                queue,
                output,
                screen,
                pixels_per_point,
                Repaint::Everything,
            );
        };
        self.release_drawings();
        if self.lists.len() > 2 * self.live + LISTED {
            self.sweep(output, pixels_per_point);
        }

        self.opacities = std::mem::take(&mut frame.opacities);
        self.spaces = std::mem::take(&mut frame.spaces);
        self.settle_pictures();
        let origin = self.origin;
        self.whole = damaged.is_none();
        let scissors = damaged.map(|damaged| {
            damaged
                .into_iter()
                .map(|damaged| scissor(damaged, origin))
                .collect::<Vec<_>>()
        });
        self.scissors = match (scissors, self.bounds) {
            (scissors, None) => scissors,
            (None, Some(bounds)) => Some(vec![bounds]),
            (Some(scissors), Some(bounds)) => Some(
                scissors
                    .into_iter()
                    .map(|scissor| intersected(scissor, bounds))
                    .collect(),
            ),
        };
        self.steps = steps;
        self.filters = prepared;
        if let Some(composition) = &mut self.composition {
            let background = match repaint {
                Repaint::Region { background, .. } => Some(background),
                Repaint::Everything => None,
            };
            composition.take(
                placed,
                pixels_per_point,
                origin,
                self.scissors.as_deref(),
                background,
            );
        }
        repaint
    }

    fn emit(&mut self, frame: &mut Frame, output: &WindowOutput, repaint: Repaint) -> Vec<Step> {
        let mut runs = Vec::new();
        let mut cleared = Vec::new();
        for region in frame.damaged.unwrap_or_default() {
            let punch = Item::Punch {
                rect: *region,
                clip: *region,
                radius: 0.0,
                turn: Turn::NONE,
            };
            Run::extend(&mut cleared, 0, std::slice::from_ref(&punch));
            if let Repaint::Region { background, .. } = repaint {
                let [.., alpha] = background.to_array();
                if alpha < u8::MAX {
                    Run::extend(&mut runs, 0, std::slice::from_ref(&punch));
                }
                if alpha > 0 {
                    Run::extend(
                        &mut runs,
                        0,
                        &[Item::Rect {
                            rect: *region,
                            clip: *region,
                            color: background.to_array(),
                            radii: [0.0; 4],
                            stroke: 0.0,
                            turn: Turn::NONE,
                        }],
                    );
                }
            }
        }
        let mut steps = vec![Step::Runs(runs.clone())];
        let at = Walk {
            factor: frame.pixels_per_point,
            space: 0,
            origin: [0.0; 2],
            bound: OPEN,
            offset: Vec2::ZERO,
            clip: Rect::EVERYTHING,
            fade: Fade::NONE,
        };
        self.cleared.clone_from(&cleared);
        let mut emitted = Emitted {
            background: runs,
            cleared,
            filters: 0,
        };
        self.emit_layers(frame, &output.layers, at, &mut emitted, &mut steps, true);
        steps
    }

    fn emit_layers(
        &mut self,
        frame: &mut Frame,
        layers: &[Layer],
        at: Walk,
        emitted: &mut Emitted,
        steps: &mut Vec<Step>,
        bottom: bool,
    ) {
        let mut encodings = Vec::new();
        for (index, layer) in layers.iter().enumerate() {
            let bottom = bottom && index == 0;
            match layer {
                Layer::Shape(shape) => {
                    let shape = placed_shape(shape, at.offset, at.clip);
                    let damaged = match &shape {
                        Shape::Drawing { drawing, .. } if frame.forced.drawing(drawing) => None,
                        _ => frame.damaged,
                    };
                    encodings.clear();
                    self.encode_shape(frame.queue, &shape, at.factor, damaged, &mut encodings);
                    for encoding in encodings.drain(..) {
                        match encoding {
                            Encoding::Item(item) => {
                                self.upload(frame.queue, &item);
                                push_items(steps, at.space, std::slice::from_ref(&item));
                            }
                            Encoding::Drawing {
                                drawing,
                                rect,
                                clip,
                            } => {
                                if let Some(run) = self.push_drawing(
                                    frame,
                                    drawing,
                                    translated(rect, at.origin),
                                    translated(clip, at.origin),
                                    at.bound,
                                ) {
                                    push_run(steps, run);
                                }
                            }
                        }
                    }
                }
                Layer::Display(translation, display) => {
                    self.walk(frame, display, at.child(*translation), steps);
                }
                Layer::Group(Group::Filter(_), inner) => {
                    let index = emitted.filters;
                    emitted.filters += 1;
                    let beneath = match bottom {
                        true => emitted.background.clone(),
                        false => emitted.cleared.clone(),
                    };
                    let mut filtered = vec![Step::Runs(beneath)];
                    self.emit_layers(frame, inner, at, emitted, &mut filtered, bottom);
                    steps.push(Step::Filter {
                        index,
                        bottom,
                        steps: filtered,
                    });
                }
                Layer::Group(group, inner) => {
                    let within = at.group(group, &mut |origin, bound, fade| {
                        frame.space(origin, bound, fade)
                    });
                    match *group {
                        Group::Mask { opacity, .. } if opacity < 1.0 => {
                            let mut grouped = Vec::new();
                            self.emit_layers(frame, inner, within, emitted, &mut grouped, false);
                            let bounds = at.reach_of(layer.bounds());
                            frame.group(opacity, bounds, grouped, steps);
                        }
                        _ => self.emit_layers(frame, inner, within, emitted, steps, false),
                    }
                }
            }
        }
    }

    fn walk(&mut self, frame: &mut Frame, display: &Display, at: Walk, steps: &mut Vec<Step>) {
        let reach = at.reach(display);
        let forced = frame.forced.displays.contains(&display.key) && shows(reach);
        if !(forced || frame.damages(reach)) {
            return;
        }
        let key = at.key(display);
        let mut encoded = match self.lists.remove(&key) {
            Some(encoded) if encoded.atlas == self.resources.0.borrow().epoch => encoded,
            _ => self.encode_list(frame.queue, display, &at),
        };
        encoded.seen = self.frames;
        if encoded
            .runs
            .iter()
            .any(|run| matches!(run, ListRun::Drawing { .. }))
        {
            self.drawn.insert(key);
        }
        let mut shapes = 0;
        self.walk_parts(frame, &display.parts, at, &encoded, &mut shapes, steps);
        self.lists.insert(key, encoded);
    }

    fn walk_parts(
        &mut self,
        frame: &mut Frame,
        parts: &[Part],
        at: Walk,
        encoded: &Encoded,
        shapes: &mut usize,
        steps: &mut Vec<Step>,
    ) {
        for part in parts {
            match part {
                Part::Shapes { .. } => {
                    let (from, to) = encoded.parts[*shapes];
                    *shapes += 1;
                    for run in &encoded.runs[from..to] {
                        match run {
                            ListRun::Items { start, count } => {
                                let items = &encoded.items[*start..*start + *count];
                                for item in items {
                                    self.upload(frame.queue, item);
                                }
                                push_items(steps, at.space, items);
                            }
                            ListRun::Drawing {
                                drawing,
                                rect,
                                clip,
                            } => {
                                if let Some(run) = self.push_drawing(
                                    frame,
                                    drawing.clone(),
                                    translated(*rect, at.origin),
                                    translated(*clip, at.origin),
                                    at.bound,
                                ) {
                                    push_run(steps, run);
                                }
                            }
                        }
                    }
                }
                Part::Child(_, translation, child) => {
                    self.walk(frame, child, at.child(*translation), steps);
                }
                Part::Group(group, inner) => {
                    let within = at.group(group, &mut |origin, bound, fade| {
                        frame.space(origin, bound, fade)
                    });
                    match *group {
                        Group::Mask { opacity, .. } if opacity < 1.0 => {
                            let mut grouped = Vec::new();
                            self.walk_parts(frame, inner, within, encoded, shapes, &mut grouped);
                            let bounds = at.reach_of(bounds_of(inner));
                            frame.group(opacity, bounds, grouped, steps);
                        }
                        _ => self.walk_parts(frame, inner, within, encoded, shapes, steps),
                    }
                }
            }
        }
    }

    fn push_drawing(
        &self,
        frame: &mut Frame,
        drawing: Drawing,
        rect: [f32; 4],
        clip: [f32; 4],
        bound: [f32; 4],
    ) -> Option<Run> {
        let clip = overlap(clip, bound);
        let reach = overlap(rect, clip);
        if !(frame.forced.drawing(&drawing) && shows(reach) || frame.damages(reach)) {
            return None;
        }
        let at = self.draw_at(
            rect,
            clip,
            frame.screen,
            frame.pixels_per_point,
            frame.composited,
        );
        frame.drawings.push((drawing.clone(), at));
        Some(Run::Drawing(drawing, at))
    }

    fn encode_list(&mut self, queue: &wgpu::Queue, display: &Display, at: &Walk) -> Encoded {
        let mut encoded = Encoded {
            items: Vec::new(),
            runs: Vec::new(),
            parts: Vec::new(),
            atlas: self.resources.0.borrow().epoch,
            seen: self.frames,
        };
        self.encode_parts(queue, display, &display.parts, *at, &mut encoded);
        self.encoded += encoded.items.len();
        encoded
    }

    fn encode_parts(
        &mut self,
        queue: &wgpu::Queue,
        display: &Display,
        parts: &[Part],
        at: Walk,
        encoded: &mut Encoded,
    ) {
        let mut encodings = Vec::new();
        for part in parts {
            match part {
                Part::Child(..) => {}
                Part::Group(group, inner) => {
                    let within = at.group(group, &mut |_, _, _| 0);
                    self.encode_parts(queue, display, inner, within, encoded);
                }
                Part::Shapes { start, end, .. } => {
                    let from = encoded.runs.len();
                    for shape in &display.shapes[*start as usize..*end as usize] {
                        let shape = placed_shape(shape, at.offset, at.clip);
                        encodings.clear();
                        self.encode_shape(queue, &shape, at.factor, None, &mut encodings);
                        for encoding in encodings.drain(..) {
                            match encoding {
                                Encoding::Item(item) => {
                                    encoded.items.push(item);
                                    if let Some(ListRun::Items { count, .. }) =
                                        encoded.runs[from..].last_mut()
                                    {
                                        *count += 1;
                                        continue;
                                    }
                                    encoded.runs.push(ListRun::Items {
                                        start: encoded.items.len() - 1,
                                        count: 1,
                                    });
                                }
                                Encoding::Drawing {
                                    drawing,
                                    rect,
                                    clip,
                                } => encoded.runs.push(ListRun::Drawing {
                                    drawing,
                                    rect,
                                    clip,
                                }),
                            }
                        }
                    }
                    encoded.parts.push((from, encoded.runs.len()));
                }
            }
        }
    }

    fn release_drawings(&mut self) {
        let frames = self.frames;
        let lists = &mut self.lists;
        self.drawn.retain(|key| match lists.get(key) {
            Some(encoded) if encoded.seen == frames => true,
            Some(_) => {
                lists.remove(key);
                false
            }
            None => false,
        });
        self.frames += 1;
    }

    fn sweep(&mut self, output: &WindowOutput, pixels_per_point: f32) {
        let mut live = be_lib_stable_hash::HashSet::default();
        let at = Walk {
            factor: pixels_per_point,
            space: 0,
            origin: [0.0; 2],
            bound: OPEN,
            offset: Vec2::ZERO,
            clip: Rect::EVERYTHING,
            fade: Fade::NONE,
        };
        mark_layers(&output.layers, at, &mut live);
        self.lists.retain(|key, _| live.contains(key));
        self.live = live.len();
    }

    /// The items `shape` draws at `factor` pixels a point, leaving out what lies outside
    /// `damaged`: a path as Vello fills or strokes it, anything else as `push_quads` places it.
    fn encode_shape(
        &mut self,
        queue: &wgpu::Queue,
        shape: &Shape,
        factor: f32,
        damaged: Option<&[[f32; 4]]>,
        out: &mut Vec<Encoding>,
    ) {
        if let Shape::Path {
            path,
            scale,
            offset,
            color,
            rotation,
            clip,
        } = shape
        {
            let clip = pixel_bounds(*clip, factor);
            let reach = beui_core::damage::path_bounds(path, *scale, *offset, *rotation);
            if !reach.is_positive() || skipped(damaged, pixel_bounds(reach, factor), clip) {
                return;
            }
            let turn = match rotation.turns() {
                true => {
                    let (sin, cos) = rotation.angle.sin_cos();
                    beui_vello::turn(
                        [rotation.pivot.x * factor, rotation.pivot.y * factor],
                        cos,
                        sin,
                    )
                }
                false => Affine::IDENTITY,
            };
            let transform = turn
                * Affine::translate((
                    f64::from(offset.x * factor),
                    f64::from(offset.y * factor),
                ))
                * Affine::scale(f64::from(scale * factor));
            let style = match path.style() {
                PathStyle::Fill(rule) => Painted::Fill(match rule {
                    beui_types::FillRule::NonZero => beui_vello::vello_common::peniko::Fill::NonZero,
                    beui_types::FillRule::EvenOdd => beui_vello::vello_common::peniko::Fill::EvenOdd,
                }),
                PathStyle::Stroke(stroke) => {
                    Painted::Stroke(beui_core::path::kurbo_stroke(stroke, 1.0))
                }
            };
            let outline = beui_core::path::kurbo_path(path.path(), 1.0);
            let color = color.to_array();
            let seamed = path.seamed()
                && style == Painted::Fill(beui_vello::vello_common::peniko::Fill::NonZero);
            out.push(Encoding::Item(Item::Path {
                path: Rc::new(outline),
                style,
                transform,
                clip,
                color,
                seamed,
            }));
            return;
        }
        let mut quads = Vec::new();
        push_quads(shape, factor, damaged, &mut quads);
        out.extend(
            quads
                .into_iter()
                .filter_map(|quad| self.encoding(queue, quad)),
        );
    }

    fn encoding(&mut self, queue: &wgpu::Queue, quad: Quad) -> Option<Encoding> {
        Some(Encoding::Item(match quad {
            Quad::Rect {
                rect,
                clip,
                color,
                corner_radius,
                stroke_width,
                turn,
            } => Item::Rect {
                rect,
                clip,
                color: color.to_array(),
                radii: corner_radius,
                stroke: stroke_width,
                turn,
            },
            Quad::Glyph {
                rect,
                clip,
                color,
                glyph,
                turn,
            } => {
                let image = glyph.image()?;
                let region = self
                    .resources
                    .0
                    .borrow_mut()
                    .glyph(queue, glyph.id, image)?;
                Item::Glyph {
                    rect,
                    clip,
                    color: color.to_array(),
                    region,
                    colored: image.color,
                    turn,
                }
            }
            Quad::Image {
                rect,
                clip,
                source,
                image,
                tint,
                corner_radius,
                smooth,
                turn,
            } => {
                let texels = [
                    (source[2] - source[0]).abs() * image.width() as f32,
                    (source[3] - source[1]).abs() * image.height() as f32,
                ];
                let level = match smooth {
                    true => beui_vello::mip_level(texels, [rect[2] - rect[0], rect[3] - rect[1]]),
                    false => 0,
                };
                Item::Image {
                    rect,
                    clip,
                    source,
                    image,
                    level,
                    tint: tint.to_array(),
                    radius: corner_radius,
                    smooth,
                    turn,
                }
            }
            Quad::Line {
                clip,
                segment,
                width,
                color,
                ..
            } => Item::Line {
                clip,
                segment,
                width,
                color: color.to_array(),
            },
            Quad::Punch {
                rect,
                clip,
                corner_radius,
                turn,
            } => Item::Punch {
                rect,
                clip,
                radius: corner_radius,
                turn,
            },
            Quad::Drawing {
                rect,
                clip,
                drawing,
            } => {
                return Some(Encoding::Drawing {
                    drawing,
                    rect,
                    clip,
                });
            }
            // Paths are encoded whole (`encode_shape`), never as the triangles of a mesh.
            Quad::Triangle { .. } => return None,
        }))
    }

    /// Where a drawing emit places at `rect` within `clip` (in the document's pixels) is drawn.
    fn draw_at(
        &self,
        rect: [f32; 4],
        clip: [f32; 4],
        screen: Vec2,
        pixels_per_point: f32,
        composited: bool,
    ) -> DrawAt {
        let shift = |[left, top, right, bottom]: [f32; 4]| {
            [
                left - self.origin.x,
                top - self.origin.y,
                right - self.origin.x,
                bottom - self.origin.y,
            ]
        };
        DrawAt {
            rect: shift(rect),
            clip: shift(clip),
            screen,
            pixels_per_point,
            format: self.format,
            renderer: self.id,
            composited,
        }
    }

    /// Every drawing a composited window shows, in paint order, each where emit places it,
    /// found by walking only the displays that hold drawings (`Display::drawings`); and what the
    /// frame visits whether or not it is damaged: the drawings that placed a composited layer
    /// last frame, and the displays that hold them.
    fn found(
        &self,
        output: &WindowOutput,
        screen: Vec2,
        pixels_per_point: f32,
    ) -> (Vec<(Drawing, DrawAt)>, Forced) {
        struct Finding<'a> {
            renderer: &'a Renderer,
            screen: Vec2,
            pixels_per_point: f32,
            lifted: Option<&'a be_lib_stable_hash::HashSet<usize>>,
            path: Vec<u64>,
            found: Vec<(Drawing, DrawAt)>,
            forced: Forced,
        }
        impl Finding<'_> {
            fn shape(&mut self, shape: &Shape, at: Walk) {
                if !matches!(shape, Shape::Drawing { .. }) {
                    return;
                }
                let mut quads = Vec::new();
                push_quads(
                    &placed_shape(shape, at.offset, at.clip),
                    at.factor,
                    None,
                    &mut quads,
                );
                for quad in quads {
                    let Quad::Drawing {
                        rect,
                        clip,
                        drawing,
                    } = quad
                    else {
                        continue;
                    };
                    let (rect, clip) = (
                        translated(rect, at.origin),
                        overlap(translated(clip, at.origin), at.bound),
                    );
                    if !shows(overlap(rect, clip)) {
                        continue;
                    }
                    if self
                        .lifted
                        .is_some_and(|lifted| lifted.contains(&drawing.identity().0))
                    {
                        self.forced.drawings.insert(drawing.identity().0);
                        self.forced.displays.extend(self.path.iter().copied());
                    }
                    let at =
                        self.renderer
                            .draw_at(rect, clip, self.screen, self.pixels_per_point, true);
                    self.found.push((drawing, at));
                }
            }

            fn display(&mut self, display: &Display, at: Walk) {
                if display.drawings == 0 || !shows(at.reach(display)) {
                    return;
                }
                self.path.push(display.key);
                self.parts(display, &display.parts, at);
                self.path.pop();
            }

            fn parts(&mut self, display: &Display, parts: &[Part], at: Walk) {
                for part in parts {
                    match part {
                        Part::Shapes { start, end, .. } => {
                            for shape in &display.shapes[*start as usize..*end as usize] {
                                self.shape(shape, at);
                            }
                        }
                        Part::Child(_, translation, child) => {
                            self.display(child, at.child(*translation))
                        }
                        Part::Group(group, inner) => {
                            self.parts(display, inner, at.group(group, &mut |_, _, _| 0))
                        }
                    }
                }
            }

            fn layers(&mut self, layers: &[Layer], at: Walk) {
                for layer in layers {
                    match layer {
                        Layer::Shape(shape) => self.shape(shape, at),
                        Layer::Display(translation, display) => {
                            self.display(display, at.child(*translation))
                        }
                        Layer::Group(group, inner) => {
                            self.layers(inner, at.group(group, &mut |_, _, _| 0))
                        }
                    }
                }
            }
        }
        let mut finding = Finding {
            renderer: self,
            screen,
            pixels_per_point,
            lifted: self
                .composition
                .as_ref()
                .map(|composition| &composition.lifted),
            path: Vec::new(),
            found: Vec::new(),
            forced: Forced::default(),
        };
        finding.layers(
            &output.layers,
            Walk {
                factor: pixels_per_point,
                space: 0,
                origin: [0.0; 2],
                bound: OPEN,
                offset: Vec2::ZERO,
                clip: Rect::EVERYTHING,
                fade: Fade::NONE,
            },
        );
        (finding.found, finding.forced)
    }

    /// The layers a composited window's drawings place this frame, bottom first, each with the
    /// segment it lies in (how many anchored views lie under it): a drawing prepared this frame
    /// (`prepared`) says them anew, one that is where it was keeps last frame's, and any other
    /// is prepared now. Splits the frame's steps between the roots at each drawing that places a
    /// composited layer (an anchored view): a new root's steps begin there, cleared first where
    /// it is damaged. Returns `None` when the anchored views the roots split at are not last
    /// frame's on a frame not painted whole.
    fn place(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        found: Vec<(Drawing, DrawAt)>,
        prepared: &[(Drawing, DrawAt)],
        steps: &mut Vec<Step>,
        whole: bool,
    ) -> Option<Vec<(Placed, usize)>> {
        let fresh: be_lib_stable_hash::HashSet<(usize, u64)> = prepared
            .iter()
            .map(|(drawing, _)| drawing.identity())
            .collect();
        let composition = self.composition.as_mut()?;
        let mut encoder = None;
        let mut known = be_lib_stable_hash::HashMap::default();
        let mut placed = Vec::new();
        let mut lifted = be_lib_stable_hash::HashSet::default();
        let mut splits_at = Vec::new();
        for (drawing, at) in found {
            let Some(draw) = drawing.draw::<Rc<dyn Draw>>() else {
                continue;
            };
            let key = composite::Known::key(&drawing, &at);
            let layers = match composition.known.remove(&key) {
                _ if fresh.contains(&drawing.identity()) => draw.layers(),
                Some(kept) if kept.at == at => kept.placed,
                _ => {
                    let encoder = encoder.get_or_insert_with(|| {
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("beui drawings"),
                        })
                    });
                    draw.prepare(device, queue, encoder, at);
                    draw.layers()
                }
            };
            if let Some(layer) = layers.iter().find(|placed| !placed.inline) {
                lifted.insert(drawing.identity().0);
                splits_at.push(layer.layer.id);
            }
            let segment = splits_at.len();
            placed.extend(layers.iter().cloned().map(|layer| (layer, segment)));
            known.insert(
                key,
                composite::Known {
                    drawing,
                    at,
                    placed: layers,
                },
            );
        }
        if let Some(encoder) = encoder {
            queue.submit([encoder.finish()]);
        }
        composition.known = known;
        composition.lifted = lifted;
        debug_assert!(
            {
                let mut ids: Vec<LayerId> =
                    placed.iter().map(|(shown, _)| shown.layer.id).collect();
                ids.sort_unstable();
                ids.windows(2).all(|pair| pair[0] != pair[1])
            },
            "every layer a window's drawings place has an id of its own"
        );
        if !whole && splits_at != composition.splits_at {
            return None;
        }
        composition.splits_at = splits_at;
        // Where each root after the first begins in the steps: at the run of each drawing that
        // placed a composited layer, or after a group or filter holding one.
        fn lifts(run: &Run, lifted: &be_lib_stable_hash::HashSet<usize>) -> bool {
            matches!(run, Run::Drawing(drawing, _) if lifted.contains(&drawing.identity().0))
        }
        fn nested(steps: &[Step], lifted: &be_lib_stable_hash::HashSet<usize>) -> usize {
            steps
                .iter()
                .map(|step| match step {
                    Step::Runs(runs) => runs.iter().filter(|run| lifts(run, lifted)).count(),
                    Step::Filter { steps, .. } | Step::Group { steps, .. } => nested(steps, lifted),
                })
                .sum()
        }
        let lifted = &composition.lifted;
        let mut split = Vec::with_capacity(steps.len());
        let mut splits = Vec::new();
        for step in std::mem::take(steps) {
            let runs = match step {
                Step::Runs(runs) => runs,
                Step::Filter {
                    steps: ref inner, ..
                }
                | Step::Group {
                    steps: ref inner, ..
                } => {
                    // An anchored view inside a group or filter cannot be lifted out of it: the
                    // roots split after the step that holds it.
                    let count = nested(inner, lifted);
                    if count > 0 && !composition.warned_nested {
                        // Known limit (plans/host_as_platform_adapter.md, The layer list): the
                        // view ignores the group's opacity and filter, and what the group paints
                        // after it lands below it.
                        composition.warned_nested = true;
                        eprintln!(
                            "beui: an anchored view inside a group or filter is composited above \
                             the whole group, unfaded and unfiltered"
                        );
                    }
                    split.push(step);
                    splits.extend(std::iter::repeat_n(split.len(), count));
                    continue;
                }
            };
            let mut held = Vec::new();
            for run in runs {
                if lifts(&run, lifted) {
                    if !held.is_empty() {
                        split.push(Step::Runs(std::mem::take(&mut held)));
                    }
                    splits.push(split.len());
                }
                held.push(run);
            }
            if !held.is_empty() {
                split.push(Step::Runs(held));
            }
        }
        *steps = split;
        // An anchored view not in the steps is not damaged, and nothing damaged lies under or
        // over it, so the damaged steps stay on the roots below it; the roots above it are
        // cleared where damaged.
        match whole {
            true => debug_assert_eq!(
                splits.len(),
                composition.splits_at.len(),
                "a frame painted whole holds every anchored view's drawing"
            ),
            false => debug_assert!(
                splits.len() <= composition.splits_at.len(),
                "a damaged frame holds at most the anchored views the window shows"
            ),
        }
        splits.resize(composition.splits_at.len(), steps.len());
        if !whole {
            for (inserted, at) in splits.iter_mut().enumerate() {
                *at += inserted;
                steps.insert(*at, Step::Runs(self.cleared.clone()));
            }
        }
        composition.splits = splits;
        Some(placed)
    }

    fn record_drawings(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        drawings: &[(Drawing, DrawAt)],
    ) {
        if drawings.is_empty() {
            return;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("beui drawings"),
        });
        for (drawing, at) in drawings {
            if let Some(draw) = drawing.draw::<Rc<dyn Draw>>() {
                draw.prepare(device, queue, &mut encoder, *at);
            }
        }
        queue.submit([encoder.finish()]);
    }

    /// Where the frame draws on its target, `None` where it draws it whole: what its UI
    /// damaged, or for a composited window, where its layers changed (`composite`).
    pub fn scissors(&self) -> Option<&[[u32; 4]]> {
        match &self.composition {
            Some(composition) => composition.composed.as_deref(),
            None => self.scissors.as_deref(),
        }
    }

    /// Draws `output` onto `target`, `size` pixels showing `screen` points at
    /// `pixels_per_point`, over what it holds: prepares `repaint`, or `whole` if the renderer
    /// finds it must repaint everything, then renders and submits. How a guest draws a
    /// document into a surface's texture (be-sdk-beui-platform's screens, be-sdk-beui-root's
    /// surfaces).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_onto(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &WindowOutput,
        (screen, pixels_per_point): (Vec2, f32),
        (repaint, whole): (Repaint, Repaint),
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let prepared = self.prepare(device, queue, output, screen, pixels_per_point, repaint);
        if matches!(repaint, Repaint::Region { .. }) && matches!(prepared, Repaint::Everything) {
            self.prepare(device, queue, output, screen, pixels_per_point, whole);
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.render(
            device,
            queue,
            &mut encoder,
            target,
            size,
            wgpu::LoadOp::Load,
        );
        queue.submit([encoder.finish()]);
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        load: wgpu::LoadOp<wgpu::Color>,
    ) {
        self.target = size;
        if self.opacities.is_empty() {
            self.idle_groups += 1;
            if self.idle_groups > IDLE_GROUP_FRAMES {
                self.groups = None;
            }
        } else {
            self.idle_groups = 0;
        }
        if let Some(vello) = &mut self.vello {
            vello.rendered = 0;
        }
        let Some(mut composition) = self.composition.take() else {
            self.render_into(device, queue, encoder, target, load, 0..self.steps.len());
            return;
        };
        // A composited window: its UI into the roots, one below its first anchored view and one
        // above each, then the window from its layers.
        composition.fit_roots(device);
        let whole = self.whole;
        let length = self.steps.len();
        let bounds: Vec<usize> = std::iter::once(0)
            .chain(composition.splits.iter().map(|at| (*at).min(length)))
            .chain(std::iter::once(length))
            .collect();
        for (index, root) in composition.roots.iter().enumerate() {
            let load = match (whole, index, load) {
                (true, 0, wgpu::LoadOp::Clear(color)) => wgpu::LoadOp::Clear(color),
                (true, _, _) => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                (false, _, _) => wgpu::LoadOp::Load,
            };
            let view = root.view.clone();
            let steps = bounds[index]..bounds[index + 1].max(bounds[index]);
            self.render_into(device, queue, encoder, &view, load, steps);
        }
        composition.compose(queue, encoder, target, load);
        self.composition = Some(composition);
    }

    /// Renders `steps` of the frame into `target`: what lies between the drawings and filters,
    /// and whatever a group or filter holds that a Vello layer can, as Vello scenes.
    fn render_into(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
        steps: std::ops::Range<usize>,
    ) {
        if self.empty() {
            return;
        }
        let size = self.target;
        if Step::splits(&self.steps[steps.clone()]) && !self.opacities.is_empty() {
            let depth = Step::depth(&self.steps);
            let format = self.format;
            self.groups
                .get_or_insert_with(|| group::Groups::new(device, format))
                .prepare(device, queue, &self.opacities, depth, size);
        }
        let format = self.format;
        self.effects.truncate(self.filters.len());
        while self.effects.len() < self.filters.len() {
            self.effects.push(filter::Effects::new(device, format));
        }
        for effects in &mut self.effects {
            effects.ensure(device, size);
        }
        let clear = match load {
            wgpu::LoadOp::Clear(color) => Some(srgb(self.format, color)),
            wgpu::LoadOp::Load | wgpu::LoadOp::DontCare(_) => None,
        };
        let clip = self.scissors.clone();
        let held = std::mem::take(&mut self.steps);
        let mut onto = Onto::new(target, size, clear, clip);
        self.begin_scene(&onto);
        self.paint_steps(device, queue, encoder, &mut onto, &held[steps], 0);
        self.flush(device, queue, encoder, &mut onto);
        self.steps = held;
    }

    /// Starts the scene for `onto`: clipped to what it draws, and over its background when it
    /// is cleared whole.
    fn begin_scene(&mut self, onto: &Onto<'_>) {
        let (width, height) = onto.size;
        let (width, height) = (width.clamp(1, u32::from(u16::MAX)) as u16, height.clamp(1, u32::from(u16::MAX)) as u16);
        let vello = self.vello.get_or_insert_with(|| Vello::scene_only(width, height));
        vello.scene.reset_and_resize(width, height);
        let mut canvas = scene::Gpu(&mut vello.scene);
        use beui_vello::Canvas;
        canvas.set_transform(Affine::IDENTITY);
        if let Some(clip) = &onto.clip {
            let mut path = beui_vello::vello_common::kurbo::BezPath::new();
            for [left, top, width, height] in clip {
                let rect = beui_vello::rect([
                    *left as f32,
                    *top as f32,
                    (*left + *width) as f32,
                    (*top + *height) as f32,
                ]);
                path.extend(beui_vello::vello_common::kurbo::Shape::path_elements(&rect, 0.1));
            }
            canvas.push_clip_path(&path);
        }
        if onto.first
            && let Some(color) = onto.clear
            && color[3] > 0
        {
            beui_vello::filled_rect(
                &mut canvas,
                [0.0, 0.0, width as f32, height as f32],
                [0.0; 4],
                0.0,
                color,
                Affine::IDENTITY,
            );
        }
        onto.drawn.set(false);
    }

    /// Renders the scene built for `onto` and starts the next.
    fn flush(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        onto: &mut Onto<'_>,
    ) {
        let format = onto.view.texture().format().remove_srgb_suffix();
        let Some(vello) = &mut self.vello else {
            return;
        };
        if onto.clip.is_some() {
            vello.scene.pop_clip();
        }
        // Checked at each render: a submit before it may have found the device lost.
        let wanted = (onto.first || onto.drawn.get()) && !self.resources.lost();
        if wanted {
            if vello.format != format || !vello.made() {
                vello.make(device, queue, format, onto.size);
            }
            if vello.rendered > 0 {
                let done = std::mem::replace(
                    encoder,
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("beui encoder"),
                    }),
                );
                queue.submit([done.finish()]);
            }
            let rects: Vec<vello_gpu::geometry::RectU16> = onto
                .clip
                .iter()
                .flatten()
                .map(|[left, top, width, height]| {
                    vello_gpu::geometry::RectU16::new(
                        *left as u16,
                        *top as u16,
                        (*left + *width).min(u32::from(u16::MAX)) as u16,
                        (*top + *height).min(u32::from(u16::MAX)) as u16,
                    )
                })
                .collect();
            let target_init = match (onto.first, &onto.clip) {
                (false, _) => vello_gpu::TargetInit::SrcOver,
                (true, Some(_)) => vello_gpu::TargetInit::Clear(vello_gpu::ClearSettings::Rects {
                    color: beui_vello::vello_common::peniko::Color::TRANSPARENT,
                    rects: &rects,
                }),
                (true, None) if onto.clear.is_some() => {
                    vello_gpu::TargetInit::Clear(vello_gpu::ClearSettings::default())
                }
                (true, None) => vello_gpu::TargetInit::SrcOver,
            };
            let drawn = drawn_view(onto.view);
            let mut options = vello_gpu::RenderOptions::default();
            options.target_init = target_init;
            let Vello {
                renderer: Some((renderer, resources)),
                scene,
                ..
            } = vello
            else {
                unreachable!("Vello's renderer was made");
            };
            if let Err(error) = renderer.render(
                scene,
                resources,
                encoder,
                vello_gpu::RenderTarget {
                    view: &drawn,
                    depth_view: None,
                },
                &self.textures.bindings(self.filtered.as_ref()),
                options,
            ) {
                eprintln!("beui: Vello could not render the frame: {error:?}");
            }
            vello.rendered += 1;
        }
        onto.first = false;
        self.begin_scene(onto);
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_steps(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        onto: &mut Onto<'_>,
        steps: &[Step],
        depth: usize,
    ) {
        for step in steps {
            match step {
                Step::Runs(runs) => {
                    for run in runs {
                        match run {
                            Run::Items { space, items } => {
                                let Some(vello) = &mut self.vello else {
                                    continue;
                                };
                                let space = self.spaces[*space as usize];
                                scene::run(
                                    &mut scene::Gpu(&mut vello.scene),
                                    &space,
                                    items,
                                    [self.origin.x, self.origin.y],
                                    &self.textures,
                                );
                                onto.drawn.set(true);
                            }
                            Run::Drawing(drawing, at) => {
                                self.flush(device, queue, encoder, onto);
                                let Some(draw) = drawing.draw::<Rc<dyn Draw>>() else {
                                    continue;
                                };
                                let scissors: Vec<Option<[u32; 4]>> = match &onto.clip {
                                    Some(clip) => clip.iter().copied().map(Some).collect(),
                                    None => vec![None],
                                };
                                let mut pass = self.begin_at(encoder, onto.view, wgpu::LoadOp::Load, depth);
                                for scissor in scissors {
                                    let Some(at) = within(*at, scissor) else {
                                        continue;
                                    };
                                    let bounds = scissor.unwrap_or([0, 0, at.screen.x as u32, at.screen.y as u32]);
                                    if clip(&mut pass, clipped(at.clip, bounds)) {
                                        draw.paint(&mut pass, at);
                                    }
                                }
                            }
                        }
                    }
                }
                Step::Group {
                    index,
                    bounds,
                    steps: grouped,
                } => {
                    let opacity = self.opacities.get(*index).copied().unwrap_or(1.0);
                    if !Step::splits(grouped) {
                        if let Some(vello) = &mut self.vello {
                            vello.scene.push_opacity_layer(opacity);
                        }
                        self.paint_steps(device, queue, encoder, onto, grouped, depth);
                        if let Some(vello) = &mut self.vello {
                            vello.scene.pop_layer();
                        }
                        continue;
                    }
                    // A group holding a drawing or a filter: its own layer, composed at its
                    // opacity.
                    self.flush(device, queue, encoder, onto);
                    let region = self.region(*bounds);
                    let regions: Vec<[u32; 4]> = match &onto.clip {
                        Some(clip) => clip
                            .iter()
                            .map(|scissor| intersected(*scissor, region))
                            .filter(|region| region[2] > 0 && region[3] > 0)
                            .collect(),
                        None => vec![region].into_iter().filter(|region| region[2] > 0 && region[3] > 0).collect(),
                    };
                    let Some(layer) = self
                        .groups
                        .as_ref()
                        .and_then(|groups| groups.layer_view(depth))
                        .cloned()
                    else {
                        continue;
                    };
                    if regions.is_empty() {
                        continue;
                    }
                    let mut inner = Onto::new(&layer, layer_size(&layer), None, Some(regions.clone()));
                    self.begin_scene(&inner);
                    self.paint_steps(device, queue, encoder, &mut inner, grouped, depth + 1);
                    self.flush(device, queue, encoder, &mut inner);
                    // The scene is shared: what follows the group is drawn onto this target again.
                    self.begin_scene(onto);
                    let drawn = drawn_view(onto.view);
                    let mut pass = self.begin_at(encoder, &drawn, wgpu::LoadOp::Load, depth);
                    for region in regions {
                        clip(&mut pass, region);
                        if let Some(groups) = &self.groups {
                            groups.compose(&mut pass, depth, *index);
                        }
                    }
                }
                Step::Filter {
                    index,
                    bottom,
                    steps: filtered,
                } => {
                    self.flush(device, queue, encoder, onto);
                    let Some(scene_view) = self.effects.get(*index).and_then(filter::Effects::scene).cloned()
                    else {
                        continue;
                    };
                    let clear = match (self.whole, onto.clear) {
                        (true, Some(color)) if *bottom => Some(color),
                        (true, _) => Some([0; 4]),
                        (false, _) => None,
                    };
                    let mut inner = Onto::new(&scene_view, onto.size, clear, onto.clip.clone());
                    self.begin_scene(&inner);
                    self.paint_steps(device, queue, encoder, &mut inner, filtered, depth);
                    self.flush(device, queue, encoder, &mut inner);
                    let (prepared, srgb) = (self.filters[*index], self.format.is_srgb());
                    let blurred = self.effects.get(*index).and_then(filter::Effects::blurred).cloned();
                    if let Some(blurred) = blurred.filter(|_| prepared.sigma() > 0.0) {
                        let region = prepared.region();
                        // Blurred whole: under a clip, vello_gpu 0.4's filter layer takes
                        // nothing from outside the clip's pixels, so light from beside the damage
                        // went missing. The combine pass composes only the damage.
                        let mut onto_blur = Onto::new(&blurred, onto.size, Some([0; 4]), None);
                        self.begin_scene(&onto_blur);
                        if let Some(vello) = &mut self.vello {
                            scene::blur(&mut vello.scene, onto.size, region, prepared.sigma());
                        }
                        onto_blur.drawn.set(true);
                        self.filtered = Some(drawn_view(&scene_view));
                        self.flush(device, queue, encoder, &mut onto_blur);
                        self.filtered = None;
                    }
                    self.begin_scene(onto);
                    let scissors: Vec<Option<[u32; 4]>> = match &onto.clip {
                        Some(clip) => clip.iter().copied().map(Some).collect(),
                        None => vec![None],
                    };
                    if let Some(effects) = self.effects.get_mut(*index) {
                        effects.record(device, queue, &prepared, srgb);
                    }
                    let mut pass = self.begin_at(encoder, onto.view, wgpu::LoadOp::Load, depth);
                    for scissor in scissors {
                        if let Some(scissor) = scissor {
                            clip(&mut pass, scissor);
                        }
                        if let Some(effects) = self.effects.get(*index) {
                            effects.compose(&mut pass);
                        }
                    }
                }
            }
        }
    }

    /// The pixels of the target that `bounds`, in physical pixels of the document,
    /// covers, as a scissor.
    fn region(&self, bounds: [f32; 4]) -> [u32; 4] {
        let (width, height) = self.target;
        let [left, top, right, bottom] = translated(bounds, [-self.origin.x, -self.origin.y]);
        let left = left.floor().clamp(0.0, width as f32) as u32;
        let top = top.floor().clamp(0.0, height as f32) as u32;
        let right = right.ceil().clamp(0.0, width as f32) as u32;
        let bottom = bottom.ceil().clamp(0.0, height as f32) as u32;
        [
            left,
            top,
            right.saturating_sub(left),
            bottom.saturating_sub(top),
        ]
    }

    fn empty(&self) -> bool {
        self.scissors.as_ref().is_some_and(|scissors| {
            scissors
                .iter()
                .all(|[_, _, width, height]| *width == 0 || *height == 0)
        })
    }

    fn begin<'pass>(
        &self,
        encoder: &'pass mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> wgpu::RenderPass<'pass> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("beui pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    /// Begins a pass on a target `depth` groups deep: a group's layer may be larger
    /// than the window's target, so it draws onto the part of it the target's size.
    fn begin_at<'pass>(
        &self,
        encoder: &'pass mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
        depth: usize,
    ) -> wgpu::RenderPass<'pass> {
        let mut pass = self.begin(encoder, target, load);
        let (width, height) = self.target;
        if depth > 0 && width > 0 && height > 0 {
            pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
        }
        pass
    }
}

fn mark(display: &Display, at: Walk, live: &mut be_lib_stable_hash::HashSet<ListKey>) {
    let reach = at.reach(display);
    if reach[0] >= reach[2] || reach[1] >= reach[3] {
        return;
    }
    live.insert(at.key(display));
    mark_parts(&display.parts, at, live);
}

fn mark_parts(parts: &[Part], at: Walk, live: &mut be_lib_stable_hash::HashSet<ListKey>) {
    for part in parts {
        match part {
            Part::Shapes { .. } => {}
            Part::Child(_, translation, child) => mark(child, at.child(*translation), live),
            Part::Group(group, inner) => mark_parts(inner, at.group(group, &mut |_, _, _| 0), live),
        }
    }
}

fn mark_layers(layers: &[Layer], at: Walk, live: &mut be_lib_stable_hash::HashSet<ListKey>) {
    for layer in layers {
        match layer {
            Layer::Shape(_) => {}
            Layer::Display(translation, display) => mark(display, at.child(*translation), live),
            Layer::Group(group, inner) => {
                mark_layers(inner, at.group(group, &mut |_, _, _| 0), live)
            }
        }
    }
}

fn physical(region: Rect, origin: Vec2, screen: Vec2, pixels_per_point: f32) -> [f32; 4] {
    [
        (region.left() * pixels_per_point)
            .floor()
            .clamp(origin.x, origin.x + screen.x),
        (region.top() * pixels_per_point)
            .floor()
            .clamp(origin.y, origin.y + screen.y),
        (region.right() * pixels_per_point)
            .ceil()
            .clamp(origin.x, origin.x + screen.x),
        (region.bottom() * pixels_per_point)
            .ceil()
            .clamp(origin.y, origin.y + screen.y),
    ]
}

fn clip(pass: &mut wgpu::RenderPass<'_>, [left, top, width, height]: [u32; 4]) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    pass.set_scissor_rect(left, top, width, height);
    true
}

fn within(at: DrawAt, scissor: Option<[u32; 4]>) -> Option<DrawAt> {
    let Some([left, top, width, height]) = scissor else {
        return Some(at);
    };
    let [left, top] = [left as f32, top as f32];
    let clip = [
        at.clip[0].max(left),
        at.clip[1].max(top),
        at.clip[2].min(left + width as f32),
        at.clip[3].min(top + height as f32),
    ];
    (clip[0] < clip[2] && clip[1] < clip[3]).then_some(DrawAt { clip, ..at })
}

fn clipped(clip: [f32; 4], [left, top, width, height]: [u32; 4]) -> [u32; 4] {
    let (right, bottom) = (left + width, top + height);
    let from = |value: f32, low: u32, high: u32| value.clamp(low as f32, high as f32);
    let (x0, y0) = (
        from(clip[0].floor(), left, right),
        from(clip[1].floor(), top, bottom),
    );
    let (x1, y1) = (
        from(clip[2].ceil(), left, right),
        from(clip[3].ceil(), top, bottom),
    );
    [
        x0 as u32,
        y0 as u32,
        (x1 - x0).max(0.0) as u32,
        (y1 - y0).max(0.0) as u32,
    ]
}

fn intersected([left, top, width, height]: [u32; 4], bounds: [u32; 4]) -> [u32; 4] {
    let [bound_left, bound_top, bound_width, bound_height] = bounds;
    let x0 = left.max(bound_left);
    let y0 = top.max(bound_top);
    let x1 = (left + width).min(bound_left + bound_width);
    let y1 = (top + height).min(bound_top + bound_height);
    [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
}

fn scissor(damaged: [f32; 4], origin: Vec2) -> [u32; 4] {
    let width = (damaged[2] - damaged[0]).max(0.0) as u32;
    let height = (damaged[3] - damaged[1]).max(0.0) as u32;
    [
        (damaged[0] - origin.x) as u32,
        (damaged[1] - origin.y) as u32,
        width,
        height,
    ]
}

pub fn wgpu_color(color: Color32) -> wgpu::Color {
    wgpu_color_in(wgpu::TextureFormat::Rgba8UnormSrgb, color)
}

pub fn wgpu_color_in(format: wgpu::TextureFormat, color: Color32) -> wgpu::Color {
    let [red, green, blue, alpha] = match format.is_srgb() {
        true => color.to_linear_f32(),
        false => color.to_normalized_gamma_f32(),
    };
    wgpu::Color {
        r: red as f64,
        g: green as f64,
        b: blue as f64,
        a: alpha as f64,
    }
}

/// What a run of scenes draws onto: a target (or a group's layer, or a filter's scene), the
/// part of it they draw, and how it starts.
struct Onto<'a> {
    view: &'a wgpu::TextureView,
    size: (u32, u32),
    /// The colour the target starts as when it is cleared whole, straight sRGB.
    clear: Option<[u8; 4]>,
    /// The pixels drawn (left, top, width, height), or none for the whole target.
    clip: Option<Vec<[u32; 4]>>,
    /// Whether the next scene is the first, which clears what it draws.
    first: bool,
    /// Whether the scene being built holds anything.
    drawn: std::cell::Cell<bool>,
}

impl<'a> Onto<'a> {
    fn new(
        view: &'a wgpu::TextureView,
        size: (u32, u32),
        clear: Option<[u8; 4]>,
        clip: Option<Vec<[u32; 4]>>,
    ) -> Self {
        Self {
            view,
            size,
            clear,
            clip,
            first: true,
            drawn: std::cell::Cell::new(false),
        }
    }
}

impl Vello {
    fn scene_only(width: u16, height: u16) -> Self {
        Self {
            renderer: None,
            format: wgpu::TextureFormat::Rgba8Unorm,
            scene: vello_gpu::Scene::new(width, height),
            rendered: 0,
        }
    }

    fn made(&self) -> bool {
        self.renderer.is_some()
    }

    fn make(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
    ) {
        let config = vello_gpu::RenderTargetConfig {
            format,
            width: width.clamp(1, u32::from(u16::MAX)) as u16,
            height: height.clamp(1, u32::from(u16::MAX)) as u16,
        };
        self.renderer = Some(vello_gpu::Renderer::new(device, queue, &config));
        self.format = format;
    }

}

/// A clear colour as wgpu takes it for a target of `format` (linear on an sRGB target), back
/// to straight sRGB bytes.
fn srgb(format: wgpu::TextureFormat, color: wgpu::Color) -> [u8; 4] {
    let encode = |value: f64| {
        let value = value.clamp(0.0, 1.0);
        match format.is_srgb() {
            true if value <= 0.003_130_8 => value * 12.92,
            true => 1.055 * value.powf(1.0 / 2.4) - 0.055,
            false => value,
        }
    };
    [
        (encode(color.r) * 255.0).round() as u8,
        (encode(color.g) * 255.0).round() as u8,
        (encode(color.b) * 255.0).round() as u8,
        (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn layer_size(view: &wgpu::TextureView) -> (u32, u32) {
    let size = view.texture().size();
    (size.width, size.height)
}

fn pixel_bounds(rect: Rect, pixels_per_point: f32) -> [f32; 4] {
    [
        (rect.min.x * pixels_per_point).round(),
        (rect.min.y * pixels_per_point).round(),
        (rect.max.x * pixels_per_point).round(),
        (rect.max.y * pixels_per_point).round(),
    ]
}

fn skipped(damaged: Option<&[[f32; 4]]>, rect: [f32; 4], clip: [f32; 4]) -> bool {
    let Some(damaged) = damaged else {
        return false;
    };
    !damaged.iter().any(|damaged| {
        let left = rect[0].max(clip[0]).max(damaged[0]);
        let top = rect[1].max(clip[1]).max(damaged[1]);
        let right = rect[2].min(clip[2]).min(damaged[2]);
        let bottom = rect[3].min(clip[3]).min(damaged[3]);
        left < right && top < bottom
    })
}

#[cfg(test)]
mod tests;

impl Drop for Renderer {
    fn drop(&mut self) {
        let mut shared = self.resources.0.borrow_mut();
        for id in self.held.drain() {
            shared.release(id);
        }
    }
}
