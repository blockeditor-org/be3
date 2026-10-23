use super::*;
use block_editor_beui::beui::styled::Root;
use std::cell::Cell;

use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{Input, NodeId, Pen, PenPhase, PointerButton, Pos2};

use crate::app::actions::canvas_actions;
use crate::app::canvas::CanvasStage;
use crate::app::entities::Entities;
use crate::app::state::{CanvasState, Tool};

thread_local! {
    static ENTITIES: Cell<Option<Entities>> = const { Cell::new(None) };
}

#[component]
fn CataloguedCanvas(editor: block_editor_beui::Editor) -> NodeId {
    let state = CanvasState::new(editor, false);
    state.watch();
    provide_context(canvas_actions(state));
    state.set_tool(Tool::Pen);
    ENTITIES.set(Some(state.entities));
    view! {
        <CanvasStage state={state} />
    }
}

fn pen(editor: &mut BeuiTest<CanvasApp>, phase: PenPhase, pos: Pos2) {
    editor.step(vec![Input::Pen {
        phase,
        pos,
        pen: Pen {
            pressure: 0.5,
            ..Pen::default()
        },
    }]);
}

#[test]
fn a_pen_lift_catalogues_only_the_new_stroke() {
    let mut frame = entity(Uuid::from_u128(1));
    frame.shape = CanvasShape::Frame;
    frame.transform = CanvasTransform::new(
        CanvasPoint::new(0.0, 0.0),
        CanvasPoint::new(400.0, 300.0),
        0.0,
    );
    let shapes = (2..50).map(|index| {
        let mut shape = entity(Uuid::from_u128(index));
        shape.transform = CanvasTransform::new(
            CanvasPoint::new((index % 8) as f32 * 70.0 - 245.0, (index / 8) as f32 * 50.0),
            CanvasPoint::new(60.0, 40.0),
            0.0,
        );
        shape
    });
    let host = EditorHost::default();
    host.set_editable(true);
    let owner = OwnedEditor::new(host, Uuid::new_v4());
    let handle = *owner;
    let mut editor = BeuiTest::<CanvasApp>::with_view(owner, move || {
        view! {
            <Root>
                <Loaded>
                    <CataloguedCanvas editor={handle} />
                </Loaded>
            </Root>
        }
    })
    .in_viewport();
    editor.hold(
        None,
        CanvasContent::new(&Canvas::with_entities(std::iter::once(frame).chain(shapes))),
    );
    editor.run();
    let entities = ENTITIES.get().expect("the canvas was built");
    editor.settle();
    let from = editor.rect_of("infinite-canvas.canvas").center() - Vec2::new(100.0, 60.0);
    pen(&mut editor, PenPhase::Move, from);
    for (phase, at) in [
        (PenPhase::Down(PointerButton::Primary), from),
        (PenPhase::Move, from + Vec2::new(20.0, 5.0)),
        (PenPhase::Move, from + Vec2::new(40.0, 0.0)),
        (
            PenPhase::Up(PointerButton::Primary),
            from + Vec2::new(40.0, 0.0),
        ),
    ] {
        let before = entities.catalogued();
        pen(&mut editor, phase, at);
        editor.run();
        assert!(
            entities.catalogued() - before <= 1,
            "a frame of the stroke catalogued other entities than the stroke"
        );
    }

    assert_eq!(
        super::entities(&editor).len(),
        50,
        "the stroke was not added"
    );
    assert_eq!(
        untrack(|| entities.first_frame()).map(|frame| frame.id),
        Some(Uuid::from_u128(1)),
    );
}
