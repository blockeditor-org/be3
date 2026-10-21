use super::*;
use crate::app::sketch::Sketch;

#[test]
fn a_hand_drawn_shape_is_the_same_for_the_same_seed() {
    for shape in [
        CanvasShape::Rectangle,
        CanvasShape::Ellipse,
        CanvasShape::Diamond,
    ] {
        let mut entity = placed(1, shape, CanvasPoint::new(10.0, 20.0));
        entity.style.look = CanvasLook::HandDrawn;
        entity.style.fill = CanvasFill::Hatched;
        entity.style.seed = 7;
        let drawn = Sketch::of(&entity);
        assert!(!drawn.parts.is_empty(), "{shape:?} draws something");
        assert_eq!(
            drawn,
            Sketch::of(&entity.clone()),
            "{shape:?} is drawn the same again"
        );

        let mut reseeded = entity.clone();
        reseeded.style.seed = 8;
        assert_ne!(
            drawn,
            Sketch::of(&reseeded),
            "another seed draws {shape:?} differently"
        );

        let mut stored = entity.clone();
        stored.text = "a label".into();
        assert_eq!(
            drawn,
            Sketch::of(&stored),
            "a label leaves the {shape:?} as it was"
        );
    }
}
