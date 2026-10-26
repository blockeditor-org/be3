use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use block_editor_beui::beui::Image;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{ContentProjection, Editor, PerformanceReporter, Waker};
use pixel_ray_tracer_types::PixelRayTracerContent;
use pixel_ray_tracer_types::{
    PIXEL_RAY_TRACER_SIZE, PixelRayTracerOperation, PixelUpdate, Point, RayEntity, RaySettings,
    Scene,
};

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

struct RayInner {
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
    pointer: Cell<Option<Point>>,
    waker: RefCell<Waker>,
}

#[derive(Clone, Copy)]
pub(crate) struct RayState {
    editor: Editor,
    pub(crate) entities: ReadSignal<Vec<RayEntity>>,
    pub(crate) lighting: Signal<Option<Image>>,
    pub(crate) rays: Signal<Option<Image>>,
    pub(crate) overlay: Signal<Draw>,
    pub(crate) tool: Signal<Tool>,
    pub(crate) color_index: Signal<u8>,
    pub(crate) selected: Signal<Option<u64>>,
    pub(crate) new_light_intensity: Signal<f32>,
    pub(crate) new_surface: Signal<Surface>,
    poked: Signal<u64>,
    inner: Stored<RayInner>,
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
    pub(crate) fn new(editor: Editor) -> Self {
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
        let lighting = create_signal(None);
        let rays = create_signal(None);
        let overlay = create_signal(overlay::draw(Vec::new(), None, Preview::None));
        let tool = create_signal(Tool::Pencil);
        let color_index = create_signal(0);
        let selected = create_signal(None);
        let new_light_intensity = create_signal(2.0);
        let new_surface = create_signal(Surface::default());
        let poked = create_signal(0);
        Self {
            editor,
            entities,
            lighting,
            rays,
            overlay,
            tool,
            color_index,
            selected,
            new_light_intensity,
            new_surface,
            poked,
            inner: store(RayInner {
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
                pointer: Cell::new(None),
                waker: RefCell::new(Waker::default()),
            }),
        }
    }

    pub(crate) fn editor(&self) -> Editor {
        self.editor
    }

    pub(crate) fn settings(&self, view: bool) -> RaySettings {
        let inner = self.inner.value();
        inner.block.read().map_or_else(
            |_| RaySettings::default(),
            |scene| match view {
                true => scene.view_ray_settings(),
                false => scene.lighting_ray_settings(),
            },
        )
    }

    pub(crate) fn set_settings(&self, view: bool, settings: RaySettings) {
        let inner = self.inner.value();
        match view {
            true => {
                inner
                    .block
                    .operate(PixelRayTracerOperation::SetViewRaySettings { settings });
                inner.ray_state.borrow_mut().take();
            }
            false => inner
                .block
                .operate(PixelRayTracerOperation::SetLightingRaySettings { settings }),
        }
    }

    pub(crate) fn choose_tool(&self, tool: Tool) {
        let inner = self.inner.value();
        self.tool.set(tool);
        inner.interaction.borrow_mut().take();
        if tool != Tool::Select {
            self.selected.set(None);
        }
        self.poke();
    }

    fn poke(&self) {
        self.poked.update(|poked| *poked = poked.wrapping_add(1));
    }

    pub(crate) fn choose_color(&self, index: u8) {
        let inner = self.inner.value();
        self.color_index.set(index);
        let Some(mut entity) = self.selected_entity() else {
            return;
        };
        match &mut entity {
            RayEntity::Surface { color_index, .. } | RayEntity::Light { color_index, .. } => {
                *color_index = index;
                inner
                    .block
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
        let inner = self.inner.value();
        inner
            .block
            .operate(PixelRayTracerOperation::UpdateEntity { entity });
    }

    pub(crate) fn delete_selected(&self) {
        let inner = self.inner.value();
        let Some(id) = self.selected.get_untracked() else {
            return;
        };
        self.selected.set(None);
        inner
            .block
            .operate(PixelRayTracerOperation::DeleteEntity { id });
    }

    pub(crate) fn reset(&self) {
        let inner = self.inner.value();
        inner.block.operate(PixelRayTracerOperation::Reset);
        self.selected.set(None);
    }

    pub(crate) fn set_light_intensity(&self, value: f32) {
        self.new_light_intensity.set(value);
    }

    pub(crate) fn set_new_surface(&self, surface: Surface) {
        self.new_surface.set(surface);
    }

    pub(crate) fn hover(&self, at: Option<Point>) {
        let inner = self.inner.value();
        if inner.pointer.replace(at) != at {
            self.poke();
        }
    }

    pub(crate) fn press(&self, at: Point) {
        let inner = self.inner.value();
        let tool = self.tool.get_untracked();
        let snapped = snap(at);
        match tool {
            Tool::Pencil | Tool::PixelLine => {
                if let Some(pixel) = pixel_at(at, false) {
                    *inner.interaction.borrow_mut() = Some(Interaction::Pixels {
                        start: pixel,
                        current: pixel,
                        path: vec![pixel],
                    });
                }
            }
            Tool::Surface | Tool::Water => {
                *inner.interaction.borrow_mut() = Some(Interaction::Entity {
                    start: snapped,
                    current: snapped,
                });
            }
            Tool::Light => {
                if inside(at) {
                    let id = inner.block.read().map_or(1, |scene| scene.next_entity_id());
                    inner.block.operate(PixelRayTracerOperation::AddEntity {
                        entity: RayEntity::Light {
                            id,
                            position: snapped,
                            color_index: self.color_index.get_untracked(),
                            intensity: self.new_light_intensity.get_untracked(),
                        },
                    });
                    self.selected.set(Some(id));
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
        let inner = self.inner.value();
        let tool = self.tool.get_untracked();
        let mut held = inner.interaction.borrow_mut();
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
                    inner
                        .interaction_revision
                        .set(inner.interaction_revision.get().wrapping_add(1));
                }
            }
        }
    }

    pub(crate) fn release(&self) {
        let inner = self.inner.value();
        let Some(interaction) = inner.interaction.borrow_mut().take() else {
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
                inner
                    .block
                    .operate(PixelRayTracerOperation::Paint { pixels });
            }
            Interaction::Entity { start, current } if start != current => {
                let id = inner.block.read().map_or(1, |scene| scene.next_entity_id());
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
                inner
                    .block
                    .operate(PixelRayTracerOperation::AddEntity { entity });
                self.selected.set(Some(id));
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
        let inner = self.inner.value();
        let entities = self.entities.get_untracked();
        if let Some(id) = self.selected.get_untracked()
            && let Some(entity) = entities.iter().find(|entity| entity.id() == id).cloned()
        {
            let tolerance = 3.0;
            match entity {
                RayEntity::Light { position, .. } if distance(at, position) <= tolerance => {
                    *inner.interaction.borrow_mut() = Some(Interaction::Light {
                        id,
                        current: position,
                    });
                    return;
                }
                RayEntity::Surface { start, .. } | RayEntity::Water { start, .. }
                    if distance(at, start) <= tolerance =>
                {
                    *inner.interaction.borrow_mut() = Some(Interaction::Endpoint {
                        id,
                        start: true,
                        current: start,
                    });
                    return;
                }
                RayEntity::Surface { end, .. } | RayEntity::Water { end, .. }
                    if distance(at, end) <= tolerance =>
                {
                    *inner.interaction.borrow_mut() = Some(Interaction::Endpoint {
                        id,
                        start: false,
                        current: end,
                    });
                    return;
                }
                _ => {}
            }
        }
        self.selected
            .set(closest(&entities, at).map(|entity| entity.id()));
    }

    fn drafted(&self) -> Option<RayEntity> {
        let inner = self.inner.value();
        let held = inner.interaction.borrow();
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
        let inner = self.inner.value();
        let held = inner.interaction.borrow();
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

    pub(crate) fn watch(self) {
        let inner = self.inner.value();
        let (waker, woken) = self.editor.woken();
        *inner.waker.borrow_mut() = waker;
        create_effect(move || {
            woken.with(|_| ());
            self.poked.with(|_| ());
            self.entities.with(|_| ());
            self.tool.with(|_| ());
            self.selected.with(|_| ());
            self.color_index.with(|_| ());
            let _ = self.inner.with(|inner| inner.block.content.revision());
            untrack(|| self.settle());
        });
    }

    pub(crate) fn settle(&self) {
        self.settle_lighting();
        self.settle_rays();
        self.settle_overlay();
    }

    fn settle_overlay(&self) {
        let inner = self.inner.value();
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
        if inner.shown_overlay.borrow().as_ref() == Some(&shown) {
            return;
        }
        self.overlay.set_unconditionally(overlay::draw(
            shown.entities.clone(),
            shown.selected,
            shown.preview.clone(),
        ));
        *inner.shown_overlay.borrow_mut() = Some(shown);
    }

    fn settle_lighting(&self) {
        let inner = self.inner.value();
        let Ok(scene) = inner.block.read() else {
            return;
        };
        let key = LightingKey {
            block_revision: scene.lighting_revision(),
            preview_revision: match self.drafted().is_some() {
                true => inner.interaction_revision.get(),
                false => 0,
            },
        };
        self.land_lighting();
        if inner.lighting_key.get() == Some(key) {
            inner.performance.record_count("Lighting cache hits", 1);
            return;
        }
        if inner.lighting_job.borrow().is_some() {
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
        let waker = inner.waker.borrow().clone();
        let (sender, receiver) = mpsc::channel();
        run_job("pixel-ray-tracer-lighting", move || {
            let started = Instant::now();
            let result = raytracer::trace_lighting(&pixels, &entities, settings);
            let _ = sender.send((key, result, started.elapsed()));
            waker.wake();
        });
        *inner.lighting_job.borrow_mut() = Some(receiver);
        inner.performance.record_count("Lighting cache misses", 1);
        self.land_lighting();
    }

    fn land_lighting(&self) {
        let inner = self.inner.value();
        let landed =
            inner
                .lighting_job
                .borrow()
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(Ok(result)),
                    Err(TryRecvError::Disconnected) => Some(Err(())),
                    Err(TryRecvError::Empty) => None,
                });
        if let Some(landed) = landed {
            inner.lighting_job.borrow_mut().take();
            if let Ok((landed_key, pixels, duration)) = landed {
                inner
                    .performance
                    .record_duration("Lighting trace", duration);
                self.lighting.set(Some(image_of(&pixels)));
                *inner.rendered.borrow_mut() = pixels;
                inner.ray_state.borrow_mut().take();
                self.rays.set(None);
                inner.lighting_key.set(Some(landed_key));
            }
        }
    }

    fn settle_rays(&self) {
        let inner = self.inner.value();
        if self.tool.get_untracked() != Tool::RayTrace {
            return;
        }
        let Some(origin) = inner.pointer.get() else {
            return;
        };
        self.land_rays();
        let Ok(scene) = inner.block.read() else {
            return;
        };
        let settings = scene.view_ray_settings();
        let revision = scene.revision();
        let ready = inner.lighting_key.get()
            == Some(LightingKey {
                block_revision: scene.lighting_revision(),
                preview_revision: 0,
            });
        let hit = ready
            && inner.ray_state.borrow().as_ref().is_some_and(|held| {
                distance(held.origin, origin) <= 0.25
                    && held.revision == revision
                    && held.settings == settings
            });
        if hit {
            inner.performance.record_count("View-ray cache hits", 1);
            return;
        }
        if !ready || inner.ray_job.borrow().is_some() {
            return;
        }
        let source = inner.rendered.borrow().clone();
        let entities = scene.entities().to_vec();
        drop(scene);
        let waker = inner.waker.borrow().clone();
        let (sender, receiver) = mpsc::channel();
        run_job("pixel-ray-tracer-view-rays", move || {
            let started = Instant::now();
            let pixels = raytracer::trace_rays(&source, &entities, origin, settings);
            let _ = sender.send((origin, pixels, revision, settings, started.elapsed()));
            waker.wake();
        });
        *inner.ray_job.borrow_mut() = Some(receiver);
        inner.performance.record_count("View-ray cache misses", 1);
        self.land_rays();
    }

    fn land_rays(&self) {
        let inner = self.inner.value();
        let landed =
            inner
                .ray_job
                .borrow()
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(Ok(result)),
                    Err(TryRecvError::Disconnected) => Some(Err(())),
                    Err(TryRecvError::Empty) => None,
                });
        if let Some(landed) = landed {
            inner.ray_job.borrow_mut().take();
            if let Ok((origin, pixels, revision, settings, duration)) = landed {
                inner
                    .performance
                    .record_duration("View-ray trace", duration);
                self.rays.set(Some(image_of(&pixels)));
                *inner.ray_state.borrow_mut() = Some(TracedRays {
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
    content: ContentProjection<PixelRayTracerContent>,
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
    fn new(content: ContentProjection<PixelRayTracerContent>) -> Self {
        Self {
            content,
            cached: RefCell::new(None),
        }
    }

    fn read(&self) -> Ready<Snapshot> {
        let revision = self.content.revision()?;
        let mut cached = self.cached.borrow_mut();
        if let Some(held) = cached.as_ref().filter(|held| held.revision == revision) {
            return Ok(held.clone());
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
        Ok(snapshot)
    }

    fn operate(&self, operation: PixelRayTracerOperation) {
        let Ok(edit) = self
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
