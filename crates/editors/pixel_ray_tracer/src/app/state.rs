use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use block_editor_beui::be_block::PixelRayTracerContent;
use block_editor_beui::be_block::pixel_ray_tracer::{
    PIXEL_RAY_TRACER_SIZE, PixelRayTracerOperation, PixelUpdate, Point, RayEntity, RaySettings,
    Scene,
};
use block_editor_beui::beui::Image;
use block_editor_beui::beui::reactive::Draw;
use block_editor_beui::beui::reactive::{
    ReadSignal, WriteSignal, create_effect, create_signal, untrack,
};
use block_editor_beui::{ContentProjection, Editor, PerformanceReporter, Waker};

use crate::geometry::{distance, distance_to_segment, inside, pixel_at, raster_line, snap};
use crate::overlay::{self, Preview};
use crate::raytracer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Tool {
    Pencil,
    PixelLine,
    Select,
    Surface,
    Water,
    Light,
    RayTrace,
}

impl Tool {
    pub(crate) const ALL: [Self; 7] = [
        Self::Pencil,
        Self::PixelLine,
        Self::Select,
        Self::Surface,
        Self::Water,
        Self::Light,
        Self::RayTrace,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::PixelLine => "Pixel line",
            Self::Select => "Select entity",
            Self::Surface => "Surface",
            Self::Water => "Water",
            Self::Light => "Light",
            Self::RayTrace => "Ray trace",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LightingKey {
    block_revision: u64,
    preview_revision: u64,
}

type LightingResult = (LightingKey, Vec<[u8; 4]>, Duration);
type RayResult = (Point, Vec<[u8; 4]>, u64, RaySettings, Duration);

#[derive(Clone)]
enum Interaction {
    Pixels {
        start: (u16, u16),
        current: (u16, u16),
        path: Vec<(u16, u16)>,
    },
    Entity {
        start: Point,
        current: Point,
    },
    Endpoint {
        id: u64,
        start: bool,
        current: Point,
    },
    Light {
        id: u64,
        current: Point,
    },
}

#[derive(PartialEq)]
struct Overlay {
    entities: Vec<RayEntity>,
    selected: Option<u64>,
    preview: Preview,
}

struct TracedRays {
    origin: Point,
    revision: u64,
    settings: RaySettings,
}

pub(crate) struct RayState {
    editor: Editor,
    block: Scenes,
    performance: PerformanceReporter,
    interaction: RefCell<Option<Interaction>>,
    interaction_revision: Cell<u64>,
    lighting_job: RefCell<Option<Receiver<LightingResult>>>,
    lighting_key: Cell<Option<LightingKey>>,
    rendered: RefCell<Vec<[u8; 4]>>,
    ray_job: RefCell<Option<Receiver<RayResult>>>,
    ray_state: RefCell<Option<TracedRays>>,
    shown_overlay: RefCell<Option<Overlay>>,
    pub(crate) entities: ReadSignal<Vec<RayEntity>>,
    pub(crate) lighting: ReadSignal<Option<Image>>,
    set_lighting: WriteSignal<Option<Image>>,
    pub(crate) rays: ReadSignal<Option<Image>>,
    set_rays: WriteSignal<Option<Image>>,
    pub(crate) overlay: ReadSignal<Draw>,
    set_overlay: WriteSignal<Draw>,
    pub(crate) tool: ReadSignal<Tool>,
    set_tool: WriteSignal<Tool>,
    pub(crate) color_index: ReadSignal<u8>,
    set_color_index: WriteSignal<u8>,
    pub(crate) selected: ReadSignal<Option<u64>>,
    set_selected: WriteSignal<Option<u64>>,
    pub(crate) new_light_intensity: ReadSignal<f32>,
    set_new_light_intensity: WriteSignal<f32>,
    pub(crate) new_surface: ReadSignal<Surface>,
    set_new_surface: WriteSignal<Surface>,
    pointer: Cell<Option<Point>>,
    poked: ReadSignal<u64>,
    set_poked: WriteSignal<u64>,
    waker: RefCell<Waker>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Surface {
    pub(crate) roughness: f32,
    pub(crate) metalness: f32,
    pub(crate) transmission: f32,
    pub(crate) refractive_index: f32,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            roughness: 0.0,
            metalness: 0.0,
            transmission: 0.0,
            refractive_index: 1.5,
        }
    }
}

impl RayState {
    pub(crate) fn new(editor: &Editor) -> Rc<Self> {
        let block = Scenes::new(editor.block_content::<PixelRayTracerContent>());
        let performance = editor
            .host()
            .performance(format!("Pixel ray tracer ({})", editor.block_id()));
        let entities = block.content.project(|content| {
            content
                .root()
                .entities
                .values()
                .cloned()
                .collect::<Vec<_>>()
        });
        let (lighting, set_lighting) = create_signal(None);
        let (rays, set_rays) = create_signal(None);
        let (overlay, set_overlay) = create_signal(overlay::draw(Vec::new(), None, Preview::None));
        let (tool, set_tool) = create_signal(Tool::Pencil);
        let (color_index, set_color_index) = create_signal(0);
        let (selected, set_selected) = create_signal(None);
        let (new_light_intensity, set_new_light_intensity) = create_signal(2.0);
        let (new_surface, set_new_surface) = create_signal(Surface::default());
        let (poked, set_poked) = create_signal(0);
        Rc::new(Self {
            editor: editor.clone(),
            block,
            performance,
            interaction: RefCell::new(None),
            interaction_revision: Cell::new(0),
            lighting_job: RefCell::new(None),
            lighting_key: Cell::new(None),
            rendered: RefCell::new(Vec::new()),
            ray_job: RefCell::new(None),
            ray_state: RefCell::new(None),
            shown_overlay: RefCell::new(None),
            entities,
            lighting,
            set_lighting,
            rays,
            set_rays,
            overlay,
            set_overlay,
            tool,
            set_tool,
            color_index,
            set_color_index,
            selected,
            set_selected,
            new_light_intensity,
            set_new_light_intensity,
            new_surface,
            set_new_surface,
            pointer: Cell::new(None),
            poked,
            set_poked,
            waker: RefCell::new(Waker::default()),
        })
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.editor
    }

    pub(crate) fn settings(&self, view: bool) -> RaySettings {
        self.block
            .read()
            .map_or_else(RaySettings::default, |scene| match view {
                true => scene.view_ray_settings(),
                false => scene.lighting_ray_settings(),
            })
    }

    pub(crate) fn set_settings(&self, view: bool, settings: RaySettings) {
        match view {
            true => {
                self.block
                    .operate(PixelRayTracerOperation::SetViewRaySettings { settings });
                self.ray_state.borrow_mut().take();
            }
            false => self
                .block
                .operate(PixelRayTracerOperation::SetLightingRaySettings { settings }),
        }
    }

    pub(crate) fn choose_tool(&self, tool: Tool) {
        self.set_tool.set(tool);
        self.interaction.borrow_mut().take();
        if tool != Tool::Select {
            self.set_selected.set(None);
        }
        self.poke();
    }

    fn poke(&self) {
        self.set_poked
            .update(|poked| *poked = poked.wrapping_add(1));
    }

    pub(crate) fn choose_color(&self, index: u8) {
        self.set_color_index.set(index);
        let Some(mut entity) = self.selected_entity() else {
            return;
        };
        match &mut entity {
            RayEntity::Surface { color_index, .. } | RayEntity::Light { color_index, .. } => {
                *color_index = index;
                self.block
                    .operate(PixelRayTracerOperation::UpdateEntity { entity });
            }
            RayEntity::Water { .. } => {}
        }
    }

    pub(crate) fn selected_entity(&self) -> Option<RayEntity> {
        let id = self.selected.get()?;
        self.entities
            .get()
            .into_iter()
            .find(|entity| entity.id() == id)
    }

    pub(crate) fn update_entity(&self, entity: RayEntity) {
        self.block
            .operate(PixelRayTracerOperation::UpdateEntity { entity });
    }

    pub(crate) fn delete_selected(&self) {
        let Some(id) = self.selected.get_untracked() else {
            return;
        };
        self.set_selected.set(None);
        self.block
            .operate(PixelRayTracerOperation::DeleteEntity { id });
    }

    pub(crate) fn reset(&self) {
        self.block.operate(PixelRayTracerOperation::Reset);
        self.set_selected.set(None);
    }

    pub(crate) fn set_light_intensity(&self, value: f32) {
        self.set_new_light_intensity.set(value);
    }

    pub(crate) fn set_new_surface(&self, surface: Surface) {
        self.set_new_surface.set(surface);
    }

    pub(crate) fn hover(&self, at: Option<Point>) {
        if self.pointer.replace(at) != at {
            self.poke();
        }
    }

    pub(crate) fn press(&self, at: Point) {
        let tool = self.tool.get_untracked();
        let snapped = snap(at);
        match tool {
            Tool::Pencil | Tool::PixelLine => {
                if let Some(pixel) = pixel_at(at, false) {
                    *self.interaction.borrow_mut() = Some(Interaction::Pixels {
                        start: pixel,
                        current: pixel,
                        path: vec![pixel],
                    });
                }
            }
            Tool::Surface | Tool::Water => {
                *self.interaction.borrow_mut() = Some(Interaction::Entity {
                    start: snapped,
                    current: snapped,
                });
            }
            Tool::Light => {
                if inside(at) {
                    let id = self.block.read().map_or(1, |scene| scene.next_entity_id());
                    self.block.operate(PixelRayTracerOperation::AddEntity {
                        entity: RayEntity::Light {
                            id,
                            position: snapped,
                            color_index: self.color_index.get_untracked(),
                            intensity: self.new_light_intensity.get_untracked(),
                        },
                    });
                    self.set_selected.set(Some(id));
                }
            }
            Tool::Select => self.select_or_drag(at),
            Tool::RayTrace => {}
        }
        self.poke();
    }

    pub(crate) fn drag(&self, at: Point) {
        self.follow(at);
        self.poke();
    }

    fn follow(&self, at: Point) {
        let tool = self.tool.get_untracked();
        let mut held = self.interaction.borrow_mut();
        let Some(interaction) = held.as_mut() else {
            return;
        };
        match interaction {
            Interaction::Pixels { current, path, .. } => {
                let next = pixel_at(at, true).unwrap_or(*current);
                if tool == Tool::Pencil && next != *current {
                    path.extend(raster_line(*current, next).into_iter().skip(1));
                }
                *current = next;
            }
            Interaction::Entity { current, .. } => *current = snap(at),
            Interaction::Endpoint { current, .. } | Interaction::Light { current, .. } => {
                let next = snap(at);
                if *current != next {
                    *current = next;
                    self.interaction_revision
                        .set(self.interaction_revision.get().wrapping_add(1));
                }
            }
        }
    }

    pub(crate) fn release(&self) {
        let Some(interaction) = self.interaction.borrow_mut().take() else {
            return;
        };
        let tool = self.tool.get_untracked();
        let color_index = self.color_index.get_untracked();
        match interaction {
            Interaction::Pixels {
                start,
                current,
                path,
            } => {
                let points = match tool {
                    Tool::Pencil => path,
                    _ => raster_line(start, current),
                };
                let pixels = points
                    .into_iter()
                    .map(|(x, y)| PixelUpdate { x, y, color_index })
                    .collect();
                self.block
                    .operate(PixelRayTracerOperation::Paint { pixels });
            }
            Interaction::Entity { start, current } if start != current => {
                let id = self.block.read().map_or(1, |scene| scene.next_entity_id());
                let surface = self.new_surface.get_untracked();
                let entity = match tool {
                    Tool::Surface => RayEntity::Surface {
                        id,
                        start,
                        end: current,
                        color_index,
                        roughness: surface.roughness,
                        metalness: surface.metalness,
                        transmission: surface.transmission,
                        refractive_index: surface.refractive_index,
                    },
                    _ => RayEntity::Water {
                        id,
                        start,
                        end: current,
                    },
                };
                self.block
                    .operate(PixelRayTracerOperation::AddEntity { entity });
                self.set_selected.set(Some(id));
            }
            Interaction::Endpoint { id, start, current } => {
                if let Some(entity) =
                    moved(&self.entities.get_untracked(), id, Some(start), current)
                {
                    self.update_entity(entity);
                }
            }
            Interaction::Light { id, current } => {
                if let Some(entity) = moved(&self.entities.get_untracked(), id, None, current) {
                    self.update_entity(entity);
                }
            }
            Interaction::Entity { .. } => {}
        }
        self.poke();
    }

    fn select_or_drag(&self, at: Point) {
        let entities = self.entities.get_untracked();
        if let Some(id) = self.selected.get_untracked()
            && let Some(entity) = entities.iter().find(|entity| entity.id() == id).cloned()
        {
            let tolerance = 3.0;
            match entity {
                RayEntity::Light { position, .. } if distance(at, position) <= tolerance => {
                    *self.interaction.borrow_mut() = Some(Interaction::Light {
                        id,
                        current: position,
                    });
                    return;
                }
                RayEntity::Surface { start, .. } | RayEntity::Water { start, .. }
                    if distance(at, start) <= tolerance =>
                {
                    *self.interaction.borrow_mut() = Some(Interaction::Endpoint {
                        id,
                        start: true,
                        current: start,
                    });
                    return;
                }
                RayEntity::Surface { end, .. } | RayEntity::Water { end, .. }
                    if distance(at, end) <= tolerance =>
                {
                    *self.interaction.borrow_mut() = Some(Interaction::Endpoint {
                        id,
                        start: false,
                        current: end,
                    });
                    return;
                }
                _ => {}
            }
        }
        self.set_selected
            .set(closest(&entities, at).map(|entity| entity.id()));
    }

    fn drafted(&self) -> Option<RayEntity> {
        let held = self.interaction.borrow();
        let entities = self.entities.get_untracked();
        match held.as_ref()? {
            Interaction::Endpoint { id, start, current } => {
                moved(&entities, *id, Some(*start), *current)
            }
            Interaction::Light { id, current } => moved(&entities, *id, None, *current),
            _ => None,
        }
    }

    fn preview(&self) -> Preview {
        let held = self.interaction.borrow();
        let tool = self.tool.get_untracked();
        let color_index = self.color_index.get_untracked();
        match held.as_ref() {
            Some(Interaction::Pixels {
                start,
                current,
                path,
            }) => Preview::Pixels(
                match tool {
                    Tool::Pencil => path.clone(),
                    _ => raster_line(*start, *current),
                },
                color_index,
            ),
            Some(Interaction::Entity { start, current }) if tool == Tool::Surface => {
                Preview::Surface(*start, *current, color_index)
            }
            Some(Interaction::Entity { start, current }) => Preview::Water(*start, *current),
            _ => Preview::None,
        }
    }

    pub(crate) fn watch(self: &Rc<Self>) {
        let (waker, woken) = self.editor.woken();
        *self.waker.borrow_mut() = waker;
        let state = Rc::clone(self);
        create_effect(move || {
            woken.with(|_| ());
            state.poked.with(|_| ());
            state.entities.with(|_| ());
            state.tool.with(|_| ());
            state.selected.with(|_| ());
            state.color_index.with(|_| ());
            state.block.content.revision();
            untrack(|| state.settle());
        });
    }

    pub(crate) fn settle(&self) {
        self.settle_lighting();
        self.settle_rays();
        self.settle_overlay();
    }

    fn settle_overlay(&self) {
        let mut entities = self.entities.get_untracked();
        if let Some(draft) = self.drafted() {
            for entity in &mut entities {
                if entity.id() == draft.id() {
                    *entity = draft.clone();
                }
            }
        }
        let shown = Overlay {
            entities,
            selected: self.selected.get_untracked(),
            preview: self.preview(),
        };
        if self.shown_overlay.borrow().as_ref() == Some(&shown) {
            return;
        }
        self.set_overlay.set_unconditionally(overlay::draw(
            shown.entities.clone(),
            shown.selected,
            shown.preview.clone(),
        ));
        *self.shown_overlay.borrow_mut() = Some(shown);
    }

    fn settle_lighting(&self) {
        let Some(scene) = self.block.read() else {
            return;
        };
        let key = LightingKey {
            block_revision: scene.lighting_revision(),
            preview_revision: match self.drafted().is_some() {
                true => self.interaction_revision.get(),
                false => 0,
            },
        };
        self.land_lighting();
        if self.lighting_key.get() == Some(key) {
            self.performance.record_count("Lighting cache hits", 1);
            return;
        }
        if self.lighting_job.borrow().is_some() {
            return;
        }
        let pixels = scene.pixels().to_vec();
        let draft = self.drafted();
        let entities = scene
            .entities()
            .iter()
            .map(|entity| {
                draft
                    .as_ref()
                    .filter(|draft| draft.id() == entity.id())
                    .unwrap_or(entity)
                    .clone()
            })
            .collect::<Vec<_>>();
        let settings = scene.lighting_ray_settings();
        drop(scene);
        let waker = self.waker.borrow().clone();
        let (sender, receiver) = mpsc::channel();
        run_job("pixel-ray-tracer-lighting", move || {
            let started = Instant::now();
            let result = raytracer::trace_lighting(&pixels, &entities, settings);
            let _ = sender.send((key, result, started.elapsed()));
            waker.wake();
        });
        *self.lighting_job.borrow_mut() = Some(receiver);
        self.performance.record_count("Lighting cache misses", 1);
        self.land_lighting();
    }

    fn land_lighting(&self) {
        let landed =
            self.lighting_job
                .borrow()
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(Ok(result)),
                    Err(TryRecvError::Disconnected) => Some(Err(())),
                    Err(TryRecvError::Empty) => None,
                });
        if let Some(landed) = landed {
            self.lighting_job.borrow_mut().take();
            if let Ok((landed_key, pixels, duration)) = landed {
                self.performance.record_duration("Lighting trace", duration);
                self.set_lighting.set(Some(image_of(&pixels)));
                *self.rendered.borrow_mut() = pixels;
                self.ray_state.borrow_mut().take();
                self.set_rays.set(None);
                self.lighting_key.set(Some(landed_key));
            }
        }
    }

    fn settle_rays(&self) {
        if self.tool.get_untracked() != Tool::RayTrace {
            return;
        }
        let Some(origin) = self.pointer.get() else {
            return;
        };
        self.land_rays();
        let Some(scene) = self.block.read() else {
            return;
        };
        let settings = scene.view_ray_settings();
        let revision = scene.revision();
        let ready = self.lighting_key.get()
            == Some(LightingKey {
                block_revision: scene.lighting_revision(),
                preview_revision: 0,
            });
        let hit = ready
            && self.ray_state.borrow().as_ref().is_some_and(|held| {
                distance(held.origin, origin) <= 0.25
                    && held.revision == revision
                    && held.settings == settings
            });
        if hit {
            self.performance.record_count("View-ray cache hits", 1);
            return;
        }
        if !ready || self.ray_job.borrow().is_some() {
            return;
        }
        let source = self.rendered.borrow().clone();
        let entities = scene.entities().to_vec();
        drop(scene);
        let waker = self.waker.borrow().clone();
        let (sender, receiver) = mpsc::channel();
        run_job("pixel-ray-tracer-view-rays", move || {
            let started = Instant::now();
            let pixels = raytracer::trace_rays(&source, &entities, origin, settings);
            let _ = sender.send((origin, pixels, revision, settings, started.elapsed()));
            waker.wake();
        });
        *self.ray_job.borrow_mut() = Some(receiver);
        self.performance.record_count("View-ray cache misses", 1);
        self.land_rays();
    }

    fn land_rays(&self) {
        let landed =
            self.ray_job
                .borrow()
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(Ok(result)),
                    Err(TryRecvError::Disconnected) => Some(Err(())),
                    Err(TryRecvError::Empty) => None,
                });
        if let Some(landed) = landed {
            self.ray_job.borrow_mut().take();
            if let Ok((origin, pixels, revision, settings, duration)) = landed {
                self.performance.record_duration("View-ray trace", duration);
                self.set_rays.set(Some(image_of(&pixels)));
                *self.ray_state.borrow_mut() = Some(TracedRays {
                    origin,
                    revision,
                    settings,
                });
            }
        }
    }
}

#[cfg(not(test))]
fn run_job(name: &str, work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(work)
        .expect("failed to start a pixel ray tracer job");
}

#[cfg(test)]
fn run_job(_name: &str, work: impl FnOnce() + Send + 'static) {
    work();
}

fn image_of(pixels: &[[u8; 4]]) -> Image {
    Image::from_rgba(
        u32::from(PIXEL_RAY_TRACER_SIZE),
        u32::from(PIXEL_RAY_TRACER_SIZE),
        pixels.iter().flat_map(|pixel| *pixel).collect(),
    )
}

fn moved(
    entities: &[RayEntity],
    id: u64,
    endpoint: Option<bool>,
    point: Point,
) -> Option<RayEntity> {
    let mut entity = entities.iter().find(|entity| entity.id() == id).cloned()?;
    match (&mut entity, endpoint) {
        (RayEntity::Light { position, .. }, None) => *position = point,
        (RayEntity::Surface { start, .. } | RayEntity::Water { start, .. }, Some(true)) => {
            *start = point;
        }
        (RayEntity::Surface { end, .. } | RayEntity::Water { end, .. }, Some(false)) => {
            *end = point;
        }
        _ => return None,
    }
    Some(entity)
}

fn closest(entities: &[RayEntity], point: Point) -> Option<RayEntity> {
    let mut nearest: Option<(f32, RayEntity)> = None;
    for entity in entities {
        let reach = match entity {
            RayEntity::Light { position, .. } => distance(point, *position),
            RayEntity::Surface { start, end, .. } => distance_to_segment(point, *start, *end),
            RayEntity::Water { start, end, .. } => {
                let left = start.x.min(end.x);
                let right = start.x.max(end.x);
                let top = start.y.min(end.y);
                let bottom = start.y.max(end.y);
                if (left..=right).contains(&point.x) && (top..=bottom).contains(&point.y) {
                    0.0
                } else {
                    distance_to_segment(point, Point::new(left, top), Point::new(right, bottom))
                }
            }
        };
        if reach <= 4.0 && nearest.as_ref().is_none_or(|(held, _)| reach < *held) {
            nearest = Some((reach, entity.clone()));
        }
    }
    nearest.map(|(_, entity)| entity)
}

struct Scenes {
    content: Rc<ContentProjection<PixelRayTracerContent>>,
    cached: RefCell<Option<Snapshot>>,
}

#[derive(Clone)]
struct Snapshot {
    scene: Rc<Scene>,
    revision: u64,
    lighting_revision: u64,
}

impl std::ops::Deref for Snapshot {
    type Target = Scene;

    fn deref(&self) -> &Scene {
        &self.scene
    }
}

impl Snapshot {
    const fn revision(&self) -> u64 {
        self.revision
    }

    const fn lighting_revision(&self) -> u64 {
        self.lighting_revision
    }
}

impl Scenes {
    fn new(content: Rc<ContentProjection<PixelRayTracerContent>>) -> Self {
        Self {
            content,
            cached: RefCell::new(None),
        }
    }

    fn read(&self) -> Option<Snapshot> {
        let revision = self.content.revision()?;
        let mut cached = self.cached.borrow_mut();
        if let Some(held) = cached.as_ref().filter(|held| held.revision == revision) {
            return Some(held.clone());
        }
        let scene = Rc::new(self.content.read(|content| content.root().scene())?);
        let lighting_revision = match cached.as_ref() {
            Some(held)
                if held.pixels() == scene.pixels()
                    && held.entities() == scene.entities()
                    && held.lighting_ray_settings() == scene.lighting_ray_settings() =>
            {
                held.lighting_revision
            }
            Some(held) => held.lighting_revision + 1,
            None => 0,
        };
        let snapshot = Snapshot {
            scene,
            revision,
            lighting_revision,
        };
        *cached = Some(snapshot.clone());
        Some(snapshot)
    }

    fn operate(&self, operation: PixelRayTracerOperation) {
        let Some(edit) = self
            .content
            .read(|content| content.root().edit_for(&operation))
        else {
            return;
        };
        if !edit.0.is_empty() {
            self.content.operate(edit);
        }
    }
}
