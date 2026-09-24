use std::collections::HashSet;

use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BlockRef, ChildChange, Root};

pub const MAX_LATITUDE: f64 = 85.051_128_78;

pub const MIN_REGION_SPAN: f64 = 0.000_01;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct MapCoordinate {
    pub longitude: f64,
    pub latitude: f64,
}

impl MapCoordinate {
    pub const fn new(longitude: f64, latitude: f64) -> Self {
        Self {
            longitude,
            latitude,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MapRegion {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl MapRegion {
    pub const fn new(west: f64, south: f64, east: f64, north: f64) -> Self {
        Self {
            west,
            south,
            east,
            north,
        }
    }

    pub const WORLD: Self = Self::new(-180.0, -MAX_LATITUDE, 180.0, MAX_LATITUDE);

    pub fn center(self) -> MapCoordinate {
        MapCoordinate::new(
            (self.west + self.east) * 0.5,
            (self.south + self.north) * 0.5,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MapColor {
    #[default]
    Default,
    Rgb {
        red: u8,
        green: u8,
        blue: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapPoint {
    pub id: Uuid,
    pub block_id: BlockRef,
    pub position: MapCoordinate,
    pub color: MapColor,
}

impl MapPoint {
    pub fn new(block_id: BlockRef, position: MapCoordinate) -> Self {
        Self {
            id: Uuid::new_v4(),
            block_id,
            position,
            color: MapColor::Default,
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Map {
    pub places: List<MapPlace>,
    pub preview_region: Option<MapRegion>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct MapPlace {
    pub block: Option<BlockRef>,
    pub position: MapCoordinate,
    pub color: MapColor,
}

impl Map {
    pub fn points(&self) -> Vec<MapPoint> {
        self.places
            .iter()
            .filter_map(|place| {
                Some(MapPoint {
                    id: place.id.as_uuid(),
                    block_id: place.block?,
                    position: place.position,
                    color: place.color,
                })
            })
            .collect()
    }

    pub fn point(&self, id: Uuid) -> Option<MapPoint> {
        self.points().into_iter().find(|point| point.id == id)
    }

    pub fn displayed_region(&self) -> MapRegion {
        self.preview_region.unwrap_or(MapRegion::WORLD)
    }

    pub fn add(point: &MapPoint) -> Edit {
        let point = normalized_point(*point);
        Self::PLACES
            .insert_as(
                ObjectId::from_uuid(point.id),
                ObjectId::ROOT,
                Anchor::End,
                &MapPlace {
                    block: Some(point.block_id),
                    position: point.position,
                    color: point.color,
                },
            )
            .into()
    }

    pub fn update(points: &[MapPoint]) -> Edit {
        points
            .iter()
            .flat_map(|point| {
                let point = normalized_point(*point);
                let id = ObjectId::from_uuid(point.id);
                [
                    MapPlace::BLOCK.set(id, &Some(point.block_id)),
                    MapPlace::POSITION.set(id, &point.position),
                    MapPlace::COLOR.set(id, &point.color),
                ]
            })
            .collect()
    }

    pub fn remove(ids: &[Uuid]) -> Edit {
        ids.iter()
            .map(|id| Change::remove(ObjectId::from_uuid(*id)))
            .collect()
    }

    pub fn set_preview_region(region: Option<MapRegion>) -> Edit {
        Self::PREVIEW_REGION
            .set(ObjectId::ROOT, &region.map(normalized_region))
            .into()
    }

    fn showing(&self, block: Uuid) -> Vec<MapPoint> {
        self.points()
            .into_iter()
            .filter(|point| point.block_id == BlockRef::Direct(block))
            .collect()
    }
}

impl Root for Map {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6d61_702d_636f_6e74_656e_742d_7479_0002);

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.points()
            .into_iter()
            .filter_map(|point| point.block_id.as_direct())
            .filter(|block| seen.insert(*block))
            .collect()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        Some(match change {
            ChildChange::Add(block) => {
                if !self.showing(block).is_empty() {
                    return Some(Edit::default());
                }
                Self::add(&MapPoint::new(
                    BlockRef::Direct(block),
                    self.displayed_region().center(),
                ))
            }
            ChildChange::Delete(block) => {
                let ids: Vec<Uuid> = self.showing(block).iter().map(|point| point.id).collect();
                Self::remove(&ids)
            }
            ChildChange::Replace { old, new } => {
                let points: Vec<MapPoint> = self
                    .showing(old)
                    .into_iter()
                    .map(|point| MapPoint {
                        block_id: BlockRef::Direct(new),
                        ..point
                    })
                    .collect();
                Self::update(&points)
            }
        })
    }
}

pub type MapContent = Document<Map>;

fn normalized_point(mut point: MapPoint) -> MapPoint {
    point.position.longitude = clamp_finite(point.position.longitude, -180.0, 180.0);
    point.position.latitude = clamp_finite(point.position.latitude, -MAX_LATITUDE, MAX_LATITUDE);
    point
}

fn normalized_region(region: MapRegion) -> MapRegion {
    let west = clamp_finite(region.west, -180.0, 180.0);
    let east = clamp_finite(region.east, -180.0, 180.0);
    let south = clamp_finite(region.south, -MAX_LATITUDE, MAX_LATITUDE);
    let north = clamp_finite(region.north, -MAX_LATITUDE, MAX_LATITUDE);
    let (west, east) = ordered_span(west, east, -180.0, 180.0);
    let (south, north) = ordered_span(south, north, -MAX_LATITUDE, MAX_LATITUDE);
    MapRegion::new(west, south, east, north)
}

fn ordered_span(low: f64, high: f64, limit_low: f64, limit_high: f64) -> (f64, f64) {
    let (mut low, mut high) = (low.min(high), low.max(high));
    if high - low >= MIN_REGION_SPAN {
        return (low, high);
    }
    let center = (low + high) * 0.5;
    low = center - MIN_REGION_SPAN * 0.5;
    high = center + MIN_REGION_SPAN * 0.5;
    if low < limit_low {
        (limit_low, limit_low + MIN_REGION_SPAN)
    } else if high > limit_high {
        (limit_high - MIN_REGION_SPAN, limit_high)
    } else {
        (low, high)
    }
}

fn clamp_finite(value: f64, low: f64, high: f64) -> f64 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        0.0
    }
}
