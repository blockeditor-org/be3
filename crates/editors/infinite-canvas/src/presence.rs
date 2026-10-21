use std::collections::{BTreeMap, HashSet};

use block_editor_beui::SharedCaret;
use block_editor_beui::be_block::canvas::{CanvasEntity, CanvasPoint, CanvasStyle};
use block_editor_beui::be_block::presence::{PresenceColor, PresenceKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CANVAS_CURSOR: Uuid = Uuid::from_u128(0x6361_6e76_6173_5f63_7572_736f_725f_5f5f);
const CANVAS_DRAFT: Uuid = Uuid::from_u128(0x6361_6e76_6173_5f64_7261_6674_0000_0000);

pub(crate) const STROKE_PART: usize = 32;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasCursor {
    pub pointer: Option<CanvasPoint>,
    pub selection: Vec<Uuid>,
    pub caret: Option<CanvasCaret>,
    pub color: PresenceColor,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CanvasCaret {
    pub entity: Uuid,
    pub caret: SharedCaret,
}

impl PresenceKind for CanvasCursor {
    const ID: Uuid = CANVAS_CURSOR;
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum CanvasDraft {
    Shape(Box<CanvasEntity>),
    Change {
        entity: Box<CanvasEntity>,
        base: u64,
    },
    Stroke {
        id: Uuid,
        part: u16,
        style: Option<CanvasStyle>,
        points: Vec<[f32; 2]>,
        pressures: Vec<f32>,
    },
}

impl PresenceKind for CanvasDraft {
    const ID: Uuid = CANVAS_DRAFT;
    const SLOTS: u16 = 128;
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Draft {
    Stroke {
        id: Uuid,
        points: Vec<CanvasPoint>,
        pressures: Vec<f32>,
        style: CanvasStyle,
    },
    Shape(Box<CanvasEntity>),
    Change {
        entity: Box<CanvasEntity>,
        base: u64,
    },
}

impl Draft {
    pub(crate) fn id(&self) -> Uuid {
        match self {
            Self::Stroke { id, .. } => *id,
            Self::Shape(entity) | Self::Change { entity, .. } => entity.id,
        }
    }

    fn key(&self) -> (Uuid, u64) {
        match self {
            Self::Change { base, .. } => (self.id(), *base),
            _ => (self.id(), 0),
        }
    }
}

impl CanvasDraft {
    pub(crate) fn id(&self) -> Uuid {
        match self {
            Self::Stroke { id, .. } => *id,
            Self::Shape(entity) | Self::Change { entity, .. } => entity.id,
        }
    }
}

pub(crate) fn fingerprint(entity: &CanvasEntity) -> u64 {
    struct Fnv(u64);
    impl std::io::Write for Fnv {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            for byte in bytes {
                self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hash = Fnv(0xcbf2_9ce4_8422_2325);
    serde_json::to_writer(&mut hash, entity).expect("an entity serializes");
    hash.0
}

pub(crate) fn stroke_parts(
    id: Uuid,
    style: CanvasStyle,
    points: &[CanvasPoint],
    pressures: &[f32],
    from: usize,
) -> Vec<CanvasDraft> {
    let parts = points
        .len()
        .div_ceil(STROKE_PART)
        .min(CanvasDraft::SLOTS as usize);
    (from..parts)
        .map(|part| {
            let range = part * STROKE_PART..((part + 1) * STROKE_PART).min(points.len());
            let pressures = match pressures.len() == points.len() {
                true => pressures[range.clone()].to_vec(),
                false => Vec::new(),
            };
            CanvasDraft::Stroke {
                id,
                part: part as u16,
                style: (part == 0).then_some(style),
                points: points[range].iter().map(|at| [at.x, at.y]).collect(),
                pressures,
            }
        })
        .collect()
}

pub(crate) fn assemble(peers: &[(u64, CanvasDraft)]) -> Vec<(u64, Draft)> {
    let mut by_client: BTreeMap<u64, Vec<&CanvasDraft>> = BTreeMap::new();
    for (client, draft) in peers {
        by_client.entry(*client).or_default().push(draft);
    }
    by_client
        .into_iter()
        .flat_map(|(client, drafts)| {
            assemble_one(&drafts)
                .into_iter()
                .map(move |draft| (client, draft))
        })
        .collect()
}

fn assemble_one(drafts: &[&CanvasDraft]) -> Vec<Draft> {
    let entities: Vec<Draft> = drafts
        .iter()
        .filter_map(|draft| match draft {
            CanvasDraft::Shape(entity) => Some(Draft::Shape(entity.clone())),
            CanvasDraft::Change { entity, base } => Some(Draft::Change {
                entity: entity.clone(),
                base: *base,
            }),
            CanvasDraft::Stroke { .. } => None,
        })
        .collect();
    if !entities.is_empty() {
        return entities;
    }
    assemble_stroke(drafts).into_iter().collect()
}

fn assemble_stroke(drafts: &[&CanvasDraft]) -> Option<Draft> {
    let (id, style) = drafts.iter().find_map(|draft| match draft {
        CanvasDraft::Stroke {
            id,
            style: Some(style),
            ..
        } => Some((*id, *style)),
        _ => None,
    })?;
    let mut parts: Vec<_> = drafts
        .iter()
        .filter_map(|draft| match draft {
            CanvasDraft::Stroke {
                id: of,
                part,
                points,
                pressures,
                ..
            } if *of == id => Some((*part as usize, points, pressures)),
            _ => None,
        })
        .collect();
    parts.sort_by_key(|(part, ..)| *part);
    let mut points = Vec::new();
    let mut pressures = Vec::new();
    let mut pressed = true;
    for (expected, (part, held, held_pressures)) in parts.into_iter().enumerate() {
        if part != expected {
            break;
        }
        pressed &= held_pressures.len() == held.len();
        points.extend(held.iter().map(|[x, y]| CanvasPoint::new(*x, *y)));
        pressures.extend_from_slice(held_pressures);
    }
    if !pressed {
        pressures.clear();
    }
    Some(Draft::Stroke {
        id,
        points,
        pressures,
        style,
    })
}

pub(crate) fn unlanded(
    drafts: Vec<(u64, Draft)>,
    landed_here: impl Fn(&Draft) -> bool,
    landed: &mut HashSet<(Uuid, u64)>,
) -> Vec<(u64, Draft)> {
    let drafted: HashSet<(Uuid, u64)> = drafts.iter().map(|(_, draft)| draft.key()).collect();
    landed.retain(|key| drafted.contains(key));
    drafts
        .into_iter()
        .filter(|(_, draft)| {
            let key = draft.key();
            if landed_here(draft) {
                landed.insert(key);
            }
            !landed.contains(&key)
        })
        .collect()
}

#[cfg(test)]
mod tests;
