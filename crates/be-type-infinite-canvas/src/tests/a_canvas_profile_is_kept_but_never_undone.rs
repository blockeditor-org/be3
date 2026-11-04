use super::*;
use crate::CanvasProfile;

#[test]
fn a_canvas_profile_is_kept_but_never_undone() {
    let canvas = CanvasContent::default();
    assert_eq!(canvas.root().profile, CanvasProfile::Prototyping);

    let switch = canvas.root().set_profile(CanvasProfile::Vector);
    assert!(Undo::step(&canvas, &switch).is_none());
    let canvas = edited(&canvas, [switch]);
    assert_eq!(canvas.root().profile, CanvasProfile::Vector);
    assert_eq!(CanvasContent::decode(&canvas.encode()), Ok(canvas.clone()));

    let shape = CanvasEntity::new(CanvasShape::Rectangle, CanvasTransform::default());
    let add = crate::Canvas::edit_in(
        &canvas,
        &InfiniteCanvasOperation::Add {
            entity: Box::new(shape),
            pixels: None,
        },
    );
    let both = Edit(
        add.0
            .iter()
            .cloned()
            .chain(canvas.root().set_profile(CanvasProfile::Prototyping).0)
            .collect(),
    );
    let step = Undo::step(&canvas, &both).expect("adding a shape is undoable");
    let canvas = edited(&canvas, [both]);
    let canvas = edited(&canvas, canvas.revert(&step));
    assert!(canvas.root().entities().is_empty());
    assert_eq!(canvas.root().profile, CanvasProfile::Prototyping);
}
