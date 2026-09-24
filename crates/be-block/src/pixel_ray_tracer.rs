use be_model::{Bounds, Document, Edit, Grid, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

pub const PIXEL_RAY_TRACER_SIZE: u16 = 128;
pub const PIXEL_RAY_TRACER_BACKGROUND: u8 = 7;
pub const PIXEL_RAY_TRACER_PALETTE: [[u8; 3]; 16] = [
    [0x00, 0x00, 0x00],
    [0x1d, 0x2b, 0x53],
    [0x7e, 0x25, 0x53],
    [0x00, 0x87, 0x51],
    [0xab, 0x52, 0x36],
    [0x5f, 0x57, 0x4f],
    [0xc2, 0xc3, 0xc7],
    [0xff, 0xf1, 0xe8],
    [0xff, 0x00, 0x4d],
    [0xff, 0xa3, 0x00],
    [0xff, 0xec, 0x27],
    [0x00, 0xe4, 0x36],
    [0x29, 0xad, 0xff],
    [0x83, 0x76, 0x9c],
    [0xff, 0x77, 0xa8],
    [0xff, 0xcc, 0xaa],
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RayEntity {
    Surface {
        id: u64,
        start: Point,
        end: Point,
        color_index: u8,
        roughness: f32,
        metalness: f32,
        transmission: f32,
        refractive_index: f32,
    },
    Water {
        id: u64,
        start: Point,
        end: Point,
    },
    Light {
        id: u64,
        position: Point,
        color_index: u8,
        intensity: f32,
    },
}

impl RayEntity {
    pub const fn id(&self) -> u64 {
        match self {
            Self::Surface { id, .. } | Self::Water { id, .. } | Self::Light { id, .. } => *id,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct RaySettings {
    pub ray_count: u16,
    pub step_distance: f32,
    pub maximum_steps: u16,
}

impl Default for RaySettings {
    fn default() -> Self {
        Self {
            ray_count: 800,
            step_distance: 0.5,
            maximum_steps: 512,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PixelUpdate {
    pub x: u16,
    pub y: u16,
    pub color_index: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum PixelRayTracerOperation {
    Paint { pixels: Vec<PixelUpdate> },
    AddEntity { entity: RayEntity },
    UpdateEntity { entity: RayEntity },
    DeleteEntity { id: u64 },
    SetViewRaySettings { settings: RaySettings },
    SetLightingRaySettings { settings: RaySettings },
    Reset,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct RayScene {
    pub pixels: Grid<[u8; 1]>,
    pub entities: Map<u64, RayEntity>,
    pub view_ray_settings: Option<RaySettings>,
    pub lighting_ray_settings: Option<RaySettings>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pixels: Vec<u8>,
    entities: Vec<RayEntity>,
    view_ray_settings: RaySettings,
    lighting_ray_settings: RaySettings,
}

impl Scene {
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn entities(&self) -> &[RayEntity] {
        &self.entities
    }

    pub const fn view_ray_settings(&self) -> RaySettings {
        self.view_ray_settings
    }

    pub const fn lighting_ray_settings(&self) -> RaySettings {
        self.lighting_ray_settings
    }

    pub fn next_entity_id(&self) -> u64 {
        self.entities.iter().map(RayEntity::id).max().unwrap_or(0) + 1
    }
}

const fn stored(color_index: u8) -> [u8; 1] {
    [color_index ^ PIXEL_RAY_TRACER_BACKGROUND]
}

fn bounds() -> Bounds {
    let side = u32::from(PIXEL_RAY_TRACER_SIZE);
    Bounds::new(0, 0, side, side)
}

impl RayScene {
    pub fn scene(&self) -> Scene {
        let area = bounds().area();
        let pixels = match self.pixels.bounds() == bounds() {
            true => self
                .pixels
                .bytes()
                .iter()
                .map(|byte| byte ^ PIXEL_RAY_TRACER_BACKGROUND)
                .collect(),
            false => vec![PIXEL_RAY_TRACER_BACKGROUND; area],
        };
        Scene {
            pixels,
            entities: self.entities.values().cloned().collect(),
            view_ray_settings: self.view_ray_settings.unwrap_or_default(),
            lighting_ray_settings: self.lighting_ray_settings.unwrap_or_default(),
        }
    }

    pub fn edit_for(&self, operation: &PixelRayTracerOperation) -> Edit {
        let mut changes = Vec::new();
        if self.pixels.bounds() != bounds() {
            changes.push(Self::PIXELS.reshape(ObjectId::ROOT, bounds()));
        }
        let before = changes.len();
        match operation {
            PixelRayTracerOperation::Paint { pixels } => {
                let scene = self.scene();
                let side = usize::from(PIXEL_RAY_TRACER_SIZE);
                let cells: Vec<(i32, i32, [u8; 1])> = pixels
                    .iter()
                    .filter(|update| {
                        update.x < PIXEL_RAY_TRACER_SIZE
                            && update.y < PIXEL_RAY_TRACER_SIZE
                            && usize::from(update.color_index) < PIXEL_RAY_TRACER_PALETTE.len()
                            && scene.pixels[usize::from(update.y) * side + usize::from(update.x)]
                                != update.color_index
                    })
                    .map(|update| {
                        (
                            i32::from(update.x),
                            i32::from(update.y),
                            stored(update.color_index),
                        )
                    })
                    .collect();
                if !cells.is_empty() {
                    changes.push(Self::PIXELS.paint(ObjectId::ROOT, cells));
                }
            }
            PixelRayTracerOperation::AddEntity { entity } => {
                if valid_entity(entity) && !self.entities.contains_key(&entity.id()) {
                    changes.push(Self::ENTITIES.put(ObjectId::ROOT, &entity.id(), Some(entity)));
                }
            }
            PixelRayTracerOperation::UpdateEntity { entity } => {
                if valid_entity(entity)
                    && self
                        .entities
                        .get(&entity.id())
                        .is_some_and(|current| current != entity)
                {
                    changes.push(Self::ENTITIES.put(ObjectId::ROOT, &entity.id(), Some(entity)));
                }
            }
            PixelRayTracerOperation::DeleteEntity { id } => {
                if self.entities.contains_key(id) {
                    changes.push(Self::ENTITIES.put(ObjectId::ROOT, id, None));
                }
            }
            PixelRayTracerOperation::SetViewRaySettings { settings } => {
                if valid_settings(*settings) && self.view_ray_settings != Some(*settings) {
                    changes.push(Self::VIEW_RAY_SETTINGS.set(ObjectId::ROOT, &Some(*settings)));
                }
            }
            PixelRayTracerOperation::SetLightingRaySettings { settings } => {
                if valid_settings(*settings) && self.lighting_ray_settings != Some(*settings) {
                    changes.push(Self::LIGHTING_RAY_SETTINGS.set(ObjectId::ROOT, &Some(*settings)));
                }
            }
            PixelRayTracerOperation::Reset => {
                let cells: Vec<(i32, i32, [u8; 1])> = bounds()
                    .points()
                    .filter(|(x, y)| self.pixels.get(*x, *y).is_some_and(|cell| cell != [0]))
                    .map(|(x, y)| (x, y, [0]))
                    .collect();
                if !cells.is_empty() {
                    changes.push(Self::PIXELS.paint(ObjectId::ROOT, cells));
                }
                for id in self.entities.keys() {
                    changes.push(Self::ENTITIES.put(ObjectId::ROOT, id, None));
                }
            }
        }
        if changes.len() == before {
            changes.clear();
        }
        Edit(changes)
    }
}

impl Root for RayScene {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7069_7865_6c2d_7261_7974_7261_6365_7201);
}

pub type PixelRayTracerContent = Document<RayScene>;

fn valid_settings(settings: RaySettings) -> bool {
    (1..=2048).contains(&settings.ray_count)
        && (1..=2048).contains(&settings.maximum_steps)
        && settings.step_distance.is_finite()
        && (0.05..=128.0).contains(&settings.step_distance)
}

fn valid_point(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn valid_entity(entity: &RayEntity) -> bool {
    match entity {
        RayEntity::Surface {
            start,
            end,
            color_index,
            roughness,
            metalness,
            transmission,
            refractive_index,
            ..
        } => {
            valid_point(*start)
                && valid_point(*end)
                && usize::from(*color_index) < PIXEL_RAY_TRACER_PALETTE.len()
                && roughness.is_finite()
                && (0.0..=1.0).contains(roughness)
                && metalness.is_finite()
                && (0.0..=1.0).contains(metalness)
                && transmission.is_finite()
                && (0.0..=1.0).contains(transmission)
                && refractive_index.is_finite()
                && (1.0..=3.0).contains(refractive_index)
        }
        RayEntity::Water { start, end, .. } => valid_point(*start) && valid_point(*end),
        RayEntity::Light {
            position,
            color_index,
            intensity,
            ..
        } => {
            valid_point(*position)
                && usize::from(*color_index) < PIXEL_RAY_TRACER_PALETTE.len()
                && intensity.is_finite()
                && (0.1..=8.0).contains(intensity)
        }
    }
}
