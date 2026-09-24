use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PixelRayTracer;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PixelRayTracerOperation {}

impl Block for PixelRayTracer {
    type Operation = PixelRayTracerOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7069_7865_6c2d_7261_7974_7261_6365_7201);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}
