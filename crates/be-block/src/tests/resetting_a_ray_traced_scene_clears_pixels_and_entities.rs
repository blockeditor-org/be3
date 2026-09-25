use super::*;
use crate::pixel_ray_tracer::{
    PIXEL_RAY_TRACER_BACKGROUND, PixelRayTracerContent, PixelRayTracerOperation, PixelUpdate,
    Point, RayEntity,
};

#[test]
fn resetting_a_ray_traced_scene_clears_pixels_and_entities() {
    let blank = PixelRayTracerContent::default();
    assert!(
        blank
            .root()
            .scene()
            .pixels()
            .iter()
            .all(|pixel| *pixel == PIXEL_RAY_TRACER_BACKGROUND)
    );

    let painted = operated(
        &blank,
        &PixelRayTracerOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 3,
                y: 1,
                color_index: 2,
            }],
        },
    );
    let lit = operated(
        &painted,
        &PixelRayTracerOperation::AddEntity {
            entity: RayEntity::Light {
                id: 1,
                position: Point { x: 10.0, y: 10.0 },
                color_index: 1,
                intensity: 2.0,
            },
        },
    );
    let scene = lit.root().scene();
    assert_eq!(scene.pixels()[128 + 3], 2);
    assert_eq!(scene.entities().len(), 1);
    assert_eq!(scene.next_entity_id(), 2);

    let reset = operated(&lit, &PixelRayTracerOperation::Reset);
    let scene = reset.root().scene();
    assert!(
        scene
            .pixels()
            .iter()
            .all(|pixel| *pixel == PIXEL_RAY_TRACER_BACKGROUND)
    );
    assert!(scene.entities().is_empty());
    assert!(
        reset
            .root()
            .edit_for(&PixelRayTracerOperation::Reset)
            .0
            .is_empty()
    );
}

fn operated(
    content: &PixelRayTracerContent,
    operation: &PixelRayTracerOperation,
) -> PixelRayTracerContent {
    edited(content, [content.root().edit_for(operation)])
}
